//! Selected Constructor data; ValueParameter bodies remain opaque.
use super::{
    string_table,
    wire::{self, Value},
};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::ops::Range;

pub use super::wire::{Error, ErrorKind};

/// Callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Logical requested vector capacity, excluding allocator surplus/churn/RSS.
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

#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameterBody<'a> {
    pub raw: &'a [u8],
    /// Body-only offsets relative to the supplied Constructor bytes.
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Constructor<'a> {
    /// Complete input, preserving unknown/wrong-wire fields, duplicates and order.
    pub raw: &'a [u8],
    /// Absent has schema default 6; distinct from explicit Some(6).
    pub flags: Option<i32>,
    pub value_parameters: Vec<ValueParameterBody<'a>>,
    /// Raw signed IDs in packed/unpacked encounter order, without index lookup.
    pub version_requirements: Vec<i32>,
}

/// Scan ONE caller-selected unframed Constructor under one aggregate budget.
/// Caller separately selects supported mv=[2,3,0] Class field8/wire2 data;
/// this bytes-only API cannot enforce header/version/kind or nested initialization.
/// Parameters remain opaque, including empty/duplicate/malformed bodies.
/// Wrong-wire/unselected fields stay raw; recognized flags last-win.
/// No constructor symbols, overloads, types, names or semantic validation.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Constructor<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    decode_with_budget(bytes, &mut budget)
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    budget: &mut string_table::Budget,
) -> Result<Constructor<'a>, Error> {
    let mut out = Constructor {
        raw: bytes,
        flags: None,
        value_parameters: Vec::new(),
        version_requirements: Vec::new(),
    };
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => out.flags = Some(value as u32 as i32),
            (2, Value::Bytes(raw)) => {
                let value = ValueParameterBody {
                    raw,
                    span: field.span.end - raw.len()..field.span.end,
                };
                budget.push(&mut out.value_parameters, value, start)?;
            }
            (31, Value::Varint(_) | Value::Bytes(_)) => {
                budget.integers(bytes, &field, &mut out.version_requirements)?;
            }
            _ => {}
        }
    }
    Ok(out)
}
