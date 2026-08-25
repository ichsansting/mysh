# `diff --quick`: a disposable Fingerprint cache instead of decrypting, no `fetch`

`diff` needed a mode cheap and safe enough to run on every shell-prompt render (see the
`mysh` Starship module). Plain `diff` can't be that mode as-is: it decrypts every Secret
and composes every Fragment to know if Target drifted (blocking on the passphrase prompt
the moment any Secret exists), and it `git fetch`es before judging Remote drift (a network
call on every prompt). `diff --quick` skips both: Target drift for Secret/Fragment units is
judged by comparing the live Target's content hash against a Fingerprint recorded in a new,
disposable cache file (`~/.mysh/fingerprints`, decoupled from the Application Log, whose
correctness Teardown actually depends on — see ADR-0009's ownership-explicit lesson) written
by Apply and Save, the two points Source and Target are known to agree; Remote drift is
judged against `origin/main` as last fetched, the same staleness contract `git_status`
prompts already carry. A path with no Fingerprint recorded yet is reported as unknown, not
clean — there's nothing to compare against before that unit's first Apply/Save on this
device.

Remote drift, in both `diff` and `diff --quick`, is now reported per-path as `ahead` /
`behind` / `diverged` (via `git merge-base`) rather than one flat `remote` bucket — the
distinction the whole feature exists to expose. `diverged` is deliberately never
auto-resolved either direction; mysh does not attempt three-way merges.

## Considered and rejected

Holding the passphrase in `MYSH_PASSPHRASE` for the whole shell session was the first idea —
it already works with zero code changes. Rejected: it's inherited by every child process the
session spawns, readable by anything that can read that shell's `/proc/<pid>/environ`, and
is a direct, standing exception to the "prompted fresh, never cached" contract in ADR-0003 —
for a feature that only needs an instant yes/no on a prompt line, not real content.
