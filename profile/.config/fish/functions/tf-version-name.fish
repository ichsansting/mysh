function tf-version-name --description 'Resolve exact Terraform version from .terraform-version (supports latest, latest:<regex>)'
    set -l requested $TFENV_TERRAFORM_VERSION

    if test -z "$requested"
        set -l dir (pwd)
        set -l file ""
        while test "$dir" != "/"
            if test -f "$dir/.terraform-version"
                set file "$dir/.terraform-version"
                break
            end
            set dir (dirname $dir)
        end
        if test -z "$file"; and test -f "$HOME/.terraform-version"
            set file "$HOME/.terraform-version"
        end

        if test -n "$file"
            set requested (string trim < $file)
        else
            set requested "latest"
        end
    end

    switch $requested
        case "latest"
            __tf_resolve_from_remote ""
        case "latest:*"
            __tf_resolve_from_remote (string replace "latest:" "" $requested)
        case "min-required"
            echo "min-required needs .tf file parsing — not handled here" >&2
            return 1
        case "*"
            echo $requested
    end
end

# Internal helper for tf-version-name; bundled here (not its own autoload file)
# since fish only autoloads by the primary function name and this one is never
# called directly.
function __tf_resolve_from_remote --description 'Internal: resolve latest stable version matching optional regex from releases.hashicorp.com'
    set -l regex $argv[1]
    set -l list (curl -sSf https://releases.hashicorp.com/terraform/ \
        | grep -oE 'terraform_[0-9]+\.[0-9]+\.[0-9]+[a-zA-Z0-9.-]*' \
        | sed -E 's/terraform_//' \
        | grep -viE 'alpha|beta|rc' \
        | sort -uV)

    if test -n "$regex"
        set list (printf '%s\n' $list | grep -E "$regex")
    end

    printf '%s\n' $list | tail -n1
end
