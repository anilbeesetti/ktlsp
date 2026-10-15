//! Bounded UTF-16 coordinates and atomic, versioned document changes.
//!
//! Positions exclude line terminators. LF, CRLF, and lone CR terminate lines;
//! the byte between CR and LF has no position. Invalid coordinates are errors,
//! including UTF-16 surrogate interiors, rather than clamped positions.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Maximum UTF-8 document size, including line terminators.
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum changes applied atomically in one notification.
pub const MAX_CHANGES: usize = 1024;

/// An LSP position, with `character` measured in UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// A range replaces part of the current text; no range replaces all of it.
/// When supplied, `range_length` must match the replaced text's UTF-16 length,
/// including for full replacements and including CR/LF code units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
    pub range: Option<Range>,
    pub range_length: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    DocumentTooLarge { bytes: usize, limit: usize },
    TooManyChanges { count: usize, limit: usize },
    StaleVersion { current: i32, proposed: i32 },
    InvalidPosition(Position),
    InvalidOffset(usize),
    ReversedRange(Range),
    RangeLengthMismatch { expected: u32, actual: u32 },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentTooLarge { bytes, limit } => {
                write!(f, "document size {bytes} exceeds {limit} bytes")
            }
            Self::TooManyChanges { count, limit } => {
                write!(f, "change count {count} exceeds {limit}")
            }
            Self::StaleVersion { current, proposed } => {
                write!(
                    f,
                    "version {proposed} must exceed current version {current}"
                )
            }
            Self::InvalidPosition(position) => write!(f, "invalid UTF-16 position {position:?}"),
            Self::InvalidOffset(offset) => write!(f, "invalid byte offset {offset}"),
            Self::ReversedRange(range) => write!(f, "reversed range {range:?}"),
            Self::RangeLengthMismatch { expected, actual } => {
                write!(
                    f,
                    "rangeLength {expected} differs from replaced length {actual}"
                )
            }
        }
    }
}

impl Error for TextError {}

fn check_size(bytes: usize) -> Result<(), TextError> {
    if bytes > MAX_DOCUMENT_BYTES {
        return Err(TextError::DocumentTooLarge {
            bytes,
            limit: MAX_DOCUMENT_BYTES,
        });
    }
    Ok(())
}

/// Convert a strict UTF-16 position into a UTF-8 byte offset.
/// An empty document has position `(0, 0)`; a trailing newline creates a final
/// empty line. Input exceeding [`MAX_DOCUMENT_BYTES`] is rejected.
pub fn position_to_offset(text: &str, position: Position) -> Result<usize, TextError> {
    check_size(text.len())?;
    let bytes = text.as_bytes();
    let mut start = 0;
    for _ in 0..position.line {
        let Some(relative_end) = bytes[start..]
            .iter()
            .position(|b| matches!(b, b'\r' | b'\n'))
        else {
            return Err(TextError::InvalidPosition(position));
        };
        let end = start + relative_end;
        start = end
            + if bytes[end..].starts_with(b"\r\n") {
                2
            } else {
                1
            };
    }
    let end = start
        + bytes[start..]
            .iter()
            .position(|b| matches!(b, b'\r' | b'\n'))
            .unwrap_or(bytes.len() - start);
    position_in_line(text, position, start, end)
}

fn position_in_line(
    text: &str,
    position: Position,
    start: usize,
    end: usize,
) -> Result<usize, TextError> {
    let mut units = 0;
    for (relative_offset, character) in text[start..end].char_indices() {
        if units == position.character {
            return Ok(start + relative_offset);
        }
        units += character.len_utf16() as u32;
        if units > position.character {
            return Err(TextError::InvalidPosition(position));
        }
    }
    if units == position.character {
        Ok(end)
    } else {
        Err(TextError::InvalidPosition(position))
    }
}

/// Convert a byte offset to a strict UTF-16 position.
/// Offsets at newline starts map to the preceding line end. UTF-8 interiors,
/// CRLF interiors, offsets beyond EOF, and oversized input are rejected.
pub fn offset_to_position(text: &str, offset: usize) -> Result<Position, TextError> {
    check_size(text.len())?;
    if !text.is_char_boundary(offset) {
        return Err(TextError::InvalidOffset(offset));
    }
    let mut position = Position {
        line: 0,
        character: 0,
    };
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if index == offset {
            return Ok(position);
        }
        match character {
            '\r' | '\n' => {
                if character == '\r' && characters.peek().is_some_and(|(_, c)| *c == '\n') {
                    // The consumed CR ends the line, but its following LF is
                    // part of the same terminator and cannot be addressed.
                    if offset == index + 1 {
                        return Err(TextError::InvalidOffset(offset));
                    }
                    characters.next();
                }
                position.line += 1;
                position.character = 0;
            }
            _ => position.character += character.len_utf16() as u32,
        }
    }
    Ok(position) // Boundary validation above ensures the only remaining offset is EOF.
}

// At most text.len() + 1 entries; offsets fit u32 under MAX_DOCUMENT_BYTES.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LineIndex(Vec<u32>);

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        let mut bytes = text.bytes().enumerate().peekable();
        while let Some((offset, byte)) = bytes.next() {
            if matches!(byte, b'\r' | b'\n') {
                let end = if byte == b'\r' && bytes.peek().is_some_and(|(_, b)| *b == b'\n') {
                    bytes.next();
                    offset + 2
                } else {
                    offset + 1
                };
                starts.push(end as u32);
            }
        }
        Self(starts)
    }

    fn position_to_offset(&self, text: &str, position: Position) -> Result<usize, TextError> {
        let line = position.line as usize;
        let start = *self
            .0
            .get(line)
            .ok_or(TextError::InvalidPosition(position))? as usize;
        let mut end = self.0.get(line + 1).map_or(text.len(), |end| *end as usize);
        if end > start && text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        if end > start && text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        position_in_line(text, position, start, end)
    }

    fn replace(&mut self, text: &mut String, start: usize, end: usize, replacement: &str) {
        // Include the preceding line so CR/LF pairing across either seam is rescanned.
        let first =
            (self.0.partition_point(|offset| *offset as usize <= start) - 1).saturating_sub(1);
        let suffix = self.0.partition_point(|offset| *offset as usize <= end);
        let anchor = self.0[first] as usize;
        let suffix_start = self
            .0
            .get(suffix)
            .map_or(text.len(), |offset| *offset as usize);
        let new_suffix_start = suffix_start - (end - start) + replacement.len();
        text.replace_range(start..end, replacement);
        let mut rescanned = Self::new(&text[anchor..new_suffix_start]).0;
        if suffix < self.0.len() {
            // The stable suffix already owns the boundary after its terminator.
            rescanned.pop();
        }
        for offset in &mut rescanned {
            *offset += anchor as u32;
        }
        for offset in &mut self.0[suffix..] {
            *offset = (*offset as usize - (end - start) + replacement.len()) as u32;
        }
        self.0.splice(first..suffix, rescanned);
    }
}

/// A valid bounded document. Fields stay private so edits cannot bypass the
/// size/version invariants; use [`Document::text`] and [`Document::version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    version: i32,
    text: String,
    lines: LineIndex,
}

impl Document {
    pub fn new(version: i32, text: String) -> Result<Self, TextError> {
        check_size(text.len())?;
        let lines = LineIndex::new(&text);
        Ok(Self {
            version,
            text,
            lines,
        })
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Apply changes sequentially to a temporary document, committing the text
    /// and version only if every change succeeds. An empty batch advances the
    /// version. Every intermediate document must respect the byte limit.
    pub fn apply_changes(&mut self, version: i32, changes: &[TextChange]) -> Result<(), TextError> {
        if version <= self.version {
            return Err(TextError::StaleVersion {
                current: self.version,
                proposed: version,
            });
        }
        if changes.len() > MAX_CHANGES {
            return Err(TextError::TooManyChanges {
                count: changes.len(),
                limit: MAX_CHANGES,
            });
        }
        let mut text = self.text.clone();
        let mut lines = self.lines.clone();
        for change in changes {
            let (start, end) = match change.range {
                Some(range) => {
                    if range.start > range.end {
                        return Err(TextError::ReversedRange(range));
                    }
                    (
                        lines.position_to_offset(&text, range.start)?,
                        lines.position_to_offset(&text, range.end)?,
                    )
                }
                None => (0, text.len()),
            };
            if let Some(expected) = change.range_length {
                let actual = text[start..end].encode_utf16().count() as u32;
                if expected != actual {
                    return Err(TextError::RangeLengthMismatch { expected, actual });
                }
            }
            // Both terms are bounded by actual String lengths. Overflow is
            // impossible for allocated strings, but saturating arithmetic keeps
            // the resource decision explicit before replace_range allocates.
            let bytes = (text.len() - (end - start)).saturating_add(change.text.len());
            check_size(bytes)?;
            lines.replace(&mut text, start, end, &change.text);
        }
        self.text = text;
        self.lines = lines;
        self.version = version;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_positions_preserve_literal_unicode_and_terminator_boundaries() {
        for (text, expected) in [
            ("", vec![(0, 0, 0)]),
            (
                "a😀e\u{301}水",
                vec![
                    (0, 0, 0),
                    (0, 1, 1),
                    (0, 3, 5),
                    (0, 4, 6),
                    (0, 5, 8),
                    (0, 6, 11),
                ],
            ),
            (
                "a\r\n😀\rb\n",
                vec![
                    (0, 0, 0),
                    (0, 1, 1),
                    (1, 0, 3),
                    (1, 2, 7),
                    (2, 0, 8),
                    (2, 1, 9),
                    (3, 0, 10),
                ],
            ),
            ("\r\r\n\n", vec![(0, 0, 0), (1, 0, 1), (2, 0, 3), (3, 0, 4)]),
        ] {
            let lines = LineIndex::new(text);
            for line in 0..=5 {
                for character in 0..=12 {
                    let position = Position { line, character };
                    let result = expected
                        .iter()
                        .find(|(l, c, _)| *l == line && *c == character)
                        .map_or(
                            Err(TextError::InvalidPosition(position)),
                            |(_, _, offset)| Ok(*offset),
                        );
                    assert_eq!(lines.position_to_offset(text, position), result, "{text:?}");
                }
            }
            let position = Position {
                line: u32::MAX,
                character: u32::MAX,
            };
            assert_eq!(
                lines.position_to_offset(text, position),
                Err(TextError::InvalidPosition(position))
            );
        }
    }

    #[test]
    fn indexed_edits_rescan_both_newline_seams_and_preserve_suffix() {
        let alphabet = ["x", "😀", "\r", "\n"];
        for length in 0..=3 {
            for mut seed in 0..alphabet.len().pow(length) {
                let mut original = String::new();
                for _ in 0..length {
                    original.push_str(alphabet[seed % alphabet.len()]);
                    seed /= alphabet.len();
                }
                let positions: Vec<_> = (0..=original.len())
                    .filter(|offset| offset_to_position(&original, *offset).is_ok())
                    .collect();
                for &start in &positions {
                    for &end in positions.iter().filter(|end| **end >= start) {
                        for replacement in ["", "x", "😀", "\r", "\n", "\r\n", "\n\r"] {
                            // Retain later lines to exercise suffix shifting as well as EOF edits.
                            for tail in ["", "\r\nkeep\nlast"] {
                                let mut text = format!("{original}{tail}");
                                let mut lines = LineIndex::new(&text);
                                lines.replace(&mut text, start, end, replacement);
                                assert_eq!(
                                    lines,
                                    LineIndex::new(&text),
                                    "{original:?} {start}..{end} {replacement:?} {tail:?}"
                                );
                                assert!(lines.0.len() <= text.len() + 1);
                                for offset in 0..=text.len() {
                                    if let Ok(position) = offset_to_position(&text, offset) {
                                        assert_eq!(
                                            lines.position_to_offset(&text, position),
                                            Ok(offset)
                                        );
                                    }
                                }
                                let eof = text.len();
                                lines.replace(&mut text, eof, eof, "\n😀");
                                assert_eq!(lines, LineIndex::new(&text));
                            }
                        }
                    }
                }
            }
        }
    }
}
