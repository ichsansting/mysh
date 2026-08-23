use crate::config::Config;
use crate::domain::drift::{self, Drift, DriftSide};
use crate::domain::render::{self, RenderKind};
use crate::domain::overlay;
use crate::error::{IoCtx, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{crypto, fsx, git};
use std::fs;

/// Diff: report drift across the three-state model — live Target vs a fresh
/// in-memory render of Source, and Source vs Remote — without touching anything.
pub fn run(config: &Config, passphrase: &mut PassphraseFn) -> Result<String> {
    Ok(drift::format(&collect(config, passphrase)?))
}

/// The shared drift collection save/reset confirm against.
pub fn collect(config: &Config, passphrase: &mut PassphraseFn) -> Result<Vec<Drift>> {
    let plan = render::enumerate(&config.source_dir)?;
    let mut drifts = Vec::new();

    for unit in &plan.units {
        let source = config.source_dir.join(&unit.source_rel);
        let expected = match unit.kind {
            RenderKind::Plain => fs::read(&source).at("read", &source)?,
            // Always plaintext-to-plaintext: a fresh decrypt of Source against
            // the live Target, never ciphertext against plaintext.
            RenderKind::Secret => {
                let envelope = fs::read(&source).at("read", &source)?;
                crypto::decrypt(&envelope, &passphrase()?, &source)?
            }
            RenderKind::Fragment => crate::domain::fragment::compose(&source, passphrase)?,
            // Overlay drift is key-level, not whole-content: only a declared
            // key disagreeing (or the file missing) counts — other keys are
            // other programs' business.
            RenderKind::Overlay => {
                let declared = overlay::read_declared(&source)?;
                let live = fsx::read_opt(&config.target_dir.join(&unit.target_rel))?;
                if !overlay::keys_match(live.as_deref(), &declared) {
                    drifts.push(Drift { rel: unit.target_rel.clone(), side: DriftSide::Target });
                }
                continue;
            }
        };
        if fsx::read_opt(&config.target_dir.join(&unit.target_rel))?.as_deref()
            != Some(&expected[..])
        {
            drifts.push(Drift { rel: unit.target_rel.clone(), side: DriftSide::Target });
        }
    }

    // Remote drift only exists where Source actually has git history to compare.
    if git::is_repo(&config.source_dir) {
        for rel in git::paths_differing_from_remote(&config.source_dir)? {
            drifts.push(Drift { rel, side: DriftSide::Remote });
        }
    }

    Ok(drifts)
}
