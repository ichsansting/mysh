mod support;

use mysh::git;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use support::temp_dir;

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("save-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    remote
}

fn run_save(source: &Path, target: &Path, answer: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("save")
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mysh save");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(answer.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn remote_content(remote_url: &str, relative: &str) -> Vec<u8> {
    let check = temp_dir("save-remote-check");
    git::clone(remote_url, &check).unwrap();
    fs::read(check.join(relative)).unwrap()
}

#[test]
fn confirmed_save_captures_target_drift_commits_and_pushes() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("save-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("save-source");
    git::clone(&remote_url, &source).unwrap();

    // A live edit on disk, not yet captured into Source.
    let target = temp_dir("save-target");
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let output = run_save(&source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=nano\n");
    assert_eq!(remote_content(&remote_url, "bashrc"), b"export EDITOR=nano\n");
}

#[test]
fn declined_save_leaves_source_target_and_remote_unchanged() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("save-decline-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("save-decline-source");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("save-decline-target");
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let output = run_save(&source, &target, "n\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Source untouched by the declined save.
    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=vim\n");
    // Target untouched (save never writes to Target).
    assert_eq!(fs::read(target.join("bashrc")).unwrap(), b"export EDITOR=nano\n");
    // Remote untouched.
    assert_eq!(remote_content(&remote_url, "bashrc"), b"export EDITOR=vim\n");
}

#[test]
fn save_with_no_target_drift_is_a_noop() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("save-noop-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("save-noop-source");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("save-noop-target");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();

    // No confirmation needed: nothing drifted, so nothing is prompted or mutated.
    let output = run_save(&source, &target, "");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "nothing to save\n");
}
