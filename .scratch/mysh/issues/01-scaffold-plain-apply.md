# 01 — Scaffold + plain-file Apply

**What to build:** The foundational `mysh` CLI: a Rust binary with the injectable `SOURCE_DIR`/`TARGET_DIR`/`REMOTE_URL`/passphrase entrypoint (the project's testing seam per the spec), a subprocess wrapper around the real `git` binary, and an `apply` command that renders every plain file in `Source` to its mirrored path under `Target` via identity copy.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] CLI accepts `SOURCE_DIR`, `TARGET_DIR`, `REMOTE_URL`, and a non-interactive passphrase override, each overridable via flag/env var
- [x] `git` operations (clone, fetch, status, commit, push) are invoked as subprocess calls to the real `git` binary, discovered via `PATH`
- [x] `apply` renders every plain file in `Source` to its mirrored relative path under `Target` via identity copy
- [x] Running `apply` twice with no changes in `Source` is a no-op (idempotent — no needless rewrites)
- [x] An integration test drives `apply` against temp `Source`/`Target` directories and asserts the rendered files match byte-for-byte

## Comments

Implemented in commit f88e9f8 on `main`: `src/config.rs`, `src/git.rs`, `src/apply.rs`, `src/main.rs`, plus `tests/apply_integration.rs` and `tests/git_integration.rs`. `cargo build`, `cargo test`, and `cargo clippy --all-targets` all pass clean.
