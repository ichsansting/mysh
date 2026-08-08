# 05 — Flip ichsansting/mysh to public

**Type:** task

**Status:** resolved

**Blocked by:** 04

## Question

Per ticket 03, make `ichsansting/mysh` public. Before flipping: re-check the repo
(tool source + freshly-migrated `profile/`) for anything that shouldn't be
world-readable per ADR-0004's boundary (nothing outside `.age` files). This is a
visible, hard-to-reverse action against a shared/external system (GitHub) — confirm
with the user immediately before running it, even though ticket 03 already agreed to
it in principle.

## Answer

Pre-flip check: no `.age` files exist yet in the repo (ticket 09 — real secrets —
still open), and `git grep` across `profile/` for key/token/password patterns found
nothing. `gh` config, git config, and Claude settings under `profile/` are all
non-sensitive. User confirmed via prompt; ran
`gh repo edit ichsansting/mysh --visibility public --accept-visibility-change-consequences`.
Verified: `gh repo view ichsansting/mysh --json visibility` now reports `PUBLIC`.

## Comments
