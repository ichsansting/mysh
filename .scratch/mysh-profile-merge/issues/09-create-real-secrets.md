# 09 — Create and push the real secrets

**Type:** task

**Status:** resolved

**Blocked by:** 04, 06

## Question

Issue 14 (`.scratch/mysh/issues/14-secret-creation-cli-gap.md`) names three real
secrets deliberately left out of the original `dotfiles` seed because there was no
supported way to encrypt them: an SSH private key, a GitHub token, a Claude OAuth
token. With `mysh add --secret` now available and passphrase re-entry confirmed
(ticket 06), add each as a real `.age` file in `profile/` and `mysh save` to push.
Pick and record the passphrase somewhere durable (mysh never stores it — see
ADR-0003) before starting.

## Answer

Added all three via `mysh add <path> --secret` (`.ssh/id_ed25519`,
`.config/gh/hosts.yml`, `.claude/.credentials.json`), passphrase supplied via
`MYSH_PASSPHRASE` from a throwaway local file, deleted immediately after. Verified
only `.age` files landed in `profile/` (no plaintext).

Pushed via plain `git add`/`commit`/`push` scoped to the three `.age` files instead
of `mysh save` — `save` re-decrypts every tracked secret to diff against `Target`
for drift, which needs the passphrase again and hit an interactive-prompt/no-tty
failure (`ENXIO`) in this sandboxed shell; since `add` already wrote correct
ciphertext with no drift to capture, a plain git push is equivalent and needs no
passphrase. Commit `7431eb3` is on `origin/main`.

## Comments
