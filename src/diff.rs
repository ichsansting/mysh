use crate::apply::{is_internal_dir_name, walk_files};
use crate::git;
use crate::secret::{self, PassphraseFn};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A file's drift, relative to `Source`, across the other two states.
pub struct FileDrift {
    /// `Target`-relative path (a `Secret`'s `.age` suffix is stripped).
    pub path: PathBuf,
    /// `Source`-relative path (a `Secret` keeps its `.age` suffix).
    pub source_path: PathBuf,
    pub is_secret: bool,
    /// `Target` (live disk) differs from `Source`.
    pub target_drift: bool,
    /// `Source` differs from `Remote` (either side may have unpulled/unpushed commits).
    pub remote_drift: bool,
}

/// Three-way drift report: `Target` (live disk) vs `Source` (repo working tree) vs
/// `Remote` (git remote). Fetches `Remote` first so the comparison reflects commits
/// pushed from elsewhere but not yet pulled.
///
/// For a `Secret`, `Source`-vs-`Remote` compares raw ciphertext (a byte-identical blob
/// means identical plaintext, since `save` only ever rewrites it when the decrypted
/// content actually changed) but `Target`-vs-`Source` always decrypts a fresh copy of
/// `Source` and compares plaintext-to-plaintext against `Target` — never ciphertext to
/// plaintext. `get_passphrase` is only called when a `Secret` is actually encountered.
pub fn diff(
    source: &Path,
    target: &Path,
    get_passphrase: &mut PassphraseFn,
) -> Result<Vec<FileDrift>, String> {
    git::fetch(source)?;
    let upstream = git::upstream_ref(source)?;

    let mut paths: BTreeSet<PathBuf> = walk_files(source)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|p| p.strip_prefix(source).expect("entry is under source").to_path_buf())
        .collect();
    paths.extend(git::list_tree(source, &upstream)?);
    paths.extend(tracked_new_paths(source, target, &paths).map_err(|e| e.to_string())?);

    let mut drifts = Vec::new();
    for source_path in paths {
        let is_secret = secret::is_secret(&source_path);
        let path = if is_secret { secret::strip_suffix(&source_path) } else { source_path.clone() };

        let source_content = fs::read(source.join(&source_path)).ok();
        let target_content = fs::read(target.join(&path)).ok();
        let remote_content = git::show(source, &upstream, &source_path)?;

        let expected_target_content = if is_secret {
            match &source_content {
                Some(ciphertext) => Some(secret::decrypt(ciphertext, &get_passphrase()?)?),
                None => None,
            }
        } else {
            source_content.clone()
        };

        let target_drift = target_content != expected_target_content;
        let remote_drift = source_content != remote_content;

        if target_drift || remote_drift {
            drifts.push(FileDrift { path, source_path, is_secret, target_drift, remote_drift });
        }
    }
    Ok(drifts)
}

/// Directories marked with a `.track` file at their root (recursive search, skipping
/// `.git`/`.mysh`), returned as paths relative to `source`.
fn tracked_dirs(source: &Path) -> io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, source: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        if dir.join(".track").is_file() {
            out.push(dir.strip_prefix(source).expect("entry is under source").to_path_buf());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir()
                && !is_internal_dir_name(path.file_name().and_then(|n| n.to_str()))
            {
                walk(&path, source, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(source, source, &mut out)?;
    Ok(out)
}

/// `.track`'s content as newline-separated glob patterns, blank lines dropped.
fn track_patterns(track_file: &Path) -> Vec<String> {
    fs::read_to_string(track_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Whether `relative` (a path relative to a tracked directory) matches any ignore
/// pattern: patterns containing `/` match the full relative path, others match any
/// single path component (so `.cache` excludes a whole subtree, `*.log` excludes any
/// file named like that at any depth).
fn matches_ignore(patterns: &[String], relative: &Path) -> bool {
    let relative_str = relative.to_string_lossy().replace('\\', "/");
    patterns.iter().any(|p| {
        if p.contains('/') {
            glob_match(p, &relative_str)
        } else {
            relative
                .components()
                .any(|c| glob_match(p, &c.as_os_str().to_string_lossy()))
        }
    })
}

/// Minimal shell-style glob match: `*` any sequence (incl. empty), `?` any single char.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(pc), Some(tc)) if pc == tc => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), text.as_bytes())
}

/// For every `.track`-marked directory in `source`, recursively walks the live
/// `Target` counterpart (independent of git) and returns the paths found there that
/// aren't already in `known` (i.e. absent from Source/Remote) and don't match that
/// directory's ignore patterns. These feed into `diff`'s loop above, where their
/// absence from `Source` naturally trips `target_drift` (new/save candidates).
/// Directories without `.track` are never scanned.
fn tracked_new_paths(
    source: &Path,
    target: &Path,
    known: &BTreeSet<PathBuf>,
) -> io::Result<Vec<PathBuf>> {
    let mut new_paths = Vec::new();
    for relative_dir in tracked_dirs(source)? {
        let target_dir = target.join(&relative_dir);
        if !target_dir.is_dir() {
            continue;
        }
        let patterns = track_patterns(&source.join(&relative_dir).join(".track"));
        for entry in walk_files(&target_dir)? {
            let relative = entry.strip_prefix(target).expect("entry is under target").to_path_buf();
            if known.contains(&relative) {
                continue;
            }
            let relative_to_tracked_dir =
                entry.strip_prefix(&target_dir).expect("entry is under target_dir");
            if matches_ignore(&patterns, relative_to_tracked_dir) {
                continue;
            }
            new_paths.push(relative);
        }
    }
    Ok(new_paths)
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
