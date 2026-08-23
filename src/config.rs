use crate::error::{Error, Result};
use std::path::PathBuf;

/// The injectable locations every command shares (the testing seam is a real,
/// user-facing configuration surface, not scaffolding): flag > `MYSH_*` env > default.
/// Defaults: target = `$HOME`, source = `<target>/.mysh/source/profile`.
/// No remote flag: Remote is always Source's own git `origin`.
pub struct Config {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub passphrase: Option<String>,
}

impl Config {
    /// Parses the shared flags out of `args` (`--flag value` or `--flag=value`),
    /// returning everything unrecognized — positionals and command-specific flags —
    /// untouched, in order, for the command to interpret with `take_flag`/`take_switch`.
    pub fn parse(args: &[String]) -> Result<(Config, Vec<String>)> {
        let mut leftover: Vec<String> = args.to_vec();
        let source_dir = take_flag(&mut leftover, "--source-dir")?;
        let target_dir = take_flag(&mut leftover, "--target-dir")?;
        let passphrase = take_flag(&mut leftover, "--passphrase")?;

        let target_dir = target_dir
            .or_else(|| std::env::var("MYSH_TARGET_DIR").ok())
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
            .ok_or_else(|| Error::Usage("no --target-dir, MYSH_TARGET_DIR, or HOME".into()))?;
        let source_dir = source_dir
            .or_else(|| std::env::var("MYSH_SOURCE_DIR").ok())
            .map(PathBuf::from)
            .unwrap_or_else(|| target_dir.join(".mysh/source/profile"));
        let passphrase = passphrase.or_else(|| std::env::var("MYSH_PASSPHRASE").ok());

        Ok((
            Config {
                source_dir,
                target_dir,
                passphrase,
            },
            leftover,
        ))
    }
}

/// Removes `--name value` / `--name=value` from `args`, returning the value.
pub fn take_flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    let prefix = format!("{name}=");
    let Some(i) = args
        .iter()
        .position(|a| a == name || a.starts_with(&prefix))
    else {
        return Ok(None);
    };
    let arg = args.remove(i);
    if let Some(value) = arg.strip_prefix(&prefix) {
        return Ok(Some(value.to_string()));
    }
    if i < args.len() {
        return Ok(Some(args.remove(i)));
    }
    Err(Error::Usage(format!("{name} requires a value")))
}

/// Removes a boolean `--name` switch from `args`, returning whether it was present.
pub fn take_switch(args: &mut Vec<String>, name: &str) -> bool {
    let Some(i) = args.iter().position(|a| a == name) else {
        return false;
    };
    args.remove(i);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_beat_env_and_defaults_and_leftovers_pass_through() {
        let (config, leftover) = Config::parse(&strs(&[
            "extra",
            "--source-dir",
            "/s",
            "--target-dir=/t",
            "--passphrase",
            "pw",
            "--eager",
        ]))
        .unwrap();
        assert_eq!(config.source_dir, PathBuf::from("/s"));
        assert_eq!(config.target_dir, PathBuf::from("/t"));
        assert_eq!(config.passphrase.as_deref(), Some("pw"));
        assert_eq!(leftover, strs(&["extra", "--eager"]));
    }

    #[test]
    fn source_defaults_under_target() {
        let (config, _) = Config::parse(&strs(&["--target-dir", "/t"])).unwrap();
        assert_eq!(config.source_dir, PathBuf::from("/t/.mysh/source/profile"));
    }

    #[test]
    fn a_flag_missing_its_value_is_a_usage_error() {
        assert!(Config::parse(&strs(&["--target-dir"])).is_err());
    }

    #[test]
    fn take_switch_removes_only_the_switch() {
        let mut args = strs(&["a", "--eager", "b"]);
        assert!(take_switch(&mut args, "--eager"));
        assert!(!take_switch(&mut args, "--eager"));
        assert_eq!(args, strs(&["a", "b"]));
    }
}
