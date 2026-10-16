//! Finite Class-owned parameter-ID property returns, not substitution or inference.
use super::{
    Outcome,
    class_context::{self, Owner, Parameter},
    class_header, name_resolver, payload, property_header, string_table, type_header,
    type_parameter, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct ReturnType<'a> {
    pub header: type_header::TypeHeader<'a>,
    pub body_span: Range<usize>,
    /// Index of the original declaration in THIS result's parameters, not its encoded ID.
    /// Identity is the returned owner context plus this declaration ordinal.
    pub parameter_ordinal: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Property<'a> {
    /// Encoded position among all Class fields, not a global symbol ID.
    pub ordinal: usize,
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub header: property_header::PropertyHeader<'a>,
    pub name: Vec<u16>,
    pub return_type: ReturnType<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassProperties<'a> {
    pub owner: Owner<'a>,
    /// Original Class parameter declarations in encoded order; names need not be unique.
    pub parameters: Vec<Parameter<'a>>,
    pub properties: Vec<Property<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnsupportedContainer,
    Wire(wire::ErrorKind),
    Class(class_header::ErrorKind),
    TypeParameter(type_parameter::ErrorKind),
    Property(property_header::ErrorKind),
    Name(name_resolver::Error),
    UnsupportedClass,
    UnsupportedParameter,
    DuplicateParameterId,
    UnknownParameterId,
    UnsupportedProperty,
    UnsupportedTypeParameters,
    MissingRole,
    UnsupportedTableRole,
    DuplicateInlineRole,
    UnsupportedType,
    MissingParameterId,
}

/// All error offsets are decoded-d1-relative; raw nested bound spans remain local.
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
        failure(error.offset, ErrorKind::Wire(error.kind))
    }
}
impl From<class_context::Error> for Error {
    fn from(error: class_context::Error) -> Self {
        use class_context::ErrorKind as Kind;
        let kind = match error.kind {
            Kind::UnsupportedContainer => ErrorKind::UnsupportedContainer,
            Kind::Wire(e) => ErrorKind::Wire(e),
            Kind::Class(e) => ErrorKind::Class(e),
            Kind::TypeParameter(e) => ErrorKind::TypeParameter(e),
            Kind::Name(e) => ErrorKind::Name(e),
            Kind::UnsupportedClass => ErrorKind::UnsupportedClass,
            Kind::UnsupportedParameter => ErrorKind::UnsupportedParameter,
            Kind::DuplicateParameterId => ErrorKind::DuplicateParameterId,
        };
        failure(error.offset, kind)
    }
}
fn failure(offset: usize, kind: ErrorKind) -> Error {
    Error { offset, kind }
}
fn wire_error(start: usize, error: wire::Error) -> Error {
    failure(start + error.offset, ErrorKind::Wire(error.kind))
}
fn name_error(offset: usize, error: name_resolver::Error) -> Error {
    failure(offset, ErrorKind::Name(error))
}

fn return_type<'a>(
    bytes: &'a [u8],
    start: usize,
    parameters: &[Parameter<'_>],
    budget: &mut string_table::Budget,
) -> Result<ReturnType<'a>, Error> {
    // Flat leaf selection, NOT recursive Class -> Property -> Type depth-3 admission.
    budget.node(2, start)?;
    let header = type_header::decode_with_budget(bytes, &mut budget.wire)
        .map_err(|e| wire_error(start, e))?;
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3 | 7, wire::Value::Varint(_)) => {}
            // NAME only searches the immediate property scope, NEVER the parent Class.
            (1..=14, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
            _ => {}
        }
    }
    if header.flags.unwrap_or(0) != 0 {
        return Err(failure(start, ErrorKind::UnsupportedType));
    }
    let id = header
        .type_parameter
        .ok_or(failure(start, ErrorKind::MissingParameterId))?;
    // ponytail: bounded linear ID lookup; a shared index is only needed for larger budgets.
    let mut ordinal = None;
    for (index, parameter) in parameters.iter().enumerate() {
        budget.wire.work(1, start)?;
        if parameter.parameter.id == id {
            ordinal = Some(index);
            break;
        }
    }
    Ok(ReturnType {
        header,
        body_span: start..start + bytes.len(),
        parameter_ordinal: ordinal.ok_or(failure(start, ErrorKind::UnknownParameterId))?,
    })
}

fn property<'a>(
    bytes: &'a [u8],
    start: usize,
    ordinal: usize,
    span: Range<usize>,
    parameters: &[Parameter<'_>],
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<Property<'a>, Error> {
    budget.node(2, start)?;
    let header = property_header::decode_with_budget(bytes, &mut budget.wire)
        .map_err(|e| failure(start + e.offset, ErrorKind::Property(e.kind)))?;
    let mut pos = 0;
    let mut inline = None;
    let mut table = false;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 2 | 7 | 8 | 11, wire::Value::Varint(_)) => {}
            (3, wire::Value::Bytes(body)) => {
                if inline.is_some() {
                    return Err(failure(offset, ErrorKind::DuplicateInlineRole));
                }
                inline = Some((body, start + field.span.end - body.len()));
            }
            (9, wire::Value::Varint(_)) => table = true,
            (4, _) => return Err(failure(offset, ErrorKind::UnsupportedTypeParameters)),
            (1..=5 | 7..=13 | 17, _) => {
                return Err(failure(offset, ErrorKind::UnsupportedProperty));
            }
            _ => {}
        }
    }
    let (body, type_start) = inline.ok_or(failure(
        start,
        if table {
            ErrorKind::UnsupportedTableRole
        } else {
            ErrorKind::MissingRole
        },
    ))?;
    let name = resolver
        .get_string_with_budget(header.name, budget)
        .map_err(|e| name_error(start, e))?;
    let return_type = return_type(body, type_start, parameters, budget)?;
    Ok(Property {
        ordinal,
        span,
        body_span: start..start + bytes.len(),
        header,
        name,
        return_type,
    })
}

/// Bind every selected property return to an original parameter of the SAME Class.
/// Requires decoded kind1/mv2.3.0 and one inline bare parameter-ID Type per property.
/// Inline wins over an accompanying table ID; duplicate inline messages refuse merging.
/// Any unsupported selected member fails atomically. Bounds/accessors/annotations and
/// unselected tables remain opaque. nullable is encoded marking, NOT effective nullability.
/// Header/annotation/d1 decoding are earlier independently bounded operations.
/// One monotonic schema/wire/resolver/output budget covers this entire operation.
pub fn decode<'a>(metadata: &'a Outcome<'_>, limits: Limits) -> Result<ClassProperties<'a>, Error> {
    let mut budget = class_context::admit(metadata, limits)?;
    let Outcome::Decoded { header, bytes } = metadata else {
        unreachable!()
    };
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let start = framed.payload_span.start;
    let mut resolver = class_context::resolver(&framed, &header.d2, limits, &mut budget)?;
    let class_context::Context { owner, parameters } = class_context::decode_with_budget(
        bytes,
        framed.payload_span.clone(),
        &mut resolver,
        &mut budget,
    )?;
    let mut properties = Vec::new();
    let mut pos = start;
    let mut ordinal = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if let (10, wire::Value::Bytes(body)) = (field.number, field.value) {
            let property = property(
                body,
                field.span.end - body.len(),
                ordinal,
                field.span.clone(),
                &parameters,
                &mut resolver,
                &mut budget,
            )?;
            budget.push(&mut properties, property, field.span.start)?;
        }
        ordinal += 1;
    }
    Ok(ClassProperties {
        owner,
        parameters,
        properties,
    })
}
