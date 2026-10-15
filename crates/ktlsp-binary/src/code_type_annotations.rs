//! Encoded Code type annotations; no opcode, control-flow or type verification.
use crate::annotations::{self, Annotation, Budget, Visibility};
use crate::code::CodeBody;
use crate::type_annotations::{TypePathStep, vector};
use crate::{ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, Reader};

type Result<T> = std::result::Result<T, Error>;

/// Caller limits tighten the hard defaults. Nodes are logical, not owned bytes/RSS.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub nodes: usize,
    pub work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            nodes: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LocalVariableTarget {
    pub start_pc: u16,
    pub length: u16,
    pub index: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CodeTargetInfo {
    LocalVariable(Vec<LocalVariableTarget>),
    Catch {
        exception_table_index: u16,
    },
    Offset {
        offset: u16,
    },
    TypeArgument {
        offset: u16,
        type_argument_index: u8,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct CodeTypeAnnotation<'a> {
    /// Exact target tag through final element value, within the attribute payload.
    pub raw: &'a [u8],
    pub target_type: u8,
    pub target_info: CodeTargetInfo,
    pub path: Vec<TypePathStep>,
    pub annotation: Annotation<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CodeTypeAnnotationAttribute<'a> {
    pub raw: &'a [u8],
    pub visibility: Visibility,
    pub annotations: Vec<CodeTypeAnnotation<'a>>,
}

/// Decode one caller-selected Code body with its matching class constant pool.
/// Revalidate consumed public body fields, raw framing and borrowed slice identity.
/// One aggregate raw-byte/logical-node/work/depth budget covers both visibilities,
/// including unknown payloads. Annotation allocations inherit logical accounting;
/// allocator failure recovery and owned-byte/RSS bounds are not guaranteed.
/// Body membership in class.methods, instruction boundaries, opcode/target/path
/// applicability and resolved types are unverified. Errors return no partial data.
pub fn decode<'a>(
    class: &'a ClassFile<'_>,
    code: &'a CodeBody<'_>,
    limits: Limits,
) -> Result<Vec<CodeTypeAnnotationAttribute<'a>>> {
    if !(class.major == 45 && class.minor <= 3
        || (46..=67).contains(&class.major) && class.minor == 0)
    {
        return Err(Error::UnsupportedVersion {
            major: class.major,
            minor: class.minor,
        });
    }
    if class.constants.is_empty() || class.constants.len() > MAX_RECORDS {
        return Err(Error::Malformed("annotation constant pool size"));
    }
    if code.raw.len() > limits.bytes.min(MAX_BYTES) {
        return Err(Error::Limit("code type annotation bytes"));
    }
    let mut budget = Budget::default();
    budget.nodes(MAX_RECORDS - limits.nodes.min(MAX_RECORDS))?;
    budget.work(MAX_VALIDATION_UNITS - limits.work.min(MAX_VALIDATION_UNITS))?;
    budget.work(code.raw.len())?;
    validate_body(code, &class.constants, &mut budget)?;
    let mut out = Vec::new();
    let mut seen = [false; 2];
    for attr in &code.attributes {
        budget.work(1)?;
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
        let mut r = reader(attr.bytes);
        let count = usize::from(r.u16()?);
        let mut entries = vector(count, &mut budget)?;
        for _ in 0..count {
            budget.work(1)?;
            let start = r.at;
            let target_type = r.u8()?;
            let target_info = target(&mut r, target_type, code, &mut budget)?;
            let count = usize::from(r.u8()?);
            budget.work(1 + 2 * count)?;
            let mut path = vector(count, &mut budget)?;
            for _ in 0..count {
                let kind = r.u8()?;
                let argument_index = r.u8()?;
                if kind > 3 || kind < 3 && argument_index != 0 {
                    return Err(Error::Malformed("type annotation path"));
                }
                path.push(TypePathStep {
                    kind,
                    argument_index,
                });
            }
            let annotation = annotations::annotation(&mut r, &class.constants, &mut budget, 1)?;
            entries.push(CodeTypeAnnotation {
                raw: &attr.bytes[start..r.at],
                target_type,
                target_info,
                path,
                annotation,
            });
        }
        if r.at != attr.bytes.len() {
            return Err(Error::Malformed("type annotation trailing bytes"));
        }
        if out.is_empty() {
            out = vector(2, &mut budget)?;
        }
        out.push(CodeTypeAnnotationAttribute {
            raw: attr.bytes,
            visibility: if slot == 0 {
                Visibility::RuntimeVisible
            } else {
                Visibility::RuntimeInvisible
            },
            annotations: entries,
        });
    }
    Ok(out)
}

fn reader(bytes: &[u8]) -> Reader<'_> {
    Reader {
        bytes,
        at: 0,
        records: 0,
    }
}
fn same_span(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.as_ptr() == b.as_ptr()
}
fn mismatch() -> Error {
    Error::Malformed("code body mismatch")
}
fn validate_body(code: &CodeBody<'_>, cp: &[Constant], budget: &mut Budget) -> Result<()> {
    budget.nodes(code.exceptions.len())?;
    budget.nodes(code.attributes.len())?;
    let mut r = reader(code.raw);
    if r.u16()? != code.max_stack || r.u16()? != code.max_locals {
        return Err(mismatch());
    }
    let len = r.u32()?;
    if !(1..=65_535).contains(&len) {
        return Err(Error::Malformed("code length"));
    }
    if !same_span(
        r.take(usize::try_from(len).map_err(|_| Error::Truncated)?)?,
        code.bytecode,
    ) {
        return Err(mismatch());
    }
    if usize::from(r.u16()?) != code.exceptions.len() {
        return Err(mismatch());
    }
    for e in &code.exceptions {
        budget.work(1)?;
        let encoded = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
        if encoded != (e.start_pc, e.end_pc, e.handler_pc, e.catch_type) {
            return Err(mismatch());
        }
        if e.start_pc >= e.end_pc
            || usize::from(e.end_pc) > code.bytecode.len()
            || usize::from(e.handler_pc) >= code.bytecode.len()
        {
            return Err(Error::Malformed("code exception range"));
        }
        if e.catch_type != 0 {
            budget.work(1)?;
            if !matches!(crate::constant(cp, e.catch_type)?, Constant::Class(_)) {
                return Err(Error::Malformed("class index"));
            }
        }
    }
    if usize::from(r.u16()?) != code.attributes.len() {
        return Err(mismatch());
    }
    for attr in &code.attributes {
        budget.work(1)?;
        if r.u16()? != attr.name {
            return Err(mismatch());
        }
        budget.text(cp, attr.name)?;
        let len = usize::try_from(r.u32()?).map_err(|_| Error::Truncated)?;
        if !same_span(r.take(len)?, attr.bytes) {
            return Err(mismatch());
        }
    }
    if r.at != code.raw.len() {
        return Err(Error::Malformed("code trailing bytes"));
    }
    Ok(())
}

fn target(
    r: &mut Reader<'_>,
    tag: u8,
    code: &CodeBody<'_>,
    budget: &mut Budget,
) -> Result<CodeTargetInfo> {
    Ok(match tag {
        0x40 | 0x41 => {
            let count = usize::from(r.u16()?);
            let mut table = vector(count, budget)?;
            for _ in 0..count {
                budget.work(1)?;
                let row = LocalVariableTarget {
                    start_pc: r.u16()?,
                    length: r.u16()?,
                    index: r.u16()?,
                };
                let end = usize::from(row.start_pc)
                    .checked_add(usize::from(row.length))
                    .ok_or(Error::Malformed("code type annotation local range"))?;
                if usize::from(row.start_pc) >= code.bytecode.len()
                    || end > code.bytecode.len()
                    || row.index >= code.max_locals
                {
                    return Err(Error::Malformed("code type annotation local range"));
                }
                table.push(row);
            }
            CodeTargetInfo::LocalVariable(table)
        }
        0x42 => {
            let exception_table_index = r.u16()?;
            if usize::from(exception_table_index) >= code.exceptions.len() {
                return Err(Error::Malformed("code type annotation catch index"));
            }
            CodeTargetInfo::Catch {
                exception_table_index,
            }
        }
        0x43..=0x4b => {
            let offset = r.u16()?;
            if usize::from(offset) >= code.bytecode.len() {
                return Err(Error::Malformed("code type annotation offset"));
            }
            if tag >= 0x47 {
                CodeTargetInfo::TypeArgument {
                    offset,
                    type_argument_index: r.u8()?,
                }
            } else {
                CodeTargetInfo::Offset { offset }
            }
        }
        _ => return Err(Error::Malformed("code type annotation target")),
    })
}
