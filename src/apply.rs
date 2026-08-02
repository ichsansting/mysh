use crate::log::AppLog;
use std::fs;
use std::io;
use std::path::Path;

/// Renders every plain file in `source` to its mirrored relative path under `target`
/// via identity copy. Skips `.git` (Source is a git working tree). Idempotent: a file
/// is only (re)written when its content differs from what's already at the target path.
///
/// The first time a given path is applied, pre-existing content at that path is backed
/// up and the Application Log records it as overwritten; a path with no prior content
/// is recorded as created. Once a path is logged, later applies never re-back-up or
/// re-classify it.
pub fn apply(source: &Path, target: &Path) -> Result<(), String> {
    render(source, target).map_err(|e| e.to_string())
}

fn render(source: &Path, target: &Path) -> io::Result<()> {
    let log = AppLog::open(target);
    for entry in walk_files(source)? {
        let relative = entry.strip_prefix(source).expect("entry is under source");
        let dest = target.join(relative);
        let first_touch = !log.is_managed(relative)?;
        let backup = if first_touch && dest.exists() {
            Some(back_up(&log, target, relative, &dest)?)
        } else {
            None
        };

        copy_if_changed(&entry, &dest)?;

        // Only recorded once the real write above has succeeded, so the log never
        // claims a path was created/overwritten when it wasn't actually written.
        if first_touch {
            match backup {
                Some(backup_relative) => log.record_overwritten(relative, &backup_relative)?,
                None => log.record_created(relative)?,
            }
        }
    }
    Ok(())
}

/// Copies `dest`'s pre-existing content to its backup location and returns that
/// location, relative to `target`.
fn back_up(
    log: &AppLog,
    target: &Path,
    relative: &Path,
    dest: &Path,
) -> io::Result<std::path::PathBuf> {
    let backup_relative = log.backup_path_for(relative);
    let backup_absolute = target.join(&backup_relative);
    if let Some(parent) = backup_absolute.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(dest, &backup_absolute)?;
    Ok(backup_relative)
}

/// Whether `name` is one of mysh's own internal directories (`.git`, the Source
/// working tree's VCS dir; `.mysh`, mysh's state dir under Target) — never user
/// content, so never walked by `walk_files` or `diff`'s directory-mode tracking.
pub(crate) fn is_internal_dir_name(name: Option<&str>) -> bool {
    matches!(name, Some(".git") | Some(".mysh"))
}

/// Recursively lists files under `dir`, skipping `.git` and `.mysh` (shared with
/// `diff`, which also needs the set of plain files Source/Target currently have on
/// disk).
pub(crate) fn walk_files(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if is_internal_dir_name(path.file_name().and_then(|n| n.to_str())) {
                continue;
            }
            files.extend(walk_files(&path)?);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn copy_if_changed(src: &Path, dest: &Path) -> io::Result<()> {
    let content = fs::read(src)?;
    if fs::read(dest).map(|existing| existing == content).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, content)
}
