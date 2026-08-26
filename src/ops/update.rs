use crate::config::Config;
use crate::domain::drift::{self, DriftSide};
use crate::error::{Error, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{git, prompt, release};
use crate::ops::{apply, diff};
use std::io::BufRead;

/// Update: discard all local drift — Source is forced to match Remote, then
/// every Target is re-rendered. Remote wins. No three-way merge, ever: a
/// Diverged path (both sides changed since they last agreed) is refused
/// outright rather than silently resolved in Remote's favor — `force_match_remote`
/// would otherwise discard local-only history a plain "Remote wins" never had
/// to reckon with, since ordinary Behind drift has no local-only content to lose.
/// Also refreshes the installed mysh binary itself (ADR-0013) — independent of
/// the Source-side outcome above, so it runs even when there's nothing to update
/// or a diverged path gets refused.
pub fn run(
    config: &Config,
    input: &mut dyn BufRead,
    passphrase: &mut PassphraseFn,
) -> Result<String> {
    release::refresh_binary(config);

    let drifts = diff::collect(config, passphrase, false)?;
    if drifts.is_empty() {
        return Ok("nothing to update\n".to_string());
    }

    let diverged: Vec<_> = drifts
        .iter()
        .filter(|d| d.side == DriftSide::Diverged)
        .cloned()
        .collect();
    if !diverged.is_empty() {
        return Err(Error::Rejected(format!(
            "diverged from remote — resolve with git directly, mysh does not merge:\n{}",
            drift::format(&diverged)
        )));
    }

    if !prompt::confirm(input, &drift::format(&drifts))? {
        return Ok("aborted\n".to_string());
    }

    git::force_match_remote(&config.source_dir)?;
    apply::run(config, passphrase)?;
    Ok("updated\n".to_string())
}
