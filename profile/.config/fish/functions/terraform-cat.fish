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
        # jsonencode() each value so the batch result is one JSON-escaped line per
        # ref (no literal newlines from values like rendered container_definitions,
        # which would otherwise desync the line-per-array-element assumption below).
        set -l wrapped
        for ref in $valid_refs
            set -a wrapped "jsonencode($ref)"
        end
        set -l batch_expr "[" (string join ', ' $wrapped) "]"
        set -l batch_out (echo (string join '' $batch_expr) | terraform console 2>/dev/null)
        if test (count $batch_out) -gt 2
            set refs $valid_refs
            set resolved
            for line in (string trim -c ', ' -- $batch_out[2..-2])
                set -a resolved (string unescape --style=script -- $line)
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
        set -l refs_joined (string join \x1f -- $refs)
        set -l vals_joined (string join \x1f -- $resolved)

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
