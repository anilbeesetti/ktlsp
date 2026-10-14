//! One-level Class/Constructor data; ValueParameter bodies remain opaque.
use super::{class_header, constructor, string_table, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::constructor::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct ConstructorOccurrence<'a> {
    /// Body-only offsets relative to the supplied Class bytes.
    pub body_span: Range<usize>,
    pub constructor: constructor::Constructor<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassConstructors<'a> {
    pub header: class_header::ClassHeader<'a>,
    /// Encoded order; repeated messages remain separate and unmerged.
    pub constructors: Vec<ConstructorOccurrence<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Wire(wire::ErrorKind),
    Class(class_header::ErrorKind),
    Constructor(constructor::ErrorKind),
}

/// Offsets are relative to the supplied unframed Class bytes.
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

/// Decode ONE caller-selected unframed Class for kind1/mv=[2,3,0].
/// Complete Class framing/fq_name validation precedes selected field8/wire2
/// Constructors. Parent, selection scan, children and every retained vector share
/// one records/work/requested-capacity budget. Other legal fields remain opaque.
/// Child parameters and their spans remain Constructor-relative and opaque.
/// Header/d1/payload operations are outside this call. Success establishes no
/// full initialization, names/types, constructor symbols, overloads or semantics;
/// requested capacity excludes allocator surplus, churn, stack and RSS.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<ClassConstructors<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        }
        .into());
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    let header =
        class_header::decode_with_budget(bytes, &mut budget.wire).map_err(|error| Error {
            offset: error.offset,
            kind: ErrorKind::Class(error.kind),
        })?;
    let mut constructors = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if let (8, wire::Value::Bytes(body)) = (field.number, field.value) {
            let body_start = field.span.end - body.len();
            let constructor =
                constructor::decode_with_budget(body, &mut budget).map_err(|error| Error {
                    offset: body_start + error.offset,
                    kind: ErrorKind::Constructor(error.kind),
                })?;
            budget.push(
                &mut constructors,
                ConstructorOccurrence {
                    body_span: body_start..field.span.end,
                    constructor,
                },
                field.span.start,
            )?;
        }
    }
    Ok(ClassConstructors {
        header,
        constructors,
    })
}
