# 07 — Show the passphrase prompt as asterisks instead of fully silent

**Type:** research

**Status:** resolved

## Question

`secret::passphrase_provider` uses `rpassword::prompt_password`, which is fully
silent (no echo at all, not even asterisks) — the user has no feedback that keystrokes
are registering. `rpassword` (the only relevant existing dependency) doesn't support
masked/asterisk echo; it's deliberately silent-only.

Research what it takes to get asterisk-masked input instead, without pulling in a
heavy new dependency: does any already-installed crate support it, is there a small
well-maintained crate purpose-built for masked prompts (vs. hand-rolling raw-terminal
character-by-character echo), and what's the actual security tradeoff being accepted
(masked length-revealing echo vs. fully silent) — worth surfacing since ADR-0003
already accepts a fairly permissive security posture for this single-user tool.
Report back a recommendation for ticket 06/passphrase-entry code to act on.

## Answer

The ticket's premise was wrong: `rpassword` 7.5.4 — already pinned in
`Cargo.toml`, no version bump needed — ships `ConfigBuilder::password_feedback_mask(char)`
plus `prompt_password_with_config`, giving asterisk-masked echo out of the box.
`dialoguer::Password` has no masking option at all; `inquire::Password` does support
it but is a full prompt-UI framework, unjustified for one passphrase line. No new
dependency.

Fix (drop into `passphrase_provider`, `src/secret.rs`, replacing the
`rpassword::prompt_password("mysh passphrase: ")` call):

```rust
let config = rpassword::ConfigBuilder::new()
    .password_feedback_mask('*')
    .build();
let entered = rpassword::prompt_password_with_config("mysh passphrase: ", config)
    .map_err(|e| e.to_string())?;
```

Security note: masked echo reveals passphrase length on screen (unlike fully
silent); accepted as a small additive tradeoff on top of ADR-0003's already-permissive
single-shared-passphrase posture.

Full findings, sources, and crate survey:
`.scratch/mysh-profile-merge/research-passphrase-masking.md` on branch
`research/passphrase-masking` (commit `0bbb3e1`) — not merged to `main`, fetch via
`git show research/passphrase-masking:.scratch/mysh-profile-merge/research-passphrase-masking.md`.

## Comments
