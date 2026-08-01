# 11 — Teardown

**What to build:** The `teardown` command — full reversal of everything `mysh` has done to a device, by replaying the `Application Log`: files, packages, `mise` itself, PATH/rc lines, and the bootstrap installer's own footprint.

**Blocked by:** 02 — Application Log foundation + pre-existing-file backup, 09 — Packages: lazy install via shims

**Status:** ready-for-agent

- [ ] `teardown` deletes every file `mysh` created and restores the backed-up original for every file it overwrote
- [ ] `teardown` uninstalls every package it installed via `mise`, and removes `mise` itself if `mysh` installed it
- [ ] `teardown` strips every `PATH`/rc-file line `mysh` added, including the bootstrap installer's own `PATH` addition
- [ ] `teardown` removes the `mysh` binary and installer footprint last
- [ ] After `teardown`, every managed path matches its pre-bootstrap state exactly — no residue
- [ ] A test drives a full bootstrap-to-teardown cycle against injected temp directories and asserts no residue remains
