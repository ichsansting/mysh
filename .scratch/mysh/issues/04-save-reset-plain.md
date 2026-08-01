# 04 — Save & Reset for plain files

**What to build:** The two opposite mutating operations for plain files: `save` (local wins — capture `Target` drift into `Source`, commit, push) and `reset` (remote wins — discard local drift, force `Source` to match `Remote`, re-apply). Both always show the pending diff and require explicit confirmation before acting.

**Blocked by:** 03 — Three-way diff for plain files

**Status:** ready-for-agent

- [ ] `save` captures `Target` drift into `Source`, commits, and pushes to `Remote`
- [ ] `save` shows the pending diff and requires explicit confirmation before committing/pushing
- [ ] `reset` discards local drift in both `Source` and `Target`, force-fetches `Remote`, and re-applies
- [ ] `reset` shows the pending diff and requires explicit confirmation before executing
- [ ] Declining confirmation on either command leaves `Source`, `Target`, and `Remote` unchanged
- [ ] Tests cover `save` and `reset` end-to-end against the injected temp repos, including the declined-confirmation no-op case
