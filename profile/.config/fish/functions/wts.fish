# wt switch, then cd to where the branch's work actually is.
# The work dir is the longest directory prefix shared by every file the
# branch changed vs the default branch. Stays put if there is no diff.
function wts --description 'wt switch, then cd into the branch work dir'
    wt switch $argv; or return

    set -l base (string replace origin/ '' -- (git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null))
    test -n "$base"; or return 0

    # sorted unique dirs: the common prefix of the whole set is the common
    # prefix of just the first and last entries
    set -l dirs (git diff --name-only $base... 2>/dev/null | path dirname | sort -u)
    set -q dirs[1]; or return 0

    set -l first (string split / -- $dirs[1])
    set -l last (string split / -- $dirs[-1])
    set -l n (math min (count $first), (count $last))
    set -l common
    for i in (seq $n)
        test "$first[$i]" = "$last[$i]"; or break
        set -a common $first[$i]
    end

    set -q common[1]; or return 0
    set -l dir (string join / -- $common)
    test -d $dir; and cd $dir
end

# borrow branch completions from the real command
complete -c wts -w 'wt switch'
