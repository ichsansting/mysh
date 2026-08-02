# 04 — Save & Reset for plain files

**What to build:** The two opposite mutating operations for plain files: `save` (local wins — capture `Target` drift into `Source`, commit, push) and `reset` (remote wins — discard local drift, force `Source` to match `Remote`, re-apply). Both always show the pending diff and require explicit confirmation before acting.

**Blocked by:** 03 — Three-way diff for plain files

**Status:** done

- [x] `save` captures `Target` drift into `Source`, commits, and pushes to `Remote`
- [x] `save` shows the pending diff and requires explicit confirmation before committing/pushing
- [x] `reset` discards local drift in both `Source` and `Target`, force-fetches `Remote`, and re-applies
- [x] `reset` shows the pending diff and requires explicit confirmation before executing
- [x] Declining confirmation on either command leaves `Source`, `Target`, and `Remote` unchanged
- [x] Tests cover `save` and `reset` end-to-end against the injected temp repos, including the declined-confirmation no-op case

## Comments

Implemented in new `src/save.rs` (`save()`) and `src/reset.rs` (`reset()`), sharing a `src/confirm.rs` helper that prints the pending diff and reads a y/N line from an injected `BufRead` (real stdin from the CLI). `save` filters `diff::diff` for `Target`-drifted paths, copies their `Target` content into `Source` (removing the `Source` file if it was deleted on `Target`), then `git commit` + `git push`. `reset` runs `diff::diff` (which already fetches `Remote`), hard-resets `Source` to the upstream ref via a new `git::reset_hard`, then re-applies via the existing `apply::apply` to force `Target` back in line. Both commands no-op ("nothing to save"/"nothing to reset") when there's no drift, and leave everything untouched when the confirmation is declined. Wired up as `mysh save`/`mysh reset` in `src/main.rs`. Tests in `tests/save_integration.rs` and `tests/reset_integration.rs` cover the confirmed-mutation, declined-no-op, and no-drift-no-op cases for each command, driving the compiled binary with piped stdin against real temp git repos. `cargo test` and `cargo clippy --all-targets` pass clean. Reviewed via `/code-review` (Standards: a few optional two-occurrence duplication smells, left as-is per YAGNI; Spec: 0 real findings). Committed as `8321ccc`.
