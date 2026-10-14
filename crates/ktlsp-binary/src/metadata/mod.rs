//! Bounded Kotlin Metadata header/d1, raw wire, StringTableTypes and JVM names.
//! No declaration/type reconstruction or Kotlin semantic analysis.

pub mod class_header;
pub mod class_member_names;
pub mod constructor;
mod encoding;
pub mod function_header;
mod header;
pub mod jvm_method_signature;
pub mod jvm_property_signature;
pub mod name_resolver;
pub mod package_member_names;
pub mod payload;
pub mod property_header;
pub mod property_types;
pub mod string_table;
pub mod type_alias;
pub mod type_argument;
pub mod type_arguments;
pub mod type_header;
pub mod type_parameter;
pub mod type_table;
pub mod value_parameter;
pub mod value_parameter_types;
pub mod version_requirement;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
