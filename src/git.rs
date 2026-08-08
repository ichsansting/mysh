use std::path::{Path, PathBuf};
use std::process::Command;

/// Thin subprocess wrapper around the real `git` binary, discovered via PATH
/// (`Command::new("git")` resolves through PATH itself — no vendored git implementation).
fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;

    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Clones `remote_url` into `dest`, creating `dest`'s parent directory if needed.
pub fn clone(remote_url: &str, dest: &Path) -> Result<(), String> {
    let parent = dest.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    run(parent, &["clone", remote_url, &dest.to_string_lossy()]).map(|_| ())
}

/// Fetches from the repo's configured remote.
pub fn fetch(repo_dir: &Path) -> Result<(), String> {
    run(repo_dir, &["fetch"]).map(|_| ())
}

/// Porcelain working-tree status; empty string means clean.
pub fn status(repo_dir: &Path) -> Result<String, String> {
    run(repo_dir, &["status", "--porcelain"])
}

/// Stages every change under `repo_dir` (`git add -A -- .`) and commits it. The
/// explicit `-- .` pathspec matters when `repo_dir` is a subdirectory of a larger
/// repo: bare `git add -A` stages the *entire* working tree regardless of cwd.
pub fn commit(repo_dir: &Path, message: &str) -> Result<(), String> {
    run(repo_dir, &["add", "-A", "--", "."])?;
    run(repo_dir, &["commit", "-m", message]).map(|_| ())
}

/// Pushes the current branch to its configured remote.
pub fn push(repo_dir: &Path) -> Result<(), String> {
    run(repo_dir, &["push"]).map(|_| ())
}

/// Hard-resets the working tree and index to `rev`, discarding local drift in Source.
///
/// `git reset --hard` cannot be scoped to a pathspec — it always resets the *whole*
/// working tree/index the repo at `repo_dir` belongs to, regardless of `repo_dir`
/// itself. Safe when `repo_dir` is a dedicated Source clone (nothing else lives
/// there), but `repo_dir` must never be a subdirectory of a working tree that holds
/// other unrelated uncommitted state — that state would be discarded too.
pub fn reset_hard(repo_dir: &Path, rev: &str) -> Result<(), String> {
    run(repo_dir, &["reset", "--hard", rev]).map(|_| ())
}

/// The current branch's upstream ref (e.g. `origin/main`), representing `Remote`.
pub fn upstream_ref(repo_dir: &Path) -> Result<String, String> {
    run(
        repo_dir,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )
    .map(|s| s.trim().to_string())
}

/// Relative paths tracked in `rev`'s tree.
pub fn list_tree(repo_dir: &Path, rev: &str) -> Result<Vec<PathBuf>, String> {
    Ok(run(repo_dir, &["ls-tree", "-r", "--name-only", rev])?
        .lines()
        .map(PathBuf::from)
        .collect())
}

/// `relative_path`'s content at `rev`, or `None` if it doesn't exist there.
///
/// The leading `./` in the spec matters: `git show rev:path` resolves a bare path
/// against the repo *root*, not cwd, and errors outright when `repo_dir` is a
/// subdirectory. `./`-prefixing makes it resolve against `repo_dir` either way.
pub fn show(repo_dir: &Path, rev: &str, relative_path: &Path) -> Result<Option<Vec<u8>>, String> {
    let spec = format!("{rev}:./{}", relative_path.to_string_lossy());
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(["show", &spec])
        .output()
        .map_err(|e| format!("failed to run git show {spec}: {e}"))?;

    if output.status.success() {
        return Ok(Some(output.stdout));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("does not exist") || stderr.contains("exists on disk, but not") {
        Ok(None)
    } else {
        Err(format!("git show {spec} failed: {stderr}"))
    }
}
