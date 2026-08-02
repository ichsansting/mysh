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
    let remote = temp_dir("secret-remote");
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
    let device = temp_dir("secret-other-device");
    git::clone(remote_url, &device).unwrap();
    let path = device.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
    git::commit(&device, "update from another device").unwrap();
    git::push(&device).unwrap();
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

#[cfg(unix)]
fn mode_bits(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn apply_decrypts_secret_and_writes_restrictive_permissions() {
    let source = temp_dir("secret-apply-source");
    let target = temp_dir("secret-apply-target");

    let plaintext = b"-----BEGIN OPENSSH PRIVATE KEY-----\nsuper secret\n";
    let envelope = secret::encrypt(plaintext, PASSPHRASE).unwrap();
    fs::create_dir_all(source.join("ssh")).unwrap();
    fs::write(source.join("ssh/id_rsa.age"), &envelope).unwrap();

    run_apply(&source, &target);

    let rendered = target.join("ssh/id_rsa");
    assert_eq!(fs::read(&rendered).unwrap(), plaintext);
    assert!(!target.join("ssh/id_rsa.age").exists());

    #[cfg(unix)]
    assert_eq!(mode_bits(&rendered), 0o600, "decrypted secret must not be group/world accessible");
}

#[test]
fn diff_compares_decrypted_source_against_decrypted_target_plaintext() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let plaintext = b"token=abc123\n";
    let envelope = secret::encrypt(plaintext, PASSPHRASE).unwrap();
    push_from_another_device(&remote_url, "creds.age", &envelope);

    let source = temp_dir("secret-diff-source");
    git::clone(&remote_url, &source).unwrap();

    // Target holds the same plaintext (as if already applied): no drift, even though
    // the raw ciphertext bytes on Source obviously differ from Target's plaintext —
    // proving the comparison decrypts rather than comparing ciphertext to plaintext.
    let target = temp_dir("secret-diff-target");
    fs::write(target.join("creds"), plaintext).unwrap();
    assert_eq!(run_diff(&source, &target), "");

    // A live edit on the decrypted Target file is real drift.
    fs::write(target.join("creds"), b"token=edited\n").unwrap();
    assert_eq!(run_diff(&source, &target), "creds\ttarget\n");
}

#[test]
fn save_on_edited_secret_reencrypts_into_source() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let original = b"token=original\n";
    push_from_another_device(&remote_url, "creds.age", &secret::encrypt(original, PASSPHRASE).unwrap());

    let source = temp_dir("secret-save-source");
    git::clone(&remote_url, &source).unwrap();

    let edited = b"token=edited-locally\n";
    let target = temp_dir("secret-save-target");
    fs::write(target.join("creds"), edited).unwrap();

    let output = run_save_or_reset("save", &source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Source's .age file holds a freshly re-encrypted envelope of the edited plaintext,
    // never plaintext itself.
    let source_envelope = fs::read(source.join("creds.age")).unwrap();
    assert_ne!(source_envelope, edited.to_vec());
    assert_eq!(secret::decrypt(&source_envelope, PASSPHRASE).unwrap(), edited);

    // And it was pushed to Remote.
    let check = temp_dir("secret-save-remote-check");
    git::clone(&remote_url, &check).unwrap();
    let remote_envelope = fs::read(check.join("creds.age")).unwrap();
    assert_eq!(secret::decrypt(&remote_envelope, PASSPHRASE).unwrap(), edited);
}

#[test]
fn reset_on_secret_redecrypts_source_into_target() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let original = b"token=original\n";
    push_from_another_device(&remote_url, "creds.age", &secret::encrypt(original, PASSPHRASE).unwrap());

    let source = temp_dir("secret-reset-source");
    git::clone(&remote_url, &source).unwrap();

    // Local drift on Target that reset must discard.
    let target = temp_dir("secret-reset-target");
    fs::write(target.join("creds"), b"token=locally-drifted\n").unwrap();

    let output = run_save_or_reset("reset", &source, &target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let rendered = target.join("creds");
    assert_eq!(fs::read(&rendered).unwrap(), original);

    #[cfg(unix)]
    assert_eq!(mode_bits(&rendered), 0o600);
}
