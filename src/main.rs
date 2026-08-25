use mysh::config::{Config, take_switch};
use mysh::error::{Error, Result};
use std::process::ExitCode;

const USAGE: &str = "usage: mysh <apply|diff|save|reset|add|teardown> \
[--source-dir <dir>] [--target-dir <dir>] [--passphrase <p>] [--quick]";

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
    let (config, mut leftover) = Config::parse(rest)?;
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
            let quick = take_switch(&mut leftover, "--quick");
            expect_no_args(&leftover)?;
            mysh::ops::diff::run(&config, &mut passphrase, quick)
        }
        "save" => {
            expect_no_args(&leftover)?;
            // No pre-locked stdin handed in here (unlike reset/teardown below):
            // save may hand off to the interactive picker, which needs its own
            // fresh lock on stdin for raw-mode key reads — holding one from out
            // here for the whole call would deadlock against it.
            mysh::ops::save::run(&config, &mut passphrase)
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
