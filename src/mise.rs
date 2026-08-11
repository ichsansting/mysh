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
/// lives, and where lazy-package shims land — real files in Source, identity-copied
/// here by ordinary Apply like any other tracked file (see ADR-0006) — so invoking a
/// lazy tool's plain command name resolves to its shim.
pub fn bin_dir(target: &Path) -> PathBuf {
    target.join(".mysh/bin")
}

/// Where mysh's own bootstrap installs `mise`, if no system-wide copy is found.
fn owned_mise_bin(target: &Path) -> PathBuf {
    bin_dir(target).join("mise")
}

/// The already-usable `mise` binary, if one exists: a pre-existing system-wide `mise`
/// (never duplicated), or one mysh bootstrapped for this `target` in an earlier run.
/// `None` means `ensure_installed` actually needs to install it. `pub(crate)` (rather
/// than folded into `ensure_installed`) because `add`'s eager path needs this exact
/// read-only resolution too: `add` never touches `Target` (see `add::package_add`), so
/// it must never fall through to bootstrapping — only use `mise` if one is already
/// resolvable.
pub(crate) fn resolved_mise_bin(target: &Path) -> Option<PathBuf> {
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

/// Relative path of `data_dir`, `$HOME`-joined. A separate constant (not just inlined
/// into `data_dir`) because `package::shim_script` needs the same subpath rendered as
/// literal `$HOME`-relative shell text, not a `PathBuf` resolved against a concrete
/// `target` — sharing this constant is what keeps the two from drifting apart. `pub`
/// (matching `data_dir` itself) so `tests/bootstrap_integration.rs` can assert
/// `bootstrap.sh`'s independent copy — necessarily hardcoded there, since it runs
/// before the `mysh` binary exists to share this with — still matches.
pub const DATA_DIR_REL: &str = ".mysh/mise";

/// Where packages get installed: an isolated, mysh-owned prefix under `target`'s state
/// dir (not `mise`'s system default), so Teardown can remove every installed package by
/// deleting this one directory.
pub fn data_dir(target: &Path) -> PathBuf {
    target.join(DATA_DIR_REL)
}

/// Where mise's own config lives once rendered by ordinary Apply from
/// `profile/.config/mise/config.toml` in Source — under `root` for any root (`target`
/// during `apply`, `source` for `add`'s eager path, which only ever edits Source).
/// Eager packages are declared in this file's `[tools]` table now, not in a
/// mysh-owned declarations file (see ADR-0006).
pub fn config_path(root: &Path) -> PathBuf {
    root.join(".config/mise/config.toml")
}

/// Marks `config_path` trusted so `mise` will parse it without prompting — required
/// for any config file outside its own default lookup locations (found live: even
/// `config set`/`config get`/`install` against an explicit `--file` refuse to parse an
/// untrusted one, e.g. `mise ERROR Config files in ... are not trusted`). Safe for
/// mysh to do unconditionally here: this is always a path mysh itself renders from
/// Source, content the user already authored and asked mysh to manage — no different
/// in trust level from every other file mysh already renders and sources unsandboxed.
fn trust(mise_bin: &Path, config_path: &Path) -> Result<(), String> {
    let status = Command::new(mise_bin)
        .arg("trust")
        .arg(config_path)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("failed to trust config file {}", config_path.display()));
    }
    Ok(())
}

/// Installs every package declared in `target`'s rendered `config.toml` `[tools]`
/// table in one call, scoped to the same isolated data/config dirs eager installs and
/// lazy shims both rely on — so resolution is deterministic regardless of the ambient
/// environment (e.g. in tests, where `target` isn't the real `$HOME`).
pub fn install_declared(mise_bin: &Path, target: &Path) -> Result<(), String> {
    trust(mise_bin, &config_path(target))?;
    let status = Command::new(mise_bin)
        .arg("install")
        .env("MISE_DATA_DIR", data_dir(target))
        .env("MISE_CONFIG_DIR", target.join(".config/mise"))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("failed to install declared packages".to_string());
    }
    Ok(())
}

/// The specifiers declared in `config_path`'s `[tools]` table, reassembled as
/// `<name>@<version>` (matching the specifier shape this codebase logs everywhere
/// else, e.g. `widget@1.0`) — `install_declared` installs all of them in one call, so
/// this is the only way to know which ones to individually record in the Application
/// Log. An absent `[tools]` table (no eager packages declared at all) is empty, not an
/// error.
pub fn declared_tools(mise_bin: &Path, config_path: &Path) -> Result<Vec<String>, String> {
    if !config_path.is_file() {
        return Ok(Vec::new());
    }
    trust(mise_bin, config_path)?;
    let output = Command::new(mise_bin)
        .args(["config", "get", "tools", "--file"])
        .arg(config_path)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let (name, version) = line.split_once('=')?;
            Some(format!("{}@{}", name.trim().trim_matches('"'), version.trim().trim_matches('"')))
        })
        .collect())
}

/// Declares `name` (a bare or backend-prefixed specifier, no `@version` suffix — split
/// that out before calling) at `version` in `config_path`'s `[tools]` table via `mise
/// config set`, creating the file first if it doesn't exist yet. Never installs
/// anything — confirmed live that `config set` only edits the file — so it's safe for
/// `add` to call without violating "`add` never touches `Target`".
pub fn declare_tool(mise_bin: &Path, config_path: &Path, name: &str, version: &str) -> Result<(), String> {
    if !config_path.is_file() {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(config_path, "").map_err(|e| e.to_string())?;
    }
    trust(mise_bin, config_path)?;
    let status = Command::new(mise_bin)
        .arg("config")
        .arg("set")
        .arg(format!("tools.{name}"))
        .arg(version)
        .arg("--file")
        .arg(config_path)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("failed to declare package {name}"));
    }
    Ok(())
}
