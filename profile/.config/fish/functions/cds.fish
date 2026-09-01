# fzf-pick a subdirectory and cd into it. `cds -H` includes hidden dirs.
function cds --description 'cd into a subdirectory picked with fzf'
    set -l dir (fd --type d $argv | fzf --preview 'eza --icons --tree --level=1 {}')
    and cd $dir
end
