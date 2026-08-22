# Teardown leaves Overlay targets in place

Overlay's contract (CONTEXT.md) is that mysh enforces only the declared keys of a Target file and never owns the rest. Teardown broke that contract: the overlay apply path went through the same first-touch bookkeeping as every other render kind, so the whole pre-existing file was backed up and logged as overwritten (or logged as created when the overlay built it from `{}`). Replaying that log, teardown would restore a day-zero snapshot — destroying every key other programs (e.g. Claude Code writing into `.claude.json`) had accumulated since — or delete the file outright. For a config that grows daily, "full, clean uninstall" became data destruction for content mysh explicitly never owned.

The fix: an Overlay apply records its own Application Log entry kind, `overlay-touched` (once, on first touch, after a successful merge — no whole-file backup at all), and teardown's replay leaves the file exactly as it is, saying so in its pre-confirmation summary (`leave <path> (overlay; declared keys stay merged)`). The declared keys remain merged as residue.

## Considered Options

- **Key-level unmerge** — log the original values of just the declared keys (including "was absent") and have teardown restore exactly those. The only option that removes even the residue while sparing accumulated keys. Rejected for now: it needs a value-carrying log format and per-type unmerge logic, for residue that in practice is a couple of harmless keys (e.g. `hasCompletedOnboarding: true`). Revisit if an overlay ever declares a key whose leftover value could actually hurt.
- **Keep whole-file restore** (status quo). Rejected: it's the bug — the backup snapshots keys mysh never owned, and restoring it destroys everything written since.
- **Delete only when the overlay created the file.** Rejected: by teardown time a created file is indistinguishable from a pre-existing one — other programs have long since written their own keys into it, and deleting destroys them just the same.

## Consequences

- Teardown's "returns the device to its pre-mysh state" promise now carries one deliberate exception, stated in the summary output: overlay-declared keys stay merged.
- Overlay targets no longer produce `.mysh/backups/` entries, and are never classified created/overwritten.
- **Migration, existing devices:** a device that applied an overlay *before* this change may have the target logged as `created`/`overwritten` (with a whole-file backup) — teardown there would still delete/restore it. Fix by hand once per affected device: delete that line from `~/.mysh/log` (and the stale file under `~/.mysh/backups/`). Devices whose declared keys already matched at every apply never logged anything and need nothing.
- Old mysh binaries reading a new log skip `overlay-touched` lines (unrecognized kinds were already tolerated), so a version mismatch degrades to exactly this ADR's behavior: the file is left alone.
