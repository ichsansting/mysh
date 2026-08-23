use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs git in `dir`, erroring (with stderr) on non-zero exit.
pub fn git(dir: &Path, args: &[&str]) -> Result<Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| Error::Subprocess { program: "git", detail: e.to_string() })?;
    if !output.status.success() {
        return Err(Error::Subprocess {
            program: "git",
            detail: format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(output)
}

fn stdout_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout).lines().map(str::to_string).collect()
}

/// Whether `dir` is inside a git working tree at all. Source without git (or
/// without a Remote) still supports apply/diff-vs-target — Remote drift is
/// simply not reported.
pub fn is_repo(dir: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Every Source path that differs from Remote's tip: fetches origin, then the
/// union of tracked differences vs `origin/main` (uncommitted, committed-but-
/// unpushed, and remote-only files) and untracked files. On a Remote with no
/// commits yet, everything unpushed is drift.
pub fn paths_differing_from_remote(dir: &Path) -> Result<Vec<PathBuf>> {
    git(dir, &["fetch", "-q", "origin"])?;
    let mut paths: Vec<String> = Vec::new();
    if has_remote_main(dir) {
        paths.extend(stdout_lines(&git(dir, &["diff", "--name-only", "origin/main"])?));
    } else {
        paths.extend(stdout_lines(&git(dir, &["ls-files"])?));
    }
    paths.extend(stdout_lines(&git(dir, &["ls-files", "--others", "--exclude-standard"])?));
    paths.sort();
    paths.dedup();
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

fn has_remote_main(dir: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--verify", "-q", "origin/main"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Stages everything, commits, and pushes Source's main to origin (Save's git leg).
pub fn commit_and_push(dir: &Path, message: &str) -> Result<()> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", message])?;
    git(dir, &["push", "-q", "origin", "HEAD:main"])?;
    Ok(())
}

/// Forces Source to exactly match Remote's tip, discarding local commits,
/// edits, and untracked files (Reset's git leg).
pub fn force_match_remote(dir: &Path) -> Result<()> {
    git(dir, &["fetch", "-q", "origin"])?;
    git(dir, &["reset", "-q", "--hard", "origin/main"])?;
    git(dir, &["clean", "-qfd"])?;
    Ok(())
}
