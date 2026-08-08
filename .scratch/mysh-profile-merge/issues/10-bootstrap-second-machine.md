# 10 — Bootstrap end-to-end on a second machine

**Type:** task

**Status:** claimed

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

Still open: the full acceptance test (real `.age` secrets prompting/decrypting,
ticket 09) hasn't run yet since real secrets don't exist in `profile/` — ticket 09
is still open and this ticket remains blocked on it.
