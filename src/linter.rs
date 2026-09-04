use std::fmt;
use std::io::BufRead;

use crate::parser::{BlockError, Format, Subtitle, SubtitleReader};
use crate::timestamp::Timestamp;

// Common subtitling guideline (e.g. Netflix timed text style guides) caps
// a single displayed line around this many characters for readability.
const MAX_LINE_LEN: usize = 42;

// Below this normalized length, short cues ("Yes." / "No.") are too likely
// to collide by chance to be worth flagging as near-duplicates.
const NEAR_DUPLICATE_MIN_LEN: usize = 8;

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
///
/// The format (SRT or VTT) is sniffed from the stream itself rather than
/// passed in, since it's determined entirely by whether the first block is
/// a `WEBVTT` header — the caller (stdin included) doesn't need to know.
pub fn lint<R: BufRead>(reader: R) -> Vec<Finding> {
    let mut reader = SubtitleReader::new(reader);
    let mut findings = Vec::new();
    // Only the previous cue's end time and text are kept around, not the
    // cue itself — enough to catch back-to-back overlaps and duplicates
    // without holding more than one cue's worth of state at a time.
    let mut previous_end: Option<Timestamp> = None;
    let mut previous_text: Option<(usize, String)> = None;

    let mut pending = reader.next_block();
    let format = if matches!(&pending, Some(Ok(raw)) if raw.is_webvtt_header()) {
        pending = reader.next_block(); // consume the header, it's not a cue
        Format::Vtt
    } else {
        Format::Srt
    };

    while let Some(block) = pending {
        pending = reader.next_block();

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

        if format == Format::Vtt && raw.is_skippable_vtt_block() {
            continue;
        }

        let subtitle = match Subtitle::from_raw(&raw, format) {
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
        check_duplicate_text(&subtitle, previous_text.as_ref(), &mut findings);

        previous_end = Some(subtitle.end);
        if !subtitle.text.is_empty() {
            let report_line = subtitle.text.first().map(|(n, _)| *n).unwrap_or(subtitle.timing_line);
            previous_text = Some((report_line, joined_cue_text(&subtitle)));
        }
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

fn joined_cue_text(subtitle: &Subtitle) -> String {
    subtitle
        .text
        .iter()
        .map(|(_, s)| s.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lowercases and strips everything but letters, digits, and single spaces
/// between them, so "Hello, world!" and "hello world" compare equal — the
/// kind of edit that leaves a cue's meaning unchanged but its text differs.
fn normalize_for_comparison(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            pending_space = true;
        }
        // other punctuation is dropped entirely
    }
    out
}

/// Edit distance between two strings, compared character by character.
/// Cue text is short (a couple of lines at most), so the O(n*m) table is
/// negligible even though it's computed once per cue.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut previous_row: Vec<usize> = (0..=b.len()).collect();
    let mut current_row = vec![0usize; b.len() + 1];

    for i in 1..=a.len() {
        current_row[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            current_row[j] = (previous_row[j] + 1)
                .min(current_row[j - 1] + 1)
                .min(previous_row[j - 1] + cost);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }

    previous_row[b.len()]
}

fn check_duplicate_text(subtitle: &Subtitle, previous: Option<&(usize, String)>, findings: &mut Vec<Finding>) {
    if subtitle.text.is_empty() {
        return;
    }
    let Some((previous_line, previous_raw)) = previous else {
        return;
    };

    let current_raw = joined_cue_text(subtitle);
    let report_line = subtitle.text.first().map(|(n, _)| *n).unwrap_or(subtitle.timing_line);

    if current_raw == *previous_raw {
        findings.push(Finding::new(
            report_line,
            Severity::Warning,
            format!("cue text is identical to the previous cue at line {}", previous_line),
        ));
        return;
    }

    let current_norm = normalize_for_comparison(&current_raw);
    let previous_norm = normalize_for_comparison(previous_raw);
    if current_norm.is_empty() || previous_norm.is_empty() {
        return;
    }

    if current_norm == previous_norm {
        findings.push(Finding::new(
            report_line,
            Severity::Warning,
            format!(
                "cue text is nearly identical to the previous cue at line {} (differs only in case or punctuation)",
                previous_line
            ),
        ));
        return;
    }

    let max_len = current_norm.chars().count().max(previous_norm.chars().count());
    if max_len < NEAR_DUPLICATE_MIN_LEN {
        return;
    }

    // Within roughly a 20% edit distance of each other counts as a near
    // duplicate — enough to catch a word swapped or a typo fixed between
    // two cues that should probably have been merged or left alone.
    let distance = levenshtein(&current_norm, &previous_norm);
    if distance * 5 <= max_len {
        findings.push(Finding::new(
            report_line,
            Severity::Warning,
            format!("cue text is nearly identical to the previous cue at line {}", previous_line),
        ));
    }
}
