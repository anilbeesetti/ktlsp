//! Selected TypeAlias data; inline Type occurrences remain opaque and unmerged.
use super::wire::{self, Value};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::mem::size_of;

/// Callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Logical requested vector capacity across both lists, excluding churn/RSS.
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
pub struct TypeAlias<'a> {
    /// Complete input, preserving unknown fields, duplicates and cross-field order.
    pub raw: &'a [u8],
    /// Absent has schema default 6; distinct from explicit Some(6).
    pub flags: Option<i32>,
    /// Required raw signed index, without name-table validation.
    pub name: i32,
    /// Absent has schema default 0; no precedence over inline occurrences.
    pub underlying_type_id: Option<i32>,
    /// Absent has schema default 0; no precedence over inline occurrences.
    pub expanded_type_id: Option<i32>,
    /// All field-4 bodies in encounter order, including empty/duplicate bodies.
    pub underlying_types: Vec<&'a [u8]>,
    /// All field-6 bodies in encounter order, including empty/duplicate bodies.
    pub expanded_types: Vec<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    MissingName,
}

/// Offsets are relative to the supplied unframed TypeAlias body.
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
fn limit(label: &'static str, offset: usize) -> Error {
    wire::Error {
        offset,
        kind: wire::ErrorKind::Limit(label),
    }
    .into()
}

struct Budget {
    wire: wire::Budget,
    owned: usize,
}
impl Budget {
    fn push<'a>(
        &mut self,
        values: &mut Vec<&'a [u8]>,
        value: &'a [u8],
        offset: usize,
    ) -> Result<(), Error> {
        let len = values.len();
        // Logical reservations stay deterministic even if the allocator grants surplus.
        if len == 0 || len.is_power_of_two() {
            let slots = len.max(1);
            len.checked_add(slots).ok_or(limit("owned_bytes", offset))?;
            let bytes = slots
                .checked_mul(size_of::<&[u8]>())
                .ok_or(limit("owned_bytes", offset))?;
            self.owned = self
                .owned
                .checked_sub(bytes)
                .ok_or(limit("owned_bytes", offset))?;
            self.wire.work(bytes, offset)?;
            values
                .try_reserve_exact(slots)
                .map_err(|_| limit("allocation", offset))?;
        }
        values.push(value);
        Ok(())
    }
}

/// Scan ONE unframed TypeAlias body under one aggregate budget.
/// Caller independently selects supported mv=[2,3,0] Class/Package TypeAlias data;
/// this bytes-only API cannot enforce kind/version or full message initialization.
/// Wrong-wire/unselected fields stay raw; recognized scalars last-win.
/// shortcut: singular inline Types retain every occurrence, implement pinned merging when Type interpretation is added.
/// No nested decoding, alias expansion, name/type lookup or semantic validation.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeAlias<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(limit("bytes", 0));
    }
    let mut budget = Budget {
        wire: wire::Budget::new(limits.records, limits.work),
        owned: limits.owned_bytes.min(MAX_BYTES),
    };
    let mut pos = 0;
    let mut flags = None;
    let mut name = None;
    let mut underlying_type_id = None;
    let mut expanded_type_id = None;
    let mut underlying_types = Vec::new();
    let mut expanded_types = Vec::new();
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => flags = Some(value as u32 as i32),
            (2, Value::Varint(value)) => name = Some(value as u32 as i32),
            (4, Value::Bytes(raw)) => budget.push(&mut underlying_types, raw, start)?,
            (5, Value::Varint(value)) => underlying_type_id = Some(value as u32 as i32),
            (6, Value::Bytes(raw)) => budget.push(&mut expanded_types, raw, start)?,
            (7, Value::Varint(value)) => expanded_type_id = Some(value as u32 as i32),
            _ => {}
        }
    }
    let name = name.ok_or(Error {
        offset: bytes.len(),
        kind: ErrorKind::MissingName,
    })?;
    Ok(TypeAlias {
        raw: bytes,
        flags,
        name,
        underlying_type_id,
        expanded_type_id,
        underlying_types,
        expanded_types,
    })
}
