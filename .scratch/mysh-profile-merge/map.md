# Map: Merge the profile into mysh, go live

## Destination

`ichsansting/mysh` becomes the single home for both the tool and the user's actual
profile: a `profile/` subdirectory holds what's currently
`git@github.com:ichsansting/dotfiles.git` (private, one commit, already shaped like
mysh's Source — `.config/`, `.gitconfig.d/`, `.packages`, `.claude/`). `bootstrap.sh`
sparse-clones just `profile/` as Source instead of cloning a separate repo. The repo
goes public per ADR-0004.

Done when: the real secrets deferred out of the original seed (SSH key, GitHub token,
Claude OAuth token — see issue 14's "Found while" note in `.scratch/mysh/issues/`) are
pushed as `.age` files, and the user has bootstrapped a second, fresh machine
end-to-end from the public repo — passphrase prompt and all.

## Notes

Domain: `CONTEXT.md` (Source/Target/Remote/Apply/Secret/Passphrase) and
`docs/adr/0001`–`0005`.

Standing constraint: ADR-0004 requires the Source repo to be public with only `.age`
files protected — the merged repo inherits that. Nothing outside an `.age` file may
ever hold anything sensitive, including inside `profile/.claude/`.

Gotcha: this repo already has its own root `.claude/` (Claude Code project skills for
hacking on mysh itself, including this wayfinder skill) — unrelated to
`profile/.claude/`, which is Source content Apply renders to `~/.claude` on target
machines. Don't confuse the two when editing.

**Execution override:** this map carries implementation, not just decisions. Task
tickets include doing the actual work (migration, code changes) — resolved one at a
time like any other ticket, not handed off to a separate build session.

Skills: `/grilling` for further open decisions; `/research` for external/crate
investigation.

## Decisions so far

- [Where the profile lives in the repo](issues/01-repo-shape.md) — subdirectory
  `profile/`; `--source-dir` points at it; Save/Reset/directory-tracking stay scoped
  to `profile/` only.
- [How bootstrap.sh fetches Source without the whole tool repo](issues/02-clone-strategy.md)
  — sparse checkout (`git clone --filter=blob:none` + `sparse-checkout set profile/`).
- [Whether the merged repo goes public](issues/03-repo-visibility.md) — yes, per
  ADR-0004; only `.age` files stay protected.
- [Asterisk-masked passphrase prompt](issues/07-passphrase-masking.md) — `rpassword`
  (already pinned) supports this natively via `ConfigBuilder::password_feedback_mask`;
  no new dependency, one-line fix in `passphrase_provider`.
- [Migrate profile/ and rewire bootstrap.sh](issues/04-migrate-and-rewire.md) — done;
  also fixed a real `git.rs` bug the subdirectory shape exposed (`add -A`/`git show`
  were repo-root-scoped, not source-dir-scoped).
- [Flip ichsansting/mysh to public](issues/05-flip-visibility.md) — pre-flip check
  found no secrets outside `.age` files (none exist yet); user confirmed; repo is now
  public.
- [Confirm passphrase re-entry on first secret creation](issues/06-passphrase-confirm.md)
  — `add --secret` now prompts twice and loops on mismatch, but only when the
  passphrase wasn't already given via `--passphrase`/`MYSH_PASSPHRASE`.

## Not yet specified

Whatever further profile content (additional secrets, packages, fragments) gets added
after the initial go-live isn't sharp until the user actually needs a specific one.

## Out of scope

See `spec.md`'s existing Out of Scope section (per-device variation, automatic merge,
full system packages, keychain integration, per-recipient secret revocation) —
unchanged by this effort.
