//! Typed StringTableTypes framing with a borrowed, wholly uninterpreted payload.
//! Header/d1 decoding and name resolution remain separately budgeted operations.
use super::{string_table, wire};
use crate::MAX_BYTES;
use std::ops::Range;

pub use super::string_table::Limits;

#[derive(Debug, PartialEq, Eq)]
pub struct Payload<'a> {
    pub prefix_span: Range<usize>,
    pub prefix: &'a [u8],
    pub string_table_span: Range<usize>,
    /// Nested Record/Field spans remain relative to this table, not d1.
    pub string_table: string_table::StringTableTypes<'a>,
    pub payload_span: Range<usize>,
    pub payload: &'a [u8],
}

/// Compose already-decoded d1 framing and typed table under one shared budget.
/// Errors are d1-relative. The remaining class/package bytes are not validated,
/// scanned or work-charged; they count toward the total input byte limit only.
/// Header kind/version checks remain the caller's separate responsibility.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Payload<'_>, wire::Error> {
    if bytes.len() > limits.bytes.min(MAX_BYTES) {
        return Err(wire::Error {
            offset: 0,
            kind: wire::ErrorKind::Limit("bytes"),
        });
    }
    decode_with_budget(bytes, &mut string_table::Budget::new(limits))
}

pub(super) fn decode_with_budget<'a>(
    bytes: &'a [u8],
    budget: &mut string_table::Budget,
) -> Result<Payload<'a>, wire::Error> {
    let envelope = wire::envelope(bytes, &mut budget.wire)?;
    let string_table =
        string_table::decode_with_budget(&bytes[envelope.string_table_span.clone()], budget)
            .map_err(
                |error| match envelope.string_table_span.start.checked_add(error.offset) {
                    Some(offset) => wire::Error { offset, ..error },
                    None => wire::Error {
                        offset: bytes.len(),
                        kind: wire::ErrorKind::Limit("bytes"),
                    },
                },
            )?;
    Ok(Payload {
        prefix: &bytes[envelope.prefix_span.clone()],
        prefix_span: envelope.prefix_span,
        string_table,
        string_table_span: envelope.string_table_span,
        payload: &bytes[envelope.payload_span.clone()],
        payload_span: envelope.payload_span,
    })
}
