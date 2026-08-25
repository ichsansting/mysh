use crate::config::Config;
use crate::domain::drift::{self, DriftSide};
use crate::domain::picker::Item;
use crate::domain::render::{self, RenderKind};
use crate::error::{Error, IoCtx, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{crypto, fsx, git, prompt, tty};
use crate::ops::diff;
use std::fs;

/// Save: capture live Target edits (and directory-mode's new-in-Target files)
/// back into Source, then commit and push whatever's selected — including
/// anything already sitting in Source unpushed (e.g. from `add`), even where
/// nothing on the Target side has drifted at all. Local wins. Refused for
/// derived Targets (Fragment/Overlay) — there is no unambiguous Source piece
/// to attribute a hand-edit to. `Missing` drift is never offered here — that's
/// a Reset candidate, not Save's job.
pub fn run(config: &Config, passphrase: &mut PassphraseFn) -> Result<String> {
    let drifts = diff::collect(config, passphrase)?;
    let actionable: Vec<_> = drifts
        .into_iter()
        .filter(|d| {
            matches!(
                d.side,
                DriftSide::Target | DriftSide::New | DriftSide::Remote
            )
        })
        .collect();
    if actionable.is_empty() {
        return Ok("nothing to save\n".to_string());
    }

    let plan = render::enumerate(&config.source_dir)?;
    for drift in actionable.iter().filter(|d| d.side == DriftSide::Target) {
        let unit = plan
            .units
            .iter()
            .find(|u| u.target_rel == drift.rel)
            .ok_or_else(|| {
                Error::Rejected(format!("{}: drifted but untracked", drift.rel.display()))
            })?;
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

    let items: Vec<Item> = actionable.iter().cloned().map(Item::from).collect();
    let selected: Vec<Item> = match tty::pick(items) {
        tty::PickResult::Picked(items) => items.into_iter().filter(|i| i.selected).collect(),
        tty::PickResult::Aborted => return Ok("aborted\n".to_string()),
        tty::PickResult::Unavailable => {
            // No real terminal (piped stdin — scripts, tests, CI): fall back to
            // the classic whole-list confirm, acting on everything actionable.
            // Locked here, not passed in from main — tty::pick already tried
            // and released its own stdin lock above; grabbing ours only now
            // (never held concurrently with the picker's) is what keeps this
            // from deadlocking against it.
            let stdin = std::io::stdin();
            if !prompt::confirm(&mut stdin.lock(), &drift::format(&actionable))? {
                return Ok("aborted\n".to_string());
            }
            actionable.iter().cloned().map(Item::from).collect()
        }
    };
    if selected.is_empty() {
        return Ok("nothing to save\n".to_string());
    }

    let mut git_paths = Vec::new();
    for item in &selected {
        match item.side {
            DriftSide::Target => {
                let unit = plan
                    .units
                    .iter()
                    .find(|u| u.target_rel == item.rel)
                    .expect("checked above");
                let live = config.target_dir.join(&unit.target_rel);
                // A Target-drift entry whose Target file doesn't exist at all
                // (e.g. `add`ed but never applied) has nothing to capture — that's
                // an Apply job, not an edit for Save to attribute anywhere.
                let Some(content) = fsx::read_opt(&live)? else {
                    continue;
                };
                let dest = config.source_dir.join(&unit.source_rel);
                match unit.kind {
                    RenderKind::Plain => {
                        fs::write(&dest, content).at("write", &dest)?;
                    }
                    // Captured edits go back encrypted — plaintext never lands in Source.
                    RenderKind::Secret => {
                        let envelope = crypto::encrypt(&content, &passphrase()?, &dest)?;
                        fs::write(&dest, envelope).at("write", &dest)?;
                    }
                    RenderKind::Fragment | RenderKind::Overlay => unreachable!("rejected above"),
                }
                git_paths.push(unit.source_rel.clone());
            }
            // Directory-mode: a file Target has that Source doesn't yet. Tracked
            // directories mirror 1:1, so the same relative path is valid under
            // both Target and Source roots.
            DriftSide::New => {
                let live = config.target_dir.join(&item.rel);
                let content = fs::read(&live).at("read", &live)?;
                fsx::write_if_changed(&config.source_dir.join(&item.rel), &content, None)?;
                git_paths.push(item.rel.clone());
            }
            // Already correct in Source (e.g. staged there by `add`) — nothing
            // to capture, it just needs pushing.
            DriftSide::Remote => git_paths.push(item.rel.clone()),
            DriftSide::Missing => unreachable!("not actionable, never offered"),
        }
    }

    git::commit_and_push(&config.source_dir, "mysh save", &git_paths)?;
    Ok("saved\n".to_string())
}
