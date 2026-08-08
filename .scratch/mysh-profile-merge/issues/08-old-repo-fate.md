# 08 — What happens to the old ichsansting/dotfiles repo?

**Type:** grilling

**Status:** resolved

**Blocked by:** 04

## Question

Once `profile/` inside `ichsansting/mysh` is the real Source (ticket 04), the old
`ichsansting/dotfiles` repo is redundant. Archive it, delete it, or leave it as-is
(risk: someone/something still points at it, e.g. old `MYSH_REMOTE_URL` overrides on
an existing device)?

## Answer

Delete `ichsansting/dotfiles`. `ichsansting/mysh` (with `profile/`) is now the sole,
public source of truth per ADR-0004; the old private repo's one-commit seed predates
the real secrets (ticket 09) and has no reason to keep existing, even read-only. All
of its tracked content had already been migrated into `profile/` by ticket 04
(credential-scanned, none found); the three real secrets in ticket 09 were never in
`dotfiles` to begin with.

Done: deleted by the user via GitHub's UI (the `gh` CLI token lacked `delete_repo`
scope). Confirmed gone (`gh repo view ichsansting/dotfiles` → repository not found).

## Comments
