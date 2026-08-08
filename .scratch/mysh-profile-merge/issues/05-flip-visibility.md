# 05 — Flip ichsansting/mysh to public

**Type:** task

**Status:** open

**Blocked by:** 04

## Question

Per ticket 03, make `ichsansting/mysh` public. Before flipping: re-check the repo
(tool source + freshly-migrated `profile/`) for anything that shouldn't be
world-readable per ADR-0004's boundary (nothing outside `.age` files). This is a
visible, hard-to-reverse action against a shared/external system (GitHub) — confirm
with the user immediately before running it, even though ticket 03 already agreed to
it in principle.

## Comments
