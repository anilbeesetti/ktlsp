//! One-level ValueParameter/Type occurrence data under one aggregate budget.
use super::{
    string_table, type_header, value_parameter,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;
pub use super::value_parameter::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Exactly 3 (type) or 4 (vararg_element_type).
    pub field_number: u32,
    /// Body-only offsets relative to the supplied ValueParameter bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameterWithTypes<'a> {
    pub parameter: value_parameter::ValueParameter<'a>,
    /// Every recognized field3/4 Type in wire order, including empty bodies.
    pub types: Vec<SelectedType<'a>>,
}

/// Decode ONE caller-selected ValueParameter body for supported mv=[2,3,0].
/// Caller selects version/kind/container. Complete parent validation and required
/// name checking precede children; both scans and all Types share one budget.
/// Errors and body spans are ValueParameter-relative. Deeper bodies stay opaque;
/// IDs remain uninterpreted, without lookup, precedence, semantics or providers.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: Type occurrences are unmerged; upgrade when merged Type interpretation is implemented.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ValueParameterWithTypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let parameter = value_parameter::decode_with_budget(bytes, &mut budget.wire)?;
    let mut types = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (3 | 4, Value::Bytes(raw)) = (field.number, field.value) {
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
    Ok(ValueParameterWithTypes { parameter, types })
}
