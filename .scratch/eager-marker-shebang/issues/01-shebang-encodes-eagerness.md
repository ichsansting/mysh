# 01 — Rewrite the eager marker mechanism: shebang instead of comment line

**Type:** task

**Status:** ready-for-agent

## Task

Implement [ADR-0015](../../../docs/adr/0015-eager-marker-is-the-shebang-line.md) /
[spec.md](../spec.md) in code and docs. No profile migration here — that's ticket 02.

`src/domain/package.rs`:

- Delete `EAGER_MARKER`.
- `shim_script(specifier, invoke_name, eager)`:
  - `eager == true`: unchanged shape —
    ```sh
    #!/bin/sh
    export MISE_DATA_DIR="$HOME/{MISE_DATA_DIR_REL}"
    exec mise x {specifier} -- {invoke_name} "$@"
    ```
    (just drop the `# mysh: eager` line that used to follow the shebang.)
  - `eager == false`: new shape —
    ```fish
    #!/usr/bin/env fish
    set -x MISE_DATA_DIR "$HOME/{MISE_DATA_DIR_REL}"
    exec mise x {specifier} -- {invoke_name} $argv
    ```
- `is_eager(content)`: exact match on the first line — `content.lines().next() ==
  Some("#!/bin/sh")`. Anything else (empty file, different shebang, malformed) is lazy —
  no error path.
- `shim_specifier` is unchanged — its `exec mise x ... -- ...` line-prefix parsing works
  identically regardless of trailing `"$@"` vs `$argv` syntax. Confirm this with a test
  rather than assuming it.

Existing unit tests in `package.rs` (`shim_round_trips_specifier_and_eagerness`,
`shim_contains_no_device_specific_absolute_path`, `hand_edited_content_without_the_exec_line_is_not_a_shim`)
already exercise both branches through the public functions — update their expectations for
the new shapes rather than deleting them.

`apply.rs`/`add.rs` call `is_eager`/`shim_specifier`/`shim_script` only through these public
functions — no call-site changes expected, but verify after the rewrite (`cargo test`).

`tests/steps.rs`'s `"source eager/lazy shim"` step defs call `package::shim_script`
directly, so `features/*.feature` scenarios should pass unchanged once the unit-level
rewrite is correct — run the full feature suite to confirm, don't assume.

Docs to update in this same ticket (keep code and glossary in sync):

- `CONTEXT.md` — "Shim" entry ("An Eager package's shim carries the `# mysh: eager` marker
  line; a Lazy package's doesn't") and "Eager package" entry ("A Package whose Shim carries
  the `# mysh: eager` marker line") both need rewording to describe the shebang-based
  signal instead.
- `features/package.feature`'s Feature description line ("An eager shim carries the
  '# mysh: eager' marker...").

## Verify

- `cargo test` passes (unit + feature/cucumber suite).
- `rg "mysh: eager"` across the repo returns nothing outside `docs/adr/000[67]` and
  `docs/adr/0015` (historical ADRs describing the old mechanism are fine to keep as-is —
  they're a record of what was decided *then*).

## Comments
