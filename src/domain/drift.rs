use std::fmt;
use std::path::PathBuf;

/// Which comparison a piece of drift came from — the three-state model
/// (Target/Source/Remote) plus Directory-mode's new/missing scan results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriftSide {
    /// Live Target disagrees with a fresh render of Source.
    Target,
    /// Source disagrees with Remote (uncommitted, unpushed, or remote-only).
    Remote,
    /// Directory-mode: present in Target, absent from Source (save candidate).
    New,
    /// Directory-mode: present in Source, absent from Target (reset candidate).
    Missing,
}

impl fmt::Display for DriftSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DriftSide::Target => "target",
            DriftSide::Remote => "remote",
            DriftSide::New => "new",
            DriftSide::Missing => "missing",
        })
    }
}

#[derive(Debug)]
pub struct Drift {
    /// Target-relative path (source-relative for Remote drift, where the raw
    /// Source file — `.age` suffix and all — is what git compares).
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

    #[test]
    fn drift_lines_are_tab_separated_rel_and_side() {
        let drifts = [
            Drift { rel: ".bashrc".into(), side: DriftSide::Target },
            Drift { rel: ".vimrc".into(), side: DriftSide::Remote },
        ];
        assert_eq!(format(&drifts), ".bashrc\ttarget\n.vimrc\tremote\n");
    }
}
