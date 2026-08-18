# checkout a PR into its own worktree, sparse-checked out to just the dir the last commit touched
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

    set -l pr_dir "$repo_root/$pr_branch"

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

    set -l last_commit_dir (git diff-tree --no-commit-id --name-only -r HEAD | head -n 1 | xargs dirname)

    if test -z "$last_commit_dir"
        echo "Error: Could not determine directory from last commit; sparse checkout aborted."
        return 1
    end

    echo "Sparse-checkout directory: $last_commit_dir"
    git sparse-checkout set "$last_commit_dir"

    git checkout HEAD

    echo "Switched to PR #$pr_num with sparse checkout of $last_commit_dir"
end

# remove every git worktree except the ones on master/main
function pr-clean
    git worktree list | awk '$3 != "[master]" && $3 != "[main]" { print $1 }' | while read -l wt_path
        echo "Removing worktree at: $wt_path"
        if not git worktree remove "$wt_path"
            echo "Error: Failed to remove worktree: $wt_path"
        end
    end
end

# open an SSM session to an EC2 instance found by its Name tag
function ssm
    set -l target (aws ec2 describe-instances \
        --region ap-southeast-1 \
        --filters "Name=tag:Name,Values=$argv[1]" "Name=instance-state-name,Values=running" \
        --query 'Reservations[*].Instances[*].InstanceId' \
        --output text)

    aws ssm start-session --region ap-southeast-1 --target $target
end

# forward a local port to an RDS instance through an SSM bastion (bstn-tagged instance)
function rds_forward
    set -l target (aws ec2 describe-instances \
        --region ap-southeast-1 \
        --filters "Name=tag:Name,Values='*bstn*'" "Name=instance-state-name,Values=running" \
        --query 'Reservations[*].Instances[*].InstanceId' \
        --output text)

    set -l host (aws rds describe-db-instances \
        --query "DBInstances[?contains(DBInstanceIdentifier, '$argv[1]')].[Endpoint.Address]" \
        --output text)

    aws ssm start-session \
        --region ap-southeast-1 \
        --target "$target" \
        --document-name AWS-StartPortForwardingSessionToRemoteHost \
        --parameters \
        host="$host",portNumber="5432",localPortNumber="$argv[2]"
end

# rebase local master onto upstream/master, then push to origin
function git_update
    git fetch upstream
    git rebase upstream/master
    git push origin master
end

# unset every AWS_* variable (handy when switching between AWS profiles/sessions)
function clear_aws_env
    for var in (set -n)
        if string match -q 'AWS_*' $var
            echo "Unsetting $var"
            set -e $var
        end
    end
end

# run terraform inside a pinned linux/amd64 container (for Apple Silicon), with SSH access wired up
# for private git modules. Originally podman (used `podman secret` + `podman run --secret`); plain
# `docker run` has no --secret flag (that's Swarm-only), so the key is bind-mounted read-only instead.
function tf_amd64
    set -l ssh_key_path "$HOME/.ssh/id_rsa"
    set -l tf_version (tfenv version-name)
    set -l image_name "hashicorp/terraform:$tf_version"

    if not test -f "$ssh_key_path"
        echo "Error: SSH private key not found at $ssh_key_path." >&2
        echo "Please ensure the key exists or update ssh_key_path." >&2
        return 1
    end

    set -l commands "
        mkdir -p /root/.ssh && chmod 700 /root/.ssh && \
        cp /run/secrets/ssh_key /root/.ssh/id_rsa && \
        chmod 600 /root/.ssh/id_rsa && \
        git config --global core.sshCommand 'ssh -i /root/.ssh/id_rsa -o StrictHostKeyChecking=no' && \
        git config --global --add safe.directory \"*\" && \
        git config --global url.\"git@github.com:\".insteadOf \"https://github.com/\" && \
        terraform \"\$@\"
    "

    docker run --rm -it \
        -v "$PWD:/data" \
        -v "$ssh_key_path:/run/secrets/ssh_key:ro" \
        --env AWS_ACCESS_KEY_ID="$AWS_ACCESS_KEY_ID" \
        --env AWS_SECRET_ACCESS_KEY="$AWS_SECRET_ACCESS_KEY" \
        --env AWS_REGION="$AWS_REGION" \
        --env AWS_SESSION_TOKEN="$AWS_SESSION_TOKEN" \
        -w /data --platform=linux/amd64 \
        --entrypoint sh "$image_name" \
        -c "$commands" -- $argv
end
