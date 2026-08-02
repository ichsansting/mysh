mod support;

use mysh::git;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use support::temp_dir;

fn init_bare_remote_with_file(relative: &str, content: &[u8]) -> std::path::PathBuf {
    let remote = temp_dir("zero-flag-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());

    let seed = temp_dir("zero-flag-seed");
    let remote_url = format!("file://{}", remote.to_string_lossy());
    git::clone(&remote_url, &seed).unwrap();
    fs::write(seed.join(relative), content).unwrap();
    git::commit(&seed, "seed").unwrap();
    git::push(&seed).unwrap();

    remote
}

/// Runs `mysh <args>` the way a real post-bootstrap user would: no `--source-dir`/
/// `--target-dir`/etc flags, and no `MYSH_*` env vars — only `HOME` (redirected to the
/// simulated device, standing in for the real home directory) and `PATH` (so `git`
/// resolves), feeding `stdin` for commands that prompt for confirmation.
fn run_zero_flag(home: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", std::env::var("PATH").unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to run mysh {args:?}: {e}"));
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

/// After a real `bootstrap.sh` run, `apply`, `diff`, `save`, `reset`, and `teardown`
/// must all work with no flags and no env vars set — the zero-flag daily-usage
/// contract this ticket adds. `apply` renders a clean checkout, so `diff`/`save`/
/// `reset` see no drift; `teardown` still has apply's log entries to reverse.
#[test]
fn every_documented_command_succeeds_post_bootstrap_with_no_flags_or_env() {
    let home = temp_dir("zero-flag-home");
    // Mirrors bootstrap.sh's own SOURCE_DIR default exactly.
    let source = home.join(".mysh/source");
    let remote = init_bare_remote_with_file("bashrc", b"export EDITOR=vim\n");
    let remote_url = format!("file://{}", remote.to_string_lossy());
    git::clone(&remote_url, &source).unwrap();

    let apply_out = run_zero_flag(&home, &["apply"], "");
    assert!(apply_out.status.success(), "apply: {}", String::from_utf8_lossy(&apply_out.stderr));
    assert_eq!(fs::read(home.join("bashrc")).unwrap(), b"export EDITOR=vim\n");

    let diff_out = run_zero_flag(&home, &["diff"], "");
    assert!(diff_out.status.success(), "diff: {}", String::from_utf8_lossy(&diff_out.stderr));
    assert_eq!(diff_out.stdout, b"");

    let save_out = run_zero_flag(&home, &["save"], "");
    assert!(save_out.status.success(), "save: {}", String::from_utf8_lossy(&save_out.stderr));
    assert_eq!(save_out.stdout, b"nothing to save\n");

    let reset_out = run_zero_flag(&home, &["reset"], "");
    assert!(reset_out.status.success(), "reset: {}", String::from_utf8_lossy(&reset_out.stderr));
    assert_eq!(reset_out.stdout, b"nothing to reset\n");

    let teardown_out = run_zero_flag(&home, &["teardown"], "y\n");
    assert!(teardown_out.status.success(), "teardown: {}", String::from_utf8_lossy(&teardown_out.stderr));
    assert!(!home.join("bashrc").exists());
}
