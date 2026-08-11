# 17 — `MISE_DATA_DIR` isolation never made durable, so bare `mise` escapes it

**What to build:** ADR-0006's whole premise is that mysh packages install into an isolated, `target`-owned data dir (`mise::data_dir`, `<target>/.mysh/mise`) so `Teardown` can delete it wholesale. That isolation is real *inside mysh's own subprocess calls* — `package::install_declared` and every generated lazy shim (`package::shim_script`) explicitly `export MISE_DATA_DIR=...` before invoking `mise`. But nothing makes that env var durable anywhere a human — or any other tool — invokes `mise` directly: `bootstrap.sh` only ever adds `PATH` to the rc file, never `MISE_DATA_DIR`. Result: a bare `mise` command (typed by the user, or run by anything else on the machine) resolves to mise's own real default data dir instead, silently creating a **second, out-of-sync install location** that can each be partially populated depending on which one a given invocation happened to see.

**Blocked by:** 15 — Packages: real lazy shims in Source, eager via mise's native `[tools]` table (this closes a durability gap that ADR-0006 assumed but never wired up)

**Status:** done

- [x] `bootstrap.sh` exports `MISE_DATA_DIR` in the same rc file it already adds `PATH` to, immediately after the `PATH` block
- [x] Checked independently of `path_line`'s own idempotency guard, so a machine already bootstrapped before this fix (rc file has the old `PATH` line but no `MISE_DATA_DIR` line) still picks up the new line on the next `bootstrap.sh` run, rather than being permanently skipped by the outer `path_line` presence check
- [x] Reuses the existing `bootstrap-path-added` Application Log entry kind (already a generic `(rc_file, line)` pair per `src/log.rs`) — no Rust changes needed, `teardown.rs`'s existing `BootstrapPathAdded` handling strips it automatically
- [x] Verified idempotent and self-healing under `dash`: an rc file seeded with only the pre-fix `PATH` line gets exactly the new `MISE_DATA_DIR` line appended once; a second run adds nothing further

**Found while:** grilling the same bug report as issue 16. Live evidence gathered in this session's own sandbox (which already runs mysh, dogfooded):

```
~/.local/share/mise/installs/   (mise's real default dir, predates this session's own mysh apply)
  → fish, atuin, direnv, eza, fzf, gh, starship, zellij, zoxide   (9 tools — exactly the eager set)

~/.mysh/mise/installs/          (mysh's isolated dir, populated by today's actual apply run)
  → same 9 eager tools, PLUS bat, helix, jq, node, npm-anthropic-ai-claude-code (lazy, on first use)
```

A bare `mise list` (no `MISE_DATA_DIR` override — what a user actually types) read the *first* directory, not mysh's isolated one — reproduced in a fully clean (`env -i`) shell to rule out ambient env-var contamination from this session's own tooling.

## Comments

This single gap explains the two vaguest parts of the original report: "after bootstrap, `mise list` doesn't reflect the install" (bare `mise list` reads the wrong, empty-at-that-point directory) and "`mise list` shows installed but I still can't launch it" (the tool got installed into the *other*, non-isolated dir by some earlier bare `mise` invocation, so mysh's isolated `.mysh/mise/shims` — the one actually on `PATH` — never got that tool's shim).

Deliberately scoped to the one rc file `bootstrap.sh` already picks (see issue 16) — the user explicitly rejected writing to multiple candidate rc files. Fish is explicitly out of scope for the same reason issue 16 leaves it out: fish never sources this rc file at all, and touching fish's own config was declined for this round.

`cargo test` could not be run to verify no regression in this sandbox — see issue 16's Comments for why (network-blocked `rustup` hang). Verified instead via direct `dash` execution of the exact snippet, against both a fresh rc file and one seeded with the pre-fix state, plus applying the same fix live to this sandbox's own `~/.bashrc` (and its matching Application Log entry) as a real-world smoke test.

## Agent Brief

**Category:** bug
**Summary:** `MISE_DATA_DIR` is scoped correctly inside every subprocess call mysh's own Rust code makes, but was never exported anywhere persistent — so any `mise` invocation outside mysh's own generated shims (most commonly: the user typing `mise` directly) silently resolves against a different, non-isolated data directory, defeating ADR-0006's isolation guarantee and producing state that looks nondeterministic (installed vs. not, reachable vs. not) depending on which directory a given check happens to hit.

**Current behavior (pre-fix):** `bootstrap.sh` adds only a `PATH` line to the detected rc file. `MISE_DATA_DIR` exists only transiently, inside individual `mise` subprocess invocations mysh's own code makes.

**Desired/implemented behavior:** `bootstrap.sh` also durably exports `MISE_DATA_DIR="<target>/.mysh/mise"` in the rc file, guarded and logged the same way the existing `PATH` line already is, but independently idempotency-checked so it self-heals on already-bootstrapped machines.

**Key interfaces:**
- `bootstrap.sh`, immediately after the existing `PATH`-line block.
- `mise::data_dir` (`src/mise.rs`) is the Rust-side source of truth for the same path — `bootstrap.sh`'s `mise_data_dir` variable must stay in sync with it (`<target>/.mysh/mise`) since nothing enforces that at compile time (`bootstrap.sh` predates the `mysh` binary existing on the machine, so it can't just ask the binary).
- `LogEntry::BootstrapPathAdded` (`src/log.rs`) / `teardown.rs`'s handling of it — reused as-is, no changes.

**Out of scope:**
- Fish shell's own `MISE_DATA_DIR` exposure — same reasoning as issue 16.
- `MISE_CONFIG_DIR` — not exported, since `target` is `$HOME` in the common case and mise's own default config lookup already finds `$HOME/.config/mise/config.toml` without an override; only diverges for a non-`$HOME` `target` (test/custom scenarios), not reported as a live problem.
- Reconciling or migrating whatever ended up in the stray non-isolated `~/.local/share/mise` on already-affected machines — this fix stops new drift, it doesn't clean up existing duplication. A future `teardown`/`doctor`-style command could detect and offer to merge/delete it if this turns out to matter in practice.
