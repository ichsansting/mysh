use crate::config::{take_flag, take_switch, Config};
use crate::domain::render::{self, TRACK_MARKER};
use crate::domain::{glob, package, BIN_DIR_REL};
use crate::error::{Error, IoCtx, Result};
use crate::infra::{crypto, fsx, prompt};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Add: start tracking a new path in Source, dispatching on what exists on
/// disk under Target — a file is copied in (encrypted with --secret), a
/// directory becomes a .track-marked mirror after confirmation, and a path
/// that exists nowhere is a package specifier (lazy by default). Add only
/// ever touches Source: never Target, never git history.
pub fn run(config: &Config, leftover: Vec<String>, input: &mut dyn BufRead) -> Result<String> {
    let mut args = leftover;
    let secret = take_switch(&mut args, "--secret");
    let eager = take_switch(&mut args, "--eager");
    take_switch(&mut args, "--lazy"); // the default; accepted for symmetry
    let bin = take_flag(&mut args, "--bin")?;
    let mut ignores = Vec::new();
    while let Some(pattern) = take_flag(&mut args, "--ignore")? {
        ignores.push(pattern);
    }
    let [argument] = args.as_slice() else {
        return Err(Error::Usage("add expects exactly one <path|specifier>".to_string()));
    };

    let live = config.target_dir.join(argument);
    if live.is_file() {
        add_file(config, Path::new(argument), secret)
    } else if live.is_dir() {
        if secret {
            return Err(Error::Usage("--secret applies to a single file, not a directory".into()));
        }
        add_folder(config, Path::new(argument), &ignores, input)
    } else {
        if secret {
            return Err(Error::Usage("--secret applies to a file, not a package specifier".into()));
        }
        add_package(config, argument, bin.as_deref(), eager)
    }
}

fn add_file(config: &Config, rel: &Path, secret: bool) -> Result<String> {
    let plan = render::enumerate(&config.source_dir)?;
    if plan.units.iter().any(|u| u.target_rel == rel) {
        return Err(Error::Rejected(format!(
            "{}: already tracked in Source; edit it live and use save",
            rel.display()
        )));
    }
    let live = config.target_dir.join(rel);
    let content = fs::read(&live).at("read", &live)?;
    if secret {
        let passphrase = prompt::new_secret_passphrase(&config.passphrase)?;
        let mut source_rel = rel.as_os_str().to_owned();
        source_rel.push(".age");
        let dest = config.source_dir.join(&source_rel);
        let envelope = crypto::encrypt(&content, &passphrase, &dest)?;
        fsx::write_if_changed(&dest, &envelope, None)?;
    } else {
        fsx::write_if_changed(&config.source_dir.join(rel), &content, None)?;
    }
    Ok("added\n".to_string())
}

fn add_folder(
    config: &Config,
    rel: &Path,
    ignores: &[String],
    input: &mut dyn BufRead,
) -> Result<String> {
    let plan = render::enumerate(&config.source_dir)?;
    if plan.tracked_dirs.iter().any(|t| t.rel == rel) {
        return Err(Error::Rejected(format!(
            "{}: directory already tracked in Source",
            rel.display()
        )));
    }
    let live_root = config.target_dir.join(rel);
    let matched: Vec<PathBuf> = fsx::walk(&live_root, &|_| true)?
        .into_iter()
        .filter(|file| !glob::is_ignored(file, ignores))
        .collect();

    let mut summary = String::new();
    for file in &matched {
        summary.push_str(&format!("{}\n", rel.join(file).display()));
    }
    // Nothing is written before this confirmation — a decline leaves Source
    // byte-for-byte untouched, empty parent directories included.
    if !prompt::confirm(input, &summary)? {
        return Ok("aborted\n".to_string());
    }

    let source_root = config.source_dir.join(rel);
    fsx::write_if_changed(&source_root.join(TRACK_MARKER), ignores.join("\n").as_bytes(), None)?;
    for file in &matched {
        let live = live_root.join(file);
        let content = fs::read(&live).at("read", &live)?;
        fsx::write_if_changed(&source_root.join(file), &content, None)?;
    }
    Ok("added\n".to_string())
}

fn add_package(config: &Config, specifier: &str, bin: Option<&str>, eager: bool) -> Result<String> {
    let bin_name =
        bin.map(str::to_string).unwrap_or_else(|| package::default_bin_name(specifier));
    let shim_rel = Path::new(BIN_DIR_REL).join(&bin_name);
    let dest = config.source_dir.join(&shim_rel);
    if dest.exists() {
        return Err(Error::Rejected(format!(
            "{}: already declared; edit or remove the shim file",
            shim_rel.display()
        )));
    }
    let shim = package::shim_script(specifier, &bin_name, eager);
    fsx::write_if_changed(&dest, shim.as_bytes(), Some(0o755))?;
    Ok("added\n".to_string())
}
