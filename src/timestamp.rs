use std::fmt;

/// A SubRip timestamp, stored as milliseconds since 00:00:00,000.
///
/// Keeping this as a single integer instead of four separate fields makes
/// ordering and duration arithmetic trivial and free of edge cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    millis: u32,
}

impl Timestamp {
    /// Parses "HH:MM:SS,mmm". Returns None for anything else, including the
    /// "HH:MM:SS.mmm" variant some tools emit — that's a distinct lint
    /// finding, not a silently-accepted format.
    pub fn parse(raw: &str) -> Option<Timestamp> {
        let raw = raw.trim();
        let (time_part, ms_part) = raw.split_once(',')?;
        if ms_part.len() != 3 || !ms_part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let millis: u32 = ms_part.parse().ok()?;

        let mut fields = time_part.split(':');
        let hours: u32 = fields.next()?.parse().ok()?;
        let minutes: u32 = fields.next()?.parse().ok()?;
        let seconds: u32 = fields.next()?.parse().ok()?;
        if fields.next().is_some() {
            return None;
        }
        if minutes >= 60 || seconds >= 60 {
            return None;
        }

        Some(Timestamp {
            millis: ((hours * 3600 + minutes * 60 + seconds) * 1000) + millis,
        })
    }

    /// Parses a WebVTT timestamp: "HH:MM:SS.mmm" or, since WebVTT allows the
    /// hours field to be dropped for cues under an hour, "MM:SS.mmm".
    pub fn parse_vtt(raw: &str) -> Option<Timestamp> {
        let raw = raw.trim();
        let (time_part, ms_part) = raw.split_once('.')?;
        if ms_part.len() != 3 || !ms_part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let millis: u32 = ms_part.parse().ok()?;

        let fields: Vec<&str> = time_part.split(':').collect();
        let (hours, minutes, seconds): (u32, u32, u32) = match fields.as_slice() {
            [h, m, s] => (h.parse().ok()?, m.parse().ok()?, s.parse().ok()?),
            [m, s] => (0, m.parse().ok()?, s.parse().ok()?),
            _ => return None,
        };
        if minutes >= 60 || seconds >= 60 {
            return None;
        }

        Some(Timestamp {
            millis: ((hours * 3600 + minutes * 60 + seconds) * 1000) + millis,
        })
    }

    pub fn as_millis(&self) -> u32 {
        self.millis
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = self.millis % 1000;
        let total_secs = self.millis / 1000;
        let s = total_secs % 60;
        let total_mins = total_secs / 60;
        let m = total_mins % 60;
        let h = total_mins / 60;
        write!(f, "{:02}:{:02}:{:02},{:03}", h, m, s, ms)
    }
}
