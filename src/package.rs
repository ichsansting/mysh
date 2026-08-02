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
    /// The binary name this specifier produces. Defaults from `specifier` when not
    /// explicitly declared.
    pub bin_name: String,
    /// Installed immediately during `apply` (`true`) vs. on first invocation via a
    /// generated shim (`false`, see issue 09).
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

/// One package per line, tab-separated: `<specifier>\t<eager|lazy>[\t<bin_name>]`.
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
    Ok(Package { specifier, bin_name, eager })
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

/// Installs every eager package declared in `source`'s `.packages` file, self-bootstrapping
/// `mise` first if it isn't already present. A no-op when no eager packages are declared —
/// `apply` never touches `mise` for a device with nothing eager to install.
pub fn install_eager(source: &Path, target: &Path, log: &AppLog) -> Result<(), String> {
    let eager: Vec<Package> = load(source)?.into_iter().filter(|p| p.eager).collect();
    if eager.is_empty() {
        return Ok(());
    }
    mise::ensure_installed(log)?;
    for package in &eager {
        mise::install(target, &package.specifier)?;
        log.record_package_installed(&package.specifier).map_err(|e| e.to_string())?;
    }
    Ok(())
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
        assert_eq!(pkg, Package { specifier: "go@latest".to_string(), bin_name: "go".to_string(), eager: true });
    }

    #[test]
    fn parse_line_honors_explicit_bin_name_override() {
        let pkg = parse_line("github:elio-fm/elio@latest\tlazy\telio-cli").unwrap();
        assert_eq!(pkg.bin_name, "elio-cli");
        assert!(!pkg.eager);
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
}
