# Spec: Eager marker becomes the shim's shebang line

## Decision

Resolved via `/grill-with-docs` on 2026-08-28; full rationale in
[ADR-0015](../../docs/adr/0015-eager-marker-is-the-shebang-line.md).

Replace the `# mysh: eager` comment-line marker (`src/domain/package.rs`) with the shim's
shebang itself: eager shims stay `#!/bin/sh` (body unchanged); lazy shims become
`#!/usr/bin/env fish` with their body rewritten to fish syntax (`set -x` for env vars,
`$argv` for args). `shim_specifier`'s parsing (looks for the `exec mise x ... -- ...` line)
is unaffected by the syntax change and needs no rewrite.

`is_eager` becomes an exact match: first line `== "#!/bin/sh"` → eager, anything else
(including a malformed/hand-edited shebang) → lazy. No new special case — matches the
existing "unmatched shim shape stays lazy, never an error" rule.

Lazy shims gain a real (undeclared, unenforced) runtime dependency on `fish` already being
on `PATH` — true today because `fish` is already an ordinary eager Package in this profile
(`profile/.mysh/bin/fish`). **No code enforces this** — a lazy-only profile with no eager
`fish` package is a valid-but-unsupported configuration that fails with a plain shell
"command not found", by deliberate choice (avoids `add` special-casing `fish`).

## Scope

- Code: `src/domain/package.rs` (`shim_script`, `is_eager`; `EAGER_MARKER` deleted).
- Docs: `CONTEXT.md`'s "Shim" and "Eager package" entries (currently describe the old
  comment-line marker); `features/package.feature`'s description text (same).
- Data migration: all 46 shim files in `profile/.mysh/bin/` — 9 eager (drop the now-redundant
  marker line, shebang unchanged) and 37 lazy (shebang + full body rewritten to fish syntax).

## Out of scope

- Re-litigating the eager/lazy split itself (ADR-0006/0007 stand).
- Enforcing the `fish`-must-be-eager invariant in code (explicitly rejected, see ADR-0015).
- Any change to how `apply`'s prewarm pass collects specifiers (`shim_specifier` parsing is
  untouched).

## Tickets

- [01 — Rewrite the marker mechanism in code](issues/01-shebang-encodes-eagerness.md) — done
- [02 — Migrate the 46 real profile shims](issues/02-migrate-profile-shims.md) — done
