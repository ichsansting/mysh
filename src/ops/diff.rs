use crate::config::Config;
use crate::domain::drift::{self, Drift, DriftSide};
use crate::domain::render::{self, RenderKind};
use crate::error::{Error, IoCtx, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{crypto, git};
use std::fs;
use std::path::Path;

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
            RenderKind::Overlay => {
                return Err(Error::Rejected(
                    "diff: Overlay rendering not implemented yet".to_string(),
                ));
            }
        };
        if live_content(&config.target_dir.join(&unit.target_rel))?.as_deref() != Some(&expected[..])
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

fn live_content(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).at("read", path),
    }
}
