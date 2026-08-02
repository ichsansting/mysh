mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::{bare_env_path, temp_dir, write_executable};

const TRIPLES: [&str; 3] =
    ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl", "aarch64-apple-darwin"];

fn release_sh_source() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("release.sh")).unwrap()
}

/// A real git repo (not the actual mysh repo — release.sh `cd`s to its own script
/// directory, so a copy of it is committed into an isolated throwaway repo) with a
/// local bare remote as `origin`, standing in for GitHub.
fn init_repo_with_release_sh() -> (PathBuf, PathBuf) {
    let remote = temp_dir("release-remote");
    Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();

    let repo = temp_dir("release-repo");
    Command::new("git").args(["init", "--initial-branch=main"]).arg(&repo).status().unwrap();
    let remote_url = format!("file://{}", remote.to_string_lossy());
    Command::new("git")
        .current_dir(&repo)
        .args(["remote", "add", "origin", &remote_url])
        .status()
        .unwrap();
    // Test isolation only: a maintainer's real global `tag.gpgsign=true` still applies
    // to their real run of release.sh (which supplies `-m`, so it works either way) —
    // this repo-local override just keeps the test from requiring a real GPG key.
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "tag.gpgsign", "false"])
        .status()
        .unwrap();

    fs::write(repo.join("release.sh"), release_sh_source()).unwrap();
    fs::write(repo.join(".gitignore"), "/target\n").unwrap();
    fs::write(repo.join("README"), "v1\n").unwrap();

    Command::new("git").current_dir(&repo).args(["add", "-A"]).status().unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["-c", "user.email=t@t.com", "-c", "user.name=t", "commit", "-m", "seed"])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["push", "-u", "origin", "main"])
        .status()
        .unwrap();

    (repo, remote)
}

/// Fake `zig`/`cargo-zigbuild`/`rustup`/`gh`/`cargo`, so the test exercises release.sh's
/// own orchestration logic (clean-tree gate, tag force-move, create-vs-update branching)
/// without a real cross-compiler, network, or GitHub auth. `gh`'s fake persists a marker
/// file across invocations to simulate "the v1 release already exists on GitHub."
fn write_stub_tools(stub_dir: &Path) {
    write_executable(&stub_dir.join("zig"), "#!/bin/sh\nexit 0\n");
    write_executable(&stub_dir.join("cargo-zigbuild"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &stub_dir.join("rustup"),
        &format!(
            r#"#!/bin/sh
echo "$@" >> "{stub}/rustup.calls"
if [ "$1" = "target" ] && [ "$2" = "list" ]; then
  echo "x86_64-unknown-linux-musl"
  echo "aarch64-unknown-linux-musl"
  echo "aarch64-apple-darwin"
  exit 0
fi
exit 1
"#,
            stub = stub_dir.display()
        ),
    );
    write_executable(
        &stub_dir.join("cargo"),
        &format!(
            r#"#!/bin/sh
echo "$@" >> "{stub}/cargo.calls"
prev=""
triple=""
for arg in "$@"; do
  if [ "$prev" = "--target" ]; then
    triple="$arg"
  fi
  prev="$arg"
done
mkdir -p "target/$triple/release"
printf 'fake-mysh-binary-for-%s' "$triple" > "target/$triple/release/mysh"
"#,
            stub = stub_dir.display()
        ),
    );
    write_executable(
        &stub_dir.join("gh"),
        &format!(
            r#"#!/bin/sh
echo "$@" >> "{stub}/gh.calls"
case "$1 $2" in
  "release view")
    [ -f "{stub}/gh-release-exists" ] && exit 0 || exit 1
    ;;
  "release create")
    touch "{stub}/gh-release-exists"
    exit 0
    ;;
  "release edit")
    exit 0
    ;;
  "release upload")
    exit 0
    ;;
  *)
    exit 1
    ;;
esac
"#,
            stub = stub_dir.display()
        ),
    );
}

fn run_release(repo: &Path, stub_dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(repo.join("release.sh"))
        .current_dir(repo)
        .env("PATH", bare_env_path(stub_dir))
        .output()
        .expect("failed to run release.sh")
}

fn head_commit(repo: &Path) -> String {
    let out = Command::new("git").current_dir(repo).args(["rev-parse", "HEAD"]).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// The commit `v1` points to on the remote, dereferencing through the tag object —
/// `-m` (needed for `tag.gpgsign=true` maintainers) makes `v1` an annotated tag, so its
/// own object SHA differs from the commit SHA `git ls-remote` would show directly.
fn remote_v1_commit(remote: &Path) -> String {
    let scratch = temp_dir("release-verify-clone");
    Command::new("git")
        .args(["clone", "--bare", "--quiet"])
        .arg(format!("file://{}", remote.to_string_lossy()))
        .arg(&scratch)
        .status()
        .unwrap();
    let out = Command::new("git")
        .current_dir(&scratch)
        .args(["rev-parse", "v1^{commit}"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn release_script_creates_then_updates_the_single_v1_release_idempotently() {
    let (repo, remote) = init_repo_with_release_sh();
    let stub_dir = temp_dir("release-stub");
    write_stub_tools(&stub_dir);

    let first_commit = head_commit(&repo);

    let first = run_release(&repo, &stub_dir);
    assert!(first.status.success(), "stderr: {}", String::from_utf8_lossy(&first.stderr));

    // Built (and cross-compiled, via the same cargo-zigbuild invocation) all three
    // declared targets.
    let cargo_calls = fs::read_to_string(stub_dir.join("cargo.calls")).unwrap();
    for triple in TRIPLES {
        assert!(cargo_calls.contains(triple), "cargo calls were: {cargo_calls:?}");
    }

    // No prior v1 release: must create it, uploading all three assets, with notes
    // naming the commit just built.
    let gh_calls = fs::read_to_string(stub_dir.join("gh.calls")).unwrap();
    assert!(gh_calls.contains("release create v1"), "gh calls were: {gh_calls:?}");
    assert!(gh_calls.contains(&first_commit), "gh calls were: {gh_calls:?}");
    for triple in TRIPLES {
        assert!(gh_calls.contains(&format!("mysh-{triple}")), "gh calls were: {gh_calls:?}");
    }

    // The single `v1` tag was force-pushed to the remote, pointing at this commit.
    let tag_commit_1 = remote_v1_commit(&remote);
    assert_eq!(tag_commit_1, first_commit, "v1 tag on remote must point at the commit just released");

    // Uploaded assets are cleaned out of the working tree afterward.
    for triple in TRIPLES {
        assert!(!repo.join(format!("mysh-{triple}")).exists());
    }

    // --- Second run, against a new commit: must UPDATE the same release, not create
    // a second one, and must force-move the tag to the new commit. ---
    fs::write(repo.join("README"), "v2\n").unwrap();
    Command::new("git").current_dir(&repo).args(["add", "-A"]).status().unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["-c", "user.email=t@t.com", "-c", "user.name=t", "commit", "-m", "update"])
        .status()
        .unwrap();
    let second_commit = head_commit(&repo);

    let second = run_release(&repo, &stub_dir);
    assert!(second.status.success(), "stderr: {}", String::from_utf8_lossy(&second.stderr));

    let gh_calls_2 = fs::read_to_string(stub_dir.join("gh.calls")).unwrap();
    assert_eq!(
        gh_calls_2.matches("release create v1").count(),
        1,
        "must not create a second release on update: {gh_calls_2:?}"
    );
    assert!(gh_calls_2.contains("release edit v1"), "gh calls were: {gh_calls_2:?}");
    assert!(gh_calls_2.contains("release upload v1"), "gh calls were: {gh_calls_2:?}");
    assert!(gh_calls_2.contains("--clobber"), "gh calls were: {gh_calls_2:?}");
    assert!(gh_calls_2.contains(&second_commit), "gh calls were: {gh_calls_2:?}");

    let tag_commit_2 = remote_v1_commit(&remote);
    assert_eq!(tag_commit_2, second_commit, "v1 tag on remote must force-move to the new commit");
    assert_ne!(tag_commit_1, tag_commit_2, "the v1 tag must force-move to the new commit");
}

#[test]
fn release_script_refuses_a_dirty_working_tree() {
    let (repo, _remote) = init_repo_with_release_sh();
    let stub_dir = temp_dir("release-stub-dirty");
    write_stub_tools(&stub_dir);

    fs::write(repo.join("uncommitted.txt"), "oops").unwrap();

    let output = run_release(&repo, &stub_dir);
    assert!(!output.status.success(), "release.sh must refuse a dirty working tree");
    assert!(
        !stub_dir.join("gh.calls").exists(),
        "must not touch gh at all when the tree is dirty"
    );
}

#[test]
fn release_script_fails_with_instructions_when_a_tool_is_missing() {
    let (repo, _remote) = init_repo_with_release_sh();
    let stub_dir = temp_dir("release-stub-missing-tool");
    write_stub_tools(&stub_dir);
    // Simulate zig not being installed: remove the stub, don't fall back to
    // auto-installing anything.
    fs::remove_file(stub_dir.join("zig")).unwrap();

    let output = run_release(&repo, &stub_dir);
    assert!(!output.status.success(), "release.sh must refuse to run without zig");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("zig"), "stderr was: {stderr:?}");
    assert!(
        !stub_dir.join("gh.calls").exists(),
        "must not attempt a release when a required tool is missing"
    );
}
