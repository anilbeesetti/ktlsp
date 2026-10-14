//! Selected Class fq_name/flags data, without nested-message or semantic validation.
use super::wire::{self, Value};
use crate::MAX_BYTES;

pub use super::wire::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct ClassHeader<'a> {
    /// Complete supplied Class bytes, preserving unknown fields and duplicates.
    pub raw: &'a [u8],
    /// Absent has effective schema default 6; presence remains distinct.
    pub flags: Option<i32>,
    /// Required signed index data; not checked against any name table.
    pub fq_name: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingFqName,
}

/// Offsets are relative to the supplied unframed Class bytes.
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
impl From<wire::Error> for Error {
    fn from(error: wire::Error) -> Self {
        Self {
            offset: error.offset,
            kind: ErrorKind::Wire(error.kind),
        }
    }
}

/// Scan ONE unframed Class message using a separately bounded operation.
/// Caller must select supported header kind 1/version [2,3,0] before passing
/// payload::Payload.payload. This bytes-only API cannot distinguish Class/Package.
/// Wrong-wire/unselected fields remain raw; singular recognized fields last-win.
/// Success does not establish full Class initialization, names or semantics.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassHeader<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    decode_with_budget(bytes, &mut wire::Budget::new(limits.records, limits.work))
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    budget: &mut wire::Budget,
) -> Result<ClassHeader<'a>, Error> {
    let mut pos = 0;
    let mut flags = None;
    let mut fq_name = None;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => flags = Some(value as u32 as i32),
            (3, Value::Varint(value)) => fq_name = Some(value as u32 as i32),
            _ => {}
        }
    }
    let fq_name = fq_name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingFqName,
    })?;
    Ok(ClassHeader {
        raw: bytes,
        flags,
        fq_name,
    })
}
