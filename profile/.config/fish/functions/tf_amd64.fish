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
