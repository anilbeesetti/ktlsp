//! One-level Class/TypeParameter occurrence data under one aggregate budget.
use super::{class_header, string_table, type_parameter, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedTypeParameter<'a> {
    /// Body-only offsets relative to the supplied Class bytes.
    pub span: Range<usize>,
    /// Opaque upper-bound spans remain relative to this parameter's raw bytes.
    pub parameter: type_parameter::TypeParameter<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassWithTypeParameters<'a> {
    pub header: class_header::ClassHeader<'a>,
    /// Every field5/wire2 occurrence, in encoded order without merging.
    pub type_parameters: Vec<SelectedTypeParameter<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    Class(class_header::ErrorKind),
    TypeParameter(type_parameter::ErrorKind),
}

/// All errors, including selected child faults, are Class-relative.
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

/// Decode ONE caller-selected Class body for supported k=1/mv=[2,3,0].
/// Complete parent wire and required fq_name validation precede child selection.
/// Both scans, all children and retained capacities share one monotonic budget.
/// Child faults rebase once; opaque child upper-bound spans remain child-relative.
/// Names/IDs and deeper bodies stay raw, without full nested initialization,
/// lookup, generic semantics, type reconstruction or a server consumer.
/// Logical retained capacity excludes allocator surplus, churn, stack and RSS.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassWithTypeParameters<'_>, Error> {
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
        class_header::decode_with_budget(bytes, &mut budget.wire).map_err(|error| Error {
            offset: error.offset,
            kind: ErrorKind::Class(error.kind),
        })?;
    let mut type_parameters = Vec::new();
    let mut pos = 0;
    // A charged second scan reuses parent validation without duplicating its schema.
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (5, wire::Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            budget.node(2, span.start)?;
            let parameter =
                type_parameter::decode_with_budget(raw, &mut budget).map_err(|error| Error {
                    offset: span.start + error.offset,
                    kind: ErrorKind::TypeParameter(error.kind),
                })?;
            budget.push(
                &mut type_parameters,
                SelectedTypeParameter { span, parameter },
                start,
            )?;
        }
    }
    Ok(ClassWithTypeParameters {
        header,
        type_parameters,
    })
}
