# Add materializes a package's Shim into Target immediately, not on the next Apply

`add`'s three shapes were asymmetric: adding an existing file or an existing directory
copies content that's already live in Target into Source, so both sides already agree the
instant `add` returns. Adding a package specifier wrote a new Shim into Source only — Target
didn't have it until the next `apply`. Diff would then report a real, if harmless and
expected, drift window between the two commands.

`add`'s package case now renders its new Shim straight into Target as part of the same
operation — the same first-touch write (and Application Log entry) `apply` already performs
for any newly-tracked unit, invoked here for just the one unit `add` created. All three `add`
shapes now leave Target and Source agreeing the moment `add` returns.

## Considered and rejected

- **Leave the asymmetry, document it instead.** Rejected: it's a real, user-visible gap (a
  freshly-`add`ed package "isn't there yet" until you remember to `apply`), not just a naming
  quirk — worth fixing in behavior, not prose.
- **Require an explicit `apply` after every `add`.** Rejected: makes the common case (`add` a
  package, expect to use it) two commands instead of one, for no benefit over just doing the
  one-unit render inline.

## Consequences

- `add`'s package path now depends on the same render/write machinery `apply` uses, not just
  `fsx::write_if_changed` into Source.
- `add --eager` materializes the Shim into Target immediately but does not itself prewarm it
  via `mise install` — that stays an `apply`-time batching concern (ADR-0007); the tool is
  still installed lazily on first invocation until the next `apply` prewarms it.
