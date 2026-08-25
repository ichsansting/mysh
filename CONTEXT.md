# mysh

A git-backed home-folder manager: syncs dotfiles, packages, and secrets across as many devices (including temporary ones) as possible, with minimal dependencies and a full, clean uninstall.

## Language

**Source**:
The git working tree in the repo — the human-editable form of every managed file (plain files, fragments, encrypted secrets).
_Avoid_: repo, config

**Target**:
The live, rendered file on disk (e.g. `~/.bashrc`), produced by Apply. Also directly hand-editable.
_Avoid_: applied file, live file

**Remote**:
The git remote holding the canonical shared history of Source. Public — see ADR-0004.

**Apply**:
The render step that turns Source into Target: identity copy for plain files, decrypt for Secrets, concatenate-in-filename-order for Fragment directories.

**Fragment**:
One piece of a composed Target, stored as a file inside a `<target-name>.frag/` directory in Source. Fragments render in lexical filename order into a single Target file. A composed Target is derived-only: hand-edits to it show up under Diff but cannot be captured back into Source via Save — only discarded via Reset. The suffix is `.frag`, not the more common `.d` (fish's native `conf.d`, `sudoers.d`, etc.), precisely so a native multi-file directory like that can be tracked as plain/Directory-mode content without mysh mistaking it for a Fragment to compose.
_Avoid_: partial, snippet

**Secret**:
A file in Source whose name ends in `.age`. Its content is only ever decrypted during Apply/Diff, never stored in Source as plaintext. Diffing a Secret compares a freshly-decrypted copy of Source against the plaintext Target — never ciphertext against plaintext.
_Avoid_: credential

**Overlay**:
A file in Source whose name ends in `.overlay` (e.g. `.claude.json.overlay`, declaring keys for Target's `.claude.json`), holding only the specific keys mysh should enforce onto an existing Target file it doesn't otherwise own — unlike Fragment, which composes a Target's entire content from pieces it fully owns. Apply shallow-merges the declared keys onto Target's current content, creating the file from `{}` if it doesn't exist yet; every other key already in Target is left untouched and is never read into Source. Derived-only, like Fragment: Diff reports drift only when a declared key's value disagrees with Target, and that drift can't be captured back into Source via Save — only re-enforced via Apply/Reset. The type to parse/merge as is picked from the filename left after stripping `.overlay` (`.json` today; other types are rejected, not guessed at). Teardown leaves an Overlay's Target in place — mysh never owned the file's other keys, so the declared keys stay merged as residue rather than risking them (see ADR-0008).
_Avoid_: patch, sync

**Passphrase**:
The single, shared secret used to derive the encryption key for every Secret, on every device. Prompted fresh on any command that needs to decrypt something; never cached or stored.

**Diff**:
Reports drift across the three-state model — Target vs a fresh render of Source, and Source vs Remote — as a list of paths and which side changed, without touching anything. Remote drift is reported per path as Ahead, Behind, or Diverged rather than one flat side: Ahead means Source has content Remote doesn't (a Save candidate), Behind means Remote has content Source doesn't (a Reset candidate), and Diverged means both sides changed the same path since they last agreed (mysh does no three-way merge, so this is never auto-resolved either direction). Any listed path can be inspected further for the actual content difference behind the drift, not just that it exists — decrypted plaintext for a Secret, never ciphertext. A `--quick` mode trades accuracy for speed — no network call, and Secret/Fragment drift judged against a cached Fingerprint instead of freshly decrypted/composed content — cheap enough to run on every shell-prompt render; see ADR-0012.
_Avoid_: status

**Fingerprint**:
A per-device, per-unit content hash recorded at Apply and Save — the two moments Source and Target are known to agree — kept in its own disposable cache file, separate from the Application Log. Lets Diff's `--quick` mode judge a Secret or Fragment unit's Target drift by comparing a hash instead of decrypting or composing it. Losing this cache costs nothing but a stale/unknown quick-Diff reading until the next Apply or Save; unlike the Application Log, Teardown never depends on it.
_Avoid_: hash, checksum, cache — each names only the mechanism, not what it's for.

**Save**:
The operation that captures live edits in Target back into Source and pushes Source's current state to Remote — including anything already sitting in Source unpushed (e.g. from Add) even where Target itself hasn't drifted. Local wins. Shows a diff and requires explicit confirmation before it runs, narrowable to specific paths before committing.
_Avoid_: sync, push, commit — each names only part of what this does, or collides with a narrower git operation.

**Reset**:
The operation that discards local drift in both Source and Target, forces Source to match Remote, then re-applies. Remote wins. Shows a diff and requires explicit confirmation before it runs. Refuses outright if any path is Diverged — mysh does no three-way merge, so it will not silently pick Remote's side of a path both sides changed; that has to be resolved with git directly first.
_Avoid_: pull, merge — mysh does not attempt three-way merges; drop into git directly for that.

**File-mode tracking**:
The default for any file in Source: only that exact path is managed. A sibling file that appears live in the same Target directory is invisible to mysh — never scanned, never flagged.

**Directory-mode tracking**:
Opted into per-directory by placing a `.track` marker file at that directory's root in Source. On Diff, mysh recursively walks the live Target directory (plain filesystem walk, independent of git) and compares the file list against Source: files present in Target but not Source are flagged **new** (candidate for Save); files present in Source but missing from Target are flagged **missing** (candidate for Reset).

**`.track`**:
The marker file that turns on Directory-mode tracking for the directory it lives in. Its content doubles as an ignore list — one glob pattern per line; empty file means track everything underneath.

**Package**:
A CLI tool/binary managed via `mise`, installed into an isolated, mysh-owned prefix so it can be cleanly removed. Not a system package (library, GUI app, compiler) — those are out of scope, see ADR-0005.

**Eager package**:
A Package whose shim file carries the `# mysh: eager` marker line. Same file, same mechanism as a Lazy package — the marker only changes *when* the tool is installed: Apply collects every eager shim's specifier and prewarms them all in one batched `mise install` (mise parallelizes the downloads), so always-needed tools are ready before first use. See ADR-0007.

**Lazy package**:
A Package declared as a real, portable shim file in Source (not a line in a declarations file) — a thin wrapper script whose content resolves `$HOME` and `mise` at run time rather than baking in a device-specific path, so the identical file is correct on every device it's rendered to. Rendered into the isolated prefix like any ordinary tracked file. Not installed until first invoked.

**Application Log**:
The per-device record of everything mysh has done to that machine: every Target it rendered — each entry carrying its ownership explicitly, `full` (created, or overwritten with the original backed up) vs `partial` (Overlay: only declared keys merged) — plus the mise bootstrap, every PATH/rc-file line added, and the bootstrap installer's own footprint. What makes Teardown possible. Schema v2, see ADR-0009; installed Packages are not logged individually — Teardown removes the isolated mise prefix wholesale.

**Teardown**:
The operation that reverses everything in the Application Log, returning the device to its state from before mysh's install script was ever run. One deliberate exception: an Overlay's Target is left in place, declared keys still merged (see ADR-0008).
