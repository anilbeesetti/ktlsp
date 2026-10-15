//! One-level TypeParameter upper-bound data under one aggregate budget.
use super::{string_table, type_header, type_parameter, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;
pub use super::type_parameter::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedUpperBound<'a> {
    /// Body-only offsets relative to the supplied TypeParameter bytes.
    pub span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeParameterWithBounds<'a> {
    pub raw: &'a [u8],
    pub id: i32,
    pub name: i32,
    pub reified: Option<bool>,
    pub variance: Option<type_parameter::Variance>,
    pub upper_bound: Vec<SelectedUpperBound<'a>>,
    pub upper_bound_id: Vec<i32>,
}

/// Decode ONE caller-selected unframed TypeParameter for supported mv=[2,3,0].
/// Complete parent validation and required checks precede children. Parent,
/// children and cumulative staging/typed reservations share one budget, without
/// refunds; only the typed directory and moved IDs remain in the result.
/// Spans/errors are parent-relative. IDs and deeper bodies stay opaque; no lookup,
/// substitution, generic semantics or providers. Requested capacity excludes
/// allocator surplus/churn, stack and RSS; upstream operations stay separate.
/// shortcut: inline bounds are unmerged data; upgrade when Type interpretation is implemented.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeParameterWithBounds<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let type_parameter::TypeParameter {
        raw,
        id,
        name,
        reified,
        variance,
        upper_bound: staging,
        upper_bound_id,
    } = type_parameter::decode_with_budget(bytes, &mut budget)?;
    let mut upper_bound = Vec::new();
    for bound in staging {
        budget.node(2, bound.span.start)?;
        let header =
            type_header::decode_with_budget(bound.raw, &mut budget.wire).map_err(|error| {
                wire::Error {
                    offset: bound.span.start + error.offset,
                    ..error
                }
            })?;
        let start = bound.span.start;
        budget.push(
            &mut upper_bound,
            SelectedUpperBound {
                span: bound.span,
                header,
            },
            start,
        )?;
    }
    Ok(TypeParameterWithBounds {
        raw,
        id,
        name,
        reified,
        variance,
        upper_bound,
        upper_bound_id,
    })
}
