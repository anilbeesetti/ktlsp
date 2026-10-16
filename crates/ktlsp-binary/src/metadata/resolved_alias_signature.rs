//! Finite, resolved nominal Package TypeAlias signatures; not alias expansion or linkage.
use super::{Outcome, name_resolver, payload, string_table, wire};
use crate::{MAX_BYTES, MAX_RECORDS};
use std::ops::Range;

pub use super::package_member_names::Limits;

/// Lossless Kotlin ClassId parts, sharing the resolver's one UTF-16 allocation.
#[derive(Debug, PartialEq, Eq)]
pub struct ClassId {
    text: Vec<u16>,
    package_end: usize,
    relative_start: usize,
    pub local: bool,
}
impl ClassId {
    pub fn package_name(&self) -> &[u16] { &self.text[..self.package_end] }
    pub fn relative_class_name(&self) -> &[u16] { &self.text[self.relative_start..] }
}

#[derive(Debug, PartialEq, Eq)]
pub struct NominalType {
    pub class_id: ClassId,
    /// Effective nullability; absent encodings mean false.
    pub nullable: bool,
    pub nullable_presence: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Signature {
    /// Encoded position among all Package fields, not a semantic symbol ID.
    pub ordinal: usize,
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub name: Vec<u16>,
    pub underlying: NominalType,
    pub expanded: NominalType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnsupportedContainer,
    Wire(wire::ErrorKind),
    Name(name_resolver::Error),
    MissingName,
    MissingRole,
    UnsupportedTableRole,
    DuplicateInlineRole,
    UnsupportedTypeParameters,
    MissingClass,
    UnsupportedType,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error { pub offset: usize, pub kind: ErrorKind }
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at byte {}", self.kind, self.offset)
    }
}
impl std::error::Error for Error {}
impl From<wire::Error> for Error {
    fn from(e: wire::Error) -> Self { Self { offset: e.offset, kind: ErrorKind::Wire(e.kind) } }
}
fn failure(offset: usize, kind: ErrorKind) -> Error { Error { offset, kind } }
fn name_error(offset: usize, e: name_resolver::Error) -> Error { failure(offset, ErrorKind::Name(e)) }

fn nominal(
    bytes: &[u8], start: usize, resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<NominalType, Error> {
    // Flat leaf validation, not recursive wrapper traversal: no nested Type is admitted.
    budget.node(2, start)?;
    let mut pos = 0;
    let mut class = None;
    let mut nullable = None;
    while pos < bytes.len() {
        let f = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| failure(start + e.offset, ErrorKind::Wire(e.kind)))?;
        let offset = start + f.span.start;
        budget.wire.work(1, offset)?;
        match (f.number, f.value) {
            (1, wire::Value::Varint(0)) => {},
            (3, wire::Value::Varint(v)) => nullable = Some(v != 0),
            (6, wire::Value::Varint(v)) => class = Some(v as u32 as i32),
            // Reject all significant non-nominal forms, including wrong-wire encodings.
            (1..=14, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
            _ => {},
        }
    }
    let index = class.ok_or(failure(start, ErrorKind::MissingClass))?;
    let mut text = resolver.get_string_with_budget(index, budget).map_err(|e| name_error(start, e))?;
    let local = resolver.is_local_class_name_with_budget(index, budget).map_err(|e| name_error(start, e))?;
    // Kotlin ClassId.fromString: last slash BEFORE first backtick, not last slash overall.
    // One bounded scan selects the package; one in-place pass preserves lone surrogates.
    budget.wire.work(text.len(), start)?;
    let mut split = None;
    for (i, &unit) in text.iter().enumerate() {
        if unit == u16::from(b'`') { break; }
        if unit == u16::from(b'/') { split = Some(i); }
    }
    budget.wire.work(text.len(), start)?;
    let relative_start = split.map_or(0, |i| i + 1);
    let package_end = split.unwrap_or(0);
    for unit in &mut text[..package_end] {
        if *unit == u16::from(b'/') { *unit = u16::from(b'.'); }
    }
    let mut write = relative_start;
    for read in relative_start..text.len() {
        if text[read] != u16::from(b'`') {
            text[write] = text[read];
            write += 1;
        }
    }
    text.truncate(write);
    Ok(NominalType { class_id: ClassId { text, package_end, relative_start, local }, nullable: nullable.unwrap_or(false), nullable_presence: nullable })
}

fn alias(
    bytes: &[u8], start: usize, ordinal: usize, span: Range<usize>,
    resolver: &mut name_resolver::Resolver<'_, '_>, budget: &mut string_table::Budget,
) -> Result<Signature, Error> {
    budget.node(2, start)?;
    let mut pos = 0;
    let mut name = None;
    let mut roles = [None, None];
    let mut ids = [false, false];
    while pos < bytes.len() {
        let f = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| failure(start + e.offset, ErrorKind::Wire(e.kind)))?;
        let offset = start + f.span.start;
        budget.wire.work(1, offset)?;
        match (f.number, f.value) {
            (2, wire::Value::Varint(v)) => name = Some(v as u32 as i32),
            (3, _) => return Err(failure(offset, ErrorKind::UnsupportedTypeParameters)),
            (4 | 6, wire::Value::Bytes(body)) => {
                let role = if f.number == 4 { 0 } else { 1 };
                if roles[role].is_some() { return Err(failure(offset, ErrorKind::DuplicateInlineRole)); }
                roles[role] = Some((body, start + f.span.end - body.len()));
            },
            (5 | 7, wire::Value::Varint(_)) => ids[if f.number == 5 { 0 } else { 1 }] = true,
            _ => {},
        }
    }
    let name = name.ok_or(failure(start, ErrorKind::MissingName))?;
    let mut selected = [(&[][..], 0); 2];
    for i in 0..2 {
        selected[i] = roles[i].ok_or(failure(start, if ids[i] { ErrorKind::UnsupportedTableRole } else { ErrorKind::MissingRole }))?;
    }
    let name = resolver.get_string_with_budget(name, budget).map_err(|e| name_error(start, e))?;
    let underlying = nominal(selected[0].0, selected[0].1, resolver, budget)?;
    let expanded = nominal(selected[1].0, selected[1].1, resolver, budget)?;
    Ok(Signature { ordinal, span, body_span: start..start + bytes.len(), name, underlying, expanded })
}

/// Resolve both required inline nominal roles for Package aliases (kind2/5, mv2.3.0).
/// Inline wins over an accompanying ID; duplicate inline messages explicitly refuse.
/// Every successful call scans the entire Package. Errors return no partial signatures.
/// Header/annotation/d1 decoding are earlier, independently bounded operations.
/// One shared budget covers framing, selection, leaf Types, names and owned output.
/// Unknown fields stay opaque; groups and significant unsupported Types refuse.
pub fn decode(metadata: &Outcome<'_>, limits: Limits) -> Result<Vec<Signature>, Error> {
    let Outcome::Decoded { header, bytes } = metadata else { return Err(failure(0, ErrorKind::UnsupportedContainer)); };
    if !matches!(header.k, 2 | 5) || header.mv != [2, 3, 0] { return Err(failure(0, ErrorKind::UnsupportedContainer)); }
    if header.d2.len() > limits.entries.min(MAX_RECORDS) {
        return Err(name_error(0, name_resolver::Error { kind: name_resolver::ErrorKind::Limit("entries"), index: None, record: None }));
    }
    let byte_error = failure(0, ErrorKind::Wire(wire::ErrorKind::Limit("bytes")));
    let mut input_bytes = bytes.len();
    if input_bytes > limits.bytes.min(MAX_BYTES) { return Err(byte_error); }
    for text in &header.d2 {
        input_bytes = text.len().checked_mul(2).and_then(|n| input_bytes.checked_add(n)).ok_or(byte_error)?;
        if input_bytes > limits.bytes.min(MAX_BYTES) { return Err(byte_error); }
    }
    let mut budget = string_table::Budget::new(string_table::Limits { bytes: limits.bytes, records: limits.records, nodes: limits.nodes, depth: limits.depth, work: limits.work, owned_bytes: limits.owned_bytes });
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let start = framed.payload_span.start;
    budget.node(1, start)?;
    let mut resolver = name_resolver::Resolver::new_with_budget(&framed.string_table, &header.d2, name_resolver::Limits { entries: limits.entries, input_bytes: limits.bytes, range_slots: limits.range_slots, queries: limits.queries, work: limits.work, output_units: limits.output_units }, &mut budget).map_err(|e| name_error(framed.string_table_span.start, e))?;
    let mut signatures = Vec::new();
    let mut pos = start;
    let mut ordinal = 0;
    while pos < bytes.len() {
        let f = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, f.span.start)?;
        if let (5, wire::Value::Bytes(body)) = (f.number, f.value) {
            let signature = alias(body, f.span.end - body.len(), ordinal, f.span.clone(), &mut resolver, &mut budget)?;
            budget.push(&mut signatures, signature, f.span.start)?;
        }
        ordinal += 1;
    }
    Ok(signatures)
}
