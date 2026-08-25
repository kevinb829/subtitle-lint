use std::io::{self, BufRead, Lines};

use crate::timestamp::Timestamp;

/// A single cue's worth of raw lines, still tagged with their original
/// line numbers so later stages can report accurate positions.
pub struct RawBlock {
    pub lines: Vec<(usize, String)>,
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
    pub fn from_raw(raw: &RawBlock) -> Result<Subtitle, BlockError> {
        let mut lines = raw.lines.iter();

        let (index_line, index_text) = lines.next().ok_or_else(|| BlockError::Malformed {
            line: 0,
            reason: "empty block".to_string(),
        })?;
        if index_text.trim().parse::<u32>().is_err() {
            return Err(BlockError::Malformed {
                line: *index_line,
                reason: format!("expected a numeric cue index, found \"{}\"", index_text.trim()),
            });
        }

        let (timing_line, timing_text) = lines.next().ok_or_else(|| BlockError::Malformed {
            line: *index_line,
            reason: "cue has an index but no timing line".to_string(),
        })?;

        let (start_raw, end_raw) =
            timing_text.split_once("-->").ok_or_else(|| BlockError::Malformed {
                line: *timing_line,
                reason: format!("timing line has no \"-->\": \"{}\"", timing_text),
            })?;

        let start = Timestamp::parse(start_raw).ok_or_else(|| BlockError::Malformed {
            line: *timing_line,
            reason: format!("could not parse start time \"{}\"", start_raw.trim()),
        })?;
        let end = Timestamp::parse(end_raw).ok_or_else(|| BlockError::Malformed {
            line: *timing_line,
            reason: format!("could not parse end time \"{}\"", end_raw.trim()),
        })?;

        let text = lines.map(|(n, s)| (*n, s.clone())).collect();

        Ok(Subtitle {
            timing_line: *timing_line,
            start,
            end,
            text,
        })
    }
}
