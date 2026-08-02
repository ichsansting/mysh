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

    /// Records that this device didn't already have `mise` and mysh installed it.
    pub fn record_mise_bootstrapped(&self) -> io::Result<()> {
        self.append("mise-bootstrapped\n")
    }

    /// Records that mysh installed `specifier` via `mise` — what lets `teardown` later
    /// uninstall it.
    pub fn record_package_installed(&self, specifier: &str) -> io::Result<()> {
        self.append(&format!("package-installed\t{specifier}\n"))
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
