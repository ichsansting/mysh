use crate::log::AppLog;
use crate::mise;
use std::fs;
use std::io;
use std::path::Path;

/// The package-declaration file at the root of `Source`. mysh metadata, not a dotfile —
/// `apply::render` and `diff::diff` both exclude this exact top-level path from ordinary
/// per-path rendering/drift so it's never mirrored into `Target`. Only the top-level
/// path is special-cased: a same-named file nested in some subdirectory is just an
/// ordinary file mysh doesn't otherwise treat specially.
pub const DECLARATIONS_FILE: &str = ".packages";

/// A CLI-tool package mysh installs via `mise` (see ADR-0005 for the scope decision: CLI
/// tools/binaries only, not full system packages).
#[derive(Debug, PartialEq)]
pub struct Package {
    /// A `mise`-compatible specifier: bare (`go@latest`) or backend-prefixed
    /// (`github:owner/repo@version`, `npm:pkg@version`, ...).
    pub specifier: String,
    /// The name the generated shim is exposed under on `PATH`. Defaults from `specifier`
    /// when not explicitly declared.
    pub bin_name: String,
    /// The binary name actually passed to `mise x <specifier> -- <name>` — i.e. the name
    /// the specifier's install really produces. Defaults to `bin_name` when not separately
    /// declared, which is correct whenever the two coincide; only needs to differ when the
    /// exposed shim name is deliberately not the tool's own binary name (e.g. installing
    /// under an alias to avoid colliding with a differently-sourced package of the same
    /// tool).
    pub invoke_name: String,
    /// Whether the install cost is paid immediately during `apply` (`true`) or deferred to
    /// first invocation (`false`, see issue 09). Both classifications get a generated shim
    /// on `PATH` after `apply` (see issue 13) — this only changes *when* `mise install`
    /// runs, not whether the tool ends up reachable under its plain binary name.
    pub eager: bool,
}

/// Loads the packages declared in `source`'s `.packages` file. A missing file means no
/// packages declared — an empty list, not an error.
pub fn load(source: &Path) -> Result<Vec<Package>, String> {
    let text = match fs::read_to_string(source.join(DECLARATIONS_FILE)) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    text.lines().filter(|line| !line.trim().is_empty()).map(parse_line).collect()
}

/// One package per line, tab-separated:
/// `<specifier>\t<eager|lazy>[\t<bin_name>[\t<invoke_name>]]`.
fn parse_line(line: &str) -> Result<Package, String> {
    let mut fields = line.split('\t');
    let specifier = fields
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("malformed package declaration (missing specifier): {line:?}"))?
        .to_string();
    let classification = fields.next().ok_or_else(|| {
        format!("malformed package declaration (missing eager/lazy): {line:?}")
    })?;
    let eager = match classification {
        "eager" => true,
        "lazy" => false,
        other => {
            return Err(format!("unknown package classification {other:?} in line: {line:?}"));
        }
    };
    let bin_name = match fields.next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => default_bin_name(&specifier),
    };
    let invoke_name = match fields.next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => bin_name.clone(),
    };
    Ok(Package { specifier, bin_name, invoke_name, eager })
}

/// The binary name a specifier produces when not explicitly overridden: any backend
/// prefix (`github:`, `npm:`, ...) and version pin (`@...`) stripped, then the last
/// `/`-separated segment — e.g. `github:elio-fm/elio@latest` -> `elio`, `go@latest` ->
/// `go`.
fn default_bin_name(specifier: &str) -> String {
    let rest = specifier.split_once(':').map_or(specifier, |(_backend, rest)| rest);
    let rest = rest.split('@').next().unwrap_or(rest);
    rest.rsplit('/').next().unwrap_or(rest).to_string()
}

/// Installs every eager package immediately and generates a shim for every declared
/// package, eager and lazy alike, self-bootstrapping `mise` first if it isn't already
/// present. A no-op — `mise` is never touched — only when no packages of either kind are
/// declared at all: a lazy-only device still resolves/bootstraps `mise` up front (without
/// installing the lazy tools themselves) so each generated shim has a concrete `mise` to
/// invoke on first real use, rather than hoping one turns up on `PATH` later. Every
/// package gets a shim (not just lazy ones) because `mise install` alone doesn't put
/// anything on `PATH` — mise's own shim/activation mechanism depends on a config file mysh
/// doesn't write (see issue 13) — so the shim is the only thing that makes an eager
/// package's plain binary name resolvable after `apply`, same as it already does for lazy.
pub fn apply(source: &Path, target: &Path, log: &AppLog) -> Result<(), String> {
    let packages = load(source)?;
    if packages.is_empty() {
        return Ok(());
    }
    let mise_bin = mise::ensure_installed(target, log)?;

    for package in packages.iter().filter(|p| p.eager) {
        mise::install(&mise_bin, target, &package.specifier)?;
        log.record_package_installed(&package.specifier).map_err(|e| e.to_string())?;
    }

    let bin_dir = mise::bin_dir(target);
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    for package in &packages {
        let shim = shim_script(&mise_bin, target, &package.specifier, &package.invoke_name);
        write_if_changed_executable(&bin_dir.join(&package.bin_name), &shim)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A lazy package's shim: installs (on first use) and execs the real tool via
/// `mise x <specifier> -- <invoke_name> "$@"`, using the exact `mise` binary `apply` already
/// resolved (bare `mise` when system-wide, so future `PATH` lookups keep working; an
/// absolute owned path when mysh bootstrapped it, so the shim doesn't depend on that path
/// being on `PATH` by the time it's actually run) and scoped to the same isolated
/// `MISE_DATA_DIR` eager installs use.
fn shim_script(mise_bin: &Path, target: &Path, specifier: &str, invoke_name: &str) -> String {
    format!(
        "#!/bin/sh\nexport MISE_DATA_DIR=\"{}\"\nexec \"{}\" x {specifier} -- {invoke_name} \"$@\"\n",
        mise::data_dir(target).display(),
        mise_bin.display(),
    )
}

/// Writes an executable file at `path`, matching the idempotence every other render path
/// in this codebase already has (`apply::write_if_changed`, `secret::write_restricted`):
/// a no-op when `content` already matches what's on disk.
#[cfg(unix)]
fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if fs::read_to_string(path).map(|existing| existing == content).unwrap_or(false) {
        return Ok(());
    }
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(path)?
        .write_all(content.as_bytes())
}

#[cfg(not(unix))]
fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bin_name_handles_bare_backend_prefixed_and_versioned_specifiers() {
        assert_eq!(default_bin_name("go@latest"), "go");
        assert_eq!(default_bin_name("github:elio-fm/elio@latest"), "elio");
        assert_eq!(default_bin_name("npm:eslint@9"), "eslint");
        assert_eq!(default_bin_name("cargo:ripgrep"), "ripgrep");
    }

    #[test]
    fn parse_line_defaults_bin_name_when_not_declared() {
        let pkg = parse_line("go@latest\teager").unwrap();
        assert_eq!(
            pkg,
            Package {
                specifier: "go@latest".to_string(),
                bin_name: "go".to_string(),
                invoke_name: "go".to_string(),
                eager: true,
            }
        );
    }

    #[test]
    fn parse_line_honors_explicit_bin_name_override() {
        let pkg = parse_line("github:elio-fm/elio@latest\tlazy\telio-cli").unwrap();
        assert_eq!(pkg.bin_name, "elio-cli");
        assert_eq!(pkg.invoke_name, "elio-cli");
        assert!(!pkg.eager);
    }

    #[test]
    fn parse_line_honors_explicit_invoke_name_distinct_from_bin_name() {
        let pkg =
            parse_line("npm:@anthropic-ai/claude-code@latest\tlazy\tclauden\tclaude").unwrap();
        assert_eq!(pkg.bin_name, "clauden");
        assert_eq!(pkg.invoke_name, "claude");
    }

    #[test]
    fn parse_line_rejects_unknown_classification() {
        let err = parse_line("go@latest\tnow").unwrap_err();
        assert!(err.contains("now"));
    }

    #[test]
    fn load_returns_empty_when_declarations_file_is_absent() {
        let dir = std::env::temp_dir().join(format!("mysh-package-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir).unwrap(), Vec::new());
    }

    #[test]
    fn shim_script_execs_the_resolved_mise_bin_scoped_to_the_isolated_data_dir() {
        let target = Path::new("/home/user");
        let mise_bin = Path::new("/home/user/.mysh/bin/mise");
        let script = shim_script(mise_bin, target, "widget@1.0", "widget");
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("MISE_DATA_DIR=\"/home/user/.mysh/mise\""));
        assert!(script.contains("exec \"/home/user/.mysh/bin/mise\" x widget@1.0 -- widget \"$@\"\n"));
    }
}
