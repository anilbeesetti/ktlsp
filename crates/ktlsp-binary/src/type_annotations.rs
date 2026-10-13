//! JVMS 23 §§4.7.20–21 field type annotations, retained as encoded data.
//! Class/method/Code/Record type attributes and path applicability remain opaque.

use crate::annotation_extensions::{self, AnnotationExtensions};
use crate::annotations::{self, Annotation, Budget, Visibility};
use crate::{ClassFile, Error};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub struct TypePathStep {
    pub kind: u8,
    pub argument_index: u8,
}

/// The field target is always 0x13; paths are not resolved against field types.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldTypeAnnotation<'a> {
    pub path: Vec<TypePathStep>,
    pub annotation: Annotation<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldTypeAnnotationAttribute<'a> {
    pub visibility: Visibility,
    pub annotations: Vec<FieldTypeAnnotation<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldTypeAnnotations<'a> {
    pub extensions: AnnotationExtensions<'a>,
    /// One entry per input field, including fields with no type attributes.
    pub fields: Vec<Vec<FieldTypeAnnotationAttribute<'a>>>,
}

/// Decode declarations, extensions, then field type annotations with one budget.
/// Recognized field attributes require 52.0–67.0. Field names/descriptors and
/// unrelated CP entries are unconsumed; this is not whole-class verification.
/// New vector capacity consumes nodes; inherited vectors keep their original
/// logical-node bounds. Neither allocator overhead nor RSS is bounded here.
/// Existing public declaration/extension decoders retain their own behavior.
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<FieldTypeAnnotations<'a>> {
    decode_with_budget(class, &mut Budget::default())
}

pub(crate) fn decode_with_budget<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<FieldTypeAnnotations<'a>> {
    let extensions = annotation_extensions::decode_with_budget(class, budget)?;
    let fields = decode_fields(class, budget)?;
    Ok(FieldTypeAnnotations { extensions, fields })
}

fn decode_fields<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<Vec<Vec<FieldTypeAnnotationAttribute<'a>>>> {
    let mut fields = vector(class.fields.len(), budget)?;
    for field in &class.fields {
        let mut out = Vec::new();
        let mut seen = [false; 2];
        for attr in &field.attributes {
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
                if reader.u8()? != 0x13 {
                    return Err(Error::Malformed("field type annotation target"));
                }
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
                annotations.push(FieldTypeAnnotation { path, annotation });
            }
            if reader.at != attr.bytes.len() {
                return Err(Error::Malformed("type annotation trailing bytes"));
            }
            if out.is_empty() {
                out = vector(2, budget)?;
            }
            out.push(FieldTypeAnnotationAttribute {
                visibility: if slot == 0 {
                    Visibility::RuntimeVisible
                } else {
                    Visibility::RuntimeInvisible
                },
                annotations,
            });
        }
        fields.push(out);
    }
    Ok(fields)
}

pub(crate) fn vector<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>> {
    budget.nodes(count)?;
    let mut out = Vec::new();
    if count != 0 {
        out.try_reserve_exact(count)
            .map_err(|_| Error::Limit("annotation allocation"))?;
        extra_capacity(count, out.capacity(), budget)?;
    }
    Ok(out)
}

fn extra_capacity(requested: usize, observed: usize, budget: &mut Budget) -> Result<()> {
    budget.nodes(
        observed
            .checked_sub(requested)
            .ok_or(Error::Limit("annotation allocation"))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_RECORDS;

    #[test]
    fn fixed_capacity_and_synthetic_overshoot_accounting() {
        let mut budget = Budget::default();
        assert!(vector::<TypePathStep>(0, &mut budget).unwrap().is_empty());
        budget.nodes(MAX_RECORDS).unwrap();
        assert_eq!(
            vector::<u8>(1, &mut budget),
            Err(Error::Limit("annotation nodes"))
        );

        let mut budget = Budget::default();
        let values = vector::<TypePathStep>(5, &mut budget).unwrap();
        budget.nodes(MAX_RECORDS - values.capacity()).unwrap();
        assert_eq!(budget.nodes(1), Err(Error::Limit("annotation nodes")));

        let mut budget = Budget::default();
        extra_capacity(3, 7, &mut budget).unwrap();
        budget.nodes(MAX_RECORDS - 4).unwrap();
        extra_capacity(7, 7, &mut budget).unwrap();
        assert_eq!(
            extra_capacity(7, 8, &mut budget),
            Err(Error::Limit("annotation nodes"))
        );
        assert_eq!(
            extra_capacity(8, 7, &mut Budget::default()),
            Err(Error::Limit("annotation allocation"))
        );
        assert_eq!(
            extra_capacity(0, usize::MAX, &mut Budget::default()),
            Err(Error::Limit("annotation nodes"))
        );
    }
}
