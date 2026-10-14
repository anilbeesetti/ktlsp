//! JVMS 23 §§4.7.20–21 method type annotations, retained as encoded data.
//! All six method targets are decoded without semantic index validation.

use crate::annotations::{self, Annotation, Budget, Visibility};
use crate::type_annotations::{self, FieldTypeAnnotations, TypePathStep, vector};
use crate::{ClassFile, Error};

type Result<T> = std::result::Result<T, Error>;

/// Encoded target_info, without descriptor, Signature or Exceptions validation.
#[derive(Debug, PartialEq, Eq)]
pub enum MethodTargetInfo {
    Empty,
    TypeParameter(u8),
    TypeParameterBound {
        type_parameter_index: u8,
        bound_index: u8,
    },
    FormalParameter(u8),
    Throws(u16),
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodTypeAnnotation<'a> {
    /// Encoded target_type: 0x01, 0x12, 0x14, 0x15, 0x16 or 0x17.
    pub target_type: u8,
    pub target_info: MethodTargetInfo,
    pub path: Vec<TypePathStep>,
    pub annotation: Annotation<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodTypeAnnotationAttribute<'a> {
    pub visibility: Visibility,
    pub annotations: Vec<MethodTypeAnnotation<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodTypeAnnotations<'a> {
    pub field_types: FieldTypeAnnotations<'a>,
    /// One entry per input method, including methods with no type attributes.
    pub methods: Vec<Vec<MethodTypeAnnotationAttribute<'a>>>,
}

/// Decode declarations, extensions, field types, then method types with one budget.
/// Recognized method type attributes require 52.0–67.0. Other target tags are
/// malformed in method context; indices and type-path applicability are unvalidated.
/// Any failure returns no partial output and leaves raw ClassFile bytes available.
/// Method names/descriptors are unconsumed here; earlier parameter/default extensions
/// can consume them. Void/constructor annotations and paths are encoded data only.
/// New vectors share the field capacity accounting, not an RSS/owned-byte bound.
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<MethodTypeAnnotations<'a>> {
    let mut budget = Budget::default();
    let field_types = type_annotations::decode_with_budget(class, &mut budget)?;
    let methods = decode_methods(class, &mut budget)?;
    Ok(MethodTypeAnnotations {
        field_types,
        methods,
    })
}

fn decode_methods<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<Vec<Vec<MethodTypeAnnotationAttribute<'a>>>> {
    let mut methods = vector(class.methods.len(), budget)?;
    for method in &class.methods {
        let mut out = Vec::new();
        let mut seen = [false; 2];
        for attr in &method.attributes {
            // Earlier passes charged raw payloads/containers; this scan charges names.
            let name = budget.text(&class.constants, attr.name)?;
            let slot = if crate::eq(name, "RuntimeVisibleTypeAnnotations") {
                0
            } else if crate::eq(name, "RuntimeInvisibleTypeAnnotations") {
                1
            } else {
                continue;
            };
            if class.major < 52 {
                return Err(Error::Malformed("type annotation version"));
            }
            if seen[slot] {
                return Err(Error::Malformed("duplicate type annotation attribute"));
            }
            seen[slot] = true;
            let mut reader = crate::Reader {
                bytes: attr.bytes,
                at: 0,
                records: 0,
            };
            let count = usize::from(reader.u16()?);
            let mut annotations = vector(count, budget)?;
            for _ in 0..count {
                budget.work(1)?;
                let target_type = reader.u8()?;
                let target_info = match target_type {
                    0x01 => {
                        budget.work(1)?;
                        MethodTargetInfo::TypeParameter(reader.u8()?)
                    }
                    0x12 => {
                        budget.work(1)?;
                        let type_parameter_index = reader.u8()?;
                        budget.work(1)?;
                        MethodTargetInfo::TypeParameterBound {
                            type_parameter_index,
                            bound_index: reader.u8()?,
                        }
                    }
                    0x14 | 0x15 => MethodTargetInfo::Empty,
                    0x16 => {
                        budget.work(1)?;
                        MethodTargetInfo::FormalParameter(reader.u8()?)
                    }
                    0x17 => {
                        budget.work(2)?;
                        MethodTargetInfo::Throws(reader.u16()?)
                    }
                    _ => return Err(Error::Malformed("method type annotation target")),
                };
                let count = usize::from(reader.u8()?);
                budget.work(1 + 2 * count)?;
                let mut path = vector(count, budget)?;
                for _ in 0..count {
                    let kind = reader.u8()?;
                    let argument_index = reader.u8()?;
                    if kind > 3 || kind < 3 && argument_index != 0 {
                        return Err(Error::Malformed("type annotation path"));
                    }
                    path.push(TypePathStep {
                        kind,
                        argument_index,
                    });
                }
                let annotation = annotations::annotation(&mut reader, &class.constants, budget, 1)?;
                annotations.push(MethodTypeAnnotation {
                    target_type,
                    target_info,
                    path,
                    annotation,
                });
            }
            if reader.at != attr.bytes.len() {
                return Err(Error::Malformed("type annotation trailing bytes"));
            }
            if out.is_empty() {
                out = vector(2, budget)?;
            }
            out.push(MethodTypeAnnotationAttribute {
                visibility: if slot == 0 {
                    Visibility::RuntimeVisible
                } else {
                    Visibility::RuntimeInvisible
                },
                annotations,
            });
        }
        methods.push(out);
    }
    Ok(methods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Constant, MAX_RECORDS, MAX_VALIDATION_UNITS, Member};

    #[test]
    fn reservation_work_and_exhaustion_precede_future_reads() {
        let mut class = ClassFile {
            major: 52,
            minor: 0,
            constants: vec![
                Constant::Unusable,
                Constant::Utf8("RuntimeVisibleTypeAnnotations".encode_utf16().collect()),
            ],
            access_flags: 0,
            this_class: 0,
            super_class: 0,
            interfaces: vec![],
            fields: vec![],
            methods: vec![Member {
                access_flags: 0,
                name: 0,
                descriptor: 0,
                attributes: vec![Attribute {
                    name: 1,
                    bytes: &[0, 0, 0],
                }],
            }],
            attributes: vec![],
        };
        let mut budget = Budget::default();
        budget.nodes(MAX_RECORDS - 1).unwrap();
        // Outer capacity overshoot can exhaust nodes before the trailing-byte check.
        assert!(matches!(
            decode_methods(&class, &mut budget),
            Err(Error::Malformed("type annotation trailing bytes"))
                | Err(Error::Limit("annotation nodes"))
        ));
        class.methods[0].attributes[0].bytes = &[0, 0];
        let mut budget = Budget::default();
        budget.nodes(MAX_RECORDS - 1).unwrap();
        assert_eq!(
            decode_methods(&class, &mut budget),
            Err(Error::Limit("annotation nodes"))
        );
        class.methods[0].attributes[0].bytes = &[0, 1];
        let mut budget = Budget::default();
        budget.work(MAX_VALIDATION_UNITS - 30).unwrap();
        assert_eq!(
            decode_methods(&class, &mut budget),
            Err(Error::Limit("annotation work"))
        );
        let mut budget = Budget::default();
        budget.nodes(MAX_RECORDS - 1).unwrap();
        assert_eq!(
            decode_methods(&class, &mut budget),
            Err(Error::Limit("annotation nodes"))
        );
        class.methods[0].attributes[0].bytes = &[0, 1, 0x14, 1];
        let mut budget = Budget::default();
        budget.nodes(MAX_RECORDS - 2).unwrap();
        assert_eq!(
            decode_methods(&class, &mut budget),
            Err(Error::Limit("annotation nodes"))
        );
    }

    #[test]
    fn fixed_target_info_work_precedes_reads_but_not_missing_path_length() {
        let mut class = ClassFile {
            major: 52,
            minor: 0,
            constants: vec![
                Constant::Unusable,
                Constant::Utf8("RuntimeVisibleTypeAnnotations".encode_utf16().collect()),
            ],
            access_flags: 0,
            this_class: 0,
            super_class: 0,
            interfaces: vec![],
            fields: vec![],
            methods: vec![Member {
                access_flags: 0,
                name: 0,
                descriptor: 0,
                attributes: vec![Attribute {
                    name: 1,
                    bytes: &[0, 1, 0x16],
                }],
            }],
            attributes: vec![],
        };
        for (remaining, expected) in [
            (31, Error::Limit("annotation work")),
            (32, Error::Truncated),
        ] {
            let mut budget = Budget::default();
            budget.work(MAX_VALIDATION_UNITS - remaining).unwrap();
            assert_eq!(decode_methods(&class, &mut budget), Err(expected));
        }
        class.methods[0].attributes[0].bytes = &[0, 1, 0x16, 255];
        let mut budget = Budget::default();
        budget.work(MAX_VALIDATION_UNITS - 32).unwrap();
        assert_eq!(decode_methods(&class, &mut budget), Err(Error::Truncated));
        class.methods[0].attributes[0].bytes = &[0, 1, 0x16, 255, 0];
        let mut budget = Budget::default();
        budget.work(MAX_VALIDATION_UNITS - 32).unwrap();
        assert_eq!(
            decode_methods(&class, &mut budget),
            Err(Error::Limit("annotation work"))
        );
        for (bytes, remaining, expected) in [
            (&[0, 1, 0x01][..], 31, Error::Limit("annotation work")),
            (&[0, 1, 0x01], 32, Error::Truncated),
            (&[0, 1, 0x01, 255], 32, Error::Truncated),
            (&[0, 1, 0x01, 255, 0], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x12], 31, Error::Limit("annotation work")),
            (&[0, 1, 0x12], 32, Error::Truncated),
            (&[0, 1, 0x12, 255], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x12, 255], 33, Error::Truncated),
            (&[0, 1, 0x12, 255, 255], 33, Error::Truncated),
            (
                &[0, 1, 0x12, 255, 255, 0],
                33,
                Error::Limit("annotation work"),
            ),
            (&[0, 1, 0x15], 31, Error::Truncated),
            (&[0, 1, 0x15, 0], 31, Error::Limit("annotation work")),
            (&[0, 1, 0x17], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x17], 33, Error::Truncated),
            (&[0, 1, 0x17, 255], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x17, 255], 33, Error::Truncated),
            (&[0, 1, 0x17, 255, 255], 33, Error::Truncated),
            (
                &[0, 1, 0x17, 255, 255, 0],
                33,
                Error::Limit("annotation work"),
            ),
        ] {
            class.methods[0].attributes[0].bytes = bytes;
            let mut budget = Budget::default();
            budget.work(MAX_VALIDATION_UNITS - remaining).unwrap();
            assert_eq!(decode_methods(&class, &mut budget), Err(expected));
        }
    }
}
