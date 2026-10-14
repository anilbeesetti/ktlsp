//! Bounded Kotlin Metadata header/d1, raw wire, StringTableTypes and JVM names.
//! No declaration/type reconstruction or Kotlin semantic analysis.

pub mod class_header;
pub mod class_member_names;
mod encoding;
pub mod function_header;
mod header;
pub mod name_resolver;
pub mod payload;
pub mod property_header;
pub mod string_table;
pub mod type_alias;
pub mod type_argument;
pub mod type_header;
pub mod type_parameter;
pub mod type_table;
pub mod value_parameter;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
