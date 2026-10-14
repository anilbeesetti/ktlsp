//! One-level selected Type/Argument data under one aggregate operation budget.
use super::{
    string_table, type_argument, type_header,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;
pub use super::wire::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedArgument<'a> {
    /// Body-only offsets relative to the supplied Type bytes.
    pub span: Range<usize>,
    pub argument: type_argument::TypeArgument<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeWithArguments<'a> {
    pub header: type_header::TypeHeader<'a>,
    /// Every recognized field2 Argument in encoded order, including empty bodies.
    pub arguments: Vec<SelectedArgument<'a>>,
}

/// Decode ONE caller-selected Type body for supported metadata mv=[2,3,0].
/// Caller independently selects version/kind/container. Header validation finishes
/// before children; both parent scans and all selected Arguments share one budget.
/// Errors are Type-relative; inline Type spans remain Argument-relative.
/// Inline Type bodies stay opaque and unmerged. No initialization, lookup,
/// nullability application, type reconstruction, semantics or server consumer.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeWithArguments<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let header = type_header::decode_with_budget(bytes, &mut budget.wire)?;
    let mut arguments = Vec::new();
    let mut pos = 0;
    // A charged second pass reuses the existing scalar decoder without schema duplication.
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (2, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let argument =
                type_argument::decode_with_budget(raw, &mut budget).map_err(|error| Error {
                    offset: span.start + error.offset,
                    ..error
                })?;
            budget.push(&mut arguments, SelectedArgument { span, argument }, start)?;
        }
    }
    Ok(TypeWithArguments { header, arguments })
}
