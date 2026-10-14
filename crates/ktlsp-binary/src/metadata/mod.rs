//! Bounded Kotlin Metadata header/d1, raw wire, StringTableTypes and JVM names.
//! No declaration/type reconstruction or Kotlin semantic analysis.

pub mod class_header;
mod encoding;
pub mod function_header;
mod header;
pub mod name_resolver;
pub mod payload;
pub mod string_table;
pub mod type_alias;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
