use crate::config::Config;
use crate::domain::drift::{self, Drift, DriftSide};
use crate::domain::render::{self, RenderKind};
use crate::domain::{glob, overlay};
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

    // Directory-mode: scan each .track-marked directory's live side for files
    // Source doesn't know (new) and Source files gone live (missing).
    for tracked in &plan.tracked_dirs {
        let source_side: Vec<_> = plan
            .units
            .iter()
            .filter_map(|u| u.target_rel.strip_prefix(&tracked.rel).ok())
            .collect();
        let live_root = config.target_dir.join(&tracked.rel);
        let live_side = fsx::walk(&live_root, &|_| true)?;
        for rel in &live_side {
            if !source_side.iter().any(|s| *s == rel)
                && !glob::is_ignored(rel, &tracked.ignore)
            {
                drifts.push(Drift { rel: tracked.rel.join(rel), side: DriftSide::New });
            }
        }
        for rel in source_side {
            if !live_side.iter().any(|l| l == rel) {
                drifts.push(Drift { rel: tracked.rel.join(rel), side: DriftSide::Missing });
            }
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
