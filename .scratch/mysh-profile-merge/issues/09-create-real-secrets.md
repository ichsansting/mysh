# 09 — Create and push the real secrets

**Type:** task

**Status:** open

**Blocked by:** 04, 06

## Question

Issue 14 (`.scratch/mysh/issues/14-secret-creation-cli-gap.md`) names three real
secrets deliberately left out of the original `dotfiles` seed because there was no
supported way to encrypt them: an SSH private key, a GitHub token, a Claude OAuth
token. With `mysh add --secret` now available and passphrase re-entry confirmed
(ticket 06), add each as a real `.age` file in `profile/` and `mysh save` to push.
Pick and record the passphrase somewhere durable (mysh never stores it — see
ADR-0003) before starting.

## Comments
