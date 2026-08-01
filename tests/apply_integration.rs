mod support;

use std::fs;
use std::process::Command;
use support::temp_dir;

fn run_apply(source: &std::path::Path, target: &std::path::Path) {
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
fn apply_renders_plain_files_byte_for_byte() {
    let source = temp_dir("apply-source");
    let target = temp_dir("apply-target");

    fs::write(source.join("bashrc"), b"export PATH=$PATH:/usr/local/bin\n").unwrap();
    fs::create_dir_all(source.join("config/nvim")).unwrap();
    fs::write(source.join("config/nvim/init.lua"), b"-- nvim config\n").unwrap();

    // Source is a real git working tree; .git must never be mirrored into Target.
    fs::create_dir_all(source.join(".git")).unwrap();
    fs::write(source.join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();

    run_apply(&source, &target);

    assert_eq!(
        fs::read(target.join("bashrc")).unwrap(),
        fs::read(source.join("bashrc")).unwrap()
    );
    assert_eq!(
        fs::read(target.join("config/nvim/init.lua")).unwrap(),
        fs::read(source.join("config/nvim/init.lua")).unwrap()
    );
    assert!(!target.join(".git").exists());
}

#[test]
fn apply_twice_with_no_changes_is_a_noop() {
    let source = temp_dir("apply-source-idempotent");
    let target = temp_dir("apply-target-idempotent");

    fs::write(source.join("gitconfig"), b"[user]\n\tname = Test\n").unwrap();

    run_apply(&source, &target);
    let rendered = target.join("gitconfig");
    let mtime_after_first = fs::metadata(&rendered).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    run_apply(&source, &target);
    let mtime_after_second = fs::metadata(&rendered).unwrap().modified().unwrap();

    assert_eq!(
        mtime_after_first, mtime_after_second,
        "unchanged file must not be rewritten on a second apply"
    );
}
