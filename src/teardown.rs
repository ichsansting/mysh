use crate::confirm::confirm;
use crate::log::{AppLog, LogEntry};
use crate::mise;
use std::fs;
use std::io;
use std::io::BufRead;
use std::path::Path;

/// Fully reverses everything mysh has done to `target` by replaying its Application
/// Log in reverse: deletes every file mysh created and restores every backed-up
/// original, uninstalls every package mysh installed (and `mise` itself, if mysh
/// bootstrapped it), strips every `PATH`/rc-file line mysh added — including the
/// bootstrap installer's own — then removes the `mysh` binary and the rest of the
/// installer's footprint last. Always requires explicit confirmation on `input`
/// before mutating anything; declining leaves `target` unchanged. A device mysh never
/// touched (no log) is a no-op.
pub fn teardown(target: &Path, input: &mut dyn BufRead) -> Result<String, String> {
    let log = AppLog::open(target);
    let entries = log.entries().map_err(|e| e.to_string())?;
    if entries.is_empty() {
        return Ok("nothing to tear down\n".to_string());
    }

    print!("{}", summarize(&entries));
    if !confirm(input, "fully reverse everything mysh has done to this device? [y/N] ")? {
        return Ok("aborted\n".to_string());
    }

    // 1. Delete every created file, restore every backed-up original.
    for entry in &entries {
        match entry {
            LogEntry::Created(relative) => remove_file_if_exists(&target.join(relative))?,
            LogEntry::Overwritten { relative, backup } => restore_backup(target, relative, backup)?,
            _ => {}
        }
    }

    // 2. Uninstall every package: the whole isolated mise data dir is mysh-owned (see
    // mise::data_dir), so deleting it uninstalls all of them at once. Then mise itself,
    // only if this device's mise was mysh-bootstrapped — a system-wide mise never has
    // an owned binary path recorded, so it's never touched.
    remove_dir_if_exists(&mise::data_dir(target))?;
    for entry in &entries {
        if let LogEntry::MiseBootstrapped(path) = entry {
            remove_file_if_exists(path)?;
        }
    }

    // 3. Strip every PATH/rc-file line mysh added, including bootstrap's own.
    for entry in &entries {
        if let LogEntry::BootstrapPathAdded { rc_file, path_line } = entry {
            strip_line(rc_file, path_line)?;
        }
    }

    // 4. The mysh binary and the rest of the installer's footprint, removed last.
    for entry in &entries {
        if let LogEntry::BootstrapInstalled(path) = entry {
            remove_file_if_exists(path)?;
        }
    }

    // Whatever mysh-owned residue remains — lazy-package shims, the log itself, the
    // backups dir, a default-location Source clone — lives entirely under
    // `target/.mysh`, so one final sweep guarantees no residue survives.
    remove_dir_if_exists(&target.join(".mysh"))?;

    Ok("torn down\n".to_string())
}

/// One line per log entry describing the action `teardown` is about to take — shown
/// before the confirmation prompt, the same "show pending state, then confirm" shape
/// `save`/`reset` already use (`diff::format_drifts`).
fn summarize(entries: &[LogEntry]) -> String {
    entries.iter().map(describe).collect()
}

fn describe(entry: &LogEntry) -> String {
    match entry {
        LogEntry::Created(relative) => format!("delete {}\n", relative.display()),
        LogEntry::Overwritten { relative, .. } => format!("restore {}\n", relative.display()),
        LogEntry::MiseBootstrapped(path) => format!("remove mise ({})\n", path.display()),
        LogEntry::PackageInstalled(specifier) => format!("uninstall package {specifier}\n"),
        LogEntry::BootstrapInstalled(path) => format!("remove mysh binary ({})\n", path.display()),
        LogEntry::BootstrapPathAdded { rc_file, .. } => {
            format!("strip PATH line from {}\n", rc_file.display())
        }
    }
}

/// Moves a backed-up original back over its managed path, discarding whatever drifted
/// there since — `target`-relative `relative` and `backup` match `AppLog::record_overwritten`.
fn restore_backup(target: &Path, relative: &Path, backup: &Path) -> Result<(), String> {
    let dest = target.join(relative);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(target.join(backup), &dest).map_err(|e| e.to_string())
}

/// The exact comment `bootstrap.sh` writes immediately before its own PATH line.
const BOOTSTRAP_COMMENT: &str = "# added by mysh bootstrap.sh";

/// Removes `path_line` from `rc_file`. `bootstrap.sh` always appends its PATH addition
/// as one fixed block — `printf '\n# added by mysh bootstrap.sh\n%s\n' "$path_line"` —
/// so removing that exact block restores the rc file byte-for-byte to its
/// pre-bootstrap content. Any other `path_line` (or a bootstrap block a later hand-edit
/// has disturbed) falls back to stripping just the matching line(s). A missing rc file
/// is a no-op — nothing to strip.
fn strip_line(rc_file: &Path, path_line: &str) -> Result<(), String> {
    let text = match fs::read_to_string(rc_file) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let block = format!("\n{BOOTSTRAP_COMMENT}\n{path_line}\n");
    let stripped = if text.contains(&block) {
        text.replacen(&block, "", 1)
    } else {
        text.lines()
            .filter(|line| *line != path_line && *line != BOOTSTRAP_COMMENT)
            .map(|line| format!("{line}\n"))
            .collect()
    };
    fs::write(rc_file, stripped).map_err(|e| e.to_string())
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn strip_line_removes_the_whole_bootstrap_block_byte_exact() {
        let dir = std::env::temp_dir().join(format!("mysh-teardown-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rc_file = dir.join("rcfile");
        fs::write(
            &rc_file,
            "export EDITOR=vim\n\n# added by mysh bootstrap.sh\nexport PATH=\"x:$PATH\"\n",
        )
        .unwrap();

        strip_line(&rc_file, "export PATH=\"x:$PATH\"").unwrap();

        assert_eq!(fs::read_to_string(&rc_file).unwrap(), "export EDITOR=vim\n");
    }

    #[test]
    fn strip_line_falls_back_to_line_removal_when_the_bootstrap_block_is_disturbed() {
        let dir =
            std::env::temp_dir().join(format!("mysh-teardown-test-disturbed-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rc_file = dir.join("rcfile");
        // No blank-line/comment framing, e.g. a hand-added line rather than bootstrap's own.
        fs::write(&rc_file, "export EDITOR=vim\nexport PATH=\"x:$PATH\"\n").unwrap();

        strip_line(&rc_file, "export PATH=\"x:$PATH\"").unwrap();

        assert_eq!(fs::read_to_string(&rc_file).unwrap(), "export EDITOR=vim\n");
    }

    #[test]
    fn summarize_describes_every_entry_kind_on_its_own_line() {
        let entries = vec![
            LogEntry::Created(PathBuf::from("bashrc")),
            LogEntry::Overwritten {
                relative: PathBuf::from("gitconfig"),
                backup: PathBuf::from(".mysh/backups/gitconfig"),
            },
            LogEntry::MiseBootstrapped(PathBuf::from("/home/u/.mysh/bin/mise")),
            LogEntry::PackageInstalled("widget@1.0".to_string()),
            LogEntry::BootstrapInstalled(PathBuf::from("/home/u/.mysh/bin/mysh")),
            LogEntry::BootstrapPathAdded {
                rc_file: PathBuf::from("/home/u/.bashrc"),
                path_line: "export PATH=\"x:$PATH\"".to_string(),
            },
        ];
        let summary = summarize(&entries);
        assert_eq!(summary.lines().count(), entries.len());
        assert!(summary.contains("delete bashrc\n"));
        assert!(summary.contains("restore gitconfig\n"));
        assert!(summary.contains("remove mise (/home/u/.mysh/bin/mise)\n"));
        assert!(summary.contains("uninstall package widget@1.0\n"));
        assert!(summary.contains("remove mysh binary (/home/u/.mysh/bin/mysh)\n"));
        assert!(summary.contains("strip PATH line from /home/u/.bashrc\n"));
    }

    #[test]
    fn strip_line_on_missing_rc_file_is_a_noop() {
        let dir = std::env::temp_dir().join(format!("mysh-teardown-test-missing-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        strip_line(&dir.join("no-such-file"), "export PATH=\"x:$PATH\"").unwrap();
    }
}
