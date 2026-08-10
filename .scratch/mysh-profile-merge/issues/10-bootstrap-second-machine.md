# 10 — Bootstrap end-to-end on a second machine

**Type:** task

**Status:** resolved

**Blocked by:** 04, 05, 09

## Question

The destination's actual acceptance test: on a genuinely fresh second machine, run
the one-line `curl -fsSL <raw bootstrap.sh url> | sh` against the now-public
`ichsansting/mysh`, and confirm it downloads the binary, sparse-clones `profile/` as
Source, applies successfully, and prompts for the passphrase on hitting the real
`.age` secrets (ticket 09) — decrypting them correctly. HITL: needs the user's actual
second machine.

## Comments

Ran the real bootstrap one-liner on a fresh machine. `apply` completed (silently —
it prints nothing on success, which read as "nothing happened"), but every lazy
package shim except `python`'s own failed: `rg --version` errored with

```
mise ERROR failed to parse template: '{{ tools.python.path }}'
mise ERROR Variable `tools.python.path` not found in context
```

Root cause: `profile/.config/mise/config.toml` set `env.UV_PYTHON` to the
unconditional template `{{ tools.python.path }}`. Each lazy shim invokes
`mise x <its-own-specifier> -- <bin> "$@"`, scoping mise to just that one tool —
so `tools.python` isn't in the template context unless the tool being invoked
*is* python, and rendering aborts the whole command before the real binary ever
runs. Broke every lazy tool except python.

Reproduced locally against the real `mise` binary and fixed by guarding the
template: `{% if tools.python is defined %}{{ tools.python.path }}{% endif %}`.
Verified both directions — `rg --version` now runs clean, and `UV_PYTHON` still
resolves when python is the active tool. Fix applied to
`profile/.config/mise/config.toml`.

Ticket 09 landed, unblocking the rest of this test. Ran `mysh apply` on this
machine: all three secrets decrypted correctly — `~/.ssh/id_ed25519`,
`~/.claude/.credentials.json`, `~/.config/gh/hosts.yml` all present, `gh auth
status` confirmed the token valid.

`mysh save` then failed with a `git push` username/password prompt. Root cause
wasn't mysh: `~/.gitconfig`'s `credential.https://github.com.helper` (and the
`gist.github.com` one) was set to the bare string `gh auth git-credential`,
missing the leading `!`. Without `!`, git treats the value as
`<name> <args>` and prepends `git-credential-` to only the first word —
`git-credential-gh`, which doesn't exist — instead of running the whole thing
as a shell command. Reproduced directly with `git push --dry-run`:
`'credential-gh' is not a git command` → falls through to the disabled
terminal-prompt fallback. Not a `mysh` bug — the `.gitconfig` entry itself was
malformed (unclear whether `gh auth setup-git` wrote it that way originally or
something else rewrote it later).

Fixed by hand: `helper = !gh auth git-credential` (kept relative, not the
absolute `gh` binary path `gh auth setup-git --hostname github.com` writes —
both work, relative was the smaller diff off the broken line). Verified with
`git push --dry-run` (auth succeeds, no prompt), then ran the real `mysh save`
— pushed clean (`85f7237`), working tree clean, up to date with
`origin/main`.

Full acceptance test now passes end-to-end: fresh-machine bootstrap, apply,
real secret decryption, and save/push all confirmed working.
