# 02 — Migrate the 46 real shims in profile/.mysh/bin to the new marker shape

**Type:** task

**Status:** ready-for-agent

**Blocked by:** 01

## Task

Once ticket 01 lands, `src/domain/package.rs::shim_script` produces the new shapes but the
real, checked-in files in `profile/.mysh/bin/` (this user's live daily-driver shims,
rendered to `~/.mysh/bin` on every real machine) still carry the old shape. Regenerate all
46 of them in place:

- **9 eager shims** (currently `#!/bin/sh` + `# mysh: eager` + sh body) — drop the marker
  line only; body and shebang otherwise unchanged. Find them: `grep -l "mysh: eager"
  profile/.mysh/bin/*`.
- **37 lazy shims** (currently `#!/bin/sh`, sh body, no marker) — rewrite shebang to
  `#!/usr/bin/env fish` and rewrite the body to fish syntax. Find them: `grep -rL "mysh:
  eager" profile/.mysh/bin/*`.

For every file, extract its existing `specifier` and `invoke_name` (parseable today via
`package::shim_specifier` and the text between ` -- ` and ` "$@"` on the `exec mise x` line)
and regenerate the file's full content via the *new* `package::shim_script(specifier,
invoke_name, eager)` — don't hand-edit each file; drive it through the same function
`add` uses, so the migrated files are byte-identical to what `add` would produce today.
A small one-off script (Rust test binary, or a `cargo run --bin` scratch tool, deleted after
use) is the lazy way to do this across 46 files without hand-editing each one — write one,
don't hand-edit.

Preserve each file's executable bit (`0o755`).

## Verify

- `for f in profile/.mysh/bin/*; do head -1 "$f"; done | sort | uniq -c` — expect exactly
  two values: `#!/bin/sh` (9 times) and `#!/usr/bin/env fish` (37 times).
- Spot-check a handful of migrated lazy shims for valid fish syntax: `fish -n <file>`
  (fish's syntax-check-only flag) if fish is available in this environment; otherwise
  visually confirm `set -x` / `$argv` against the spec's template.
- This is the user's live profile — before committing, confirm with the user whether to
  `mysh save`/push immediately or leave it staged, per the standing "confirm before
  pushing/committing" rule.

## Comments
