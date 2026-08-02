# 11 — Teardown

**What to build:** The `teardown` command — full reversal of everything `mysh` has done to a device, by replaying the `Application Log`: files, packages, `mise` itself, PATH/rc lines, and the bootstrap installer's own footprint.

**Blocked by:** 02 — Application Log foundation + pre-existing-file backup, 09 — Packages: lazy install via shims

**Status:** done

- [x] `teardown` deletes every file `mysh` created and restores the backed-up original for every file it overwrote
- [x] `teardown` uninstalls every package it installed via `mise`, and removes `mise` itself if `mysh` installed it
- [x] `teardown` strips every `PATH`/rc-file line `mysh` added, including the bootstrap installer's own `PATH` addition
- [x] `teardown` removes the `mysh` binary and installer footprint last
- [x] After `teardown`, every managed path matches its pre-bootstrap state exactly — no residue
- [x] A test drives a full bootstrap-to-teardown cycle against injected temp directories and asserts no residue remains

## Comments

Implemented as `src/teardown.rs`, wired up via `Config::resolve_target_dir` (teardown only needs `TARGET_DIR` — it never touches `Source`, so it doesn't force a pointless `--source-dir` the way every other command does) and a new `teardown` arm in `main.rs`.

`AppLog` gained `entries()`/`LogEntry`, a structured reader over the log's raw lines (parsing `created`, `overwritten`, `mise-bootstrapped`, `package-installed`, `bootstrap-installed`, `bootstrap-path-added`) so `teardown` can replay it. Reversal order matches the checklist: (1) delete created files / restore overwritten originals from their backups, (2) wipe the isolated `mise` data dir to uninstall every package at once (it's entirely mysh-owned — see `mise::data_dir`'s doc comment), then delete the owned `mise` binary only if `mise-bootstrapped` was logged, (3) strip bootstrap's `PATH`/comment block from the rc file (byte-exact block removal, falling back to line-level stripping if the block's been hand-edited), (4) delete the `bootstrap-installed` binary last. A final `remove_dir_all(target/.mysh)` sweep catches everything not individually logged — the log itself, the backups dir, and lazy-package shim files (which `package.rs` never logs per-file, since the whole `.mysh/bin` prefix is already mysh-owned). Requires confirmation first, printing a one-line-per-entry summary of what's about to be reversed — the same "show pending state, then confirm" shape `save`/`reset` already use.

Tests: unit tests for `LogEntry` parsing (`log.rs`), `resolve_target_dir`'s no-source-dir precedence (`config.rs`), and `strip_line`/`summarize` (`teardown.rs`); `tests/teardown_integration.rs` covers created/overwritten reversal, a declined teardown leaving state untouched, a no-op on an untouched device, eager+lazy package/mise teardown (asserting the untracked lazy shim still disappears), and a full `bootstrap.sh`-to-`teardown` cycle (fake `curl` serves the real compiled binary) asserting zero residue under `target`. `cargo test` (68 tests) and `cargo clippy --all-targets` pass clean.

Reviewed via `/code-review` against commit `f52497f`. Standards axis: one hard violation (teardown prompted for confirmation without first showing what it would reverse, unlike `save`/`reset`) — fixed by adding the `summarize`/`describe` pre-confirm printout. Spec axis: flagged that the only package-teardown test used an eager-only package, leaving the untracked lazy-shim removal path with no coverage — fixed by extending that test to declare a lazy package too and assert its shim is gone after teardown.
