# remove every git worktree except the ones on master/main
function pr-clean
    git worktree list | awk '$3 != "[master]" && $3 != "[main]" { print $1 }' | while read -l wt_path
        echo "Removing worktree at: $wt_path"
        if not git worktree remove "$wt_path"
            echo "Error: Failed to remove worktree: $wt_path"
        end
    end
end
