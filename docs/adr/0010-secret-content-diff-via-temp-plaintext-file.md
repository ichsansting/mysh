# Secret content-diff decrypts to a temp file rather than diffing in-memory

Diff's per-path content view needs a real diff for a drifted Secret, which means comparing
two plaintexts: Source decrypted (memory-only today) against Target's live plaintext. `git
diff` — already used for every other content diff — needs two file paths, so getting a real
diff here means writing the decrypted Source plaintext to a `0600` temp file, immediately
removed after the diff runs.

Considered keeping decrypted bytes entirely in-process (a hand-rolled in-memory line-diff)
to avoid any new plaintext-on-disk exposure. Rejected: Target already holds this same
Secret's plaintext on disk at `0600` during ordinary Apply, so a short-lived `0600` temp
copy isn't a new class of exposure — and it lets Secret diffing reuse the same `git diff`
path as everything else instead of maintaining a second diff implementation for one file
type. This is a scoped exception for the diff view specifically, not a precedent for
handling decrypted Secret content elsewhere.
