function terraform-update-modules --description 'Update module pins using the folder Terraform version; --check previews'
    if contains -- --help $argv; or contains -- -h $argv
        printf '%s\n' \
            'Usage: terraform-update-modules [--check]' \
            'Updates direct module pins in the current folder to the highest stable release' \
            'whose declared Terraform requirements (including nested modules) are satisfied.' \
            'Uses terraform-version-name, Git authentication and the cached python-hcl2 parser.' \
            'Supports Git sources and public registry modules backed by Git.' \
            'Local modules are skipped; override files and .tf.json module edits are rejected.' \
            'Branch, commit and prerelease refs are left unchanged; stable version tags only.' \
            'Registry modules need an explicit version attribute; its constraint becomes an exact pin.' \
            'No init, plan or apply is run. Provider and input compatibility need separate review.' \
            '--check reports proposed updates without changing files.'
        return 0
    end
    argparse check -- $argv
    or return 2
    if test (count $argv) -ne 0
        echo 'Usage: terraform-update-modules [--check]' >&2
        return 2
    end
    set -l terraform_version (terraform-version-name)
    or return 1
    if not string match -qr '^v?[0-9]+\.[0-9]+\.[0-9]+$' -- "$terraform_version"
        echo 'Error: terraform-version-name did not return an exact stable version.' >&2
        return 1
    end
    set -l options
    if set -q _flag_check
        set options --check
    end
    command uv run --offline --no-project --with 'python-hcl2>=8,<9' python -B \
        "$HOME/.mysh/bin/lib/terraform_update_modules.py" --terraform-version "$terraform_version" $options
end
