mod support;

use mysh::git;
use mysh::secret;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use support::{temp_dir, write_fake_mise};

const PASSPHRASE: &str = "correct horse battery staple";

fn run_add(source: &Path, target: &Path, extra_args: &[&str], stdin_answer: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mysh"));
    cmd.arg("add");
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.arg("--source-dir").arg(source);
    cmd.arg("--target-dir").arg(target);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("failed to run mysh add");
    if let Some(answer) = stdin_answer {
        // A command that errors before ever prompting may have already exited (and
        // closed its stdin) by the time this write happens — that's a legitimate
        // outcome, not a test failure, so a broken pipe here is ignored.
        let _ = child.stdin.take().unwrap().write_all(answer.as_bytes());
    }
    child.wait_with_output().unwrap()
}

#[test]
fn file_add_copies_untracked_target_content_into_source() {
    let source = temp_dir("add-file-source");
    let target = temp_dir("add-file-target");
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let output = run_add(&source, &target, &["bashrc"], None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=nano\n");
}

#[test]
fn file_add_leaves_a_subsequent_diff_showing_no_drift() {
    let remote = temp_dir("add-diff-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    let remote_url = format!("file://{}", remote.to_string_lossy());

    // A bare remote with no commits yet has no branch for a clone to track — seed one
    // so `source`'s clone has the `@{u}` upstream `diff` needs.
    let seed = temp_dir("add-diff-seed");
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(".gitkeep"), b"").unwrap();
    git::commit(&seed, "seed").unwrap();
    git::push(&seed).unwrap();

    let source = temp_dir("add-diff-source");
    git::clone(&remote_url, &source).unwrap();
    let target = temp_dir("add-diff-target");
    fs::write(target.join(".gitkeep"), b"").unwrap();
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let output = run_add(&source, &target, &["bashrc"], None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // `add` never commits/pushes, so the new path still shows up as remote drift (a
    // `save` candidate) — but critically, no *target* drift: Source and Target already
    // agree on its content, proving it's tracked correctly.
    let diff_output = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("diff")
        .arg("--source-dir")
        .arg(&source)
        .arg("--target-dir")
        .arg(&target)
        .output()
        .expect("failed to run mysh diff");
    assert!(diff_output.status.success(), "{}", String::from_utf8_lossy(&diff_output.stderr));
    assert_eq!(String::from_utf8(diff_output.stdout).unwrap(), "bashrc\tremote\n");
}

#[test]
fn file_add_on_already_tracked_path_errors_without_modifying_source() {
    let source = temp_dir("add-file-tracked-source");
    let target = temp_dir("add-file-tracked-target");
    fs::write(source.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    fs::write(target.join("bashrc"), b"export EDITOR=nano\n").unwrap();

    let output = run_add(&source, &target, &["bashrc"], None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("save"));
    assert_eq!(fs::read(source.join("bashrc")).unwrap(), b"export EDITOR=vim\n");
}

#[test]
fn file_add_secret_writes_age_suffixed_file_that_round_trips() {
    let source = temp_dir("add-secret-source");
    let target = temp_dir("add-secret-target");
    let plaintext = b"-----BEGIN OPENSSH PRIVATE KEY-----\nsuper secret\n";
    fs::write(target.join("id_rsa"), plaintext).unwrap();

    let output = run_add(&source, &target, &["id_rsa", "--secret", "--passphrase", PASSPHRASE], None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(!source.join("id_rsa").exists());
    let envelope = fs::read(source.join("id_rsa.age")).unwrap();
    assert_ne!(envelope, plaintext.to_vec());
    assert_eq!(secret::decrypt(&envelope, PASSPHRASE).unwrap(), plaintext);
}

#[test]
fn file_add_refuses_a_plain_copy_when_the_secret_variant_is_already_tracked() {
    let source = temp_dir("add-secret-collision-source");
    let target = temp_dir("add-secret-collision-target");
    let envelope = secret::encrypt(b"already tracked as a secret\n", PASSPHRASE).unwrap();
    fs::write(source.join("id_rsa.age"), &envelope).unwrap();
    fs::write(target.join("id_rsa"), b"plain content on disk\n").unwrap();

    let output = run_add(&source, &target, &["id_rsa"], None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("save"));
    // Neither the plain path was created nor the existing secret disturbed.
    assert!(!source.join("id_rsa").exists());
    assert_eq!(fs::read(source.join("id_rsa.age")).unwrap(), envelope);
}

#[test]
fn secret_flag_combined_with_a_directory_errors_and_writes_nothing() {
    let source = temp_dir("add-secret-dir-source");
    let target = temp_dir("add-secret-dir-target");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/a.conf"), b"a\n").unwrap();

    let output = run_add(&source, &target, &["plugins", "--secret", "--passphrase", PASSPHRASE], None);
    assert!(!output.status.success());
    assert!(!source.join("plugins").exists());
}

#[test]
fn secret_flag_combined_with_a_package_specifier_errors_and_writes_nothing() {
    let source = temp_dir("add-secret-pkg-source");
    let target = temp_dir("add-secret-pkg-target");

    let output = run_add(&source, &target, &["go@latest", "--secret", "--passphrase", PASSPHRASE], None);
    assert!(!output.status.success());
    assert!(!source.join(".mysh").exists());
    assert!(!source.join(".config/mise/config.toml").exists());
}

#[test]
fn folder_add_confirmed_creates_track_marker_and_copies_matched_files() {
    let source = temp_dir("add-folder-source");
    let target = temp_dir("add-folder-target");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/a.conf"), b"a\n").unwrap();
    fs::write(target.join("plugins/b.conf"), b"b\n").unwrap();

    let output = run_add(&source, &target, &["plugins"], Some("y\n"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(source.join("plugins/.track").is_file());
    assert_eq!(fs::read(source.join("plugins/a.conf")).unwrap(), b"a\n");
    assert_eq!(fs::read(source.join("plugins/b.conf")).unwrap(), b"b\n");
}

#[test]
fn folder_add_declined_leaves_source_completely_unchanged() {
    let source = temp_dir("add-folder-decline-source");
    let target = temp_dir("add-folder-decline-target");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/a.conf"), b"a\n").unwrap();

    let output = run_add(&source, &target, &["plugins"], Some("n\n"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!source.join("plugins").exists());
}

#[test]
fn folder_add_declined_on_a_nested_path_leaves_no_empty_parent_directories_behind() {
    let source = temp_dir("add-folder-decline-nested-source");
    let target = temp_dir("add-folder-decline-nested-target");
    fs::create_dir_all(target.join("config/nvim")).unwrap();
    fs::write(target.join("config/nvim/init.lua"), b"-- config\n").unwrap();

    let output = run_add(&source, &target, &["config/nvim"], Some("n\n"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    // Neither the leaf directory nor the parent `create_dir_all` had to create for it
    // survives a decline — Source ends up with zero new entries, not an empty `config/`.
    assert!(!source.join("config").exists());
}

#[test]
fn folder_add_ignore_pattern_excludes_matching_files_from_copy() {
    let source = temp_dir("add-folder-ignore-source");
    let target = temp_dir("add-folder-ignore-target");
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/keep.conf"), b"keep\n").unwrap();
    fs::write(target.join("plugins/skip.log"), b"skip\n").unwrap();

    let output = run_add(&source, &target, &["plugins", "--ignore", "*.log"], Some("y\n"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(source.join("plugins/keep.conf").is_file());
    assert!(!source.join("plugins/skip.log").exists());
    assert_eq!(fs::read_to_string(source.join("plugins/.track")).unwrap(), "*.log\n");
    // The excluded file never even shows up in the printed listing.
    assert!(!String::from_utf8_lossy(&output.stdout).contains("skip.log"));
}

#[test]
fn folder_add_on_already_tracked_directory_errors_without_modifying_source() {
    let source = temp_dir("add-folder-tracked-source");
    let target = temp_dir("add-folder-tracked-target");
    fs::create_dir_all(source.join("plugins")).unwrap();
    fs::write(source.join("plugins/.track"), b"").unwrap();
    fs::create_dir_all(target.join("plugins")).unwrap();
    fs::write(target.join("plugins/a.conf"), b"a\n").unwrap();

    let output = run_add(&source, &target, &["plugins"], Some("y\n"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("save"));
    assert!(!source.join("plugins/a.conf").exists());
}

fn run_add_with_path(source: &Path, target: &Path, extra_args: &[&str], path_env: &str) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mysh"));
    cmd.arg("add");
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.arg("--source-dir").arg(source);
    cmd.arg("--target-dir").arg(target);
    cmd.env("PATH", path_env);
    cmd.output().expect("failed to run mysh add")
}

#[test]
fn package_add_defaults_to_lazy_and_writes_a_real_portable_shim_file() {
    let source = temp_dir("add-pkg-source");
    let target = temp_dir("add-pkg-target");

    let output = run_add(&source, &target, &["go@latest"], None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let shim = source.join(".mysh/bin/go");
    assert_eq!(
        fs::read_to_string(&shim).unwrap(),
        "#!/bin/sh\nexport MISE_DATA_DIR=\"$HOME/.mysh/mise\"\nexec mise x go@latest -- go \"$@\"\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&shim).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "shim must be executable");
    }
}

#[test]
fn package_add_lazy_honors_bin_override() {
    let source = temp_dir("add-pkg-bin-source");
    let target = temp_dir("add-pkg-bin-target");

    let output = run_add(&source, &target, &["github:elio-fm/elio@latest", "--bin", "elio-cli"], None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(!source.join(".mysh/bin/elio").exists());
    assert_eq!(
        fs::read_to_string(source.join(".mysh/bin/elio-cli")).unwrap(),
        "#!/bin/sh\nexport MISE_DATA_DIR=\"$HOME/.mysh/mise\"\nexec mise x github:elio-fm/elio@latest -- elio-cli \"$@\"\n"
    );
}

#[test]
fn package_add_eager_declares_via_mise_config_set_and_touches_only_source() {
    let source = temp_dir("add-pkg-eager-source");
    let target = temp_dir("add-pkg-eager-target");
    let stub_dir = temp_dir("add-pkg-eager-stub");
    write_fake_mise(&stub_dir);
    let path_env = format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap());

    let output = run_add_with_path(&source, &target, &["github:elio-fm/elio@latest", "--eager"], &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(
        fs::read_to_string(source.join(".config/mise/config.toml")).unwrap(),
        "\n[tools]\n\"github:elio-fm/elio\" = \"latest\"\n"
    );
    // `add` never touches Target — no `.mysh` state, no rendered config, nothing.
    assert!(!target.join(".mysh").exists());
    assert!(!target.join(".config").exists());
}

#[test]
fn package_add_bin_flag_combined_with_eager_errors() {
    let source = temp_dir("add-pkg-eager-bin-source");
    let target = temp_dir("add-pkg-eager-bin-target");
    let stub_dir = temp_dir("add-pkg-eager-bin-stub");
    write_fake_mise(&stub_dir);
    let path_env = format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap());

    let output = run_add_with_path(&source, &target, &["go@latest", "--eager", "--bin", "go2"], &path_env);
    assert!(!output.status.success());
    assert!(!source.join(".config/mise/config.toml").exists());
}

#[test]
fn package_add_eager_without_a_resolvable_mise_errors_clearly() {
    let source = temp_dir("add-pkg-no-mise-source");
    let target = temp_dir("add-pkg-no-mise-target");
    // No fake `mise` on `PATH` at all, and none previously bootstrapped in `target`.
    let path_env = std::env::var("PATH")
        .unwrap()
        .split(':')
        .filter(|dir| !Path::new(dir).join("mise").exists())
        .collect::<Vec<_>>()
        .join(":");

    let output = run_add_with_path(&source, &target, &["go@latest", "--eager"], &path_env);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("apply"));
    assert!(!source.join(".config/mise/config.toml").exists());
}

#[test]
fn package_add_on_duplicate_lazy_specifier_errors_without_modifying_source() {
    let source = temp_dir("add-pkg-dup-source");
    let target = temp_dir("add-pkg-dup-target");

    let first = run_add(&source, &target, &["go@latest"], None);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let before = fs::read_to_string(source.join(".mysh/bin/go")).unwrap();

    let output = run_add(&source, &target, &["go@latest", "--bin", "go2"], None);
    assert!(!output.status.success());
    assert!(!source.join(".mysh/bin/go2").exists());
    assert_eq!(fs::read_to_string(source.join(".mysh/bin/go")).unwrap(), before);
}
