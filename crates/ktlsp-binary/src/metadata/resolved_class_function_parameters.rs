//! One original Class Function's ordered, finite inline value-parameter declarations.
use super::{
    Outcome,
    class_context::{self, Owner, Parameter},
    class_id, function_header, name_resolver, payload, string_table, type_header, value_parameter,
    wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub enum Classifier {
    /// Checked ordinal in THIS result's original Class declarations, not an encoded ID.
    ClassParameter(usize),
    /// Checked ordinal in THIS selected Function's original own declarations.
    FunctionParameter(usize),
    /// Resolver-derived nominal identity, not a loaded class or global symbol ID.
    Class(class_id::ClassId),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValueParameter<'a> {
    /// Position in the selected callable's value list, not a Class/JVM ordinal.
    pub ordinal: usize,
    /// Original outer field and body ranges, relative to decoded d1 (not source).
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub raw: value_parameter::ValueParameter<'a>,
    pub name: Vec<u16>,
    pub header: type_header::TypeHeader<'a>,
    pub type_span: Range<usize>,
    pub classifier: Classifier,
}
impl ValueParameter<'_> {
    /// Only encoded declaration flags; no default expression/inheritance/mask mapping.
    pub fn declares_default_value(&self) -> bool {
        self.raw.flags.unwrap_or(0) & 2 != 0
    }
    pub fn has_annotations(&self) -> bool {
        self.raw.flags.unwrap_or(0) & 1 != 0
    }
    pub fn is_crossinline(&self) -> bool {
        self.raw.flags.unwrap_or(0) & 4 != 0
    }
    pub fn is_noinline(&self) -> bool {
        self.raw.flags.unwrap_or(0) & 8 != 0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Function<'a> {
    /// Position among ALL original Class fields, not the Function-list ordinal.
    pub ordinal: usize,
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub header: function_header::FunctionHeader<'a>,
    pub name: Vec<u16>,
    pub type_parameters: Vec<Parameter<'a>>,
    pub parameters: Vec<ValueParameter<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedParameters<'a> {
    pub owner: Owner<'a>,
    pub parameters: Vec<Parameter<'a>>,
    pub function: Function<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Context(class_context::ErrorKind),
    Wire(wire::ErrorKind),
    Name(name_resolver::Error),
    Function(function_header::ErrorKind),
    Parameter(value_parameter::ErrorKind),
    MissingOrdinal,
    NotFunction,
    UnsupportedFunction,
    UnsupportedParameter,
    UnsupportedVararg,
    MissingRole,
    UnsupportedTableRole,
    DuplicateInlineRole,
    UnsupportedType,
    UnknownParameterId,
    UnknownParameterName,
    AmbiguousParameterName,
}
/// Decoded-d1-relative offsets; errors expose no partial declarations/context.
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
        failure(e.offset, ErrorKind::Wire(e.kind))
    }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self {
        failure(e.offset, ErrorKind::Context(e.kind))
    }
}
fn failure(offset: usize, kind: ErrorKind) -> Error {
    Error { offset, kind }
}
fn wire_error(start: usize, e: wire::Error) -> Error {
    failure(start + e.offset, ErrorKind::Wire(e.kind))
}

pub(super) fn classifier(
    header: &type_header::TypeHeader<'_>,
    start: usize,
    context: &class_context::Context<'_>,
    own: &[Parameter<'_>],
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<Classifier, Error> {
    // Same bare-leaf admission as nominal aliases / Class-owned parameter returns.
    // Fixed flat slices, not recursive Class->Function->ValueParameter->Type depth.
    let mut pos = 0;
    while pos < header.raw.len() {
        let field = wire::field(header.raw, &mut pos, header.raw.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3 | 6 | 7 | 9, wire::Value::Varint(_)) => {}
            (1..=14, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
            _ => {}
        }
    }
    if header.flags.unwrap_or(0) != 0 {
        return Err(failure(start, ErrorKind::UnsupportedType));
    }
    match (
        header.class_name,
        header.type_parameter,
        header.type_parameter_name,
    ) {
        (Some(index), None, None) => {
            let id = class_id::resolve(index, start, resolver, budget).map_err(|e| match e {
                class_id::Error::Wire(e) => Error::from(e),
                class_id::Error::Name(e) => failure(start, ErrorKind::Name(e)),
            })?;
            Ok(Classifier::Class(id))
        }
        (None, Some(id), None) => {
            // ponytail: bounded linear scoped lookup; no secondary owning index.
            for (ordinal, parameter) in own.iter().enumerate() {
                budget.wire.work(1, start)?;
                if parameter.parameter.id == id {
                    return Ok(Classifier::FunctionParameter(ordinal));
                }
            }
            for (ordinal, parameter) in context.parameters.iter().enumerate() {
                budget.wire.work(1, start)?;
                if parameter.parameter.id == id {
                    return Ok(Classifier::ClassParameter(ordinal));
                }
            }
            Err(failure(start, ErrorKind::UnknownParameterId))
        }
        (None, None, Some(index)) => {
            let query = resolver
                .get_string_with_budget(index, budget)
                .map_err(|e| failure(start, ErrorKind::Name(e)))?;
            let mut found = None;
            for (ordinal, parameter) in own.iter().enumerate() {
                budget.wire.work(1, start)?;
                if query.len() != parameter.name.len() {
                    continue;
                }
                let mut equal = true;
                for (left, right) in query.iter().zip(&parameter.name) {
                    budget.wire.work(1, start)?;
                    if left != right {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    if found.is_some() {
                        return Err(failure(start, ErrorKind::AmbiguousParameterName));
                    }
                    found = Some(ordinal);
                }
            }
            found
                .map(Classifier::FunctionParameter)
                .ok_or(failure(start, ErrorKind::UnknownParameterName))
        }
        _ => Err(failure(start, ErrorKind::UnsupportedType)),
    }
}

pub(super) fn parameter<'a>(
    body: &'a [u8],
    span: Range<usize>,
    ordinal: usize,
    context: &class_context::Context<'_>,
    own: &[Parameter<'_>],
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<ValueParameter<'a>, Error> {
    let start = span.end - body.len();
    budget.node(2, start)?;
    let raw = value_parameter::decode_with_budget(body, &mut budget.wire)
        .map_err(|e| failure(start + e.offset, ErrorKind::Parameter(e.kind)))?;
    let mut pos = 0;
    let mut inline = None;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 2 | 5, wire::Value::Varint(_)) => {}
            (3, wire::Value::Bytes(ty)) => {
                if inline.is_some() {
                    return Err(failure(offset, ErrorKind::DuplicateInlineRole));
                }
                inline = Some((ty, start + field.span.end - ty.len()));
            }
            (4 | 6, _) => return Err(failure(offset, ErrorKind::UnsupportedVararg)),
            (1 | 2 | 3 | 5, _) => {
                return Err(failure(offset, ErrorKind::UnsupportedParameter));
            }
            _ => {}
        }
    }
    let (ty, type_start) = inline.ok_or(failure(
        start,
        if raw.type_id.is_some() {
            ErrorKind::UnsupportedTableRole
        } else {
            ErrorKind::MissingRole
        },
    ))?;
    budget.node(2, type_start)?;
    let header = type_header::decode_with_budget(ty, &mut budget.wire)
        .map_err(|e| wire_error(type_start, e))?;
    let classifier = classifier(&header, type_start, context, own, resolver, budget)?;
    let name = resolver
        .get_string_with_budget(raw.name, budget)
        .map_err(|e| failure(start, ErrorKind::Name(e)))?;
    Ok(ValueParameter {
        ordinal,
        span,
        body_span: start..start + body.len(),
        raw,
        name,
        header,
        type_span: type_start..type_start + ty.len(),
        classifier,
    })
}

/// Select one original field9/wire2 by ALL-Class-field ordinal from the SAME Outcome.
/// Every field6 value declaration is interpreted in order; unsupported types never
/// disappear from the list. Inline type3 beats type-ID5; duplicate messages,
/// table-only roles and vararg roles refuse. ID searches own then Class declarations;
/// NAME requires exactly one matching own resolved UTF-16 name, never a parent.
/// Nullable presence is encoded marking, not effective bound-derived nullability.
/// Return/receiver/context/contracts/JVM extensions and parent TypeTables stay opaque.
/// One monotonic d1+d2/schema/wire/name/output allowance covers the entire operation.
pub fn decode<'a>(
    metadata: &'a Outcome<'_>,
    ordinal: usize,
    limits: Limits,
) -> Result<SelectedParameters<'a>, Error> {
    let mut budget = class_context::admit(metadata, limits)?;
    let Outcome::Decoded { header, bytes } = metadata else {
        unreachable!()
    };
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let mut resolver = class_context::resolver(&framed, &header.d2, limits, &mut budget)?;
    let context = class_context::decode_with_budget(
        bytes,
        framed.payload_span.clone(),
        &mut resolver,
        &mut budget,
    )?;
    let function = function_with_budget(
        bytes,
        framed.payload_span.start,
        ordinal,
        &context,
        &mut resolver,
        &mut budget,
    )?;
    Ok(SelectedParameters {
        owner: context.owner,
        parameters: context.parameters,
        function,
    })
}

/// Shared selected Function path; the caller retains the sole context and allowance.
pub(super) fn function_with_budget<'a>(
    bytes: &'a [u8],
    class_start: usize,
    ordinal: usize,
    context: &class_context::Context<'_>,
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<Function<'a>, Error> {
    let mut selected = None;
    let mut pos = class_start;
    let mut current = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if current == ordinal {
            let (9, wire::Value::Bytes(body)) = (field.number, field.value) else {
                return Err(failure(field.span.start, ErrorKind::NotFunction));
            };
            selected = Some((body, field.span));
        }
        current += 1;
    }
    let (body, span) = selected.ok_or(failure(bytes.len(), ErrorKind::MissingOrdinal))?;
    let start = span.end - body.len();
    budget.node(2, start)?;
    let function_header = function_header::decode_with_budget(body, &mut budget.wire)
        .map_err(|e| failure(start + e.offset, ErrorKind::Function(e.kind)))?;
    // Finish ALL own declarations BEFORE interpreting any value, including late fields.
    let mut type_parameters = Vec::new();
    let mut pos = 0;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 2 | 9, wire::Value::Varint(_)) | (6, wire::Value::Bytes(_)) => {}
            (4, wire::Value::Bytes(raw)) => {
                class_context::push_parameter(
                    raw,
                    start + field.span.end - raw.len()..start + field.span.end,
                    offset,
                    &mut type_parameters,
                    resolver,
                    budget,
                )?;
            }
            (1 | 2 | 4 | 6 | 9, _) => {
                return Err(failure(offset, ErrorKind::UnsupportedFunction));
            }
            _ => {}
        }
    }
    let name = resolver
        .get_string_with_budget(function_header.name, budget)
        .map_err(|e| failure(start, ErrorKind::Name(e)))?;
    let mut parameters = Vec::new();
    let mut pos = 0;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        if let (6, wire::Value::Bytes(raw)) = (field.number, field.value) {
            let selected = parameter(
                raw,
                offset..start + field.span.end,
                parameters.len(),
                context,
                &type_parameters,
                resolver,
                budget,
            )?;
            budget.push(&mut parameters, selected, offset)?;
        }
    }
    Ok(Function {
        ordinal,
        span,
        body_span: start..start + body.len(),
        header: function_header,
        name,
        type_parameters,
        parameters,
    })
}
