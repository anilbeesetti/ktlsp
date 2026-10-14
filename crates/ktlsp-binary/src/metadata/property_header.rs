//! Selected Property name/raw flags, without nested-message or semantic validation.
use super::wire::{self, Value};
use crate::MAX_BYTES;

pub use super::wire::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct PropertyHeader<'a> {
    /// Complete supplied Property bytes, preserving unknown fields and duplicates.
    pub raw: &'a [u8],
    /// Absent has schema default 518; never converted from old_flags.
    pub flags: Option<i32>,
    /// Independent presence/raw value, with schema default 2054 when absent.
    pub old_flags: Option<i32>,
    /// Absent has protobuf default 0; semantic inheritance is not computed.
    pub getter_flags: Option<i32>,
    /// Absent has protobuf default 0; semantic inheritance is not computed.
    pub setter_flags: Option<i32>,
    /// Required signed index data; not checked against any name table.
    pub name: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingName,
}

/// Offsets are relative to the supplied unframed Property bytes.
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

/// Scan ONE unframed Property body using a separately bounded operation.
/// Caller independently selects supported version [2,3,0] Class/Package data
/// and its Property body; this bytes-only API cannot enforce message kind/version.
/// Wrong-wire/unselected fields remain raw; singular recognized fields last-win.
/// Success does not establish full Property initialization, names or semantics.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<PropertyHeader<'_>, Error> {
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
) -> Result<PropertyHeader<'a>, Error> {
    let mut pos = 0;
    let mut flags = None;
    let mut old_flags = None;
    let mut getter_flags = None;
    let mut setter_flags = None;
    let mut name = None;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => old_flags = Some(value as u32 as i32),
            (2, Value::Varint(value)) => name = Some(value as u32 as i32),
            (7, Value::Varint(value)) => getter_flags = Some(value as u32 as i32),
            (8, Value::Varint(value)) => setter_flags = Some(value as u32 as i32),
            (11, Value::Varint(value)) => flags = Some(value as u32 as i32),
            _ => {}
        }
    }
    let name = name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingName,
    })?;
    Ok(PropertyHeader {
        raw: bytes,
        flags,
        old_flags,
        getter_flags,
        setter_flags,
        name,
    })
}
