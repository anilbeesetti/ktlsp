//! JVMS 23 §4.7.16–17 declaration annotations, decoded as data, without linkage.
//!
//! `decode` borrows exact UTF-16 code units from the constant pool. Raw attributes
//! remain unchanged. Fields/methods have one output entry per input member, even
//! when empty. Parameter/type annotations and defaults remain #71; record
//! components and Code interiors are opaque. No semantics or JVM verification.

use crate::{Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

/// Maximum combined annotation/element-value nesting, including the root annotation.
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub enum Visibility {
    RuntimeVisible,
    RuntimeInvisible,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnotationAttribute<'a> {
    pub visibility: Visibility,
    pub annotations: Vec<Annotation<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DeclarationAnnotations<'a> {
    pub class: Vec<AnnotationAttribute<'a>>,
    pub fields: Vec<Vec<AnnotationAttribute<'a>>>,
    pub methods: Vec<Vec<AnnotationAttribute<'a>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Annotation<'a> {
    pub descriptor: &'a [u16],
    pub elements: Vec<Element<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Element<'a> {
    pub name: &'a [u16],
    pub value: Value<'a>,
}

/// Numeric payloads retain CP bits, even for narrow integer tags and NaNs.
/// Strings reference CONSTANT_Utf8 directly, never CONSTANT_String.
#[derive(Debug, PartialEq, Eq)]
pub enum Value<'a> {
    Byte(u32),
    Char(u32),
    Double(u64),
    Float(u32),
    Int(u32),
    Long(u64),
    Short(u32),
    Boolean(u32),
    String(&'a [u16]),
    Enum {
        descriptor: &'a [u16],
        name: &'a [u16],
    },
    Class(&'a [u16]),
    Annotation(Annotation<'a>),
    Array(Vec<Value<'a>>),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Default)]
pub(crate) struct Budget {
    bytes: usize,
    nodes: usize,
    work: usize,
}
fn charge(used: &mut usize, n: usize, max: usize, label: &'static str) -> Result<()> {
    *used = used.checked_add(n).ok_or(Error::Limit(label))?;
    if *used > max {
        return Err(Error::Limit(label));
    }
    Ok(())
}
impl Budget {
    pub(crate) fn nodes(&mut self, n: usize) -> Result<()> {
        charge(&mut self.nodes, n, MAX_RECORDS, "annotation nodes")
    }
    pub(crate) fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.work, n, MAX_VALIDATION_UNITS, "annotation work")
    }
    pub(crate) fn text<'a>(&mut self, cp: &'a [Constant], index: u16) -> Result<&'a [u16]> {
        self.work(1)?;
        if index == 0 {
            return Err(Error::Malformed("annotation constant index"));
        }
        let text = crate::utf16(cp, index)?;
        self.work(text.len())?;
        Ok(text)
    }
}

/// One aggregate budget per call: 16 MiB of all visited attribute payloads;
/// 65,535 member/attribute/annotation/pair/value nodes; 64 Mi work units for CP
/// references, UTF-16 validation and duplicate-name comparisons. Counts are
/// charged before loops/allocation, and nesting before recursive descent.
///
/// Public/forged inputs are checked at consumed references (including zero),
/// containers and version; unrelated CP entries/class structure are not verified.
/// Supported versions match `parse_class` (through 69.0); annotations need 49.0.
pub fn decode<'a>(class: &'a ClassFile<'_>) -> Result<DeclarationAnnotations<'a>> {
    decode_with_budget(class, &mut Budget::default())
}

pub(crate) fn decode_with_budget<'a>(
    class: &'a ClassFile<'_>,
    budget: &mut Budget,
) -> Result<DeclarationAnnotations<'a>> {
    if !crate::supported_class_version(class.major, class.minor)
    {
        return Err(Error::UnsupportedVersion {
            major: class.major,
            minor: class.minor,
        });
    }
    if class.constants.is_empty() || class.constants.len() > MAX_RECORDS {
        return Err(Error::Malformed("annotation constant pool size"));
    }
    budget.nodes(class.fields.len())?;
    budget.nodes(class.methods.len())?;
    let class_annotations = attributes(class, &class.attributes, budget)?;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for member in &class.fields {
        fields.push(attributes(class, &member.attributes, budget)?);
    }
    for member in &class.methods {
        methods.push(attributes(class, &member.attributes, budget)?);
    }
    Ok(DeclarationAnnotations {
        class: class_annotations,
        fields,
        methods,
    })
}

fn attributes<'a>(
    class: &'a ClassFile<'_>,
    attrs: &[Attribute<'_>],
    budget: &mut Budget,
) -> Result<Vec<AnnotationAttribute<'a>>> {
    budget.nodes(attrs.len())?;
    let mut seen = [false; 2];
    let mut out = Vec::new();
    for attr in attrs {
        charge(
            &mut budget.bytes,
            attr.bytes.len(),
            MAX_BYTES,
            "annotation bytes",
        )?;
        let name = budget.text(&class.constants, attr.name)?;
        let (slot, visibility) = if crate::eq(name, "RuntimeVisibleAnnotations") {
            (0, Visibility::RuntimeVisible)
        } else if crate::eq(name, "RuntimeInvisibleAnnotations") {
            (1, Visibility::RuntimeInvisible)
        } else {
            continue;
        };
        if class.major < 49 {
            return Err(Error::Malformed("annotation attribute version"));
        }
        if seen[slot] {
            return Err(Error::Malformed("duplicate annotation attribute"));
        }
        seen[slot] = true;
        let mut r = crate::Reader {
            bytes: attr.bytes,
            at: 0,
            records: 0,
        };
        let count = usize::from(r.u16()?);
        budget.nodes(count)?;
        let mut annotations = Vec::new();
        for _ in 0..count {
            annotations.push(annotation(&mut r, &class.constants, budget, 1)?);
        }
        if r.at != attr.bytes.len() {
            return Err(Error::Malformed("annotation trailing bytes"));
        }
        out.push(AnnotationAttribute {
            visibility,
            annotations,
        });
    }
    Ok(out)
}

fn depth_check(depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::Limit("annotation depth"));
    }
    Ok(())
}
fn object_descriptor(s: &[u16]) -> Result<()> {
    if s.first() != Some(&76) {
        return Err(Error::Malformed("annotation object descriptor"));
    }
    crate::descriptor_valid(s, false, false)
}
fn element_name(s: &[u16]) -> Result<()> {
    crate::name_valid(s, false)?;
    if s.iter().any(|c| [60, 62].contains(c)) {
        return Err(Error::Malformed("annotation element name"));
    }
    Ok(())
}
pub(crate) fn annotation<'a>(
    r: &mut crate::Reader<'_>,
    cp: &'a [Constant],
    budget: &mut Budget,
    depth: usize,
) -> Result<Annotation<'a>> {
    depth_check(depth)?;
    let descriptor = budget.text(cp, r.u16()?)?;
    object_descriptor(descriptor)?;
    let count = usize::from(r.u16()?);
    budget.nodes(count)?;
    let mut elements: Vec<Element<'a>> = Vec::new();
    for _ in 0..count {
        let name = budget.text(cp, r.u16()?)?;
        element_name(name)?;
        for previous in &elements {
            // Charge the full possible comparison before inspecting shared prefixes.
            budget.work(name.len().min(previous.name.len()) + 1)?;
            if name == previous.name {
                return Err(Error::Malformed("duplicate annotation element"));
            }
        }
        budget.nodes(1)?;
        let value = value(r, cp, budget, depth + 1)?;
        elements.push(Element { name, value });
    }
    Ok(Annotation {
        descriptor,
        elements,
    })
}
pub(crate) fn value<'a>(
    r: &mut crate::Reader<'_>,
    cp: &'a [Constant],
    budget: &mut Budget,
    depth: usize,
) -> Result<Value<'a>> {
    depth_check(depth)?;
    let tag = r.u8()?;
    Ok(match tag {
        b's' => Value::String(budget.text(cp, r.u16()?)?),
        b'e' => {
            let descriptor = budget.text(cp, r.u16()?)?;
            object_descriptor(descriptor)?;
            let name = budget.text(cp, r.u16()?)?;
            crate::name_valid(name, false)?;
            Value::Enum { descriptor, name }
        }
        b'c' => {
            let s = budget.text(cp, r.u16()?)?;
            if !crate::eq(s, "V") {
                crate::descriptor_valid(s, false, false)?;
            }
            Value::Class(s)
        }
        b'@' => {
            budget.nodes(1)?;
            Value::Annotation(annotation(r, cp, budget, depth + 1)?)
        }
        b'[' => {
            let count = usize::from(r.u16()?);
            budget.nodes(count)?;
            let mut values = Vec::new();
            for _ in 0..count {
                values.push(value(r, cp, budget, depth + 1)?);
            }
            Value::Array(values)
        }
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => {
            let index = r.u16()?;
            budget.work(1)?;
            if index == 0 {
                return Err(Error::Malformed("annotation constant index"));
            }
            match (tag, crate::constant(cp, index)?) {
                (b'B', Constant::Integer(n)) => Value::Byte(*n),
                (b'C', Constant::Integer(n)) => Value::Char(*n),
                (b'D', Constant::Double(n)) => Value::Double(*n),
                (b'F', Constant::Float(n)) => Value::Float(*n),
                (b'I', Constant::Integer(n)) => Value::Int(*n),
                (b'J', Constant::Long(n)) => Value::Long(*n),
                (b'S', Constant::Integer(n)) => Value::Short(*n),
                (b'Z', Constant::Integer(n)) => Value::Boolean(*n),
                _ => return Err(Error::Malformed("annotation constant type")),
            }
        }
        _ => return Err(Error::Malformed("annotation value tag")),
    })
}
