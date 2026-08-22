# Eager packages are marked shims, prewarmed by one batched `mise install`; mise's shims dir leaves `PATH`

ADR-0006 split eager and lazy packages into two mechanisms: eager declared in mise's native `[tools]` table and exposed through mise's own shims dir on `PATH`, lazy as real shim files in `.mysh/bin`. The eager half failed in practice: `bootstrap.sh` put `.mysh/mise/shims` at the front of `PATH`, and mise's shim directory intercepts commands mise knows but has no version configured for — observed live shadowing and breaking a host machine's pre-existing `cargo`. The reason eager existed at all was narrower than its mechanism: always-needed tools (fish, starship, atuin, ...) must be installed *in parallel, up front* during Apply, not serially on first use.

Two facts make the mechanism split unnecessary. `mise install <specifier>...` accepts explicit specifiers with no `[tools]` entry and parallelizes the downloads internally. And a shim's `mise x <specifier>` pins its version in the command itself, so resolution stays deterministic without any config entry — sidestepping exactly the silent highest-installed-version fallback ADR-0006 retested and rejected for mise's *native* shims.

So eagerness collapses to one bit on the existing lazy mechanism: every Package is a real shim file in Source's `.mysh/bin`, and an eager one carries a `# mysh: eager` marker line. Apply collects the specifiers out of marked shims (they're already in the `exec mise x <specifier> -- ...` line, written only by `add`) and prewarms them in a single batched `mise install <spec> <spec> ...`. The `[tools]` table, `mise config set/get` calls, and the shims dir's `PATH` entry are all deleted; `add --eager` now just writes a file, needs no `mise` at all, and honors `--bin` like lazy always did.

## Considered Options

- **Keep ADR-0006's split, but move `.mysh/mise/shims` to the *end* of `PATH`.** Rejected: shadowing then just reverses direction (host tools shadow mise-managed ones), resolution order becomes machine-dependent, and the second mechanism's whole cost — two `PATH` dirs, config-file writes, `mise` needed at `add` time — remains for no gain.
- **Make everything lazy, no eager at all.** Rejected: first shell startup on a new device would install fish, then starship, then atuin serially on first invocation — the exact slow path eager exists to avoid.
- **An eager declarations list (a file naming which shims to prewarm).** Rejected: a declarations file separate from the per-package source of truth is the `.packages` design ADR-0006 already killed; it can drift from the shims it points at.

## Consequences

- `PATH` needs only `.mysh/bin` again. The class of bug where mise's shim dir breaks host tools is gone for fresh bootstraps.
- **Migration, existing devices:** rc files written by the previous `bootstrap.sh` still contain the old `PATH` line with `.mysh/mise/shims` in it, and mise still regenerates that dir on installs — the shadowing risk persists there until the old line is removed (re-run teardown + bootstrap, or hand-edit the rc file). The Application Log's `bootstrap-path-added` entry still matches the old line, so teardown reverses it correctly.
- A hand-edited shim that no longer matches the `add`-written shape simply isn't prewarmed (it still renders and runs) — degraded to lazy, never an error.
- An eager tool's every invocation now goes through `mise x` (a few ms of resolution) instead of mise's native shim. For prompt-critical tools (starship), measure before assuming it matters; if it does, the fix is inside the shim (exec the resolved binary path), not a return to the shims dir.
- `apply` still re-records `package-installed` log entries on every apply that has eager packages — same cosmetic-duplication behavior as before (entries feed only teardown's summary text).
