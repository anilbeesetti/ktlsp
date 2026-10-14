//! Selected Property extension100 data, without omitted-signature inference.
use super::{
    string_table,
    wire::{self, Value},
};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

pub use super::{
    jvm_method_signature::{JvmMethodSignature, SignatureBody},
    wire::{Error, ErrorKind},
};

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
pub struct JvmFieldSignature {
    pub name: Option<i32>,
    pub desc: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct JvmPropertySignature {
    pub field: Option<JvmFieldSignature>,
    pub synthetic_method: Option<JvmMethodSignature>,
    pub getter: Option<JvmMethodSignature>,
    pub setter: Option<JvmMethodSignature>,
    pub delegate_method: Option<JvmMethodSignature>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Field,
    SyntheticMethod,
    Getter,
    Setter,
    DelegateMethod,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MemberSignatureBody<'a> {
    pub kind: Kind,
    /// Index in PropertySignatureExtension::bodies, including empty occurrences.
    pub outer_ordinal: usize,
    /// Original body-only Property-relative slice/span, never merged bytes.
    pub body: SignatureBody<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PropertySignatureExtension<'a> {
    pub raw: &'a [u8],
    /// Recursive singular-message merge; empty messages retain presence.
    pub signature: Option<JvmPropertySignature>,
    pub bodies: Vec<SignatureBody<'a>>,
    /// Outer-then-child wire order, including duplicate/empty selected leaves.
    pub member_bodies: Vec<MemberSignatureBody<'a>>,
}

/// Scan ONE caller-selected unframed Property and its JVM extension100.
/// Caller independently selects supported mv=[2,3,0], kind/container and parent.
/// Parent initialization is not enforced; missing indices remain missing.
/// Wrong-wire fields stay raw and never clear prior recognized scalar presence.
/// One aggregate budget covers all three scan levels and both retained vectors.
/// No name resolution, descriptor inference, types/linkage/accessor semantics.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<PropertySignatureExtension<'_>, Error> {
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
    let mut out = PropertySignatureExtension {
        raw: bytes,
        signature: None,
        bodies: Vec::new(),
        member_bodies: Vec::new(),
    };
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        let start = field.span.start;
        budget.wire.work(1, start)?;
        if let (100, Value::Bytes(raw)) = (field.number, field.value) {
            let span = field.span.end - raw.len()..field.span.end;
            let outer_ordinal = out.bodies.len();
            let signature = out.signature.get_or_insert(JvmPropertySignature {
                field: None,
                synthetic_method: None,
                getter: None,
                setter: None,
                delegate_method: None,
            });
            let mut child_pos = span.start;
            while child_pos < span.end {
                let child = wire::field(bytes, &mut child_pos, span.end, &mut budget.wire)?;
                let child_start = child.span.start;
                budget.wire.work(1, child_start)?;
                let Value::Bytes(child_raw) = child.value else {
                    continue;
                };
                let (kind, name, desc) = if child.number == 1 {
                    let slot = signature.field.get_or_insert(JvmFieldSignature {
                        name: None,
                        desc: None,
                    });
                    (Kind::Field, &mut slot.name, &mut slot.desc)
                } else {
                    let (kind, slot) = match child.number {
                        2 => (Kind::SyntheticMethod, &mut signature.synthetic_method),
                        3 => (Kind::Getter, &mut signature.getter),
                        4 => (Kind::Setter, &mut signature.setter),
                        5 => (Kind::DelegateMethod, &mut signature.delegate_method),
                        _ => continue,
                    };
                    let slot = slot.get_or_insert(JvmMethodSignature {
                        name: None,
                        desc: None,
                    });
                    (kind, &mut slot.name, &mut slot.desc)
                };
                let child_span = child.span.end - child_raw.len()..child.span.end;
                let mut leaf_pos = child_span.start;
                while leaf_pos < child_span.end {
                    let leaf = wire::field(bytes, &mut leaf_pos, child_span.end, &mut budget.wire)?;
                    budget.wire.work(1, leaf.span.start)?;
                    match (leaf.number, leaf.value) {
                        (1, Value::Varint(value)) => *name = Some(value as u32 as i32),
                        (2, Value::Varint(value)) => *desc = Some(value as u32 as i32),
                        _ => {}
                    }
                }
                budget.push(
                    &mut out.member_bodies,
                    MemberSignatureBody {
                        kind,
                        outer_ordinal,
                        body: SignatureBody {
                            raw: child_raw,
                            span: child_span,
                        },
                    },
                    child_start,
                )?;
            }
            budget.push(&mut out.bodies, SignatureBody { raw, span }, start)?;
        }
    }
    Ok(out)
}
