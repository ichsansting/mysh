# Eagerness moves from a `# mysh: eager` comment line to the shim's shebang itself

ADR-0007 settled eagerness as one bit on the shim: a `# mysh: eager` comment line right
after the shebang, checked by `is_eager`. That's a second mandatory line doing one job a
line the file already needs — the shebang — could carry instead. The shebang itself now
*is* the marker: an eager shim's shebang is `#!/bin/sh` (unchanged body, sh syntax); a lazy
shim's shebang is `#!/usr/bin/env fish`, and its body is rewritten to fish syntax (`set -x`
instead of `export`, `$argv` instead of `"$@"`) since the shebang decides which interpreter
parses the whole file, not just the first line.

This makes every lazy shim depend on `fish` already being resolvable on `PATH` at shebang
time — before the script body's own `mise x` fallback logic ever runs. `fish` is already an
ordinary (not special-cased) eager Package in this profile, so on any device where `apply`
has run, `.mysh/bin/fish` is prewarmed and on `PATH`, and `env fish` resolves to that shim.
mysh enforces nothing here: a lazy-only profile with no eager `fish` package is a valid but
unsupported configuration — its lazy shims fail with a plain `fish: command not found`, no
mysh-level guard or error message. Documented, not enforced, consistent with the existing
"hand-edited/unmatched shim degrades to lazy, never an error" philosophy — `is_eager` is an
exact match on `#!/bin/sh` as the first line; anything else, including a malformed or
hand-edited shebang, is lazy.

## Considered Options

- **Keep the marker as a separate comment line, just change its text/position.** Rejected:
  doesn't address the actual motivation — the shebang line is already mandatory in every
  shim, so a second marker line is one line of pure redundancy once the shebang itself can
  carry the same one bit.
- **Enforce the `fish`-must-be-eager invariant in code** (e.g. `add` refuses/warns on a lazy
  package with no eager `fish` shim in Source). Rejected: makes every lazy `add` call aware
  of `fish` specifically — exactly the kind of special-casing ADR-0006 already rejected once.

## Consequences

- `EAGER_MARKER` and the `# mysh: eager` comment line are gone; `is_eager` reads the shim's
  first line instead of scanning for a marker line anywhere in the file.
- Every lazy shim's body must be valid fish syntax, not sh — a broader rewrite than the
  marker line alone, touching `shim_script`'s lazy branch and all 37 existing lazy shims in
  `profile/.mysh/bin`.
- Lazy execution now has a real, undeclared runtime dependency on an eager `fish` package
  existing in Source — true for this profile today, not guaranteed for a hypothetical
  lazy-only `mysh` configuration (see `features/package.feature`'s "lazy-only device" test,
  which predates this change and exercises exactly that gap).
- `CONTEXT.md`'s "Shim" and "Eager package" entries need updating in the implementation
  ticket, not here — this ADR records the decision, the code and glossary catch up together.
