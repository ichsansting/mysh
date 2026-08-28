//! Step definitions for every phrase used in features/. Grouped by vocabulary:
//! world setup (source/target/remote fixtures), running commands, and assertions
//! on files, output, the Application Log, and the stub call records.

use crate::{TEST_PASSPHRASE, World, write_executable, write_file};
use cucumber::{given, then, when};
use mysh::domain::package;
use mysh::infra::crypto;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

// =========================================================================
// Given: source, target, remote fixtures
// =========================================================================

#[given(expr = "a bare remote")]
fn bare_remote(w: &mut World) {
    w.init_bare_remote();
}

#[given(expr = "a bare remote whose profile contains file {string} with content {string}")]
fn bare_remote_with_profile(w: &mut World, rel: String, content: String) {
    w.init_bare_remote();
    write_file(
        &w.source.join("profile").join(&rel),
        unescape(&content).as_bytes(),
    );
    w.commit_source();
    w.push_source();
}

#[given(expr = "source file {string} with content {string}")]
fn source_file(w: &mut World, rel: String, content: String) {
    write_file(&w.source_path(&rel), unescape(&content).as_bytes());
}

#[given(expr = "source file {string} with content {string} committed and pushed")]
fn source_file_pushed(w: &mut World, rel: String, content: String) {
    write_file(&w.source_path(&rel), unescape(&content).as_bytes());
    w.commit_source();
    w.push_source();
}

#[given(expr = "source file {string} with content {string} committed but not pushed")]
fn source_file_committed(w: &mut World, rel: String, content: String) {
    write_file(&w.source_path(&rel), unescape(&content).as_bytes());
    w.commit_source();
}

#[given(expr = "source file {string} is edited to {string} and committed but not pushed")]
fn source_file_edited_committed(w: &mut World, rel: String, content: String) {
    write_file(&w.source_path(&rel), unescape(&content).as_bytes());
    w.commit_source();
}

#[given(expr = "the source is committed and pushed")]
fn source_committed_and_pushed(w: &mut World) {
    w.commit_source();
    w.push_source();
}

#[given(expr = "another device pushed file {string} with content {string}")]
fn another_device_pushed(w: &mut World, rel: String, content: String) {
    w.push_from_another_device(&rel, &unescape(&content));
}

#[given(expr = "source directory {string} tracked with an empty {string} marker")]
fn tracked_dir_empty_marker(w: &mut World, dir: String, _marker: String) {
    write_file(&w.source_path(&dir).join(".track"), b"");
}

#[given(expr = "source directory {string} tracked with a {string} marker containing {string}")]
fn tracked_dir_marker_with_patterns(w: &mut World, dir: String, _marker: String, patterns: String) {
    write_file(
        &w.source_path(&dir).join(".track"),
        format!("{patterns}\n").as_bytes(),
    );
}

#[given(expr = "source secret {string} encrypting {string}")]
fn source_secret(w: &mut World, rel: String, plaintext: String) {
    let path = w.source_path(&rel);
    let envelope = crypto::encrypt(plaintext.as_bytes(), TEST_PASSPHRASE, &path).unwrap();
    write_file(&path, &envelope);
}

#[given(expr = "source secret {string} encrypting {string} committed and pushed")]
fn source_secret_pushed(w: &mut World, rel: String, plaintext: String) {
    source_secret(w, rel, plaintext);
    w.commit_source();
    w.push_source();
}

#[given(expr = "source fragment {string} with content {string}")]
fn source_fragment(w: &mut World, rel: String, content: String) {
    write_file(&w.source_path(&rel), unescape(&content).as_bytes());
}

#[given(expr = "source fragment secret {string} encrypting {string}")]
fn source_fragment_secret(w: &mut World, rel: String, plaintext: String) {
    source_secret(w, rel, plaintext);
}

#[given(regex = r#"^source overlay "([^"]*)" declaring (.+)$"#)]
fn source_overlay(w: &mut World, rel: String, declared: String) {
    write_file(&w.source_path(&rel), declared.as_bytes());
}

#[given(expr = "source eager shim {string} for specifier {string}")]
fn source_eager_shim(w: &mut World, rel: String, specifier: String) {
    let bin = Path::new(&rel)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_executable(
        &w.source_path(&rel),
        package::shim_script(&specifier, &bin, true).as_bytes(),
    );
}

#[given(expr = "source lazy shim {string} for specifier {string}")]
fn source_lazy_shim(w: &mut World, rel: String, specifier: String) {
    let bin = Path::new(&rel)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_executable(
        &w.source_path(&rel),
        package::shim_script(&specifier, &bin, false).as_bytes(),
    );
}

#[given(expr = "target file {string} already exists with content {string}")]
fn target_file_pre_existing(w: &mut World, rel: String, content: String) {
    write_file(&w.target_path(&rel), unescape(&content).as_bytes());
}

#[given(expr = "target file {string} is hand-edited to {string}")]
fn target_file_hand_edited(w: &mut World, rel: String, content: String) {
    write_file(&w.target_path(&rel), unescape(&content).as_bytes());
}

#[given(expr = "target file {string} is deleted")]
fn target_file_deleted(w: &mut World, rel: String) {
    fs::remove_file(w.target_path(&rel)).unwrap();
}

#[given(expr = "target directory {string} exists containing file {string} with {string}")]
fn target_dir_with_file(w: &mut World, dir: String, file: String, content: String) {
    write_file(
        &w.target_path(&dir).join(&file),
        unescape(&content).as_bytes(),
    );
}

#[given(expr = "target file {string} accumulates key {string} with value {int}")]
fn target_json_accumulates(w: &mut World, rel: String, key: String, value: i64) {
    let path = w.target_path(&rel);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert(key, serde_json::Value::from(value));
    fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
}

// =========================================================================
// Given: stubs and environment
// =========================================================================

#[given(expr = "a stubbed mise on PATH")]
fn stubbed_mise(w: &mut World) {
    w.stub_mise();
}

#[given(expr = "real fish resolvable on PATH")]
fn real_fish(w: &mut World) {
    w.stub_fish();
}

#[given(expr = "no mise resolvable on PATH")]
fn no_mise_on_path(w: &mut World) {
    w.hide_real_mise = true;
}

#[given(expr = "a stubbed curl that installs a recording mise")]
fn curl_installs_mise(w: &mut World) {
    w.curl_delivers_recording_mise();
}

#[given(expr = "a stubbed curl that downloads the real mysh binary")]
fn curl_downloads_mysh(w: &mut World) {
    w.curl_delivers_real_mysh();
}

#[given(expr = "a mysh binary is installed with content {string}")]
fn mysh_binary_installed(w: &mut World, content: String) {
    write_executable(&w.target_path(".mysh/bin/mysh"), content.as_bytes());
}

#[given(expr = "a mysh binary is installed matching the real compiled binary")]
fn mysh_binary_installed_matching_real(w: &mut World) {
    let content = fs::read(env!("CARGO_BIN_EXE_mysh")).unwrap();
    write_executable(&w.target_path(".mysh/bin/mysh"), &content);
}

#[given(expr = "an rc file with content {string}")]
fn rc_file(w: &mut World, content: String) {
    write_file(&w.rc_file.clone(), unescape(&content).as_bytes());
}

// =========================================================================
// Given / When: running commands
// =========================================================================

#[given(expr = "I ran {string}")]
fn ran(w: &mut World, cmdline: String) {
    w.run(&cmdline, Some("y\n"));
    w.assert_success();
}

#[when(expr = "I run {string}")]
fn run(w: &mut World, cmdline: String) {
    w.run(&cmdline, None);
}

#[given(expr = "I ran {string} answering {string}")]
#[when(expr = "I run {string} answering {string}")]
fn run_answering(w: &mut World, cmdline: String, answer: String) {
    w.run(&cmdline, Some(&format!("{answer}\n")));
}

#[when(expr = "I invoke the rendered shim {string} with argument {string}")]
fn invoke_shim(w: &mut World, rel: String, arg: String) {
    let output = Command::new(w.target_path(&rel))
        .arg(&arg)
        .env("PATH", w.child_path())
        .env("HOME", &w.target)
        .stdin(Stdio::null())
        .output()
        .expect("failed to invoke rendered shim");
    w.last = Some(output);
}

fn bootstrap(w: &mut World) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("bootstrap.sh");
    let remote = w.remote.as_ref().expect("bootstrap needs a bare remote");
    let output = Command::new("sh")
        .arg(script)
        .env("PATH", w.child_path())
        .env("HOME", &w.target)
        .env("MYSH_REMOTE_URL", format!("file://{}", remote.display()))
        .env("MYSH_TARGET_DIR", &w.target)
        .env("MYSH_RC_FILE", &w.rc_file)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run bootstrap.sh");
    w.last = Some(output);
}

#[when(expr = "I bootstrap the device")]
fn bootstrap_when(w: &mut World) {
    bootstrap(w);
}

#[given(expr = "I bootstrapped the device")]
fn bootstrapped(w: &mut World) {
    bootstrap(w);
    w.assert_success();
}

#[when(expr = "I run every documented command with no flags and HOME set to the target")]
fn run_every_command_zero_flag(w: &mut World) {
    let mysh = w.target_path(".mysh/bin/mysh");
    assert!(
        mysh.is_file(),
        "bootstrap must have installed the binary first"
    );
    write_file(&w.target_path("zeroflag.txt"), b"zero flag fodder");
    let commands: &[(&str, &[&str], &str)] = &[
        ("apply", &[], ""),
        ("diff", &[], ""),
        ("save", &[], "n\n"),
        ("update", &[], "n\n"),
        ("add", &["zeroflag.txt"], ""),
        ("teardown", &[], "n\n"),
    ];
    for (name, args, answer) in commands {
        let mut child = Command::new(&mysh)
            .arg(name)
            .args(*args)
            .env_clear()
            .env("PATH", w.child_path())
            .env("HOME", &w.target)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn installed mysh");
        use std::io::Write as _;
        match child.stdin.take().unwrap().write_all(answer.as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => panic!("failed to write stdin: {e}"),
        }
        let output = child.wait_with_output().unwrap();
        w.batch.push((name.to_string(), output));
    }
}

// =========================================================================
// Given: snapshots
// =========================================================================

#[given(expr = "I record the state of the target tree")]
fn record_target_tree(w: &mut World) {
    let root = w.target.clone();
    w.snapshot_tree(root);
}

#[given(expr = "I record the state of the source tree")]
fn record_source_tree(w: &mut World) {
    let root = w.source.clone();
    w.snapshot_tree(root);
}

#[given(expr = "I record the state of the remote")]
fn record_remote(w: &mut World) {
    w.remote_snapshot = Some(w.remote_head());
}

// =========================================================================
// Then: exit status and output
// =========================================================================

#[then(expr = "it succeeds")]
fn succeeds(w: &mut World) {
    w.assert_success();
}

#[then(expr = "it fails")]
fn fails(w: &mut World) {
    assert!(
        !w.last_output().status.success(),
        "expected failure but it succeeded\nstdout: {}",
        w.stdout()
    );
}

#[then(expr = "it fails mentioning {string}")]
fn fails_mentioning(w: &mut World, needle: String) {
    fails(w);
    assert!(
        w.stderr().contains(&needle),
        "stderr does not mention {needle:?}: {}",
        w.stderr()
    );
}

#[then(expr = "the output reports no drift")]
fn output_no_drift(w: &mut World) {
    assert_eq!(w.stdout(), "clean\n", "expected clean diff");
}

fn has_line(w: &World, line: &str) -> bool {
    w.stdout().lines().any(|l| l == line)
}

#[then(expr = "the output reports target drift for {string}")]
fn output_target_drift(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\ttarget")),
        "no target drift line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports drift ahead of remote for {string}")]
fn output_ahead_drift(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\tahead")),
        "no ahead drift line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports drift behind remote for {string}")]
fn output_behind_drift(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\tbehind")),
        "no behind drift line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports diverged drift for {string}")]
fn output_diverged_drift(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\tdiverged")),
        "no diverged drift line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports no target drift")]
fn output_no_target_drift(w: &mut World) {
    assert!(
        !w.stdout().lines().any(|l| l.ends_with("\ttarget")),
        "unexpected target drift: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports no remote drift")]
fn output_no_remote_drift(w: &mut World) {
    assert!(
        !w.stdout().lines().any(|l| {
            l.ends_with("\tahead") || l.ends_with("\tbehind") || l.ends_with("\tdiverged")
        }),
        "unexpected remote drift: {}",
        w.stdout()
    );
}

#[then(expr = "the output flags {string} as new")]
fn output_flags_new(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\tnew")),
        "no new-file line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output flags {string} as missing")]
fn output_flags_missing(w: &mut World, rel: String) {
    assert!(
        has_line(w, &format!("{rel}\tmissing")),
        "no missing-file line for {rel}: {}",
        w.stdout()
    );
}

#[then(expr = "the output does not mention {string}")]
fn output_does_not_mention(w: &mut World, needle: String) {
    assert!(
        !w.stdout().contains(&needle),
        "output mentions {needle:?}: {}",
        w.stdout()
    );
}

#[then(expr = "the output reports nothing to save")]
fn output_nothing_to_save(w: &mut World) {
    assert_eq!(w.stdout(), "nothing to save\n");
}

#[then(expr = "the output reports nothing to update")]
fn output_nothing_to_update(w: &mut World) {
    assert_eq!(w.stdout(), "nothing to update\n");
}

#[then(expr = "the output does not contain ciphertext")]
fn output_no_ciphertext(w: &mut World) {
    let stdout = w.last_output().stdout.clone();
    for rel in age_files(&w.source, &w.source) {
        let envelope = fs::read(w.source_path(&rel)).unwrap();
        // The AEAD body (past salt+nonce) is what must never leak into output.
        let body = &envelope[40..];
        assert!(
            !contains_subslice(&stdout, body),
            "output contains raw ciphertext of {rel}"
        );
    }
}

#[then(expr = "each of them succeeds")]
fn batch_succeeds(w: &mut World) {
    assert!(!w.batch.is_empty(), "no batch of commands was run");
    for (name, output) in &w.batch {
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// =========================================================================
// Then: files on disk
// =========================================================================

#[then(expr = "target {string} contains exactly {string}")]
fn target_contains(w: &mut World, rel: String, content: String) {
    let actual = fs::read_to_string(w.target_path(&rel))
        .unwrap_or_else(|e| panic!("cannot read target {rel}: {e}"));
    assert_eq!(actual, unescape(&content), "target {rel} content mismatch");
}

#[then(expr = "target {string} does not exist")]
fn target_does_not_exist(w: &mut World, rel: String) {
    assert!(
        !w.target_path(&rel).exists(),
        "target {rel} should not exist"
    );
}

#[then(expr = "source {string} contains exactly {string}")]
#[then(expr = "source fragment {string} contains exactly {string}")]
fn source_contains(w: &mut World, rel: String, content: String) {
    let actual = fs::read_to_string(w.source_path(&rel))
        .unwrap_or_else(|e| panic!("cannot read source {rel}: {e}"));
    assert_eq!(actual, unescape(&content), "source {rel} content mismatch");
}

#[then(expr = "source {string} exists")]
fn source_exists(w: &mut World, rel: String) {
    assert!(w.source_path(&rel).exists(), "source {rel} should exist");
}

#[then(expr = "source {string} does not exist")]
fn source_does_not_exist(w: &mut World, rel: String) {
    assert!(
        !w.source_path(&rel).exists(),
        "source {rel} should not exist"
    );
}

#[then(expr = "source {string} does not contain the plaintext {string}")]
fn source_not_plaintext(w: &mut World, rel: String, plaintext: String) {
    let bytes = fs::read(w.source_path(&rel)).unwrap();
    assert!(
        !contains_subslice(&bytes, plaintext.as_bytes()),
        "source {rel} leaks plaintext"
    );
}

#[then(expr = "re-rendering {string} from source yields exactly {string}")]
fn rerender_yields(w: &mut World, rel: String, content: String) {
    let path = w.source_path(&format!("{rel}.age"));
    let envelope =
        fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let plaintext = crypto::decrypt(&envelope, TEST_PASSPHRASE, &path).unwrap();
    assert_eq!(String::from_utf8_lossy(&plaintext), unescape(&content));
}

#[then(expr = "target {string} has permissions {string}")]
fn target_permissions(w: &mut World, rel: String, mode: String) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let actual = fs::metadata(w.target_path(&rel))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let expected = u32::from_str_radix(&mode, 8).unwrap();
        assert_eq!(
            actual, expected,
            "target {rel} mode {actual:o} != {expected:o}"
        );
    }
}

#[then(expr = "target {string} is executable")]
fn target_executable(w: &mut World, rel: String) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(w.target_path(&rel))
            .unwrap()
            .permissions()
            .mode();
        assert!(
            mode & 0o111 != 0,
            "target {rel} is not executable (mode {mode:o})"
        );
    }
}

#[then(expr = "target {string} is byte-identical to its source shim")]
fn target_identical_to_source(w: &mut World, rel: String) {
    let source = fs::read(w.source_path(&rel)).unwrap();
    let target = fs::read(w.target_path(&rel)).unwrap();
    assert_eq!(source, target, "target {rel} differs from its source");
}

#[then(regex = r#"^target "([^"]*)" as JSON has "([^"]*)" equal to (.+)$"#)]
fn target_json_key(w: &mut World, rel: String, key: String, expected: String) {
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(w.target_path(&rel)).unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(&expected).unwrap();
    assert_eq!(
        json.get(&key).unwrap_or(&serde_json::Value::Null),
        &expected,
        "target {rel} key {key}"
    );
}

#[then(expr = "no file under the target changed")]
#[then(expr = "no file under the source changed")]
fn tree_unchanged(w: &mut World) {
    w.assert_tree_unchanged();
}

#[then(expr = "no mysh residue remains under the target")]
fn no_mysh_residue(w: &mut World) {
    let mysh_dir = w.target_path(".mysh");
    assert!(!mysh_dir.exists(), ".mysh residue remains under the target");
}

#[then(expr = "the mise data directory does not exist")]
fn mise_data_dir_gone(w: &mut World) {
    assert!(
        !w.target_path(".mysh/mise").exists(),
        "mise data dir remains"
    );
}

// =========================================================================
// Then: shims
// =========================================================================

#[then(expr = "the shim {string} invokes mise with specifier {string}")]
fn shim_specifier_is(w: &mut World, rel: String, specifier: String) {
    let content = fs::read_to_string(w.source_path(&rel)).unwrap();
    assert_eq!(
        package::shim_specifier(&content),
        Some(specifier.as_str()),
        "shim {rel}"
    );
}

#[then(expr = "the shim {string} contains no absolute device-specific path")]
fn shim_portable(w: &mut World, rel: String) {
    let content = fs::read_to_string(w.source_path(&rel)).unwrap();
    let tmp = w.tmp.0.to_string_lossy().into_owned();
    assert!(
        !content.contains(&tmp),
        "shim {rel} bakes in the device path"
    );
    assert!(
        content.contains("$HOME"),
        "shim {rel} must resolve $HOME at run time"
    );
}

#[then(expr = "the shim {string} is marked eager")]
fn shim_eager(w: &mut World, rel: String) {
    let content = fs::read_to_string(w.source_path(&rel)).unwrap();
    assert!(
        package::is_eager(&content),
        "shim {rel} lacks the eager marker"
    );
}

#[then(expr = "the shim {string} is not marked eager")]
fn shim_not_eager(w: &mut World, rel: String) {
    let content = fs::read_to_string(w.source_path(&rel)).unwrap();
    assert!(
        !package::is_eager(&content),
        "shim {rel} unexpectedly eager"
    );
}

// =========================================================================
// Then: Application Log
// =========================================================================

#[then(expr = "the log records full ownership of {string} with no backup")]
fn log_full_no_backup(w: &mut World, rel: String) {
    let entries = w.log_target_entries(&rel);
    assert_eq!(
        entries.len(),
        1,
        "expected one log entry for {rel}: {}",
        w.log_text()
    );
    assert_eq!(entries[0][1], "full");
    assert_eq!(
        entries[0].len(),
        3,
        "expected no backup field: {:?}",
        entries[0]
    );
}

#[then(expr = "the log records full ownership of {string} with a backup")]
fn log_full_with_backup(w: &mut World, rel: String) {
    let entries = w.log_target_entries(&rel);
    assert_eq!(
        entries.len(),
        1,
        "expected one log entry for {rel}: {}",
        w.log_text()
    );
    assert_eq!(entries[0][1], "full");
    assert_eq!(
        entries[0].len(),
        4,
        "expected a backup field: {:?}",
        entries[0]
    );
}

#[then(expr = "the log records partial ownership of {string}")]
fn log_partial(w: &mut World, rel: String) {
    let entries = w.log_target_entries(&rel);
    assert_eq!(
        entries.len(),
        1,
        "expected one log entry for {rel}: {}",
        w.log_text()
    );
    assert_eq!(entries[0][1], "partial");
}

#[then(expr = "the backup for {string} contains exactly {string}")]
fn backup_contains(w: &mut World, rel: String, content: String) {
    let entries = w.log_target_entries(&rel);
    let backup_rel = entries
        .first()
        .and_then(|fields| fields.get(3))
        .unwrap_or_else(|| panic!("no backup recorded for {rel}: {}", w.log_text()));
    let actual = fs::read_to_string(w.target_path(backup_rel)).unwrap();
    assert_eq!(actual, unescape(&content), "backup content for {rel}");
}

#[then(expr = "the log has exactly one entry for {string}")]
fn log_one_entry(w: &mut World, rel: String) {
    assert_eq!(w.log_target_entries(&rel).len(), 1, "log: {}", w.log_text());
}

#[then(expr = "the log records the mise bootstrap")]
fn log_mise_bootstrap(w: &mut World) {
    assert!(
        w.log_text()
            .lines()
            .any(|l| l.starts_with("mise-bootstrapped\t")),
        "no mise-bootstrapped entry: {}",
        w.log_text()
    );
}

#[then(expr = "the summary says {string} is left in place")]
fn summary_left_in_place(w: &mut World, rel: String) {
    let stdout = w.stdout();
    assert!(
        stdout
            .lines()
            .any(|l| l.contains("leave") && l.contains(&rel)),
        "no leave-in-place summary line for {rel}: {stdout}"
    );
}

// =========================================================================
// Then: remote
// =========================================================================

#[then(expr = "the remote's latest commit contains {string} with {string}")]
fn remote_commit_contains(w: &mut World, rel: String, content: String) {
    let actual = w.remote_file(&rel);
    assert_eq!(
        String::from_utf8_lossy(&actual),
        unescape(&content),
        "remote {rel}"
    );
}

#[then(expr = "the remote's latest commit contains {string}")]
fn remote_commit_contains_path(w: &mut World, rel: String) {
    assert!(
        w.remote_has_path(&rel),
        "remote main does not contain {rel}"
    );
}

#[then(expr = "the remote is unchanged")]
fn remote_unchanged(w: &mut World) {
    let before = w
        .remote_snapshot
        .as_ref()
        .expect("no remote snapshot recorded");
    assert_eq!(&w.remote_head(), before, "remote HEAD moved");
}

// =========================================================================
// Then: stub call records
// =========================================================================

#[then(expr = "the curl stub was invoked exactly once")]
fn curl_once(w: &mut World) {
    assert_eq!(w.stub_calls("curl.calls").len(), 1);
}

#[then(expr = "the curl stub was never invoked")]
fn curl_never(w: &mut World) {
    assert_eq!(w.stub_calls("curl.calls").len(), 0);
}

#[then(expr = "the installed mysh binary matches the real compiled binary")]
fn installed_mysh_matches_real(w: &mut World) {
    let installed = fs::read(w.target_path(".mysh/bin/mysh")).unwrap();
    let real = fs::read(env!("CARGO_BIN_EXE_mysh")).unwrap();
    assert_eq!(installed, real, "installed mysh binary was not refreshed");
}

#[then(expr = "the mysh-owned mise binary exists")]
fn owned_mise_exists(w: &mut World) {
    assert!(
        w.target_path(".mysh/bin/mise").is_file(),
        "no mise at .mysh/bin/mise"
    );
}

#[then(expr = "the mysh-owned mise binary does not exist")]
fn owned_mise_absent(w: &mut World) {
    assert!(
        !w.target_path(".mysh/bin/mise").exists(),
        "unexpected mise at .mysh/bin/mise"
    );
}

fn install_lines(w: &World) -> Vec<String> {
    w.stub_calls("mise.calls")
        .into_iter()
        .filter(|l| l.starts_with("install"))
        .collect()
}

#[then(expr = "the mise stub saw exactly one install invocation")]
fn mise_one_install(w: &mut World) {
    assert_eq!(
        install_lines(w).len(),
        1,
        "calls: {:?}",
        w.stub_calls("mise.calls")
    );
}

#[then(expr = "that install invocation named specifiers {string} and {string}")]
fn install_named(w: &mut World, a: String, b: String) {
    let lines = install_lines(w);
    let line = lines.first().expect("no install invocation recorded");
    let words: Vec<&str> = line.split_whitespace().collect();
    assert!(words.contains(&a.as_str()), "{line} lacks {a}");
    assert!(words.contains(&b.as_str()), "{line} lacks {b}");
}

#[then(expr = "that install invocation did not name {string}")]
fn install_not_named(w: &mut World, spec: String) {
    let lines = install_lines(w);
    let line = lines.first().expect("no install invocation recorded");
    let words: Vec<&str> = line.split_whitespace().collect();
    assert!(
        !words.contains(&spec.as_str()),
        "{line} unexpectedly names {spec}"
    );
}

#[then(
    expr = "the mise stub saw an x invocation for specifier {string} running {string} with {string}"
)]
fn mise_x_invocation(w: &mut World, spec: String, bin: String, arg: String) {
    let expected = format!("x {spec} -- {bin} {arg}");
    let calls = w.stub_calls("mise.calls");
    assert!(
        calls.iter().any(|l| l == &expected),
        "no call {expected:?} in {calls:?}"
    );
}

// =========================================================================
// Then / Given: bootstrap and rc file
// =========================================================================

#[then(expr = "the mysh binary is installed under the target")]
fn mysh_installed(w: &mut World) {
    let path = w.target_path(".mysh/bin/mysh");
    assert!(path.is_file(), "no mysh binary at {}", path.display());
}

fn rc_text(w: &World) -> String {
    fs::read_to_string(&w.rc_file).unwrap_or_default()
}

fn path_line(w: &World, rel: &str) -> String {
    format!("export PATH=\"{}/{rel}:$PATH\"", w.target.display())
}

#[then(expr = "the rc file adds {string} to PATH")]
fn rc_adds_path(w: &mut World, rel: String) {
    assert!(
        rc_text(w).contains(&path_line(w, &rel)),
        "rc file: {}",
        rc_text(w)
    );
}

#[then(expr = "the rc file adds {string} to PATH exactly once")]
fn rc_adds_path_once(w: &mut World, rel: String) {
    let line = path_line(w, &rel);
    let count = rc_text(w).matches(&line).count();
    assert_eq!(
        count,
        1,
        "PATH line appears {count} times in rc: {}",
        rc_text(w)
    );
}

#[then(expr = "the rc file exports MISE_DATA_DIR pointing at {string}")]
fn rc_exports_data_dir(w: &mut World, rel: String) {
    let line = format!("export MISE_DATA_DIR=\"{}/{rel}\"", w.target.display());
    assert!(rc_text(w).contains(&line), "rc file: {}", rc_text(w));
}

#[then(expr = "the rc file contains exactly {string}")]
fn rc_contains_exactly(w: &mut World, content: String) {
    assert_eq!(
        rc_text(w).trim_end(),
        unescape(&content).trim_end(),
        "rc file not restored"
    );
}

#[then(expr = "the log records the bootstrap install and the PATH addition")]
fn log_bootstrap_entries(w: &mut World) {
    let log = w.log_text();
    assert!(
        log.lines().any(|l| l.starts_with("bootstrap-installed\t")),
        "log: {log}"
    );
    assert!(
        log.lines().any(|l| l.starts_with("bootstrap-path-added\t")),
        "log: {log}"
    );
}

#[then(expr = "the log records the bootstrap install exactly once")]
fn log_bootstrap_once(w: &mut World) {
    let count = w
        .log_text()
        .lines()
        .filter(|l| l.starts_with("bootstrap-installed\t"))
        .count();
    assert_eq!(count, 1, "log: {}", w.log_text());
}

#[then(expr = "the source checkout contains only {string}")]
fn source_checkout_only(w: &mut World, entry: String) {
    let checkout = w.target_path(".mysh/source");
    let mut entries: Vec<String> = fs::read_dir(&checkout)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".git")
        .collect();
    entries.sort();
    assert_eq!(entries, vec![entry], "sparse checkout leaked extra entries");
}

#[then(expr = "the bootstrap script's default release download points at {string}")]
fn bootstrap_default_releases_repo(w: &mut World, repo: String) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("bootstrap.sh");
    let text = fs::read_to_string(script).unwrap();
    assert!(
        text.contains(&format!("MYSH_RELEASES_REPO:-{repo}")),
        "bootstrap.sh default releases repo is not {repo}"
    );
    let _ = w;
}

// =========================================================================
// Release script
// =========================================================================

/// A pristine clone of this repo's HEAD wired to a throwaway bare origin, so
/// release.sh's tag pushes never touch the real repository.
fn release_checkout(w: &mut World) -> std::path::PathBuf {
    if let Some(dir) = &w.release_dir {
        return dir.clone();
    }
    let bare = w.tmp.0.join("release-origin.git");
    let checkout = w.tmp.0.join("release");
    w.git(&[
        "init",
        "-q",
        "--bare",
        "--initial-branch=main",
        bare.to_str().unwrap(),
    ]);
    w.git(&[
        "clone",
        "-q",
        env!("CARGO_MANIFEST_DIR"),
        checkout.to_str().unwrap(),
    ]);
    w.git(&[
        "-C",
        checkout.to_str().unwrap(),
        "remote",
        "set-url",
        "origin",
        bare.to_str().unwrap(),
    ]);
    w.git(&[
        "-C",
        checkout.to_str().unwrap(),
        "push",
        "-q",
        "origin",
        "HEAD:main",
    ]);
    w.release_dir = Some(checkout.clone());
    checkout
}

fn write_release_stubs(w: &World, cargo_zigbuild_works: bool) {
    let gh_calls = w.stub.join("gh.calls");
    let released = w.stub.join("gh-released");
    // release.sh's `mise exec <tools> -- "$0" "$@"` needs to actually run the
    // wrapped command (the script re-invoking itself) for the rest of the run
    // to happen at all — the tools it'd install are these same stubs, already
    // on PATH directly, so the stub only has to thread the command through.
    write_executable(
        &w.stub.join("mise"),
        b"#!/bin/sh\nif [ \"$1\" = \"exec\" ]; then\n  shift\n  while [ \"$#\" -gt 0 ] && [ \"$1\" != \"--\" ]; do shift; done\n  shift\n  exec \"$@\"\nfi\nexit 0\n",
    );
    write_executable(
        &w.stub.join("rustup"),
        b"#!/bin/sh\nif [ \"$1 $2\" = \"target list\" ]; then\n  echo x86_64-unknown-linux-musl\n  echo aarch64-unknown-linux-musl\n  echo aarch64-apple-darwin\nfi\nexit 0\n",
    );
    write_executable(&w.stub.join("zig"), b"#!/bin/sh\nexit 0\n");
    let cargo = if cargo_zigbuild_works {
        // `cargo zigbuild --release --target T` fakes the build artifact in cwd.
        "#!/bin/sh\nif [ \"$1\" = \"zigbuild\" ]; then\n  for a in \"$@\"; do :; done\n  target=\"\"\n  prev=\"\"\n  for a in \"$@\"; do\n    [ \"$prev\" = \"--target\" ] && target=\"$a\"\n    prev=\"$a\"\n  done\n  if [ -n \"$target\" ]; then\n    mkdir -p \"target/$target/release\"\n    echo fake-binary > \"target/$target/release/mysh\"\n  fi\nfi\nexit 0\n"
            .to_string()
    } else {
        "#!/bin/sh\n[ \"$1\" = \"zigbuild\" ] && exit 1\nexit 0\n".to_string()
    };
    write_executable(&w.stub.join("cargo"), cargo.as_bytes());
    let gh = format!(
        "#!/bin/sh\necho \"$*\" >> \"{calls}\"\nif [ \"$1 $2\" = \"release view\" ]; then\n  [ -f \"{released}\" ] || exit 1\nfi\nif [ \"$1 $2\" = \"release create\" ]; then\n  touch \"{released}\"\nfi\nexit 0\n",
        calls = gh_calls.display(),
        released = released.display(),
    );
    write_executable(&w.stub.join("gh"), gh.as_bytes());
}

fn run_release(w: &mut World) {
    let checkout = release_checkout(w);
    let output = Command::new("bash")
        .arg(checkout.join("release.sh"))
        .current_dir(&checkout)
        .env("PATH", w.child_path())
        .env("GIT_AUTHOR_NAME", "bdd")
        .env("GIT_AUTHOR_EMAIL", "bdd@test")
        .env("GIT_COMMITTER_NAME", "bdd")
        .env("GIT_COMMITTER_EMAIL", "bdd@test")
        .stdin(Stdio::null())
        .output()
        .expect("failed to run release.sh");
    let name = format!("release-{}", w.batch.len());
    w.batch.push((name, output.clone()));
    w.last = Some(output);
}

#[given(expr = "stubbed build and gh tools that record their invocations")]
fn release_stubs(w: &mut World) {
    write_release_stubs(w, true);
}

#[given(expr = "a repo checkout with an uncommitted change")]
fn release_dirty_checkout(w: &mut World) {
    write_release_stubs(w, true);
    let checkout = release_checkout(w);
    write_file(&checkout.join("dirty.txt"), b"uncommitted");
}

#[given(expr = "a PATH without the cross-build toolchain")]
fn release_missing_tool(w: &mut World) {
    write_release_stubs(w, false);
}

#[when(expr = "I run the release script")]
fn run_release_once(w: &mut World) {
    run_release(w);
}

#[when(expr = "I run the release script twice")]
fn run_release_twice(w: &mut World) {
    run_release(w);
    run_release(w);
}

#[then(expr = "both runs succeed")]
fn release_both_succeed(w: &mut World) {
    let releases: Vec<_> = w
        .batch
        .iter()
        .filter(|(n, _)| n.starts_with("release-"))
        .collect();
    assert_eq!(releases.len(), 2);
    for (name, output) in releases {
        assert!(
            output.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[then(expr = "the first run creates the {string} release and the second updates it")]
fn release_create_then_update(w: &mut World, tag: String) {
    let calls = w.stub_calls("gh.calls");
    let create = calls
        .iter()
        .position(|l| l.starts_with(&format!("release create {tag}")));
    let edit = calls
        .iter()
        .position(|l| l.starts_with(&format!("release edit {tag}")));
    let create = create.unwrap_or_else(|| panic!("no release create in {calls:?}"));
    let edit = edit.unwrap_or_else(|| panic!("no release edit in {calls:?}"));
    assert!(create < edit, "create must precede edit: {calls:?}");
}

// =========================================================================
// helpers
// =========================================================================

/// Cucumber's {string} parameter keeps backslash escapes literally; undo the
/// one that matters for quoted JSON content in feature files.
fn unescape(s: &str) -> String {
    s.replace("\\\"", "\"")
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Every `.age` file under `dir`, source-relative.
fn age_files(root: &Path, dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if path.is_dir() {
            found.extend(age_files(root, &path));
        } else if path.extension().is_some_and(|e| e == "age") {
            found.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    found
}
