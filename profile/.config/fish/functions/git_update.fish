# rebase local master onto upstream/master, then push to origin
function git_update
    git fetch upstream
    git rebase upstream/master
    git push origin master
end
