use mysh::apply;
use mysh::config::Config;
use mysh::diff;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1) else {
        eprintln!("usage: mysh <apply|diff> [--source-dir DIR] [--target-dir DIR] [--remote-url URL] [--passphrase PASS]");
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
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::FAILURE
        }
    }
}
