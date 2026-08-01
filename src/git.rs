use std::path::Path;
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

/// Stages every change in the working tree (`git add -A`) and commits it.
pub fn commit(repo_dir: &Path, message: &str) -> Result<(), String> {
    run(repo_dir, &["add", "-A"])?;
    run(repo_dir, &["commit", "-m", message]).map(|_| ())
}

/// Pushes the current branch to its configured remote.
pub fn push(repo_dir: &Path) -> Result<(), String> {
    run(repo_dir, &["push"]).map(|_| ())
}
