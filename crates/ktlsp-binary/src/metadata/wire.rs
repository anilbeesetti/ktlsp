//! Bounded flat d1 framing/fields; callers supply bytes from supported Outcome::Decoded.
// Wire rules derived from pinned Kotlin/protobuf sources; see fixture notices/provenance.
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::ops::Range;

/// Callers can tighten, but cannot raise, these hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            records: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Truncated,
    MalformedVarint,
    InvalidFieldNumber,
    InvalidWireType,
    UnsupportedGroup,
    NegativeLength,
    Limit(&'static str),
}

/// Offsets are absolute in the original d1 bytes; see METADATA_WIRE_SPEC.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at byte {}", self.kind, self.offset)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Field<'a> {
    pub number: u32,
    pub span: Range<usize>,
    pub raw: &'a [u8],
    pub value: Value<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Section<'a> {
    pub span: Range<usize>,
    pub raw: &'a [u8],
    pub fields: Vec<Field<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Sections<'a> {
    pub prefix_span: Range<usize>,
    pub prefix: &'a [u8],
    pub string_table: Section<'a>,
    pub payload: Section<'a>,
}

pub(crate) struct Budget {
    records: usize,
    pub(super) work: usize,
}
fn charge(left: &mut usize, n: usize, offset: usize, label: &'static str) -> Result<(), Error> {
    *left = left.checked_sub(n).ok_or(Error {
        offset,
        kind: ErrorKind::Limit(label),
    })?;
    Ok(())
}
impl Budget {
    pub(super) fn new(records: usize, work: usize) -> Self {
        Self {
            records: records.min(MAX_RECORDS),
            work: work.min(MAX_VALIDATION_UNITS),
        }
    }

    pub(crate) fn records(&mut self, count: usize) -> Result<(), Error> {
        charge(&mut self.records, count, 0, "records")?;
        self.work(count, 0)
    }
    pub(super) fn record(&mut self, offset: usize) -> Result<(), Error> {
        charge(&mut self.records, 1, offset, "records")?;
        self.work(1, offset)
    }
    pub(crate) fn work(&mut self, n: usize, offset: usize) -> Result<(), Error> {
        charge(&mut self.work, n, offset, "work")
    }
}

struct Reader<'a, 'b> {
    bytes: &'a [u8],
    pos: usize,
    end: usize,
    budget: &'b mut Budget,
}
impl<'a> Reader<'a, '_> {
    fn error(&self, kind: ErrorKind, offset: usize) -> Error {
        Error { offset, kind }
    }
    fn short(&mut self) -> Result<(), Error> {
        let work = (self.end - self.pos)
            .checked_add(1)
            .ok_or_else(|| self.error(ErrorKind::Limit("work"), self.pos))?;
        self.budget.work(work, self.pos)?;
        Err(self.error(ErrorKind::Truncated, self.end))
    }
    fn boundary(&mut self, n: usize) -> Result<usize, Error> {
        if n > self.end - self.pos {
            self.short()?;
        }
        // Both positions are bounded by the checked input length.
        Ok(self.pos + n)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.boundary(n)?;
        self.budget.work(n, self.pos)?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    fn varint(&mut self) -> Result<u64, Error> {
        let mut value = 0;
        for shift in (0..64).step_by(7) {
            let byte = self.take(1)?[0];
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(self.error(ErrorKind::MalformedVarint, self.pos - 1))
    }
    fn length(&mut self) -> Result<usize, Error> {
        let start = self.pos;
        let n = self.varint()? as u32;
        if n > i32::MAX as u32 {
            return Err(self.error(ErrorKind::NegativeLength, start));
        }
        Ok(n as usize)
    }
    fn field(&mut self) -> Result<Field<'a>, Error> {
        let field_start = self.pos;
        self.budget.record(field_start)?;
        let tag = self.varint()? as u32;
        let number = tag >> 3;
        if number == 0 {
            return Err(self.error(ErrorKind::InvalidFieldNumber, field_start));
        }
        let value = match tag & 7 {
            0 => Value::Varint(self.varint()?),
            1 => Value::Fixed64(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            2 => {
                let n = self.length()?;
                Value::Bytes(self.take(n)?)
            }
            3 | 4 => return Err(self.error(ErrorKind::UnsupportedGroup, field_start)),
            5 => Value::Fixed32(u32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            _ => return Err(self.error(ErrorKind::InvalidWireType, field_start)),
        };
        Ok(Field {
            number,
            span: field_start..self.pos,
            raw: &self.bytes[field_start..self.pos],
            value,
        })
    }
    fn section(&mut self, end: usize) -> Result<Section<'a>, Error> {
        let start = self.pos;
        self.end = end;
        let mut fields = Vec::new();
        while self.pos < end {
            fields.push(self.field()?);
        }
        Ok(Section {
            span: start..end,
            raw: &self.bytes[start..end],
            fields,
        })
    }
}

// Schema callers share one budget while revisiting bounded nested/packed spans.
fn cursor<'a, 'b>(
    bytes: &'a [u8],
    pos: usize,
    end: usize,
    budget: &'b mut Budget,
) -> Result<Reader<'a, 'b>, Error> {
    if pos > end || end > bytes.len() {
        return Err(Error {
            offset: bytes.len(),
            kind: ErrorKind::Truncated,
        });
    }
    Ok(Reader {
        bytes,
        pos,
        end,
        budget,
    })
}

pub(super) fn field<'a>(
    bytes: &'a [u8],
    pos: &mut usize,
    end: usize,
    budget: &mut Budget,
) -> Result<Field<'a>, Error> {
    let mut reader = cursor(bytes, *pos, end, budget)?;
    let out = reader.field()?;
    *pos = reader.pos;
    Ok(out)
}

pub(super) fn varint(
    bytes: &[u8],
    pos: &mut usize,
    end: usize,
    budget: &mut Budget,
) -> Result<u64, Error> {
    let mut reader = cursor(bytes, *pos, end, budget)?;
    let out = reader.varint()?;
    *pos = reader.pos;
    Ok(out)
}

pub(super) struct Envelope {
    pub(super) prefix_span: Range<usize>,
    pub(super) string_table_span: Range<usize>,
    pub(super) payload_span: Range<usize>,
}

pub(super) fn envelope(bytes: &[u8], budget: &mut Budget) -> Result<Envelope, Error> {
    let mut reader = Reader {
        bytes,
        pos: 0,
        end: bytes.len(),
        budget,
    };
    reader.budget.record(0)?;
    let length = reader.length()?;
    let prefix_span = 0..reader.pos;
    let string_end = reader.boundary(length)?;
    Ok(Envelope {
        prefix_span,
        string_table_span: reader.pos..string_end,
        payload_span: string_end..bytes.len(),
    })
}

/// Read a delimited string-table section and the remaining class/package section.
/// Length-delimited fields stay opaque; unknown fields and duplicates retain order.
/// Nonminimal varints are accepted: tags/lengths retain low32, values low64 bits.
/// shortcut: raw64 follows the pinned slow path, revisit if buffer-dependent JVM compatibility is needed.
/// Negative/short envelopes are rejected more strictly than pinned LimitedInputStream.
/// The bytes-only API cannot validate header k/mv or required schema fields.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Sections<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut budget = Budget::new(limits.records, limits.work);
    let envelope = envelope(bytes, &mut budget)?;
    let mut reader = Reader {
        bytes,
        pos: envelope.string_table_span.start,
        end: bytes.len(),
        budget: &mut budget,
    };
    let prefix_span = envelope.prefix_span;
    let prefix = &bytes[prefix_span.clone()];
    let string_table = reader.section(envelope.string_table_span.end)?;
    let payload = reader.section(bytes.len())?;
    Ok(Sections {
        prefix_span,
        prefix,
        string_table,
        payload,
    })
}
