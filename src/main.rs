mod linter;
mod parser;
mod timestamp;

use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::process::ExitCode;

use linter::Severity;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("usage: subtitle-lint <file.srt|->");
            return ExitCode::from(2);
        }
    };

    let findings = if path == "-" {
        let stdin = io::stdin();
        linter::lint(stdin.lock())
    } else {
        match File::open(&path) {
            Ok(file) => linter::lint(BufReader::new(file)),
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
