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
One piece of a composed Target, stored as a file inside a `<target-name>.frag/` directory in Source. Fragments render in lexical filename order into a single Target file. A composed Target is derived-only: hand-edits to it show up under Diff but cannot be captured back into Source via Save — only discarded via Update. The suffix is `.frag`, not the more common `.d` (fish's native `conf.d`, `sudoers.d`, etc.), precisely so a native multi-file directory like that can be tracked as plain/Directory-mode content without mysh mistaking it for a Fragment to compose.
_Avoid_: partial, snippet

**Secret**:
A file in Source whose name ends in `.age`. Its content is only ever decrypted during Apply/Diff, never stored in Source as plaintext. Diffing a Secret compares a freshly-decrypted copy of Source against the plaintext Target — never ciphertext against plaintext.
_Avoid_: credential

**Overlay**:
A file in Source whose name ends in `.overlay` (e.g. `.claude.json.overlay`, declaring keys for Target's `.claude.json`), holding only the specific keys mysh should enforce onto an existing Target file it doesn't otherwise own — unlike Fragment, which composes a Target's entire content from pieces it fully owns. Apply shallow-merges the declared keys onto Target's current content, creating the file from `{}` if it doesn't exist yet; every other key already in Target is left untouched and is never read into Source. Derived-only, like Fragment: Diff reports drift only when a declared key's value disagrees with Target, and that drift can't be captured back into Source via Save — only re-enforced via Apply/Update. The type to parse/merge as is picked from the filename left after stripping `.overlay` (`.json` today; other types are rejected, not guessed at). Teardown leaves an Overlay's Target in place — mysh never owned the file's other keys, so the declared keys stay merged as residue rather than risking them (see ADR-0008).
_Avoid_: patch, sync

**Passphrase**:
The single, shared secret used to derive the encryption key for every Secret, on every device. Prompted fresh on any command that needs to decrypt something; never cached or stored.

**Add**:
The operation that starts tracking a new path in Source, dispatching on what currently exists at that exact path under Target: an existing file is copied in (`--secret` to encrypt it), an existing directory becomes a `.track`-marked mirror after confirmation, and a path that exists in neither is a Package Specifier. Only ever touches Source, never git history — and, for the Package case, also renders the new Shim straight into Target, so all three shapes leave Target and Source agreeing the moment Add returns. An already-tracked path is rejected outright, not merged.
_Avoid_: track, import, install — each collides with a narrower thing Add's package case also does.

**Diff**:
Reports drift across the three-state model — Target vs a fresh render of Source, and Source vs Remote — as a list of paths and which side changed, without touching anything. Remote drift is reported per path as Ahead, Behind, or Diverged rather than one flat side: Ahead means Source has content Remote doesn't (a Save candidate), Behind means Remote has content Source doesn't (an Update candidate), and Diverged means both sides changed the same path since they last agreed (mysh does no three-way merge, so this is never auto-resolved either direction). Any listed path can be inspected further for the actual content difference behind the drift, not just that it exists — decrypted plaintext for a Secret, never ciphertext. A `--quick` mode trades accuracy for speed — no network call, and Secret/Fragment drift judged against a cached Fingerprint instead of freshly decrypted/composed content — cheap enough to run on every shell-prompt render; see ADR-0012.
_Avoid_: status

**Fingerprint**:
A per-device, per-unit content hash recorded at Apply and Save — the two moments Source and Target are known to agree — kept in its own disposable cache file, separate from the Application Log. Lets Diff's `--quick` mode judge a Secret or Fragment unit's Target drift by comparing a hash instead of decrypting or composing it. Losing this cache costs nothing but a stale/unknown quick-Diff reading until the next Apply or Save; unlike the Application Log, Teardown never depends on it.
_Avoid_: hash, checksum, cache — each names only the mechanism, not what it's for.

**Save**:
The operation that captures live edits in Target back into Source and pushes Source's current state to Remote — including anything already sitting in Source unpushed (e.g. from Add) even where Target itself hasn't drifted. Local wins. Shows a diff and requires explicit confirmation before it runs, narrowable to specific paths before committing.
_Avoid_: sync, push, commit — each names only part of what this does, or collides with a narrower git operation.

**Update**:
The operation that discards local drift in both Source and Target, forces Source to match Remote, then re-applies. Remote wins. Also refreshes the mysh binary itself: the installed binary is hash-compared against the current release asset for this device's arch, and replaced if different — independent of the Source-side change, and regardless of whether Source had any drift to resolve. Shows a diff of the Source-side change and requires explicit confirmation before that part runs. Refuses outright if any path is Diverged — mysh does no three-way merge, so it will not silently pick Remote's side of a path both sides changed; that has to be resolved with git directly first.
_Avoid_: reset — the old name; renamed because it collided with "put Target back to what Source already says," which Apply already does on its own. pull, merge — mysh does not attempt three-way merges; drop into git directly for that.

**File-mode tracking**:
The default for any file in Source: only that exact path is managed. A sibling file that appears live in the same Target directory is invisible to mysh — never scanned, never flagged.

**Directory-mode tracking**:
Opted into per-directory by placing a `.track` marker file at that directory's root in Source. On Diff, mysh recursively walks the live Target directory (plain filesystem walk, independent of git) and compares the file list against Source: files present in Target but not Source are flagged **new** (candidate for Save); files present in Source but missing from Target are flagged **missing** (candidate for Update).

**`.track`**:
The marker file that turns on Directory-mode tracking for the directory it lives in. Its content doubles as an ignore list — one glob pattern per line; empty file means track everything underneath.

**Package**:
A CLI tool/binary managed via `mise`, declared as a Shim and installed into an isolated, mysh-owned prefix so it can be cleanly removed. Not a system package (library, GUI app, compiler) — those are out of scope, see ADR-0005.

**Specifier**:
The `mise`-format string a Package declares (e.g. `github:owner/repo@latest`, `npm:pkg@version`): backend prefix, tool identifier, optional version pin. What a Shim actually execs. The bin name a Package installs under defaults to the specifier's last `/`-segment with backend prefix and version pin stripped, overridable with `--bin` at Add time.
_Avoid_: package name, tool name — a specifier is the exact syntax mise resolves, not a human label.

**Shim**:
The real, portable file a Package renders as: a thin wrapper script that resolves `$HOME` and `mise` at run time rather than baking in a device-specific path, so the identical file is correct on every device it's rendered to. Execs `mise x <specifier> -- <bin name> "$@"`. An Eager package's shim carries the `# mysh: eager` marker line; a Lazy package's doesn't. A hand-edit that no longer matches this shape simply isn't prewarmable as Eager — it stays Lazy, never an error.
_Avoid_: wrapper, stub — name it by what it's for, not just its shape.

**Eager package**:
A Package whose Shim carries the `# mysh: eager` marker line. Same mechanism as a Lazy package — the marker only changes *when* the tool is installed: Apply collects every eager Shim's Specifier and prewarms them all in one batched `mise install` (mise parallelizes the downloads), so always-needed tools are ready before first use. See ADR-0007.

**Lazy package**:
A Package declared as a Shim (not a line in a declarations file). Not installed until first invoked.

**Application Log**:
The per-device record of everything mysh has done to that machine: every Target it rendered — each entry carrying its ownership explicitly, `full` (created, or overwritten with the original backed up) vs `partial` (Overlay: only declared keys merged) — plus the mise bootstrap, every PATH/rc-file line added, and the bootstrap installer's own footprint. What makes Teardown possible. Schema v2, see ADR-0009; installed Packages are not logged individually — Teardown removes the isolated mise prefix wholesale.

**Teardown**:
The operation that reverses everything in the Application Log, returning the device to its state from before mysh's install script was ever run. One deliberate exception: an Overlay's Target is left in place, declared keys still merged (see ADR-0008).
