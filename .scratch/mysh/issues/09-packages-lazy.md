# 09 — Packages: lazy install via shims

**What to build:** Lazy package installation. Instead of installing at `apply` time, `mysh` generates a thin shim per lazy package's binary name into its isolated `PATH`-resident prefix; running the plain command name transparently installs the tool on first real use.

**Blocked by:** 08 — Packages: eager install via mise

**Status:** done

- [x] Lazy packages are not installed during `apply`
- [x] `mysh` generates a shim script per lazy package's binary name into its isolated, `PATH`-resident prefix
- [x] Invoking the plain binary name for a lazy package runs the shim, which calls `mise x <specifier>@<version> -- <bin_name>`, installing on first real invocation
- [x] Subsequent invocations reuse the already-installed tool without re-triggering install
- [x] Tests cover shim generation and the install-then-exec behavior on first invocation, using a stubbed `mise` on `PATH`

## Comments

`package::install_eager` and the new lazy-shim generation were merged into one `package::apply(source, target, log)`, called once from `apply::render`. Reason: a lazy package's shim calls bare `mise x ...` when run later, so `mise` must actually be resolvable by then — the only place that can be guaranteed is during `apply` itself, so `apply` now resolves/bootstraps `mise` whenever *any* package (eager or lazy) is declared, not just eager ones. A device with zero packages of either kind still never touches `mise`. Each shim embeds the exact `mise` binary `ensure_installed` resolved (bare `mise` for a system-wide install — still relies on `PATH`, which is guaranteed since a system-wide `mise` is on `PATH` by definition; an absolute `.mysh/bin/mise` path when mysh bootstrapped it, so the shim doesn't depend on that directory being on `PATH` by the time it's actually invoked later). Shims live in `mise::bin_dir` (`<target>/.mysh/bin`), the same isolated prefix `mise` itself bootstraps into. Each shim sets `MISE_DATA_DIR` to the same isolated data dir eager installs use before calling `exec "<mise_bin>" x <specifier> -- <bin_name> "$@"`, keeping lazy installs inside the same mysh-owned, teardown-able prefix (ADR-0005). Shim writes are idempotent (skip the write when content already matches), matching every other render path in this codebase.

Tests: a unit test on `shim_script`'s generated content in `src/package.rs`, plus in `tests/package_integration.rs` — a lazy-only device still bootstraps `mise` (via a fake `curl`) so its shim has something to invoke, and the shim's script embeds the resolved absolute path; a shim generated against an already-present stubbed `mise` is executable, is never installed during `apply` itself, and on first real invocation installs-then-execs (asserted via a fake `mise x` that records the install exactly once and echoes back the passed-through args), with a second invocation reusing the install without re-triggering it. `cargo test` (57 tests) and `cargo clippy --all-targets` pass clean.

Reviewed via `/code-review` against commit `786d340`. Standards axis: one hard violation (shim writes weren't idempotent, unlike every other write path in the codebase) — fixed. Spec axis: one real gap (a lazy-only device with no system-wide `mise` never bootstrapped it during `apply`, so the shim's `mise x` call would fail with "command not found" on first real invocation, contradicting the issue's "installing on first real invocation") — fixed by resolving `mise` whenever any package is declared and embedding the resolved binary into each shim.
