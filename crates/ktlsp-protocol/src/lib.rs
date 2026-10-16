//! Bounded LSP byte framing, independent of JSON-RPC parsing and session state.
//!
//! Framing errors are fatal: callers must discard the stream rather than attempt
//! resynchronization. A fully read payload may be parsed separately, with JSON
//! errors handled by the session. This module never writes logs to stdout.

use std::collections::TryReserveError;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::task::Poll;

/// Maximum complete header section, including the terminating CRLF-CRLF.
pub const MAX_HEADER_BYTES: usize = 8 * 1024;
/// Maximum payload size, measured in bytes rather than Unicode characters.
pub const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

/// A fatal transport error. Some bytes may already have been read or written.
#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    HeaderTooLarge,
    PayloadTooLarge,
    InvalidHeader,
    MissingContentLength,
    DuplicateContentLength,
    InvalidContentLength,
    InvalidContentType,
    UnsupportedCharset,
    TruncatedHeader,
    TruncatedPayload,
    Allocation(TryReserveError),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "frame I/O failed: {error}"),
            Self::HeaderTooLarge => write!(f, "frame headers exceed {MAX_HEADER_BYTES} bytes"),
            Self::PayloadTooLarge => write!(f, "frame payload exceeds {MAX_PAYLOAD_BYTES} bytes"),
            Self::InvalidHeader => f.write_str("invalid ASCII CRLF header grammar"),
            Self::MissingContentLength => f.write_str("missing Content-Length header"),
            Self::DuplicateContentLength => f.write_str("duplicate Content-Length header"),
            Self::InvalidContentLength => f.write_str("Content-Length must be an unsigned decimal"),
            Self::InvalidContentType => f.write_str("invalid Content-Type header"),
            Self::UnsupportedCharset => f.write_str("Content-Type charset must be UTF-8 or UTF8"),
            Self::TruncatedHeader => f.write_str("EOF inside frame headers"),
            Self::TruncatedPayload => f.write_str("EOF inside frame payload"),
            Self::Allocation(error) => write!(f, "cannot allocate bounded frame payload: {error}"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Allocation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Read one frame, leaving subsequent buffered frames untouched.
///
/// `Ok(None)` means EOF before any frame bytes. EOF inside a header or payload
/// is an error. Unknown syntactically valid ASCII headers are ignored. Header
/// names and charset values are case insensitive. JSON and UTF-8 payload
/// validation belong to the caller.
/// This blocking adapter cannot retain a partial frame across WouldBlock;
/// use `FrameReader::poll` for nonblocking/timed streams.
pub fn read_frame<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>, ProtocolError> {
    match FrameReader::default().poll(reader)? {
        Poll::Ready(frame) => Ok(frame),
        Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock).into()),
    }
}

/// One bounded, resumable frame. This owns only the current header/payload,
/// never a queue, and leaves all following frames in the caller's buffer.
/// Framing errors are terminal: discard this decoder and its input stream.
#[derive(Debug)]
pub struct FrameReader {
    header: Vec<u8>,
    payload: Vec<u8>,
    length: Option<usize>,
}
impl Default for FrameReader {
    fn default() -> Self {
        Self {
            header: Vec::with_capacity(256),
            payload: Vec::new(),
            length: None,
        }
    }
}
impl FrameReader {
    /// Continue until one complete frame, EOF, or a reader WouldBlock. Partial
    /// headers and payloads survive Pending; all grammar validation is shared
    /// with the blocking `read_frame` adapter.
    pub fn poll<R: BufRead>(
        &mut self,
        reader: &mut R,
    ) -> Result<Poll<Option<Vec<u8>>>, ProtocolError> {
        loop {
            if self
                .length
                .is_some_and(|length| self.payload.len() == length)
            {
                self.length = None;
                self.header.clear();
                return Ok(Poll::Ready(Some(std::mem::take(&mut self.payload))));
            }
            let available = match reader.fill_buf() {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(Poll::Pending);
                }
                Err(error) => return Err(error.into()),
            };
            if available.is_empty() {
                return if self.length.is_some() {
                    Err(ProtocolError::TruncatedPayload)
                } else if self.header.is_empty() {
                    Ok(Poll::Ready(None))
                } else {
                    Err(ProtocolError::TruncatedHeader)
                };
            }
            if let Some(length) = self.length {
                let consumed = available.len().min(length - self.payload.len());
                self.payload.extend_from_slice(&available[..consumed]);
                reader.consume(consumed);
                continue;
            }
            let mut consumed = 0;
            let mut complete = false;
            for &byte in available.iter().take(MAX_HEADER_BYTES - self.header.len()) {
                self.header.push(byte);
                consumed += 1;
                if self.header.ends_with(b"\r\n\r\n") {
                    complete = true;
                    break;
                }
            }
            reader.consume(consumed);
            if complete {
                let length = payload_length(&self.header)?;
                if length > MAX_PAYLOAD_BYTES {
                    return Err(ProtocolError::PayloadTooLarge);
                }
                self.payload
                    .try_reserve_exact(length)
                    .map_err(ProtocolError::Allocation)?;
                self.length = Some(length);
            } else if self.header.len() == MAX_HEADER_BYTES {
                return Err(ProtocolError::HeaderTooLarge);
            }
        }
    }
}

fn payload_length(header: &[u8]) -> Result<usize, ProtocolError> {
    // read_header guarantees the delimiter; each interior newline must still be
    // a CRLF pair. Retaining LF lets validation reject lone CR/LF and folding.
    let mut length = None;
    let mut saw_content_type = false;
    for line in header[..header.len() - 2].split_inclusive(|&byte| byte == b'\n') {
        let line = line
            .strip_suffix(b"\r\n")
            .ok_or(ProtocolError::InvalidHeader)?;
        let colon = line
            .iter()
            .position(|&byte| byte == b':')
            .ok_or(ProtocolError::InvalidHeader)?;
        let name = &line[..colon];
        let value = &line[colon + 1..];
        if name.is_empty()
            || !name.iter().all(|&byte| is_token(byte))
            || !value
                .iter()
                .all(|&byte| byte == b'\t' || (32..=126).contains(&byte))
        {
            return Err(ProtocolError::InvalidHeader);
        }
        let value = trim_space(value);
        if name.eq_ignore_ascii_case(b"content-length") {
            if length.is_some() {
                return Err(ProtocolError::DuplicateContentLength);
            }
            if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
                return Err(ProtocolError::InvalidContentLength);
            }
            let mut parsed = 0usize;
            for &digit in value {
                parsed = parsed
                    .checked_mul(10)
                    .and_then(|number| number.checked_add(usize::from(digit - b'0')))
                    .ok_or(ProtocolError::InvalidContentLength)?;
            }
            length = Some(parsed);
        } else if name.eq_ignore_ascii_case(b"content-type") {
            if saw_content_type {
                return Err(ProtocolError::InvalidContentType);
            }
            saw_content_type = true;
            validate_content_type(value)?;
        }
    }
    length.ok_or(ProtocolError::MissingContentLength)
}

fn trim_space(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ' | b'\t')) {
        value = &value[1..];
    }
    while matches!(value.last(), Some(b' ' | b'\t')) {
        value = &value[..value.len() - 1];
    }
    value
}

fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn validate_content_type(value: &[u8]) -> Result<(), ProtocolError> {
    let mut parts = value.split(|&byte| byte == b';');
    let media_type = trim_space(parts.next().ok_or(ProtocolError::InvalidContentType)?);
    let slash = media_type.iter().position(|&byte| byte == b'/');
    let Some(slash) = slash else {
        return Err(ProtocolError::InvalidContentType);
    };
    if slash == 0
        || slash + 1 == media_type.len()
        || !media_type[..slash].iter().all(|&byte| is_token(byte))
        || !media_type[slash + 1..].iter().all(|&byte| is_token(byte))
    {
        return Err(ProtocolError::InvalidContentType);
    }
    let mut saw_charset = false;
    for parameter in parts {
        let parameter = trim_space(parameter);
        let equal = parameter.iter().position(|&byte| byte == b'=');
        let Some(equal) = equal else {
            return Err(ProtocolError::InvalidContentType);
        };
        let name = trim_space(&parameter[..equal]);
        let value = trim_space(&parameter[equal + 1..]);
        let value = if value.starts_with(b"\"") && value.ends_with(b"\"") && value.len() >= 2 {
            &value[1..value.len() - 1]
        } else {
            value
        };
        if name.is_empty()
            || !name.iter().all(|&byte| is_token(byte))
            || value.is_empty()
            || !value.iter().all(|&byte| is_token(byte))
        {
            return Err(ProtocolError::InvalidContentType);
        }
        if name.eq_ignore_ascii_case(b"charset") {
            if saw_charset {
                return Err(ProtocolError::InvalidContentType);
            }
            saw_charset = true;
            if !value.eq_ignore_ascii_case(b"utf-8") && !value.eq_ignore_ascii_case(b"utf8") {
                return Err(ProtocolError::UnsupportedCharset);
            }
        }
    }
    Ok(())
}

/// Write a byte-counted frame, rejecting oversized payloads before any output.
///
/// This does not flush: the session controls flushing and output serialization.
/// A write failure is fatal because a partial frame may have reached the peer.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), ProtocolError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge);
    }
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(payload)?;
    Ok(())
}
