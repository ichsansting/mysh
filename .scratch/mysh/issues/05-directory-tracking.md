# 05 — Directory-mode tracking (`.track`)

**What to build:** Opt-in whole-directory tracking. A directory marked with a `.track` file at its root is recursively walked on `diff` (independent of git) and compared against `Source`'s contents for that directory, so new or missing files are surfaced automatically instead of staying invisible.

**Blocked by:** 03 — Three-way diff for plain files

**Status:** ready-for-agent

- [ ] A `.track`-marked directory is recursively walked on `diff`, comparing the live `Target` listing against `Source`'s listing for that directory
- [ ] A file present in `Target` but absent from `Source`, under a `.track`-marked directory, is flagged **new** (a `save` candidate)
- [ ] A file present in `Source` but absent from `Target`, under a `.track`-marked directory, is flagged **missing** (a `reset` candidate)
- [ ] A directory without `.track` never scans for sibling files — file-mode tracking (the default) only manages files explicitly present in `Source`
- [ ] `.track`'s content is parsed as newline-separated glob patterns; matching files are excluded from the new/missing scan
- [ ] An empty `.track` file tracks everything under that directory
- [ ] Tests cover: new file detection, missing file detection, ignore-pattern exclusion, and confirming file-mode directories are never scanned
