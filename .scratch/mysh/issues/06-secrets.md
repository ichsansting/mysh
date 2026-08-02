# 06 — Secrets

**What to build:** Passphrase-encrypted `Secret` files. A `.age`-suffixed file in `Source` is decrypted on `apply`/`diff` and participates in `save`/`reset` like any other file, with a single shared Passphrase prompted fresh per command and never cached.

**Blocked by:** 04 — Save & Reset for plain files

**Status:** done

- [x] A file ending in `.age` in `Source` is treated as a `Secret`; its rendered `Target` path has the suffix stripped
- [x] The Passphrase is requested interactively, fresh, on any command that needs to decrypt a `Secret`, and is never cached, stored, or handed to a keychain/agent
- [x] The encryption key is derived from the Passphrase via Argon2id; content is encrypted with an AEAD cipher via an established, audited crate (no hand-rolled crypto)
- [x] `diff` on a `Secret` decrypts a fresh copy of `Source` and compares plaintext-to-plaintext against `Target` — never ciphertext-to-plaintext
- [x] Decrypted `Target` files are written with restrictive permissions (no group/world access)
- [x] `save` on an edited `Secret` re-encrypts the live `Target` content back into `Source`'s `.age` file
- [x] `reset` on a `Secret` re-decrypts `Source` into `Target`, discarding local drift
- [x] Tests cover the encrypt/decrypt round-trip, decrypted-vs-decrypted diffing, permission bits on the written file, and `save`/`reset` for a `Secret`

## Comments

Implemented in `src/secret.rs`: `is_secret`/`strip_suffix` for the `.age` naming convention; `encrypt`/`decrypt` (Argon2id via the `argon2` crate deriving a 32-byte key per call from a fresh random 16-byte salt, XChaCha20-Poly1305 via the `chacha20poly1305` crate, envelope layout `salt(16) || nonce(24) || ciphertext+tag`, randomness from `getrandom`); `passphrase_provider` builds a `FnMut` closure that returns the `--passphrase`/`MYSH_PASSPHRASE`-configured value if set, otherwise prompts once via `rpassword` (no terminal echo) and reuses that answer only for the rest of the process — never written to disk/keychain/agent. `write_restricted` creates decrypted `Target` files with `0600` via `OpenOptions::mode` (atomic — no window at default permissions) and re-chmods afterward to also cover the pre-existing-file-backup case.

`apply`/`diff`/`save`/`reset`/`main` now thread a `get_passphrase: &mut PassphraseFn` through, only invoked lazily when a `Secret` is actually encountered — commands touching no secret are never prompted. `diff::FileDrift` gained `source_path` (raw, `.age`-suffixed) and `is_secret` alongside the existing `path` (now always `Target`-relative); `Target`-vs-`Source` drift decrypts fresh `Source` ciphertext and compares plaintext-to-plaintext against live `Target` content, while `Source`-vs-`Remote` drift compares raw ciphertext bytes (no decryption needed there — `save` only ever rewrites the `.age` file when the decrypted content actually changed, so a stable blob is a valid proxy for "nothing changed," and neither the issue nor spec.md requires decrypting `Remote`).

Tests: unit tests in `src/secret.rs` (round-trip, wrong-passphrase failure, fresh-nonce-per-call, suffix handling) plus `tests/secret_integration.rs` driving the real CLI end-to-end — decrypt-on-apply with permission-bit assertions, decrypted-vs-decrypted diffing (including a case that would show false drift under a naive ciphertext-vs-plaintext comparison), re-encrypt-on-save, and re-decrypt-on-reset. `cargo test` (17 lib+integration suites) and `cargo clippy --all-targets` pass clean. Reviewed via `/code-review`: Standards axis found no hard violations (no documented standards in-repo; two minor "duplicated code" judgement calls noted — `write_restricted` vs `copy_if_changed`'s shared read-compare-write shape, and `passphrase_provider(...)` constructed identically in `main.rs`'s four match arms — left as-is, not worth an abstraction for this little duplication). Spec axis found one real issue (the permission-window gap above, now fixed) and confirmed the rest of the checklist against the issue text and ADR-0003.
