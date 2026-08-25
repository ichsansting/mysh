use crate::domain::picker::Item;

/// Result of trying to run the interactive picker.
pub enum PickResult {
    /// The user confirmed — items carry their final selected/deselected state.
    Picked(Vec<Item>),
    /// The user aborted (`q`/`n`).
    Aborted,
    /// No real terminal to be interactive on (piped stdin — tests, CI, scripts).
    /// Callers fall back to the pre-picker behavior; this is not an error.
    Unavailable,
}

pub fn pick(items: Vec<Item>) -> PickResult {
    #[cfg(target_os = "linux")]
    {
        linux::pick(items)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Correct termios layout for a raw-mode picker isn't implemented for
        // this platform yet (see infra/tty.rs's linux module for why) — degrade
        // to the same fallback a non-tty run takes, rather than guess an ABI.
        let _ = items;
        PickResult::Unavailable
    }
}

/// Hand-rolled raw-terminal-mode via direct termios FFI (Linux glibc/musl struct
/// layout) — no new dependency, matching this module's existing zero-dep style
/// (see `infra/prompt.rs`). Deliberately not attempted for macOS: its termios
/// struct layout differs (no `c_line` field, wider `c_cc`) and guessing it wrong
/// risks corrupting a real user's terminal state rather than just failing a test.
#[cfg(target_os = "linux")]
mod linux {
    use super::{Item, PickResult};
    use crate::domain::picker::{self, Key, Outcome, State};
    use std::io::{self, Read, Write};

    const STDIN_FILENO: i32 = 0;
    const TCSANOW: i32 = 0;
    const ICANON: u32 = 0o000002;
    const ECHO: u32 = 0o000010;
    const VMIN: usize = 6;
    const VTIME: usize = 5;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }

    unsafe extern "C" {
        fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    }

    struct RawMode {
        original: Termios,
    }

    impl RawMode {
        fn enable() -> io::Result<Self> {
            let mut original = unsafe { std::mem::zeroed::<Termios>() };
            if unsafe { tcgetattr(STDIN_FILENO, &mut original) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let mut raw = original;
            raw.c_lflag &= !(ICANON | ECHO);
            raw.c_cc[VMIN] = 1;
            raw.c_cc[VTIME] = 0;
            if unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &raw) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(RawMode { original })
        }
    }

    impl Drop for RawMode {
        fn drop(&mut self) {
            unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &self.original) };
        }
    }

    pub fn pick(items: Vec<Item>) -> PickResult {
        let _raw = match RawMode::enable() {
            Ok(raw) => raw,
            Err(_) => return PickResult::Unavailable,
        };
        let mut state = State::new(items);
        loop {
            render(&state);
            if let Some(outcome) = state.outcome {
                return match outcome {
                    Outcome::Confirmed => PickResult::Picked(state.items),
                    Outcome::Aborted => PickResult::Aborted,
                };
            }
            let key = match read_key() {
                Ok(key) => key,
                Err(_) => return PickResult::Aborted,
            };
            state = picker::apply(state, key);
        }
    }

    fn render(state: &State) {
        print!("\x1b[2J\x1b[H");
        println!(
            "\x1b[1mSelect paths\x1b[0m  \x1b[2m(space toggle, enter confirm, q abort)\x1b[0m\n"
        );
        for (i, item) in state.items.iter().enumerate() {
            let mark = if item.selected { "x" } else { " " };
            let cursor = if i == state.cursor { ">" } else { " " };
            let line = format!("{cursor} [{mark}] {:<32}{}", item.rel.display(), item.side);
            if i == state.cursor {
                println!("\x1b[7m{line}\x1b[0m");
            } else {
                println!("{line}");
            }
        }
        if let Some(msg) = &state.message {
            println!("\n\x1b[2m{msg}\x1b[0m");
        }
        println!(
            "\n\x1b[1m\u{2191}\u{2193}\x1b[0m\x1b[2m move\x1b[0m  \x1b[1m[space]\x1b[0m\x1b[2m toggle\x1b[0m  \
             \x1b[1m[a]\x1b[0m\x1b[2m all\x1b[0m  \x1b[1m[0]\x1b[0m\x1b[2m none\x1b[0m  \
             \x1b[1m[q]\x1b[0m\x1b[2m abort\x1b[0m  \x1b[1m[enter]\x1b[0m\x1b[2m confirm\x1b[0m"
        );
        let _ = io::stdout().flush();
    }

    /// Blocks for the first byte (VMIN=1/VTIME=0 baseline); on ESC, briefly
    /// switches to a 100ms-timeout read to see if `[A`/`[B` follows (an arrow
    /// key) before restoring the blocking baseline.
    fn read_key() -> io::Result<Key> {
        let mut buf = [0u8; 1];
        io::stdin().lock().read_exact(&mut buf)?;
        match buf[0] {
            0x1b => {
                set_vmin_vtime(0, 1)?;
                let key = read_escape_sequence()?;
                set_vmin_vtime(1, 0)?;
                Ok(key)
            }
            b'\r' | b'\n' => Ok(Key::Enter),
            b => Ok(Key::Char(b as char)),
        }
    }

    fn read_escape_sequence() -> io::Result<Key> {
        let mut b1 = [0u8; 1];
        if io::stdin().lock().read(&mut b1)? == 0 || b1[0] != b'[' {
            return Ok(Key::Char('\x1b'));
        }
        let mut b2 = [0u8; 1];
        if io::stdin().lock().read(&mut b2)? == 0 {
            return Ok(Key::Char('\x1b'));
        }
        Ok(match b2[0] {
            b'A' => Key::Up,
            b'B' => Key::Down,
            _ => Key::Char('\x1b'),
        })
    }

    fn set_vmin_vtime(vmin: u8, vtime: u8) -> io::Result<()> {
        let mut t = unsafe { std::mem::zeroed::<Termios>() };
        if unsafe { tcgetattr(STDIN_FILENO, &mut t) } != 0 {
            return Err(io::Error::last_os_error());
        }
        t.c_cc[VMIN] = vmin;
        t.c_cc[VTIME] = vtime;
        if unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &t) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
