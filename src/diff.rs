use crate::apply::walk_files;
use crate::git;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A plain file's drift, relative to `Source`, across the other two states.
pub struct FileDrift {
    pub path: PathBuf,
    /// `Target` (live disk) differs from `Source`.
    pub target_drift: bool,
    /// `Source` differs from `Remote` (either side may have unpulled/unpushed commits).
    pub remote_drift: bool,
}

/// Three-way drift report for plain files: `Target` (live disk) vs `Source` (repo
/// working tree) vs `Remote` (git remote). Fetches `Remote` first so the comparison
/// reflects commits pushed from elsewhere but not yet pulled.
pub fn diff(source: &Path, target: &Path) -> Result<Vec<FileDrift>, String> {
    git::fetch(source)?;
    let upstream = git::upstream_ref(source)?;

    let mut paths: BTreeSet<PathBuf> = walk_files(source)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|p| p.strip_prefix(source).expect("entry is under source").to_path_buf())
        .collect();
    paths.extend(git::list_tree(source, &upstream)?);

    let mut drifts = Vec::new();
    for path in paths {
        let source_content = fs::read(source.join(&path)).ok();
        let target_content = fs::read(target.join(&path)).ok();
        let remote_content = git::show(source, &upstream, &path)?;

        let target_drift = target_content != source_content;
        let remote_drift = source_content != remote_content;

        if target_drift || remote_drift {
            drifts.push(FileDrift { path, target_drift, remote_drift });
        }
    }
    Ok(drifts)
}

/// One line per drifted path: `<path>\t<target,remote>`. Empty string when clean.
pub fn format_drifts(drifts: &[FileDrift]) -> String {
    let mut out = String::new();
    for d in drifts {
        let mut sides = Vec::new();
        if d.target_drift {
            sides.push("target");
        }
        if d.remote_drift {
            sides.push("remote");
        }
        out.push_str(&format!("{}\t{}\n", d.path.to_string_lossy(), sides.join(",")));
    }
    out
}
