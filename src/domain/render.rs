use crate::error::{IoCtx, Result};
use crate::infra::fsx;
use std::fs;
use std::path::{Path, PathBuf};

/// How one Source entry renders into its Target (CONTEXT.md's taxonomy).
/// Directory-mode tracking is deliberately *not* a kind — a `.track` marker changes
/// what diff/add scan, not how any file renders — but it is classified by the same
/// `enumerate` so the whole taxonomy has exactly one decision point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderKind {
    /// Identity copy.
    Plain,
    /// `.age` file: decrypted during Apply/Diff, suffix stripped, written 0600.
    Secret,
    /// `.frag/` directory: members concatenate (in lexical order) into one Target.
    Fragment,
    /// `.overlay` file: declared keys shallow-merged onto a Target mysh doesn't own.
    Overlay,
}

impl RenderKind {
    /// Derived-only Targets (composed or merged) can never be saved back into
    /// Source — there is no unambiguous piece to attribute a hand-edit to.
    pub fn is_derived(self) -> bool {
        matches!(self, RenderKind::Fragment | RenderKind::Overlay)
    }
}

/// One renderable unit of Source: a file (or, for Fragment, a whole `.frag` dir).
#[derive(Debug)]
pub struct RenderUnit {
    /// Source-relative path of the file or `.frag` directory.
    pub source_rel: PathBuf,
    /// Target-relative path it renders to (kind suffix stripped).
    pub target_rel: PathBuf,
    pub kind: RenderKind,
}

/// A directory opted into Directory-mode tracking by a `.track` marker, whose
/// content is the ignore list (one glob per line, empty = track everything).
#[derive(Debug)]
pub struct TrackedDir {
    /// Source-relative (= target-relative) directory path.
    pub rel: PathBuf,
    pub ignore: Vec<String>,
}

/// Everything `enumerate` found in Source, classified once for every consumer:
/// apply renders `units`, diff walks `units` + `tracked_dirs`, save filters
/// derived units, add consults both for already-tracked checks.
#[derive(Debug)]
pub struct SourcePlan {
    pub units: Vec<RenderUnit>,
    pub tracked_dirs: Vec<TrackedDir>,
}

const SECRET_SUFFIX: &str = "age";
const FRAGMENT_SUFFIX: &str = "frag";
const OVERLAY_SUFFIX: &str = "overlay";
/// The Directory-mode marker file name.
pub const TRACK_MARKER: &str = ".track";

pub fn is_secret(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == SECRET_SUFFIX)
}

pub fn is_fragment_dir(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == FRAGMENT_SUFFIX)
}

pub fn is_overlay(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == OVERLAY_SUFFIX)
}

/// The single walk-and-classify over Source. `.frag` directories surface as one
/// Fragment unit each (never descended); `.track` markers register their directory
/// instead of rendering; everything else is Secret/Overlay/Plain by suffix.
pub fn enumerate(source_dir: &Path) -> Result<SourcePlan> {
    let entries = fsx::walk(source_dir, &|rel| !is_fragment_dir(rel))?;
    let mut units = Vec::new();
    let mut tracked_dirs = Vec::new();
    for rel in entries {
        if is_fragment_dir(&rel) {
            units.push(RenderUnit {
                target_rel: rel.with_extension(""),
                source_rel: rel,
                kind: RenderKind::Fragment,
            });
        } else if rel.file_name().is_some_and(|n| n == TRACK_MARKER) {
            let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            let content =
                fs::read_to_string(source_dir.join(&rel)).at("read", &source_dir.join(&rel))?;
            let ignore = content
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            tracked_dirs.push(TrackedDir { rel: dir, ignore });
        } else if is_secret(&rel) {
            units.push(RenderUnit {
                target_rel: rel.with_extension(""),
                source_rel: rel,
                kind: RenderKind::Secret,
            });
        } else if is_overlay(&rel) {
            units.push(RenderUnit {
                target_rel: rel.with_extension(""),
                source_rel: rel,
                kind: RenderKind::Overlay,
            });
        } else {
            units.push(RenderUnit {
                target_rel: rel.clone(),
                source_rel: rel,
                kind: RenderKind::Plain,
            });
        }
    }
    Ok(SourcePlan {
        units,
        tracked_dirs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn plan_of(layout: &[(&str, &str)]) -> SourcePlan {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mysh-render-{}-{n}", std::process::id()));
        for (rel, content) in layout {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
        }
        let plan = enumerate(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        plan
    }

    fn kinds(plan: &SourcePlan) -> Vec<(String, String, RenderKind)> {
        plan.units
            .iter()
            .map(|u| {
                (
                    u.source_rel.to_string_lossy().into_owned(),
                    u.target_rel.to_string_lossy().into_owned(),
                    u.kind,
                )
            })
            .collect()
    }

    #[test]
    fn classifies_all_five_taxonomy_members_in_one_pass() {
        let plan = plan_of(&[
            (".bashrc", "plain"),
            (".netrc.age", "sealed"),
            (".gitconfig.frag/10-base", "[user]"),
            (".claude.json.overlay", "{}"),
            (".config/fish/.track", "*.log\n"),
            (".config/fish/config.fish", "fish"),
        ]);
        assert_eq!(
            kinds(&plan),
            vec![
                (".bashrc".into(), ".bashrc".into(), RenderKind::Plain),
                (
                    ".claude.json.overlay".into(),
                    ".claude.json".into(),
                    RenderKind::Overlay
                ),
                (
                    ".config/fish/config.fish".into(),
                    ".config/fish/config.fish".into(),
                    RenderKind::Plain
                ),
                (
                    ".gitconfig.frag".into(),
                    ".gitconfig".into(),
                    RenderKind::Fragment
                ),
                (".netrc.age".into(), ".netrc".into(), RenderKind::Secret),
            ]
        );
        assert_eq!(plan.tracked_dirs.len(), 1);
        assert_eq!(plan.tracked_dirs[0].rel, Path::new(".config/fish"));
        assert_eq!(plan.tracked_dirs[0].ignore, vec!["*.log"]);
    }

    #[test]
    fn a_native_multi_file_dir_is_not_mistaken_for_a_fragment() {
        // The suffix is .frag, not .d, precisely so fish's conf.d stays plain.
        let plan = plan_of(&[(".config/fish/conf.d/a.fish", "x")]);
        assert_eq!(plan.units[0].kind, RenderKind::Plain);
    }

    #[test]
    fn fragment_members_are_not_units_of_their_own() {
        let plan = plan_of(&[
            (".gitconfig.frag/10-base", "a"),
            (".gitconfig.frag/20-token.age", "b"),
        ]);
        assert_eq!(plan.units.len(), 1);
        assert_eq!(plan.units[0].kind, RenderKind::Fragment);
    }

    #[test]
    fn derived_kinds_are_exactly_fragment_and_overlay() {
        assert!(RenderKind::Fragment.is_derived());
        assert!(RenderKind::Overlay.is_derived());
        assert!(!RenderKind::Plain.is_derived());
        assert!(!RenderKind::Secret.is_derived());
    }
}
