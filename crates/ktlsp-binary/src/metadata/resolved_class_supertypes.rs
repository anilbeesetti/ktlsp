//! Original Class-owned inline bounds and finite nominal supertype occurrences.
use super::{
    Outcome,
    class_context::{self, Owner, Parameter},
    class_id, payload,
    resolved_class_function_parameters::{self as parameters, Classifier},
    resolved_class_function_signature as signature, string_table, type_header, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Supertype<'a> {
    /// Position among ALL original Class fields (or TypeParameter fields for a bound).
    pub ordinal: usize,
    /// Original outer field and body ranges, relative to decoded d1 (not source).
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    /// Original borrowed Type bytes and encoded scalar Options.
    pub header: type_header::TypeHeader<'a>,
    /// Resolver-derived nominal identity/local membership; no target is loaded.
    pub class_id: class_id::ClassId,
    /// Original invariant/default inline Class-parameter leaves, in encoded order.
    pub arguments: Vec<signature::Argument<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bound<'a> {
    /// Checked ordinal in THIS result's original Class declarations, not an ID.
    pub parameter: usize,
    pub ty: Supertype<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassSupertypes<'a> {
    pub owner: Owner<'a>,
    /// Complete original ordered declarations, retaining their original bound data.
    pub parameters: Vec<Parameter<'a>>,
    /// Every original inline bound, in declaration order then encoded bound order.
    /// Absence never supplies an implicit bound or effective nullability.
    pub bounds: Vec<Bound<'a>>,
    /// Every original inline occurrence in encoded order, including repetitions.
    pub supertypes: Vec<Supertype<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Context(class_context::ErrorKind),
    Wire(wire::ErrorKind),
    Classifier(parameters::ErrorKind),
    UnsupportedClass,
    UnsupportedParameter,
    UnsupportedTableRole,
    UnsupportedType,
    UnsupportedProjection,
    DuplicateInlineRole,
    MissingRole,
}

/// Decoded-d1-relative offsets; errors expose no partial owner/list/declarations.
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
        Self {
            offset: e.offset,
            kind: ErrorKind::Wire(e.kind),
        }
    }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self {
        Self {
            offset: e.offset,
            kind: ErrorKind::Context(e.kind),
        }
    }
}
impl From<signature::Error> for Error {
    fn from(e: signature::Error) -> Self {
        let kind = match e.kind {
            signature::ErrorKind::Parameters(kind) => ErrorKind::Classifier(kind),
            signature::ErrorKind::Wire(kind) => ErrorKind::Wire(kind),
            signature::ErrorKind::Name(error) => {
                ErrorKind::Classifier(parameters::ErrorKind::Name(error))
            }
            signature::ErrorKind::UnsupportedTableRole => ErrorKind::UnsupportedTableRole,
            signature::ErrorKind::UnsupportedProjection => ErrorKind::UnsupportedProjection,
            signature::ErrorKind::DuplicateInlineRole => ErrorKind::DuplicateInlineRole,
            signature::ErrorKind::MissingRole => ErrorKind::MissingRole,
            signature::ErrorKind::UnsupportedFunction | signature::ErrorKind::UnsupportedType => {
                ErrorKind::UnsupportedType
            }
        };
        Self {
            offset: e.offset,
            kind,
        }
    }
}

fn nominal<'a>(
    body: &'a [u8],
    span: Range<usize>,
    ordinal: usize,
    context: &class_context::Context<'_>,
    resolver: &mut super::name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<Supertype<'a>, Error> {
    let start = span.end - body.len();
    budget.node(2, start)?;
    // None explicitly means immediate Class NAME scope, not Function-own scope.
    // Reuse the finite signature grammar and move its one owned ClassId/arguments.
    let interpreted = signature::finite_type(body, span, context, None, resolver, budget)?;
    let Classifier::Class(class_id) = interpreted.classifier else {
        unreachable!("Class scope admits only nominal roots")
    };
    Ok(Supertype {
        ordinal,
        span: interpreted.span,
        body_span: interpreted.body_span,
        header: interpreted.header,
        class_id,
        arguments: interpreted.arguments,
    })
}

/// Interpret ALL original Class inline bounds and field6/wire2 supertypes from
/// the SAME supported kind1/mv2.3.0 Outcome. Roots are bare nominal Types or
/// nominal Types with invariant/default single-inline Class-parameter leaves.
/// NAME resolves uniquely in the immediate original Class; signed IDs search
/// original declaration IDs, never positional ordinals. Bound references are
/// identity edges, not recursive expansion. Every selected ID/table role, mixed
/// representation, richer shape, significant wrong wire, malformed late child
/// or exhausted resource refuses the WHOLE operation without partial results.
/// Unused tables/unknown extensions/other nested declarations remain bounded
/// opaque custody. Scalar Options are encoded markings, not inferred semantics.
/// ONE context/resolver/monotonic allowance covers d1+d2, framing, declarations,
/// bounds, supertypes, arguments, resolution and output including repetitions.
/// Earlier class/annotation/header APIs have separate budgets. Limits bound
/// logical work/requested storage, not CPU/RSS/OS. Same-Outcome borrowing does not
/// authenticate models/archive bytes or correspondence to classfile this_class.
/// No implicit bounds, target loading, type graph, legal inheritance, hierarchy,
/// substitution, classpath, symbol authentication or server wiring is supplied.
pub fn decode<'a>(metadata: &'a Outcome<'_>, limits: Limits) -> Result<ClassSupertypes<'a>, Error> {
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
    let mut bounds = Vec::new();
    for (parameter, declaration) in context.parameters.iter().enumerate() {
        let raw = declaration.parameter.raw;
        let start = declaration.body_span.start;
        let mut pos = 0;
        let mut ordinal = 0;
        while pos < raw.len() {
            let field =
                wire::field(raw, &mut pos, raw.len(), &mut budget.wire).map_err(|e| Error {
                    offset: start + e.offset,
                    kind: ErrorKind::Wire(e.kind),
                })?;
            let offset = start + field.span.start;
            budget.wire.work(1, offset)?;
            match (field.number, field.value) {
                (1..=4, wire::Value::Varint(_)) => {}
                (5, wire::Value::Bytes(body)) => {
                    let ty = nominal(
                        body,
                        offset..start + field.span.end,
                        ordinal,
                        &context,
                        &mut resolver,
                        &mut budget,
                    )?;
                    budget.push(&mut bounds, Bound { parameter, ty }, offset)?;
                }
                (6, _) => {
                    return Err(Error {
                        offset,
                        kind: ErrorKind::UnsupportedTableRole,
                    });
                }
                (1..=5, _) => {
                    return Err(Error {
                        offset,
                        kind: ErrorKind::UnsupportedParameter,
                    });
                }
                _ => {}
            }
            ordinal += 1;
        }
    }
    let mut supertypes = Vec::new();
    let mut pos = framed.payload_span.start;
    let mut ordinal = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let offset = field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (2, _) => {
                return Err(Error {
                    offset,
                    kind: ErrorKind::UnsupportedTableRole,
                });
            }
            (6, wire::Value::Bytes(body)) => {
                let ty = nominal(
                    body,
                    field.span.clone(),
                    ordinal,
                    &context,
                    &mut resolver,
                    &mut budget,
                )?;
                budget.push(&mut supertypes, ty, offset)?;
            }
            (6, _) => {
                return Err(Error {
                    offset,
                    kind: ErrorKind::UnsupportedClass,
                });
            }
            _ => {}
        }
        ordinal += 1;
    }
    Ok(ClassSupertypes {
        owner: context.owner,
        parameters: context.parameters,
        bounds,
        supertypes,
    })
}
