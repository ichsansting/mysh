# 09 — Packages: lazy install via shims

**What to build:** Lazy package installation. Instead of installing at `apply` time, `mysh` generates a thin shim per lazy package's binary name into its isolated `PATH`-resident prefix; running the plain command name transparently installs the tool on first real use.

**Blocked by:** 08 — Packages: eager install via mise

**Status:** ready-for-agent

- [ ] Lazy packages are not installed during `apply`
- [ ] `mysh` generates a shim script per lazy package's binary name into its isolated, `PATH`-resident prefix
- [ ] Invoking the plain binary name for a lazy package runs the shim, which calls `mise x <specifier>@<version> -- <bin_name>`, installing on first real invocation
- [ ] Subsequent invocations reuse the already-installed tool without re-triggering install
- [ ] Tests cover shim generation and the install-then-exec behavior on first invocation, using a stubbed `mise` on `PATH`
