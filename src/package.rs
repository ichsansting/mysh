use crate::log::AppLog;
use crate::mise;
use std::fs;
use std::io;
use std::path::Path;

/// The binary name a specifier produces when not explicitly overridden: any backend
/// prefix (`github:`, `npm:`, ...) and version pin (`@...`) stripped, then the last
/// `/`-separated segment — e.g. `github:elio-fm/elio@latest` -> `elio`, `go@latest` ->
/// `go`. Only used by `add`'s lazy path now — eager packages let `mise` resolve their
/// own binary name.
pub(crate) fn default_bin_name(specifier: &str) -> String {
    let rest = specifier.split_once(':').map_or(specifier, |(_backend, rest)| rest);
    let rest = rest.split('@').next().unwrap_or(rest);
    rest.rsplit('/').next().unwrap_or(rest).to_string()
}

/// A lazy package's shim: installs (on first use) and execs the real tool via `mise x
/// <specifier> -- <invoke_name> "$@"`. Portable by design (see ADR-0006) — resolves
/// `$HOME` and `mise` at *run time* rather than baking in a device-specific path, since
/// this is now a real file checked into Source and shared across every device it's
/// rendered to, not something generated fresh per `apply`. `mise` is called bare,
/// relying on it being resolvable on `PATH` by the time this actually runs (either a
/// system-wide `mise`, or mysh's own bootstrapped one in `.mysh/bin`, which `apply`
/// guarantees gets bootstrapped whenever any package — eager or lazy — is declared).
pub(crate) fn shim_script(specifier: &str, invoke_name: &str) -> String {
    format!(
        "#!/bin/sh\nexport MISE_DATA_DIR=\"$HOME/{}\"\nexec mise x {specifier} -- {invoke_name} \"$@\"\n",
        mise::DATA_DIR_REL,
    )
}

/// Writes an executable file at `path`, matching the idempotence every other write path
/// in this codebase already has (`apply::copy_if_changed`, `secret::write_restricted`):
/// a no-op when `content` already matches what's on disk. Used by `add`'s lazy path to
/// write a shim directly into Source.
#[cfg(unix)]
pub(crate) fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
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
pub(crate) fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
    fs::write(path, content)
}

/// Whether `source` has any file in the mirrored lazy-shim dir — cheap presence check,
/// no need to read file contents.
fn is_lazy_declared(source: &Path) -> bool {
    fs::read_dir(mise::bin_dir(source)).map(|mut entries| entries.next().is_some()).unwrap_or(false)
}

/// Whether `source`'s `config.toml` has a `[tools]` table — a plain substring check,
/// not a real TOML parse, just a cheap gate for whether it's worth invoking `mise` at
/// all. A false positive costs one harmless no-op `mise install`; a false negative
/// can't happen since `declare_tool` always writes a literal `[tools]` header.
fn is_eager_declared(source: &Path) -> bool {
    fs::read_to_string(mise::config_path(source)).map(|text| text.contains("[tools]")).unwrap_or(false)
}

/// Installs every eager package (declared in `config.toml`'s `[tools]` table) in one
/// blanket `mise install`, self-bootstrapping `mise` first if needed. A no-op — `mise`
/// never touched — only when `source` declares no package of either kind at all. A
/// lazy-only device still bootstraps `mise` up front (without installing anything, and
/// without any eager-specific step) so its shim files have something to invoke on
/// first real use, rather than hoping one turns up on `PATH` later. Lazy packages
/// themselves need no apply-time step here — they're real files already
/// identity-copied into `.mysh/bin` by the ordinary render pass before this runs.
pub fn apply(source: &Path, target: &Path, log: &AppLog) -> Result<(), String> {
    let eager = is_eager_declared(source);
    if !is_lazy_declared(source) && !eager {
        return Ok(());
    }
    let mise_bin = mise::ensure_installed(target, log)?;

    if eager {
        mise::install_declared(&mise_bin, target)?;
        for specifier in mise::declared_tools(&mise_bin, &mise::config_path(target))? {
            log.record_package_installed(&specifier).map_err(|e| e.to_string())?;
        }
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
    fn shim_script_resolves_home_and_mise_at_run_time_not_write_time() {
        let script = shim_script("widget@1.0", "widget");
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(r#"MISE_DATA_DIR="$HOME/.mysh/mise""#));
        assert!(script.contains("exec mise x widget@1.0 -- widget \"$@\"\n"));
        // No absolute, device-specific path baked in anywhere.
        assert!(!script.contains("/home/"));
    }

    #[test]
    fn lazy_and_eager_declared_are_false_for_an_empty_source() {
        let dir = std::env::temp_dir().join(format!("mysh-package-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_lazy_declared(&dir));
        assert!(!is_eager_declared(&dir));
    }
}
