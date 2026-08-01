# 06 — Secrets

**What to build:** Passphrase-encrypted `Secret` files. A `.age`-suffixed file in `Source` is decrypted on `apply`/`diff` and participates in `save`/`reset` like any other file, with a single shared Passphrase prompted fresh per command and never cached.

**Blocked by:** 04 — Save & Reset for plain files

**Status:** ready-for-agent

- [ ] A file ending in `.age` in `Source` is treated as a `Secret`; its rendered `Target` path has the suffix stripped
- [ ] The Passphrase is requested interactively, fresh, on any command that needs to decrypt a `Secret`, and is never cached, stored, or handed to a keychain/agent
- [ ] The encryption key is derived from the Passphrase via Argon2id; content is encrypted with an AEAD cipher via an established, audited crate (no hand-rolled crypto)
- [ ] `diff` on a `Secret` decrypts a fresh copy of `Source` and compares plaintext-to-plaintext against `Target` — never ciphertext-to-plaintext
- [ ] Decrypted `Target` files are written with restrictive permissions (no group/world access)
- [ ] `save` on an edited `Secret` re-encrypts the live `Target` content back into `Source`'s `.age` file
- [ ] `reset` on a `Secret` re-decrypts `Source` into `Target`, discarding local drift
- [ ] Tests cover the encrypt/decrypt round-trip, decrypted-vs-decrypted diffing, permission bits on the written file, and `save`/`reset` for a `Secret`
