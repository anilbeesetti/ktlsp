//! Selected Function name/flags data, without nested-message or semantic validation.
use super::wire::{self, Value};
use crate::MAX_BYTES;

pub use super::wire::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionHeader<'a> {
    /// Complete supplied Function bytes, preserving unknown fields and duplicates.
    pub raw: &'a [u8],
    /// Absent has schema default 6; never supplied or converted from old_flags.
    pub flags: Option<i32>,
    /// Independent presence/raw value, with schema default 6 when absent.
    pub old_flags: Option<i32>,
    /// Required signed index data; not checked against any name table.
    pub name: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingName,
}

/// Offsets are relative to the supplied unframed Function bytes.
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

/// Scan ONE unframed Function body using a separately bounded operation.
/// Caller must independently select supported version [2,3,0] Class/Package data
/// and the Function body; this bytes-only API cannot enforce message kind/version.
/// Wrong-wire/unselected fields remain raw; singular recognized fields last-win.
/// Success does not establish full Function initialization, names or semantics.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionHeader<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = wire::Budget::new(limits.records, limits.work);
    let mut pos = 0;
    let mut flags = None;
    let mut old_flags = None;
    let mut name = None;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => old_flags = Some(value as u32 as i32),
            (2, Value::Varint(value)) => name = Some(value as u32 as i32),
            (9, Value::Varint(value)) => flags = Some(value as u32 as i32),
            _ => {}
        }
    }
    let name = name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingName,
    })?;
    Ok(FunctionHeader {
        raw: bytes,
        flags,
        old_flags,
        name,
    })
}
