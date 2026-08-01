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
One piece of a composed Target, stored as a file inside a `<target-name>.d/` directory in Source. Fragments render in lexical filename order into a single Target file. A composed Target is derived-only: hand-edits to it show up under Diff but cannot be captured back into Source via Save — only discarded via Reset.
_Avoid_: partial, snippet

**Secret**:
A file in Source whose name ends in `.age`. Its content is only ever decrypted during Apply/Diff, never stored in Source as plaintext. Diffing a Secret compares a freshly-decrypted copy of Source against the plaintext Target — never ciphertext against plaintext.
_Avoid_: credential

**Passphrase**:
The single, shared secret used to derive the encryption key for every Secret, on every device. Prompted fresh on any command that needs to decrypt something; never cached or stored.

**Save**:
The operation that captures live edits in Target back into Source, commits, and pushes to Remote. Local wins. Shows a diff and requires explicit confirmation before it runs.
_Avoid_: sync, push, commit — each names only part of what this does, or collides with a narrower git operation.

**Reset**:
The operation that discards local drift in both Source and Target, forces Source to match Remote, then re-applies. Remote wins. Shows a diff and requires explicit confirmation before it runs.
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
A Package installed immediately during Apply.

**Lazy package**:
A Package not installed until first invoked, via a generated shim — a thin wrapper script in the isolated prefix that calls `mise x <specifier>@<version> -- <bin_name>`.

**Application Log**:
The per-device record of everything mysh has done to that machine: every file it created vs. overwrote (with the original backed up), every Package installed, every PATH/rc-file line it added, and the bootstrap installer's own footprint. What makes Teardown possible.

**Teardown**:
The operation that reverses everything in the Application Log, returning the device to its state from before mysh's install script was ever run.
