use std::io::{self, BufRead, Lines};

use crate::timestamp::Timestamp;

/// Which subtitle format a stream is being read as. Detected once, from the
/// first block of the file, and then applied to every cue after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Srt,
    Vtt,
}

/// A single cue's worth of raw lines, still tagged with their original
/// line numbers so later stages can report accurate positions.
pub struct RawBlock {
    pub lines: Vec<(usize, String)>,
}

impl RawBlock {
    /// True for the file's leading `WEBVTT` header block, which carries no
    /// timing of its own and is optionally followed by free-text metadata
    /// on the same line.
    pub fn is_webvtt_header(&self) -> bool {
        self.lines.first().map_or(false, |(_, line)| {
            let line = line.trim_start_matches('\u{feff}'); // optional BOM
            line == "WEBVTT" || line.starts_with("WEBVTT ") || line.starts_with("WEBVTT\t")
        })
    }

    /// True for WebVTT `NOTE`, `STYLE`, and `REGION` blocks, none of which
    /// are cues and all of which should be skipped rather than parsed.
    pub fn is_skippable_vtt_block(&self) -> bool {
        self.lines.first().map_or(false, |(_, line)| {
            let line = line.trim_start();
            line.starts_with("NOTE") || line.starts_with("STYLE") || line.starts_with("REGION")
        })
    }
}

pub enum BlockError {
    Io(io::Error),
    Malformed { line: usize, reason: String },
}

/// Pulls one subtitle block at a time off a buffered reader.
///
/// The reader only ever holds the lines of the block currently being
/// assembled — once a block is handed to the caller and dropped, that
/// memory is freed. A multi-hour, multi-thousand-cue file costs the same
/// few hundred bytes of state as a five-line one.
pub struct SubtitleReader<R> {
    lines: Lines<R>,
    line_no: usize,
}

impl<R: BufRead> SubtitleReader<R> {
    pub fn new(reader: R) -> Self {
        SubtitleReader {
            lines: reader.lines(),
            line_no: 0,
        }
    }

    /// Returns the next non-empty block, or None once the input is
    /// exhausted. Blank lines between blocks are consumed here.
    pub fn next_block(&mut self) -> Option<Result<RawBlock, BlockError>> {
        let mut block_lines: Vec<(usize, String)> = Vec::new();

        loop {
            match self.lines.next() {
                None => {
                    return if block_lines.is_empty() {
                        None
                    } else {
                        Some(Ok(RawBlock { lines: block_lines }))
                    };
                }
                Some(Err(e)) => return Some(Err(BlockError::Io(e))),
                Some(Ok(line)) => {
                    self.line_no += 1;
                    if line.trim().is_empty() {
                        if block_lines.is_empty() {
                            continue; // leading blank line between blocks
                        }
                        return Some(Ok(RawBlock { lines: block_lines }));
                    }
                    block_lines.push((self.line_no, line));
                }
            }
        }
    }
}

pub struct Subtitle {
    pub timing_line: usize,
    pub start: Timestamp,
    pub end: Timestamp,
    pub text: Vec<(usize, String)>,
}

impl Subtitle {
    /// Parses a raw block according to `format`. SRT blocks are always
    /// `index, timing, text...`. VTT blocks are `[identifier], timing,
    /// text...` — the identifier is optional, so a VTT block's timing line
    /// is whichever of the first two lines contains "-->". VTT timing lines
    /// may also carry cue settings (e.g. "align:start line:0") after the
    /// end timestamp, which are accepted but ignored.
    pub fn from_raw(raw: &RawBlock, format: Format) -> Result<Subtitle, BlockError> {
        let lines = &raw.lines;

        let (first_line, first_text) = lines.first().ok_or_else(|| BlockError::Malformed {
            line: 0,
            reason: "empty block".to_string(),
        })?;

        let timing_idx = match format {
            Format::Srt => {
                if first_text.trim().parse::<u32>().is_err() {
                    return Err(BlockError::Malformed {
                        line: *first_line,
                        reason: format!(
                            "expected a numeric cue index, found \"{}\"",
                            first_text.trim()
                        ),
                    });
                }
                1
            }
            Format::Vtt => {
                if first_text.contains("-->") {
                    0
                } else {
                    1
                }
            }
        };

        let (timing_line, timing_text) = lines.get(timing_idx).ok_or_else(|| BlockError::Malformed {
            line: *first_line,
            reason: "block is missing a timing line".to_string(),
        })?;

        let (start_raw, rest) =
            timing_text.split_once("-->").ok_or_else(|| BlockError::Malformed {
                line: *timing_line,
                reason: format!("timing line has no \"-->\": \"{}\"", timing_text),
            })?;

        // SRT has nothing after the end timestamp; VTT may have cue
        // settings, so only take the first whitespace-separated token.
        let end_raw = match format {
            Format::Srt => rest,
            Format::Vtt => rest.trim_start().split_whitespace().next().unwrap_or(rest),
        };

        let parse_ts: fn(&str) -> Option<Timestamp> = match format {
            Format::Srt => Timestamp::parse,
            Format::Vtt => Timestamp::parse_vtt,
        };

        let start = parse_ts(start_raw).ok_or_else(|| BlockError::Malformed {
            line: *timing_line,
            reason: format!("could not parse start time \"{}\"", start_raw.trim()),
        })?;
        let end = parse_ts(end_raw).ok_or_else(|| BlockError::Malformed {
            line: *timing_line,
            reason: format!("could not parse end time \"{}\"", end_raw.trim()),
        })?;

        let text = lines[timing_idx + 1..]
            .iter()
            .map(|(n, s)| (*n, s.clone()))
            .collect();

        Ok(Subtitle {
            timing_line: *timing_line,
            start,
            end,
            text,
        })
    }
}
