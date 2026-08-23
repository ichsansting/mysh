//! BDD harness: every scenario in features/ drives the real compiled binary
//! (`CARGO_BIN_EXE_mysh`) against a scenario-scoped temp world — real git over
//! local `file://` bare remotes, mise/curl stubbed via PATH — and asserts only
//! externally observable outcomes: files on disk, exit codes, output, the
//! Application Log, commits in the test repos.

mod steps;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub const TEST_PASSPHRASE: &str = "bdd test passphrase";

/// Scenario-scoped temp directory, removed on drop — no leaked dirs.
#[derive(Debug)]
pub struct TempDir(pub PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("mysh-bdd-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

type TreeSnapshot = BTreeMap<String, (Vec<u8>, SystemTime)>;

#[derive(cucumber::World, Debug)]
#[world(init = Self::new)]
pub struct World {
    pub tmp: TempDir,
    pub source: PathBuf,
    pub target: PathBuf,
    pub remote: Option<PathBuf>,
    pub stub: PathBuf,
    pub rc_file: PathBuf,
    pub hide_real_mise: bool,
    pub last: Option<Output>,
    pub batch: Vec<(String, Output)>,
    pub tree_snapshot: Option<(PathBuf, TreeSnapshot)>,
    pub remote_snapshot: Option<String>,
    pub release_dir: Option<PathBuf>,
}

impl World {
    fn new() -> Self {
        let tmp = TempDir::new();
        let root = tmp.0.clone();
        let world = World {
            tmp,
            source: root.join("source"),
            target: root.join("target"),
            remote: None,
            stub: root.join("stub"),
            rc_file: root.join("rc"),
            hide_real_mise: false,
            last: None,
            batch: Vec::new(),
            tree_snapshot: None,
            remote_snapshot: None,
            release_dir: None,
        };
        fs::create_dir_all(&world.source).unwrap();
        fs::create_dir_all(&world.target).unwrap();
        fs::create_dir_all(&world.stub).unwrap();
        world
    }

    // --- running mysh -----------------------------------------------------

    /// PATH for every child process: the stub dir first, then the ambient PATH —
    /// with any directory holding a real `mise` filtered out when a scenario
    /// declared "no mise resolvable on PATH".
    pub fn child_path(&self) -> std::ffi::OsString {
        let ambient = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs = vec![self.stub.clone()];
        for dir in std::env::split_paths(&ambient) {
            if self.hide_real_mise && dir.join("mise").is_file() {
                continue;
            }
            dirs.push(dir);
        }
        std::env::join_paths(dirs).unwrap()
    }

    /// Spawns the real mysh binary with the World's injected locations and test
    /// passphrase, piping `answer` (e.g. "y\n") into stdin, capturing everything.
    pub fn run(&mut self, cmdline: &str, answer: Option<&str>) {
        let mut tokens = cmdline.split_whitespace();
        let command = tokens.next().expect("empty mysh command line");
        let mut child = Command::new(env!("CARGO_BIN_EXE_mysh"))
            .arg(command)
            .args(tokens)
            .arg("--source-dir")
            .arg(&self.source)
            .arg("--target-dir")
            .arg(&self.target)
            .arg("--passphrase")
            .arg(TEST_PASSPHRASE)
            .env("PATH", self.child_path())
            .env("HOME", &self.target)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn mysh");
        // A command that never reads stdin (most of them) may exit before the
        // answer is written — that BrokenPipe is expected, not a failure.
        match child.stdin.take().unwrap().write_all(answer.unwrap_or("").as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => panic!("failed to write mysh stdin: {e}"),
        }
        self.last = Some(child.wait_with_output().expect("failed to wait for mysh"));
    }

    pub fn last_output(&self) -> &Output {
        self.last.as_ref().expect("no command was run yet")
    }

    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.last_output().stdout).into_owned()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.last_output().stderr).into_owned()
    }

    pub fn assert_success(&self) {
        let output = self.last_output();
        assert!(
            output.status.success(),
            "expected success, got {:?}\nstdout: {}\nstderr: {}",
            output.status,
            self.stdout(),
            self.stderr(),
        );
    }

    // --- git --------------------------------------------------------------

    pub fn git(&self, args: &[&str]) -> Output {
        let output = Command::new("git")
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("failed to spawn git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    /// Initializes the bare Remote and turns the (empty) source dir into a clone of it.
    pub fn init_bare_remote(&mut self) {
        let remote = self.tmp.0.join("remote.git");
        self.git(&["init", "-q", "--bare", "--initial-branch=main", remote.to_str().unwrap()]);
        assert!(
            fs::read_dir(&self.source).unwrap().next().is_none(),
            "a bare remote must be declared before any source file"
        );
        fs::remove_dir(&self.source).unwrap();
        self.git(&["clone", "-q", remote.to_str().unwrap(), self.source.to_str().unwrap()]);
        self.git(&["-C", self.source.to_str().unwrap(), "symbolic-ref", "HEAD", "refs/heads/main"]);
        self.remote = Some(remote);
    }

    pub fn commit_source(&self) {
        let src = self.source.to_str().unwrap();
        self.git(&["-C", src, "add", "-A"]);
        self.git(&[
            "-C", src,
            "-c", "user.name=bdd",
            "-c", "user.email=bdd@test",
            "commit", "-q", "--allow-empty", "-m", "bdd seed",
        ]);
    }

    pub fn push_source(&self) {
        let src = self.source.to_str().unwrap();
        self.git(&["-C", src, "push", "-q", "origin", "HEAD:main"]);
    }

    /// Simulates a second device: clone the Remote, write, commit, push, discard.
    pub fn push_from_another_device(&self, rel: &str, content: &str) {
        let remote = self.remote.as_ref().expect("no bare remote declared");
        let device = self.tmp.0.join("device2");
        self.git(&["clone", "-q", remote.to_str().unwrap(), device.to_str().unwrap()]);
        let dev = device.to_str().unwrap();
        self.git(&["-C", dev, "symbolic-ref", "HEAD", "refs/heads/main"]);
        write_file(&device.join(rel), content.as_bytes());
        self.git(&["-C", dev, "add", "-A"]);
        self.git(&[
            "-C", dev,
            "-c", "user.name=bdd2",
            "-c", "user.email=bdd2@test",
            "commit", "-q", "-m", "from another device",
        ]);
        self.git(&["-C", dev, "push", "-q", "origin", "HEAD:main"]);
        fs::remove_dir_all(&device).unwrap();
    }

    /// A file's content at the Remote's current main tip.
    pub fn remote_file(&self, rel: &str) -> Vec<u8> {
        let remote = self.remote.as_ref().expect("no bare remote declared");
        self.git(&["-C", remote.to_str().unwrap(), "show", &format!("main:{rel}")]).stdout
    }

    pub fn remote_head(&self) -> String {
        let remote = self.remote.as_ref().expect("no bare remote declared");
        let output = Command::new("git")
            .args(["-C", remote.to_str().unwrap(), "rev-parse", "main"])
            .output()
            .expect("failed to spawn git");
        // An empty remote has no main yet — represent that as a distinct state.
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    // --- stubs ------------------------------------------------------------

    /// A recording fake mise: appends every invocation to stub/mise.calls,
    /// fakes install/x well enough for shims and prewarm to "work".
    pub fn stub_mise(&self) {
        let calls = self.stub.join("mise.calls");
        let script = format!(
            "#!/bin/sh\necho \"$*\" >> \"{calls}\"\ncase \"$1\" in\n  --version) echo \"mise 0.0.0-bdd\" ;;\nesac\nexit 0\n",
            calls = calls.display()
        );
        write_executable(&self.stub.join("mise"), script.as_bytes());
    }

    /// A recording fake curl standing in for both download paths: it appends its
    /// invocation to stub/curl.calls, then delivers stub/curl-payload — to `-o <path>`
    /// when given, else to `$MISE_INSTALL_PATH` (the `curl https://mise.run | sh` shape).
    pub fn stub_curl(&self) {
        let calls = self.stub.join("curl.calls");
        let payload = self.stub.join("curl-payload");
        let script = format!(
            r#"#!/bin/sh
echo "$*" >> "{calls}"
out=""
prev=""
for a in "$@"; do
  [ "$prev" = "-o" ] && out="$a"
  prev="$a"
done
[ -n "$out" ] || out="${{MISE_INSTALL_PATH:-}}"
if [ -n "$out" ]; then
  mkdir -p "$(dirname "$out")"
  cp "{payload}" "$out"
  chmod +x "$out"
fi
exit 0
"#,
            calls = calls.display(),
            payload = payload.display(),
        );
        write_executable(&self.stub.join("curl"), script.as_bytes());
    }

    /// Points the curl stub's payload at a recording mise script.
    pub fn curl_delivers_recording_mise(&self) {
        self.stub_curl();
        let calls = self.stub.join("mise.calls");
        let script = format!(
            "#!/bin/sh\necho \"$*\" >> \"{calls}\"\ncase \"$1\" in\n  --version) echo \"mise 0.0.0-bdd\" ;;\nesac\nexit 0\n",
            calls = calls.display()
        );
        fs::write(self.stub.join("curl-payload"), script).unwrap();
    }

    /// Points the curl stub's payload at the real compiled mysh binary (bootstrap).
    pub fn curl_delivers_real_mysh(&self) {
        self.stub_curl();
        fs::copy(env!("CARGO_BIN_EXE_mysh"), self.stub.join("curl-payload")).unwrap();
    }

    pub fn stub_calls(&self, name: &str) -> Vec<String> {
        fs::read_to_string(self.stub.join(name))
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    // --- snapshots --------------------------------------------------------

    pub fn snapshot_tree(&mut self, root: PathBuf) {
        let mut map = TreeSnapshot::new();
        snapshot_into(&root, &root, &mut map);
        self.tree_snapshot = Some((root, map));
    }

    pub fn assert_tree_unchanged(&self) {
        let (root, before) = self.tree_snapshot.as_ref().expect("no tree snapshot recorded");
        let mut after = TreeSnapshot::new();
        snapshot_into(root, root, &mut after);
        let before_keys: Vec<_> = before.keys().collect();
        let after_keys: Vec<_> = after.keys().collect();
        assert_eq!(before_keys, after_keys, "file set under {} changed", root.display());
        for (rel, (bytes, mtime)) in before {
            let (after_bytes, after_mtime) = &after[rel];
            assert_eq!(bytes, after_bytes, "{rel} content changed");
            assert_eq!(mtime, after_mtime, "{rel} was rewritten (mtime changed)");
        }
    }

    // --- common paths -----------------------------------------------------

    pub fn source_path(&self, rel: &str) -> PathBuf {
        self.source.join(rel)
    }

    pub fn target_path(&self, rel: &str) -> PathBuf {
        self.target.join(rel)
    }

    pub fn log_text(&self) -> String {
        fs::read_to_string(self.target.join(".mysh/log")).unwrap_or_default()
    }

    /// The Application Log's `target` entries for one target-relative path.
    pub fn log_target_entries(&self, rel: &str) -> Vec<Vec<String>> {
        self.log_text()
            .lines()
            .map(|line| line.split('\t').map(str::to_string).collect::<Vec<_>>())
            .filter(|fields| fields.first().is_some_and(|k| k == "target") && fields.get(2).is_some_and(|p| p == rel))
            .collect()
    }
}

pub fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

pub fn write_executable(path: &Path, content: &[u8]) {
    write_file(path, content);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn snapshot_into(root: &Path, dir: &Path, map: &mut TreeSnapshot) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if path.is_dir() {
            snapshot_into(root, &path, map);
        } else {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            let bytes = fs::read(&path).unwrap();
            let mtime = fs::metadata(&path).unwrap().modified().unwrap();
            map.insert(rel, (bytes, mtime));
        }
    }
}

#[tokio::main]
async fn main() {
    use cucumber::World as _;
    World::cucumber().fail_on_skipped().run_and_exit("features").await;
}
