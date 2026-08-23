use crate::error::{IoCtx, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Directory names that are never part of managed content.
pub fn is_internal_dir(name: &str) -> bool {
    name == ".git"
}

/// Every file under `root`, root-relative, sorted for deterministic order.
/// `descend` decides whether to walk into a directory (given its root-relative path);
/// a directory it refuses is reported as a single entry instead, so callers can treat
/// e.g. a `.frag` directory as one unit.
pub fn walk(root: &Path, descend: &dyn Fn(&Path) -> bool) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    walk_into(root, root, descend, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk_into(
    root: &Path,
    dir: &Path,
    descend: &dyn Fn(&Path) -> bool,
    found: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).at("walk", dir),
    };
    for entry in entries {
        let entry = entry.at("walk", dir)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = path.strip_prefix(root).expect("walked path is under root").to_path_buf();
        if path.is_dir() {
            if is_internal_dir(&name) {
                continue;
            }
            if descend(&rel) {
                walk_into(root, &path, descend, found)?;
            } else {
                found.push(rel);
            }
        } else {
            found.push(rel);
        }
    }
    Ok(())
}

/// Writes `content` at `path` (creating parents) only when it differs from what's
/// already there, so an unchanged apply never rewrites a file (mtime stays put).
/// When `mode` is given it is enforced unconditionally with an explicit chmod after
/// any write — create-time modes get filtered through the umask, chmod applies the
/// bits verbatim.
pub fn write_if_changed(path: &Path, content: &[u8], mode: Option<u32>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).at("create directory", parent)?;
    }
    let unchanged = fs::read(path).map(|existing| existing == content).unwrap_or(false);
    if !unchanged {
        fs::write(path, content).at("write", path)?;
    }
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).at("chmod", path)?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

/// A file's permission bits, unix only (`None` elsewhere).
pub fn mode_of(path: &Path) -> Result<Option<u32>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path).at("stat", path)?;
        Ok(Some(meta.permissions().mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_if_changed_leaves_mtime_alone_when_content_matches() {
        let dir = std::env::temp_dir().join(format!("mysh-fsx-{}", std::process::id()));
        let path = dir.join("f");
        write_if_changed(&path, b"same", None).unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_if_changed(&path, b"same", None).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
        fs::remove_dir_all(&dir).unwrap();
    }
}
