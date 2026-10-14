//! Ordered Class Function/Property UTF-16 names and raw flags, without symbol identity.
//! One budget covers already-decoded d1/d2 composition; opaque nested types stay opaque.
use super::{
    Outcome, class_header, function_header, name_resolver, payload, property_header, string_table,
    wire,
};
use crate::{MAX_BYTES, MAX_RECORDS};
use std::ops::Range;

/// Caller values can tighten, but cannot raise, the existing schema/resolver defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Combined decoded d1 and d2 UTF-16 bytes; header decoding is outside this call.
    pub bytes: usize,
    pub records: usize,
    pub nodes: usize,
    pub depth: usize,
    pub work: usize,
    /// Logical requested table/member capacity and name bytes, excluding allocator surplus/RSS.
    pub owned_bytes: usize,
    pub entries: usize,
    pub range_slots: usize,
    pub queries: usize,
    pub output_units: usize,
}
impl Default for Limits {
    fn default() -> Self {
        let schema = string_table::Limits::default();
        let names = name_resolver::Limits::default();
        Self {
            bytes: schema.bytes,
            records: schema.records,
            nodes: schema.nodes,
            depth: schema.depth,
            work: schema.work,
            owned_bytes: schema.owned_bytes,
            entries: names.entries,
            range_slots: names.range_slots,
            queries: names.queries,
            output_units: names.output_units,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Function,
    Property,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Member {
    pub kind: Kind,
    /// Zero-based position among ALL Class wire fields, not a semantic SymbolId.
    pub ordinal: usize,
    /// Entire outer field and nested body spans, both decoded-d1-relative.
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub name: Vec<u16>,
    pub flags: Option<i32>,
    pub old_flags: Option<i32>,
    /// Function accessor flags are None; Property presence is preserved independently.
    pub getter_flags: Option<i32>,
    pub setter_flags: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnsupportedContainer,
    Wire(wire::ErrorKind),
    Class(class_header::ErrorKind),
    Function(function_header::ErrorKind),
    Property(property_header::ErrorKind),
    Name(name_resolver::Error),
}

/// d1-relative offset; Name errors identify the member body start (table start at construction).
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

/// Require a supported Decoded Class, validate complete Class framing/fq_name, then
/// resolve selected member headers in encoded order. Duplicate names remain separate.
/// Wrong-wire member fields and constructor/alias/type bodies stay opaque. Successful
/// headers do not establish nested protobuf initialization, types or Kotlin semantics.
pub fn decode(metadata: &Outcome<'_>, limits: Limits) -> Result<Vec<Member>, Error> {
    let Outcome::Decoded { header, bytes } = metadata else {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::UnsupportedContainer,
        });
    };
    if header.k != 1 || header.mv != [2, 3, 0] {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::UnsupportedContainer,
        });
    }
    // Bound this length-only preflight before visiting caller-constructible d2 slices.
    if header.d2.len() > limits.entries.min(MAX_RECORDS) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Name(name_resolver::Error {
                kind: name_resolver::ErrorKind::Limit("entries"),
                index: None,
                record: None,
            }),
        });
    }
    let byte_error = wire::Error {
        offset: 0,
        kind: wire::ErrorKind::Limit("bytes"),
    };
    let mut input_bytes = bytes.len();
    if input_bytes > limits.bytes.min(MAX_BYTES) {
        return Err(byte_error.into());
    }
    for text in &header.d2 {
        input_bytes = text
            .len()
            .checked_mul(2)
            .and_then(|n| input_bytes.checked_add(n))
            .ok_or(byte_error)?;
        if input_bytes > limits.bytes.min(MAX_BYTES) {
            return Err(byte_error.into());
        }
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        nodes: limits.nodes,
        depth: limits.depth,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
    });
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let start = framed.payload_span.start;
    budget.node(1, start)?;
    class_header::decode_with_budget(framed.payload, &mut budget.wire).map_err(|e| Error {
        // Decoder offsets are bounded by their borrowed slice, itself within d1.
        offset: start + e.offset,
        kind: ErrorKind::Class(e.kind),
    })?;
    let mut resolver = name_resolver::Resolver::new_with_budget(
        &framed.string_table,
        &header.d2,
        name_resolver::Limits {
            entries: limits.entries,
            input_bytes: limits.bytes,
            range_slots: limits.range_slots,
            queries: limits.queries,
            work: limits.work,
            output_units: limits.output_units,
        },
        &mut budget,
    )
    .map_err(|e| Error {
        offset: framed.string_table_span.start,
        kind: ErrorKind::Name(e),
    })?;
    let mut members = Vec::new();
    let mut pos = start;
    let mut ordinal = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if let (9 | 10, wire::Value::Bytes(body)) = (field.number, field.value) {
            let body_start = field.span.end - body.len();
            budget.node(2, body_start)?;
            let (kind, name, flags, old_flags, getter_flags, setter_flags) =
                if field.number == 9 {
                    let h = function_header::decode_with_budget(body, &mut budget.wire).map_err(
                        |e| Error {
                            offset: body_start + e.offset,
                            kind: ErrorKind::Function(e.kind),
                        },
                    )?;
                    (Kind::Function, h.name, h.flags, h.old_flags, None, None)
                } else {
                    let h = property_header::decode_with_budget(body, &mut budget.wire).map_err(
                        |e| Error {
                            offset: body_start + e.offset,
                            kind: ErrorKind::Property(e.kind),
                        },
                    )?;
                    (
                        Kind::Property,
                        h.name,
                        h.flags,
                        h.old_flags,
                        h.getter_flags,
                        h.setter_flags,
                    )
                };
            let name = resolver
                .get_string_with_budget(name, &mut budget)
                .map_err(|e| Error {
                    offset: body_start,
                    kind: ErrorKind::Name(e),
                })?;
            let member = Member {
                kind,
                ordinal,
                body_span: body_start..field.span.end,
                span: field.span.clone(),
                name,
                flags,
                old_flags,
                getter_flags,
                setter_flags,
            };
            budget.push(&mut members, member, field.span.start)?;
        }
        ordinal += 1;
    }
    Ok(members)
}
