//! One-level Class/supertype Type occurrence data under one aggregate budget.
use super::{
    string_table, type_header,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::class_header::{Error, ErrorKind};
pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedSupertype<'a> {
    /// Body-only offsets relative to the supplied Class bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassWithSupertypes<'a> {
    pub header: super::class_header::ClassHeader<'a>,
    /// Every field6/wire2 Type in encoded order, including empty bodies.
    pub supertypes: Vec<SelectedSupertype<'a>>,
}

/// Decode ONE caller-selected Class body for supported k=1/mv=[2,3,0].
/// Caller selects version/kind/container. Complete parent wire and required
/// fq_name validation precede children; both scans and all Types share one budget.
/// Errors and body spans are Class-relative. Occurrences stay separate and
/// unmerged; deeper bodies and supertype IDs stay opaque. No initialization,
/// lookup, inheritance semantics, type reconstruction or server consumer.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassWithSupertypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let header = super::class_header::decode_with_budget(bytes, &mut budget.wire)?;
    let mut supertypes = Vec::new();
    let mut pos = 0;
    // A charged second scan reuses the existing scalar decoder without schema duplication.
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (6, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let header =
                type_header::decode_with_budget(raw, &mut budget.wire).map_err(|error| {
                    wire::Error {
                        offset: span.start + error.offset,
                        ..error
                    }
                })?;
            budget.push(&mut supertypes, SelectedSupertype { span, header }, start)?;
        }
    }
    Ok(ClassWithSupertypes { header, supertypes })
}
