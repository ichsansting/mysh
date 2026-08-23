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
    let (_config, _leftover) = Config::parse(rest)?;
    match command {
        "apply" | "diff" | "save" | "reset" | "add" | "teardown" => {
            Err(Error::Rejected(format!("{command}: not implemented yet")))
        }
        _ => Err(Error::Usage(USAGE.to_string())),
    }
}
