//! One-level Function/TypeParameter occurrence data under one aggregate budget.
use super::{
    function_header, string_table, type_parameter,
    wire::{self, Value},
};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedTypeParameter<'a> {
    /// Body-only offsets relative to the supplied Function bytes.
    pub span: Range<usize>,
    /// Nested upper-bound spans remain relative to this TypeParameter body.
    pub parameter: type_parameter::TypeParameter<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionWithTypeParameters<'a> {
    pub header: function_header::FunctionHeader<'a>,
    /// Every recognized field4 TypeParameter in encoded order, without merging.
    pub parameters: Vec<SelectedTypeParameter<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    Function(function_header::ErrorKind),
    TypeParameter(type_parameter::ErrorKind),
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
/// Both scans, all selected parameters and their vectors share one operation budget.
/// Raw bytes borrow the input; outer spans and errors are Function-relative.
/// Nested upper-bound spans remain TypeParameter-relative, as in the child API.
/// Deeper Types/IDs stay opaque, without lookup, precedence, semantics or providers.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: deeper bodies remain opaque; upgrade when nested metadata interpretation is implemented.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<FunctionWithTypeParameters<'_>, Error> {
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
        if let (4, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let parameter =
                type_parameter::decode_with_budget(raw, &mut budget).map_err(|error| Error {
                    offset: span.start + error.offset,
                    kind: ErrorKind::TypeParameter(error.kind),
                })?;
            budget.push(
                &mut parameters,
                SelectedTypeParameter { span, parameter },
                start,
            )?;
        }
    }
    Ok(FunctionWithTypeParameters { header, parameters })
}
