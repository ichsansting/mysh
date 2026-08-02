# 10 — Bootstrap one-liner

**What to build:** The single-command onboarding flow. A `bootstrap.sh` hosted at the root of the user's own repo detects OS/architecture, downloads the matching prebuilt `mysh` binary from `mysh`'s own GitHub Releases, puts it on `PATH` (logged), clones the same repo as `Source`, and hands off to the binary to bootstrap `mise` and run the initial `apply`.

**Blocked by:** 01 — Scaffold + plain-file Apply, 02 — Application Log foundation + pre-existing-file backup

**Status:** done

- [x] `bootstrap.sh` exists at the root of the managed repo and is runnable via a single `curl -fsSL <url> | sh`
- [x] The script detects OS/architecture and downloads the matching prebuilt `mysh` binary from `mysh`'s own GitHub Releases
- [x] The script places the binary on `PATH` and records that addition in the `Application Log`
- [x] The script clones the repo it was fetched from as `Source`
- [x] The script hands off to the `mysh` binary to bootstrap `mise` and run the initial `apply`
- [x] The script's logic is tested against a simulated bare environment (only `git` present) without requiring a real network `curl` in CI

## Comments

`bootstrap.sh` is POSIX `/bin/sh`, not Rust — the only new code surface in this repo that isn't. OS/arch is mapped to the Rust target-triple naming convention (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, etc.) so a GitHub Releases asset can just be `mysh-<triple>`; the download URL uses GitHub's `/releases/latest/download/<asset>` redirect, avoiding an API/`jq` dependency for the default case (`MYSH_VERSION` can still pin a tag). The binary installs to `<target>/.mysh/bin/mysh` — the same isolated, `PATH`-resident prefix `mise.rs` already uses for its own bootstrap and for lazy-package shims (`src/mise.rs`'s `bin_dir`), so there's one directory both to add to `PATH` and for a future `teardown` to delete. Two new tab-separated Application Log entry kinds are appended directly by the shell script (`bootstrap-installed`, `bootstrap-path-added`) since the `mysh` binary doesn't exist yet at that point in the flow — `AppLog` (`src/log.rs`) has no Rust-side counterpart for these; a future `teardown` ticket will need to parse them by hand. Placing the binary on `PATH` for future shells means editing a shell rc file (detected via `$SHELL`, overridable via `MYSH_RC_FILE`), not just copying the binary — both the binary placement and the rc-file edit are idempotent (grep-guarded) so rerunning `bootstrap.sh` doesn't duplicate log entries or PATH lines.

`REMOTE_URL` reuses the same env var name (`MYSH_REMOTE_URL`) and default-empty-placeholder pattern already established by `src/config.rs`, satisfying the testing seam (a test points it at a local `file://` bare remote, no real network needed for the clone step either). `MYSH_RELEASES_REPO` is a separate, fixed-infrastructure override (mysh's own release host, not per-user) used only by tests.

Tested end-to-end in `tests/bootstrap_integration.rs`: a fake `curl` on `PATH` writes a fake `mysh` executable to the `-o` destination instead of hitting the network, and that fake binary records its own invocation so the mise-bootstrap-and-apply hand-off is provable; `git` is real, cloning from a local bare remote. Covers the full happy path (download, PATH placement + log, Source clone, hand-off with `apply --source-dir ... --target-dir ...`) and a rerun test proving no duplicate PATH lines or log entries. `write_executable`, previously duplicated in `tests/package_integration.rs`, moved to `tests/support.rs` and is now shared by both.

Reviewed via `/code-review` against `HEAD` (022c8db). Standards axis: one hard violation (`REPO_URL` naming collided with CONTEXT.md's `_Avoid_: repo` list for the `Source` vocabulary term, and didn't match `config.rs`'s `remote_url` convention) — fixed by renaming to `REMOTE_URL`; one duplicated-code judgement call (`write_executable`) — fixed by extracting to `tests/support.rs`. Spec axis: one real bug (the placeholder guard compared `REPO_URL` against a `DEFAULT_REPO_URL` that mirrored itself, so even a correctly-edited script would always refuse to run without also setting `MYSH_REMOTE_URL` — breaking the "single `curl | sh`" requirement) — fixed by matching on the placeholder text itself instead of on whether an env var was set, verified manually against an edited copy of the script.
