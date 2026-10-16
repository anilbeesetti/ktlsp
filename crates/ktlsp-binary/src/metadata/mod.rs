//! Bounded Kotlin Metadata header/d1, raw wire, StringTableTypes and JVM names.
//! Includes finite Package alias and Class-owned parameter-reference consumers.

pub mod class_constructors;
pub mod class_context;
pub mod class_header;
pub mod class_id;
pub mod class_member_names;
pub mod class_supertypes;
pub mod class_type_parameters;
pub mod constructor;
pub mod constructor_parameters;
mod encoding;
pub mod function_header;
pub mod function_parameters;
pub mod function_type_parameters;
pub mod function_types;
mod header;
pub mod jvm_method_signature;
pub mod jvm_property_signature;
pub mod name_resolver;
pub mod package_member_names;
pub mod payload;
pub mod property_header;
pub mod property_types;
pub mod resolved_alias_signature;
pub mod resolved_class_function_return;
pub mod resolved_class_properties;
pub mod string_table;
pub mod type_alias;
pub mod type_alias_types;
pub mod type_argument;
pub mod type_argument_types;
pub mod type_arguments;
pub mod type_header;
pub mod type_parameter;
pub mod type_parameter_bounds;
pub mod type_table;
pub mod value_parameter;
pub mod value_parameter_types;
pub mod version_requirement;
pub mod wire;

pub use header::{Error, Header, Limits, Outcome, Unsupported, decode};
