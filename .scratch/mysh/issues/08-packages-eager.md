# 08 — Packages: eager install via mise

**What to build:** Package declarations and eager installation. `mysh` declares CLI-tool packages (install specifier, optional binary-name override, eager/lazy classification), self-bootstraps `mise` if it's missing, and installs every eager package during `apply`.

**Blocked by:** 01 — Scaffold + plain-file Apply

**Status:** ready-for-agent

- [ ] Packages are declared with a `mise`-compatible specifier — either bare (`go@latest`, for tools `mise` natively supports) or backend-prefixed (`github:`, `npm:`, `pip:`, `cargo:`, etc.) — plus an eager/lazy classification
- [ ] The resulting binary name defaults to the bare specifier name and only needs to be declared explicitly when a backend-prefixed install produces a differently-named binary (e.g. `github:elio-fm/elio` → `elio`)
- [ ] If `mise` is not already present on the device, `mysh` installs it automatically and records the bootstrap in the `Application Log`
- [ ] Every package declared eager is installed via `mise install` during `apply`
- [ ] Tests cover: `mise` absent → bootstrapped and logged; an eager package declared → installed and runnable after `apply` (using a stubbed `mise` on `PATH` to avoid real network installs)
