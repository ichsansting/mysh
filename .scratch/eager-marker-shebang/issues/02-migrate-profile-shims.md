# 02 — Migrate the 46 real shims in profile/.mysh/bin to the new marker shape

**Type:** task

**Status:** done

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

- 44 of the 46 shims matched the plain `add`-generated template exactly and were
  regenerated in bulk via `package::shim_script`, driven by a scratch `examples/`
  binary (deleted after use, per the ticket).
- `docker` and `terraform` are hand-written wrappers with custom sh logic around the
  `exec mise x` line (conditional subcommand dispatch, AWS-profile-assume wrapping).
  Driving them through `shim_script` would have discarded that logic, so their sh
  bodies were hand-translated to fish syntax instead, preserving behavior. Verified
  with `fish -n` on both.
- Full verify checklist passed: shebang count is exactly 9 `#!/bin/sh` / 37
  `#!/usr/bin/env fish`; `fish -n` clean on all 37 fish shims; `rg "mysh: eager"`
  returns nothing under `profile/`; executable bits (`0o777` on this profile, not
  `0o755` — preserved as-is rather than normalized) unchanged; `cargo test` green
  (45 unit tests, 93 cucumber scenarios / 586 steps).
