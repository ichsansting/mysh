use crate::log::AppLog;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The official installer one-liner (https://mise.jdx.dev/installing-mise.html), run via
/// `sh` when `mise` isn't already found on `PATH`.
const INSTALL_CMD: &str = "curl -fsSL https://mise.run | sh";

/// Whether `mise` is discoverable on `PATH`.
pub fn is_installed() -> bool {
    Command::new("mise").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Installs `mise` via its official installer if it isn't already present, recording the
/// bootstrap in the Application Log. No-op (and no log entry) when `mise` is already on
/// `PATH`.
pub fn ensure_installed(log: &AppLog) -> Result<(), String> {
    if is_installed() {
        return Ok(());
    }
    let status =
        Command::new("sh").arg("-c").arg(INSTALL_CMD).status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("failed to install mise".to_string());
    }
    log.record_mise_bootstrapped().map_err(|e| e.to_string())
}

/// Where packages get installed: an isolated, mysh-owned prefix under `target`'s state
/// dir (not `mise`'s system default), so Teardown can remove every installed package by
/// deleting this one directory.
pub fn data_dir(target: &Path) -> PathBuf {
    target.join(".mysh/mise")
}

/// Installs `specifier` via `mise install`, scoped to `data_dir`.
pub fn install(target: &Path, specifier: &str) -> Result<(), String> {
    let status = Command::new("mise")
        .arg("install")
        .arg(specifier)
        .env("MISE_DATA_DIR", data_dir(target))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("failed to install package {specifier}"));
    }
    Ok(())
}
