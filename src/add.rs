use crate::apply::walk_files;
use crate::confirm::confirm;
use crate::diff::matches_ignore;
use crate::mise;
use crate::package;
use crate::secret;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// `add`'s own flags, parsed out of the args every other subcommand also shares
/// (`--source-dir`/`--target-dir`/`--remote-url`/`--passphrase`, forwarded untouched to
/// `Config::resolve`).
pub struct AddFlags {
    pub path: String,
    pub secret: bool,
    pub eager: Option<bool>,
    pub bin_name: Option<String>,
    pub ignore: Vec<String>,
}

/// Splits `add`'s own flags and its one positional `<path>` argument out of `args`,
/// returning whatever's left (the shared config flags) for `Config::resolve`.
pub fn parse_flags(args: &[String]) -> Result<(AddFlags, Vec<String>), String> {
    let mut path = None;
    let mut secret = false;
    let mut eager = None;
    let mut bin_name = None;
    let mut ignore = Vec::new();
    let mut forwarded = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            if path.is_some() {
                return Err(format!("unexpected extra argument: {arg}"));
            }
            path = Some(arg.clone());
            i += 1;
            continue;
        }

        let (name, inline_value) = match arg.split_once('=') {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (arg.clone(), None),
        };
        match name.as_str() {
            "--secret" => {
                secret = true;
                i += 1;
            }
            "--eager" => {
                eager = Some(true);
                i += 1;
            }
            "--lazy" => {
                eager = Some(false);
                i += 1;
            }
            "--bin" => {
                let (value, consumed) = take_value(&inline_value, args, i, &name)?;
                bin_name = Some(value);
                i += consumed;
            }
            "--ignore" => {
                let (value, consumed) = take_value(&inline_value, args, i, &name)?;
                ignore.push(value);
                i += consumed;
            }
            "--source-dir" | "--target-dir" | "--remote-url" | "--passphrase" => {
                let (value, consumed) = take_value(&inline_value, args, i, &name)?;
                forwarded.push(name);
                forwarded.push(value);
                i += consumed;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let path = path.ok_or("add requires a path or package specifier argument")?;
    Ok((AddFlags { path, secret, eager, bin_name, ignore }, forwarded))
}

fn take_value(
    inline: &Option<String>,
    args: &[String],
    i: usize,
    name: &str,
) -> Result<(String, usize), String> {
    match inline {
        Some(v) => Ok((v.clone(), 1)),
        None => {
            let v = args.get(i + 1).ok_or_else(|| format!("{name} requires a value"))?;
            Ok((v.clone(), 2))
        }
    }
}

/// Starts tracking a new path in `Source`. Dispatches purely on what `flags.path`
/// resolves to on disk under `target`: a file → file-add, a directory → folder-add, a
/// path that doesn't exist at all → package-add (treating `flags.path` as a `mise`
/// specifier, not a filesystem path).
pub fn add(
    source: &Path,
    target: &Path,
    flags: AddFlags,
    input: &mut dyn BufRead,
    passphrase: Option<String>,
) -> Result<String, String> {
    let resolved = resolve_path(target, &flags.path);

    if !resolved.exists() {
        if flags.secret {
            return Err("--secret cannot be combined with a package specifier".to_string());
        }
        return package_add(
            source,
            target,
            &flags.path,
            flags.eager.unwrap_or(false),
            flags.bin_name.as_deref(),
        );
    }

    let relative = resolved
        .strip_prefix(target)
        .map_err(|_| format!("'{}' is not under target dir '{}'", resolved.display(), target.display()))?
        .to_path_buf();

    if resolved.is_dir() {
        if flags.secret {
            return Err("--secret cannot be combined with a directory".to_string());
        }
        folder_add(source, target, &relative, &flags.ignore, input)
    } else {
        file_add(source, target, &relative, flags.secret, passphrase)
    }
}

/// Resolves `arg` (as given on the command line) to an absolute path: `~`-prefixed
/// expands against `target` (which stands in for `$HOME`), an absolute path is used
/// as-is, anything else is taken relative to `target`.
fn resolve_path(target: &Path, arg: &str) -> PathBuf {
    if let Some(rest) = arg.strip_prefix("~/") {
        return target.join(rest);
    }
    if arg == "~" {
        return target.to_path_buf();
    }
    let p = Path::new(arg);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        target.join(p)
    }
}

/// `relative` with `.age` appended, e.g. `ssh/id_rsa` -> `ssh/id_rsa.age`.
fn with_age_suffix(relative: &Path) -> PathBuf {
    let mut name = relative.as_os_str().to_os_string();
    name.push(".age");
    PathBuf::from(name)
}

/// The `Source`-relative path `relative` is already tracked under, plain or secret,
/// if any — a plain add and a `--secret` add of the same `Target` path must not be
/// able to both go through and collide on the same rendered `Target` file.
fn existing_source_path(source: &Path, relative: &Path) -> Option<PathBuf> {
    if source.join(relative).is_file() {
        return Some(relative.to_path_buf());
    }
    let secret_path = with_age_suffix(relative);
    if source.join(&secret_path).is_file() {
        return Some(secret_path);
    }
    None
}

/// The first (topmost) path under `source`, walking down `relative` component by
/// component, that doesn't exist yet — i.e. the directory a `create_dir_all(relative)`
/// would actually create from. `None` if `relative` already exists in full.
fn first_missing_ancestor(source: &Path, relative: &Path) -> Option<PathBuf> {
    let mut built = PathBuf::new();
    for component in relative.components() {
        built.push(component);
        let candidate = source.join(&built);
        if !candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn file_add(
    source: &Path,
    target: &Path,
    relative: &Path,
    secret_flag: bool,
    passphrase: Option<String>,
) -> Result<String, String> {
    if let Some(existing) = existing_source_path(source, relative) {
        return Err(format!(
            "'{}' is already tracked in Source (as '{}'); use `save` to capture changes to it",
            relative.display(),
            existing.display()
        ));
    }

    let content = fs::read(target.join(relative)).map_err(|e| e.to_string())?;
    let (source_relative, to_write) = if secret_flag {
        let passphrase = secret::new_secret_passphrase(&passphrase)?;
        (with_age_suffix(relative), secret::encrypt(&content, &passphrase)?)
    } else {
        (relative.to_path_buf(), content)
    };

    let dest = source.join(&source_relative);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&dest, to_write).map_err(|e| e.to_string())?;
    Ok(format!("added '{}'\n", relative.display()))
}

fn folder_add(
    source: &Path,
    target: &Path,
    relative: &Path,
    ignore: &[String],
    input: &mut dyn BufRead,
) -> Result<String, String> {
    let dest_dir = source.join(relative);
    let track_marker = dest_dir.join(".track");
    if track_marker.is_file() {
        return Err(format!(
            "'{}' is already tracked as a directory in Source; use `save` to capture changes to it",
            relative.display()
        ));
    }

    // The topmost ancestor `create_dir_all` below is about to create, if any — e.g. for
    // relative == "config/nvim" with neither existing yet, that's "config", not
    // "config/nvim". Removing this (rather than just `dest_dir`) on decline is what
    // makes a decline leave Source with zero new directory entries, not just an empty
    // leaf dir.
    let created_root = first_missing_ancestor(source, relative);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let track_content = if ignore.is_empty() { String::new() } else { format!("{}\n", ignore.join("\n")) };
    fs::write(&track_marker, track_content).map_err(|e| e.to_string())?;

    let target_dir = target.join(relative);
    let files = matching_files(&target_dir, ignore).map_err(|e| e.to_string())?;

    for f in &files {
        println!("{}", f.display());
    }
    if !confirm(input, "copy these files into Source? [y/N] ")? {
        match created_root {
            Some(root) => fs::remove_dir_all(root).map_err(|e| e.to_string())?,
            None => fs::remove_file(&track_marker).map_err(|e| e.to_string())?,
        }
        return Ok("aborted\n".to_string());
    }

    for f in &files {
        let dest_file = dest_dir.join(f);
        if let Some(parent) = dest_file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(target_dir.join(f), &dest_file).map_err(|e| e.to_string())?;
    }
    Ok(format!("added '{}' ({} files)\n", relative.display(), files.len()))
}

/// Files under `target_dir`, relative to it, that don't match any `ignore` pattern —
/// same recursive walk (skipping `.git`/`.mysh`/fragment dirs) `apply` already uses.
fn matching_files(target_dir: &Path, ignore: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = walk_files(target_dir)?
        .into_iter()
        .map(|p| p.strip_prefix(target_dir).expect("entry is under target_dir").to_path_buf())
        .filter(|relative| !matches_ignore(ignore, relative))
        .collect();
    files.sort();
    Ok(files)
}

/// Dispatches a package specifier to the lazy or eager path — the two no longer share
/// a mechanism (see ADR-0006), so `--bin` (a lazy-only concept: it names the shim file
/// `add` writes) is rejected outright for `--eager`, where `mise` resolves the binary
/// name itself.
fn package_add(
    source: &Path,
    target: &Path,
    specifier: &str,
    eager: bool,
    bin_name: Option<&str>,
) -> Result<String, String> {
    if eager {
        if bin_name.is_some() {
            return Err("--bin has no effect with --eager; mise resolves the binary name itself".to_string());
        }
        eager_add(source, target, specifier)
    } else {
        lazy_add(source, specifier, bin_name)
    }
}

/// Writes a real, portable shim file into Source at the mirrored path a lazy package's
/// shim renders to (`.mysh/bin/<bin_name>`, see ADR-0006) — `add` never touches
/// `Target`, so this only ever creates the file in Source; the next `apply` identity-
/// copies it through like any other tracked file.
fn lazy_add(source: &Path, specifier: &str, bin_name: Option<&str>) -> Result<String, String> {
    let bin_name = bin_name.map(str::to_string).unwrap_or_else(|| package::default_bin_name(specifier));
    let dest = mise::bin_dir(source).join(&bin_name);
    if dest.exists() {
        return Err(format!(
            "'{bin_name}' is already declared as a lazy package in Source; edit '{}' directly instead",
            dest.strip_prefix(source).unwrap_or(&dest).display()
        ));
    }
    if lazy_specifier_already_declared(source, specifier) {
        return Err(format!("'{specifier}' is already declared as a lazy package in Source"));
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let shim = package::shim_script(specifier, &bin_name);
    package::write_if_changed_executable(&dest, &shim).map_err(|e| e.to_string())?;
    Ok(format!("added '{specifier}' (lazy, as '{bin_name}')\n"))
}

/// Whether any existing lazy shim file in Source already execs `specifier` — a
/// specifier-level duplicate check (not just a `bin_name` collision), matching this
/// codebase's existing "one CLI-driven declaration per specifier" intent. A specifier
/// deliberately exposed under a second `bin_name` (e.g. `rust@stable` as both `cargo`
/// and `rustc`) still works, just not through this CLI — hand-add the second file
/// directly, same as today.
fn lazy_specifier_already_declared(source: &Path, specifier: &str) -> bool {
    let needle = format!("x {specifier} -- ");
    fs::read_dir(mise::bin_dir(source))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| fs::read_to_string(entry.path()).map(|c| c.contains(&needle)).unwrap_or(false))
}

/// Declares `specifier` in Source's `config.toml` `[tools]` table via `mise config
/// set` (see `mise::declare_tool`). Requires an already-resolvable `mise` — system-wide
/// or previously bootstrapped by an earlier `apply` — rather than bootstrapping one
/// itself, since bootstrapping writes into `Target` and `add` never touches `Target`.
fn eager_add(source: &Path, target: &Path, specifier: &str) -> Result<String, String> {
    let mise_bin = mise::resolved_mise_bin(target).ok_or_else(|| {
        "no `mise` found (system-wide, or previously bootstrapped by `apply`) — run `apply` once \
         first; `add` never installs anything into Target"
            .to_string()
    })?;

    let (name, version) = specifier.rsplit_once('@').unwrap_or((specifier, "latest"));
    let config_path = mise::config_path(source);
    let already_declared = mise::declared_tools(&mise_bin, &config_path)?
        .iter()
        .any(|declared| declared.rsplit_once('@').map_or(declared.as_str(), |(n, _)| n) == name);
    if already_declared {
        return Err(format!("'{name}' is already declared as an eager package in Source's config.toml"));
    }

    mise::declare_tool(&mise_bin, &config_path, name, version)?;
    Ok(format!("added '{specifier}' (eager)\n"))
}
