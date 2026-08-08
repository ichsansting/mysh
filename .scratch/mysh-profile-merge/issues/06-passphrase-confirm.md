# 06 — Confirm passphrase re-entry on first secret creation

**Type:** task

**Status:** resolved

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

## Answer

Added `secret::new_secret_passphrase(configured: &Option<String>)` in `src/secret.rs`:
if `--passphrase`/`MYSH_PASSPHRASE` was given, returns it as-is (nothing was typed blind,
so nothing to re-check); otherwise prompts twice via `rpassword::prompt_password` and
loops, re-prompting both, until the two entries match.

Wired in only at `add.rs`'s `file_add`, the sole call site that creates a new Secret
(`add --secret` on an untracked file). `apply`/`diff`/`save`/`reset` all decrypt an
*existing* Secret, never call this path, and keep using the plain cached
`passphrase_provider`.

Since `add.rs` only ever needed the passphrase once (at `file_add`'s encrypt call), the
existing `&mut PassphraseFn` closure parameter was replaced end-to-end (`add::add`,
`add::file_add`, and the `main.rs` "add" dispatch) with the raw `Option<String>` from
`config.passphrase` — simpler than threading a second closure alongside the first, and
removes a now-pointless caching layer for a value only read once.

Files touched: `src/secret.rs`, `src/add.rs`, `src/main.rs`.

Verified: `cargo build`, `cargo test` (all 74 tests across the suite, including the
existing `file_add_secret_writes_age_suffixed_file_that_round_trips` which exercises the
configured/no-confirm branch), and `cargo clippy --all-targets` (clean, no new
warnings).

## Comments
