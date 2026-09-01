# granted's `assume` must run *in* the shell to export AWS_* into it, so it is a
# function, not a bin shim. Wraps granted's own assume.fish, which needs the
# sibling `assumego` binary on PATH.
function assume --description "granted assume (exports AWS_* into this shell)"
    set -lx MISE_DATA_DIR "$HOME/.mysh/mise"
    # `mise x`, not `mise where`: keeps first use lazily installing granted.
    set -l dir (path dirname (mise x github:fwdcloudsec/granted -- sh -c 'command -v assumego'))
    or return 1
    set -lx PATH $dir $PATH
    source $dir/assume.fish $argv
end
