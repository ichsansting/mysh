use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Every failure mysh can report. One small enum instead of anyhow/thiserror:
/// the dependency floor matters here (ADR-0001), and six variants cover the domain.
#[derive(Debug)]
pub enum Error {
    /// A filesystem operation failed; `op` is a short verb ("read", "write", "walk").
    Io { op: &'static str, path: PathBuf, source: io::Error },
    /// A subprocess (git, mise, curl) failed or couldn't be spawned.
    Subprocess { program: &'static str, detail: String },
    /// Encryption/decryption failed — wrong passphrase and corrupt file are
    /// indistinguishable behind the AEAD tag, so the message never claims to know which.
    Crypto { path: PathBuf, detail: String },
    /// An Overlay's declared keys or its Target's content couldn't be parsed/merged.
    Overlay { path: PathBuf, detail: String },
    /// Bad flags or arguments.
    Usage(String),
    /// A well-formed request mysh deliberately refuses (save on a derived Target,
    /// add on an already-tracked path).
    Rejected(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { op, path, source } => {
                write!(f, "failed to {op} {}: {source}", path.display())
            }
            Error::Subprocess { program, detail } => write!(f, "{program}: {detail}"),
            Error::Crypto { path, detail } => write!(f, "{}: {detail}", path.display()),
            Error::Overlay { path, detail } => write!(f, "{}: {detail}", path.display()),
            Error::Usage(detail) | Error::Rejected(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// Attaches the operation and path to a raw `io::Result`, so no I/O error ever
/// surfaces without saying what was being done to which file.
pub trait IoCtx<T> {
    fn at(self, op: &'static str, path: &Path) -> Result<T>;
}

impl<T> IoCtx<T> for io::Result<T> {
    fn at(self, op: &'static str, path: &Path) -> Result<T> {
        self.map_err(|source| Error::Io { op, path: path.to_path_buf(), source })
    }
}
