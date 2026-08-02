mod support;

use mysh::git;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use support::temp_dir;

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("reset-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    remote
}

fn run_reset(source: &Path, target: &Path, answer: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("reset")
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mysh reset");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(answer.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn remote_content(remote_url: &str, relative: &str) -> Vec<u8> {
    let check = temp_dir("reset-remote-check");
    git::clone(remote_url, &check).unwrap();
    fs::read(check.join(relative)).unwrap()
}

#[test]
fn confirmed_reset_discards_local_drift_and_reapplies_remote() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("reset-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("reset-source");
    git::clone(&remote_url, &source).unwrap();
    // Uncommitted local drift in Source, never pushed.
    fs::write(source.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    // Separate, independent drift on Target.
    let target = temp_dir("reset-target");
    fs::write(target.join("bashrc"), b"export EDITOR=emacs\n").unwrap();

    let output = run_reset(&source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=vim\n");
    assert_eq!(fs::read(target.join("bashrc")).unwrap(), b"export EDITOR=vim\n");
    assert_eq!(remote_content(&remote_url, "bashrc"), b"export EDITOR=vim\n");
}

#[test]
fn declined_reset_leaves_source_target_and_remote_unchanged() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("reset-decline-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("reset-decline-source");
    git::clone(&remote_url, &source).unwrap();
    fs::write(source.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let target = temp_dir("reset-decline-target");
    fs::write(target.join("bashrc"), b"export EDITOR=emacs\n").unwrap();

    let output = run_reset(&source, &target, "n\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=nano\n");
    assert_eq!(fs::read(target.join("bashrc")).unwrap(), b"export EDITOR=emacs\n");
    assert_eq!(remote_content(&remote_url, "bashrc"), b"export EDITOR=vim\n");
}

#[test]
fn reset_with_no_drift_is_a_noop() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("reset-noop-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    git::commit(&seed, "seed bashrc").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("reset-noop-source");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("reset-noop-target");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();

    // No confirmation needed: nothing drifted, so nothing is prompted or mutated.
    let output = run_reset(&source, &target, "");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "nothing to reset\n");
}
