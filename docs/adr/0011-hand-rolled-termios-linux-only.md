# Hand-rolled termios FFI for the picker's raw mode, Linux-only

The interactive picker (`diff`/`save` path selection) needs raw single-keystroke
terminal input — nothing already in the dependency tree provides that, and the
project avoids adding new runtime dependencies (ADR-0001) except where the
alternative is materially worse.

Considered a small terminal crate (e.g. `crossterm`, `termios`) for correct,
tested cross-platform raw-mode handling. Chose instead to hand-roll the
`tcgetattr`/`tcsetattr` FFI directly against Linux's glibc/musl `termios`
struct layout only — matching `infra/prompt.rs`'s existing zero-dependency
style, and keeping the dependency floor exactly where ADR-0001 put it.

Consequence, accepted deliberately: macOS's `termios` layout differs (no
`c_line` field, wider `c_cc`) and isn't implemented — guessing it wrong risks
corrupting a real user's terminal state, not just failing a test — so macOS
(and every other non-Linux target) falls back to the exact non-interactive
behavior (plain list, classic y/N confirm) instead of the picker. This is a
live gap, not a hypothetical one: `release.sh` ships an
`aarch64-apple-darwin` binary, so macOS users of `diff`/`save` never see the
picker today. Revisit if that turns out to matter — either implement the
correct macOS struct layout, or take the dependency.
