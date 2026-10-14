//! Ordered opaque Type bodies and raw first_nullable data; no type interpretation.
use super::wire::{self, Value};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::mem::size_of;

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

#[derive(Debug, PartialEq, Eq)]
pub struct TypeTable<'a> {
    /// Complete supplied message, preserving unknown fields and duplicates.
    pub raw: &'a [u8],
    /// Ordered borrowed bodies, including empty entries; contents stay opaque.
    pub types: Vec<&'a [u8]>,
    /// Absent has schema default -1; negative/out-of-range values remain raw.
    pub first_nullable: Option<i32>,
}

fn limit(label: &'static str, offset: usize) -> Error {
    Error {
        offset,
        kind: ErrorKind::Limit(label),
    }
}

/// Scan ONE caller-selected unframed TypeTable body with a separate operation budget.
/// Caller independently selects supported metadata version [2,3,0] and table body;
/// this bytes-only API cannot enforce message kind/version or locate a container.
/// Wrong-wire/unselected fields remain raw; recognized scalars last-win.
/// Success does not establish nested initialization, linked types or nullability.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeTable<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(limit("bytes", 0));
    }
    let mut budget = wire::Budget::new(limits.records, limits.work);
    let mut owned = limits.owned_bytes.min(MAX_BYTES);
    let mut pos = 0;
    let mut types = Vec::new();
    let mut first_nullable = None;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget)?;
        let start = field.span.start;
        budget.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Bytes(body)) => {
                let len = types.len();
                // Logical reservations stay deterministic even if the allocator grants surplus.
                if len == 0 || len.is_power_of_two() {
                    let slots = len.max(1);
                    len.checked_add(slots).ok_or(limit("owned_bytes", start))?;
                    let size = slots
                        .checked_mul(size_of::<&[u8]>())
                        .ok_or(limit("owned_bytes", start))?;
                    owned = owned.checked_sub(size).ok_or(limit("owned_bytes", start))?;
                    budget.work(size, start)?;
                    types
                        .try_reserve_exact(slots)
                        .map_err(|_| limit("allocation", start))?;
                }
                types.push(body);
            }
            (2, Value::Varint(value)) => first_nullable = Some(value as u32 as i32),
            _ => {}
        }
    }
    Ok(TypeTable {
        raw: bytes,
        types,
        first_nullable,
    })
}
