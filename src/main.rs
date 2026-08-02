use mysh::apply;
use mysh::config::Config;
use mysh::diff;
use mysh::reset;
use mysh::save;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1) else {
        eprintln!("usage: mysh <apply|diff|save|reset> [--source-dir DIR] [--target-dir DIR] [--remote-url URL] [--passphrase PASS]");
        return ExitCode::FAILURE;
    };

    let rest = &args[2..];

    match command.as_str() {
        "apply" => match Config::resolve(rest) {
            Ok(config) => match apply::apply(&config.source_dir, &config.target_dir) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        "diff" => match Config::resolve(rest) {
            Ok(config) => match diff::diff(&config.source_dir, &config.target_dir) {
                Ok(drifts) => {
                    print!("{}", diff::format_drifts(&drifts));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        "save" => match Config::resolve(rest) {
            Ok(config) => {
                let stdin = io::stdin();
                let mut input = stdin.lock();
                match save::save(&config.source_dir, &config.target_dir, &mut input) {
                    Ok(msg) => {
                        print!("{msg}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        "reset" => match Config::resolve(rest) {
            Ok(config) => {
                let stdin = io::stdin();
                let mut input = stdin.lock();
                match reset::reset(&config.source_dir, &config.target_dir, &mut input) {
                    Ok(msg) => {
                        print!("{msg}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::FAILURE
        }
    }
}
