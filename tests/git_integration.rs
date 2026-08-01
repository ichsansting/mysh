mod support;

use mysh::git;
use std::fs;
use std::process::Command;
use support::temp_dir;

fn init_bare_remote() -> std::path::PathBuf {
    let remote = temp_dir("git-remote");
    let status = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    remote
}

#[test]
fn clone_commit_push_fetch_status_round_trip_against_real_git() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let workdir = temp_dir("git-workdir");
    let clone_dest = workdir.join("clone");
    git::clone(&remote_url, &clone_dest).expect("clone should succeed");
    assert!(clone_dest.join(".git").exists());

    fs::write(clone_dest.join("bashrc"), b"export EDITOR=vim\n").unwrap();
    assert!(git::status(&clone_dest)
        .expect("status should succeed")
        .contains("bashrc"));

    git::commit(&clone_dest, "add bashrc").expect("commit should succeed");
    assert_eq!(git::status(&clone_dest).unwrap(), "");

    git::push(&clone_dest).expect("push should succeed");

    let second_clone = workdir.join("second-clone");
    git::clone(&remote_url, &second_clone).expect("second clone should succeed");
    assert_eq!(
        fs::read(second_clone.join("bashrc")).unwrap(),
        b"export EDITOR=vim\n"
    );

    fs::write(clone_dest.join("gitconfig"), b"[core]\n").unwrap();
    git::commit(&clone_dest, "add gitconfig").unwrap();
    git::push(&clone_dest).unwrap();

    git::fetch(&second_clone).expect("fetch should succeed");

    let clone_head = rev_parse(&clone_dest, "HEAD");
    let second_clone_remote_head = rev_parse(&second_clone, "origin/main");
    assert_eq!(
        clone_head, second_clone_remote_head,
        "fetch should bring the pushed commit into origin/main"
    );
}

fn rev_parse(repo_dir: &std::path::Path, rev: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(["rev-parse", rev])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
