use crate::config::Config;
use crate::domain::log::{AppLog, LogEntry, Ownership};
use crate::domain::render::{self, RenderKind, SourcePlan};
use crate::domain::BACKUP_DIR_REL;
use crate::error::{Error, IoCtx, Result};
use crate::infra::fsx;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Apply: render every unit of Source into Target through the uniform render
/// step (ADR-0002), with first-touch bookkeeping in the Application Log.
pub fn run(config: &Config) -> Result<String> {
    let plan = render::enumerate(&config.source_dir)?;
    let log = AppLog::at(&config.target_dir);
    render_plan(&plan, config, &log)?;
    Ok(String::new())
}

/// Renders a whole SourcePlan. Shared with reset (which re-applies after
/// forcing Source to match Remote).
pub fn render_plan(plan: &SourcePlan, config: &Config, log: &AppLog) -> Result<()> {
    let managed: HashSet<PathBuf> = log.managed_targets()?.into_iter().collect();
    for unit in &plan.units {
        let source = config.source_dir.join(&unit.source_rel);
        match unit.kind {
            RenderKind::Plain => {
                let content = fs::read(&source).at("read", &source)?;
                let mode = fsx::mode_of(&source)?;
                write_fully_owned(config, log, &managed, &unit.target_rel, &content, mode)?;
            }
            RenderKind::Secret | RenderKind::Fragment | RenderKind::Overlay => {
                return Err(Error::Rejected(format!(
                    "apply: {:?} rendering not implemented yet",
                    unit.kind
                )));
            }
        }
    }
    Ok(())
}

/// The first-touch contract for a fully-owned Target: back up pre-existing
/// content once (so teardown can restore it), log created-vs-overwritten once,
/// then write idempotently.
fn write_fully_owned(
    config: &Config,
    log: &AppLog,
    managed: &HashSet<PathBuf>,
    target_rel: &Path,
    content: &[u8],
    mode: Option<u32>,
) -> Result<()> {
    let dest = config.target_dir.join(target_rel);
    if !managed.contains(target_rel) {
        let backup_rel = if dest.exists() {
            let backup_rel = Path::new(BACKUP_DIR_REL).join(target_rel);
            let existing = fs::read(&dest).at("read", &dest)?;
            fsx::write_if_changed(&config.target_dir.join(&backup_rel), &existing, None)?;
            Some(backup_rel)
        } else {
            None
        };
        log.record(&LogEntry::Target {
            ownership: Ownership::Full,
            rel: target_rel.to_path_buf(),
            backup_rel,
        })?;
    }
    fsx::write_if_changed(&dest, content, mode)
}
