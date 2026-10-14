//! Selected TypeParameter data; nested Type/Annotation initialization remains opaque.
use super::{
    string_table,
    wire::{self, Value},
};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::ops::Range;

/// Callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Requested retained vector capacity, excluding allocator surplus/churn and RSS.
    pub owned_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            records: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
            owned_bytes: MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variance {
    In,
    Out,
    Inv,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UpperBound<'a> {
    /// Opaque message body, without its enclosing tag/length.
    pub raw: &'a [u8],
    /// Body offsets relative to the supplied TypeParameter bytes.
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeParameter<'a> {
    /// Complete input, preserving unknown/wrong-wire fields, duplicates and order.
    pub raw: &'a [u8],
    /// Required signed data, without index or semantic validation.
    pub id: i32,
    pub name: i32,
    /// Absent has schema default false; presence is retained.
    pub reified: Option<bool>,
    /// Absent has schema default Inv; unknown enums do not replace known values.
    pub variance: Option<Variance>,
    pub upper_bound: Vec<UpperBound<'a>>,
    pub upper_bound_id: Vec<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingId,
    MissingName,
}

/// Offsets are relative to the supplied unframed TypeParameter bytes.
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

/// Decode ONE unframed TypeParameter using a separately bounded operation.
/// Caller independently selects supported version [2,3,0] and the message body.
/// Singular recognized scalars last-win; bound IDs append across packed/unpacked
/// fields. Unknown/wrong-wire fields stay raw. Success does not establish nested
/// initialization, name/type resolution, generic semantics or metadata parity.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeParameter<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    let mut pos = 0;
    let mut id = None;
    let mut name = None;
    let mut reified = None;
    let mut variance = None;
    let mut upper_bound = Vec::new();
    let mut upper_bound_id = Vec::new();
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => id = Some(value as u32 as i32),
            (2, Value::Varint(value)) => name = Some(value as u32 as i32),
            (3, Value::Varint(value)) => reified = Some(value != 0),
            (4, Value::Varint(value)) => {
                let known = match value as u32 as i32 {
                    0 => Some(Variance::In),
                    1 => Some(Variance::Out),
                    2 => Some(Variance::Inv),
                    _ => None,
                };
                if let Some(known) = known {
                    variance = Some(known);
                }
            }
            (5, Value::Bytes(raw)) => {
                let span = field.span.end - raw.len()..field.span.end;
                budget.push(&mut upper_bound, UpperBound { raw, span }, start)?;
            }
            (6, Value::Varint(_) | Value::Bytes(_)) => {
                budget.integers(bytes, &field, &mut upper_bound_id)?;
            }
            _ => {}
        }
    }
    let id = id.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingId,
    })?;
    let name = name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingName,
    })?;
    Ok(TypeParameter {
        raw: bytes,
        id,
        name,
        reified,
        variance,
        upper_bound,
        upper_bound_id,
    })
}
