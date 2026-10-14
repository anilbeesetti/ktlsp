//! One-level TypeAlias/Type occurrence data under one aggregate budget.
use super::{string_table, type_alias, type_header, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;
pub use super::type_alias::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedType<'a> {
    /// Exactly 4 (underlying_type) or 6 (expanded_type).
    pub field_number: u32,
    /// Body-only offsets relative to the supplied TypeAlias bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeAliasWithTypes<'a> {
    pub raw: &'a [u8],
    pub flags: Option<i32>,
    pub name: i32,
    pub underlying_type_id: Option<i32>,
    pub expanded_type_id: Option<i32>,
    /// Every field4/6 Type in wire order, including empty/duplicate bodies.
    pub types: Vec<SelectedType<'a>>,
}

fn body_span(bytes: &[u8], body: &[u8]) -> Result<Range<usize>, Error> {
    let invalid = || wire::Error {
        offset: bytes.len(),
        kind: wire::ErrorKind::Truncated,
    };
    let start = (body.as_ptr() as usize)
        .checked_sub(bytes.as_ptr() as usize)
        .ok_or_else(invalid)?;
    let end = start.checked_add(body.len()).ok_or_else(invalid)?;
    bytes.get(start..end).ok_or_else(invalid)?;
    Ok(start..end)
}

/// Decode ONE caller-selected TypeAlias body for supported mv=[2,3,0].
/// Complete parent validation and required name checking precede composition.
/// Parent staging lists and typed growth share one monotonic budget; staging
/// directories are consumed, never retained in the output or refunded.
/// Errors/body spans are TypeAlias-relative; IDs and deeper bodies stay opaque.
/// Requested storage excludes allocator surplus, churn, stack and RSS.
/// shortcut: singular Type occurrences are unmerged; upgrade with merged Type interpretation.
/// No alias expansion, type/name lookup, semantics or server consumer.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeAliasWithTypes<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    let type_alias::TypeAlias {
        raw,
        flags,
        name,
        underlying_type_id,
        expanded_type_id,
        underlying_types,
        expanded_types,
    } = type_alias::decode_with_budget(bytes, &mut budget)?;
    budget.node(1, 0)?;
    let mut underlying = underlying_types.into_iter();
    let mut expanded = expanded_types.into_iter();
    let mut types = Vec::new();
    loop {
        let u = underlying
            .as_slice()
            .first()
            .map(|body| body_span(bytes, body))
            .transpose()?;
        let e = expanded
            .as_slice()
            .first()
            .map(|body| body_span(bytes, body))
            .transpose()?;
        let both = u.is_some() && e.is_some();
        let (field_number, span) = match (u, e) {
            (Some(u), Some(e)) if u.start <= e.start => (4, u),
            (Some(_), Some(e)) | (None, Some(e)) => (6, e),
            (Some(u), None) => (4, u),
            (None, None) => break,
        };
        if both {
            budget.wire.work(1, span.start)?;
        }
        budget.wire.work(1, span.start)?;
        let body = if field_number == 4 {
            underlying.next()
        } else {
            expanded.next()
        }
        .ok_or(wire::Error {
            offset: bytes.len(),
            kind: wire::ErrorKind::Truncated,
        })?;
        budget.node(2, span.start)?;
        let header =
            type_header::decode_with_budget(body, &mut budget.wire).map_err(|error| match span
                .start
                .checked_add(error.offset)
                .filter(|&offset| offset <= span.end)
            {
                Some(offset) => wire::Error { offset, ..error },
                None => wire::Error {
                    offset: bytes.len(),
                    kind: wire::ErrorKind::Truncated,
                },
            })?;
        let start = span.start;
        budget.push(
            &mut types,
            SelectedType {
                field_number,
                span,
                header,
            },
            start,
        )?;
    }
    Ok(TypeAliasWithTypes {
        raw,
        flags,
        name,
        underlying_type_id,
        expanded_type_id,
        types,
    })
}
