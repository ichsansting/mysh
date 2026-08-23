use mysh::config::Config;
use mysh::error::{Error, Result};
use std::process::ExitCode;

const USAGE: &str = "usage: mysh <apply|diff|save|reset|add|teardown> \
[--source-dir <dir>] [--target-dir <dir>] [--passphrase <p>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match dispatch(command, &args[1..]) {
        Ok(message) => {
            print!("{message}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mysh: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(command: &str, rest: &[String]) -> Result<String> {
    let (config, leftover) = Config::parse(rest)?;
    let mut passphrase = mysh::infra::prompt::passphrase_provider(config.passphrase.clone());
    match command {
        "apply" => {
            expect_no_args(&leftover)?;
            mysh::ops::apply::run(&config, &mut passphrase)
        }
        "teardown" => {
            expect_no_args(&leftover)?;
            mysh::ops::teardown::run(&config, &mut std::io::stdin().lock())
        }
        "diff" => {
            expect_no_args(&leftover)?;
            mysh::ops::diff::run(&config, &mut passphrase)
        }
        "save" => {
            expect_no_args(&leftover)?;
            mysh::ops::save::run(&config, &mut std::io::stdin().lock(), &mut passphrase)
        }
        "reset" => {
            expect_no_args(&leftover)?;
            mysh::ops::reset::run(&config, &mut std::io::stdin().lock(), &mut passphrase)
        }
        "add" => mysh::ops::add::run(&config, leftover, &mut std::io::stdin().lock()),
        _ => Err(Error::Usage(USAGE.to_string())),
    }
}

fn expect_no_args(leftover: &[String]) -> Result<()> {
    match leftover.first() {
        None => Ok(()),
        Some(arg) => Err(Error::Usage(format!("unexpected argument: {arg}"))),
    }
}
