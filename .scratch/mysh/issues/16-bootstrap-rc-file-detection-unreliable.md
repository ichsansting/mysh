# 16 — Bootstrap: rc-file detection off `$SHELL` picks the wrong file, or none

**What to build:** `bootstrap.sh`'s rc-file selection (`bootstrap.sh:60-67`, pre-fix) picked which file to append the `PATH` line to purely off the `$SHELL` env var — the account's *login* shell, not necessarily what's actually interactive when `bootstrap.sh` runs. Under Docker in particular, `$SHELL` is frequently unset or `/bin/sh`, silently falling through to `.profile` — a file plenty of interactive shells (bash run non-login, for one) never source. Net effect: `PATH` never reaches the shell the user actually types into, and `.mysh/bin`/`.mysh/mise/shims` end up unreachable even though `apply` ran successfully.

**Blocked by:** 10 — Bootstrap one-liner (this refines its rc-file-detection step, doesn't replace it)

**Status:** done

- [x] Detection prefers the real parent process of the `sh` running `bootstrap.sh` (`/proc/$PPID/comm`, Linux-only, no new dependency — procfs is always present) over the `$SHELL` env var
- [x] Falls back to the previous `$SHELL`-based `case` logic when `/proc` isn't available (e.g. macOS, not reported broken there)
- [x] `case` patterns match both a path-shaped value (`$SHELL`, e.g. `/bin/bash`) and a bare name (`/proc/.../comm` returns just `bash`, no path)
- [x] Manually verified under `dash` (the actual interpreter bootstrap.sh's `#!/bin/sh` resolves to on Debian/Ubuntu): `$PPID` and `/proc/$PPID/comm` both resolve correctly, correctly identifying the invoking shell

**Found while:** grilling a live bug report ("eager install works inconsistently, sometimes I can't launch things after bootstrap, varies mac/linux-arm/linux-docker"). Reproduced live in a Debian ARM sandbox that this session runs inside: `/proc/$PPID/comm` correctly resolved to `bash` where `$SHELL` would have been whatever the account's passwd entry says, not necessarily what invoked the pipe.

## Comments

Existing tests in `tests/bootstrap_integration.rs` all set `MYSH_RC_FILE` explicitly, bypassing this detection branch entirely — so this change isn't covered by (or breaking) that suite. `cargo test` itself couldn't be run to confirm no regression: this sandbox's own `cargo`/`rustc` are mysh lazy packages, and invoking them here triggered `rustup toolchain install stable`, which hung — the sandbox's network access to `crates.io`/rustup's install servers appears blocked (a `403` from a Heroku edge in front of `crates.io` was observed directly). Verification for this change is therefore: `bash -n` syntax check, plus manual `dash -c` execution of the exact detection snippet, both passing.

## Agent Brief

**Category:** bug
**Summary:** `bootstrap.sh` picked the rc file to append `PATH` to based on the `$SHELL` env var alone, which is unreliable (esp. under Docker) and can silently mean the `PATH` edit lands nowhere the user's actual shell reads.

**Current behavior (pre-fix):**
```sh
case "${SHELL:-}" in
    */zsh) rc_file="$HOME/.zshrc" ;;
    */bash) rc_file="$HOME/.bashrc" ;;
    *) rc_file="$HOME/.profile" ;;
esac
```

**Desired/implemented behavior:**
Prefer `/proc/$PPID/comm` (the actual parent shell process) when readable; fall back to the original `$SHELL`-based logic otherwise. `case` arms extended to match both path-shaped and bare-name values.

**Key interfaces:**
- `bootstrap.sh`'s rc-file-detection block, immediately before the `PATH`-line append block.
- `MYSH_RC_FILE` env var still overrides detection entirely, unchanged — this is the seam `tests/bootstrap_integration.rs` uses.

**Out of scope:**
- Fish shell — deliberately left unaddressed this round (fish never sources `.bashrc`/`.zshrc`/`.profile` at all; see issue 17 for the related-but-separate `MISE_DATA_DIR` durability fix, which also stops short of touching fish). Revisit only if fish's own PATH exposure becomes a live complaint.
- Writing to multiple candidate rc files "just in case" — considered and explicitly rejected per user preference (one correct file, not several guessed ones).
