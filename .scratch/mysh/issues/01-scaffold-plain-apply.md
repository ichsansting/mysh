# 01 — Scaffold + plain-file Apply

**What to build:** The foundational `mysh` CLI: a Rust binary with the injectable `SOURCE_DIR`/`TARGET_DIR`/`REMOTE_URL`/passphrase entrypoint (the project's testing seam per the spec), a subprocess wrapper around the real `git` binary, and an `apply` command that renders every plain file in `Source` to its mirrored path under `Target` via identity copy.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] CLI accepts `SOURCE_DIR`, `TARGET_DIR`, `REMOTE_URL`, and a non-interactive passphrase override, each overridable via flag/env var
- [ ] `git` operations (clone, fetch, status, commit, push) are invoked as subprocess calls to the real `git` binary, discovered via `PATH`
- [ ] `apply` renders every plain file in `Source` to its mirrored relative path under `Target` via identity copy
- [ ] Running `apply` twice with no changes in `Source` is a no-op (idempotent — no needless rewrites)
- [ ] An integration test drives `apply` against temp `Source`/`Target` directories and asserts the rendered files match byte-for-byte
