//! JVMS 23 §§4.7.6–7, 4.7.28–29. Reads declarations, not resolved nest membership.

use std::collections::HashMap;

use crate::{Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Class<'a> {
    pub index: u16,
    /// Internal binary name, lossless even for unpaired UTF-16 surrogates.
    pub name: &'a [u16],
}

#[derive(Debug, PartialEq, Eq)]
pub struct InnerClass<'a> {
    pub inner: Class<'a>,
    /// None means not a member class; it does not identify a lexical owner.
    pub outer: Option<Class<'a>>,
    /// None denotes an anonymous class. The index addresses the original pool.
    pub name_index: Option<u16>,
    pub access_flags: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Method<'a> {
    pub index: u16,
    pub name: &'a [u16],
    pub descriptor: &'a [u16],
}

#[derive(Debug, PartialEq, Eq)]
pub struct Enclosing<'a> {
    pub class: Class<'a>,
    /// None denotes an initializer context, not an unresolved method.
    pub method: Option<Method<'a>>,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct Nesting<'a> {
    /// None distinguishes an absent attribute from an empty table.
    pub inner_classes: Option<Vec<InnerClass<'a>>>,
    pub enclosing: Option<Enclosing<'a>>,
    pub nest_host: Option<Class<'a>>,
    /// Preserves order, duplicates and self references; access control resolves these.
    pub nest_members: Option<Vec<Class<'a>>>,
}

#[derive(Default)]
struct Budget {
    records: usize,
    bytes: usize,
    work: usize,
}
fn charge(value: &mut usize, n: usize, limit: usize, label: &'static str) -> Result<()> {
    *value = value.checked_add(n).ok_or(Error::Limit(label))?;
    if *value > limit {
        return Err(Error::Limit(label));
    }
    Ok(())
}
impl Budget {
    fn records(&mut self, n: usize) -> Result<()> {
        charge(&mut self.records, n, MAX_RECORDS, "nesting records")
    }
    fn bytes(&mut self, n: usize) -> Result<()> {
        charge(&mut self.bytes, n, MAX_BYTES, "nesting bytes")
    }
    fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.work, n, MAX_VALIDATION_UNITS, "nesting work")
    }
    fn text<'a>(&mut self, c: &'a ClassFile<'_>, i: u16) -> Result<&'a [u16]> {
        checked_constant(c, i)?;
        let text = c.utf16(i)?;
        self.work(text.len())?;
        Ok(text)
    }
    fn class<'a>(&mut self, c: &'a ClassFile<'_>, i: u16) -> Result<Class<'a>> {
        let Constant::Class(n) = checked_constant(c, i)? else {
            return Err(Error::Malformed("nesting class reference"));
        };
        let name = self.text(c, *n)?;
        crate::internal_name(name)?;
        Ok(Class { index: i, name })
    }
    fn kind(&mut self, c: &ClassFile<'_>, a: &Attribute<'_>) -> Result<Option<usize>> {
        self.records(1)?;
        self.bytes(a.bytes.len())?;
        let name = self.text(c, a.name)?;
        Ok(
            ["InnerClasses", "EnclosingMethod", "NestHost", "NestMembers"]
                .iter()
                .position(|s| crate::eq(name, s)),
        )
    }
}

fn checked_constant<'a>(c: &'a ClassFile<'_>, i: u16) -> Result<&'a Constant> {
    let index = usize::from(i);
    if index == 0 {
        return Err(Error::Malformed("nesting zero reference"));
    }
    // Public constructors can replace the placeholder, but the slot stays unusable.
    if matches!(
        c.constants.get(index - 1),
        Some(Constant::Long(_) | Constant::Double(_))
    ) {
        return Err(Error::Malformed("nesting wide second slot"));
    }
    match c.constants.get(index) {
        None | Some(Constant::Unusable) => Err(Error::Malformed("nesting constant reference")),
        Some(constant) => Ok(constant),
    }
}

fn word(bytes: &[u8], at: usize) -> Result<u16> {
    let pair = bytes.get(at..at + 2).ok_or(Error::Truncated)?;
    Ok(u16::from_be_bytes([pair[0], pair[1]]))
}
fn table(bytes: &[u8], width: usize, b: &mut Budget) -> Result<usize> {
    let count = usize::from(word(bytes, 0)?);
    b.records(count)?;
    // A u2 count and fixed widths cannot overflow usize on supported targets.
    if bytes.len() != 2 + count * width {
        return Err(Error::Malformed("nesting attribute length"));
    }
    Ok(count)
}
fn optional(i: u16) -> Option<u16> {
    (i != 0).then_some(i)
}

/// Decodes only class-level nesting attributes, without modifying the class or flags.
///
/// Publicly constructed models are checked for supported version, aggregate sizes,
/// attribute contexts/names and all references used here. Unrelated CP entries,
/// member descriptors and opaque attribute interiors are not reverified.
/// Budgets: MAX_RECORDS across pool slots, interfaces, members, attributes and table
/// entries; MAX_BYTES across UTF-16 pool storage and raw attribute payloads;
/// MAX_VALIDATION_UNITS across referenced text and relationship validation.
pub fn decode<'a>(c: &'a ClassFile<'_>) -> Result<Nesting<'a>> {
    if !(c.major == 45 && c.minor <= 3 || (46..=67).contains(&c.major) && c.minor == 0) {
        return Err(Error::UnsupportedVersion {
            major: c.major,
            minor: c.minor,
        });
    }
    let mut b = Budget::default();
    for n in [
        c.constants.len(),
        c.interfaces.len(),
        c.fields.len(),
        c.methods.len(),
    ] {
        b.records(n)?;
    }
    for constant in &c.constants {
        if let Constant::Utf8(s) = constant {
            b.bytes(
                s.len()
                    .checked_mul(2)
                    .ok_or(Error::Limit("nesting bytes"))?,
            )?;
        }
    }
    let current = b.class(c, c.this_class)?;
    for member in c.fields.iter().chain(&c.methods) {
        for attr in &member.attributes {
            if b.kind(c, attr)?.is_some() {
                return Err(Error::Malformed("nesting attribute context"));
            }
        }
    }
    let mut out = Nesting::default();
    let mut seen = [false; 4];
    for attr in &c.attributes {
        let Some(kind) = b.kind(c, attr)? else {
            continue;
        };
        if c.access_flags & 0x8000 != 0 && kind != 0 {
            return Err(Error::Malformed("nesting attribute context"));
        }
        if std::mem::replace(&mut seen[kind], true) {
            return Err(Error::Malformed("duplicate nesting attribute"));
        }
        let minimum = [45, 49, 55, 55][kind];
        if c.major < minimum || kind == 0 && c.major == 45 && c.minor < 3 {
            return Err(Error::Malformed("nesting attribute version"));
        }
        let bytes = attr.bytes;
        match kind {
            0 => {
                let count = table(bytes, 8, &mut b)?;
                let mut entries = Vec::new();
                for i in 0..count {
                    let at = 2 + i * 8;
                    let inner = b.class(c, word(bytes, at)?)?;
                    let outer = optional(word(bytes, at + 2)?)
                        .map(|n| b.class(c, n))
                        .transpose()?;
                    let name_index = optional(word(bytes, at + 4)?);
                    if let Some(n) = name_index {
                        crate::name_valid(b.text(c, n)?, false)?;
                    }
                    if outer.is_some_and(|o| o.name == inner.name)
                        || c.major >= 51 && name_index.is_none() && outer.is_some()
                    {
                        return Err(Error::Malformed("inconsistent inner class"));
                    }
                    entries.push(InnerClass {
                        inner,
                        outer,
                        name_index,
                        access_flags: word(bytes, at + 6)?,
                    });
                }
                out.inner_classes = Some(entries);
            }
            1 => {
                if bytes.len() != 4 {
                    return Err(Error::Malformed("nesting attribute length"));
                }
                let class = b.class(c, word(bytes, 0)?)?;
                let method = if let Some(index) = optional(word(bytes, 2)?) {
                    let Constant::NameAndType { name, descriptor } = checked_constant(c, index)?
                    else {
                        return Err(Error::Malformed("enclosing method reference"));
                    };
                    let name = b.text(c, *name)?;
                    let descriptor = b.text(c, *descriptor)?;
                    crate::name_valid(name, true)?;
                    crate::descriptor_valid(descriptor, true, crate::eq(name, "<init>"))?;
                    if crate::eq(name, "<clinit>")
                        || crate::eq(name, "<init>") && descriptor.last() != Some(&86)
                    {
                        return Err(Error::Malformed("enclosing method name"));
                    }
                    Some(Method {
                        index,
                        name,
                        descriptor,
                    })
                } else {
                    None
                };
                if class.name == current.name {
                    return Err(Error::Malformed("self enclosing class"));
                }
                out.enclosing = Some(Enclosing { class, method });
            }
            2 => {
                if bytes.len() != 2 {
                    return Err(Error::Malformed("nesting attribute length"));
                }
                out.nest_host = Some(b.class(c, word(bytes, 0)?)?);
            }
            3 => {
                let count = table(bytes, 2, &mut b)?;
                let mut entries = Vec::new();
                for i in 0..count {
                    entries.push(b.class(c, word(bytes, 2 + i * 2)?)?);
                }
                out.nest_members = Some(entries);
            }
            _ => unreachable!(),
        }
    }
    if seen[2] && seen[3] {
        return Err(Error::Malformed("conflicting nest attributes"));
    }
    relationships(&out, current, &mut b)?;
    Ok(out)
}

fn relationships(out: &Nesting<'_>, current: Class<'_>, b: &mut Budget) -> Result<()> {
    let mut owners = HashMap::new();
    if let Some(entries) = &out.inner_classes {
        for entry in entries {
            b.work(entry.inner.name.len() + entry.outer.map_or(0, |o| o.name.len()))?;
            if owners
                .insert(entry.inner.name, entry.outer.map(|o| o.name))
                .is_some()
            {
                return Err(Error::Malformed("duplicate inner class"));
            }
        }
    }
    if let Some(enclosing) = &out.enclosing {
        b.work(current.name.len() + enclosing.class.name.len())?;
        if owners
            .insert(current.name, Some(enclosing.class.name))
            .flatten()
            .is_some()
        {
            return Err(Error::Malformed("member class has enclosing method"));
        }
    }
    let mut states = HashMap::new();
    for &start in owners.keys() {
        let mut path = Vec::new();
        let mut next = Some(start);
        while let Some(name) = next {
            b.work(1 + name.len())?;
            match states.get(name) {
                Some(1) => return Err(Error::Malformed("cyclic nesting")),
                Some(2) => break,
                _ => {}
            }
            states.insert(name, 1);
            path.push(name);
            next = owners.get(name).copied().flatten();
        }
        for name in path {
            b.work(1 + name.len())?;
            states.insert(name, 2);
        }
    }
    Ok(())
}
