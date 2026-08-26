use crate::config::Config;
use crate::domain::BIN_DIR_REL;
use std::fs;
use std::process::Command;

const DEFAULT_RELEASES_REPO: &str = "ichsansting/mysh";

/// The `mysh-<arch>-<os>` asset name matching this device, mirroring
/// bootstrap.sh's own OS/arch detection. `None` on a target bootstrap.sh
/// itself doesn't support — nothing to compare against, not an error.
fn asset_name() -> Option<String> {
    let os_part = match std::env::consts::OS {
        "linux" => "unknown-linux-musl",
        "macos" => "apple-darwin",
        _ => return None,
    };
    let arch_part = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    Some(format!("mysh-{arch_part}-{os_part}"))
}

/// Update's binary leg (ADR-0013): if `.mysh/bin/mysh` is already installed,
/// download the current release asset for this device's arch and replace it
/// if the content differs. Never installs a first copy — that's bootstrap.sh's
/// job — and never fails Update: a missing `curl`, no network, or an
/// unsupported arch just leaves the binary as it is.
pub fn refresh_binary(config: &Config) {
    let installed = config.target_dir.join(BIN_DIR_REL).join("mysh");
    if !installed.is_file() {
        return;
    }
    let Some(asset) = asset_name() else {
        return;
    };
    let repo =
        std::env::var("MYSH_RELEASES_REPO").unwrap_or_else(|_| DEFAULT_RELEASES_REPO.to_string());
    let url = format!("https://github.com/{repo}/releases/latest/download/{asset}");
    let tmp = installed.with_extension("new");

    let status = Command::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(&tmp)
        .status();
    let downloaded_ok = matches!(status, Ok(status) if status.success());
    if !downloaded_ok {
        let _ = fs::remove_file(&tmp);
        return;
    }

    let (Ok(downloaded), Ok(current)) = (fs::read(&tmp), fs::read(&installed)) else {
        let _ = fs::remove_file(&tmp);
        return;
    };
    if downloaded == current {
        let _ = fs::remove_file(&tmp);
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    }
    // Rename, not an in-place write: `installed` may be this very process's
    // own running executable, and overwriting that in place fails with
    // "text file busy" on Linux. A rename only swaps the directory entry —
    // the running process keeps its already-mapped (now-unlinked) original.
    let _ = fs::rename(&tmp, &installed);
}
