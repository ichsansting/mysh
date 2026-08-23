use crate::config::Config;
use crate::domain::drift::{self, DriftSide};
use crate::domain::render::{self, RenderKind};
use crate::error::{Error, IoCtx, Result};
use crate::infra::{git, prompt};
use crate::ops::diff;
use std::fs;
use std::io::BufRead;

/// Save: capture live Target edits back into Source, commit, push. Local wins.
/// Refused for derived Targets (Fragment/Overlay) — there is no unambiguous
/// Source piece to attribute the edit to.
pub fn run(config: &Config, input: &mut dyn BufRead) -> Result<String> {
    let drifts = diff::collect(config)?;
    let target_drifts: Vec<_> =
        drifts.into_iter().filter(|d| d.side == DriftSide::Target).collect();
    if target_drifts.is_empty() {
        return Ok("nothing to save\n".to_string());
    }

    let plan = render::enumerate(&config.source_dir)?;
    for drift in &target_drifts {
        let unit = plan
            .units
            .iter()
            .find(|u| u.target_rel == drift.rel)
            .ok_or_else(|| Error::Rejected(format!("{}: drifted but untracked", drift.rel.display())))?;
        if unit.kind.is_derived() {
            let kind = match unit.kind {
                RenderKind::Fragment => "composed from fragments",
                _ => "enforced by an overlay",
            };
            return Err(Error::Rejected(format!(
                "{}: cannot save a target {kind}; use reset to discard the drift",
                drift.rel.display()
            )));
        }
    }

    if !prompt::confirm(input, &drift::format(&target_drifts))? {
        return Ok("aborted\n".to_string());
    }

    for drift in &target_drifts {
        let unit = plan.units.iter().find(|u| u.target_rel == drift.rel).expect("checked above");
        let live = config.target_dir.join(&unit.target_rel);
        let content = fs::read(&live).at("read", &live)?;
        let dest = config.source_dir.join(&unit.source_rel);
        match unit.kind {
            RenderKind::Plain => {
                fs::write(&dest, content).at("write", &dest)?;
            }
            RenderKind::Secret => {
                return Err(Error::Rejected(
                    "save: Secret capture not implemented yet".to_string(),
                ));
            }
            RenderKind::Fragment | RenderKind::Overlay => unreachable!("rejected above"),
        }
    }

    git::commit_and_push(&config.source_dir, "mysh save")?;
    Ok("saved\n".to_string())
}
