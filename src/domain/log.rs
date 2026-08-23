use crate::domain::LOG_REL;
use crate::error::{IoCtx, Result};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// How much of a Target file mysh owns — explicit in every entry so teardown
/// never has to guess (the flaw behind ADR-0008's bug).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// mysh owns the whole file: teardown deletes it, or restores its backup.
    Full,
    /// mysh owns only declared keys (Overlay): teardown leaves the file in place.
    Partial,
}

/// One line of the Application Log. TSV, append-only, POSIX-printf-writable —
/// the `Bootstrap*` kinds are written by bootstrap.sh, not by mysh.
#[derive(Debug, PartialEq, Eq)]
pub enum LogEntry {
    /// `target\t<full|partial>\t<rel>[\t<backup-rel>]` — a rendered Target file.
    /// Full + backup = pre-existing content saved under `.mysh/backups/`;
    /// Full without backup = created fresh.
    Target { ownership: Ownership, rel: PathBuf, backup_rel: Option<PathBuf> },
    /// `mise-bootstrapped\t<abs-data-path>` — mysh installed mise itself.
    MiseBootstrapped { path: PathBuf },
    /// `bootstrap-installed\t<abs-path>` — bootstrap.sh placed the mysh binary.
    BootstrapInstalled { path: PathBuf },
    /// `bootstrap-path-added\t<rc-file>\t<line>` — bootstrap.sh appended an rc line.
    BootstrapPathAdded { rc_file: PathBuf, line: String },
}

impl LogEntry {
    fn format(&self) -> String {
        match self {
            LogEntry::Target { ownership, rel, backup_rel } => {
                let ownership = match ownership {
                    Ownership::Full => "full",
                    Ownership::Partial => "partial",
                };
                match backup_rel {
                    Some(backup) => {
                        format!("target\t{ownership}\t{}\t{}", rel.display(), backup.display())
                    }
                    None => format!("target\t{ownership}\t{}", rel.display()),
                }
            }
            LogEntry::MiseBootstrapped { path } => format!("mise-bootstrapped\t{}", path.display()),
            LogEntry::BootstrapInstalled { path } => {
                format!("bootstrap-installed\t{}", path.display())
            }
            LogEntry::BootstrapPathAdded { rc_file, line } => {
                format!("bootstrap-path-added\t{}\t{line}", rc_file.display())
            }
        }
    }

    /// The single parser. `None` for an unrecognized or malformed line — skipped
    /// silently on read, the log's forward-compatibility rule.
    fn parse(line: &str) -> Option<LogEntry> {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["target", ownership, rel] | ["target", ownership, rel, _] => {
                let ownership = match *ownership {
                    "full" => Ownership::Full,
                    "partial" => Ownership::Partial,
                    _ => return None,
                };
                let backup_rel = fields.get(3).map(PathBuf::from);
                Some(LogEntry::Target { ownership, rel: PathBuf::from(rel), backup_rel })
            }
            ["mise-bootstrapped", path] => {
                Some(LogEntry::MiseBootstrapped { path: PathBuf::from(path) })
            }
            ["bootstrap-installed", path] => {
                Some(LogEntry::BootstrapInstalled { path: PathBuf::from(path) })
            }
            ["bootstrap-path-added", rc_file, line] => Some(LogEntry::BootstrapPathAdded {
                rc_file: PathBuf::from(rc_file),
                line: line.to_string(),
            }),
            _ => None,
        }
    }
}

/// The per-device Application Log at `<target>/.mysh/log`.
pub struct AppLog {
    path: PathBuf,
}

impl AppLog {
    pub fn at(target_dir: &Path) -> AppLog {
        AppLog { path: target_dir.join(LOG_REL) }
    }

    /// Every recognized entry plus the count of unrecognized lines (which stay
    /// in the file untouched — an older mysh must never destroy a newer one's state).
    pub fn read(&self) -> Result<(Vec<LogEntry>, usize)> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(e) => return Err(e).at("read", &self.path),
        };
        let mut entries = Vec::new();
        let mut unrecognized = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match LogEntry::parse(line) {
                Some(entry) => entries.push(entry),
                None => unrecognized += 1,
            }
        }
        Ok((entries, unrecognized))
    }

    /// The set of target-relative paths with a `Target` entry — i.e. already under
    /// first-touch bookkeeping, so re-applying must not re-backup.
    pub fn managed_targets(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .read()?
            .0
            .into_iter()
            .filter_map(|entry| match entry {
                LogEntry::Target { rel, .. } => Some(rel),
                _ => None,
            })
            .collect())
    }

    pub fn record(&self, entry: &LogEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).at("create directory", parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .at("open", &self.path)?;
        writeln!(file, "{}", entry.format()).at("append", &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_kind_round_trips_through_format_and_parse() {
        let entries = [
            LogEntry::Target {
                ownership: Ownership::Full,
                rel: PathBuf::from(".bashrc"),
                backup_rel: None,
            },
            LogEntry::Target {
                ownership: Ownership::Full,
                rel: PathBuf::from(".gitconfig"),
                backup_rel: Some(PathBuf::from(".mysh/backups/.gitconfig")),
            },
            LogEntry::Target {
                ownership: Ownership::Partial,
                rel: PathBuf::from(".claude.json"),
                backup_rel: None,
            },
            LogEntry::MiseBootstrapped { path: PathBuf::from("/t/.mysh/bin/mise") },
            LogEntry::BootstrapInstalled { path: PathBuf::from("/t/.mysh/bin/mysh") },
            LogEntry::BootstrapPathAdded {
                rc_file: PathBuf::from("/t/rc"),
                line: "export PATH=\"/t/.mysh/bin:$PATH\"".into(),
            },
        ];
        for entry in entries {
            assert_eq!(LogEntry::parse(&entry.format()).as_ref(), Some(&entry));
        }
    }

    #[test]
    fn unknown_kinds_are_skipped_not_errors() {
        assert_eq!(LogEntry::parse("future-kind\twhatever\textra"), None);
        assert_eq!(LogEntry::parse("target\tsomeday-ownership\tx"), None);
    }
}
