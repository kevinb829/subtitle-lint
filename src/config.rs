use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// Rule settings, loaded from a plain `key = value` config file or left at
/// their defaults. Kept separate from the linter so the file format can
/// change without touching the checks themselves.
#[derive(Debug, Clone)]
pub struct Config {
    pub max_line_length: usize,
    pub check_overlap: bool,
    pub check_duplicate_text: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_line_length: 42,
            check_overlap: true,
            check_duplicate_text: true,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse { line: usize, reason: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "{}", e),
            ConfigError::Parse { line, reason } => write!(f, "line {}: {}", line, reason),
        }
    }
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Config, ConfigError> {
        let contents = fs::read_to_string(path).map_err(ConfigError::Io)?;
        Config::parse(&contents)
    }

    fn parse(contents: &str) -> Result<Config, ConfigError> {
        let mut config = Config::default();

        for (i, raw_line) in contents.lines().enumerate() {
            let line_no = i + 1;
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| ConfigError::Parse {
                line: line_no,
                reason: format!("expected \"key = value\", found \"{}\"", raw_line.trim()),
            })?;
            let key = key.trim();
            let value = value.trim();

            match key {
                "max_line_length" => {
                    config.max_line_length = value.parse().map_err(|_| ConfigError::Parse {
                        line: line_no,
                        reason: format!(
                            "max_line_length must be a positive integer, found \"{}\"",
                            value
                        ),
                    })?;
                }
                "check_overlap" => config.check_overlap = parse_bool(value, line_no)?,
                "check_duplicate_text" => config.check_duplicate_text = parse_bool(value, line_no)?,
                other => {
                    return Err(ConfigError::Parse {
                        line: line_no,
                        reason: format!("unknown config key \"{}\"", other),
                    });
                }
            }
        }

        Ok(config)
    }
}

fn parse_bool(value: &str, line: usize) -> Result<bool, ConfigError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ConfigError::Parse {
            line,
            reason: format!("expected \"true\" or \"false\", found \"{}\"", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.max_line_length, 42);
        assert!(config.check_overlap);
        assert!(config.check_duplicate_text);
    }

    #[test]
    fn overrides_and_ignores_comments_and_blank_lines() {
        let config = Config::parse(
            "\n# a comment\nmax_line_length = 60\ncheck_duplicate_text = false  # trailing comment\n",
        )
        .unwrap();
        assert_eq!(config.max_line_length, 60);
        assert!(config.check_overlap);
        assert!(!config.check_duplicate_text);
    }

    #[test]
    fn rejects_unknown_key() {
        let err = Config::parse("frobnicate = true").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }

    #[test]
    fn rejects_bad_bool() {
        let err = Config::parse("check_overlap = maybe").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }

    #[test]
    fn rejects_bad_line_length() {
        let err = Config::parse("max_line_length = wide").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }

    #[test]
    fn rejects_line_without_equals() {
        let err = Config::parse("max_line_length").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }
}
