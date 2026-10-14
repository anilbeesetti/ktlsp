//! Bounded, data-only StringTableTypes/Record decoding from an unframed message.
//! Rules derive from pinned Kotlin sources; see METADATA_STRING_TABLE_SPEC.md.
use super::wire::{self, Error, ErrorKind, Field, Value};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::{mem::size_of, ops::Range};

/// Callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub nodes: usize,
    pub depth: usize,
    pub work: usize,
    /// Requested retained vector capacity; excludes reallocation churn and RSS.
    pub owned_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            records: MAX_RECORDS,
            nodes: MAX_RECORDS,
            depth: 2,
            work: MAX_VALIDATION_UNITS,
            owned_bytes: MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    None,
    InternalToClassId,
    DescToClassId,
}

/// Optional presence is retained. Absent defaults: range1/index0/None/empty string.
/// Lists and range remain data; no expansion, validation or transformations.
#[derive(Debug, PartialEq, Eq)]
pub struct Record<'a> {
    pub raw: &'a [u8],
    /// Record body offsets relative to the supplied table message.
    pub span: Range<usize>,
    pub range: Option<i32>,
    pub predefined_index: Option<i32>,
    pub string: Option<&'a [u8]>,
    pub operation: Option<Operation>,
    pub substring_index: Vec<i32>,
    pub replace_char: Vec<i32>,
    pub unknown: Vec<Field<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StringTableTypes<'a> {
    /// Complete original message, preserving all fields, duplicates and order.
    pub raw: &'a [u8],
    pub records: Vec<Record<'a>>,
    pub local_name: Vec<i32>,
    pub unknown: Vec<Field<'a>>,
}

fn limit(label: &'static str, offset: usize) -> Error {
    Error {
        offset,
        kind: ErrorKind::Limit(label),
    }
}
pub(super) struct Budget {
    pub(super) wire: wire::Budget,
    nodes: usize,
    depth: usize,
    owned: usize,
}
impl Budget {
    pub(super) fn new(limits: Limits) -> Self {
        Self {
            wire: wire::Budget::new(limits.records, limits.work),
            nodes: limits.nodes.min(MAX_RECORDS),
            depth: limits.depth.min(2),
            owned: limits.owned_bytes.min(MAX_BYTES),
        }
    }

    fn node(&mut self, depth: usize, offset: usize) -> Result<(), Error> {
        if depth > self.depth {
            return Err(limit("depth", offset));
        }
        self.nodes = self.nodes.checked_sub(1).ok_or(limit("nodes", offset))?;
        self.wire.work(1, offset)
    }
    pub(super) fn push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        offset: usize,
    ) -> Result<(), Error> {
        let len = values.len();
        // Logical reservations stay deterministic even if the allocator grants surplus.
        if len == 0 || len.is_power_of_two() {
            let slots = len.max(1);
            len.checked_add(slots).ok_or(limit("owned_bytes", offset))?;
            let bytes = slots
                .checked_mul(size_of::<T>())
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
    fn integer(&mut self, values: &mut Vec<i32>, value: u64, offset: usize) -> Result<(), Error> {
        self.wire.work(1, offset)?;
        self.push(values, value as u32 as i32, offset)
    }
    pub(super) fn integers(
        &mut self,
        bytes: &[u8],
        field: &Field<'_>,
        values: &mut Vec<i32>,
    ) -> Result<(), Error> {
        match field.value {
            Value::Varint(value) => self.integer(values, value, field.span.start),
            Value::Bytes(raw) => {
                let mut pos = field.span.end - raw.len();
                while pos < field.span.end {
                    let start = pos;
                    self.wire.record(start)?;
                    let value = wire::varint(bytes, &mut pos, field.span.end, &mut self.wire)?;
                    self.integer(values, value, start)?;
                }
                Ok(())
            }
            _ => unreachable!("only matched integer wire types reach this helper"),
        }
    }
}

fn record<'a>(
    bytes: &'a [u8],
    span: Range<usize>,
    budget: &mut Budget,
) -> Result<Record<'a>, Error> {
    budget.node(2, span.start)?;
    let mut out = Record {
        raw: &bytes[span.clone()],
        span: span.clone(),
        range: None,
        predefined_index: None,
        string: None,
        operation: None,
        substring_index: Vec::new(),
        replace_char: Vec::new(),
        unknown: Vec::new(),
    };
    let mut pos = span.start;
    while pos < span.end {
        let field = wire::field(bytes, &mut pos, span.end, &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => out.range = Some(value as u32 as i32),
            (2, Value::Varint(value)) => out.predefined_index = Some(value as u32 as i32),
            (3, Value::Varint(value)) => {
                let op = match value as u32 as i32 {
                    0 => Some(Operation::None),
                    1 => Some(Operation::InternalToClassId),
                    2 => Some(Operation::DescToClassId),
                    _ => None,
                };
                if let Some(op) = op {
                    out.operation = Some(op);
                } else {
                    budget.push(&mut out.unknown, field, start)?;
                }
            }
            (4, Value::Varint(_) | Value::Bytes(_)) => {
                budget.integers(bytes, &field, &mut out.substring_index)?;
            }
            (5, Value::Varint(_) | Value::Bytes(_)) => {
                budget.integers(bytes, &field, &mut out.replace_char)?;
            }
            (6, Value::Bytes(raw)) => out.string = Some(raw),
            _ => budget.push(&mut out.unknown, field, start)?,
        }
    }
    Ok(out)
}

/// Validate ONE unframed StringTableTypes message; offsets are relative to `bytes`.
/// Caller-constructible wire fields/spans are never trusted. Header/framing and
/// class/package parsing are separate operations with their own budgets.
/// Unknown and wrong-wire fields stay opaque; groups remain unsupported.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<StringTableTypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(limit("bytes", 0));
    }
    decode_with_budget(bytes, &mut Budget::new(limits))
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    budget: &mut Budget,
) -> Result<StringTableTypes<'a>, Error> {
    budget.node(1, 0)?;
    let mut out = StringTableTypes {
        raw: bytes,
        records: Vec::new(),
        local_name: Vec::new(),
        unknown: Vec::new(),
    };
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        match (field.number, field.value) {
            (1, Value::Bytes(raw)) => {
                let span = field.span.end - raw.len()..field.span.end;
                let value = record(bytes, span.clone(), budget)?;
                budget.push(&mut out.records, value, span.start)?;
            }
            (5, Value::Varint(_) | Value::Bytes(_)) => {
                budget.integers(bytes, &field, &mut out.local_name)?;
            }
            _ => budget.push(&mut out.unknown, field, start)?,
        }
    }
    Ok(out)
}
