# 07 — Fragments

**What to build:** Multi-fragment composition. A `<name>.d/` directory in `Source` renders its contents — plain and/or `Secret` fragments — concatenated in lexical filename order into a single `Target` file, with new fragments picked up automatically. Composed targets are derived-only: `diff` shows drift, `reset` discards it, but `save` is not available.

**Blocked by:** 06 — Secrets

**Status:** ready-for-agent

- [ ] A directory named `<name>.d/` in `Source` renders its contents, concatenated in lexical filename order, into a single `Target` file named `<name>`
- [ ] A fragment ending in `.age` within a `.d/` directory is decrypted before concatenation
- [ ] A new fragment file dropped into a `.d/` directory is picked up by the next `apply` with no registration step
- [ ] `diff` on a composed `Target` shows drift between the live merged file and a fresh re-render from `Source`
- [ ] `save` is rejected for composed targets, with a message directing the user to edit the relevant fragment directly
- [ ] `reset` on a composed target discards drift by re-rendering fresh from fragments
- [ ] Tests cover multi-fragment composition (plain + secret fragments together), automatic pickup of a newly added fragment, the `save` rejection, and `reset`
