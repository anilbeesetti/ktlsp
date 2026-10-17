//! Original Class ownership shared by finite metadata consumers.
use super::{
    Outcome, class_header, class_id, name_resolver, payload, string_table, type_parameter, wire,
};
use crate::{MAX_BYTES, MAX_RECORDS};
use std::ops::Range;

use super::package_member_names::Limits;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnsupportedContainer,
    Wire(wire::ErrorKind),
    Class(class_header::ErrorKind),
    TypeParameter(type_parameter::ErrorKind),
    Name(name_resolver::Error),
    UnsupportedClass,
    UnsupportedParameter,
    DuplicateParameterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}
impl From<wire::Error> for Error {
    fn from(error: wire::Error) -> Self {
        Self {
            offset: error.offset,
            kind: ErrorKind::Wire(error.kind),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} at byte {}", self.kind, self.offset)
    }
}
impl std::error::Error for Error {}
fn failure(offset: usize, kind: ErrorKind) -> Error {
    Error { offset, kind }
}
pub(super) fn name_error(offset: usize, error: name_resolver::Error) -> Error {
    failure(offset, ErrorKind::Name(error))
}

pub(super) struct Context<'a> {
    pub owner: Owner<'a>,
    pub parameters: Vec<Parameter<'a>>,
}

pub(super) fn admit(metadata: &Outcome<'_>, limits: Limits) -> Result<string_table::Budget, Error> {
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
    Ok(string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        nodes: limits.nodes,
        depth: limits.depth,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
    }))
}

pub(super) fn resolver<'a, 'b>(
    framed: &'a payload::Payload<'b>,
    d2: &'a [&'b [u16]],
    limits: Limits,
    budget: &mut string_table::Budget,
) -> Result<name_resolver::Resolver<'a, 'b>, Error> {
    name_resolver::Resolver::new_with_budget(
        &framed.string_table,
        d2,
        name_resolver::Limits {
            entries: limits.entries,
            input_bytes: limits.bytes,
            range_slots: limits.range_slots,
            queries: limits.queries,
            work: limits.work,
            output_units: limits.output_units,
        },
        budget,
    )
    .map_err(|e| name_error(framed.string_table_span.start, e))
}

/// Append one original declaration to ONE scope, sharing the caller's allowance.
pub(super) fn push_parameter<'a>(
    body: &'a [u8],
    body_span: Range<usize>,
    offset: usize,
    parameters: &mut Vec<Parameter<'a>>,
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<(), Error> {
    budget.node(2, body_span.start)?;
    let parameter = type_parameter::decode_with_budget(body, budget)
        .map_err(|e| failure(body_span.start + e.offset, ErrorKind::TypeParameter(e.kind)))?;
    let mut pos = 0;
    while pos < body.len() {
        let child = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| failure(body_span.start + e.offset, ErrorKind::Wire(e.kind)))?;
        let child_offset = body_span.start + child.span.start;
        budget.wire.work(1, child_offset)?;
        if matches!(child.number, 1 | 2) && !matches!(child.value, wire::Value::Varint(_)) {
            return Err(failure(child_offset, ErrorKind::UnsupportedParameter));
        }
    }
    // ponytail: bounded quadratic duplicate detection; no secondary owning index.
    for previous in parameters.iter() {
        budget.wire.work(1, body_span.start)?;
        if previous.parameter.id == parameter.id {
            return Err(failure(body_span.start, ErrorKind::DuplicateParameterId));
        }
    }
    let name = resolver
        .get_string_with_budget(parameter.name, budget)
        .map_err(|e| name_error(body_span.start, e))?;
    budget.push(
        parameters,
        Parameter {
            body_span,
            parameter,
            name,
        },
        offset,
    )?;
    Ok(())
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    span: Range<usize>,
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<Context<'a>, Error> {
    let start = span.start;
    budget.node(1, start)?;
    let class = class_header::decode_with_budget(&bytes[span.clone()], &mut budget.wire)
        .map_err(|e| failure(start + e.offset, ErrorKind::Class(e.kind)))?;
    let class_id =
        class_id::resolve(class.fq_name, start, resolver, budget).map_err(|e| match e {
            class_id::Error::Wire(e) => Error::from(e),
            class_id::Error::Name(e) => name_error(start, e),
        })?;
    let mut parameters: Vec<Parameter<'a>> = Vec::new();
    let mut pos = start;
    // Read ALL declarations and finish Class framing before selecting any consumer.
    while pos < span.end {
        let field = wire::field(bytes, &mut pos, span.end, &mut budget.wire)?;
        let offset = field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3, wire::Value::Varint(_)) | (10, wire::Value::Bytes(_)) => {}
            (5, wire::Value::Bytes(body)) => {
                let body_span = field.span.end - body.len()..field.span.end;
                push_parameter(body, body_span, offset, &mut parameters, resolver, budget)?;
            }
            (1 | 3 | 5 | 10, _) => return Err(failure(offset, ErrorKind::UnsupportedClass)),
            _ => {}
        }
    }
    Ok(Context {
        owner: Owner {
            header: class,
            body_span: span,
            class_id,
        },
        parameters,
    })
}
