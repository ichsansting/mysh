mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use support::{temp_dir, write_executable, write_fake_curl, write_fake_mise};

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

/// Writes a real, portable lazy shim file directly into `source`'s mirrored shim dir —
/// standing in for what `add --lazy` would write (see ADR-0006), without going through
/// the CLI, matching how other fixtures in this file are set up directly.
fn write_lazy_shim(source: &Path, specifier: &str, bin_name: &str) {
    let dir = source.join(".mysh/bin");
    fs::create_dir_all(&dir).unwrap();
    let content = format!(
        "#!/bin/sh\nexport MISE_DATA_DIR=\"$HOME/.mysh/mise\"\nexec mise x {specifier} -- {bin_name} \"$@\"\n"
    );
    write_executable(&dir.join(bin_name), &content);
}

/// Writes a `[tools]`-only `config.toml` into `source` — standing in for what `add
/// --eager` would write via `mise config set` (see ADR-0006), without going through the
/// CLI or a real `mise` binary.
fn write_eager_declaration(source: &Path, name: &str, version: &str) {
    let dir = source.join(".config/mise");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.toml"), format!("[tools]\n{name} = \"{version}\"\n")).unwrap();
}

#[test]
fn apply_bootstraps_missing_mise_and_logs_it() {
    let source = temp_dir("package-source-bootstrap");
    let target = temp_dir("package-target-bootstrap");
    let stub_dir = temp_dir("package-stub-bootstrap");

    write_eager_declaration(&source, "widget", "1.0");
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
        mise_calls.contains("installed widget@1.0"),
        "eager package must be installed right after bootstrap, calls were: {mise_calls:?}"
    );
}

#[test]
fn second_apply_reuses_bootstrapped_mise_without_reinvoking_installer() {
    let source = temp_dir("package-source-idempotent");
    let target = temp_dir("package-target-idempotent");
    let stub_dir = temp_dir("package-stub-idempotent");

    write_eager_declaration(&source, "widget", "1.0");
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
    // No lazy shim files, no `[tools]` declaration, and no `curl`/`mise` stub: if apply
    // tried to touch mise, the real (absent from this PATH) binary would fail to spawn
    // and the command would error out.

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!target.join(".mysh/log").exists());
}

#[test]
fn apply_bootstraps_mise_for_a_lazy_only_device_so_the_shim_has_something_to_invoke() {
    let source = temp_dir("package-source-lazy-bootstrap");
    let target = temp_dir("package-target-lazy-bootstrap");
    let stub_dir = temp_dir("package-stub-lazy-bootstrap");

    write_lazy_shim(&source, "widget@1.0", "widget");
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
}

#[test]
fn lazy_shim_is_identity_copied_verbatim_and_stays_executable() {
    let source = temp_dir("package-source-lazy-copy");
    let target = temp_dir("package-target-lazy-copy");
    let stub_dir = temp_dir("package-stub-lazy-copy");

    write_lazy_shim(&source, "widget@1.0", "widget");
    write_fake_mise(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let source_shim = source.join(".mysh/bin/widget");
    let target_shim = target.join(".mysh/bin/widget");
    assert!(target_shim.exists(), "the lazy shim must be identity-copied into Target");
    assert_eq!(
        fs::read_to_string(&target_shim).unwrap(),
        fs::read_to_string(&source_shim).unwrap(),
        "a lazy shim is a real, portable file — copied verbatim, never regenerated"
    );
    // No mysh-generated shim content anywhere — the file's own `$HOME`/bare-`mise`
    // resolution is the whole mechanism now (see ADR-0006).
    assert!(!fs::read_to_string(&target_shim).unwrap().contains(&target.display().to_string()));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&target_shim).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "identity-copy must preserve the executable bit");
    }
}

#[test]
fn eager_package_is_installed_via_blanket_mise_install_and_exposed_via_mises_own_shim() {
    let source = temp_dir("package-source-eager");
    let target = temp_dir("package-target-eager");
    let stub_dir = temp_dir("package-stub-eager");

    write_eager_declaration(&source, "widget", "1.0");
    write_fake_mise(&stub_dir);
    let path_env = stub_path_env(&stub_dir);

    let output = run_apply(&source, &target, &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let mise_calls = fs::read_to_string(stub_dir.join("mise.calls")).unwrap();
    assert!(mise_calls.contains("install\n"), "must call blanket `mise install`, calls were: {mise_calls:?}");
    assert!(mise_calls.contains("installed widget@1.0"), "calls were: {mise_calls:?}");

    let log_text = fs::read_to_string(target.join(".mysh/log")).unwrap_or_default();
    // Already-present `mise` must never be re-bootstrapped.
    assert!(!log_text.contains("mise-bootstrapped"));
    // The Application Log must record the install so `teardown` can later reverse it.
    assert!(log_text.contains("package-installed\twidget@1.0\n"), "log was: {log_text:?}");

    // Eager packages get no mysh-generated shim — they resolve through mise's own
    // native shim mechanism instead, in mise's own data dir (`.mysh/mise/shims`, the
    // directory `bootstrap.sh` now also adds to `PATH` alongside `.mysh/bin`).
    assert!(!target.join(".mysh/bin/widget").exists(), "eager must not get a mysh-generated shim");
    let native_shim = target.join(".mysh/mise/shims/widget");
    assert!(native_shim.exists(), "eager package must be reachable via mise's own shim dir");
    let run = Command::new(&native_shim).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ran-widget");
}

#[test]
fn lazy_and_eager_packages_land_in_their_own_distinct_directories() {
    let source = temp_dir("package-source-mixed");
    let target = temp_dir("package-target-mixed");
    let stub_dir = temp_dir("package-stub-mixed");

    write_eager_declaration(&source, "widget", "1.0");
    write_lazy_shim(&source, "elio@1.0", "elio");
    write_fake_mise(&stub_dir);

    let output = run_apply(&source, &target, &stub_path_env(&stub_dir));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    assert!(target.join(".mysh/bin/elio").exists(), "lazy package must land in .mysh/bin");
    assert!(!target.join(".mysh/mise/shims/elio").exists());
    assert!(target.join(".mysh/mise/shims/widget").exists(), "eager package must land in mise's shims dir");
    assert!(!target.join(".mysh/bin/widget").exists());
}

#[test]
fn lazy_shim_installs_then_execs_on_first_invocation() {
    let source = temp_dir("package-source-lazy");
    let target = temp_dir("package-target-lazy");
    let stub_dir = temp_dir("package-stub-lazy");

    write_lazy_shim(&source, "widget@1.0", "elio-cli");

    // A stubbed, already-installed `mise` on `PATH` at apply time (the resolved-and-
    // bootstrapped case is covered separately by
    // `apply_bootstraps_mise_for_a_lazy_only_device_so_the_shim_has_something_to_invoke`).
    write_fake_mise(&stub_dir);
    let path_env = stub_path_env(&stub_dir);

    let output = run_apply(&source, &target, &path_env);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let shim = target.join(".mysh/bin/elio-cli");
    assert!(shim.exists(), "the lazy shim must be identity-copied to its bin name");

    // Lazy packages are never installed during apply itself.
    let mise_calls_after_apply = fs::read_to_string(stub_dir.join("mise.calls")).unwrap_or_default();
    assert!(
        !mise_calls_after_apply.contains("installed widget"),
        "lazy package must not be installed during apply: {mise_calls_after_apply:?}"
    );

    // The shim resolves `$HOME`/`mise` at run time (portable, see ADR-0006) — so
    // invoking it standalone needs `HOME` pointed at `target`, exactly as it would be
    // on a real device where `target` *is* `$HOME`.
    let first = Command::new(&shim).env("PATH", &path_env).env("HOME", &target).arg("--flag").output().unwrap();
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert_eq!(String::from_utf8_lossy(&first.stdout).trim(), "ran-elio-cli --flag");

    let second = Command::new(&shim).env("PATH", &path_env).env("HOME", &target).arg("--flag").output().unwrap();
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
