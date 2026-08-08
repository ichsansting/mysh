mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use support::{temp_dir, write_executable};

fn run_teardown(target: &Path, answer: &str) -> std::process::Output {
    run_teardown_with_path(target, answer, &std::env::var("PATH").unwrap())
}

fn run_teardown_with_path(target: &Path, answer: &str, path_env: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("teardown")
        .arg("--target-dir")
        .arg(target)
        .env("PATH", path_env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run mysh teardown");
    use std::io::Write;
    child.stdin.take().unwrap().write_all(answer.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
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

#[test]
fn teardown_deletes_created_files_and_restores_overwritten_originals() {
    let source = temp_dir("teardown-source-files");
    let target = temp_dir("teardown-target-files");

    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("gitconfig"), b"pre-existing content\n").unwrap();
    fs::write(source.join("gitconfig"), b"[user]\n\tname = Test\n").unwrap();
    fs::write(source.join("newfile"), b"fresh content\n").unwrap();

    run_apply(&source, &target);
    assert_eq!(fs::read(target.join("gitconfig")).unwrap(), b"[user]\n\tname = Test\n");
    assert!(target.join("newfile").exists());

    let output = run_teardown(&target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8(output.stdout).unwrap().ends_with("torn down\n"));

    assert_eq!(
        fs::read(target.join("gitconfig")).unwrap(),
        b"pre-existing content\n",
        "overwritten file must be restored to its pre-mysh content"
    );
    assert!(!target.join("newfile").exists(), "created file must be deleted");
    assert!(!target.join(".mysh").exists(), "no mysh residue must remain");
}

#[test]
fn declined_teardown_leaves_target_and_log_unchanged() {
    let source = temp_dir("teardown-source-decline");
    let target = temp_dir("teardown-target-decline");

    fs::write(source.join("newfile"), b"fresh content\n").unwrap();
    run_apply(&source, &target);

    let output = run_teardown(&target, "n\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8(output.stdout).unwrap().ends_with("aborted\n"));

    assert!(target.join("newfile").exists());
    assert!(target.join(".mysh/log").exists());
}

#[test]
fn teardown_on_a_device_mysh_never_touched_is_a_noop() {
    let target = temp_dir("teardown-target-untouched");
    fs::create_dir_all(&target).unwrap();

    let output = run_teardown(&target, "");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "nothing to tear down\n");
}

/// Same fake `mise` body used by `package_integration.rs`: `--version` succeeds, and
/// `install <specifier>` records the call and drops a runnable stub binary.
fn mise_stub_script(stub_dir: &Path) -> String {
    format!(
        r#"#!/bin/sh
echo "$@" >> "{stub}/mise.calls"
case "$1" in
  --version) exit 0 ;;
  install)
    name=$(echo "$2" | sed -e 's/^[^:]*://' -e 's/@.*//' -e 's#.*/##')
    cat > "{stub}/$name" <<BIN
#!/bin/sh
exit 0
BIN
    chmod +x "{stub}/$name"
    ;;
esac
exit 0
"#,
        stub = stub_dir.display()
    )
}

/// A fake `curl` standing in for the real `curl -fsSL https://mise.run | sh` installer:
/// writes a fake `mise` to `$MISE_INSTALL_PATH` instead of hitting the network.
fn write_fake_curl(stub_dir: &Path) {
    let script = format!(
        r#"#!/bin/sh
install_path="${{MISE_INSTALL_PATH:-$HOME/.local/bin/mise}}"
mkdir -p "$(dirname "$install_path")"
cat > "$install_path" <<'MISE_STUB_EOF'
{inner}MISE_STUB_EOF
chmod +x "$install_path"
"#,
        inner = mise_stub_script(stub_dir)
    );
    write_executable(&stub_dir.join("curl"), &script);
}

fn real_path_without_mise() -> String {
    std::env::var("PATH")
        .unwrap()
        .split(':')
        .filter(|dir| !Path::new(dir).join("mise").exists())
        .collect::<Vec<_>>()
        .join(":")
}

#[test]
fn teardown_uninstalls_packages_removes_bootstrapped_mise_and_lazy_shims() {
    let source = temp_dir("teardown-source-packages");
    let target = temp_dir("teardown-target-packages");
    let stub_dir = temp_dir("teardown-stub-packages");

    // One eager (installed immediately, uninstalled via the data-dir wipe) and one lazy
    // package (never installed, but its shim — the only file it leaves behind — must
    // still disappear on teardown, even though no Application Log entry tracks it
    // individually; see mise::data_dir/`.mysh` doc comments for why that's sufficient).
    fs::write(source.join(".packages"), "widget@1.0\teager\ngadget@2.0\tlazy\n").unwrap();
    write_fake_curl(&stub_dir);
    let path_env = format!("{}:{}", stub_dir.display(), real_path_without_mise());

    let status = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("apply")
        .arg("--source-dir")
        .arg(&source)
        .arg("--target-dir")
        .arg(&target)
        .env("PATH", &path_env)
        .status()
        .unwrap();
    assert!(status.success());

    let owned_mise = target.join(".mysh/bin/mise");
    let lazy_shim = target.join(".mysh/bin/gadget");
    assert!(owned_mise.exists(), "setup must have bootstrapped an owned mise");
    assert!(lazy_shim.exists(), "setup must have generated the lazy package's shim");
    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert!(log_text.contains("package-installed\twidget@1.0\n"), "log was: {log_text:?}");

    let output = run_teardown_with_path(&target, "y\n", &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(!owned_mise.exists(), "mysh-bootstrapped mise must be removed");
    assert!(!lazy_shim.exists(), "the lazy package's shim must be removed even though it's untracked by the log");
    assert!(!target.join(".mysh/mise").exists(), "the isolated package data dir must be removed");
    assert!(!target.join(".mysh").exists(), "no mysh residue must remain");
}

fn init_bare_remote_with_files(files: &[(&str, &[u8])]) -> std::path::PathBuf {
    let remote = temp_dir("teardown-bootstrap-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());

    let seed = temp_dir("teardown-bootstrap-seed");
    let remote_url = format!("file://{}", remote.to_string_lossy());
    Command::new("git").args(["clone", &remote_url]).arg(&seed).status().unwrap();
    for (relative, content) in files {
        let dest = seed.join(relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&dest, content).unwrap();
    }
    Command::new("git").current_dir(&seed).args(["add", "-A"]).status().unwrap();
    Command::new("git")
        .current_dir(&seed)
        .args(["-c", "user.email=t@t.com", "-c", "user.name=t", "commit", "-m", "seed"])
        .status()
        .unwrap();
    Command::new("git").current_dir(&seed).args(["push"]).status().unwrap();

    remote
}

/// A fake `curl` that serves the *real* compiled `mysh` binary for `bootstrap.sh`'s
/// `-o`-flagged download, so the handed-off `apply` actually runs for real (no
/// `.packages` in this test, so it never needs to touch `mise`/a second `curl` shape).
fn write_fake_binary_download_curl(stub_dir: &Path, real_mysh: &Path) {
    let script = format!(
        r#"#!/bin/sh
out=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "-o" ]; then
    out="$arg"
  fi
  prev="$arg"
done
cp "{real_mysh}" "$out"
chmod +x "$out"
"#,
        real_mysh = real_mysh.display()
    );
    write_executable(&stub_dir.join("curl"), &script);
}

fn bootstrap_sh() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bootstrap.sh")
}

#[test]
fn full_bootstrap_to_teardown_cycle_leaves_no_residue() {
    let target = temp_dir("teardown-cycle-target");
    let stub_dir = temp_dir("teardown-cycle-stub");
    let rc_file = target.join("rcfile");
    fs::create_dir_all(&target).unwrap();
    fs::write(&rc_file, "export EDITOR=vim\n").unwrap();
    // Pre-existing content at a path Source also manages: apply must back this up.
    fs::write(target.join("gitconfig"), b"pre-existing content\n").unwrap();

    let remote = init_bare_remote_with_files(&[
        ("profile/gitconfig", b"[user]\n\tname = Test\n"),
        ("profile/newfile", b"fresh content\n"),
    ]);
    let remote_url = format!("file://{}", remote.to_string_lossy());

    write_fake_binary_download_curl(&stub_dir, Path::new(env!("CARGO_BIN_EXE_mysh")));
    let bootstrap_path_env = format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap());

    let bootstrap_output = Command::new("sh")
        .arg(bootstrap_sh())
        .env("PATH", &bootstrap_path_env)
        .env("MYSH_REMOTE_URL", &remote_url)
        .env("MYSH_RELEASES_REPO", "test-owner/mysh")
        .env("MYSH_TARGET_DIR", &target)
        .env("MYSH_RC_FILE", &rc_file)
        .output()
        .expect("failed to run bootstrap.sh");
    assert!(bootstrap_output.status.success(), "{}", String::from_utf8_lossy(&bootstrap_output.stderr));

    // Sanity-check the setup actually did what this test relies on before tearing down.
    assert_eq!(fs::read(target.join("gitconfig")).unwrap(), b"[user]\n\tname = Test\n");
    assert!(target.join("newfile").exists());
    let install_path = target.join(".mysh/bin/mysh");
    assert!(install_path.exists());
    let rc_text_after_bootstrap = fs::read_to_string(&rc_file).unwrap();
    assert!(rc_text_after_bootstrap.contains("export PATH="));

    let output = run_teardown(&target, "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(
        fs::read(target.join("gitconfig")).unwrap(),
        b"pre-existing content\n",
        "overwritten file must be restored to its pre-bootstrap content"
    );
    assert!(!target.join("newfile").exists(), "created file must be deleted");
    assert!(!install_path.exists(), "the bootstrap-installed mysh binary must be removed");

    let rc_text_after_teardown = fs::read_to_string(&rc_file).unwrap();
    assert_eq!(
        rc_text_after_teardown, "export EDITOR=vim\n",
        "the bootstrap installer's own PATH line and comment must be fully stripped, \
         restoring the rc file to its pre-bootstrap content"
    );

    assert!(!target.join(".mysh").exists(), "no mysh residue must remain anywhere under target");
}
