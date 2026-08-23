use crate::config::Config;
use crate::domain::log::{AppLog, LogEntry, Ownership};
use crate::domain::render::{self, RenderKind, SourcePlan};
use crate::domain::{overlay, package, BACKUP_DIR_REL};
use crate::error::{IoCtx, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{crypto, fsx};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Apply: render every unit of Source into Target through the uniform render
/// step (ADR-0002), with first-touch bookkeeping in the Application Log.
pub fn run(config: &Config, passphrase: &mut PassphraseFn) -> Result<String> {
    let plan = render::enumerate(&config.source_dir)?;
    let log = AppLog::at(&config.target_dir);
    render_plan(&plan, config, &log, passphrase)?;
    prewarm_packages(config, &log)?;
    Ok(String::new())
}

/// The package pass: a no-op (mise never touched) when Source declares no
/// shim at all; otherwise mise is ensured — a lazy-only device still gets it,
/// so shims have something to invoke on first use — and every eager-marked
/// shim's specifier is prewarmed in one batched install (ADR-0007).
fn prewarm_packages(config: &Config, log: &AppLog) -> Result<()> {
    let bin_dir = config.source_dir.join(crate::domain::BIN_DIR_REL);
    let shims = fsx::walk(&bin_dir, &|_| true)?;
    if shims.is_empty() {
        return Ok(());
    }
    let mise_bin = crate::infra::mise::ensure_installed(config, log)?;
    let mut specifiers = std::collections::BTreeSet::new();
    for rel in &shims {
        let path = bin_dir.join(rel);
        // Non-UTF-8 or hand-edited content that isn't an add-written shim
        // simply isn't prewarmable — it stays lazy, never an error (ADR-0007).
        let Ok(content) = fs::read_to_string(&path) else { continue };
        if package::is_eager(&content) {
            if let Some(specifier) = package::shim_specifier(&content) {
                specifiers.insert(specifier.to_string());
            }
        }
    }
    if !specifiers.is_empty() {
        let specifiers: Vec<String> = specifiers.into_iter().collect();
        crate::infra::mise::install(&mise_bin, &specifiers, config)?;
    }
    Ok(())
}

/// Renders a whole SourcePlan. Shared with reset (which re-applies after
/// forcing Source to match Remote).
pub fn render_plan(
    plan: &SourcePlan,
    config: &Config,
    log: &AppLog,
    passphrase: &mut PassphraseFn,
) -> Result<()> {
    let managed: HashSet<PathBuf> = log.managed_targets()?.into_iter().collect();
    for unit in &plan.units {
        let source = config.source_dir.join(&unit.source_rel);
        match unit.kind {
            RenderKind::Plain => {
                let content = fs::read(&source).at("read", &source)?;
                let mode = fsx::mode_of(&source)?;
                write_fully_owned(config, log, &managed, &unit.target_rel, &content, mode)?;
            }
            RenderKind::Secret => {
                let envelope = fs::read(&source).at("read", &source)?;
                let plaintext = crypto::decrypt(&envelope, &passphrase()?, &source)?;
                // 0600 always: a decrypted credential is never left group/world-readable.
                write_fully_owned(config, log, &managed, &unit.target_rel, &plaintext, Some(0o600))?;
            }
            RenderKind::Fragment => {
                let content = crate::domain::fragment::compose(&source, passphrase)?;
                write_fully_owned(config, log, &managed, &unit.target_rel, &content, None)?;
            }
            // Overlay: enforce only the declared keys; never own, back up, or
            // restore the rest of the file (ADR-0008 — Partial ownership).
            RenderKind::Overlay => {
                let declared = overlay::read_declared(&source)?;
                let dest = config.target_dir.join(&unit.target_rel);
                let live = fsx::read_opt(&dest)?;
                if overlay::keys_match(live.as_deref(), &declared) {
                    continue; // matching keys touch nothing and log nothing
                }
                let merged = overlay::merge(live.as_deref(), &declared, &dest)?;
                if !managed.contains(&unit.target_rel) {
                    log.record(&LogEntry::Target {
                        ownership: Ownership::Partial,
                        rel: unit.target_rel.clone(),
                        backup_rel: None,
                    })?;
                }
                fsx::write_if_changed(&dest, &merged, None)?;
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
