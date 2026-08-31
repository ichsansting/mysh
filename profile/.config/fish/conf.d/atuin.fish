atuin init fish --disable-up-arrow | source

# Up arrow does fish history search; 3 consecutive presses open atuin.
# Counter resets when the buffer no longer matches what our last press left
# behind, i.e. any other key was pressed in between.
function _atuin_up3
    set -q __atuin_up3_count; or set -g __atuin_up3_count 0
    test "$(commandline -b)" = "$__atuin_up3_last"; or set -g __atuin_up3_count 0

    if test $__atuin_up3_count -eq 0
        # atuin seeds its query from the buffer, so keep the pre-search text
        set -g __atuin_up3_orig (commandline -b)
    end
    set -g __atuin_up3_count (math $__atuin_up3_count + 1)

    if test $__atuin_up3_count -ge 3; and test (commandline --line) -eq 1
        set -g __atuin_up3_count 0
        commandline -r -- "$__atuin_up3_orig"
        _atuin_search --shell-up-key-binding
    else
        up-or-search
        set -g __atuin_up3_last (commandline -b)
    end
end

bind up _atuin_up3
bind -M insert up _atuin_up3 2>/dev/null
