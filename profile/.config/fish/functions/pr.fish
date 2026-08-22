# checkout a PR into its own pr-<number> worktree, sparse-checked out to every dir the PR touches
function pr
    if test -z "$argv[1]"
        echo "Usage: pr <pull-request-number>"
        return 1
    end

    set -l pr_num $argv[1]

    set -l repo_root (git rev-parse --show-toplevel)
    if test $status -ne 0
        echo "Error: Not inside a Git repository."
        return 1
    end

    # auto-find owner/repo from the upstream remote URL
    set -l remote_url (git remote get-url upstream)
    if test $status -ne 0
        echo "Error: Unable to get remote URL."
        return 1
    end

    set -l gh_repo (echo "$remote_url" | sed -E 's#(https://github.com|git@github.com:)([^/]+)/(.*)\.git#\2/\3#')
    if test -z "$gh_repo"
        echo "Error: Unable to extract owner/repo from the URL."
        return 1
    end

    if not gh auth status &>/dev/null
        echo "GitHub CLI not authenticated. Logging in..."
        if not gh auth login
            echo "Error: GitHub login failed."
            return 1
        end
    end

    set -l pr_branch (gh pr view "$pr_num" --repo "$gh_repo" --json headRefName -q .headRefName)
    if test $status -ne 0
        echo "Error: Could not fetch PR branch using gh CLI."
        return 1
    end

    echo "PR branch: $pr_branch"

    if test -z "$pr_branch"
        echo "Error: Could not determine the original PR branch."
        return 1
    end

    echo "Original PR branch: $pr_branch"

    set -l pr_dir "$repo_root/pr-$pr_num"

    if not git remote | grep -q '^upstream$'
        echo "Error: 'upstream' remote not found."
        return 1
    end

    echo "Fetching PR #$pr_num from upstream..."
    if not git fetch upstream "pull/$pr_num/head:$pr_branch"
        echo "Failed to fetch PR #$pr_num"
        return 1
    end

    echo "Adding worktree at $pr_dir..."
    if not git worktree add --no-checkout "$pr_dir" "$pr_branch"
        echo "Failed to add worktree"
        return 1
    end

    cd "$pr_dir"; or return 1

    git sparse-checkout init --cone

    set -l pr_dirs (gh pr view "$pr_num" --repo "$gh_repo" --json files -q '.files[].path' | xargs -n1 dirname | sort -u)

    if test -z "$pr_dirs"
        echo "Error: Could not determine directories from PR files; sparse checkout aborted."
        return 1
    end

    echo "Sparse-checkout directories: $pr_dirs"
    git sparse-checkout set $pr_dirs

    git checkout HEAD

    echo "Switched to PR #$pr_num with sparse checkout of $pr_dirs"
end
