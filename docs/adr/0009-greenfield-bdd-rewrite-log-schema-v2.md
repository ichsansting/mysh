# Greenfield BDD rewrite; Application Log schema v2 with explicit ownership

The v1 codebase (2,687 lines) had grown four independent decision points for the
five-way render taxonomy (plain / Secret / Fragment / Overlay / `.track`), an
Application Log that implicitly assumed full ownership of every Target until
Overlay bolted a second shape onto it (the root cause behind ADR-0008's data-loss
bug), string-typed errors, and a hand-rolled test harness duplicated across 14
files. We rewrote `src/` and `tests/` from scratch — spec-first from CONTEXT.md
and ADRs 0001–0008 — rather than refactor incrementally. The 78-scenario behavior
inventory was transcribed from the old integration-test names into Gherkin
`features/*.feature` files *before* the old code was deleted, and a checksum
baseline of the rendered `profile/` guards the render contract across the swap.

Decisions with lasting consequences:

- **BDD via cucumber.** Behavior lives in `features/` and runs through the real
  binary (`cargo test --test bdd`); `tests/bdd.rs` + `tests/steps.rs` hold the
  World and step definitions. Dev-dependencies only (`cucumber`, `tokio`);
  runtime dependency floor unchanged.
- **One classifier.** `domain/render.rs::enumerate` is the single walk-and-
  classify every op consumes (`RenderKind` + `SourcePlan`); Directory-mode is
  part of the same pass but deliberately not a `RenderKind`.
- **Application Log schema v2 — ownership is explicit.** The three v1 target
  kinds collapse into one: `target\t<full|partial>\t<rel>[\t<backup-rel>]`.
  Teardown's ADR-0008 exception (leave Overlay targets in place) is now a
  schema property (`partial`), not a special-cased entry kind. `package-installed`
  is dropped (it was cosmetic — teardown removes the mise prefix wholesale).
  `bootstrap-installed` and `bootstrap-path-added` are byte-for-byte unchanged:
  they are a contract with bootstrap.sh, which still greps and appends them from
  POSIX sh. Unknown first fields are still skipped silently (forward compat).
  **No migration:** devices applied with v1 must teardown with the v1 binary
  (or clean `~/.mysh` by hand) before re-bootstrapping — accepted deliberately,
  single-user, few devices.
- **`--remote-url` is gone.** It was parsed and never read in v1; Remote is
  Source's own git `origin`, and only bootstrap.sh consumes `MYSH_REMOTE_URL`.
- **`.track` markers no longer render into Target.** v1 copied the marker file
  into the home directory; v2 keeps it Source-side — the only intentional
  render-output change, confirmed by the baseline diff (all other rendered
  files byte-identical).
- **The `.age` envelope format is frozen** (`salt(16) || nonce(24) ||
  ciphertext+tag`, Argon2id-derived key): existing committed Secrets decrypt
  unchanged.
