//! One original Class Constructor's finite inline value declarations and Class bindings.
use super::{
    Outcome,
    class_context::{self, Owner, Parameter},
    constructor, payload, resolved_class_function_parameters as values, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Constructor<'a> {
    /// Position among ALL original Class fields, not the Constructor-list ordinal.
    pub ordinal: usize,
    /// Original outer field and body ranges relative to decoded d1.
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub raw: &'a [u8],
    /// Absent has schema default 6; distinct from explicit Some(6).
    pub flags: Option<i32>,
    /// Raw signed IDs in packed/unpacked encounter order, without index lookup.
    pub version_requirements: Vec<i32>,
    pub parameters: Vec<values::ValueParameter<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedParameters<'a> {
    pub owner: Owner<'a>,
    pub parameters: Vec<Parameter<'a>>,
    pub constructor: Constructor<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Context(class_context::ErrorKind),
    Wire(wire::ErrorKind),
    Constructor(wire::ErrorKind),
    Value(values::ErrorKind),
    MissingOrdinal,
    NotConstructor,
    UnsupportedConstructor,
}
/// Decoded-d1-relative offsets; errors expose no partial context or values.
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
    fn from(e: wire::Error) -> Self {
        Self { offset: e.offset, kind: ErrorKind::Wire(e.kind) }
    }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self {
        Self { offset: e.offset, kind: ErrorKind::Context(e.kind) }
    }
}
impl From<values::Error> for Error {
    fn from(e: values::Error) -> Self {
        Self { offset: e.offset, kind: ErrorKind::Value(e.kind) }
    }
}

/// Select original field8/wire2 by ALL-Class-field ordinal from the SAME Outcome.
/// Admission, complete Class framing/declarations and complete raw Constructor
/// (including packed IDs) precede semantic traversal. Values then fail in encounter
/// order. ID lookup searches the original Class declarations; NAME resolves its
/// string but cannot search Class names: a Constructor has no own declarations.
/// One shared resolver and monotonic allowance cover staging and final capacity;
/// dropping temporary raw value staging does not refund its charges.
/// The existing finite inline value grammar applies, including vararg/table refusal.
/// Annotations, extensions, version applicability and JVM parameter indices stay opaque.
pub fn decode<'a>(
    metadata: &'a Outcome<'_>,
    ordinal: usize,
    limits: Limits,
) -> Result<SelectedParameters<'a>, Error> {
    let mut budget = class_context::admit(metadata, limits)?;
    let Outcome::Decoded { header, bytes } = metadata else { unreachable!() };
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let mut resolver = class_context::resolver(&framed, &header.d2, limits, &mut budget)?;
    let context = class_context::decode_with_budget(
        bytes, framed.payload_span.clone(), &mut resolver, &mut budget,
    )?;
    let mut selected = None;
    let mut pos = framed.payload_span.start;
    let mut current = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if current == ordinal {
            let (8, wire::Value::Bytes(body)) = (field.number, field.value) else {
                return Err(Error { offset: field.span.start, kind: ErrorKind::NotConstructor });
            };
            selected = Some((body, field.span));
        }
        current += 1;
    }
    let (body, span) = selected.ok_or(Error {
        offset: bytes.len(), kind: ErrorKind::MissingOrdinal,
    })?;
    let start = span.end - body.len();
    budget.node(2, start)?;
    // Drop the opaque value directory before semantic growth; its charges stay spent.
    let constructor::Constructor { flags, version_requirements, .. } =
        constructor::decode_with_budget(body, &mut budget).map_err(|e| Error {
            offset: start + e.offset, kind: ErrorKind::Constructor(e.kind),
        })?;
    let mut parameters = Vec::new();
    let mut pos = 0;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| Error { offset: start + e.offset, kind: ErrorKind::Wire(e.kind) })?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 31, wire::Value::Varint(_)) | (31, wire::Value::Bytes(_)) => {}
            (2, wire::Value::Bytes(value)) => {
                let parameter = values::parameter(
                    value, offset..start + field.span.end, parameters.len(),
                    &context, &[], &mut resolver, &mut budget,
                )?;
                budget.push(&mut parameters, parameter, offset)?;
            }
            (1 | 2 | 31, _) => return Err(Error { offset, kind: ErrorKind::UnsupportedConstructor }),
            _ => {}
        }
    }
    Ok(SelectedParameters {
        owner: context.owner,
        parameters: context.parameters,
        constructor: Constructor {
            ordinal, span, body_span: start..start + body.len(), raw: body,
            flags, version_requirements, parameters,
        },
    })
}
