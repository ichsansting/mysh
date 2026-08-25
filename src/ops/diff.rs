use crate::config::Config;
use crate::domain::drift::{self, Drift, DriftSide};
use crate::domain::fingerprint::{self, Fingerprints};
use crate::domain::picker::Item;
use crate::domain::render::{self, RenderKind, SourcePlan};
use crate::domain::{fragment, glob, overlay};
use crate::error::{Error, IoCtx, Result};
use crate::infra::prompt::PassphraseFn;
use crate::infra::{crypto, fsx, git, tty};
use std::fs;
use std::path::Path;

/// Diff: report drift across the three-state model — live Target vs a fresh
/// in-memory render of Source, and Source vs Remote — without touching anything.
/// On a real terminal, an interactive picker follows the list, letting you pick
/// specific paths to see the actual content difference behind their drift
/// instead of just that it exists. Piped stdin (scripts, tests, CI) has no
/// picker to offer, so it falls back to the plain list, unchanged.
///
/// `quick` (`diff --quick`) trades accuracy for speed: no `git fetch` (Remote
/// drift is judged against `origin/main` as last fetched), and no decrypting
/// Secrets or composing Fragments (their Target drift is judged against the
/// Fingerprint cache instead of freshly-rendered content). Cheap and safe
/// enough to run on every shell prompt render — see ADR-0012. Save and Reset
/// always call `collect` with `quick: false`; only the read-only `diff`
/// command exposes the flag.
pub fn run(config: &Config, passphrase: &mut PassphraseFn, quick: bool) -> Result<String> {
    let drifts = collect(config, passphrase, quick)?;
    if drifts.is_empty() {
        return Ok(drift::format(&drifts));
    }

    let items: Vec<Item> = drifts.iter().cloned().map(Item::from).collect();
    let selected = match tty::pick(items) {
        tty::PickResult::Picked(items) => {
            items.into_iter().filter(|i| i.selected).collect::<Vec<_>>()
        }
        tty::PickResult::Aborted | tty::PickResult::Unavailable => Vec::new(),
    };
    if selected.is_empty() {
        return Ok(drift::format(&drifts));
    }

    let plan = render::enumerate(&config.source_dir)?;
    let mut out = String::new();
    for item in &selected {
        out.push_str(&format!("--- {} ({}) ---\n", item.rel.display(), item.side));
        out.push_str(&content_diff(config, &plan, item, passphrase)?);
    }
    Ok(out)
}

/// The real content behind one selected path's drift. `Remote`/`Target` have
/// two contents to compare; `New`/`Missing` are pure presence/absence — there's
/// nothing to diff, so they fall back to the same list line `diff` always shows.
fn content_diff(
    config: &Config,
    plan: &SourcePlan,
    item: &Item,
    passphrase: &mut PassphraseFn,
) -> Result<String> {
    match item.side {
        DriftSide::Ahead | DriftSide::Behind | DriftSide::Diverged => {
            git::diff_source_vs_remote(&config.source_dir, &item.rel)
        }
        DriftSide::Target => target_content_diff(config, plan, &item.rel, passphrase),
        DriftSide::New | DriftSide::Missing => {
            Ok(format!("{}\t{}\n", item.rel.display(), item.side))
        }
    }
}

fn target_content_diff(
    config: &Config,
    plan: &SourcePlan,
    rel: &Path,
    passphrase: &mut PassphraseFn,
) -> Result<String> {
    let unit = plan
        .units
        .iter()
        .find(|u| u.target_rel == rel)
        .ok_or_else(|| Error::Rejected(format!("{}: drifted but untracked", rel.display())))?;
    let target_path = config.target_dir.join(&unit.target_rel);
    let source_path = config.source_dir.join(&unit.source_rel);
    match unit.kind {
        RenderKind::Plain => git::diff_no_index(&source_path, &target_path),
        // Decrypted plaintext never touches disk except in this short-lived,
        // 0600 temp file — deleted immediately after the diff runs (see
        // ADR-0010; Target already holds this same plaintext at 0600 anyway).
        RenderKind::Secret => {
            let envelope = fs::read(&source_path).at("read", &source_path)?;
            let plaintext = crypto::decrypt(&envelope, &passphrase()?, &source_path)?;
            with_temp_file(&plaintext, Some(0o600), |tmp| {
                git::diff_no_index(tmp, &target_path)
            })
        }
        RenderKind::Fragment => {
            let composed = fragment::compose(&source_path, passphrase)?;
            with_temp_file(&composed, None, |tmp| git::diff_no_index(tmp, &target_path))
        }
        // Overlay drift is key-level (ADR-0008: partial ownership) — a whole-file
        // diff would surface other programs' keys, which aren't mysh's business.
        RenderKind::Overlay => overlay_summary(&source_path, &target_path),
    }
}

fn overlay_summary(source_path: &Path, target_path: &Path) -> Result<String> {
    let declared = overlay::read_declared(source_path)?;
    // Same object-parsing overlay::keys_match/merge already use — a genuinely
    // malformed live Target is a real error here too, not silently "no keys".
    let live_obj = match fsx::read_opt(target_path)? {
        Some(bytes) => overlay::as_object(&bytes).map_err(|detail| Error::Overlay {
            path: target_path.to_path_buf(),
            detail,
        })?,
        None => serde_json::Map::new(),
    };
    let mut out = String::new();
    for (key, want) in &declared {
        let have = live_obj.get(key);
        if have != Some(want) {
            let have_text = have.map_or_else(|| "<missing>".to_string(), |v| v.to_string());
            out.push_str(&format!("{key}: {have_text} -> {want}\n"));
        }
    }
    Ok(out)
}

/// Writes `content` to a private temp file for `f` to diff against, removed
/// immediately after — the only way to hand `git diff` two things to compare
/// when one side (a decrypt, a fragment compose) only ever existed in memory.
fn with_temp_file<T>(
    content: &[u8],
    mode: Option<u32>,
    f: impl FnOnce(&Path) -> Result<T>,
) -> Result<T> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mysh-diff-{}-{n}", std::process::id()));
    fsx::write_if_changed(&path, content, mode)?;
    let result = f(&path);
    let _ = fs::remove_file(&path);
    result
}

/// The shared drift collection save/reset confirm against. `quick` skips
/// decryption/composition for Secret/Fragment units (compared against their
/// cached Fingerprint instead) and skips `git fetch` for Remote drift — see
/// `run`'s doc comment and ADR-0012.
pub fn collect(config: &Config, passphrase: &mut PassphraseFn, quick: bool) -> Result<Vec<Drift>> {
    let plan = render::enumerate(&config.source_dir)?;
    let mut drifts = Vec::new();
    // Only ever consulted on the quick path (below) — full diff decrypts/composes
    // directly and never looks at this, so there's no reason to read it there.
    let fingerprints = quick
        .then(|| Fingerprints::at(&config.target_dir))
        .transpose()?;

    for unit in &plan.units {
        let source = config.source_dir.join(&unit.source_rel);
        let target = config.target_dir.join(&unit.target_rel);
        match unit.kind {
            RenderKind::Overlay => {
                // Overlay drift is key-level, not whole-content: only a declared
                // key disagreeing (or the file missing) counts — other keys are
                // other programs' business. Never needs decryption either way.
                let declared = overlay::read_declared(&source)?;
                let live = fsx::read_opt(&target)?;
                if !overlay::keys_match(live.as_deref(), &declared) {
                    drifts.push(Drift {
                        rel: unit.target_rel.clone(),
                        side: DriftSide::Target,
                    });
                }
                continue;
            }
            RenderKind::Secret | RenderKind::Fragment if quick => {
                // No fingerprint recorded yet (e.g. before this unit's first
                // Apply/Save on this device) is unknown, not drifted — nothing
                // to compare against, so it's silently skipped rather than
                // guessed at. A full `diff` (or `apply`) settles it for good.
                let known_quick = fingerprints.as_ref().expect("Some when quick");
                let Some(expected_hash) = known_quick.get(&unit.target_rel) else {
                    continue;
                };
                let live = fsx::read_opt(&target)?;
                if live.as_deref().map(fingerprint::hash_of) != Some(expected_hash) {
                    drifts.push(Drift {
                        rel: unit.target_rel.clone(),
                        side: DriftSide::Target,
                    });
                }
                continue;
            }
            _ => {}
        }
        let expected = match unit.kind {
            RenderKind::Plain => fs::read(&source).at("read", &source)?,
            // Always plaintext-to-plaintext: a fresh decrypt of Source against
            // the live Target, never ciphertext against plaintext.
            RenderKind::Secret => {
                let envelope = fs::read(&source).at("read", &source)?;
                crypto::decrypt(&envelope, &passphrase()?, &source)?
            }
            RenderKind::Fragment => crate::domain::fragment::compose(&source, passphrase)?,
            RenderKind::Overlay => unreachable!("handled and `continue`d above"),
        };
        if fsx::read_opt(&target)?.as_deref() != Some(&expected[..]) {
            drifts.push(Drift {
                rel: unit.target_rel.clone(),
                side: DriftSide::Target,
            });
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
            if !source_side.iter().any(|s| *s == rel) && !glob::is_ignored(rel, &tracked.ignore) {
                drifts.push(Drift {
                    rel: tracked.rel.join(rel),
                    side: DriftSide::New,
                });
            }
        }
        for rel in source_side {
            if !live_side.iter().any(|l| l == rel) {
                drifts.push(Drift {
                    rel: tracked.rel.join(rel),
                    side: DriftSide::Missing,
                });
            }
        }
    }

    // Remote drift only exists where Source actually has git history to compare.
    if git::is_repo(&config.source_dir) {
        for (rel, side) in git::paths_differing_from_remote(&config.source_dir, !quick)? {
            drifts.push(Drift { rel, side });
        }
    }

    Ok(drifts)
}
