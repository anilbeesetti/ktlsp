//! Finite Class-owned parameter-ID property returns, not substitution or inference.
use super::{
    Outcome, class_header, class_id, name_resolver, payload, property_header, string_table,
    type_header, type_parameter, wire,
};
use crate::{MAX_BYTES, MAX_RECORDS};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Owner<'a> {
    pub header: class_header::ClassHeader<'a>,
    /// Decoded-d1-relative Class body; header.raw borrows exactly this body.
    pub body_span: Range<usize>,
    pub class_id: class_id::ClassId,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parameter<'a> {
    /// Decoded-d1-relative original declaration body. Bound spans stay parameter-relative.
    pub body_span: Range<usize>,
    pub parameter: type_parameter::TypeParameter<'a>,
    pub name: Vec<u16>,
}

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
    let Outcome::Decoded { header, bytes } = metadata else {
        return Err(failure(0, ErrorKind::UnsupportedContainer));
    };
    if header.k != 1 || header.mv != [2, 3, 0] {
        return Err(failure(0, ErrorKind::UnsupportedContainer));
    }
    if header.d2.len() > limits.entries.min(MAX_RECORDS) {
        return Err(name_error(
            0,
            name_resolver::Error {
                kind: name_resolver::ErrorKind::Limit("entries"),
                index: None,
                record: None,
            },
        ));
    }
    let byte_error = failure(0, ErrorKind::Wire(wire::ErrorKind::Limit("bytes")));
    let mut input_bytes = bytes.len();
    if input_bytes > limits.bytes.min(MAX_BYTES) {
        return Err(byte_error);
    }
    for text in &header.d2 {
        input_bytes = text
            .len()
            .checked_mul(2)
            .and_then(|n| input_bytes.checked_add(n))
            .ok_or(byte_error)?;
        if input_bytes > limits.bytes.min(MAX_BYTES) {
            return Err(byte_error);
        }
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        nodes: limits.nodes,
        depth: limits.depth,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
    });
    let framed = payload::decode_with_budget(bytes, &mut budget)?;
    let start = framed.payload_span.start;
    budget.node(1, start)?;
    let class = class_header::decode_with_budget(framed.payload, &mut budget.wire)
        .map_err(|e| failure(start + e.offset, ErrorKind::Class(e.kind)))?;
    let mut resolver = name_resolver::Resolver::new_with_budget(
        &framed.string_table,
        &header.d2,
        name_resolver::Limits {
            entries: limits.entries,
            input_bytes: limits.bytes,
            range_slots: limits.range_slots,
            queries: limits.queries,
            work: limits.work,
            output_units: limits.output_units,
        },
        &mut budget,
    )
    .map_err(|e| name_error(framed.string_table_span.start, e))?;
    let class_id = class_id::resolve(class.fq_name, start, &mut resolver, &mut budget)
        .map_err(|e| match e {
            class_id::Error::Wire(e) => Error::from(e),
            class_id::Error::Name(e) => name_error(start, e),
        })?;
    let mut parameters: Vec<Parameter<'a>> = Vec::new();
    let mut pos = start;
    // Read ALL owning declarations first, including declarations encoded after properties.
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let offset = field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3, wire::Value::Varint(_)) | (10, wire::Value::Bytes(_)) => {}
            (5, wire::Value::Bytes(body)) => {
                let body_span = field.span.end - body.len()..field.span.end;
                budget.node(2, body_span.start)?;
                let parameter = type_parameter::decode_with_budget(body, &mut budget)
                    .map_err(|e| failure(body_span.start + e.offset, ErrorKind::TypeParameter(e.kind)))?;
                let mut child_pos = 0;
                while child_pos < body.len() {
                    let child = wire::field(body, &mut child_pos, body.len(), &mut budget.wire)
                        .map_err(|e| wire_error(body_span.start, e))?;
                    let child_offset = body_span.start + child.span.start;
                    budget.wire.work(1, child_offset)?;
                    if matches!(child.number, 1 | 2)
                        && !matches!(child.value, wire::Value::Varint(_))
                    {
                        return Err(failure(child_offset, ErrorKind::UnsupportedParameter));
                    }
                }
                // ponytail: bounded quadratic duplicate detection; no secondary owning index.
                for previous in &parameters {
                    budget.wire.work(1, body_span.start)?;
                    if previous.parameter.id == parameter.id {
                        return Err(failure(body_span.start, ErrorKind::DuplicateParameterId));
                    }
                }
                let name = resolver
                    .get_string_with_budget(parameter.name, &mut budget)
                    .map_err(|e| name_error(body_span.start, e))?;
                budget.push(
                    &mut parameters,
                    Parameter {
                        body_span,
                        parameter,
                        name,
                    },
                    offset,
                )?;
            }
            (1 | 3 | 5 | 10, _) => return Err(failure(offset, ErrorKind::UnsupportedClass)),
            _ => {}
        }
    }
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
        owner: Owner {
            header: class,
            body_span: framed.payload_span,
            class_id,
        },
        parameters,
        properties,
    })
}
