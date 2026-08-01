# 10 — Bootstrap one-liner

**What to build:** The single-command onboarding flow. A `bootstrap.sh` hosted at the root of the user's own repo detects OS/architecture, downloads the matching prebuilt `mysh` binary from `mysh`'s own GitHub Releases, puts it on `PATH` (logged), clones the same repo as `Source`, and hands off to the binary to bootstrap `mise` and run the initial `apply`.

**Blocked by:** 01 — Scaffold + plain-file Apply, 02 — Application Log foundation + pre-existing-file backup

**Status:** ready-for-agent

- [ ] `bootstrap.sh` exists at the root of the managed repo and is runnable via a single `curl -fsSL <url> | sh`
- [ ] The script detects OS/architecture and downloads the matching prebuilt `mysh` binary from `mysh`'s own GitHub Releases
- [ ] The script places the binary on `PATH` and records that addition in the `Application Log`
- [ ] The script clones the repo it was fetched from as `Source`
- [ ] The script hands off to the `mysh` binary to bootstrap `mise` and run the initial `apply`
- [ ] The script's logic is tested against a simulated bare environment (only `git` present) without requiring a real network `curl` in CI
