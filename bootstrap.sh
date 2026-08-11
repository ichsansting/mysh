#!/bin/sh
# bootstrap.sh — the one-line mysh installer, hosted in the mysh repo itself:
#
#   curl -fsSL <raw-content-url-of-this-file> | sh
#
# Detects OS/architecture, downloads the matching prebuilt `mysh` binary from mysh's
# own GitHub Releases, puts it on PATH, sparse-checks-out just the repo's `profile/`
# directory as Source (skipping the Rust tool source), then hands off to the binary to
# bootstrap `mise` and run the initial apply. The only assumed pre-existing tools are
# `git` and `curl`.
set -eu

# The repo holding both the mysh tool and the `profile/` Source directory.
# Override with MYSH_REMOTE_URL only for testing against a different checkout.
REMOTE_URL="${MYSH_REMOTE_URL:-https://github.com/ichsansting/mysh}"

# Where mysh's own prebuilt binaries are published. Fixed mysh project infrastructure,
# not a per-user setting — override with MYSH_RELEASES_REPO only for testing.
RELEASES_REPO="${MYSH_RELEASES_REPO:-ichsansting/mysh}"
VERSION="${MYSH_VERSION:-latest}"

TARGET_DIR="${MYSH_TARGET_DIR:-$HOME}"
SOURCE_DIR="${MYSH_SOURCE_DIR:-$TARGET_DIR/.mysh/source}"

# --- Detect OS/architecture, map to the matching Rust target triple ---
# Linux binaries are musl-linked (static, self-contained) — the asset name says so
# honestly, matching what release.sh actually builds.
case "$(uname -s)" in
    Linux) os_part="unknown-linux-musl" ;;
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
    # $SHELL is only the account's login shell, not necessarily what's actually
    # interactive right now (frequently unset/wrong under Docker) — prefer the
    # real parent process of this `sh`, readable on Linux with no extra
    # dependency via procfs. Falls back to $SHELL where /proc isn't available
    # (e.g. macOS).
    shell_name="${SHELL:-}"
    if [ -r "/proc/$PPID/comm" ]; then
        shell_name="$(cat "/proc/$PPID/comm" 2>/dev/null || echo "$shell_name")"
    fi
    case "$shell_name" in
        */zsh|zsh) rc_file="$HOME/.zshrc" ;;
        */bash|bash) rc_file="$HOME/.bashrc" ;;
        *) rc_file="$HOME/.profile" ;;
    esac
fi

# Two mysh-owned directories go on PATH: `.mysh/bin` (mysh owns every file in it — the
# mysh/mise binaries plus lazy-package shims, real files identity-copied from Source)
# and `.mysh/mise/shims` (mise owns this one entirely — it's regenerated wholesale on
# every reshim, so mysh must never write into it directly; this is where eager
# packages resolve, via mise's own native shim/activation mechanism, see ADR-0006).
mise_shims_dir="$TARGET_DIR/.mysh/mise/shims"
mise_data_dir="$TARGET_DIR/.mysh/mise"
path_line="export PATH=\"$bin_dir:$mise_shims_dir:\$PATH\""
if ! grep -qF "$path_line" "$rc_file" 2>/dev/null; then
    printf '\n# added by mysh bootstrap.sh\n%s\n' "$path_line" >> "$rc_file"
    if ! grep -qF "$(printf 'bootstrap-path-added\t%s\t%s' "$rc_file" "$path_line")" "$log_path" 2>/dev/null; then
        printf 'bootstrap-path-added\t%s\t%s\n' "$rc_file" "$path_line" >> "$log_path"
    fi
fi
export PATH="$bin_dir:$mise_shims_dir:$PATH"

# MISE_DATA_DIR must be durable too, not just threaded through mysh's own subprocess
# calls (`package::install_declared`, every lazy shim) — otherwise a bare `mise`
# command typed by the user resolves to mise's own default data dir instead of this
# isolated one, silently producing a second, out-of-sync install location (confirmed
# live: a stray `~/.local/share/mise` diverging from `~/.mysh/mise`, each partially
# populated depending on which one a given `mise` invocation happened to see).
# Checked independently of path_line so a machine bootstrapped before this line
# existed still picks it up on the next bootstrap run.
data_dir_line="export MISE_DATA_DIR=\"$mise_data_dir\""
if ! grep -qF "$data_dir_line" "$rc_file" 2>/dev/null; then
    printf '\n# added by mysh bootstrap.sh\n%s\n' "$data_dir_line" >> "$rc_file"
    if ! grep -qF "$(printf 'bootstrap-path-added\t%s\t%s' "$rc_file" "$data_dir_line")" "$log_path" 2>/dev/null; then
        printf 'bootstrap-path-added\t%s\t%s\n' "$rc_file" "$data_dir_line" >> "$log_path"
    fi
fi
export MISE_DATA_DIR="$mise_data_dir"

# --- Clone Source: sparse checkout of just profile/, skipping the Rust tool source ---
if [ ! -d "$SOURCE_DIR/.git" ]; then
    git clone --filter=blob:none --no-checkout "$REMOTE_URL" "$SOURCE_DIR"
    git -C "$SOURCE_DIR" sparse-checkout set profile
    git -C "$SOURCE_DIR" checkout
fi

# --- Hand off to the mysh binary: bootstrap mise, run the initial apply ---
exec "$install_path" apply --source-dir "$SOURCE_DIR/profile" --target-dir "$TARGET_DIR"
