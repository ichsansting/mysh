use crate::fragment;
use crate::log::AppLog;
use crate::secret::{self, PassphraseFn};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Renders every file in `source` to its mirrored relative path under `target`: plain
/// files via identity copy, `Secret`s (`.age`-suffixed) via decrypt, `Fragment`
/// directories (`<name>.d/`) via concatenate-in-filename-order into a single `<name>`
/// file. Skips `.git` (Source is a git working tree). Idempotent: a file is only
/// (re)written when its content differs from what's already at the target path.
///
/// The first time a given path is applied, pre-existing content at that path is backed
/// up and the Application Log records it as overwritten; a path with no prior content
/// is recorded as created. Once a path is logged, later applies never re-back-up or
/// re-classify it.
pub fn apply(source: &Path, target: &Path, get_passphrase: &mut PassphraseFn) -> Result<(), String> {
    render(source, target, get_passphrase).map_err(|e| e.to_string())
}

fn render(source: &Path, target: &Path, get_passphrase: &mut PassphraseFn) -> Result<(), String> {
    let log = AppLog::open(target);
    for entry in walk_files(source).map_err(|e| e.to_string())? {
        let source_relative = entry.strip_prefix(source).expect("entry is under source");
        let is_secret = secret::is_secret(source_relative);
        let relative: PathBuf = if is_secret {
            secret::strip_suffix(source_relative)
        } else {
            source_relative.to_path_buf()
        };

        if is_secret {
            let ciphertext = fs::read(&entry).map_err(|e| e.to_string())?;
            let passphrase = get_passphrase()?;
            let plaintext = secret::decrypt(&ciphertext, &passphrase)?;
            apply_one(&log, target, &relative, |dest| {
                secret::write_restricted(dest, &plaintext).map_err(|e| e.to_string())
            })?;
        } else {
            apply_one(&log, target, &relative, |dest| {
                copy_if_changed(&entry, dest).map_err(|e| e.to_string())
            })?;
        }
    }

    for fragment_dir in fragment::find_fragment_dirs(source).map_err(|e| e.to_string())? {
        let relative = fragment::target_name(&fragment_dir);
        let content = fragment::render(&source.join(&fragment_dir), get_passphrase)?;
        apply_one(&log, target, &relative, |dest| {
            write_if_changed(dest, &content).map_err(|e| e.to_string())
        })?;
    }
    Ok(())
}

/// The first-touch/backup/record bookkeeping shared by every render kind (plain,
/// secret, fragment): backs up pre-existing content at `target`-relative `relative` on
/// first touch, calls `write` to render the new content, then records
/// created/overwritten in the Application Log. Only recorded once `write` has
/// succeeded, so the log never claims a path was touched when it wasn't; once a path is
/// logged, later applies never re-back-up or re-classify it.
fn apply_one(
    log: &AppLog,
    target: &Path,
    relative: &Path,
    write: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let dest = target.join(relative);
    let first_touch = !log.is_managed(relative).map_err(|e| e.to_string())?;
    let backup = if first_touch && dest.exists() {
        Some(back_up(log, target, relative, &dest).map_err(|e| e.to_string())?)
    } else {
        None
    };

    write(&dest)?;

    if first_touch {
        match backup {
            Some(backup_relative) => {
                log.record_overwritten(relative, &backup_relative).map_err(|e| e.to_string())?
            }
            None => log.record_created(relative).map_err(|e| e.to_string())?,
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

/// Recursively lists files under `dir`, skipping `.git`/`.mysh` and not descending into
/// `Fragment` directories (`<name>.d/`, handled separately as a composed unit). Shared
/// with `diff`, which also needs the set of plain files Source/Target currently have on
/// disk.
pub(crate) fn walk_files(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if is_internal_dir_name(path.file_name().and_then(|n| n.to_str()))
                || fragment::is_fragment_dir(&path)
            {
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
    write_if_changed(dest, &fs::read(src)?)
}

fn write_if_changed(dest: &Path, content: &[u8]) -> io::Result<()> {
    if fs::read(dest).map(|existing| existing == content).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, content)
}
