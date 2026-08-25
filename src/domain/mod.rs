pub mod drift;
pub mod fingerprint;
pub mod fragment;
pub mod glob;
pub mod log;
pub mod overlay;
pub mod package;
pub mod picker;
pub mod render;

/// mysh's per-device footprint, all under one target-relative directory.
pub const MYSH_DIR_REL: &str = ".mysh";
/// The mysh/mise binaries plus every package shim — the only PATH entry mysh adds.
pub const BIN_DIR_REL: &str = ".mysh/bin";
/// mise's isolated data prefix; deleted wholesale by teardown.
pub const MISE_DATA_DIR_REL: &str = ".mysh/mise";
/// The Application Log.
pub const LOG_REL: &str = ".mysh/log";
/// Backups of pre-existing files overwritten on first apply.
pub const BACKUP_DIR_REL: &str = ".mysh/backups";
/// The Fingerprint cache — disposable, unlike the Application Log.
pub const FINGERPRINTS_REL: &str = ".mysh/fingerprints";
