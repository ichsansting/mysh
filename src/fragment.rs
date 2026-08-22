use crate::apply::is_internal_dir_name;
use crate::secret::{self, PassphraseFn};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The Source-side suffix that marks a directory as a Fragment-composed target.
///
/// Deliberately not `d`: that's an extremely common real-world directory-name suffix
/// (fish's native `conf.d`, `sudoers.d`, `cron.d`, ...) for directories whose files stay
/// separate on Target — the opposite of Fragment composition. Colliding with it would
/// silently swallow those into a single concatenated file.
pub const SUFFIX: &str = "frag";

/// Whether `path`'s file name marks it as a Fragment directory (`<name>.frag`).
pub fn is_fragment_dir(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == SUFFIX)
}

/// A Fragment directory's rendered Target path: `relative` with its `.frag` suffix stripped.
pub fn target_name(relative: &Path) -> PathBuf {
    relative.with_extension("")
}

/// Whether `path` (Source-relative) is a fragment itself — a file living inside some
/// Fragment directory — rather than an independently-rendered Target.
pub fn is_fragment_member(path: &Path) -> bool {
    path.ancestors().skip(1).any(is_fragment_dir)
}

/// Fragment directories anywhere under `source` (skipping `.git`/`.mysh`), returned as
/// paths relative to `source`. Does not descend into a Fragment directory itself —
/// nested Fragment composition isn't supported.
pub fn find_fragment_dirs(source: &Path) -> io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, source: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if is_internal_dir_name(path.file_name().and_then(|n| n.to_str())) {
                continue;
            }
            if is_fragment_dir(&path) {
                out.push(path.strip_prefix(source).expect("entry is under source").to_path_buf());
            } else {
                walk(&path, source, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(source, source, &mut out)?;
    out.sort();
    Ok(out)
}

/// Fragment files directly inside `dir`, in lexical filename order — the order they're
/// concatenated in. A newly added fragment file needs no registration: it's picked up
/// here on the next call simply by existing on disk.
fn fragment_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Renders a Fragment directory's contents into a single byte stream: every direct
/// child file, in lexical filename order, concatenated — decrypting any `.age`-suffixed
/// fragment first. `get_passphrase` is only called when a secret fragment is actually
/// encountered.
pub fn render(fragment_dir: &Path, get_passphrase: &mut PassphraseFn) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for file in fragment_files(fragment_dir).map_err(|e| e.to_string())? {
        let content = fs::read(&file).map_err(|e| e.to_string())?;
        if secret::is_secret(&file) {
            let plaintext = secret::decrypt(&content, &get_passphrase()?)?;
            out.extend(plaintext);
        } else {
            out.extend(content);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_fragment_dir_matches_only_the_frag_suffix() {
        assert!(is_fragment_dir(Path::new("nvim/init.frag")));
        assert!(!is_fragment_dir(Path::new("nvim/init")));
        assert!(!is_fragment_dir(Path::new("bashrc")));
        assert!(!is_fragment_dir(Path::new("fish/conf.d")));
    }

    #[test]
    fn target_name_strips_only_the_frag_extension() {
        assert_eq!(target_name(Path::new("nvim/init.frag")), Path::new("nvim/init"));
    }

    #[test]
    fn is_fragment_member_detects_files_under_a_fragment_dir_at_any_depth() {
        assert!(is_fragment_member(Path::new("nvim/init.frag/10-base")));
        assert!(!is_fragment_member(Path::new("nvim/init")));
    }
}
