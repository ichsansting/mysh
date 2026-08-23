use crate::domain::MISE_DATA_DIR_REL;

/// The marker line that makes a shim's Package eager: apply collects every marked
/// shim's specifier into one batched `mise install` (ADR-0007). The shim file is the
/// single per-package source of truth, so eagerness lives in it too.
pub const EAGER_MARKER: &str = "# mysh: eager";

/// The binary name a specifier produces when not overridden with `--bin`: backend
/// prefix (`github:`, `npm:`, ...) and version pin (`@...`) stripped, then the last
/// `/`-separated segment — `github:elio-fm/elio@latest` -> `elio`.
pub fn default_bin_name(specifier: &str) -> String {
    let rest = specifier.split_once(':').map_or(specifier, |(_backend, rest)| rest);
    // A version pin is an `@` after position 0 — a leading `@` is an npm scope.
    let rest = match rest.char_indices().find(|&(i, c)| c == '@' && i > 0) {
        Some((i, _)) => &rest[..i],
        None => rest,
    };
    rest.rsplit('/').next().unwrap_or(rest).to_string()
}

/// A Package's shim: installs (on first use) and execs the real tool via
/// `mise x <specifier> -- <invoke_name> "$@"`. Portable by design (ADR-0006/0007) —
/// resolves `$HOME` and `mise` at run time rather than baking in a device-specific
/// path, since this is a real file checked into Source and shared across devices.
/// An eager package gets the exact same shim plus the marker line.
pub fn shim_script(specifier: &str, invoke_name: &str, eager: bool) -> String {
    let marker = if eager { format!("{EAGER_MARKER}\n") } else { String::new() };
    format!(
        "#!/bin/sh\n{marker}export MISE_DATA_DIR=\"$HOME/{MISE_DATA_DIR_REL}\"\nexec mise x {specifier} -- {invoke_name} \"$@\"\n"
    )
}

/// The specifier a shim execs, parsed back out of its `exec mise x <specifier> -- ...`
/// line. `None` for content not following the `add`-written shape — such a hand-edited
/// shim simply isn't prewarmable and stays lazy, never an error (ADR-0007).
pub fn shim_specifier(content: &str) -> Option<&str> {
    let line = content.lines().find(|line| line.starts_with("exec mise x "))?;
    line.strip_prefix("exec mise x ")?.split_once(" -- ").map(|(specifier, _)| specifier)
}

/// Whether shim content carries the eager marker.
pub fn is_eager(content: &str) -> bool {
    content.lines().any(|line| line == EAGER_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_round_trips_specifier_and_eagerness() {
        let lazy = shim_script("ripgrep", "rg", false);
        assert_eq!(shim_specifier(&lazy), Some("ripgrep"));
        assert!(!is_eager(&lazy));

        let eager = shim_script("github:elio-fm/elio@latest", "elio", true);
        assert_eq!(shim_specifier(&eager), Some("github:elio-fm/elio@latest"));
        assert!(is_eager(&eager));
    }

    #[test]
    fn shim_contains_no_device_specific_absolute_path() {
        let shim = shim_script("fish", "fish", true);
        assert!(!shim.contains("/home/"), "must resolve $HOME at run time: {shim}");
        assert!(shim.contains("$HOME"));
    }

    #[test]
    fn default_bin_name_strips_backend_version_and_path() {
        assert_eq!(default_bin_name("ripgrep"), "ripgrep");
        assert_eq!(default_bin_name("go@latest"), "go");
        assert_eq!(default_bin_name("github:elio-fm/elio@latest"), "elio");
        assert_eq!(default_bin_name("npm:@anthropic-ai/claude-code"), "claude-code");
    }

    #[test]
    fn hand_edited_content_without_the_exec_line_is_not_a_shim() {
        assert_eq!(shim_specifier("#!/bin/sh\necho hi\n"), None);
    }
}
