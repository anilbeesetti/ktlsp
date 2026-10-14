//! One-level Function/Type occurrence data under one aggregate budget.
use super::{
    function_header, string_table, type_header,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::function_header::{Error, ErrorKind};
pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Exactly 3 (return_type) or 5 (receiver_type).
    pub field_number: u32,
    /// Body-only offsets relative to the supplied Function bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionWithTypes<'a> {
    pub header: function_header::FunctionHeader<'a>,
    /// Every recognized field3/5 Type in wire order, including empty bodies.
    pub types: Vec<SelectedType<'a>>,
}

/// Decode ONE caller-selected Function body for supported mv=[2,3,0].
/// Caller selects version/kind/container. Complete parent validation and required
/// name checking precede children; both scans and all Types share one budget.
/// Errors and body spans are Function-relative. Deeper bodies stay opaque;
/// IDs remain uninterpreted, without lookup, precedence, semantics or providers.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
/// shortcut: Type occurrences are unmerged; upgrade when merged Type interpretation is implemented.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionWithTypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let header = function_header::decode_with_budget(bytes, &mut budget.wire)?;
    let mut types = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (3 | 5, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let header =
                type_header::decode_with_budget(raw, &mut budget.wire).map_err(|error| {
                    wire::Error {
                        offset: span.start + error.offset,
                        ..error
                    }
                })?;
            budget.push(
                &mut types,
                SelectedType {
                    field_number: field.number,
                    span,
                    header,
                },
                start,
            )?;
        }
    }
    Ok(FunctionWithTypes { header, types })
}
