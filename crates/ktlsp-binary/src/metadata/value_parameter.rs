//! Selected ValueParameter scalar data, without nested-message or semantic validation.
use super::wire::{self, Value};
use crate::MAX_BYTES;

pub use super::wire::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameter<'a> {
    /// Complete supplied ValueParameter bytes, preserving unknown fields and duplicates.
    pub raw: &'a [u8],
    /// Absent has schema default 0; explicit zero remains present.
    pub flags: Option<i32>,
    /// Required signed index data; not checked against any name table.
    pub name: i32,
    /// Optional signed indices; no lookup or inline-type precedence is applied.
    pub type_id: Option<i32>,
    pub vararg_element_type_id: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingName,
}

/// Offsets are relative to the supplied unframed ValueParameter bytes.
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

/// Scan ONE unframed ValueParameter body using a separately bounded operation.
/// Caller must independently select supported version [2,3,0] Class/Package data
/// and the ValueParameter body; this bytes-only API cannot enforce message kind/version.
/// Wrong-wire/unselected fields remain raw; selected scalars last-win independently.
/// Nested fields 3/4/7/8 retain every occurrence solely in raw. Singular messages
/// 3/4/8 normally merge; this API exposes no merged or last-wins nested projection.
/// Success does not establish full ValueParameter initialization, names or semantics.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ValueParameter<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = wire::Budget::new(limits.records, limits.work);
    decode_with_budget(bytes, &mut budget)
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    budget: &mut wire::Budget,
) -> Result<ValueParameter<'a>, Error> {
    let mut pos = 0;
    let mut flags = None;
    let mut type_id = None;
    let mut vararg_element_type_id = None;
    let mut name = None;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => flags = Some(value as u32 as i32),
            (2, Value::Varint(value)) => name = Some(value as u32 as i32),
            (5, Value::Varint(value)) => type_id = Some(value as u32 as i32),
            (6, Value::Varint(value)) => vararg_element_type_id = Some(value as u32 as i32),
            // shortcut: nested payload validity stays opaque until nested metadata decoding.
            _ => {}
        }
    }
    let name = name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingName,
    })?;
    Ok(ValueParameter {
        raw: bytes,
        flags,
        name,
        type_id,
        vararg_element_type_id,
    })
}
