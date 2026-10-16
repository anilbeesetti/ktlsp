//! One original Class Function's self-applied, parameter-ID-only return.
use super::{
    Outcome, class_context::{self, Owner, Parameter}, class_id, function_header, name_resolver,
    payload, string_table, type_argument, type_arguments, type_header, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Argument<'a> {
    /// Original Argument body, decoded-d1-relative; inline spans inside raw remain local.
    pub body_span: Range<usize>,
    pub raw: type_argument::TypeArgument<'a>,
    pub header: type_header::TypeHeader<'a>,
    pub type_span: Range<usize>,
    /// Checked index into THIS result's original owning declarations, not an encoded ID.
    pub parameter_ordinal: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ReturnType<'a> {
    pub header: type_header::TypeHeader<'a>,
    pub body_span: Range<usize>,
    /// The nominal target is the returned owner; no duplicate ClassId is retained.
    pub arguments: Vec<Argument<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Function<'a> {
    /// Position among ALL original Class fields, not a Function-list/global ordinal.
    pub ordinal: usize,
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub header: function_header::FunctionHeader<'a>,
    pub name: Vec<u16>,
    pub return_type: ReturnType<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SelectedReturn<'a> {
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
    MissingOrdinal,
    NotFunction,
    UnsupportedFunction,
    UnsupportedTypeParameters,
    MissingRole,
    UnsupportedTableRole,
    DuplicateInlineRole,
    UnsupportedType,
    UnsupportedProjection,
    WrongReturnClass,
    ArgumentCount,
    UnknownParameterId,
}
/// All offsets are decoded-d1-relative. Failures never expose partial contexts/arguments.
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
    fn from(e: wire::Error) -> Self { failure(e.offset, ErrorKind::Wire(e.kind)) }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self { failure(e.offset, ErrorKind::Context(e.kind)) }
}
fn failure(offset: usize, kind: ErrorKind) -> Error { Error { offset, kind } }
fn wire_error(start: usize, e: wire::Error) -> Error {
    failure(start + e.offset, ErrorKind::Wire(e.kind))
}

fn type_shape(bytes: &[u8], start: usize, nominal: bool, budget: &mut string_table::Budget) -> Result<(), Error> {
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3, wire::Value::Varint(_)) => {}
            (6, wire::Value::Varint(_)) if nominal => {}
            (2, wire::Value::Bytes(_)) if nominal => {}
            (7, wire::Value::Varint(_)) if !nominal => {}
            (1..=14, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
            _ => {}
        }
    }
    Ok(())
}

fn applied_return<'a>(
    bytes: &'a [u8], start: usize, context: &class_context::Context<'_>,
    resolver: &mut name_resolver::Resolver<'_, '_>, budget: &mut string_table::Budget,
) -> Result<ReturnType<'a>, Error> {
    // Fixed flat selected slices retain depth <=2, NOT a recursive Class->Function->Type tree.
    let staged = type_arguments::decode_with_budget(bytes, budget)
        .map_err(|e| wire_error(start, e))?;
    type_shape(bytes, start, true, budget)?;
    if staged.header.flags.unwrap_or(0) != 0 {
        return Err(failure(start, ErrorKind::UnsupportedType));
    }
    let index = staged.header.class_name.ok_or(failure(start, ErrorKind::UnsupportedType))?;
    budget.wire.work(1, start)?;
    if index != context.owner.header.fq_name {
        let target = class_id::resolve(index, start, resolver, budget).map_err(|e| match e {
            class_id::Error::Wire(e) => Error::from(e),
            class_id::Error::Name(e) => failure(start, ErrorKind::Name(e)),
        })?;
        let owner = &context.owner.class_id;
        budget.wire.work(owner.package_name().len() + owner.relative_class_name().len() + 1, start)?;
        if target != *owner {
            return Err(failure(start, ErrorKind::WrongReturnClass));
        }
    }
    budget.wire.work(1, start)?;
    if staged.arguments.len() != context.parameters.len() {
        return Err(failure(start, ErrorKind::ArgumentCount));
    }
    let mut arguments = Vec::new();
    for selected in staged.arguments {
        let argument = selected.argument;
        let argument_start = start + selected.span.start;
        let mut pos = 0;
        while pos < argument.raw.len() {
            let field = wire::field(argument.raw, &mut pos, argument.raw.len(), &mut budget.wire)
                .map_err(|e| wire_error(argument_start, e))?;
            let offset = argument_start + field.span.start;
            budget.wire.work(1, offset)?;
            match (field.number, field.value) {
                (1, wire::Value::Varint(value)) if (0..=3).contains(&(value as u32 as i32)) => {}
                (1, _) => return Err(failure(offset, ErrorKind::UnsupportedProjection)),
                (2, wire::Value::Bytes(_)) | (3, wire::Value::Varint(_)) => {}
                (2 | 3, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
                _ => {}
            }
        }
        if !matches!(argument.projection, None | Some(type_argument::Projection::Inv)) {
            return Err(failure(argument_start, ErrorKind::UnsupportedProjection));
        }
        if argument.inline_types.len() > 1 {
            return Err(failure(argument_start, ErrorKind::DuplicateInlineRole));
        }
        let inline = argument.inline_types.first().ok_or(failure(argument_start,
            if argument.type_id.is_some() { ErrorKind::UnsupportedTableRole } else { ErrorKind::MissingRole }))?;
        let leaf_start = argument_start + inline.span.start;
        budget.node(2, leaf_start)?;
        let header = type_header::decode_with_budget(inline.raw, &mut budget.wire)
            .map_err(|e| wire_error(leaf_start, e))?;
        type_shape(inline.raw, leaf_start, false, budget)?;
        if header.flags.unwrap_or(0) != 0 {
            return Err(failure(leaf_start, ErrorKind::UnsupportedType));
        }
        let id = header.type_parameter.ok_or(failure(leaf_start, ErrorKind::UnsupportedType))?;
        let mut ordinal = None;
        // ponytail: bounded linear ID lookup; no secondary owning index.
        for (index, parameter) in context.parameters.iter().enumerate() {
            budget.wire.work(1, leaf_start)?;
            if parameter.parameter.id == id { ordinal = Some(index); break; }
        }
        let parameter_ordinal = ordinal.ok_or(failure(leaf_start, ErrorKind::UnknownParameterId))?;
        let type_span = leaf_start..leaf_start + inline.raw.len();
        budget.push(&mut arguments, Argument {
            body_span: argument_start..argument_start + argument.raw.len(),
            raw: argument, header, type_span, parameter_ordinal,
        }, argument_start)?;
    }
    Ok(ReturnType { header: staged.header, body_span: start..start + bytes.len(), arguments })
}

/// Select an original field9/wire2 by ALL-Class-field ordinal from the SAME Outcome.
/// Only a self-applied nominal return with invariant owning-parameter-ID leaves is
/// interpreted. Function-owned parameters/name classifiers/table-only roles refuse.
/// Inline beats ID; duplicate inline messages refuse merging. Nullable Options are
/// encoded MARKING, never effective bound-derived nullability. Other Functions and
/// receiver/value/context/contract/JVM bodies stay bounded opaque signature data.
/// One monotonic combined d1+d2/schema/wire/name/output budget covers the operation.
pub fn decode<'a>(metadata: &'a Outcome<'_>, ordinal: usize, limits: Limits) -> Result<SelectedReturn<'a>, Error> {
    let mut budget = class_context::admit(metadata, limits)?;
    let Outcome::Decoded { header, bytes } = metadata else { unreachable!() };
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let mut resolver = class_context::resolver(&framed, &header.d2, limits, &mut budget)?;
    let context = class_context::decode_with_budget(bytes, framed.payload_span.clone(), &mut resolver, &mut budget)?;
    let mut selected = None;
    let mut pos = framed.payload_span.start;
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
    let mut pos = 0;
    let mut inline = None;
    let mut table = false;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire).map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 2 | 9, wire::Value::Varint(_)) => {}
            (3, wire::Value::Bytes(raw)) => {
                if inline.is_some() { return Err(failure(offset, ErrorKind::DuplicateInlineRole)); }
                inline = Some((raw, start + field.span.end - raw.len()));
            }
            (7, wire::Value::Varint(_)) => table = true,
            (4, _) => return Err(failure(offset, ErrorKind::UnsupportedTypeParameters)),
            (1 | 2 | 3 | 7 | 9, _) => return Err(failure(offset, ErrorKind::UnsupportedFunction)),
            _ => {}
        }
    }
    let (raw, return_start) = inline.ok_or(failure(start,
        if table { ErrorKind::UnsupportedTableRole } else { ErrorKind::MissingRole }))?;
    let name = resolver.get_string_with_budget(function_header.name, &mut budget)
        .map_err(|e| failure(start, ErrorKind::Name(e)))?;
    let return_type = applied_return(raw, return_start, &context, &mut resolver, &mut budget)?;
    Ok(SelectedReturn {
        owner: context.owner, parameters: context.parameters,
        function: Function { ordinal, span, body_span: start..start + body.len(), header: function_header, name, return_type },
    })
}
