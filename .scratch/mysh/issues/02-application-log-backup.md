# 02 — Application Log foundation + pre-existing-file backup

**What to build:** The `Application Log` — the per-device record that later powers `teardown` — and the behavior it first enables: when `apply` targets a path that already has content mysh doesn't yet manage, back it up before overwriting, and record whether each managed path was "created fresh" or "overwrote existing (backup recorded)."

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** done

- [x] `Application Log` format is defined and persisted per-device
- [x] On `apply`, a `Target` path with pre-existing, not-yet-managed content is backed up before being overwritten
- [x] The log distinguishes "created fresh" from "overwrote existing" (with the backup location) per managed path
- [x] Re-applying to an already-managed path does not re-trigger the backup step or misclassify it as pre-existing
- [x] Tests cover both cases: a fresh `Target` path (no backup, logged as created) and a pre-existing `Target` path (backup created, logged as overwritten)

## Comments

Implemented in `src/log.rs` (new `AppLog`, an append-only tab-separated log under `<target>/.mysh/log`) plus changes to `src/apply.rs` to back up pre-existing content to `<target>/.mysh/backups/<relative>` before the first write to a path, and to log the outcome only after the real write succeeds (so a crash mid-apply can't leave the log claiming a write that didn't happen). Tests in `tests/apply_integration.rs` cover fresh-path (created, no backup), pre-existing-path (backup + overwritten), and re-apply (no re-backup, no re-classification). `cargo test` and `cargo clippy --all-targets` pass clean.
