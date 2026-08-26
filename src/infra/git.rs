use crate::domain::drift::DriftSide;
use crate::error::{Error, Result};
use std::collections::HashSet;
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

/// Whether `git -C dir rev-parse <args>` succeeds — the shared shape behind
/// every "does this ref/repo exist" check below (`is_repo`, `has_remote_main`,
/// `has_head`), which all treat a spawn failure the same way any of these
/// checks fails cheaply: not an error, just "no."
fn rev_parse_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("rev-parse")
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether `dir` is inside a git working tree at all. Source without git (or
/// without a Remote) still supports apply/diff-vs-target — Remote drift is
/// simply not reported.
pub fn is_repo(dir: &Path) -> bool {
    rev_parse_ok(dir, &["--git-dir"])
}

/// Every Source path that differs from Remote's tip, and which direction:
/// changed locally since the last common point (`Ahead`, a save candidate),
/// changed on Remote (`Behind`, an update candidate), or both (`Diverged` — no
/// three-way merge, so this is never auto-resolved either direction).
/// `fetch` controls whether Remote's tip is refreshed first: full `diff`
/// fetches for an accurate answer; `diff --quick` skips it (no network in the
/// prompt path), comparing against `origin/main` exactly as last fetched —
/// same staleness contract `git_status` prompts already have. On a Remote
/// with no commits yet, everything tracked is Ahead.
pub fn paths_differing_from_remote(dir: &Path, fetch: bool) -> Result<Vec<(PathBuf, DriftSide)>> {
    if fetch {
        git(dir, &["fetch", "-q", "origin"])?;
    }
    let mut result: Vec<(PathBuf, DriftSide)> = Vec::new();
    if has_remote_main(dir) {
        // A device that has never committed locally (its very first `diff`,
        // before ever running `save`/`add`) has no HEAD for `git diff`/`merge-base`
        // to resolve — nothing local can be Ahead by commit history, so every
        // path Remote has is simply Behind (this device's first Update candidate).
        let (local, remote): (HashSet<String>, HashSet<String>) = if has_head(dir) {
            let base = merge_base_with_remote(dir)?;
            let local = stdout_lines(&git(dir, &["diff", "--name-only", &base, "HEAD"])?)
                .into_iter()
                .chain(stdout_lines(&git(dir, &["diff", "--name-only", "HEAD"])?))
                .collect();
            let remote = stdout_lines(&git(dir, &["diff", "--name-only", &base, "origin/main"])?)
                .into_iter()
                .collect();
            (local, remote)
        } else {
            let remote = stdout_lines(&git(dir, &["ls-tree", "-r", "--name-only", "origin/main"])?)
                .into_iter()
                .collect();
            (HashSet::new(), remote)
        };
        for path in local.union(&remote) {
            let side = match (local.contains(path), remote.contains(path)) {
                (true, true) => DriftSide::Diverged,
                (true, false) => DriftSide::Ahead,
                (false, true) => DriftSide::Behind,
                (false, false) => unreachable!("path came from the union of these two sets"),
            };
            result.push((PathBuf::from(path), side));
        }
    } else {
        for path in stdout_lines(&git(dir, &["ls-files"])?) {
            result.push((PathBuf::from(path), DriftSide::Ahead));
        }
    }
    for path in stdout_lines(&git(dir, &["ls-files", "--others", "--exclude-standard"])?) {
        result.push((PathBuf::from(path), DriftSide::Ahead));
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

fn has_head(dir: &Path) -> bool {
    rev_parse_ok(dir, &["--verify", "-q", "HEAD"])
}

fn merge_base_with_remote(dir: &Path) -> Result<String> {
    let output = git(dir, &["merge-base", "HEAD", "origin/main"])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn has_remote_main(dir: &Path) -> bool {
    rev_parse_ok(dir, &["--verify", "-q", "origin/main"])
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
/// edits, and untracked files (Update's git leg).
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
