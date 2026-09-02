# typed word -> real command
abbr -a ls 'eza --icons=auto'
abbr -a ll 'eza --icons -la'
abbr -a lt 'eza --icons --tree --level=1'
abbr -a cat bat
abbr -a grep rg
abbr -a find fd
abbr -a top btop
abbr -a htop btop
abbr -a du 'dust -d1 -n 10'
abbr -a df duf
abbr -a diff delta
abbr -a claudej 'BUN_JSC_useJIT=0 claude' # JIT crash workaround
abbr -a gst 'git status'
abbr -a glg 'git log --oneline --graph --decorate --all'
abbr -a gbr 'git branch'
abbr -a gunstage 'git reset HEAD --'
abbr -a glast 'git log -1 HEAD'
abbr -a groot 'cd (git rev-parse --show-toplevel || pwd)'
