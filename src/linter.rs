use std::fmt;
use std::io::BufRead;

use crate::parser::{BlockError, Subtitle, SubtitleReader};
use crate::timestamp::Timestamp;

// Common subtitling guideline (e.g. Netflix timed text style guides) caps
// a single displayed line around this many characters for readability.
const MAX_LINE_LEN: usize = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug)]
pub struct Finding {
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    fn new(line: usize, severity: Severity, message: impl Into<String>) -> Finding {
        Finding {
            line,
            severity,
            message: message.into(),
        }
    }
}

/// Lints a subtitle stream, reading and checking one cue at a time so the
/// caller can hand this a multi-gigabyte file without it landing in memory.
pub fn lint<R: BufRead>(reader: R) -> Vec<Finding> {
    let mut reader = SubtitleReader::new(reader);
    let mut findings = Vec::new();
    // Only the previous cue's end time is kept around, not the cue itself.
    let mut previous_end: Option<Timestamp> = None;

    while let Some(block) = reader.next_block() {
        let raw = match block {
            Ok(raw) => raw,
            Err(BlockError::Io(e)) => {
                findings.push(Finding::new(0, Severity::Error, format!("read error: {}", e)));
                break;
            }
            Err(BlockError::Malformed { line, reason }) => {
                findings.push(Finding::new(line, Severity::Error, reason));
                continue;
            }
        };

        let subtitle = match Subtitle::from_raw(&raw) {
            Ok(s) => s,
            Err(BlockError::Io(e)) => {
                findings.push(Finding::new(0, Severity::Error, format!("read error: {}", e)));
                break;
            }
            Err(BlockError::Malformed { line, reason }) => {
                findings.push(Finding::new(line, Severity::Error, reason));
                continue;
            }
        };

        check_ordering(&subtitle, &mut findings);
        check_overlap(&subtitle, previous_end, &mut findings);
        check_text(&subtitle, &mut findings);

        previous_end = Some(subtitle.end);
    }

    findings
}

fn check_ordering(subtitle: &Subtitle, findings: &mut Vec<Finding>) {
    if subtitle.end <= subtitle.start {
        findings.push(Finding::new(
            subtitle.timing_line,
            Severity::Error,
            format!(
                "end time {} is not after start time {}",
                subtitle.end, subtitle.start
            ),
        ));
    }
}

fn check_overlap(subtitle: &Subtitle, previous_end: Option<Timestamp>, findings: &mut Vec<Finding>) {
    if let Some(previous_end) = previous_end {
        if subtitle.start < previous_end {
            findings.push(Finding::new(
                subtitle.timing_line,
                Severity::Warning,
                format!("cue starts at {} before the previous cue ends at {}", subtitle.start, previous_end),
            ));
        }
    }
}

fn check_text(subtitle: &Subtitle, findings: &mut Vec<Finding>) {
    if subtitle.text.is_empty() {
        findings.push(Finding::new(
            subtitle.timing_line,
            Severity::Warning,
            "cue has a timing line but no text",
        ));
        return;
    }

    for (line, text) in &subtitle.text {
        let len = text.chars().count();
        if len > MAX_LINE_LEN {
            findings.push(Finding::new(
                *line,
                Severity::Warning,
                format!("line is {} characters, longer than the recommended {}", len, MAX_LINE_LEN),
            ));
        }
    }
}
