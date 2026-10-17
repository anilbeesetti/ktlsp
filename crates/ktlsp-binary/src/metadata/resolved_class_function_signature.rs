//! One original Class Function's values and finite nominal parameter-leaf return.
use super::{
    Outcome, class_context, class_id, name_resolver, payload,
    resolved_class_function_parameters::{self as parameters, Classifier, SelectedParameters},
    string_table, type_argument, type_arguments, type_header, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Argument<'a> {
    /// Original body offsets relative to decoded d1.
    pub body_span: Range<usize>,
    /// Inline spans inside this original raw Argument remain Argument-relative.
    pub raw: type_argument::TypeArgument<'a>,
    pub header: type_header::TypeHeader<'a>,
    pub type_span: Range<usize>,
    /// Checked identity in `SelectedSignature.parameters`; never the Class variant.
    pub classifier: Classifier,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReturnType<'a> {
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub header: type_header::TypeHeader<'a>,
    /// Independently resolved target, NOT necessarily the Function's owning Class.
    pub class_id: class_id::ClassId,
    /// Original order; target arity is unknown without loading target declarations.
    pub arguments: Vec<Argument<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SelectedSignature<'a> {
    /// Original Class/Function declarations and ordered values, without copying.
    pub parameters: SelectedParameters<'a>,
    pub return_type: ReturnType<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Parameters(parameters::ErrorKind),
    Wire(wire::ErrorKind),
    Name(name_resolver::Error),
    UnsupportedFunction,
    MissingRole,
    UnsupportedTableRole,
    DuplicateInlineRole,
    UnsupportedType,
    UnsupportedProjection,
}
/// All offsets are decoded-d1-relative; no error exposes a partial signature.
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
impl From<parameters::Error> for Error {
    fn from(e: parameters::Error) -> Self {
        failure(e.offset, ErrorKind::Parameters(e.kind))
    }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self {
        parameters::Error::from(e).into()
    }
}
impl From<wire::Error> for Error {
    fn from(e: wire::Error) -> Self {
        failure(e.offset, ErrorKind::Wire(e.kind))
    }
}
fn failure(offset: usize, kind: ErrorKind) -> Error {
    Error { offset, kind }
}
fn wire_error(start: usize, e: wire::Error) -> Error {
    failure(start + e.offset, ErrorKind::Wire(e.kind))
}

fn nominal_return<'a>(
    bytes: &'a [u8],
    span: Range<usize>,
    context: &class_context::Context<'_>,
    own: &[class_context::Parameter<'_>],
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<ReturnType<'a>, Error> {
    let start = span.end - bytes.len();
    // Fixed flat selected slices, not a recursively decoded signature tree.
    let staged =
        type_arguments::decode_with_budget(bytes, budget).map_err(|e| wire_error(start, e))?;
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (1 | 3 | 6, wire::Value::Varint(_)) | (2, wire::Value::Bytes(_)) => {}
            (1..=14, _) => return Err(failure(offset, ErrorKind::UnsupportedType)),
            _ => {}
        }
    }
    if staged.header.flags.unwrap_or(0) != 0 {
        return Err(failure(start, ErrorKind::UnsupportedType));
    }
    let index = staged
        .header
        .class_name
        .ok_or(failure(start, ErrorKind::UnsupportedType))?;
    let class_id = class_id::resolve(index, start, resolver, budget).map_err(|e| match e {
        class_id::Error::Wire(e) => Error::from(e),
        class_id::Error::Name(e) => failure(start, ErrorKind::Name(e)),
    })?;
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
        if !matches!(
            argument.projection,
            None | Some(type_argument::Projection::Inv)
        ) {
            return Err(failure(argument_start, ErrorKind::UnsupportedProjection));
        }
        if argument.inline_types.len() > 1 {
            return Err(failure(argument_start, ErrorKind::DuplicateInlineRole));
        }
        let inline = argument.inline_types.first().ok_or(failure(
            argument_start,
            if argument.type_id.is_some() {
                ErrorKind::UnsupportedTableRole
            } else {
                ErrorKind::MissingRole
            },
        ))?;
        let leaf_start = argument_start + inline.span.start;
        budget.node(2, leaf_start)?;
        let header = type_header::decode_with_budget(inline.raw, &mut budget.wire)
            .map_err(|e| wire_error(leaf_start, e))?;
        // Reuse the value path's exact signed-ID and immediate-own UTF16 NAME rules.
        // Nominal leaves are outside this return grammar; do not resolve them.
        if header.class_name.is_some() {
            return Err(failure(leaf_start, ErrorKind::UnsupportedType));
        }
        let classifier = parameters::classifier(&header, leaf_start, context, own, resolver, budget)?;
        let type_span = leaf_start..leaf_start + inline.raw.len();
        budget.push(
            &mut arguments,
            Argument {
                body_span: argument_start..argument_start + argument.raw.len(),
                raw: argument,
                header,
                type_span,
                classifier,
            },
            argument_start,
        )?;
    }
    Ok(ReturnType {
        span,
        body_span: start..start + bytes.len(),
        header: staged.header,
        class_id,
        arguments,
    })
}

/// Select one original field9 by ALL-Class-field ordinal in the SAME Outcome.
/// Values retain the parameter-only contract; the return must be a nominal Class
/// with invariant inline signed-ID/own-NAME parameter leaves (possibly no leaves).
/// Nullable/projection Options remain encoded markings, not inferred semantics.
/// Receiver/context/contracts/JVM extensions/bounds/TypeTables remain opaque.
/// ONE context/resolver/monotonic allowance covers declarations, values AND return.
pub fn decode<'a>(
    metadata: &'a Outcome<'_>,
    ordinal: usize,
    limits: Limits,
) -> Result<SelectedSignature<'a>, Error> {
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
    let function = parameters::function_with_budget(
        bytes,
        framed.payload_span.start,
        ordinal,
        &context,
        &mut resolver,
        &mut budget,
    )?;
    let body = function.header.raw;
    let start = function.body_span.start;
    let mut inline = None;
    let mut table = false;
    let mut pos = 0;
    while pos < body.len() {
        let field = wire::field(body, &mut pos, body.len(), &mut budget.wire)
            .map_err(|e| wire_error(start, e))?;
        let offset = start + field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (3, wire::Value::Bytes(raw)) => {
                if inline.is_some() {
                    return Err(failure(offset, ErrorKind::DuplicateInlineRole));
                }
                inline = Some((raw, offset..start + field.span.end));
            }
            (7, wire::Value::Varint(_)) => table = true,
            (3 | 7, _) => return Err(failure(offset, ErrorKind::UnsupportedFunction)),
            _ => {}
        }
    }
    let (raw, span) = inline.ok_or(failure(
        start,
        if table {
            ErrorKind::UnsupportedTableRole
        } else {
            ErrorKind::MissingRole
        },
    ))?;
    let return_type = nominal_return(
        raw,
        span,
        &context,
        &function.type_parameters,
        &mut resolver,
        &mut budget,
    )?;
    Ok(SelectedSignature {
        parameters: SelectedParameters {
            owner: context.owner,
            parameters: context.parameters,
            function,
        },
        return_type,
    })
}
