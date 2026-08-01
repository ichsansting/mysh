# 02 — Application Log foundation + pre-existing-file backup

**What to build:** The `Application Log` — the per-device record that later powers `teardown` — and the behavior it first enables: when `apply` targets a path that already has content mysh doesn't yet manage, back it up before overwriting, and record whether each managed path was "created fresh" or "overwrote existing (backup recorded)."

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** ready-for-agent

- [ ] `Application Log` format is defined and persisted per-device
- [ ] On `apply`, a `Target` path with pre-existing, not-yet-managed content is backed up before being overwritten
- [ ] The log distinguishes "created fresh" from "overwrote existing" (with the backup location) per managed path
- [ ] Re-applying to an already-managed path does not re-trigger the backup step or misclassify it as pre-existing
- [ ] Tests cover both cases: a fresh `Target` path (no backup, logged as created) and a pre-existing `Target` path (backup created, logged as overwritten)
