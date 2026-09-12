// Runs the built binary against the fixtures in tests/fixtures, one file per
// rule, and checks its stdout and exit code. This exercises the CLI exactly
// as a user would invoke it, rather than calling the linter as a library.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn run(path: &Path) -> (String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_subtitle-lint"))
        .arg(path)
        .output()
        .expect("failed to run subtitle-lint");
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf8");
    let code = output.status.code().expect("process terminated by signal");
    (stdout, code)
}

#[test]
fn backwards_timing_is_an_error() {
    let path = fixture("backwards_timing.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:2: error: end time 00:00:03,000 is not after start time 00:00:05,000\n",
            path.display()
        )
    );
    assert_eq!(code, 1);
}

#[test]
fn overlap_is_a_warning() {
    let path = fixture("overlap.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:6: warning: cue starts at 00:00:03,000 before the previous cue ends at 00:00:04,000\n",
            path.display()
        )
    );
    assert_eq!(code, 0);
}

#[test]
fn empty_cue_is_a_warning() {
    let path = fixture("empty_cue.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!("{}:2: warning: cue has a timing line but no text\n", path.display())
    );
    assert_eq!(code, 0);
}

#[test]
fn long_line_is_a_warning() {
    let path = fixture("long_line.srt");
    let (stdout, code) = run(&path);
    assert_eq!(code, 0);
    let expected_prefix = format!("{}:3: warning: line is ", path.display());
    assert!(stdout.starts_with(&expected_prefix), "unexpected output: {}", stdout);
    assert!(stdout.contains("longer than the recommended 42"), "unexpected output: {}", stdout);
}

#[test]
fn identical_text_is_flagged() {
    let path = fixture("duplicate_text.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:7: warning: cue text is identical to the previous cue at line 3\n",
            path.display()
        )
    );
    assert_eq!(code, 0);
}

#[test]
fn punctuation_only_difference_is_flagged() {
    let path = fixture("near_duplicate_text.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:7: warning: cue text is nearly identical to the previous cue at line 3 (differs only in case or punctuation)\n",
            path.display()
        )
    );
    assert_eq!(code, 0);
}

#[test]
fn near_duplicate_by_edit_distance_is_flagged() {
    let path = fixture("edit_distance_duplicate.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:7: warning: cue text is nearly identical to the previous cue at line 3\n",
            path.display()
        )
    );
    assert_eq!(code, 0);
}

#[test]
fn malformed_cue_index_is_an_error_and_does_not_stop_the_run() {
    let path = fixture("malformed.srt");
    let (stdout, code) = run(&path);
    assert_eq!(
        stdout,
        format!(
            "{}:5: error: expected a numeric cue index, found \"not-a-number\"\n",
            path.display()
        )
    );
    assert_eq!(code, 1);
}

#[test]
fn clean_vtt_has_no_findings() {
    let path = fixture("clean.vtt");
    let (stdout, code) = run(&path);
    assert_eq!(stdout, "");
    assert_eq!(code, 0);
}

#[test]
fn vtt_notes_style_and_identifiers_are_handled() {
    let path = fixture("vtt_with_notes.vtt");
    let (stdout, code) = run(&path);
    assert_eq!(stdout, "");
    assert_eq!(code, 0);
}
