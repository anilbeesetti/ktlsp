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

/// A valid bounded document. Fields stay private so edits cannot bypass the
/// size/version invariants; use [`Document::text`] and [`Document::version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    version: i32,
    text: String,
}

impl Document {
    pub fn new(version: i32, text: String) -> Result<Self, TextError> {
        check_size(text.len())?;
        Ok(Self { version, text })
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
        for change in changes {
            let (start, end) = match change.range {
                Some(range) => {
                    if range.start > range.end {
                        return Err(TextError::ReversedRange(range));
                    }
                    (
                        position_to_offset(&text, range.start)?,
                        position_to_offset(&text, range.end)?,
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
            text.replace_range(start..end, &change.text);
        }
        self.text = text;
        self.version = version;
        Ok(())
    }
}
