# 08 — Packages: eager install via mise

**What to build:** Package declarations and eager installation. `mysh` declares CLI-tool packages (install specifier, optional binary-name override, eager/lazy classification), self-bootstraps `mise` if it's missing, and installs every eager package during `apply`.

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** done

- [x] Packages are declared with a `mise`-compatible specifier — either bare (`go@latest`, for tools `mise` natively supports) or backend-prefixed (`github:`, `npm:`, `pip:`, `cargo:`, etc.) — plus an eager/lazy classification
- [x] The resulting binary name defaults to the bare specifier name and only needs to be declared explicitly when a backend-prefixed install produces a differently-named binary (e.g. `github:elio-fm/elio` → `elio`)
- [x] If `mise` is not already present on the device, `mysh` installs it automatically and records the bootstrap in the `Application Log`
- [x] Every package declared eager is installed via `mise install` during `apply`
- [x] Tests cover: `mise` absent → bootstrapped and logged; an eager package declared → installed and runnable after `apply` (using a stubbed `mise` on `PATH` to avoid real network installs)

## Comments

Implemented in new `src/package.rs` (declaration parsing/orchestration) and `src/mise.rs` (subprocess wrapper around the real `mise` binary). Packages are declared in a `.packages` file at the root of `Source` — tab-separated lines, `<specifier>\t<eager|lazy>[\t<bin_name>]` — treated as mysh metadata, not a dotfile: `apply::render` and `diff::diff` both exclude this exact top-level path from ordinary rendering/drift (a same-named file nested in a subdirectory is untouched, still an ordinary file). `default_bin_name` derives the binary name from the specifier (strip backend prefix, strip `@version`, take the last `/`-segment) when not explicitly overridden.

`mise::ensure_installed` checks `mise --version` on `PATH`; if absent, runs the official `curl -fsSL https://mise.run | sh` installer via `sh -c` and records `mise-bootstrapped` in the `Application Log`. `package::install_eager` only touches `mise` at all when at least one eager package is declared — a device with no packages (or lazy-only) never bootstraps `mise` during `apply`. Each eager package is installed via `mise install <specifier>`, scoped to an isolated `MISE_DATA_DIR` under `<target>/.mysh/mise` (so `teardown`, issue 11, can remove everything by deleting one directory), and its own successful install is recorded via a new `AppLog::record_package_installed`.

Tests: unit tests in `src/package.rs` (`default_bin_name` across bare/backend-prefixed/versioned specifiers, `parse_line` defaulting/overriding `bin_name` and rejecting an unknown classification) plus `tests/package_integration.rs` driving the real CLI — `mise`-absent bootstrap-and-log (via a fake `curl` on `PATH` that fabricates a `mise` binary instead of hitting the real network), no-op when no eager packages are declared (asserted by leaving `mise`/`curl` off `PATH` entirely), eager install-and-runnable via a stubbed `mise` (asserts the install call, the Application Log entry, and that the installed stub binary actually runs), the `.packages` file never rendered to `Target`, and a nested same-named file rendered normally. `cargo test` (50 tests) and `cargo clippy --all-targets` pass clean. Reviewed via `/code-review`: Standards axis found no hard violations; Spec axis found two real gaps (installed packages weren't recorded in the Application Log per `CONTEXT.md`/`spec.md`'s definition; the original `.packages` exclusion was scoped too broadly, hiding a nested same-named file) — both fixed.
