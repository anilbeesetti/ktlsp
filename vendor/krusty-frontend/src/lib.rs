//! Selected Krusty Kotlin frontend, FIR, semantic checking, and real Rust plugins.
//!
//! This source extraction retains common IR required by the plugin API, but does not
//! include compiler drivers, target backends, artifact emission, or toolchain execution.

pub mod assignable;
pub mod ast;
mod ast_validate;
mod callable_access;
pub mod compilation_target;
mod context_parameters;
pub mod contracts;
mod declaration_validation;
pub mod diag;
pub mod diagnostic_wording;
pub mod enclosing_declarations;
pub mod features;
pub mod fir;
pub mod frontend;
pub mod fx_hash;
mod inline_parameter_modifier;
pub mod integer_constant;
pub mod ir;
pub mod java_source;
pub mod jvm;
pub mod kotlin_version;
pub mod kt_string;
pub mod language_version;
pub mod lexer;
pub mod lexical_policy;
pub mod libraries;
pub mod lifting_provenance;
pub mod metadata;
pub mod module_symbols;
pub mod name_tree;
pub mod names;
pub mod parser;
pub mod plugins;
mod program_entry;
mod resolve;
pub mod source;
pub mod spelling;
pub mod symbol_resolver;
pub mod symbol_source;
pub mod synthetics;
pub mod token;
pub mod trace;
pub mod type_engine;
pub mod types;
pub(crate) mod value_classes;
mod wide_stack;

pub use wide_stack::{with_limits, LimitExceeded, Limits};
