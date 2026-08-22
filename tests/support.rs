use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh, unique temp directory under the OS temp dir. Not auto-cleaned —
/// tests run in a scratch location that's fine to leave behind.
pub fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "mysh-test-{label}-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `content` to `path` as an executable script (mode 0o755) — the shape every
/// PATH-stubbed fake binary in the integration tests needs.
#[cfg(unix)]
pub fn write_executable(path: &Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The real `PATH` with `stub_dir` prepended, so fake tools written there shadow any
/// real ones of the same name — every PATH-stubbed integration test needs this.
pub fn bare_env_path(stub_dir: &Path) -> String {
    format!("{}:{}", stub_dir.display(), std::env::var("PATH").unwrap())
}

/// The fake `mise` shared by every package-related integration test: `--version`
/// succeeds (identifies an already-present `mise`); `trust` is a no-op (the real
/// `mise` requires trusting a config file before parsing it — see `mise::trust` — this
/// fake never refuses); `install <specifier>...` records each specifier to
/// `mise.calls` and leaves an `installs/<name>` directory under `$MISE_DATA_DIR` —
/// the observable footprint tests (e.g. teardown's) use as evidence a tool was
/// actually installed into the isolated data dir; `x` is the shim invocation path,
/// install-once-then-exec.
pub fn mise_stub_script(stub_dir: &Path) -> String {
    format!(
        r#"#!/bin/sh
echo "$@" >> "{stub}/mise.calls"
case "$1" in
  --version) exit 0 ;;
  trust) exit 0 ;;
  install)
    shift
    for spec in "$@"; do
      name=$(echo "$spec" | sed -e 's/@.*//' -e 's/^[^:]*://' -e 's#.*/##')
      mkdir -p "$MISE_DATA_DIR/installs/$name"
      echo "installed $spec" >> "{stub}/mise.calls"
    done
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
pub fn write_fake_mise(stub_dir: &Path) {
    write_executable(&stub_dir.join("mise"), &mise_stub_script(stub_dir));
}

/// A fake `curl` that, instead of hitting the real network, writes a fake `mise`
/// executable to `$MISE_INSTALL_PATH` — simulating the official `curl -fsSL
/// https://mise.run | sh` installer (which does exactly this: write one binary to that
/// path, nothing else) without any real download. `stub_dir` is only where the fake
/// `mise`'s own bookkeeping (`mise.calls`, installed-tool stubs) lives — independent of
/// wherever `MISE_INSTALL_PATH` ends up.
pub fn write_fake_curl(stub_dir: &Path) {
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
