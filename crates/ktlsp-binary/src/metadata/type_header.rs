//! Selected Type scalar data; nested messages and semantics remain opaque.
use super::wire::{self, Value};
use crate::MAX_BYTES;

pub use super::wire::{Error, ErrorKind, Limits};

#[derive(Debug, PartialEq, Eq)]
pub struct TypeHeader<'a> {
    /// Complete input, preserving nested duplicates, unknown fields and order.
    pub raw: &'a [u8],
    /// Absent integers have schema default zero; presence remains distinct.
    pub flags: Option<i32>,
    /// Absent has schema default false; no nullability interpretation is made.
    pub nullable: Option<bool>,
    pub flexible_type_capabilities_id: Option<i32>,
    pub class_name: Option<i32>,
    pub type_parameter: Option<i32>,
    pub flexible_upper_bound_id: Option<i32>,
    pub type_parameter_name: Option<i32>,
    pub outer_type_id: Option<i32>,
    pub type_alias_name: Option<i32>,
    pub abbreviated_type_id: Option<i32>,
}

/// Scan ONE unframed Type body with a separately bounded, allocation-free operation.
/// Caller independently selects supported version [2,3,0] and the message body.
/// Every scalar is optional; conflicting classifiers are accepted as raw data.
/// Recognized scalars last-win, wrong wires stay raw, nested bodies are not merged
/// or initialized. Errors are relative to this input, not the enclosing d1.
/// Success establishes neither type/name resolution nor Kotlin semantic validity.
/// shortcut: raw64 inherits wire's slow-path rule; revisit for JVM fast-path equivalence.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<TypeHeader<'_>, Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::Limit("bytes"),
        });
    }
    let mut out = TypeHeader {
        raw: bytes,
        flags: None,
        nullable: None,
        flexible_type_capabilities_id: None,
        class_name: None,
        type_parameter: None,
        flexible_upper_bound_id: None,
        type_parameter_name: None,
        outer_type_id: None,
        type_alias_name: None,
        abbreviated_type_id: None,
    };
    let mut budget = wire::Budget::new(limits.records, limits.work);
    let mut pos = 0;
    while pos < bytes.len() {
        let field = wire::field(bytes, &mut pos, bytes.len(), &mut budget)?;
        budget.work(1, field.span.start)?;
        match (field.number, field.value) {
            (1, Value::Varint(value)) => out.flags = Some(value as u32 as i32),
            (3, Value::Varint(value)) => out.nullable = Some(value != 0),
            (4, Value::Varint(value)) => {
                out.flexible_type_capabilities_id = Some(value as u32 as i32);
            }
            (6, Value::Varint(value)) => out.class_name = Some(value as u32 as i32),
            (7, Value::Varint(value)) => out.type_parameter = Some(value as u32 as i32),
            (8, Value::Varint(value)) => out.flexible_upper_bound_id = Some(value as u32 as i32),
            (9, Value::Varint(value)) => out.type_parameter_name = Some(value as u32 as i32),
            (11, Value::Varint(value)) => out.outer_type_id = Some(value as u32 as i32),
            (12, Value::Varint(value)) => out.type_alias_name = Some(value as u32 as i32),
            (14, Value::Varint(value)) => out.abbreviated_type_id = Some(value as u32 as i32),
            _ => {}
        }
    }
    Ok(out)
}
