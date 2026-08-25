use std::fmt;
use std::path::PathBuf;

/// Which comparison a piece of drift came from — the three-state model
/// (Target/Source/Remote) plus Directory-mode's new/missing scan results.
/// Remote drift is split by direction (ahead/behind/diverged) rather than a
/// single flat side, so a caller can tell "needs save" from "needs reset"
/// without re-deriving it — mysh does not attempt three-way merges, so a
/// diverged path is never auto-resolvable either direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriftSide {
    /// Live Target disagrees with a fresh render of Source.
    Target,
    /// Source has content Remote doesn't (uncommitted, unpushed, or untracked) — a save candidate.
    Ahead,
    /// Remote has content Source doesn't — a reset candidate.
    Behind,
    /// Both sides changed the same path since they last agreed — needs manual git resolution.
    Diverged,
    /// Directory-mode: present in Target, absent from Source (save candidate).
    New,
    /// Directory-mode: present in Source, absent from Target (reset candidate).
    Missing,
}

/// These exact strings are part of the `diff`/`diff --quick` output contract:
/// `profile/.config/starship.toml`'s `[custom.mysh]` awk script matches
/// "target"/"ahead"/"behind"/"diverged" literally, with no way for the shell
/// side to share this `impl` — a renamed arm here silently breaks the prompt.
/// `drift_side_display_strings_are_the_starship_prompt_contract` below pins
/// them so a rename fails loudly here instead.
impl fmt::Display for DriftSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DriftSide::Target => "target",
            DriftSide::Ahead => "ahead",
            DriftSide::Behind => "behind",
            DriftSide::Diverged => "diverged",
            DriftSide::New => "new",
            DriftSide::Missing => "missing",
        })
    }
}

#[derive(Clone, Debug)]
pub struct Drift {
    /// Target-relative path (source-relative for Ahead/Behind/Diverged, where
    /// the raw Source file — `.age` suffix and all — is what git compares).
    pub rel: PathBuf,
    pub side: DriftSide,
}

/// The diff output contract: one `<rel>\t<side>` line per drift, `clean` when none.
pub fn format(drifts: &[Drift]) -> String {
    if drifts.is_empty() {
        return "clean\n".to_string();
    }
    let mut out = String::new();
    for drift in drifts {
        out.push_str(&format!("{}\t{}\n", drift.rel.display(), drift.side));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_drift_formats_as_clean() {
        assert_eq!(format(&[]), "clean\n");
    }

    /// `profile/.config/starship.toml`'s awk script matches these four strings
    /// literally with no compiler to catch a rename — this is that compiler.
    #[test]
    fn drift_side_display_strings_are_the_starship_prompt_contract() {
        assert_eq!(DriftSide::Target.to_string(), "target");
        assert_eq!(DriftSide::Ahead.to_string(), "ahead");
        assert_eq!(DriftSide::Behind.to_string(), "behind");
        assert_eq!(DriftSide::Diverged.to_string(), "diverged");
    }

    #[test]
    fn drift_lines_are_tab_separated_rel_and_side() {
        let drifts = [
            Drift {
                rel: ".bashrc".into(),
                side: DriftSide::Target,
            },
            Drift {
                rel: ".vimrc".into(),
                side: DriftSide::Ahead,
            },
        ];
        assert_eq!(format(&drifts), ".bashrc\ttarget\n.vimrc\tahead\n");
    }
}
