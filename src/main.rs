mod config;
mod linter;
mod parser;
mod timestamp;

use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::process::ExitCode;

use config::Config;
use linter::Severity;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut config_path: Option<String> = None;
    let mut path: Option<String> = None;

    while let Some(arg) = args.next() {
        if arg == "--config" {
            config_path = match args.next() {
                Some(p) => Some(p),
                None => {
                    eprintln!("subtitle-lint: --config requires a file path");
                    return ExitCode::from(2);
                }
            };
        } else if path.is_none() {
            path = Some(arg);
        } else {
            eprintln!("subtitle-lint: unexpected argument \"{}\"", arg);
            return ExitCode::from(2);
        }
    }

    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("usage: subtitle-lint [--config FILE] <file.srt|->");
            return ExitCode::from(2);
        }
    };

    let config = match config_path {
        Some(cp) => match Config::from_file(Path::new(&cp)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("subtitle-lint: {}: {}", cp, e);
                return ExitCode::from(2);
            }
        },
        None => Config::default(),
    };

    let findings = if path == "-" {
        let stdin = io::stdin();
        linter::lint(stdin.lock(), &config)
    } else {
        match File::open(&path) {
            Ok(file) => linter::lint(BufReader::new(file), &config),
            Err(e) => {
                eprintln!("subtitle-lint: cannot open {}: {}", path, e);
                return ExitCode::from(2);
            }
        }
    };

    let mut had_error = false;
    for finding in &findings {
        had_error |= finding.severity == Severity::Error;
        println!("{}:{}: {}: {}", path, finding.line, finding.severity, finding.message);
    }

    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
