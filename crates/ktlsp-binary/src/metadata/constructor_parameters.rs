//! One-level Constructor/ValueParameter data under one aggregate budget.
use super::{constructor, string_table, value_parameter, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::constructor::Limits;
pub use super::value_parameter::{Error, ErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedValueParameter<'a> {
    /// Body-only offsets relative to the supplied Constructor bytes.
    pub span: Range<usize>,
    pub parameter: value_parameter::ValueParameter<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConstructorWithParameters<'a> {
    /// Complete input, preserving unknown/wrong-wire fields and duplicate order.
    pub raw: &'a [u8],
    /// Absent has schema default 6; distinct from explicit Some(6).
    pub flags: Option<i32>,
    pub value_parameters: Vec<SelectedValueParameter<'a>>,
    /// Ordered signed IDs, without lookup or applicability checks.
    pub version_requirements: Vec<i32>,
}

/// Decode ONE caller-selected Constructor body for supported mv=[2,3,0].
/// Caller selects version/kind/container. Complete parent validation precedes
/// children; parent and every selected child share records/work/storage accounting.
/// Errors and body spans are Constructor-relative. Child names/IDs are raw indices;
/// inline Types and deeper bodies remain opaque, without semantics or providers.
/// Temporary opaque-directory and retained typed-directory capacities both count;
/// requested storage excludes allocator surplus, churn, stack and RSS.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ConstructorWithParameters<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    let constructor::Constructor {
        raw,
        flags,
        value_parameters,
        version_requirements,
    } = constructor::decode_with_budget(bytes, &mut budget)?;
    let mut parameters = Vec::new();
    for body in value_parameters {
        let parameter =
            value_parameter::decode_with_budget(body.raw, &mut budget.wire).map_err(|error| {
                Error {
                    offset: body.span.start + error.offset,
                    ..error
                }
            })?;
        let start = body.span.start;
        budget.push(
            &mut parameters,
            SelectedValueParameter {
                span: body.span,
                parameter,
            },
            start,
        )?;
    }
    Ok(ConstructorWithParameters {
        raw,
        flags,
        value_parameters: parameters,
        version_requirements,
    })
}
