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

/// mysh's Source can be a subdirectory of a larger repo (e.g. `profile/` inside the
/// mysh tool repo). `commit`/`show` must stay scoped to that subdirectory rather than
/// leaking to or reading from the rest of the repo.
#[test]
fn commit_and_show_scope_to_a_subdirectory_source_dir() {
    let remote = init_bare_remote();
    let remote_url = format!("file://{}", remote.to_string_lossy());

    let workdir = temp_dir("git-subdir-workdir");
    let repo = workdir.join("repo");
    git::clone(&remote_url, &repo).unwrap();
    fs::create_dir_all(repo.join("profile")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("profile/bashrc"), b"export EDITOR=vim\n").unwrap();
    fs::write(repo.join("src/main.rs"), b"fn main() {}\n").unwrap();
    git::commit(&repo, "seed").unwrap();
    git::push(&repo).unwrap();

    let profile_dir = repo.join("profile");
    fs::write(profile_dir.join("bashrc"), b"export EDITOR=nvim\n").unwrap();
    fs::write(repo.join("src/main.rs"), b"fn main() { unrelated_edit(); }\n").unwrap();

    git::commit(&profile_dir, "update bashrc").expect("commit scoped to profile/ should succeed");

    assert!(
        git::status(&repo).unwrap().contains("src/main.rs"),
        "the unrelated src/ edit must stay uncommitted after a commit scoped to profile/"
    );

    let head = rev_parse(&repo, "HEAD");
    let committed = git::show(&profile_dir, &head, std::path::Path::new("bashrc"))
        .expect("show should resolve a subdir-relative path")
        .expect("bashrc should exist at HEAD");
    assert_eq!(committed, b"export EDITOR=nvim\n");
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
