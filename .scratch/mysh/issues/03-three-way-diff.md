# 03 — Three-way diff for plain files

**What to build:** A `diff`/`status` command that reports drift across all three states — `Target` (live disk), `Source` (local repo), and `Remote` — for plain files.

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** ready-for-agent

- [ ] `diff` reports `Target` vs `Source` drift (a live edit on disk not yet captured)
- [ ] `diff` reports `Source` vs `Remote` drift (a commit pushed from elsewhere not yet pulled)
- [ ] `diff` reports both simultaneously when they occur together, distinguishing which side changed
- [ ] No drift anywhere produces clean/empty output
- [ ] Tests cover each drift combination independently and combined, using the injected temp `Source`/`Target`/`Remote`
