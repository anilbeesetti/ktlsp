//! JVMS 23 §§4.7.20–21 class type annotations, retained as encoded data.
//! Target indices and paths are not checked against supertypes or Signatures.

use crate::annotations::{self, Annotation, Budget, DeclarationAnnotations, Visibility};
use crate::type_annotations::{TypePathStep, vector};
use crate::{ClassFile, Error};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub enum ClassTypeTarget {
    TypeParameter {
        index: u8,
    },
    /// The raw 0xffff superclass sentinel is retained alongside interface indices.
    Supertype {
        index: u16,
    },
    TypeParameterBound {
        parameter_index: u8,
        bound_index: u8,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassTypeAnnotation<'a> {
    pub target: ClassTypeTarget,
    pub path: Vec<TypePathStep>,
    pub annotation: Annotation<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassTypeAnnotationAttribute<'a> {
    pub visibility: Visibility,
    pub annotations: Vec<ClassTypeAnnotation<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClassTypeAnnotations<'a> {
    pub declarations: DeclarationAnnotations<'a>,
    pub class: Vec<ClassTypeAnnotationAttribute<'a>>,
}

/// Decode declarations then class type attributes with one structural budget.
/// Recognized class type attributes require 52.0–67.0. Field/method type
/// attributes, Code and Record interiors remain opaque. Any error returns no
/// partial output and leaves the raw input available. New vector capacity is
/// charged; inherited annotation vectors retain their logical-node accounting.
/// Neither allocator overhead nor whole-operation owned bytes/RSS are bounded.
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<ClassTypeAnnotations<'a>> {
    let mut budget = Budget::default();
    let declarations = annotations::decode_with_budget(class, &mut budget)?;
    let class = decode_class(class, &mut budget)?;
    Ok(ClassTypeAnnotations {
        declarations,
        class,
    })
}

fn decode_class<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<Vec<ClassTypeAnnotationAttribute<'a>>> {
    let mut out = Vec::new();
    let mut seen = [false; 2];
    for attr in &class.attributes {
        // The declaration pass charged raw payloads/containers; this scan charges names.
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
            let target = match reader.u8()? {
                0x00 => {
                    budget.work(1)?;
                    ClassTypeTarget::TypeParameter {
                        index: reader.u8()?,
                    }
                }
                0x10 => {
                    budget.work(2)?;
                    ClassTypeTarget::Supertype {
                        index: reader.u16()?,
                    }
                }
                0x11 => {
                    budget.work(1)?;
                    let parameter_index = reader.u8()?;
                    budget.work(1)?;
                    ClassTypeTarget::TypeParameterBound {
                        parameter_index,
                        bound_index: reader.u8()?,
                    }
                }
                _ => return Err(Error::Malformed("class type annotation target")),
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
            annotations.push(ClassTypeAnnotation {
                target,
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
        out.push(ClassTypeAnnotationAttribute {
            visibility: if slot == 0 {
                Visibility::RuntimeVisible
            } else {
                Visibility::RuntimeInvisible
            },
            annotations,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Constant, MAX_RECORDS, MAX_VALIDATION_UNITS};

    fn class(bytes: &[u8]) -> ClassFile<'_> {
        ClassFile {
            minor: 0,
            major: 52,
            constants: vec![
                Constant::Unusable,
                Constant::Utf8("RuntimeVisibleTypeAnnotations".encode_utf16().collect()),
                Constant::Utf8("LA;".encode_utf16().collect()),
            ],
            access_flags: 0,
            this_class: 0,
            super_class: 0,
            interfaces: vec![],
            fields: vec![],
            methods: vec![],
            attributes: vec![Attribute { name: 1, bytes }],
        }
    }

    #[test]
    fn target_width_work_cuts_and_missing_path_precedence() {
        for (bytes, remaining, expected) in [
            (&[0, 1][..], 30, Error::Limit("annotation work")),
            (&[0, 1][..], 31, Error::Truncated),
            (&[0, 1, 0][..], 31, Error::Limit("annotation work")),
            (&[0, 1, 0][..], 32, Error::Truncated),
            (&[0, 1, 0, 255][..], 32, Error::Truncated),
            (&[0, 1, 0x10, 255][..], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x10, 255][..], 33, Error::Truncated),
            (&[0, 1, 0x10, 255, 255][..], 33, Error::Truncated),
            (&[0, 1, 0x11][..], 31, Error::Limit("annotation work")),
            (&[0, 1, 0x11][..], 32, Error::Truncated),
            (&[0, 1, 0x11, 255][..], 32, Error::Limit("annotation work")),
            (&[0, 1, 0x11, 255][..], 33, Error::Truncated),
            (&[0, 1, 0x11, 255, 255][..], 33, Error::Truncated),
            (&[0, 1, 0, 0, 1][..], 34, Error::Limit("annotation work")),
            (&[0, 1, 0, 0, 1][..], 35, Error::Truncated),
        ] {
            let c = class(bytes);
            let mut budget = Budget::default();
            budget.work(MAX_VALIDATION_UNITS - remaining).unwrap();
            assert_eq!(
                decode_class(&c, &mut budget),
                Err(expected),
                "{bytes:?}/{remaining}"
            );
        }
    }

    #[test]
    fn reservations_exhaustion_and_checked_counter_overflow() {
        for (bytes, remaining, expected) in [
            (&[0, 1][..], 0, Error::Limit("annotation nodes")),
            (&[0, 1, 0, 0, 1][..], 1, Error::Limit("annotation nodes")),
            (&[0, 0][..], 1, Error::Limit("annotation nodes")),
            (
                &[0, 0, 0][..],
                0,
                Error::Malformed("type annotation trailing bytes"),
            ),
        ] {
            let c = class(bytes);
            let mut budget = Budget::default();
            budget.nodes(MAX_RECORDS - remaining).unwrap();
            assert_eq!(decode_class(&c, &mut budget), Err(expected));
        }
        for (bytes, required) in [
            (&[0, 1, 0, 0, 0, 0, 2, 0, 0][..], 67),
            (&[0, 1, 0x10, 0, 0, 0, 0, 2, 0, 0][..], 68),
            (&[0, 1, 0x11, 0, 0, 0, 0, 2, 0, 0][..], 68),
        ] {
            for extra in [0, 1] {
                let c = class(bytes);
                let mut budget = Budget::default();
                budget
                    .work(MAX_VALIDATION_UNITS - required + extra)
                    .unwrap();
                let out = annotations::decode_with_budget(&c, &mut budget)
                    .and_then(|_| decode_class(&c, &mut budget));
                if extra == 0 {
                    assert!(out.is_ok());
                } else {
                    assert_eq!(out, Err(Error::Limit("annotation work")));
                }
            }
        }
        let mut budget = Budget::default();
        budget.nodes(MAX_RECORDS).unwrap();
        assert_eq!(budget.nodes(1), Err(Error::Limit("annotation nodes")));
        assert_eq!(
            budget.nodes(usize::MAX),
            Err(Error::Limit("annotation nodes"))
        );
        let mut budget = Budget::default();
        budget.work(MAX_VALIDATION_UNITS).unwrap();
        assert_eq!(budget.work(1), Err(Error::Limit("annotation work")));
        assert_eq!(
            budget.work(usize::MAX),
            Err(Error::Limit("annotation work"))
        );
    }
}
