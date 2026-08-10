use crate::fragment;
use crate::log::AppLog;
use crate::package;
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
    for entry in walk_source_files(source).map_err(|e| e.to_string())? {
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
            write_if_changed(dest, &content, false).map_err(|e| e.to_string())
        })?;
    }

    package::apply(source, target, &log)?;
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
/// working tree's VCS dir; `.mysh`, mysh's *generated runtime state* under Target —
/// bootstrap binaries, the Application Log, `mise`'s isolated data dir) — never user
/// content when walking `Target`, so never walked there by `walk_files` or `diff`'s
/// directory-mode tracking. Does *not* apply to `Source`'s own `.mysh/bin/` (real,
/// git-tracked lazy-package shims — see ADR-0006), which is why `walk_source_files`
/// exists as a separate entry point that skips only `.git`.
pub(crate) fn is_internal_dir_name(name: Option<&str>) -> bool {
    matches!(name, Some(".git") | Some(".mysh"))
}

/// Recursively lists files under `dir`, skipping `.git`/`.mysh` and not descending into
/// `Fragment` directories (`<name>.d/`, handled separately as a composed unit). For
/// walking `Target` (or a `.track`-marked subdirectory of it) — `.mysh` there is always
/// mysh's own generated state, never something to discover as trackable content. Use
/// `walk_source_files` for `Source`.
pub(crate) fn walk_files(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    walk(dir, true)
}

/// Like `walk_files`, but for walking `Source`: never skips `.mysh`, since
/// `Source`'s `.mysh/bin/` holds real, git-tracked lazy-package shims that must be
/// rendered/diffed like any other tracked file (see ADR-0006). Still skips `.git`
/// (Source's own VCS directory) and doesn't descend into `Fragment` directories.
pub(crate) fn walk_source_files(dir: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    walk(dir, false)
}

fn walk(dir: &Path, skip_mysh: bool) -> io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str());
            if name == Some(".git") || (skip_mysh && name == Some(".mysh")) || fragment::is_fragment_dir(&path) {
                continue;
            }
            files.extend(walk(&path, skip_mysh)?);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

/// Identity-copies `src` to `dest`, preserving `src`'s executable bit — needed since
/// ADR-0006: a lazy package's shim is now an ordinary tracked file in Source, ferried
/// to Target through this exact path, and it must stay executable to be runnable.
fn copy_if_changed(src: &Path, dest: &Path) -> io::Result<()> {
    write_if_changed(dest, &fs::read(src)?, is_executable(src)?)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::metadata(path)?.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> io::Result<bool> {
    Ok(false)
}

fn write_if_changed(dest: &Path, content: &[u8], executable: bool) -> io::Result<()> {
    if fs::read(dest).map(|existing| existing == content).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, content)?;
    if executable {
        set_executable(dest)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
