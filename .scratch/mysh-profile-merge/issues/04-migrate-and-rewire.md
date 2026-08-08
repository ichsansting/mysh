# 04 — Migrate the profile into profile/ and rewire bootstrap.sh

**Type:** task

**Status:** resolved

**Blocked by:** 01, 02, 03

## Question

Do the actual migration: copy `ichsansting/dotfiles`'s tracked content
(`.config/`, `.gitconfig.d/`, `.packages`, `.claude/`) into this repo at `profile/`,
and rewire `bootstrap.sh` for the one-repo, sparse-checkout model decided in tickets
01–02:

- Replace the `REMOTE_URL` placeholder/clone step with the sparse-checkout sequence
  against `ichsansting/mysh` (or `MYSH_REMOTE_URL` override), landing at
  `profile/` inside the sparse clone.
- Point `--source-dir` at `<sparse-clone>/profile`.
- Update `spec.md`'s conventions and any doc references to the old two-repo model
  (e.g. `bootstrap.sh`'s own header comment, which currently says it's "hosted at the
  root of a mysh-managed dotfiles repo").
- Confirm `.gitignore` (`/target` only today) doesn't need a `profile/`-scoped
  exclusion for anything the sparse checkout shouldn't track.

Single commit in `dotfiles.git` today ("Initial commit") — no history worth
preserving, a plain copy is fine.

**Discovered while claiming this ticket — `src/git.rs` needs two fixes for a
subdirectory source-dir to work at all, verified empirically against a scratch repo:**

- `git ls-tree` and `git show <rev>:<path>` behave differently from a subdirectory:
  `ls-tree` already auto-scopes to cwd and returns cwd-relative paths (no change
  needed), but `git show <rev>:<bare-path>` resolves the path against the **repo
  root**, not cwd — it errors outright on a bare relative path from a subdirectory.
  `git.rs`'s `show()` must build the spec as `{rev}:./{relative_path}` (leading `./`)
  so it resolves against `repo_dir` regardless of whether `repo_dir` is the repo root
  or a subdirectory. Required — `diff`/`save` break entirely without this once
  `--source-dir` is `<clone>/profile`.
- `commit()`'s `git add -A` has no pathspec — confirmed empirically that modern git's
  `-A` with no pathspec stages the **entire working tree**, not just cwd-and-below,
  when run from a subdirectory. Change to `git add -A -- .` to scope staging to
  `repo_dir`. Defense-in-depth: harmless in the intended bootstrap flow (a dedicated
  sparse `~/.mysh/source` clone with nothing else in it) but would otherwise silently
  stage unrelated tool-source changes if `mysh save` were ever run with `--source-dir`
  pointed at a subdirectory of a working tree that has other content.
- `reset_hard()`'s `git reset --hard <rev>` is **not** pathspec-scopable (`git reset
  --hard` rejects pathspecs) — it always resets the whole working tree/index from
  whatever cwd it's run in. Confirmed empirically: uncommitted changes in a sibling
  directory get discarded too. Not fixing the plumbing (real subtree-scoped reset
  needs checkout+diff+rm, disproportionate for a misuse case) — instead documenting
  in `git.rs` and `CONTEXT.md`/`spec.md` that `--source-dir` must always point at a
  dedicated Source clone (exactly what `bootstrap.sh`'s `~/.mysh/source` already is),
  never a subdirectory of a working tree with other unrelated uncommitted state (e.g.
  never point mysh directly at this repo's own `profile/` during local mysh
  development — use a separate clone).

## Answer

Done. Summary of changes:

- `profile/` created, populated from `git@github.com:ichsansting/dotfiles.git`'s
  tracked content (`.config/`, `.gitconfig.d/`, `.claude/`, `.packages`) — scanned for
  credential patterns, none found.
- `src/git.rs`: `commit()` now uses `git add -A -- .` (was unscoped `-A`, which stages
  the whole repo from any subdirectory); `show()` now uses a `./`-prefixed pathspec
  (bare paths resolve against repo root, not cwd, and error from a subdirectory);
  `reset_hard()` got a doc comment on its whole-repo blast radius. New regression
  test: `commit_and_show_scope_to_a_subdirectory_source_dir` in
  `tests/git_integration.rs`.
- `bootstrap.sh`: `REMOTE_URL` now defaults to `https://github.com/ichsansting/mysh`
  (no more CHANGE_ME placeholder/fork-and-edit step); Source fetch is now
  `git clone --filter=blob:none --no-checkout` + `sparse-checkout set profile` +
  `checkout`; hands off `--source-dir "$SOURCE_DIR/profile"`.
- `src/config.rs`: zero-flag default `source_dir` is now
  `target_dir/.mysh/source/profile` (was `target_dir/.mysh/source`), to keep matching
  bootstrap.sh's handoff value for every command run after the first.
- `tests/bootstrap_integration.rs`, `tests/teardown_integration.rs`,
  `tests/zero_flag_integration.rs`: fixtures updated to seed content under `profile/`
  instead of the fake remote's root, matching the new layout.
- `.scratch/mysh/spec.md`: Repo layout, Bootstrap, and Trust model bullets updated
  for the one-repo/`profile/`-subdirectory model.
- Full test suite green (`cargo test`, 92 tests).

## Comments
