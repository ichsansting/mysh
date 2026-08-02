mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::temp_dir;

#[cfg(unix)]
fn write_executable(path: &Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The real `PATH`, minus any directory that already has a `mise` binary in it — the
/// dev/CI machine running this test may well have `mise` installed for its own use,
/// which would defeat the "mise absent" tests below if left on `PATH`.
fn real_path_without_mise() -> String {
    std::env::var("PATH")
        .unwrap()
        .split(':')
        .filter(|dir| !Path::new(dir).join("mise").exists())
        .collect::<Vec<_>>()
        .join(":")
}

fn stub_path_env(stub_dir: &Path) -> String {
    format!("{}:{}", stub_dir.display(), real_path_without_mise())
}

fn run_apply(source: &Path, target: &Path, path_env: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mysh"))
        .arg("apply")
        .arg("--source-dir")
        .arg(source)
        .arg("--target-dir")
        .arg(target)
        .env("PATH", path_env)
        .output()
        .expect("failed to run mysh apply")
}

/// A fake `curl` that, instead of hitting the real network, writes a fake `mise`
/// executable into `stub_dir` (already on `PATH`) — simulating `mise`'s official
/// `curl -fsSL https://mise.run | sh` installer without any real download.
fn write_fake_curl(stub_dir: &Path) {
    let script = format!(
        r#"#!/bin/sh
cat > "{stub}/mise" <<'MISE'
#!/bin/sh
echo "$@" >> "{stub}/mise.calls"
case "$1" in
  --version) exit 0 ;;
esac
exit 0
MISE
chmod +x "{stub}/mise"
"#,
        stub = stub_dir.display()
    );
    write_executable(&stub_dir.join("curl"), &script);
}

/// A fake `mise` that's already "installed": `--version` succeeds, and `install
/// <specifier>` records the call and drops a runnable stub binary for the specifier's
/// default bin name, so a test can prove the package is runnable after `apply`.
fn write_fake_mise(stub_dir: &Path) {
    let script = format!(
        r#"#!/bin/sh
echo "$@" >> "{stub}/mise.calls"
case "$1" in
  --version) exit 0 ;;
  install)
    name=$(echo "$2" | sed -e 's/^[^:]*://' -e 's/@.*//' -e 's#.*/##')
    cat > "{stub}/$name" <<BIN
#!/bin/sh
echo ran-$name
BIN
    chmod +x "{stub}/$name"
    ;;
esac
exit 0
"#,
        stub = stub_dir.display()
    );
    write_executable(&stub_dir.join("mise"), &script);
}

#[test]
fn apply_bootstraps_missing_mise_and_logs_it() {
    let source = temp_dir("package-source-bootstrap");
    let target = temp_dir("package-target-bootstrap");
    let stub_dir = temp_dir("package-stub-bootstrap");

    fs::write(source.join(".packages"), "widget@1.0\teager\n").unwrap();
    write_fake_curl(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert!(log_text.contains("mise-bootstrapped\n"), "log was: {log_text:?}");

    assert!(stub_dir.join("mise").exists(), "bootstrap must install a `mise` binary");

    let mise_calls = fs::read_to_string(stub_dir.join("mise.calls")).unwrap();
    assert!(
        mise_calls.contains("install widget@1.0"),
        "eager package must be installed right after bootstrap, calls were: {mise_calls:?}"
    );
}

#[test]
fn apply_does_not_touch_mise_when_no_eager_packages_are_declared() {
    let source = temp_dir("package-source-none");
    let target = temp_dir("package-target-none");
    let stub_dir = temp_dir("package-stub-none");
    // No `curl`/`mise` stub at all: if apply tried to touch mise, the real (absent from
    // this PATH) binary would fail to spawn and the command would error out.

    fs::write(source.join(".packages"), "widget@1.0\tlazy\n").unwrap();

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!target.join(".mysh/log").exists());
}

#[test]
fn apply_installs_eager_package_via_mise_and_it_is_runnable() {
    let source = temp_dir("package-source-eager");
    let target = temp_dir("package-target-eager");
    let stub_dir = temp_dir("package-stub-eager");

    fs::write(source.join(".packages"), "widget@1.0\teager\n").unwrap();
    write_fake_mise(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let mise_calls = fs::read_to_string(stub_dir.join("mise.calls")).unwrap();
    assert!(mise_calls.contains("install widget@1.0"), "calls were: {mise_calls:?}");

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap_or_default();
    // Already-present `mise` must never be re-bootstrapped.
    assert!(!log_text.contains("mise-bootstrapped"));
    // The Application Log must record the install so `teardown` can later reverse it.
    assert!(log_text.contains("package-installed\twidget@1.0\n"), "log was: {log_text:?}");

    let installed_bin = stub_dir.join("widget");
    assert!(installed_bin.exists(), "package must be installed and runnable after apply");
    let run = Command::new(&installed_bin).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ran-widget");
}

#[test]
fn dot_packages_file_is_never_rendered_to_target() {
    let source = temp_dir("package-source-not-rendered");
    let target = temp_dir("package-target-not-rendered");

    fs::write(source.join(".packages"), "widget@1.0\tlazy\n").unwrap();

    let output = run_apply(&source, &target, &std::env::var("PATH").unwrap());
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!target.join(".packages").exists());
}

#[test]
fn a_nested_dot_packages_file_is_an_ordinary_file_not_metadata() {
    let source = temp_dir("package-source-nested");
    let target = temp_dir("package-target-nested");

    // Only the top-level `.packages` is mysh metadata; the same name nested in a
    // subdirectory is just an ordinary file a user happens to have.
    fs::create_dir_all(source.join("sub")).unwrap();
    fs::write(source.join("sub/.packages"), b"not mysh metadata\n").unwrap();

    let output = run_apply(&source, &target, &std::env::var("PATH").unwrap());
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read(target.join("sub/.packages")).unwrap(), b"not mysh metadata\n");
}
