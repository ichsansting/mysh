mod support;

use mysh::git;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use support::temp_dir;

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("overlay-remote");
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to run mysh {cmd}: {e}"));
    child.stdin.take().unwrap().write_all(answer.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn apply_creates_the_target_file_when_it_does_not_exist_yet() {
    let source = temp_dir("overlay-create-source");
    let target = temp_dir("overlay-create-target");

    fs::write(source.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();

    run_apply(&source, &target);

    assert_eq!(json(&target.join(".claude.json")), serde_json::json!({"hasCompletedOnboarding": true}));
}

#[test]
fn apply_merges_declared_keys_onto_an_existing_target_file_preserving_the_rest() {
    let source = temp_dir("overlay-merge-source");
    let target = temp_dir("overlay-merge-target");

    fs::write(source.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    fs::write(
        target.join(".claude.json"),
        br#"{"hasCompletedOnboarding": false, "projects": {"/home/user/app": {}}}"#,
    )
    .unwrap();

    run_apply(&source, &target);

    assert_eq!(
        json(&target.join(".claude.json")),
        serde_json::json!({"hasCompletedOnboarding": true, "projects": {"/home/user/app": {}}})
    );
}

#[test]
fn apply_is_a_noop_once_declared_keys_already_match() {
    let source = temp_dir("overlay-noop-source");
    let target = temp_dir("overlay-noop-target");

    fs::write(source.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    fs::write(
        target.join(".claude.json"),
        br#"{"hasCompletedOnboarding": true, "custom": "formatting kept"}"#,
    )
    .unwrap();

    run_apply(&source, &target);

    // Untouched: byte-identical to what was there before, not reformatted.
    assert_eq!(
        fs::read(target.join(".claude.json")).unwrap(),
        br#"{"hasCompletedOnboarding": true, "custom": "formatting kept"}"#
    );
}

#[test]
fn diff_shows_no_drift_when_declared_key_matches_regardless_of_other_keys() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("overlay-diff-clean-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    git::commit(&seed, "seed claude.json overlay").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("overlay-diff-clean-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("overlay-diff-clean-target");
    fs::write(
        target.join(".claude.json"),
        br#"{"hasCompletedOnboarding": true, "sessions": [1, 2, 3]}"#,
    )
    .unwrap();

    assert_eq!(run_diff(&source, &target), "");
}

#[test]
fn diff_shows_drift_when_target_value_disagrees_with_declared() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("overlay-diff-drift-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    git::commit(&seed, "seed claude.json overlay").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("overlay-diff-drift-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("overlay-diff-drift-target");
    fs::write(target.join(".claude.json"), br#"{"hasCompletedOnboarding": false}"#).unwrap();

    assert_eq!(run_diff(&source, &target), ".claude.json\ttarget\n");
}

#[test]
fn diff_shows_drift_when_target_file_does_not_exist_yet() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("overlay-diff-missing-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    git::commit(&seed, "seed claude.json overlay").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("overlay-diff-missing-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("overlay-diff-missing-target");

    assert_eq!(run_diff(&source, &target), ".claude.json\ttarget\n");
}

#[test]
fn save_is_rejected_for_an_overlay_enforced_target() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("overlay-save-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    git::commit(&seed, "seed claude.json overlay").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("overlay-save-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("overlay-save-target");
    run_apply(&source, &target);

    // Hand-edit drift on the enforced key.
    fs::write(target.join(".claude.json"), br#"{"hasCompletedOnboarding": false}"#).unwrap();

    let output = run_save_or_reset("save", &source, &target, "y\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(".claude.json.overlay"), "expected message pointing at the overlay file: {stderr}");

    // Nothing was written into Source's overlay.
    assert_eq!(
        fs::read_to_string(source.join(".claude.json.overlay")).unwrap(),
        r#"{"hasCompletedOnboarding": true}"#
    );
}

#[test]
fn reset_discards_drift_by_re_enforcing_the_declared_value() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let seed = temp_dir("overlay-reset-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();
    git::commit(&seed, "seed claude.json overlay").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("overlay-reset-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("overlay-reset-target");
    run_apply(&source, &target);

    // Hand-edit drift on the enforced key, alongside an unrelated key mysh doesn't own.
    fs::write(
        target.join(".claude.json"),
        br#"{"hasCompletedOnboarding": false, "unrelated": "kept"}"#,
    )
    .unwrap();

    let output = run_save_or_reset("reset", &source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(
        json(&target.join(".claude.json")),
        serde_json::json!({"hasCompletedOnboarding": true, "unrelated": "kept"})
    );
}

#[test]
fn overlay_file_itself_is_never_copied_verbatim_into_target() {
    let source = temp_dir("overlay-no-literal-copy-source");
    let target = temp_dir("overlay-no-literal-copy-target");

    fs::write(source.join(".claude.json.overlay"), br#"{"hasCompletedOnboarding": true}"#).unwrap();

    run_apply(&source, &target);

    assert!(!target.join(".claude.json.overlay").exists());
}
