# Update refreshes the mysh binary by hash-comparing against the single release, not rebuilding per device

`update` (renamed from `reset`, see the `CONTEXT.md` diff alongside this ADR) already forces
Source to match Remote and re-applies. It now also keeps the installed `mysh` binary itself
current: on every run, it downloads the release asset matching this device's OS/arch (the
same `mysh-<arch>-<os>` naming `bootstrap.sh` and `release.sh` already use), hash-compares it
against what's installed at `.mysh/bin/mysh`, and replaces it if different. Publishing is
unchanged — a maintainer (or a user extending their own fork) still runs `release.sh` by hand
to force-move the single `v1` tag and its assets; `update` only ever pulls.

## Considered and rejected

- **Rebuild from source on each device.** mysh already knows how to bootstrap a Rust
  toolchain via `mise` (that's how `release.sh`'s own cross-build works), so this was the
  most-tempting alternative — no release pipeline needed at all, always correct per-arch by
  construction. Rejected: it forces a Rust toolchain onto every device mysh manages, not just
  the ones a maintainer uses to cut a release, and turns every `update` into a compile step
  instead of a file download.
- **Store per-arch binaries in Source, committed by `save`.** Rejected: it would make `save`
  responsible for a full cross-compile (`cargo zigbuild` for every target, exactly what
  `release.sh` already does) on every device that happens to modify mysh's own source, and
  bloats Source's git history with binary blobs for a use case (personally extending mysh)
  that already has a path — run `release.sh` yourself.

## Consequences

- A device only ever needs `curl`/an HTTP client to stay current, never a Rust toolchain,
  unless it separately builds/releases mysh itself.
- The correctness of `update`'s binary refresh is exactly as strong as `release.sh`'s
  per-arch asset naming already is — no new arch-matching logic to get right.
- Anyone who wants to run a modified `mysh` across their own devices publishes it the same
  way an upstream release would: `release.sh`, pointed at their own fork/release repo.
