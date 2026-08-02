use crate::log::AppLog;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The official installer one-liner (https://mise.jdx.dev/installing-mise.html), run via
/// `sh` when `mise` isn't already found. It only ever writes a single binary to the path
/// named by `MISE_INSTALL_PATH` (default `~/.local/bin/mise`) — no `PATH`/rc-file edits,
/// no shims — so forcing that env var to a mysh-owned location is enough to keep the
/// whole thing inside `target`'s state dir, deterministic for `teardown` to reverse.
const INSTALL_CMD: &str = "curl -fsSL https://mise.run | sh";

/// Whether a system-wide `mise` is already discoverable on `PATH`.
fn system_mise_present() -> bool {
    Command::new("mise").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// mysh's isolated, `PATH`-resident prefix: where its own bootstrapped `mise` binary
/// lives, and where lazy-package shims (see `package::apply`) are generated so invoking
/// a lazy tool's plain command name resolves to its shim.
pub fn bin_dir(target: &Path) -> PathBuf {
    target.join(".mysh/bin")
}

/// Where mysh's own bootstrap installs `mise`, if no system-wide copy is found.
fn owned_mise_bin(target: &Path) -> PathBuf {
    bin_dir(target).join("mise")
}

/// The already-usable `mise` binary, if one exists: a pre-existing system-wide `mise`
/// (never duplicated), or one mysh bootstrapped for this `target` in an earlier run.
/// `None` means `ensure_installed` actually needs to install it.
fn resolved_mise_bin(target: &Path) -> Option<PathBuf> {
    if system_mise_present() {
        return Some(PathBuf::from("mise"));
    }
    let owned = owned_mise_bin(target);
    owned.is_file().then_some(owned)
}

/// Ensures `mise` is usable, bootstrapping it via the official installer into a
/// deterministic, mysh-owned path only when neither a system-wide nor a
/// previously-bootstrapped copy already exists — and only then recording the install in
/// the Application Log. Returns the resolved binary to invoke, so `mise`'s presence is
/// checked once per `apply` rather than once per package installed.
pub fn ensure_installed(target: &Path, log: &AppLog) -> Result<PathBuf, String> {
    if let Some(bin) = resolved_mise_bin(target) {
        return Ok(bin);
    }
    let install_path = owned_mise_bin(target);
    let status = Command::new("sh")
        .arg("-c")
        .arg(INSTALL_CMD)
        .env("MISE_INSTALL_PATH", &install_path)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("failed to install mise".to_string());
    }
    log.record_mise_bootstrapped(&install_path).map_err(|e| e.to_string())?;
    Ok(install_path)
}

/// Where packages get installed: an isolated, mysh-owned prefix under `target`'s state
/// dir (not `mise`'s system default), so Teardown can remove every installed package by
/// deleting this one directory.
pub fn data_dir(target: &Path) -> PathBuf {
    target.join(".mysh/mise")
}

/// Installs `specifier` via `mise install`, invoking the binary `ensure_installed`
/// already resolved, scoped to `data_dir`.
pub fn install(mise_bin: &Path, target: &Path, specifier: &str) -> Result<(), String> {
    let status = Command::new(mise_bin)
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
