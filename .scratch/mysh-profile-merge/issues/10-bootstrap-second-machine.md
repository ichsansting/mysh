# 10 — Bootstrap end-to-end on a second machine

**Type:** task

**Status:** open

**Blocked by:** 04, 05, 09

## Question

The destination's actual acceptance test: on a genuinely fresh second machine, run
the one-line `curl -fsSL <raw bootstrap.sh url> | sh` against the now-public
`ichsansting/mysh`, and confirm it downloads the binary, sparse-clones `profile/` as
Source, applies successfully, and prompts for the passphrase on hitting the real
`.age` secrets (ticket 09) — decrypting them correctly. HITL: needs the user's actual
second machine.

## Comments
