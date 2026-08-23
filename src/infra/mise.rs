use crate::config::Config;
use crate::domain::log::{AppLog, LogEntry};
use crate::domain::{BIN_DIR_REL, MISE_DATA_DIR_REL};
use crate::error::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

/// The mise binary to use, bootstrapping if necessary. Resolution order:
/// a `mise` already on PATH (the host's — never shadowed, ADR-0007), then the
/// mysh-owned one at `.mysh/bin/mise`, else self-bootstrap via mise's installer
/// into the isolated prefix — recorded in the Application Log (ADR-0005).
pub fn ensure_installed(config: &Config, log: &AppLog) -> Result<PathBuf> {
    if Command::new("mise")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok(PathBuf::from("mise"));
    }

    let owned = config.target_dir.join(BIN_DIR_REL).join("mise");
    if owned.is_file() {
        return Ok(owned);
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://mise.run | sh")
        .env("MISE_INSTALL_PATH", &owned)
        .env("MISE_DATA_DIR", config.target_dir.join(MISE_DATA_DIR_REL))
        .status()
        .map_err(|e| Error::Subprocess { program: "curl", detail: e.to_string() })?;
    if !status.success() || !owned.is_file() {
        return Err(Error::Subprocess {
            program: "curl",
            detail: "mise bootstrap failed".to_string(),
        });
    }
    log.record(&LogEntry::MiseBootstrapped { path: owned.clone() })?;
    Ok(owned)
}

/// One batched `mise install <spec>...` — mise parallelizes the downloads,
/// which is the entire point of eager over lazy (ADR-0007).
pub fn install(mise_bin: &PathBuf, specifiers: &[String], config: &Config) -> Result<()> {
    let output = Command::new(mise_bin)
        .arg("install")
        .args(specifiers)
        .env("MISE_DATA_DIR", config.target_dir.join(MISE_DATA_DIR_REL))
        .output()
        .map_err(|e| Error::Subprocess { program: "mise", detail: e.to_string() })?;
    if !output.status.success() {
        return Err(Error::Subprocess {
            program: "mise",
            detail: format!(
                "install failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(())
}
