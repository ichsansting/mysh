use crate::error::{Error, Result};
use std::io::{BufRead, Write as _};

/// Shows the pending-state summary, asks for explicit confirmation, and reads the
/// answer from `input` (injected so tests pipe "y\n"/"n\n"). Anything but an
/// explicit yes declines — the destructive path must be opt-in, never default.
/// This is the one place an op writes to stdout before returning: a prompt is
/// meaningless after the answer.
pub fn confirm(input: &mut dyn BufRead, summary: &str) -> Result<bool> {
    print!("{summary}proceed? [y/N] ");
    std::io::stdout().flush().map_err(|e| Error::Subprocess {
        program: "stdout",
        detail: e.to_string(),
    })?;
    let mut answer = String::new();
    input.read_line(&mut answer).map_err(|e| Error::Subprocess {
        program: "stdin",
        detail: e.to_string(),
    })?;
    println!();
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}
