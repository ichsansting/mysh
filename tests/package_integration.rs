mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::{temp_dir, write_executable};

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

/// The body shared by every fake `mise`: `--version` succeeds, and `install <specifier>`
/// records the call (to `stub_dir/mise.calls`) and drops a runnable stub binary for the
/// specifier's default bin name, so a test can prove the package is runnable after
/// `apply`.
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
echo ran-$name
BIN
    chmod +x "{stub}/$name"
    ;;
  x)
    shift
    specifier="$1"; shift
    shift # drop --
    bin_name="$1"; shift
    # Real `mise x` only installs on the first invocation of a given tool@version;
    # a marker file stands in for that already-installed check here.
    if [ ! -f "{stub}/$bin_name.installed" ]; then
      touch "{stub}/$bin_name.installed"
      echo "installed $specifier" >> "{stub}/mise.calls"
    fi
    echo "ran-$bin_name $*"
    ;;
esac
exit 0
"#,
        stub = stub_dir.display()
    )
}

/// A fake `mise` that's already "installed" and on `PATH`.
fn write_fake_mise(stub_dir: &Path) {
    write_executable(&stub_dir.join("mise"), &mise_stub_script(stub_dir));
}

/// A fake `curl` that, instead of hitting the real network, writes a fake `mise`
/// executable to `$MISE_INSTALL_PATH` — simulating the official `curl -fsSL
/// https://mise.run | sh` installer (which does exactly this: write one binary to that
/// path, nothing else) without any real download. `stub_dir` is only where the fake
/// `mise`'s own bookkeeping (`mise.calls`, installed-tool stubs) lives — independent of
/// wherever `MISE_INSTALL_PATH` ends up.
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

#[test]
fn apply_bootstraps_missing_mise_and_logs_it() {
    let source = temp_dir("package-source-bootstrap");
    let target = temp_dir("package-target-bootstrap");
    let stub_dir = temp_dir("package-stub-bootstrap");

    fs::write(source.join(".packages"), "widget@1.0\teager\n").unwrap();
    write_fake_curl(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Bootstrap must land at a deterministic, mysh-owned path (not wherever the
    // installer's own default happens to be) so `teardown` can delete exactly this file.
    let expected_mise_path = target.join(".mysh/bin/mise");
    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert!(
        log_text.contains(&format!("mise-bootstrapped\t{}\n", expected_mise_path.display())),
        "log was: {log_text:?}"
    );
    assert!(expected_mise_path.exists(), "bootstrap must install `mise` at the mysh-owned path");

    let mise_calls = fs::read_to_string(stub_dir.join("mise.calls")).unwrap();
    assert!(
        mise_calls.contains("install widget@1.0"),
        "eager package must be installed right after bootstrap, calls were: {mise_calls:?}"
    );
}

#[test]
fn second_apply_reuses_bootstrapped_mise_without_reinvoking_installer() {
    let source = temp_dir("package-source-idempotent");
    let target = temp_dir("package-target-idempotent");
    let stub_dir = temp_dir("package-stub-idempotent");

    fs::write(source.join(".packages"), "widget@1.0\teager\n").unwrap();
    write_fake_curl(&stub_dir);

    let first = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert!(target.join(".mysh/bin/mise").exists());

    // Remove the fake `curl`: with `mise` already bootstrapped at the deterministic
    // owned path from the first run, `ensure_installed` must resolve it directly and
    // never fall back to re-running the (now-missing) installer.
    fs::remove_file(stub_dir.join("curl")).unwrap();

    let second = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert_eq!(
        log_text.matches("mise-bootstrapped").count(),
        1,
        "must not re-bootstrap on a second apply, log was: {log_text:?}"
    );
}

#[test]
fn apply_does_not_touch_mise_when_no_packages_are_declared() {
    let source = temp_dir("package-source-none");
    let target = temp_dir("package-target-none");
    let stub_dir = temp_dir("package-stub-none");
    // No `.packages` file at all, and no `curl`/`mise` stub: if apply tried to touch
    // mise, the real (absent from this PATH) binary would fail to spawn and the command
    // would error out.

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!target.join(".mysh/log").exists());
}

#[test]
fn apply_bootstraps_mise_for_a_lazy_only_device_so_the_shim_has_something_to_invoke() {
    let source = temp_dir("package-source-lazy-bootstrap");
    let target = temp_dir("package-target-lazy-bootstrap");
    let stub_dir = temp_dir("package-stub-lazy-bootstrap");

    fs::write(source.join(".packages"), "widget@1.0\tlazy\n").unwrap();
    write_fake_curl(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let expected_mise_path = target.join(".mysh/bin/mise");
    assert!(
        expected_mise_path.exists(),
        "a lazy-only device must still bootstrap mise, or its shim would fail with \
         `mise: command not found` on first real invocation"
    );
    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap();
    assert!(
        log_text.contains(&format!("mise-bootstrapped\t{}\n", expected_mise_path.display())),
        "log was: {log_text:?}"
    );
    // Lazy packages are never installed during apply itself, only bootstrapped-for-later.
    assert!(!log_text.contains("package-installed"), "log was: {log_text:?}");

    // The shim must embed the exact resolved mise bin, not a bare `mise` that depends on
    // `.mysh/bin` already being on `PATH` by the time the shim actually runs.
    let shim_text = fs::read_to_string(target.join(".mysh/bin/widget")).unwrap();
    assert!(
        shim_text.contains(&format!("exec \"{}\"", expected_mise_path.display())),
        "shim was: {shim_text:?}"
    );
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
fn eager_package_shim_is_generated_and_reachable_on_the_bootstrap_path() {
    let source = temp_dir("package-source-eager-shim");
    let target = temp_dir("package-target-eager-shim");
    let stub_dir = temp_dir("package-stub-eager-shim");

    fs::write(source.join(".packages"), "widget@1.0\teager\n").unwrap();
    write_fake_mise(&stub_dir);
    let path_env = stub_path_env(&stub_dir);

    let output = run_apply(&source, &target, &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    // Reachable via the same shim mechanism lazy packages use (install-time coverage is
    // `apply_installs_eager_package_via_mise_and_it_is_runnable`), so a plain
    // command-name lookup against `.mysh/bin` (the directory `bootstrap.sh` puts on
    // `PATH`) resolves it, instead of only the install-time stub_dir the test harness
    // happens to control.
    let shim = target.join(".mysh/bin/widget");
    assert!(shim.exists(), "eager package must also get a shim, not just an install");
    let run = Command::new(&shim).env("PATH", &path_env).output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ran-widget");
}

#[test]
fn both_eager_and_lazy_packages_get_a_shim_after_apply() {
    let source = temp_dir("package-source-mixed-shims");
    let target = temp_dir("package-target-mixed-shims");
    let stub_dir = temp_dir("package-stub-mixed-shims");

    fs::write(source.join(".packages"), "widget@1.0\teager\nelio@1.0\tlazy\n").unwrap();
    write_fake_mise(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(target.join(".mysh/bin/widget").exists(), "eager package must get a shim");
    assert!(target.join(".mysh/bin/elio").exists(), "lazy package must get a shim");
}

#[test]
fn dot_packages_file_is_never_rendered_to_target() {
    let source = temp_dir("package-source-not-rendered");
    let target = temp_dir("package-target-not-rendered");
    let stub_dir = temp_dir("package-stub-not-rendered");

    fs::write(source.join(".packages"), "widget@1.0\tlazy\n").unwrap();
    write_fake_mise(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!target.join(".packages").exists());
}

#[test]
fn lazy_shim_is_generated_and_installs_then_execs_on_first_invocation() {
    let source = temp_dir("package-source-lazy");
    let target = temp_dir("package-target-lazy");
    let stub_dir = temp_dir("package-stub-lazy");

    fs::write(source.join(".packages"), "widget@1.0\tlazy\telio-cli\n").unwrap();

    // A stubbed, already-installed `mise` on `PATH` at apply time (the resolved-and-
    // bootstrapped case is covered separately by
    // `apply_bootstraps_mise_for_a_lazy_only_device_so_the_shim_has_something_to_invoke`).
    write_fake_mise(&stub_dir);
    let path_env = stub_path_env(&stub_dir);

    let output = run_apply(&source, &target, &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let shim = target.join(".mysh/bin/elio-cli");
    assert!(shim.exists(), "a shim must be generated at the lazy package's bin name");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&shim).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "shim must be executable");
    }

    // Lazy packages are never installed during apply itself.
    let mise_calls_after_apply = fs::read_to_string(stub_dir.join("mise.calls")).unwrap_or_default();
    assert!(
        !mise_calls_after_apply.contains("install widget"),
        "lazy package must not be installed during apply: {mise_calls_after_apply:?}"
    );

    let first = Command::new(&shim).env("PATH", &path_env).arg("--flag").output().unwrap();
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert_eq!(String::from_utf8_lossy(&first.stdout).trim(), "ran-elio-cli --flag");

    let second = Command::new(&shim).env("PATH", &path_env).arg("--flag").output().unwrap();
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));
    assert_eq!(String::from_utf8_lossy(&second.stdout).trim(), "ran-elio-cli --flag");

    let mise_calls = fs::read_to_string(stub_dir.join("mise.calls")).unwrap();
    assert!(
        mise_calls.contains("x widget@1.0 -- elio-cli --flag"),
        "shim must call `mise x <specifier> -- <bin_name> \"$@\"`, calls were: {mise_calls:?}"
    );
    assert_eq!(
        mise_calls.matches("installed widget@1.0").count(),
        1,
        "second invocation must reuse the already-installed tool, not re-trigger install: {mise_calls:?}"
    );
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
