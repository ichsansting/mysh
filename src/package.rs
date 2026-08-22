use crate::log::AppLog;
use crate::mise;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

/// The marker line inside a shim file that makes its Package eager: `apply` collects
/// every marked shim's specifier into one batch `mise install` (see ADR-0007). The
/// shim file is the single per-package source of truth, so eagerness lives in it too —
/// not in a declarations file (killed with `.packages`) or mise's `[tools]` table
/// (killed with ADR-0006's mechanism split).
pub(crate) const EAGER_MARKER: &str = "# mysh: eager";

/// The binary name a specifier produces when not explicitly overridden: any backend
/// prefix (`github:`, `npm:`, ...) and version pin (`@...`) stripped, then the last
/// `/`-separated segment — e.g. `github:elio-fm/elio@latest` -> `elio`, `go@latest` ->
/// `go`.
pub(crate) fn default_bin_name(specifier: &str) -> String {
    let rest = specifier.split_once(':').map_or(specifier, |(_backend, rest)| rest);
    let rest = rest.split('@').next().unwrap_or(rest);
    rest.rsplit('/').next().unwrap_or(rest).to_string()
}

/// A Package's shim: installs (on first use) and execs the real tool via `mise x
/// <specifier> -- <invoke_name> "$@"`. Portable by design (see ADR-0006) — resolves
/// `$HOME` and `mise` at *run time* rather than baking in a device-specific path, since
/// this is a real file checked into Source and shared across every device it's
/// rendered to, not something generated fresh per `apply`. `mise` is called bare,
/// relying on it being resolvable on `PATH` by the time this actually runs (either a
/// system-wide `mise`, or mysh's own bootstrapped one in `.mysh/bin`, which `apply`
/// guarantees gets bootstrapped whenever any package is declared). An eager package
/// gets the exact same shim plus the `EAGER_MARKER` line — same file, same mechanism,
/// the marker only changes *when* the tool gets installed (see ADR-0007).
pub(crate) fn shim_script(specifier: &str, invoke_name: &str, eager: bool) -> String {
    let marker = if eager { format!("{EAGER_MARKER}\n") } else { String::new() };
    format!(
        "#!/bin/sh\n{marker}export MISE_DATA_DIR=\"$HOME/{}\"\nexec mise x {specifier} -- {invoke_name} \"$@\"\n",
        mise::DATA_DIR_REL,
    )
}

/// The specifier a shim file execs, parsed back out of its `exec mise x <specifier> --
/// ...` line. `None` for content that doesn't have one — a hand-written script in the
/// shim dir that doesn't follow the `add`-written shape simply isn't prewarmable, which
/// is the right failure mode: it still renders and runs like any file, it just stays
/// lazy.
pub(crate) fn shim_specifier(content: &str) -> Option<&str> {
    let line = content.lines().find(|line| line.starts_with("exec mise x "))?;
    line.strip_prefix("exec mise x ")?.split_once(" -- ").map(|(specifier, _)| specifier)
}

/// Every eager-marked shim's specifier in `source`'s shim dir, deduplicated and
/// sorted (`read_dir` order is platform-dependent; a deterministic order keeps the
/// batched `mise install` invocation — and the Application Log entries recorded from
/// it — stable across runs and machines).
fn eager_specifiers(source: &Path) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(mise::bin_dir(source)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut specifiers = BTreeSet::new();
    for entry in entries {
        let content = match fs::read_to_string(entry?.path()) {
            Ok(content) => content,
            // Non-UTF-8 (not an `add`-written shim) can't carry the marker — skip.
            Err(_) => continue,
        };
        if content.lines().any(|line| line == EAGER_MARKER) {
            if let Some(specifier) = shim_specifier(&content) {
                specifiers.insert(specifier.to_string());
            }
        }
    }
    Ok(specifiers.into_iter().collect())
}

/// Writes an executable file at `path`, matching the idempotence every other write path
/// in this codebase already has (`apply::copy_if_changed`, `secret::write_restricted`):
/// a no-op when `content` already matches what's on disk. Used by `add` to write a
/// shim directly into Source.
#[cfg(unix)]
pub(crate) fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    if !fs::read_to_string(path).map(|existing| existing == content).unwrap_or(false) {
        fs::File::create(path)?.write_all(content.as_bytes())?;
    }
    // Explicit chmod after the write, not a mode on create: create-time modes are
    // filtered through the process umask (and any default ACLs on the directory), so
    // a `mode(0o755)` open can still yield a non-executable file. `set_permissions`
    // applies the bits verbatim — same pattern as `secret::write_restricted` and
    // `apply::set_executable`.
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
pub(crate) fn write_if_changed_executable(path: &Path, content: &str) -> io::Result<()> {
    fs::write(path, content)
}

/// Whether `source` declares any Package at all — a single shim-dir presence check now
/// that eager and lazy are both real files in it (see ADR-0007).
fn is_package_declared(source: &Path) -> bool {
    fs::read_dir(mise::bin_dir(source)).map(|mut entries| entries.next().is_some()).unwrap_or(false)
}

/// Prewarms every eager package in one batched `mise install <specifier>...` (mise
/// parallelizes the downloads itself — the entire point of eager over lazy),
/// self-bootstrapping `mise` first if needed. A no-op — `mise` never touched — only
/// when `source` declares no package at all. A lazy-only device still bootstraps
/// `mise` up front (without installing anything) so its shim files have something to
/// invoke on first real use, rather than hoping one turns up on `PATH` later. The shim
/// files themselves need no step here — they're real files already identity-copied
/// into `.mysh/bin` by the ordinary render pass before this runs.
pub fn apply(source: &Path, target: &Path, log: &AppLog) -> Result<(), String> {
    if !is_package_declared(source) {
        return Ok(());
    }
    let mise_bin = mise::ensure_installed(target, log)?;

    let eager_specs = eager_specifiers(source).map_err(|e| e.to_string())?;
    if !eager_specs.is_empty() {
        mise::install(&mise_bin, target, &eager_specs)?;
        for specifier in &eager_specs {
            log.record_package_installed(specifier).map_err(|e| e.to_string())?;
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
        let script = shim_script("widget@1.0", "widget", false);
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(r#"MISE_DATA_DIR="$HOME/.mysh/mise""#));
        assert!(script.contains("exec mise x widget@1.0 -- widget \"$@\"\n"));
        // No absolute, device-specific path baked in anywhere.
        assert!(!script.contains("/home/"));
    }

    #[test]
    fn shim_script_eager_differs_only_by_the_marker_line() {
        let lazy = shim_script("widget@1.0", "widget", false);
        let eager = shim_script("widget@1.0", "widget", true);
        assert_eq!(eager, lazy.replacen("#!/bin/sh\n", &format!("#!/bin/sh\n{EAGER_MARKER}\n"), 1));
    }

    #[test]
    fn shim_specifier_round_trips_what_shim_script_writes() {
        assert_eq!(shim_specifier(&shim_script("go@latest", "go", false)), Some("go@latest"));
        assert_eq!(
            shim_specifier(&shim_script("github:elio-fm/elio@latest", "elio-cli", true)),
            Some("github:elio-fm/elio@latest")
        );
        // A hand-written script that doesn't follow the shim shape parses to nothing.
        assert_eq!(shim_specifier("#!/bin/sh\nexec some-other-tool\n"), None);
    }

    #[test]
    fn eager_specifiers_collects_only_marked_shims_sorted_and_deduplicated() {
        let dir = std::env::temp_dir().join(format!("mysh-package-eager-test-{}", std::process::id()));
        let bin = dir.join(".mysh/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("widget"), shim_script("widget@1.0", "widget", true)).unwrap();
        fs::write(bin.join("gadget"), shim_script("gadget@2.0", "gadget", true)).unwrap();
        // Same specifier exposed under a second bin name: installed once, not twice.
        fs::write(bin.join("widget2"), shim_script("widget@1.0", "widget2", true)).unwrap();
        fs::write(bin.join("lazy-one"), shim_script("lazy@1.0", "lazy-one", false)).unwrap();

        assert_eq!(eager_specifiers(&dir).unwrap(), vec!["gadget@2.0", "widget@1.0"]);
    }

    #[test]
    fn is_package_declared_is_false_for_an_empty_source() {
        let dir = std::env::temp_dir().join(format!("mysh-package-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_package_declared(&dir));
    }
}
