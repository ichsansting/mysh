# rebase current branch onto the matching upstream branch, then push to origin
function git_update
    set branch (git branch --show-current)
    git fetch upstream
    git rebase upstream/$branch
    git push origin $branch
end
