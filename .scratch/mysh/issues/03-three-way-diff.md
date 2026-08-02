# 03 — Three-way diff for plain files

**What to build:** A `diff`/`status` command that reports drift across all three states — `Target` (live disk), `Source` (local repo), and `Remote` — for plain files.

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** done

- [x] `diff` reports `Target` vs `Source` drift (a live edit on disk not yet captured)
- [x] `diff` reports `Source` vs `Remote` drift (a commit pushed from elsewhere not yet pulled)
- [x] `diff` reports both simultaneously when they occur together, distinguishing which side changed
- [x] No drift anywhere produces clean/empty output
- [x] Tests cover each drift combination independently and combined, using the injected temp `Source`/`Target`/`Remote`

## Comments

Implemented in `src/diff.rs` (new `diff()`/`format_drifts()`) plus small additions to `src/git.rs` (`upstream_ref`, `list_tree`, `show`) and a `pub(crate)` promotion of `apply::walk_files` for reuse. `diff` fetches `Remote`, unions the relative paths present in `Source`'s working tree with those tracked at the upstream ref, and for each compares `Target`/`Source`/`Remote` content, tagging drifted paths as `target`, `remote`, or both. New `mysh diff` CLI command wired in `src/main.rs`. Tests in `tests/diff_integration.rs` cover: no drift (clean output), target-only drift, remote-only drift (including a file not yet pushed and a file that only exists on the remote), and both together. `cargo test` and `cargo clippy --all-targets` pass clean.
