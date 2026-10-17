//! Original Class-owned bare nominal supertype occurrences, not a class hierarchy.
use super::{
    Outcome,
    class_context::{self, Owner, Parameter},
    class_id, payload,
    resolved_class_function_parameters::{self as parameters, Classifier},
    type_header, wire,
};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Supertype<'a> {
    /// Position among ALL original Class fields, not the supertype-list ordinal.
    pub ordinal: usize,
    /// Original outer field and body ranges, relative to decoded d1 (not source).
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    /// Original borrowed Type bytes and encoded scalar Options.
    pub header: type_header::TypeHeader<'a>,
    /// Resolver-derived nominal identity/local membership; no target is loaded.
    pub class_id: class_id::ClassId,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassSupertypes<'a> {
    pub owner: Owner<'a>,
    /// Complete original ordered declarations. Encoded bounds remain opaque;
    /// absence is not an interpreted implicit bound or effective nullability.
    pub parameters: Vec<Parameter<'a>>,
    /// Every original inline occurrence in encoded order, including repetitions.
    pub supertypes: Vec<Supertype<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Context(class_context::ErrorKind),
    Wire(wire::ErrorKind),
    Classifier(parameters::ErrorKind),
    UnsupportedClass,
    UnsupportedTableRole,
    UnsupportedType,
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
        Self { offset: e.offset, kind: ErrorKind::Wire(e.kind) }
    }
}
impl From<class_context::Error> for Error {
    fn from(e: class_context::Error) -> Self {
        Self { offset: e.offset, kind: ErrorKind::Context(e.kind) }
    }
}

/// Interpret ALL original field6/wire2 occurrences internally from the SAME
/// supported kind1/mv2.3.0 Outcome. Only finite bare nominal Types are admitted;
/// parameter ID/NAME classifiers, arguments, nonzero flags, flexible/outer/alias
/// shapes and significant wrong wires refuse atomically. Every Class field2
/// supertype-ID representation refuses, even empty, malformed or mixed with inline.
/// Nullable Options are encoded markings only. Unused tables/unknown extensions
/// stay bounded opaque raw data, not semantically validated or applied overlays.
/// One monotonic d1+d2/framing/schema/context/resolver/output allowance covers the
/// complete call, including repetitions. Earlier class/annotation/header APIs have
/// separate budgets. Limits bound logical work/requested storage, not CPU/RSS/OS.
/// Same-Outcome borrowing is custody, not authentication of caller-constructible
/// models, archive bytes or correspondence to classfile this_class. No target
/// loading, bound reconstruction, subtyping, legal inheritance or classpath proof.
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
    let mut supertypes = Vec::new();
    let mut pos = framed.payload_span.start;
    let mut ordinal = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let offset = field.span.start;
        budget.wire.work(1, offset)?;
        match (field.number, field.value) {
            (2, _) => return Err(Error { offset, kind: ErrorKind::UnsupportedTableRole }),
            (6, wire::Value::Bytes(body)) => {
                let start = field.span.end - body.len();
                // Fixed flat selected slices, not a recursive Type graph.
                budget.node(2, start)?;
                let header = type_header::decode_with_budget(body, &mut budget.wire)
                    .map_err(|e| Error { offset: start + e.offset, kind: ErrorKind::Wire(e.kind) })?;
                // Do not reinterpret Class declarations as Function-own NAME scope.
                if header.class_name.is_none()
                    || header.type_parameter.is_some()
                    || header.type_parameter_name.is_some()
                {
                    return Err(Error { offset: start, kind: ErrorKind::UnsupportedType });
                }
                let classifier = parameters::classifier(
                    &header, start, &context, &[], &mut resolver, &mut budget,
                ).map_err(|e| Error { offset: e.offset, kind: ErrorKind::Classifier(e.kind) })?;
                let Classifier::Class(class_id) = classifier else {
                    unreachable!("only nominal classifiers admitted above")
                };
                budget.push(&mut supertypes, Supertype {
                    ordinal,
                    span: field.span.clone(),
                    body_span: start..field.span.end,
                    header,
                    class_id,
                }, offset)?;
            }
            (6, _) => return Err(Error { offset, kind: ErrorKind::UnsupportedClass }),
            _ => {}
        }
        ordinal += 1;
    }
    Ok(ClassSupertypes {
        owner: context.owner,
        parameters: context.parameters,
        supertypes,
    })
}
