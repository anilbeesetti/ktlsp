//! Bounded Kotlin Metadata header/d1 and flat raw wire framing.
//! No schema, name, type or semantic interpretation.

mod encoding;
mod header;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
