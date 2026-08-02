use std::io::{self, BufRead, Write};

/// Prints `prompt`, reads a line from `input`, and reports whether it was an
/// affirmative response (`y`/`yes`, case-insensitive). Shared by `save` and `reset`,
/// mutating commands that both require explicit confirmation before acting.
pub fn confirm(input: &mut dyn BufRead, prompt: &str) -> Result<bool, String> {
    print!("{prompt}");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    input.read_line(&mut line).map_err(|e| e.to_string())?;
    let answer = line.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}
