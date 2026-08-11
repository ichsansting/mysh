mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::{bare_env_path, temp_dir, write_executable};

fn bootstrap_sh() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bootstrap.sh")
}

fn init_bare_remote_with_file(relative: &str, content: &[u8]) -> std::path::PathBuf {
    let remote = temp_dir("bootstrap-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());

    let seed = temp_dir("bootstrap-seed");
    let remote_url = format!("file://{}", remote.to_string_lossy());
    Command::new("git").args(["clone", &remote_url]).arg(&seed).status().unwrap();
    let dest = seed.join(relative);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&dest, content).unwrap();
    Command::new("git").current_dir(&seed).args(["add", "-A"]).status().unwrap();
    Command::new("git")
        .current_dir(&seed)
        .args(["-c", "user.email=t@t.com", "-c", "user.name=t", "commit", "-m", "seed"])
        .status()
        .unwrap();
    Command::new("git").current_dir(&seed).args(["push"]).status().unwrap();

    remote
}

/// A fake `curl` that, instead of hitting the real network, writes a fake `mysh`
/// executable to the `-o` destination — simulating "download the prebuilt binary"
/// without a real GitHub Releases fetch. The fake `mysh` records every invocation
/// (proving the bootstrap hand-off actually happened) to `stub_dir/mysh.calls`.
fn write_fake_curl(stub_dir: &Path) {
    let script = format!(
        r#"#!/bin/sh
echo "$@" >> "{stub}/curl.calls"
out=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-o" ]; then
    out="$arg"
  fi
  prev="$arg"
done
cat > "$out" <<'MYSH_STUB_EOF'
#!/bin/sh
echo "$@" >> "{stub}/mysh.calls"
exit 0
MYSH_STUB_EOF
chmod +x "$out"
"#,
        stub = stub_dir.display()
    );
    write_executable(&stub_dir.join("curl"), &script);
}

/// Runs the real `bootstrap.sh` (bootstrap.sh needs a real `git`) against a bare,
/// stub-tool-only `PATH` (the "only `git` present" environment from the ticket, minus a
/// real network fetch) — `releases_repo: None` leaves `MYSH_RELEASES_REPO` unset
/// entirely, to test bootstrap.sh's own default rather than overriding it.
fn run_bootstrap(
    target: &Path,
    remote_url: &str,
    stub_dir: &Path,
    rc_file: &Path,
    releases_repo: Option<&str>,
) -> std::process::Output {
    let mut cmd = Command::new("sh");
    cmd.arg(bootstrap_sh())
        .env("PATH", bare_env_path(stub_dir))
        .env("MYSH_REMOTE_URL", remote_url)
        .env("MYSH_TARGET_DIR", target)
        .env("MYSH_RC_FILE", rc_file);
    match releases_repo {
        Some(repo) => cmd.env("MYSH_RELEASES_REPO", repo),
        None => cmd.env_remove("MYSH_RELEASES_REPO"),
    };
    cmd.output().expect("failed to run bootstrap.sh")
}

#[test]
fn bootstrap_installs_binary_adds_path_clones_source_and_hands_off() {
    let target = temp_dir("bootstrap-target");
    let stub_dir = temp_dir("bootstrap-stub");
    let rc_file = target.join("rcfile");
    let remote = init_bare_remote_with_file("profile/bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_curl(&stub_dir);

    let output = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file, Some("test-owner/mysh"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Downloaded the matching prebuilt binary, from mysh's own releases.
    let curl_calls = fs::read_to_string(stub_dir.join("curl.calls")).unwrap();
    assert!(
        curl_calls.contains("https://github.com/test-owner/mysh/releases/latest/download/mysh-"),
        "curl calls were: {curl_calls:?}"
    );
    // The asset name is honest about being musl-linked (static, self-contained) on
    // Linux — release.sh actually builds musl, not glibc.
    #[cfg(target_os = "linux")]
    assert!(
        curl_calls.contains("-unknown-linux-musl"),
        "curl calls were: {curl_calls:?}"
    );

    // Binary placed at the isolated, mysh-owned bin dir and made executable.
    let install_path = target.join(".mysh/bin/mysh");
    assert!(install_path.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&install_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "installed binary must be executable");
    }

    // PATH addition and binary install both recorded in the Application Log.
    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert!(
        log_text.contains(&format!("bootstrap-installed\t{}\n", install_path.display())),
        "log was: {log_text:?}"
    );
    assert!(log_text.contains("bootstrap-path-added\t"), "log was: {log_text:?}");

    // rc file actually got the PATH line and the MISE_DATA_DIR line.
    let rc_text = fs::read_to_string(&rc_file).unwrap();
    assert!(rc_text.contains(&target.join(".mysh/bin").display().to_string()), "rc file was: {rc_text:?}");
    assert!(
        rc_text.contains(&format!("export MISE_DATA_DIR=\"{}\"", target.join(".mysh/mise").display())),
        "rc file was: {rc_text:?}"
    );

    // Source cloned from the same repo, with the seeded content present.
    assert_eq!(
        fs::read(target.join(".mysh/source/profile/bashrc")).unwrap(),
        b"export EDITOR=vim\n"
    );

    // Handed off to the binary to bootstrap mise and run the initial apply.
    let mysh_calls = fs::read_to_string(stub_dir.join("mysh.calls")).unwrap();
    assert!(mysh_calls.contains("apply"), "mysh calls were: {mysh_calls:?}");
    assert!(mysh_calls.contains("--source-dir"), "mysh calls were: {mysh_calls:?}");
    assert!(mysh_calls.contains("--target-dir"), "mysh calls were: {mysh_calls:?}");
}

#[test]
fn bootstrap_defaults_to_mysh_own_releases_repo_with_no_env_override() {
    let target = temp_dir("bootstrap-target-default-repo");
    let stub_dir = temp_dir("bootstrap-stub-default-repo");
    let rc_file = target.join("rcfile");
    let remote = init_bare_remote_with_file("profile/bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_curl(&stub_dir);

    // No MYSH_RELEASES_REPO set — must fall back to the real mysh repo, not the
    // CHANGE_ME placeholder, so a real post-bootstrap.sh device can actually find it.
    let output = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file, None);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let curl_calls = fs::read_to_string(stub_dir.join("curl.calls")).unwrap();
    assert!(
        curl_calls.contains("https://github.com/ichsansting/mysh/releases/latest/download/mysh-"),
        "curl calls were: {curl_calls:?}"
    );
}

#[test]
fn rerunning_bootstrap_does_not_duplicate_the_path_line_or_log_entries() {
    let target = temp_dir("bootstrap-target-rerun");
    let stub_dir = temp_dir("bootstrap-stub-rerun");
    let rc_file = target.join("rcfile");
    let remote = init_bare_remote_with_file("profile/bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_curl(&stub_dir);

    let first = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file, Some("test-owner/mysh"));
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));

    let second = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file, Some("test-owner/mysh"));
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));

    let rc_text = fs::read_to_string(&rc_file).unwrap();
    assert_eq!(
        rc_text.matches("export PATH=").count(),
        1,
        "rerunning bootstrap must not duplicate the PATH line: {rc_text:?}"
    );
    assert_eq!(
        rc_text.matches("export MISE_DATA_DIR=").count(),
        1,
        "rerunning bootstrap must not duplicate the MISE_DATA_DIR line: {rc_text:?}"
    );

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert_eq!(
        log_text.matches("bootstrap-installed").count(),
        1,
        "rerunning bootstrap must not duplicate the log entry: {log_text:?}"
    );
    // One entry each for the PATH line and the MISE_DATA_DIR line — two independent
    // idempotency guards (see bootstrap.sh), not duplicated by the rerun.
    assert_eq!(
        log_text.matches("bootstrap-path-added").count(),
        2,
        "rerunning bootstrap must not duplicate either rc-file line's log entry: {log_text:?}"
    );
}
