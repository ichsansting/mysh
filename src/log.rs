use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Per-device record of every Target path mysh has touched: whether it was created
/// fresh or overwrote pre-existing content (with the backup location). This is what
/// lets `teardown` later delete the former and restore the latter. Lives under
/// `<target>/.mysh` — never in Source, which is a public repo (ADR-0004).
pub struct AppLog {
    target: PathBuf,
}

enum Entry {
    Created,
    Overwritten,
}

/// One raw entry from the Application Log, corresponding 1:1 to a `record_*` write.
/// `teardown` replays these in reverse to undo everything mysh has done to a device.
/// Paths on `Created`/`Overwritten` are relative to `target` (as written by
/// `record_created`/`record_overwritten`); paths on `MiseBootstrapped`/
/// `BootstrapInstalled`/`BootstrapPathAdded.rc_file` are already absolute (as written
/// by `record_mise_bootstrapped` and by `bootstrap.sh` itself).
#[derive(Debug, PartialEq)]
pub enum LogEntry {
    Created(PathBuf),
    Overwritten { relative: PathBuf, backup: PathBuf },
    /// An `Overlay` target mysh merged declared keys into. Deliberately *not*
    /// `Created`/`Overwritten`: teardown leaves the file in place (see ADR-0008) —
    /// mysh never owned its other keys, so neither deleting it nor restoring a
    /// whole-file backup could be right.
    OverlayTouched(PathBuf),
    MiseBootstrapped(PathBuf),
    PackageInstalled(String),
    BootstrapInstalled(PathBuf),
    BootstrapPathAdded { rc_file: PathBuf, path_line: String },
}

fn parse_entry(line: &str) -> Option<LogEntry> {
    let fields: Vec<&str> = line.split('\t').collect();
    match fields.as_slice() {
        ["created", relative] => Some(LogEntry::Created(PathBuf::from(relative))),
        ["overwritten", relative, backup] => {
            Some(LogEntry::Overwritten { relative: PathBuf::from(relative), backup: PathBuf::from(backup) })
        }
        ["overlay-touched", relative] => Some(LogEntry::OverlayTouched(PathBuf::from(relative))),
        ["mise-bootstrapped", path] => Some(LogEntry::MiseBootstrapped(PathBuf::from(path))),
        ["package-installed", specifier] => Some(LogEntry::PackageInstalled(specifier.to_string())),
        ["bootstrap-installed", path] => Some(LogEntry::BootstrapInstalled(PathBuf::from(path))),
        ["bootstrap-path-added", rc_file, path_line] => Some(LogEntry::BootstrapPathAdded {
            rc_file: PathBuf::from(rc_file),
            path_line: path_line.to_string(),
        }),
        _ => None,
    }
}

impl AppLog {
    pub fn open(target: &Path) -> AppLog {
        AppLog { target: target.to_path_buf() }
    }

    fn state_dir(&self) -> PathBuf {
        self.target.join(".mysh")
    }

    fn log_path(&self) -> PathBuf {
        self.state_dir().join("log")
    }

    /// Where a backup for `relative` should be written, relative to the Target root.
    pub fn backup_path_for(&self, relative: &Path) -> PathBuf {
        Path::new(".mysh/backups").join(relative)
    }

    pub fn is_managed(&self, relative: &Path) -> io::Result<bool> {
        Ok(self.entry(relative)?.is_some())
    }

    fn entry(&self, relative: &Path) -> io::Result<Option<Entry>> {
        let text = match fs::read_to_string(self.log_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let wanted = relative.to_string_lossy();
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(kind), Some(path)) = (fields.next(), fields.next()) else {
                continue;
            };
            if path != wanted {
                continue;
            }
            return Ok(Some(if kind == "overwritten" {
                Entry::Overwritten
            } else {
                Entry::Created
            }));
        }
        Ok(None)
    }

    pub fn record_created(&self, relative: &Path) -> io::Result<()> {
        self.append(&format!("created\t{}\n", relative.to_string_lossy()))
    }

    pub fn record_overwritten(&self, relative: &Path, backup: &Path) -> io::Result<()> {
        self.append(&format!(
            "overwritten\t{}\t{}\n",
            relative.to_string_lossy(),
            backup.to_string_lossy()
        ))
    }

    /// Records that an `Overlay` merged declared keys into `relative` — makes the path
    /// `is_managed` (so it's only recorded once), while telling `teardown` to leave
    /// the file alone rather than delete/restore it (see ADR-0008).
    pub fn record_overlay_touched(&self, relative: &Path) -> io::Result<()> {
        self.append(&format!("overlay-touched\t{}\n", relative.to_string_lossy()))
    }

    /// Records that this device didn't already have `mise`, and mysh installed it at
    /// `install_path` — what lets `teardown` later delete exactly that file.
    pub fn record_mise_bootstrapped(&self, install_path: &Path) -> io::Result<()> {
        self.append(&format!("mise-bootstrapped\t{}\n", install_path.to_string_lossy()))
    }

    /// Records that mysh installed `specifier` via `mise` — what lets `teardown` later
    /// uninstall it.
    pub fn record_package_installed(&self, specifier: &str) -> io::Result<()> {
        self.append(&format!("package-installed\t{specifier}\n"))
    }

    /// Every entry ever recorded, in the order they were appended — what `teardown`
    /// replays. A missing log (mysh never touched this device) is an empty list, not
    /// an error, mirroring every other read in this module. Unrecognized lines are
    /// skipped rather than erroring, the same forward-compatible tolerance `entry()`
    /// already has for unknown `kind`s.
    pub fn entries(&self) -> io::Result<Vec<LogEntry>> {
        let text = match fs::read_to_string(self.log_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        Ok(text.lines().filter_map(parse_entry).collect())
    }

    fn append(&self, line: &str) -> io::Result<()> {
        fs::create_dir_all(self.state_dir())?;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?
            .write_all(line.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entry_covers_every_recorded_kind() {
        assert_eq!(parse_entry("created\tbashrc"), Some(LogEntry::Created(PathBuf::from("bashrc"))));
        assert_eq!(
            parse_entry("overwritten\tbashrc\t.mysh/backups/bashrc"),
            Some(LogEntry::Overwritten {
                relative: PathBuf::from("bashrc"),
                backup: PathBuf::from(".mysh/backups/bashrc"),
            })
        );
        assert_eq!(
            parse_entry("overlay-touched\t.claude.json"),
            Some(LogEntry::OverlayTouched(PathBuf::from(".claude.json")))
        );
        assert_eq!(
            parse_entry("mise-bootstrapped\t/home/u/.mysh/bin/mise"),
            Some(LogEntry::MiseBootstrapped(PathBuf::from("/home/u/.mysh/bin/mise")))
        );
        assert_eq!(
            parse_entry("package-installed\twidget@1.0"),
            Some(LogEntry::PackageInstalled("widget@1.0".to_string()))
        );
        assert_eq!(
            parse_entry("bootstrap-installed\t/home/u/.mysh/bin/mysh"),
            Some(LogEntry::BootstrapInstalled(PathBuf::from("/home/u/.mysh/bin/mysh")))
        );
        assert_eq!(
            parse_entry("bootstrap-path-added\t/home/u/.bashrc\texport PATH=\"x:$PATH\""),
            Some(LogEntry::BootstrapPathAdded {
                rc_file: PathBuf::from("/home/u/.bashrc"),
                path_line: "export PATH=\"x:$PATH\"".to_string(),
            })
        );
        assert_eq!(parse_entry("garbage"), None);
    }

    #[test]
    fn entries_returns_empty_when_log_is_absent() {
        let dir = std::env::temp_dir().join(format!("mysh-log-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(AppLog::open(&dir).entries().unwrap(), Vec::new());
    }

    #[test]
    fn entries_round_trips_every_record_method_in_append_order() {
        let dir = std::env::temp_dir().join(format!("mysh-log-test-roundtrip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let log = AppLog::open(&dir);
        log.record_created(Path::new("bashrc")).unwrap();
        log.record_overwritten(Path::new("gitconfig"), Path::new(".mysh/backups/gitconfig")).unwrap();
        log.record_overlay_touched(Path::new(".claude.json")).unwrap();
        log.record_mise_bootstrapped(Path::new("/home/u/.mysh/bin/mise")).unwrap();
        log.record_package_installed("widget@1.0").unwrap();

        assert_eq!(
            log.entries().unwrap(),
            vec![
                LogEntry::Created(PathBuf::from("bashrc")),
                LogEntry::Overwritten {
                    relative: PathBuf::from("gitconfig"),
                    backup: PathBuf::from(".mysh/backups/gitconfig"),
                },
                LogEntry::OverlayTouched(PathBuf::from(".claude.json")),
                LogEntry::MiseBootstrapped(PathBuf::from("/home/u/.mysh/bin/mise")),
                LogEntry::PackageInstalled("widget@1.0".to_string()),
            ]
        );
    }

    #[test]
    fn overlay_touched_makes_the_path_managed() {
        let dir = std::env::temp_dir().join(format!("mysh-log-test-overlay-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let log = AppLog::open(&dir);
        assert!(!log.is_managed(Path::new(".claude.json")).unwrap());
        log.record_overlay_touched(Path::new(".claude.json")).unwrap();
        assert!(log.is_managed(Path::new(".claude.json")).unwrap());
    }
}
