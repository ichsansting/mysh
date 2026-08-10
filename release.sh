#!/usr/bin/env bash
# release.sh — builds and publishes mysh's single, ever-updating `v1` GitHub Release.
# Not user-facing (bootstrap.sh is that) — a maintainer runs this by hand, from a clean
# checkout, on either Linux or macOS, whenever a new binary should ship.
#
# There is no v2, v3, etc. Every run force-moves the `v1` tag to HEAD and overwrites its
# assets in place — "release" here means "update the one release that exists."
set -euo pipefail

cd "$(dirname "$0")"

TARGETS=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin)

# --- Refuse a dirty working tree: the `v1` tag would then claim a commit that doesn't
# match the binaries it's shipping, and there's no test gate to catch that mismatch. ---
if [ -n "$(git status --porcelain)" ]; then
    echo "release.sh: working tree is dirty — commit or stash before releasing" >&2
    exit 1
fi

# --- Check required tools; never install them ourselves. ---
missing=0

if ! command -v rustup >/dev/null 2>&1; then
    echo "release.sh: rustup not found — install from https://rustup.rs" >&2
    missing=1
fi

if ! command -v zig >/dev/null 2>&1; then
    case "$(uname -s)" in
        Linux) echo "release.sh: zig not found — install with: nix profile install nixpkgs#zig" >&2 ;;
        Darwin) echo "release.sh: zig not found — install with: brew install zig" >&2 ;;
        *) echo "release.sh: zig not found" >&2 ;;
    esac
    missing=1
fi

if ! cargo zigbuild --help >/dev/null 2>&1; then
    echo "release.sh: cargo-zigbuild not found — install with: cargo install cargo-zigbuild" >&2
    missing=1
fi

if [ "$missing" -eq 0 ]; then
    installed_targets="$(rustup target list --installed)"
    for target in "${TARGETS[@]}"; do
        if ! grep -qx "$target" <<<"$installed_targets"; then
            echo "release.sh: rust target $target not installed — install with: rustup target add $target" >&2
            missing=1
        fi
    done
fi

if [ "$missing" -ne 0 ]; then
    exit 1
fi

# --- Build every target. musl targets are fully static (self-contained); the darwin
# target dynamically links only libSystem — true static linking isn't possible on
# macOS at all, so that's the ceiling there, not a gap in this script. ---
assets=()
for target in "${TARGETS[@]}"; do
    cargo zigbuild --release --target "$target"
    asset="mysh-$target"
    cp "target/$target/release/mysh" "$asset"
    assets+=("$asset")
done

notes="Built from $(git rev-parse HEAD) on $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# --- Force-move the single `v1` tag to this commit. `-m` is required even for a
# lightweight-in-spirit tag: a `tag.gpgsign=true` config (common for maintainers who
# sign everything) silently forces annotation, which then demands a message. ---
git tag -f v1 -m "$notes"
git push -f origin v1

if gh release view v1 >/dev/null 2>&1; then
    gh release edit v1 --notes "$notes"
    gh release upload v1 "${assets[@]}" --clobber
else
    gh release create v1 --title v1 --notes "$notes" "${assets[@]}"
fi

rm -f "${assets[@]}"
echo "release.sh: published v1 ($(git rev-parse --short HEAD))"
