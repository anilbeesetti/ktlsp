//! Selected Constructor/Function extension100 data; no inference or semantics.
use super::{
    string_table,
    wire::{self, Value},
};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};
use std::ops::Range;

pub use super::wire::{Error, ErrorKind};

/// Callers can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Logical requested vector capacity, excluding allocator surplus/churn/RSS.
    pub owned_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            records: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
            owned_bytes: MAX_BYTES,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SignatureBody<'a> {
    pub raw: &'a [u8],
    /// Body-only offsets relative to the supplied parent bytes.
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct JvmMethodSignature {
    /// Absent numeric default 0 does not imply index 0 or a name lookup.
    pub name: Option<i32>,
    /// No descriptor is inferred when this field is absent.
    pub desc: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodSignatureExtension<'a> {
    /// Complete parent, preserving unknown/wrong-wire fields and duplicate order.
    pub raw: &'a [u8],
    /// Singular-message merge across all recognized occurrences; empty is present.
    pub signature: Option<JvmMethodSignature>,
    pub bodies: Vec<SignatureBody<'a>>,
}

/// Scan ONE caller-selected unframed Constructor or Function and extension100.
/// Caller independently selects supported mv=[2,3,0], kind/container and parent.
/// Parent initialization (including required Function.name) is not enforced.
/// Selected body fields1/2 are signed low32 indices; wrong wires never clear them.
/// One aggregate records/work/owned budget covers parent and every selected body.
/// No name lookup, omitted-value inference, descriptor/type/linkage validation.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<MethodSignatureExtension<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes,
        records: limits.records,
        work: limits.work,
        owned_bytes: limits.owned_bytes,
        ..Default::default()
    });
    let (signature, bodies) = scan_with_budget(bytes, &mut budget, |raw, span| {
        SignatureBody { raw, span }
    })?;
    Ok(MethodSignatureExtension { raw: bytes, signature, bodies })
}

// Share the scanner without retaining self-borrows in the owning method consumer.
pub(super) fn scan_with_budget<'a, T>(
    bytes: &'a [u8],
    budget: &mut string_table::Budget,
    body: impl Fn(&'a [u8], Range<usize>) -> T,
) -> Result<(Option<JvmMethodSignature>, Vec<T>), Error> {
    let mut signature = None;
    let mut bodies = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (100, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            let signature = signature.get_or_insert(JvmMethodSignature {
                name: None,
                desc: None,
            });
            let mut child_pos = span.start;
            while child_pos < span.end {
                let child = wire::field(bytes, &mut child_pos, span.end, &mut budget.wire)?;
                budget.wire.work(1, child.span.start)?;
                match (child.number, child.value) {
                    (1, Value::Varint(value)) => signature.name = Some(value as u32 as i32),
                    (2, Value::Varint(value)) => signature.desc = Some(value as u32 as i32),
                    _ => {}
                }
            }
            budget.push(&mut bodies, body(raw, span), start)?;
        }
    }
    Ok((signature, bodies))
}
