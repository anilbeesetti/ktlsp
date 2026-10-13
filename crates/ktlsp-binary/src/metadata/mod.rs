//! Bounded Kotlin Metadata header/d1, flat raw wire and StringTableTypes data.
//! No name, type or semantic interpretation.

mod encoding;
mod header;
pub mod string_table;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
