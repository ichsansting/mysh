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
        set -l batch_expr "[" (string join ', ' $valid_refs) "]"
        set -l batch_out (echo (string join '' $batch_expr) | terraform console 2>/dev/null)
        if test (count $batch_out) -gt 2
            set refs $valid_refs
            set resolved (string trim -c ', ' -- $batch_out[2..-2])
        end
    end

    set -l rendered (
        while read -l line
            for i in (seq (count $refs))
                set line (string replace --all --regex "\b"(string escape --style=regex -- $refs[$i])"\b" "$resolved[$i]" -- "$line")
            end
            # A substituted reference inside string interpolation, e.g.
            # "prefix-${var.x}", becomes "prefix-${"value"}" — collapse the
            # now-redundant ${"..."} wrapper into the surrounding string literal.
            set line (string replace --all --regex '\$\{"([^"]*)"\}' '$1' -- "$line")
            echo $line
        end < $file
    )

    if type -q bat
        printf '%s\n' $rendered | bat --language hcl --paging auto --style plain
    else
        printf '%s\n' $rendered
    end
end
