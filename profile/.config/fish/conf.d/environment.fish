# XDG base directories
set -gx XDG_CONFIG_HOME "$HOME/.config"
set -gx XDG_DATA_HOME "$HOME/.local/share"
set -gx XDG_CACHE_HOME "$HOME/.cache"
set -gx XDG_STATE_HOME "$HOME/.local/state"

# default programs
set -gx EDITOR hx
set -gx VISUAL hx
set -gx PAGER bat
set -gx MANPAGER "bat -l man -p"
