//! Raw VersionRequirement scalars and ordered tables, without version policy or names.
use super::{
    string_table,
    wire::{self, Value},
};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::ops::Range;

pub use super::wire::{Error, ErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Warning,
    Error,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionKind {
    LanguageVersion,
    CompilerVersion,
    ApiVersion,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VersionRequirement<'a> {
    /// Complete body, preserving unknown/wrong-wire fields, duplicates and order.
    pub raw: &'a [u8],
    /// Body offsets relative to the supplied requirement or table bytes.
    pub span: Range<usize>,
    /// Absent integers have schema default zero; both versions stay independent.
    pub version: Option<i32>,
    pub version_full: Option<i32>,
    /// Absent has schema default Error; unknown enums do not replace known values.
    pub level: Option<Level>,
    pub error_code: Option<i32>,
    pub message: Option<i32>,
    /// Absent has schema default LanguageVersion; no availability check is made.
    pub version_kind: Option<VersionKind>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VersionRequirementTable<'a> {
    /// Complete table, preserving outer unknown fields, duplicates and order.
    pub raw: &'a [u8],
    pub requirements: Vec<VersionRequirement<'a>>,
}

/// Table callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Requested retained logical vector capacity, excluding allocator surplus/RSS.
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

fn limit(label: &'static str) -> Error {
    Error {
        offset: 0,
        kind: ErrorKind::Limit(label),
    }
}

fn decode_span<'a>(
    bytes: &'a [u8],
    span: Range<usize>,
    budget: &mut wire::Budget,
) -> Result<VersionRequirement<'a>, Error> {
    let mut out = VersionRequirement {
        raw: &bytes[span.clone()],
        span: span.clone(),
        version: None,
        version_full: None,
        level: None,
        error_code: None,
        message: None,
        version_kind: None,
    };
    let mut pos = span.start;
    while pos < span.end {
        let field = wire::field(bytes, &mut pos, span.end, budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => out.version = Some(value as u32 as i32),
            (2, Value::Varint(value)) => out.version_full = Some(value as u32 as i32),
            (3, Value::Varint(value)) => {
                if let Some(level) = match value as u32 as i32 {
                    0 => Some(Level::Warning),
                    1 => Some(Level::Error),
                    2 => Some(Level::Hidden),
                    _ => None,
                } {
                    out.level = Some(level);
                }
            }
            (4, Value::Varint(value)) => out.error_code = Some(value as u32 as i32),
            (5, Value::Varint(value)) => out.message = Some(value as u32 as i32),
            (6, Value::Varint(value)) => {
                if let Some(kind) = match value as u32 as i32 {
                    0 => Some(VersionKind::LanguageVersion),
                    1 => Some(VersionKind::CompilerVersion),
                    2 => Some(VersionKind::ApiVersion),
                    _ => None,
                } {
                    out.version_kind = Some(kind);
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Decode ONE caller-selected unframed requirement with a separate allocation-free budget.
/// Caller selects supported metadata version [2,3,0] and the message body independently.
/// All scalars are optional; recognized values last-win and wrong wires stay raw.
/// Errors/spans are input-relative. Success establishes no version policy or name resolution.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
pub fn decode(bytes: &[u8], limits: wire::Limits) -> Result<VersionRequirement<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(limit("bytes"));
    }
    decode_span(
        bytes,
        0..bytes.len(),
        &mut wire::Budget::new(limits.records, limits.work),
    )
}

/// Decode ONE unframed table with one aggregate wire/storage budget across all children.
/// Caller independently selects supported version [2,3,0] and the table body.
/// Ordered empty/duplicate entries stay distinct; errors and child spans are table-relative.
/// Unknown/wrong-wire outer fields stay raw. No header selection, version-policy diagnostics,
/// message/name resolution, symbol reconstruction or semantic availability is provided.
pub fn decode_table(bytes: &[u8], limits: Limits) -> Result<VersionRequirementTable<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(limit("bytes"));
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    let mut requirements = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (1, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            let requirement = decode_span(bytes, span, &mut budget.wire)?;
            budget.push(&mut requirements, requirement, start)?;
        }
    }
    Ok(VersionRequirementTable {
        raw: bytes,
        requirements,
    })
}
