# subtitle-lint

A linter for SubRip (`.srt`) and WebVTT (`.vtt`) subtitle files. It checks
cue timing and text for the kind of mistakes that don't show up until a
video plays: timestamps that run backwards, cues that overlap the one
before them, empty cues, and lines too long to read comfortably.

## Why

Subtitle files are usually fine until they aren't — a batch export drops a
frame count wrong, someone hand-edits a `.srt` in a text editor and breaks
the blank-line spacing between cues, or two cues end up overlapping after a
timing pass. None of that throws an error in a video player; it just looks
wrong on screen. This checks the file before it ships.

The other reason this exists: subtitle files for a two-hour film can run to
tens of thousands of lines, and a batch job might be linting hundreds of
them. `subtitle-lint` reads one cue at a time off a buffered reader and
never holds more than the current cue in memory, so file size doesn't
translate into memory use.

## Usage

```
$ subtitle-lint movie.srt
movie.srt:14: error: end time 00:00:22,100 is not after start time 00:00:24,000
movie.srt:29: warning: cue starts at 00:01:03,500 before the previous cue ends at 00:01:04,200
movie.srt:41: warning: line is 58 characters, longer than the recommended 42
movie.srt:67: warning: cue has a timing line but no text
```

Exit code is `1` if any error-severity finding was reported, `0` otherwise
(warnings alone don't fail the run). Pass `-` to read from stdin instead of
a file:

```
$ curl -s https://example.com/captions.srt | subtitle-lint -
```

Format is detected from the content, not the file extension or a flag: if
the first block is a `WEBVTT` header, the rest of the stream is read as
WebVTT (cue identifiers, cue settings, and `NOTE`/`STYLE`/`REGION` blocks
are all handled); otherwise it's read as SRT. This is what lets `-` work
for either format.

## What it checks today

- Malformed cue index or timing line
- End time not after start time
- A cue starting before the previous one ends
- Empty cue text
- Text lines longer than 42 characters

## Building

Standard library only — no dependencies to fetch.

```
cargo build --release
```

## Roadmap

See the roadmap in the project's issue tracker / commit history for what's
planned next: duplicate-text detection, configurable line length, and a
`--fix` mode for the mechanical cases.

## License

MIT, see [LICENSE](LICENSE).
