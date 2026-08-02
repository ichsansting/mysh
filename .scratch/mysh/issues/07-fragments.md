# 07 — Fragments

**What to build:** Multi-fragment composition. A `<name>.d/` directory in `Source` renders its contents — plain and/or `Secret` fragments — concatenated in lexical filename order into a single `Target` file, with new fragments picked up automatically. Composed targets are derived-only: `diff` shows drift, `reset` discards it, but `save` is not available.

**Blocked by:** 06 — Secrets

**Status:** done

- [x] A directory named `<name>.d/` in `Source` renders its contents, concatenated in lexical filename order, into a single `Target` file named `<name>`
- [x] A fragment ending in `.age` within a `.d/` directory is decrypted before concatenation
- [x] A new fragment file dropped into a `.d/` directory is picked up by the next `apply` with no registration step
- [x] `diff` on a composed `Target` shows drift between the live merged file and a fresh re-render from `Source`
- [x] `save` is rejected for composed targets, with a message directing the user to edit the relevant fragment directly
- [x] `reset` on a composed target discards drift by re-rendering fresh from fragments
- [x] Tests cover multi-fragment composition (plain + secret fragments together), automatic pickup of a newly added fragment, the `save` rejection, and `reset`

## Comments

Implemented in `src/fragment.rs`: `is_fragment_dir`/`target_name` for the `<name>.d` naming convention; `is_fragment_member` to keep individual fragment files out of the ordinary per-path diff/apply handling; `find_fragment_dirs` (recursive, skips `.git`/`.mysh`, doesn't descend into a `.d` dir itself — no nested composition); `render` concatenates each direct child file in lexical filename order, decrypting any `.age`-suffixed fragment via `secret::decrypt`.

`apply::render` gained a second pass over `find_fragment_dirs`, sharing first-touch/backup/Application-Log bookkeeping with the plain/secret pass via an extracted `apply_one` helper (both passes now just supply a `write` closure). `walk_files` skips descending into `.d` dirs so fragment members never show up as their own plain-file entries. `diff::diff` excludes fragment members and each fragment's merged target name from the ordinary per-path loop, then reports one `FileDrift` per composed target (`is_fragment: true`) comparing live `Target` content against a fresh render — remote drift isn't tracked at fragment granularity (not required by this issue). `save::save` refuses outright (before prompting) if any drifted path is fragment-composed, naming the `.d/` directory to edit instead. `reset::reset` needed no changes — it already composes generically through `diff` + `apply::apply`, and `apply` re-rendering fragments fresh is exactly "discard drift."

Tests: unit tests in `src/fragment.rs` (naming convention, member detection) plus `tests/fragment_integration.rs` driving the real CLI — multi-fragment concatenation mixing plain and secret fragments, automatic pickup of a newly added fragment, `diff` isolating live-vs-fresh-render drift, `save`'s refusal (asserting failure exit code and a message naming the fragment dir, and that nothing was written to Source), and `reset` re-rendering fresh. `cargo test` (40 tests) and `cargo clippy --all-targets` pass clean. Reviewed via `/code-review` (Standards + Spec axes in parallel): Spec axis found zero defects — every checklist item implemented and tested, no scope creep, double-report risk with `.track` directory-mode scanning traced and confirmed not to occur. Standards axis found no hard violations (still no documented standards in-repo) and one worthwhile judgement call — duplicated first-touch/backup/record bookkeeping between the plain/secret and fragment render passes in `apply.rs` — fixed by extracting `apply_one`.
