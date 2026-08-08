# typed word -> real command
abbr -a ls 'eza --icons'
abbr -a ll 'eza --icons -la'
abbr -a lt 'eza --icons --tree'
abbr -a cat bat
abbr -a grep rg
abbr -a find fd
abbr -a top btop
abbr -a htop btop
abbr -a du dust
abbr -a df duf
abbr -a diff delta
abbr -a fm yazi
abbr -a claudej 'BUN_JSC_useJIT=0 claude' # JIT crash workaround
abbr -a gst 'git status'
abbr -a glg 'git log --oneline --graph --decorate --all'
abbr -a gco 'git checkout'
abbr -a gbr 'git branch'
abbr -a gunstage 'git reset HEAD --'
abbr -a glast 'git log -1 HEAD'

# ponytail: the `done` notification plugin (desktop alert when a long
# command finishes) is a fisher plugin, not a file/tool ghoshell's manifest
# models — install it by hand once fisher is set up: `fisher install
# franciscolourenco/done`.
