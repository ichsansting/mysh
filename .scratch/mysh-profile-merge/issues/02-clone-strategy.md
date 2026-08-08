# 02 — How does bootstrap.sh fetch Source without pulling the whole tool repo?

**Type:** grilling

**Status:** resolved

**Blocked by:** 01

## Question

`bootstrap.sh` currently does `git clone $REMOTE_URL` to get Source. With the profile
now living in `profile/` inside the mysh repo (ticket 01), a plain clone would pull
the whole tool repo — Rust source, tests, `docs/` — onto every target machine just to
get the `profile/` subdirectory. How should the fetch work instead?

## Answer

Sparse checkout, `profile/` only: `git clone --filter=blob:none --no-checkout`, then
`git sparse-checkout set profile/`, then checkout. Only the dotfiles content lands in
Source on the target machine; no Rust source or build artifacts. `--source-dir` then
points at `<sparse-clone>/profile`.

## Comments
