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
use linter::{Finding, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut config_path: Option<String> = None;
    let mut path: Option<String> = None;
    let mut format = OutputFormat::Text;

    while let Some(arg) = args.next() {
        if arg == "--config" {
            config_path = match args.next() {
                Some(p) => Some(p),
                None => {
                    eprintln!("subtitle-lint: --config requires a file path");
                    return ExitCode::from(2);
                }
            };
        } else if arg == "--format" {
            format = match args.next().as_deref() {
                Some("text") => OutputFormat::Text,
                Some("json") => OutputFormat::Json,
                Some(other) => {
                    eprintln!(
                        "subtitle-lint: unknown format \"{}\", expected \"text\" or \"json\"",
                        other
                    );
                    return ExitCode::from(2);
                }
                None => {
                    eprintln!("subtitle-lint: --format requires a value");
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
            eprintln!("usage: subtitle-lint [--config FILE] [--format text|json] <file.srt|->");
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

    let had_error = findings.iter().any(|f| f.severity == Severity::Error);

    match format {
        OutputFormat::Text => {
            for finding in &findings {
                println!("{}:{}: {}: {}", path, finding.line, finding.severity, finding.message);
            }
        }
        OutputFormat::Json => print_json(&path, &findings),
    }

    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints findings as a JSON array, one object per finding, so a CI step can
/// parse results instead of scraping the text format's colon-separated
/// columns.
fn print_json(path: &str, findings: &[Finding]) {
    println!("[");
    let last = findings.len().saturating_sub(1);
    for (i, finding) in findings.iter().enumerate() {
        let comma = if i == last { "" } else { "," };
        println!(
            "  {{\"file\": \"{}\", \"line\": {}, \"severity\": \"{}\", \"message\": \"{}\"}}{}",
            json_escape(path),
            finding.line,
            finding.severity,
            json_escape(&finding.message),
            comma
        );
    }
    println!("]");
}

/// Escapes a string for use inside a JSON string literal. Subtitle text and
/// file paths are the only untrusted input that ends up in JSON output, and
/// neither is expected to contain anything past the control-character range.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
