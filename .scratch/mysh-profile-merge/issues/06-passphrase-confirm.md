# 06 — Confirm passphrase re-entry on first secret creation

**Type:** task

**Status:** open

## Question

`mysh add <path> --secret` prompts once for the passphrase (via
`secret::passphrase_provider`) and encrypts with whatever was typed — no validation,
no re-entry. A typo at creation time isn't discoverable until a later `apply`/`diff`
fails to decrypt with "wrong passphrase or corrupted file" — indistinguishable from
actual corruption.

Add a confirm-reentry step specifically for the case where a *new* Secret is being
created (not for `apply`/`diff`/`save`/`reset`, which only ever decrypt an existing
one): prompt twice, and if they don't match, ask again rather than silently writing a
mismatched Secret.

## Comments
