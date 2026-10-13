//! Bounded Kotlin Metadata header/d1 data decoding; no protobuf or semantics.

mod encoding;
mod header;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
