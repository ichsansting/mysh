mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::{temp_dir, write_executable};

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
    fs::write(seed.join(relative), content).unwrap();
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

/// The real `PATH` (bootstrap.sh needs a real `git`), with `stub_dir` prepended so the
/// fake `curl` shadows any real one — the "only `git` present" bare environment from
/// the ticket, minus a real network fetch.
fn bare_env_path(stub_dir: &Path) -> String {
    format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap())
}

fn run_bootstrap(target: &Path, remote_url: &str, stub_dir: &Path, rc_file: &Path) -> std::process::Output {
    Command::new("sh")
        .arg(bootstrap_sh())
        .env("PATH", bare_env_path(stub_dir))
        .env("MYSH_REMOTE_URL", remote_url)
        .env("MYSH_RELEASES_REPO", "test-owner/mysh")
        .env("MYSH_TARGET_DIR", target)
        .env("MYSH_RC_FILE", rc_file)
        .output()
        .expect("failed to run bootstrap.sh")
}

#[test]
fn bootstrap_installs_binary_adds_path_clones_source_and_hands_off() {
    let target = temp_dir("bootstrap-target");
    let stub_dir = temp_dir("bootstrap-stub");
    let rc_file = target.join("rcfile");
    let remote = init_bare_remote_with_file("bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_curl(&stub_dir);

    let output = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Downloaded the matching prebuilt binary, from mysh's own releases.
    let curl_calls = fs::read_to_string(stub_dir.join("curl.calls")).unwrap();
    assert!(
        curl_calls.contains("https://github.com/test-owner/mysh/releases/latest/download/mysh-"),
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

    // rc file actually got the PATH line.
    let rc_text = fs::read_to_string(&rc_file).unwrap();
    assert!(rc_text.contains(&target.join(".mysh/bin").display().to_string()), "rc file was: {rc_text:?}");

    // Source cloned from the same repo, with the seeded content present.
    assert_eq!(
        fs::read(target.join(".mysh/source/bashrc")).unwrap(),
        b"export EDITOR=vim\n"
    );

    // Handed off to the binary to bootstrap mise and run the initial apply.
    let mysh_calls = fs::read_to_string(stub_dir.join("mysh.calls")).unwrap();
    assert!(mysh_calls.contains("apply"), "mysh calls were: {mysh_calls:?}");
    assert!(mysh_calls.contains("--source-dir"), "mysh calls were: {mysh_calls:?}");
    assert!(mysh_calls.contains("--target-dir"), "mysh calls were: {mysh_calls:?}");
}

#[test]
fn rerunning_bootstrap_does_not_duplicate_the_path_line_or_log_entries() {
    let target = temp_dir("bootstrap-target-rerun");
    let stub_dir = temp_dir("bootstrap-stub-rerun");
    let rc_file = target.join("rcfile");
    let remote = init_bare_remote_with_file("bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_curl(&stub_dir);

    let first = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));

    let second = run_bootstrap(&target, &remote_url, &stub_dir, &rc_file);
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));

    let rc_text = fs::read_to_string(&rc_file).unwrap();
    assert_eq!(
        rc_text.matches("export PATH=").count(),
        1,
        "rerunning bootstrap must not duplicate the PATH line: {rc_text:?}"
    );

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert_eq!(
        log_text.matches("bootstrap-installed").count(),
        1,
        "rerunning bootstrap must not duplicate the log entry: {log_text:?}"
    );
    assert_eq!(
        log_text.matches("bootstrap-path-added").count(),
        1,
        "rerunning bootstrap must not duplicate the log entry: {log_text:?}"
    );
}
