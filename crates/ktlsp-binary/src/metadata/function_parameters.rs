//! One-level Function/ValueParameter occurrence data under one aggregate budget.
use super::{
    function_header, string_table, value_parameter,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedParameter<'a> {
    /// Body-only offsets relative to the supplied Function bytes.
    pub span: Range<usize>,
    pub parameter: value_parameter::ValueParameter<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionWithParameters<'a> {
    pub header: function_header::FunctionHeader<'a>,
    /// Every recognized field6 ValueParameter in encoded order, without merging.
    pub parameters: Vec<SelectedParameter<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    Function(function_header::ErrorKind),
    Parameter(value_parameter::ErrorKind),
}

/// All offsets are relative to the supplied unframed Function bytes.
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

/// Decode ONE caller-selected Function body for supported metadata mv=[2,3,0].
/// Caller independently selects version/kind/container. Complete parent framing
/// and required-name validation precede the charged selection scan and children.
/// Both scans and all selected parameters share one monotonic operation budget.
/// Raw bytes borrow the original input; body spans and errors are Function-relative.
/// Deeper Types/IDs stay opaque, without lookup, precedence, semantics or providers.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: deeper bodies remain opaque; upgrade when nested metadata interpretation is implemented.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionWithParameters<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(limits);
    budget.node(1, 0)?;
    let header =
        function_header::decode_with_budget(bytes, &mut budget.wire).map_err(|error| Error {
            offset: error.offset,
            kind: ErrorKind::Function(error.kind),
        })?;
    let mut parameters = Vec::new();
    let mut pos = 0;
    // A charged second pass preserves the existing whole-parent scalar decoder.
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (6, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let parameter =
                value_parameter::decode_with_budget(raw, &mut budget.wire).map_err(|error| {
                    Error {
                        offset: span.start + error.offset,
                        kind: ErrorKind::Parameter(error.kind),
                    }
                })?;
            budget.push(
                &mut parameters,
                SelectedParameter { span, parameter },
                start,
            )?;
        }
    }
    Ok(FunctionWithParameters { header, parameters })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_work_admission_keeps_prior_node_record_and_storage_charges() {
        let mut budget = string_table::Budget::new(Limits {
            nodes: 1,
            depth: 1,
            work: 0,
            ..Default::default()
        });
        assert_eq!(
            budget.node(1, 0).unwrap_err().kind,
            wire::ErrorKind::Limit("work")
        );
        assert_eq!(
            budget.node(1, 0).unwrap_err().kind,
            wire::ErrorKind::Limit("nodes")
        );

        let mut wire = wire::Budget::new(1, 0);
        assert_eq!(
            wire.record(0).unwrap_err().kind,
            wire::ErrorKind::Limit("work")
        );
        assert_eq!(
            wire.record(0).unwrap_err().kind,
            wire::ErrorKind::Limit("records")
        );

        let mut budget = string_table::Budget::new(Limits {
            owned_bytes: std::mem::size_of::<SelectedParameter<'_>>(),
            work: 0,
            ..Default::default()
        });
        let mut values = Vec::new();
        for expected in ["work", "owned_bytes"] {
            let value = SelectedParameter {
                span: 0..0,
                parameter: value_parameter::ValueParameter {
                    raw: &[],
                    flags: None,
                    name: 0,
                    type_id: None,
                    vararg_element_type_id: None,
                },
            };
            assert_eq!(
                budget.push(&mut values, value, 0).unwrap_err().kind,
                wire::ErrorKind::Limit(expected)
            );
            assert_eq!(budget.owned, 0);
            assert!(values.is_empty());
            assert_eq!(values.capacity(), 0);
        }
    }
}
