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

# --- Provision the cross-build toolchain via mise instead of checking for it and
# refusing: mise is assumed present (every device mysh manages already bootstraps
# it — ADR-0005/0007), so there's no manual-install step left to document.
# `mise shell` is NOT this: it only sets MISE_<TOOL>_VERSION env vars that
# `mise activate`'s interactive-prompt hook turns into a PATH update before your
# next typed command — a plain script has no prompt hook, so those vars would sit
# there doing nothing. `mise exec` is mise's actual non-interactive mechanism:
# it wraps one command with the requested tools genuinely on PATH, installing
# whatever's missing first. So this re-execs the script itself, once, under it —
# everything after this guard runs the second time through, tools in hand.
# rustup itself comes from mise's `rust` tool; `target add` is still a separate,
# idempotent step mise has no first-class equivalent for. ---
if [ -z "${MYSH_RELEASE_TOOLCHAIN_READY:-}" ]; then
    export MYSH_RELEASE_TOOLCHAIN_READY=1
    exec mise exec rust@latest zig@latest cargo:cargo-zigbuild@latest -- "$0" "$@"
fi

for target in "${TARGETS[@]}"; do
    rustup target add "$target"
done

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
