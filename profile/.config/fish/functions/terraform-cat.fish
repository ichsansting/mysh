function terraform-cat --description 'cat a .tf file with local/var/resource/module/data references replaced by their resolved values (via terraform console), highlighted with bat if available'
    set -l file $argv[1]

    if test -z "$file"
        echo "usage: terraform-cat <file.tf>" >&2
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
            n_open = gsub(/{/, "{"); depth += n_open
            n_close = gsub(/}/, "}"); depth -= n_close
            if (depth == 0) { in_locals=0; next }
            if (depth == 1 && /^[ \t]*[A-Za-z_][A-Za-z0-9_]*[ \t]*=/) {
                line = $0
                sub(/^[ \t]*/, "", line)
                sub(/[ \t]*=.*/, "", line)
                print line
            }
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
        # One terraform console call per ref, not a single batched array: a
        # single unknown value (e.g. an unset var with no default) makes an
        # entire HCL list expression unknown, which previously desynced the
        # line-per-array-element parsing for every other ref in the batch —
        # and a resolved value containing a literal newline (e.g. a rendered
        # container_definitions JSON blob) would split into multiple fish
        # array elements via command substitution, shifting every index after
        # it. Querying refs individually sidesteps both failure modes.
        set refs $valid_refs
        for ref in $refs
            set -l out (echo "jsonencode($ref)" | terraform console 2>/dev/null | string collect)
            if string match -q '*known after apply*' -- $out
                set -a resolved "(known after apply)"
            else
                # string collect keeps a multiline unescaped value as one
                # element — without it, fish's command substitution would
                # split on the literal newlines and shift every later index.
                set -a resolved (string unescape --style=script -- $out | string collect)
            end
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

        set rendered (awk -v refs_joined="$refs_joined" -v vals_joined="$vals_joined" '
            BEGIN {
                n = split(refs_joined, refs, "\x1f")
                split(vals_joined, vals, "\x1f")
                for (i = 1; i <= n; i++) table[refs[i]] = vals[i]
            }
            {
                rest = $0; out = ""
                while (match(rest, /[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*|\[[0-9]+\])+/) > 0) {
                    tok = substr(rest, RSTART, RLENGTH)
                    out = out substr(rest, 1, RSTART - 1)
                    out = out ((tok in table) ? table[tok] : tok)
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
        ' $file)
    else
        set rendered (cat $file)
    end

    if type -q bat
        printf '%s\n' $rendered | bat --language hcl --paging auto --style plain
    else
        printf '%s\n' $rendered
    end
end
