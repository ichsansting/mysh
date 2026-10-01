function terraform-cat --description 'cat a .tf file with local/var/resource/module/data references replaced by their resolved values (via terraform console), highlighted with bat if available'
    set -l file $argv[1]
    # Remaining args are passed straight through to terraform console, e.g.
    # -var-file=foo.tfvars (repeatable) or -var=key=value — needed so var.*
    # with no default resolves instead of leaving the whole batch unknown.
    set -l console_args $argv[2..-1]

    if test -z "$file"
        echo "usage: terraform-cat <file.tf> [-var-file=foo.tfvars ...]" >&2
        return 1
    end

    if not test -f "$file"
        echo "terraform-cat: file not found: $file" >&2
        return 1
    end

    set -l dir (dirname $file)

    # Valid reference prefixes declared anywhere in the module (not just this file,
    # since references commonly cross files): resource type+name, data type+name,
    # module name, variable name, local name.
    set -l declared_prefixes
    for decl in (grep -hoE '^\s*(resource|data)\s+"[A-Za-z0-9_]+"\s+"[A-Za-z0-9_]+"|^\s*(module|variable)\s+"[A-Za-z0-9_]+"' $dir/*.tf | sort -u)
        set -l words (string split ' ' -- (string trim $decl))
        switch $words[1]
            case module
                set -a declared_prefixes "module."(string trim -c '"' -- $words[2])
            case variable
                set -a declared_prefixes "var."(string trim -c '"' -- $words[2])
            case data
                set -a declared_prefixes "data."(string trim -c '"' -- $words[2])"."(string trim -c '"' -- $words[3])
            case '*'
                set -a declared_prefixes (string trim -c '"' -- $words[2])"."(string trim -c '"' -- $words[3])
        end
    end

    # locals { key = ... } keys aren't string-literal block headers, so they need
    # brace-depth tracking instead: only lines at depth 1 inside a `locals {` block.
    for name in (awk '
        /^[ \t]*locals[ \t]*{/ { in_locals=1; depth=1; next }
        in_locals {
            # Check depth as it is at the start of the line (the level a key
            # is declared at), before this lines own braces open or close a
            # nested block, otherwise a key like x = jsonencode({ is already
            # counted at depth 2 by the time it is tested, since its own
            # opening brace bumped depth first.
            if (depth == 1 && /^[ \t]*[A-Za-z_][A-Za-z0-9_]*[ \t]*=/) {
                line = $0
                sub(/^[ \t]*/, "", line)
                sub(/[ \t]*=.*/, "", line)
                print line
            }
            n_open = gsub(/{/, "{"); depth += n_open
            n_close = gsub(/}/, "}"); depth -= n_close
            if (depth == 0) { in_locals=0 }
        }
    ' $dir/*.tf | sort -u)
        set -a declared_prefixes "local.$name"
    end

    # Candidate traversals: identifier(.identifier|[index])+ — covers local.x, var.x,
    # module.x.y, data.type.name.attr, and resource_type.name.attr alike. Comments are
    # stripped first so a reference mentioned only in a commented-out line (which may
    # not actually be declared) doesn't get treated as a real candidate.
    set -l code_only (string replace --regex '#.*$' '' -- (cat $file))
    set -l candidates (string match --all --regex '\b[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*|\[[0-9]+\])+' -- $code_only | sort -u)

    set -l valid_refs
    for candidate in $candidates
        for prefix in $declared_prefixes
            if test "$candidate" = "$prefix"; or string match -q "$prefix.*" -- $candidate
                set -a valid_refs $candidate
                break
            end
        end
    end

    set -l refs
    set -l resolved
    if test (count $valid_refs) -gt 0
        # jsonencode() wraps each ref so terraform console's pretty-printed
        # HCL list keeps one element per line as a self-contained,
        # single-line JSON string — a value with a literal newline (e.g.
        # rendered container_definitions JSON) stays escaped as "\n" inside
        # that string, so it can't be mistaken for a line break and desync
        # later lines. Deliberately not wrapping the whole list in its own
        # jsonencode(): that would force Terraform to evaluate the list as a
        # single expression, and one unknown ref would make the entire
        # expression unknown. Left as a bare list literal, Terraform instead
        # prints each element independently, so an unknown ref shows up as
        # its own "(known after apply)" line without blocking the rest.
        set -l wrapped
        for ref in $valid_refs
            set -a wrapped "jsonencode($ref)"
        end
        set -l batch_expr "[" (string join ', ' $wrapped) "]"

        # Single pass, no JSON parser: each element is already on its own
        # line, so stripping the leading "[", trailing "]", indentation, and
        # trailing comma recovers it verbatim. "(known after apply)" becomes
        # a sentinel that can't collide with real jsonencode() output (which
        # always starts with a quote), marking that one ref as unresolved
        # without discarding the rest. The lock notice is filtered here
        # (not relied on from -o/2>/dev/null alone) because terraform
        # console has been observed printing it to stdout, not just stderr,
        # when a previous call's lock hadn't fully cleared.
        # awk isolates each pretty-printed element back into a JSON array
        # literal (quoting the "(known after apply)" sentinel so the result
        # is valid JSON even with unresolved refs mixed in), then jq's join
        # concatenates each element's own string *value* with "\x1f" between
        # them. Each value is exactly the jsonencode(ref) text terraform
        # console printed for that ref (quotes and internal escaping intact)
        # — already the literal HCL text main.tf should show, not something
        # to decode further.
        set -l vals_joined (echo (string join '' $batch_expr) | terraform console $console_args 2>/dev/null | awk '
            BEGIN { out = ""; n = 0 }
            /^(Acquiring|Releasing) state lock/ { next }
            NR == 1 || /^\]/ { next }
            {
                line = $0
                sub(/^[ \t]*/, "", line)
                sub(/,[ \t]*$/, "", line)
                if (line == "") { next }
                if (line == "(known after apply)") { line = "\"\\u0002UNRESOLVED\\u0002\"" }
                if (n > 0) { out = out "," }
                out = out line
                n++
            }
            END { printf "[%s]", out }
        ' | jq -j '[.[]] | join("\u001f")' | string collect)

        if test -n "$vals_joined"
            set refs $valid_refs
            set resolved (string split \x1f -- $vals_joined)
        end
    end

    set -l rendered
    if test (count $refs) -gt 0
        # Single-pass substitution: scan each line for identifier-like tokens
        # ([A-Za-z0-9_.]+) and replace only the ones that are an exact match in
        # the ref table, appending to an output buffer that is never re-scanned.
        # This is what makes it single-pass — a resolved value that happens to
        # contain another ref's name can't trigger a second, unwanted
        # substitution the way sequential `string replace` calls did.
        # string collect after each join prevents fish's command substitution
        # from re-splitting on a literal newline inside a multiline resolved
        # value (e.g. a rendered container_definitions JSON blob) — without
        # it, that single joined string would come back as multiple fish
        # array elements and desync the "\x1f"-delimited awk split below.
        set -l refs_joined (string join \x1f -- $refs | string collect)
        set -l vals_joined (string join \x1f -- $resolved | string collect)

        # refs/vals travel as the first two lines of awk's stdin, not via -v:
        # awk -v unconditionally unescapes backslash sequences in the
        # assigned value (POSIX behavior), which would corrupt a resolved
        # value that is itself JSON text containing literal \" or \\ — those
        # need to reach the lookup table byte-for-byte.
        set rendered (begin
                echo $refs_joined
                echo $vals_joined
                cat $file
            end | awk '
            NR == 1 { n = split($0, refs, "\x1f"); next }
            NR == 2 { split($0, vals, "\x1f"); for (i = 1; i <= n; i++) table[refs[i]] = vals[i]; next }
            {
                rest = $0; out = ""
                while (match(rest, /[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*|\[[0-9]+\])+/) > 0) {
                    tok = substr(rest, RSTART, RLENGTH)
                    out = out substr(rest, 1, RSTART - 1)
                    # A ref that terraform console reported as unresolved is
                    # left as its own literal text (e.g. var.expiration_days)
                    # instead of substituted, that is the partial-resolve
                    # behavior: an unresolved ref does not block every other
                    # ref in the file from resolving.
                    if (tok in table && table[tok] != "\x02UNRESOLVED\x02") {
                        out = out table[tok]
                    } else {
                        out = out tok
                    }
                    rest = substr(rest, RSTART + RLENGTH)
                }
                out = out rest
                while ((pos = index(out, "${\"")) > 0) {
                    close_pos = index(substr(out, pos), "\"}")
                    if (close_pos == 0) break
                    inner = substr(out, pos + 3, close_pos - 4)
                    out = substr(out, 1, pos - 1) inner substr(out, pos + close_pos + 1)
                }
                print out
            }
        ')
    else
        set rendered (cat $file)
    end

    if type -q bat
        printf '%s\n' $rendered | bat --language hcl --paging auto --style plain
    else
        printf '%s\n' $rendered
    end
end
