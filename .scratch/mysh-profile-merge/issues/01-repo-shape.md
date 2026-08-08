# 01 — Where does the profile live inside the mysh repo?

**Type:** grilling

**Status:** resolved

## Question

The mysh repo currently expects two separate repos: this one (the Rust tool) and a
Source repo (the user's dotfiles) that `bootstrap.sh` clones at apply time. Folding
the profile in requires picking a new shape. Two options considered: a `profile/`
subdirectory (source-dir points at the subdir; Save/Reset/directory-tracking scoped
to it; smallest change to `bootstrap.sh`/`release.sh`), or a full merge with dotfiles
at the repo root alongside `Cargo.toml`/`src/` (source-dir becomes the repo root
itself; Save/Reset/directory-tracking now operate over the same tree as the Rust
source).

## Answer

Subdirectory: `profile/` holds the dotfiles content (`.config/`, `.gitconfig.d/`,
`.packages`, `.claude/`, etc.). `--source-dir` resolves to `<clone>/profile`. Tool
code and profile content stay physically separate in the tree even though they share
one repo and one history.

## Comments
