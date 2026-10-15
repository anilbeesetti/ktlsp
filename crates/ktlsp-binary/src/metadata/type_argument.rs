//! Selected Type.Argument data; all inline Type merge inputs remain opaque.
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
    /// Requested retained logical vector capacity; excludes allocator surplus/RSS.
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
pub enum Projection {
    In,
    Out,
    Inv,
    Star,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InlineType<'a> {
    /// Opaque body, without its enclosing tag/length.
    pub raw: &'a [u8],
    /// Body offsets relative to the supplied Argument bytes.
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeArgument<'a> {
    /// Complete input, preserving unknown/wrong-wire fields, duplicates and order.
    pub raw: &'a [u8],
    /// Absent has schema default Inv; unknown enums never erase recognized values.
    pub projection: Option<Projection>,
    /// Every occurrence in order; upstream singular-message merging is not applied.
    pub inline_types: Vec<InlineType<'a>>,
    /// Absent has schema default zero; negative/out-of-range IDs remain raw data.
    pub type_id: Option<i32>,
}

/// Scan ONE caller-selected unframed Argument with a separate operation budget.
/// Caller independently selects supported metadata version [2,3,0] and body.
/// Recognized scalars last-win; inline Type bodies stay opaque and are not merged
/// or initialized. STAR/type comments impose no initialization rule here.
/// Errors are relative to this input. Success establishes no type resolution,
/// projection semantics, declaration reconstruction or metadata parity.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeArgument<'_>, Error> {
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
) -> Result<TypeArgument<'a>, Error> {
    let mut out = TypeArgument {
        raw: bytes,
        projection: None,
        inline_types: Vec::new(),
        type_id: None,
    };
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => {
                let known = match value as u32 as i32 {
                    0 => Some(Projection::In),
                    1 => Some(Projection::Out),
                    2 => Some(Projection::Inv),
                    3 => Some(Projection::Star),
                    _ => None,
                };
                if let Some(known) = known {
                    out.projection = Some(known);
                }
            }
            (2, Value::Bytes(raw)) => {
                let span = field.span.end - raw.len()..field.span.end;
                budget.push(&mut out.inline_types, InlineType { raw, span }, start)?;
            }
            (3, Value::Varint(value)) => out.type_id = Some(value as u32 as i32),
            _ => {}
        }
    }
    Ok(out)
}
