mod support;

use mysh::git;
use mysh::secret;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use support::temp_dir;

const PASSPHRASE: &str = "correct horse battery staple";

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("fragment-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    remote
}

fn run_apply(source: &Path, target: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("apply")
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .arg("--passphrase")
        .arg(PASSPHRASE)
        .status()
        .expect("failed to run mysh apply");
    assert!(status.success());
}

fn run_diff(source: &Path, target: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("diff")
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .arg("--passphrase")
        .arg(PASSPHRASE)
        .output()
        .expect("failed to run mysh diff");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

fn run_save_or_reset(cmd: &str, source: &Path, target: &Path, answer: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg(cmd)
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .arg("--passphrase")
        .arg(PASSPHRASE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to run mysh {cmd}: {e}"));
    child.stdin.take().unwrap().write_all(answer.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn apply_concatenates_plain_and_secret_fragments_in_lexical_order() {
    let source = temp_dir("fragment-apply-source");
    let target = temp_dir("fragment-apply-target");

    fs::create_dir_all(source.join("nvim/init.d")).unwrap();
    fs::write(source.join("nvim/init.d/10-base"), b"base config\n").unwrap();
    let secret_envelope = secret::encrypt(b"secret config\n", PASSPHRASE).unwrap();
    fs::write(source.join("nvim/init.d/20-secret.age"), &secret_envelope).unwrap();
    fs::write(source.join("nvim/init.d/30-work"), b"work config\n").unwrap();

    run_apply(&source, &target);

    let rendered = target.join("nvim/init");
    assert_eq!(
        fs::read_to_string(&rendered).unwrap(),
        "base config\nsecret config\nwork config\n"
    );
    // Only the merged file exists on Target — no directory, no per-fragment files.
    assert!(!target.join("nvim/init.d").exists());
}

#[test]
fn a_newly_added_fragment_is_picked_up_by_the_next_apply_with_no_registration() {
    let source = temp_dir("fragment-new-source");
    let target = temp_dir("fragment-new-target");

    fs::create_dir_all(source.join("shell.d")).unwrap();
    fs::write(source.join("shell.d/10-base"), b"alias ls=ls\n").unwrap();
    run_apply(&source, &target);
    assert_eq!(fs::read_to_string(target.join("shell")).unwrap(), "alias ls=ls\n");

    // Drop a new fragment straight onto disk — no separate registration step.
    fs::write(source.join("shell.d/20-extra"), b"alias grep=grep\n").unwrap();
    run_apply(&source, &target);

    assert_eq!(
        fs::read_to_string(target.join("shell")).unwrap(),
        "alias ls=ls\nalias grep=grep\n"
    );
}

#[test]
fn diff_shows_drift_between_live_merged_file_and_fresh_render_from_fragments() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("fragment-diff-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::create_dir_all(seed.join("shell.d")).unwrap();
    fs::write(seed.join("shell.d/10-base"), b"alias ls=ls\n").unwrap();
    git::commit(&seed, "seed shell fragment").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("fragment-diff-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("fragment-diff-target");
    run_apply(&source, &target);

    // No drift right after a fresh apply.
    assert_eq!(run_diff(&source, &target), "");

    // Hand-edit the merged Target file directly.
    fs::write(target.join("shell"), b"alias ls=ls\nhand edit\n").unwrap();
    assert_eq!(run_diff(&source, &target), "shell\ttarget\n");
}

#[test]
fn save_is_rejected_for_a_composed_target() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("fragment-save-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::create_dir_all(seed.join("shell.d")).unwrap();
    fs::write(seed.join("shell.d/10-base"), b"alias ls=ls\n").unwrap();
    git::commit(&seed, "seed shell fragment").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("fragment-save-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("fragment-save-target");
    run_apply(&source, &target);

    // Drift on the merged Target file — save must refuse, not silently attribute it to
    // a fragment.
    fs::write(target.join("shell"), b"alias ls=ls\nhand edit\n").unwrap();

    let output = run_save_or_reset("save", &source, &target, "y\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("shell.d"), "expected message pointing at the fragment dir: {stderr}");

    // Nothing was written into Source's fragments.
    assert_eq!(fs::read_to_string(source.join("shell.d/10-base")).unwrap(), "alias ls=ls\n");
    assert!(!source.join("shell").exists());
}

#[test]
fn reset_discards_drift_by_rerendering_fresh_from_fragments() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("fragment-reset-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::create_dir_all(seed.join("shell.d")).unwrap();
    fs::write(seed.join("shell.d/10-base"), b"alias ls=ls\n").unwrap();
    git::commit(&seed, "seed shell fragment").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("fragment-reset-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("fragment-reset-target");
    run_apply(&source, &target);

    // Hand-edit drift on the merged Target file.
    fs::write(target.join("shell"), b"totally different\n").unwrap();

    let output = run_save_or_reset("reset", &source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(fs::read_to_string(target.join("shell")).unwrap(), "alias ls=ls\n");
}
