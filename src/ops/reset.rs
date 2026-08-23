use crate::config::Config;
use crate::domain::drift;
use crate::error::Result;
use crate::infra::prompt::PassphraseFn;
use crate::infra::{git, prompt};
use crate::ops::{apply, diff};
use std::io::BufRead;

/// Reset: discard all local drift — Source is forced to match Remote, then
/// every Target is re-rendered. Remote wins. No three-way merge, ever.
pub fn run(config: &Config, input: &mut dyn BufRead, passphrase: &mut PassphraseFn) -> Result<String> {
    let drifts = diff::collect(config, passphrase)?;
    if drifts.is_empty() {
        return Ok("nothing to reset\n".to_string());
    }

    if !prompt::confirm(input, &drift::format(&drifts))? {
        return Ok("aborted\n".to_string());
    }

    git::force_match_remote(&config.source_dir)?;
    apply::run(config, passphrase)?;
    Ok("reset\n".to_string())
}
