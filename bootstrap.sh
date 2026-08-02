#!/bin/sh
# bootstrap.sh — the one-line mysh installer, hosted at the root of a mysh-managed
# dotfiles repo:
#
#   curl -fsSL <raw-content-url-of-this-file> | sh
#
# Detects OS/architecture, downloads the matching prebuilt `mysh` binary from mysh's
# own GitHub Releases, puts it on PATH, clones the repo this script lives in as
# Source, then hands off to the binary to bootstrap `mise` and run the initial apply.
# The only assumed pre-existing tools are `git` and `curl`.
set -eu

# EDIT ME: the dotfiles repo this script lives in — it gets cloned as Source.
# Override with MYSH_REMOTE_URL instead of editing this file when testing.
REMOTE_URL="${MYSH_REMOTE_URL:-https://github.com/CHANGE_ME/dotfiles}"

# Where mysh's own prebuilt binaries are published. Fixed mysh project infrastructure,
# not a per-user setting — override with MYSH_RELEASES_REPO only for testing.
RELEASES_REPO="${MYSH_RELEASES_REPO:-CHANGE_ME/mysh}"
VERSION="${MYSH_VERSION:-latest}"

TARGET_DIR="${MYSH_TARGET_DIR:-$HOME}"
SOURCE_DIR="${MYSH_SOURCE_DIR:-$TARGET_DIR/.mysh/source}"

# Matches on the placeholder text itself (not on whether MYSH_REMOTE_URL is set), so a
# real deployment that edited the default in place above passes this check with no env
# var needed — a single `curl -fsSL <url> | sh` must work standalone.
case "$REMOTE_URL" in
    *CHANGE_ME*)
        echo "bootstrap.sh: set MYSH_REMOTE_URL, or edit the REMOTE_URL default at the top of this script, to your dotfiles repo" >&2
        exit 1
        ;;
esac

# --- Detect OS/architecture, map to the matching Rust target triple ---
case "$(uname -s)" in
    Linux) os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *) echo "bootstrap.sh: unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) echo "bootstrap.sh: unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

asset="mysh-${arch_part}-${os_part}"
if [ "$VERSION" = "latest" ]; then
    download_url="https://github.com/${RELEASES_REPO}/releases/latest/download/${asset}"
else
    download_url="https://github.com/${RELEASES_REPO}/releases/download/${VERSION}/${asset}"
fi

# --- Download the binary, place it on PATH (logged in the Application Log) ---
bin_dir="$TARGET_DIR/.mysh/bin"
install_path="$bin_dir/mysh"
log_path="$TARGET_DIR/.mysh/log"

mkdir -p "$bin_dir"
curl -fsSL "$download_url" -o "$install_path"
chmod +x "$install_path"

if ! grep -qF "$(printf 'bootstrap-installed\t%s' "$install_path")" "$log_path" 2>/dev/null; then
    printf 'bootstrap-installed\t%s\n' "$install_path" >> "$log_path"
fi

rc_file="${MYSH_RC_FILE:-}"
if [ -z "$rc_file" ]; then
    case "${SHELL:-}" in
        */zsh) rc_file="$HOME/.zshrc" ;;
        */bash) rc_file="$HOME/.bashrc" ;;
        *) rc_file="$HOME/.profile" ;;
    esac
fi

path_line="export PATH=\"$bin_dir:\$PATH\""
if ! grep -qF "$path_line" "$rc_file" 2>/dev/null; then
    printf '\n# added by mysh bootstrap.sh\n%s\n' "$path_line" >> "$rc_file"
    if ! grep -qF "$(printf 'bootstrap-path-added\t%s\t%s' "$rc_file" "$path_line")" "$log_path" 2>/dev/null; then
        printf 'bootstrap-path-added\t%s\t%s\n' "$rc_file" "$path_line" >> "$log_path"
    fi
fi
export PATH="$bin_dir:$PATH"

# --- Clone Source ---
if [ ! -d "$SOURCE_DIR/.git" ]; then
    git clone "$REMOTE_URL" "$SOURCE_DIR"
fi

# --- Hand off to the mysh binary: bootstrap mise, run the initial apply ---
exec "$install_path" apply --source-dir "$SOURCE_DIR" --target-dir "$TARGET_DIR"
