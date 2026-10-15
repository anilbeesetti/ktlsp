//! One-level Argument/Type occurrence data under one aggregate operation budget.
use super::{string_table, type_argument, type_header};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;
pub use super::wire::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Body-only offsets relative to the supplied Argument bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeArgumentWithTypes<'a> {
    pub raw: &'a [u8],
    pub projection: Option<type_argument::Projection>,
    pub type_id: Option<i32>,
    /// Every field2/wire2 Type in encoded order, including empty bodies.
    pub inline_types: Vec<SelectedType<'a>>,
}

/// Decode ONE caller-selected Argument body for supported mv=[2,3,0].
/// Complete parent validation precedes children. Parent staging and typed growth
/// share one monotonic budget; staging is consumed, never retained or refunded.
/// Errors/body spans are Argument-relative. Deeper bodies and IDs stay opaque;
/// no initialization, projection semantics, type/name lookup or server consumer.
/// Requested storage excludes allocator surplus, churn, stack and RSS.
/// shortcut: Type occurrences are unmerged; upgrade with merged Type interpretation.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeArgumentWithTypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let type_argument::TypeArgument {
        raw,
        projection,
        type_id,
        inline_types,
    } = type_argument::decode_with_budget(bytes, &mut budget)?;
    let mut types = Vec::new();
    for inline in inline_types {
        let start = inline.span.start;
        budget.node(2, start)?;
        let header =
            type_header::decode_with_budget(inline.raw, &mut budget.wire).map_err(|error| {
                Error {
                    offset: start + error.offset,
                    ..error
                }
            })?;
        budget.push(
            &mut types,
            SelectedType {
                span: inline.span,
                header,
            },
            start,
        )?;
    }
    Ok(TypeArgumentWithTypes {
        raw,
        projection,
        type_id,
        inline_types: types,
    })
}
