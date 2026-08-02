# 12 — Zero-flag daily usage

**What to build:** The missing link for "easy to use, minimal to remember, intuitive" — asserted in prose in spec.md's Problem Statement and CONTEXT.md, but never a checked requirement: after a normal `bootstrap.sh` run, every documented command must work with no flags and no manually-set environment variables. Concretely, `Config::resolve` must default `source_dir` to `target_dir.join(".mysh/source")` when no `--source-dir`/`MYSH_SOURCE_DIR` override is given — mirroring the exact convention `bootstrap.sh` already establishes when it clones `Source` there. Today there is no such default, so `mysh diff`/`save`/`reset`/`apply` all hard-fail post-bootstrap unless the user manually passes `--source-dir` or exports `MYSH_SOURCE_DIR` themselves.

**Blocked by:** 01 — Scaffold + plain-file Apply, 10 — Bootstrap one-liner

**Status:** ready-for-agent

- [ ] `Config::resolve` defaults `source_dir` to `target_dir.join(".mysh/source")` when neither `--source-dir` nor `MYSH_SOURCE_DIR` is set, matching `bootstrap.sh`'s own convention
- [ ] Explicit `--source-dir`/`MYSH_SOURCE_DIR` still override the default, unchanged
- [ ] After a simulated bootstrap (clone into the default location, binary on `PATH`), running `mysh apply`, `mysh diff`, `mysh save`, `mysh reset`, and `mysh teardown` each succeed with **no flags and no env vars set**
- [ ] A regression test asserts the previous behavior (hard error with no default) is gone — a fresh `Config::resolve` with no flags/env set, run against a `target_dir` that has `.mysh/source`, resolves without error
- [ ] Sweep `CONTEXT.md`/`docs/adr` for any other place "easy to use / minimal to remember" was asserted only in prose, and either turn it into a checked acceptance criterion here or note explicitly that it's already satisfied and why (as this ticket does for `REMOTE_URL`, package shim `PATH` placement, and the passphrase prompt)
