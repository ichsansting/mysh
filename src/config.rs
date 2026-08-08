use std::env;
use std::path::PathBuf;

/// Resolved CLI configuration: injectable SOURCE_DIR/TARGET_DIR/REMOTE_URL/passphrase.
/// Flag > env var > default, per the testing seam described in the spec.
#[derive(Debug)]
pub struct Config {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub remote_url: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Default)]
struct Flags {
    source_dir: Option<String>,
    target_dir: Option<String>,
    remote_url: Option<String>,
    passphrase: Option<String>,
}

/// flag value, else `env_name`, else `None` — the shared flag > env precedence.
fn resolve_var(flag: Option<String>, env_name: &str) -> Option<String> {
    flag.or_else(|| env::var(env_name).ok())
}

impl Config {
    /// `args` excludes the program name and subcommand (e.g. ["--source-dir", "/tmp/src"]).
    pub fn resolve(args: &[String]) -> Result<Config, String> {
        let flags = parse_flags(args)?;

        let target_dir = resolve_var(flags.target_dir, "MYSH_TARGET_DIR")
            .or_else(|| env::var("HOME").ok())
            .ok_or("TARGET_DIR must be set via --target-dir, MYSH_TARGET_DIR, or $HOME")?;
        let target_dir = PathBuf::from(target_dir);

        // Mirrors bootstrap.sh's own `--source-dir` handoff value (its SOURCE_DIR var
        // is the sparse-clone root; this is that root's `profile/` subdirectory,
        // where the actual Source content lives), so a stock bootstrap leaves every
        // command runnable with no flags and no env vars. Note: MYSH_SOURCE_DIR means
        // slightly different things in the two processes — bootstrap.sh's shell var
        // of that name is the clone root, this env var is the final git-operations
        // directory — because bootstrap.sh always derives one from the other. An
        // override here must point at the `profile/` directory itself.
        let source_dir = resolve_var(flags.source_dir, "MYSH_SOURCE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| target_dir.join(".mysh/source/profile"));

        let remote_url = resolve_var(flags.remote_url, "MYSH_REMOTE_URL");
        let passphrase = resolve_var(flags.passphrase, "MYSH_PASSPHRASE");

        Ok(Config {
            source_dir,
            target_dir,
            remote_url,
            passphrase,
        })
    }

    /// Like `resolve`, but only requires `TARGET_DIR`. `teardown` never touches
    /// `Source` (everything it needs comes from the Application Log under `Target`),
    /// so forcing a `--source-dir` on it the way every other command needs would be a
    /// pointless UX tax.
    pub fn resolve_target_dir(args: &[String]) -> Result<PathBuf, String> {
        let flags = parse_flags(args)?;
        resolve_var(flags.target_dir, "MYSH_TARGET_DIR")
            .or_else(|| env::var("HOME").ok())
            .map(PathBuf::from)
            .ok_or_else(|| "TARGET_DIR must be set via --target-dir, MYSH_TARGET_DIR, or $HOME".to_string())
    }
}

fn parse_flags(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags::default();
    let mut i = 0;
    while i < args.len() {
        let (name, value) = match args[i].split_once('=') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None => {
                let n = args[i].clone();
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| format!("{n} requires a value"))?
                    .clone();
                (n, v)
            }
        };
        match name.as_str() {
            "--source-dir" => flags.source_dir = Some(value),
            "--target-dir" => flags.target_dir = Some(value),
            "--remote-url" => flags.remote_url = Some(value),
            "--passphrase" => flags.passphrase = Some(value),
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env vars are process-global, so every case that touches them lives in one
    // test to avoid racing against other tests running in parallel threads.
    #[test]
    fn resolve_precedence_and_validation() {
        unsafe {
            env::remove_var("MYSH_SOURCE_DIR");
            env::remove_var("MYSH_TARGET_DIR");
            env::remove_var("MYSH_REMOTE_URL");
            env::remove_var("MYSH_PASSPHRASE");
        }

        // Missing SOURCE_DIR (no flag, no env) defaults to
        // TARGET_DIR/.mysh/source/profile, mirroring bootstrap.sh's handoff value —
        // no hard error post-bootstrap. Regression coverage for the previous
        // hard-fail: target_dir actually has .mysh/source/profile on disk here, the
        // exact post-bootstrap state.
        let target_dir = env::temp_dir().join("mysh-config-test-post-bootstrap");
        std::fs::create_dir_all(target_dir.join(".mysh/source/profile")).unwrap();
        let args = vec!["--target-dir".to_string(), target_dir.to_string_lossy().into_owned()];
        let config = Config::resolve(&args).unwrap();
        assert_eq!(config.source_dir, target_dir.join(".mysh/source/profile"));
        std::fs::remove_dir_all(&target_dir).ok();

        // Env var fills in when no flag is given.
        unsafe { env::set_var("MYSH_SOURCE_DIR", "/from/env") };
        let config = Config::resolve(&[]).unwrap();
        assert_eq!(config.source_dir, PathBuf::from("/from/env"));

        // Flag takes precedence over env.
        let args = vec!["--source-dir".to_string(), "/from/flag".to_string()];
        let config = Config::resolve(&args).unwrap();
        assert_eq!(config.source_dir, PathBuf::from("/from/flag"));

        unsafe { env::remove_var("MYSH_SOURCE_DIR") };
    }

    #[test]
    fn resolve_target_dir_never_requires_source_dir() {
        unsafe { env::remove_var("MYSH_TARGET_DIR") };

        let args = vec!["--target-dir".to_string(), "/from/flag".to_string()];
        let target_dir = Config::resolve_target_dir(&args).unwrap();
        assert_eq!(target_dir, PathBuf::from("/from/flag"));
    }
}
