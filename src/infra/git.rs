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
        .map_err(|e| Error::Subprocess {
            program: "git",
            detail: e.to_string(),
        })?;
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
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
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
        paths.extend(stdout_lines(&git(
            dir,
            &["diff", "--name-only", "origin/main"],
        )?));
    } else {
        paths.extend(stdout_lines(&git(dir, &["ls-files"])?));
    }
    paths.extend(stdout_lines(&git(
        dir,
        &["ls-files", "--others", "--exclude-standard"],
    )?));
    paths.sort();
    paths.dedup();
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

pub(crate) fn has_remote_main(dir: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--verify", "-q", "origin/main"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Stages, commits, and pushes exactly `paths` (Source-relative) to origin —
/// Save's git leg, scoped to whatever the picker selected. Everything else
/// pending in Source is left untouched for a later save. A no-op (nothing
/// staged for these paths — e.g. a stale selection) is not an error.
pub fn commit_and_push(dir: &Path, message: &str, paths: &[PathBuf]) -> Result<()> {
    if paths.is_empty() {
        // `git status --porcelain --` with an empty pathspec after `--` isn't
        // an empty filter, it's *no* filter — it'd match the whole working
        // tree instead of "nothing," so this has to be caught before it ever
        // gets there rather than trusted to the emptiness check below.
        return Ok(());
    }
    let path_strs: Vec<&str> = paths.iter().filter_map(|p| p.to_str()).collect();
    let mut add_args = vec!["add", "--"];
    add_args.extend(&path_strs);
    git(dir, &add_args)?;

    let mut status_args = vec!["status", "--porcelain", "--"];
    status_args.extend(&path_strs);
    let status = git(dir, &status_args)?;
    if stdout_lines(&status).is_empty() {
        return Ok(());
    }

    git(dir, &["commit", "-q", "-m", message])?;
    git(dir, &["push", "-q", "origin", "HEAD:main"])?;
    Ok(())
}

/// Runs `git diff --no-index a b`. Exit 1 (differences found) is the expected,
/// common case here — only other exit codes are treated as failure.
pub fn diff_no_index(a: &Path, b: &Path) -> Result<String> {
    diff_output(
        Command::new("git")
            .args(["diff", "--no-index", "--"])
            .arg(a)
            .arg(b),
    )
}

/// Content diff of a Source-relative path against Remote: against `origin/main`
/// when it exists, else against nothing (the whole file reads as added — a
/// Remote with no commits yet has nothing else to compare against).
pub fn diff_source_vs_remote(source_dir: &Path, rel: &Path) -> Result<String> {
    let source_path = source_dir.join(rel);
    if has_remote_main(source_dir) {
        diff_output(
            Command::new("git")
                .arg("-C")
                .arg(source_dir)
                .args(["diff", "origin/main", "--"])
                .arg(rel),
        )
    } else {
        diff_output(
            Command::new("git")
                .args(["diff", "--no-index", "--"])
                .arg("/dev/null")
                .arg(&source_path),
        )
    }
}

fn diff_output(cmd: &mut Command) -> Result<String> {
    let output = cmd.output().map_err(|e| Error::Subprocess {
        program: "git",
        detail: e.to_string(),
    })?;
    match output.status.code() {
        Some(0) | Some(1) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        _ => Err(Error::Subprocess {
            program: "git",
            detail: format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }),
    }
}

/// Forces Source to exactly match Remote's tip, discarding local commits,
/// edits, and untracked files (Reset's git leg).
pub fn force_match_remote(dir: &Path) -> Result<()> {
    git(dir, &["fetch", "-q", "origin"])?;
    git(dir, &["reset", "-q", "--hard", "origin/main"])?;
    git(dir, &["clean", "-qfd"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A repo with unrelated dirty content, so an empty-pathspec `git status`
    /// (which matches everything, not nothing) would find something if it ran.
    fn repo_with_dirty_content() -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mysh-git-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&dir)
            .status()
            .unwrap();
        fs::write(dir.join("dirty.txt"), b"uncommitted").unwrap();
        dir
    }

    #[test]
    fn commit_and_push_with_no_paths_is_a_clean_noop() {
        // Regression: an empty pathspec after `--` isn't an empty filter to git
        // status, it's *no* filter — it'd match `dirty.txt` and this would try
        // (and fail) to commit nothing, instead of just no-op'ing as documented.
        let dir = repo_with_dirty_content();
        let result = commit_and_push(&dir, "msg", &[]);
        assert!(result.is_ok(), "{result:?}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
