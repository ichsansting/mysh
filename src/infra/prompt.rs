use crate::error::{Error, Result};
use std::io::{BufRead, Write as _};

/// The lazily-invoked Passphrase seam: commands touching no Secret never call
/// it, so they never prompt. The answer lives only in process memory for the
/// current command — never on disk, in a keychain, or with an agent (ADR-0003).
pub type PassphraseFn<'a> = dyn FnMut() -> Result<String> + 'a;

/// Returns `configured` (`--passphrase`/`MYSH_PASSPHRASE`) when set; otherwise
/// prompts without echo on first use and reuses the answer for this process.
pub fn passphrase_provider(configured: Option<String>) -> impl FnMut() -> Result<String> {
    let mut cached = configured;
    move || {
        if let Some(passphrase) = &cached {
            return Ok(passphrase.clone());
        }
        let entered = rpassword::prompt_password("mysh passphrase: ")
            .map_err(|e| Error::Subprocess { program: "passphrase prompt", detail: e.to_string() })?;
        cached = Some(entered.clone());
        Ok(entered)
    }
}

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
