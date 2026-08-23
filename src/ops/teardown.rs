use crate::config::Config;
use crate::domain::MYSH_DIR_REL;
use crate::domain::log::{AppLog, LogEntry, Ownership};
use crate::error::{IoCtx, Result};
use crate::infra::prompt;
use std::fs;
use std::io::BufRead;
use std::path::Path;

/// The comment header bootstrap.sh writes above every rc line it appends;
/// stripped together with the line itself.
const RC_COMMENT: &str = "# added by mysh bootstrap.sh";

/// Teardown: replay the Application Log to return the device to its pre-mysh
/// state. Four ordered passes (Target files, mise prefix, rc lines, bootstrap
/// binary), then the whole `.mysh` dir goes. One deliberate exception
/// (ADR-0008): partially-owned Overlay targets are left in place.
pub fn run(config: &Config, input: &mut dyn BufRead) -> Result<String> {
    let log = AppLog::at(&config.target_dir);
    let (entries, unrecognized) = log.read()?;

    if !prompt::confirm(input, &summarize(&entries, unrecognized))? {
        return Ok("aborted\n".to_string());
    }

    // Pass 1: fully-owned Target files — delete created, restore overwritten.
    for entry in &entries {
        let LogEntry::Target {
            ownership,
            rel,
            backup_rel,
        } = entry
        else {
            continue;
        };
        if *ownership == Ownership::Partial {
            continue; // ADR-0008: mysh never owned the rest of this file.
        }
        let dest = config.target_dir.join(rel);
        match backup_rel {
            Some(backup_rel) => {
                let backup = config.target_dir.join(backup_rel);
                let original = fs::read(&backup).at("read", &backup)?;
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).at("create directory", parent)?;
                }
                fs::write(&dest, original).at("restore", &dest)?;
            }
            None => remove_file_if_present(&dest)?,
        }
    }

    // Pass 2: the isolated mise prefix — all packages go with it.
    for entry in &entries {
        if let LogEntry::MiseBootstrapped { path } = entry {
            remove_file_if_present(path)?;
        }
    }

    // Pass 3: rc-file lines bootstrap.sh appended.
    for entry in &entries {
        if let LogEntry::BootstrapPathAdded { rc_file, line } = entry {
            strip_line(rc_file, line)?;
        }
    }

    // Pass 4: the bootstrap-installed binary itself.
    for entry in &entries {
        if let LogEntry::BootstrapInstalled { path } = entry {
            remove_file_if_present(path)?;
        }
    }

    // Everything mysh owns lives under .mysh (log, backups, source clone, mise
    // data dir, bin dir) — one blanket removal leaves zero residue.
    let mysh_dir = config.target_dir.join(MYSH_DIR_REL);
    match fs::remove_dir_all(&mysh_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).at("remove", &mysh_dir),
    }

    Ok("torn down\n".to_string())
}

fn summarize(entries: &[LogEntry], unrecognized: usize) -> String {
    let mut lines = Vec::new();
    for entry in entries {
        lines.push(match entry {
            LogEntry::Target {
                ownership: Ownership::Partial,
                rel,
                ..
            } => {
                format!(
                    "leave {} (overlay; declared keys stay merged)",
                    rel.display()
                )
            }
            LogEntry::Target {
                rel,
                backup_rel: Some(_),
                ..
            } => {
                format!("restore {} to its pre-mysh content", rel.display())
            }
            LogEntry::Target {
                rel,
                backup_rel: None,
                ..
            } => {
                format!("delete {}", rel.display())
            }
            LogEntry::MiseBootstrapped { path } => {
                format!("delete {} (mise and all packages)", path.display())
            }
            LogEntry::BootstrapPathAdded { rc_file, .. } => {
                format!("remove mysh lines from {}", rc_file.display())
            }
            LogEntry::BootstrapInstalled { path } => format!("delete {}", path.display()),
        });
    }
    if unrecognized > 0 {
        lines.push(format!("left {unrecognized} unrecognized entries in place"));
    }
    if lines.is_empty() {
        lines.push("nothing recorded; remove any .mysh directory".to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).at("remove", path),
    }
}

/// Removes an appended rc line (and bootstrap.sh's comment header) from `rc_file`.
fn strip_line(rc_file: &Path, line: &str) -> Result<()> {
    let text = match fs::read_to_string(rc_file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).at("read", rc_file),
    };
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| *l != line && *l != RC_COMMENT)
        .collect();
    let mut result = kept.join("\n");
    if text.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    fs::write(rc_file, result).at("write", rc_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_line_removes_the_line_and_its_comment_header() {
        let dir = std::env::temp_dir().join(format!("mysh-strip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rc = dir.join("rc");
        let line = "export PATH=\"/t/.mysh/bin:$PATH\"";
        fs::write(&rc, format!("# mine\n\n{RC_COMMENT}\n{line}\n")).unwrap();
        strip_line(&rc, line).unwrap();
        assert_eq!(fs::read_to_string(&rc).unwrap().trim_end(), "# mine");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn strip_line_on_a_missing_rc_file_is_a_noop() {
        assert!(strip_line(Path::new("/nonexistent/rc"), "x").is_ok());
    }
}
