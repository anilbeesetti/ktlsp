//! JVMS 23 §§4.7.18–19, 4.7.22 parameter annotations and defaults, as data.
//! Code/Record interiors and type annotations remain opaque; no external linkage.

use crate::annotations::{self, Annotation, Budget, DeclarationAnnotations, Value, Visibility};
use crate::{Attribute, ClassFile, Error, Member};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub struct ParameterAnnotationAttribute<'a> {
    pub visibility: Visibility,
    /// Encoded positions only; no guaranteed method-descriptor alignment.
    pub parameters: Vec<Vec<Annotation<'a>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodAnnotationExtensions<'a> {
    pub parameters: Vec<ParameterAnnotationAttribute<'a>>,
    pub default: Option<Value<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnotationExtensions<'a> {
    pub declarations: DeclarationAnnotations<'a>,
    /// One entry per input method, including methods with no extensions.
    pub methods: Vec<MethodAnnotationExtensions<'a>>,
}

/// One shared declaration/extension byte/node/work/depth budget (see #115).
/// Raw attributes remain intact; recognized extensions require 49.0 through 69.0.
/// Only consumed references/local default shapes are validated, not JVM semantics.
/// The additional scan can exhaust work on inputs accepted by declaration-only decode.
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<AnnotationExtensions<'a>> {
    decode_with_budget(class, &mut Budget::default())
}

pub(crate) fn decode_with_budget<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<AnnotationExtensions<'a>> {
    let declarations = annotations::decode_with_budget(class, budget)?;
    attributes(class, &class.attributes, None, budget)?;
    for field in &class.fields {
        attributes(class, &field.attributes, None, budget)?;
    }
    let mut methods = Vec::new();
    for method in &class.methods {
        methods.push(attributes(class, &method.attributes, Some(method), budget)?);
    }
    Ok(AnnotationExtensions {
        declarations,
        methods,
    })
}

fn attributes<'a>(
    class: &'a ClassFile<'_>,
    attrs: &[Attribute<'_>],
    method: Option<&Member<'_>>,
    budget: &mut Budget,
) -> Result<MethodAnnotationExtensions<'a>> {
    let mut out = MethodAnnotationExtensions {
        parameters: Vec::new(),
        default: None,
    };
    let mut seen = [false; 3];
    let mut validated = None;
    for attr in attrs {
        // The declaration pass already charged every container and payload once.
        let name = budget.text(&class.constants, attr.name)?;
        let slot = if crate::eq(name, "RuntimeVisibleParameterAnnotations") {
            0
        } else if crate::eq(name, "RuntimeInvisibleParameterAnnotations") {
            1
        } else if crate::eq(name, "AnnotationDefault") {
            2
        } else {
            continue;
        };
        if class.major < 49 {
            return Err(Error::Malformed("annotation extension version"));
        }
        let method = method.ok_or(Error::Malformed("annotation extension context"))?;
        if seen[slot] {
            return Err(Error::Malformed("duplicate annotation extension"));
        }
        seen[slot] = true;
        let (name, descriptor) = match validated {
            Some(pair) => pair,
            None => {
                let name = budget.text(&class.constants, method.name)?;
                let descriptor = budget.text(&class.constants, method.descriptor)?;
                crate::name_valid(name, true)?;
                crate::descriptor_valid(
                    descriptor,
                    true,
                    method.access_flags & 8 == 0 && !crate::eq(name, "<clinit>"),
                )?;
                if crate::eq(name, "<init>") && descriptor.last() != Some(&86)
                    || crate::eq(name, "<clinit>")
                        && (descriptor.last() != Some(&86)
                            || class.major >= 51
                                && (!crate::eq(descriptor, "()V") || method.access_flags & 8 == 0))
                {
                    return Err(Error::Malformed("special method"));
                }
                validated = Some((name, descriptor));
                (name, descriptor)
            }
        };
        let mut reader = crate::Reader {
            bytes: attr.bytes,
            at: 0,
            records: 0,
        };
        if slot == 2 {
            if class.access_flags & 0x2200 != 0x2200
                || method.access_flags & 0x0401 != 0x0401
                || method.access_flags & 0x013e != 0
                || class.major <= 60 && method.access_flags & 0x0800 != 0
                || name.iter().any(|c| [60, 62].contains(c))
                || !descriptor.starts_with(&[40, 41])
                || descriptor == [40, 41, 86]
            {
                return Err(Error::Malformed("annotation default method"));
            }
            budget.nodes(1)?;
            let value = annotations::value(&mut reader, &class.constants, budget, 1)?;
            if !shape(&value, &descriptor[2..], budget)? {
                return Err(Error::Malformed("annotation default value"));
            }
            out.default = Some(value);
        } else {
            let count = usize::from(reader.u8()?);
            budget.nodes(count)?;
            let mut parameters = Vec::new();
            for _ in 0..count {
                let count = usize::from(reader.u16()?);
                budget.nodes(count)?;
                let mut annotations = Vec::new();
                for _ in 0..count {
                    annotations.push(annotations::annotation(
                        &mut reader,
                        &class.constants,
                        budget,
                        1,
                    )?);
                }
                parameters.push(annotations);
            }
            out.parameters.push(ParameterAnnotationAttribute {
                visibility: if slot == 0 {
                    Visibility::RuntimeVisible
                } else {
                    Visibility::RuntimeInvisible
                },
                parameters,
            });
        }
        if reader.at != attr.bytes.len() {
            return Err(Error::Malformed("annotation extension trailing bytes"));
        }
    }
    Ok(out)
}

fn shape(value: &Value<'_>, descriptor: &[u16], budget: &mut Budget) -> Result<bool> {
    budget.work(
        descriptor
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Limit("annotation work"))?,
    )?;
    if let Some(component) = descriptor.strip_prefix(&[91]) {
        if component.first() == Some(&91) {
            return Ok(false);
        }
        let Value::Array(values) = value else {
            return Ok(false);
        };
        for value in values {
            if !shape(value, component, budget)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if crate::eq(descriptor, "Ljava/lang/String;") {
        return Ok(matches!(value, Value::String(_)));
    }
    if crate::eq(descriptor, "Ljava/lang/Class;") {
        return Ok(matches!(value, Value::Class(_)));
    }
    if descriptor.first() == Some(&76) {
        let candidate = match value {
            Value::Enum { descriptor, .. } => *descriptor,
            Value::Annotation(annotation) => annotation.descriptor,
            _ => return Ok(false),
        };
        budget.work(
            descriptor
                .len()
                .min(candidate.len())
                .checked_add(1)
                .ok_or(Error::Limit("annotation work"))?,
        )?;
        return Ok(descriptor == candidate);
    }
    Ok(matches!(
        (descriptor, value),
        ([66], Value::Byte(_))
            | ([67], Value::Char(_))
            | ([68], Value::Double(_))
            | ([70], Value::Float(_))
            | ([73], Value::Int(_))
            | ([74], Value::Long(_))
            | ([83], Value::Short(_))
            | ([90], Value::Boolean(_))
    ))
}
