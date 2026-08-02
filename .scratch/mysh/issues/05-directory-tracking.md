# 05 — Directory-mode tracking (`.track`)

**What to build:** Opt-in whole-directory tracking. A directory marked with a `.track` file at its root is recursively walked on `diff` (independent of git) and compared against `Source`'s contents for that directory, so new or missing files are surfaced automatically instead of staying invisible.

**Blocked by:** 03 — Three-way diff for plain files

**Status:** done

- [x] A `.track`-marked directory is recursively walked on `diff`, comparing the live `Target` listing against `Source`'s listing for that directory
- [x] A file present in `Target` but absent from `Source`, under a `.track`-marked directory, is flagged **new** (a `save` candidate)
- [x] A file present in `Source` but absent from `Target`, under a `.track`-marked directory, is flagged **missing** (a `reset` candidate)
- [x] A directory without `.track` never scans for sibling files — file-mode tracking (the default) only manages files explicitly present in `Source`
- [x] `.track`'s content is parsed as newline-separated glob patterns; matching files are excluded from the new/missing scan
- [x] An empty `.track` file tracks everything under that directory
- [x] Tests cover: new file detection, missing file detection, ignore-pattern exclusion, and confirming file-mode directories are never scanned

## Comments

Implemented in `src/diff.rs`: `tracked_dirs()` recursively finds directories in `Source` with a `.track` marker at their root (skipping `.git`/`.mysh`); `tracked_new_paths()` walks each tracked directory's live `Target` counterpart (independent of git) and, for any path not already known via `Source`/`Remote`, checks it against that directory's ignore patterns (`track_patterns()` — `.track`'s content as newline-separated globs, blank lines dropped) via `matches_ignore()`. Non-excluded new paths are merged into `diff()`'s existing path set, so the pre-existing per-path Source/Target/Remote comparison naturally flags them as `target_drift` (a `save` candidate) — missing files (present in `Source`, absent from `Target`) were already caught by that same existing loop, .track or not. A small hand-rolled `glob_match()` (`*`/`?`) avoids pulling in a `glob` crate; `matches_ignore` treats slash-containing patterns as full-path matches and bare patterns as matching any path component (so a bare directory name excludes its whole subtree). `apply::walk_files` (shared by both `apply` and this new `Target`-side walk) now also skips `.mysh`, mysh's own state dir, so directory-tracking a directory containing it doesn't flag mysh's own log/backups as unmanaged new files.

Tests in `tests/diff_integration.rs`: new-file detection, missing-file detection, ignore-pattern exclusion (basename wildcard, slash-qualified full path, and bare directory-component patterns), and a regression test confirming a directory without `.track` never surfaces a sibling file. `cargo test` and `cargo clippy --all-targets` pass clean. Reviewed via `/code-review` (Standards: fixed a duplicated `.git`/`.mysh` skip predicate and a stale doc comment; Spec: removed unit tests against private helpers that violated the spec's CLI-entrypoint-only testing decision, folding that coverage into the CLI-driven ignore-pattern test instead). Committed as `593dcc3`.
