mod support;

use mysh::git;
use std::fs;
use std::path::Path;
use std::process::Command;
use support::temp_dir;

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("diff-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    remote
}

/// Clones `remote_url`, writes `relative` with `content`, commits, and pushes —
/// simulating a commit made from some other device.
fn push_from_another_device(remote_url: &str, relative: &str, content: &[u8]) {
    let device = temp_dir("diff-other-device");
    git::clone(remote_url, &device).unwrap();
    let path = device.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
    git::commit(&device, "update from another device").unwrap();
    git::push(&device).unwrap();
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

#[test]
fn no_drift_anywhere_produces_clean_output() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-clean");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("diff-target-clean");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();

    assert_eq!(run_diff(&source, &target), "");
}

#[test]
fn reports_target_vs_source_drift_only() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-target-drift");
    git::clone(&remote_url, &source).unwrap();

    // A live edit on disk, never captured back into Source.
    let target = temp_dir("diff-target-target-drift");
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    assert_eq!(run_diff(&source, &target), "bashrc\ttarget\n");
}

#[test]
fn reports_source_vs_remote_drift_only() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-remote-drift");
    git::clone(&remote_url, &source).unwrap();

    // Target mirrors Source exactly (as if already applied).
    let target = temp_dir("diff-target-remote-drift");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();

    // Another device pushes a commit this Source hasn't pulled yet.
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=emacs\n");

    assert_eq!(run_diff(&source, &target), "bashrc\tremote\n");
}

#[test]
fn reports_remote_drift_for_a_file_not_yet_pushed() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-unpushed");
    git::clone(&remote_url, &source).unwrap();

    // A file that exists locally in Source (and has been applied to Target) but was
    // never pushed — Remote doesn't have it at all.
    fs::write(source.join("gitconfig"), b"[core]\n").unwrap();

    let target = temp_dir("diff-target-unpushed");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    fs::write(target.join("gitconfig"), b"[core]\n").unwrap();

    assert_eq!(run_diff(&source, &target), "gitconfig\tremote\n");
}

#[test]
fn reports_remote_drift_for_a_file_only_on_remote() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-remote-only");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("diff-target-remote-only");
    fs::write(target.join("bashrc"), b"export EDITOR=vim\n").unwrap();

    // Another device adds and pushes a brand-new file this Source never had.
    push_from_another_device(&remote_url, "newfile", b"new content\n");

    assert_eq!(run_diff(&source, &target), "newfile\tremote\n");
}

#[test]
fn track_marked_directory_flags_new_file_in_target_only() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "plugins/.track", b"");

    let source = temp_dir("diff-source-track-new");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("diff-target-track-new");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/.track"), b"").unwrap();
    // Shows up live on disk under the tracked directory, but was never added to Source.
    fs::write(target.join("plugins/new-plugin.conf"), b"enabled = true\n").unwrap();

    assert_eq!(run_diff(&source, &target), "plugins/new-plugin.conf\ttarget\n");
}

#[test]
fn track_marked_directory_flags_missing_file_absent_from_target() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "plugins/.track", b"");
    push_from_another_device(&remote_url, "plugins/existing.conf", b"enabled = true\n");

    let source = temp_dir("diff-source-track-missing");
    git::clone(&remote_url, &source).unwrap();

    // Target never got this file (e.g. deleted locally, or never applied).
    let target = temp_dir("diff-target-track-missing");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/.track"), b"").unwrap();

    assert_eq!(run_diff(&source, &target), "plugins/existing.conf\ttarget\n");
}

#[test]
fn track_ignore_pattern_excludes_matching_file_from_new_scan() {
    let remote = init_bare_remote();
    let track_content: &[u8] = b"*.log\nnotes/private.md\n.trash\n";
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "plugins/.track", track_content);

    let source = temp_dir("diff-source-track-ignore");
    git::clone(&remote_url, &source).unwrap();

    let target = temp_dir("diff-target-track-ignore");
    fs::create_dir_all(target.join("plugins/notes")).unwrap();
    fs::create_dir_all(target.join("plugins/.trash")).unwrap();
    fs::write(target.join("plugins/.track"), track_content).unwrap();
    // Basename wildcard (`*.log`): excluded.
    fs::write(target.join("plugins/debug.log"), b"noise\n").unwrap();
    // Slash-qualified full-path pattern: excluded.
    fs::write(target.join("plugins/notes/private.md"), b"secret notes\n").unwrap();
    // Directory-component pattern (`.trash`, no slash): excludes the whole subtree.
    fs::write(target.join("plugins/.trash/old.txt"), b"junk\n").unwrap();
    // Doesn't match any pattern: still surfaced as new.
    fs::write(target.join("plugins/notes/public.md"), b"shared notes\n").unwrap();

    assert_eq!(run_diff(&source, &target), "plugins/notes/public.md\ttarget\n");
}

#[test]
fn directory_without_track_never_scans_for_new_sibling_files() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "plugins/existing.conf", b"enabled = true\n");

    let source = temp_dir("diff-source-file-mode");
    git::clone(&remote_url, &source).unwrap();

    // No .track file here: file-mode tracking only, so a sibling file that showed up
    // on disk (but was never added to Source) must stay invisible to diff.
    let target = temp_dir("diff-target-file-mode");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/existing.conf"), b"enabled = true\n").unwrap();
    fs::write(target.join("plugins/untracked-sibling.conf"), b"whatever\n").unwrap();

    assert_eq!(run_diff(&source, &target), "");
}

#[test]
fn reports_both_drifts_together_distinguishing_sides() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=vim\n");

    let source = temp_dir("diff-source-both-drift");
    git::clone(&remote_url, &source).unwrap();

    // A live edit on disk, distinct from both Source and (soon) Remote.
    let target = temp_dir("diff-target-both-drift");
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    // Another device also pushes a commit this Source hasn't pulled yet.
    push_from_another_device(&remote_url, "bashrc", b"export EDITOR=emacs\n");

    assert_eq!(run_diff(&source, &target), "bashrc\ttarget,remote\n");
}
