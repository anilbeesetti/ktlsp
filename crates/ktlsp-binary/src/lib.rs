//! Bounded classfile structure reader; attribute contents are deliberately opaque.

pub mod annotation_extensions;
pub mod archive;
pub use archive::{ZipClassError, parse_zip_class};
pub mod annotations;
pub mod class_type_annotations;
pub mod code;
pub mod code_type_annotations;
pub mod line_number_table;
pub mod metadata;
pub mod method_type_annotations;
pub mod nesting;
pub mod type_annotations;

use std::fmt;

pub mod signatures;

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_RECORDS: usize = 65_535;
pub const MAX_VALIDATION_UNITS: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Truncated,
    Malformed(&'static str),
    UnsupportedVersion { major: u16, minor: u16 },
    UnsupportedTag(u8),
    Limit(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Index zero and the second slot of long/double entries are `Unusable`.
#[derive(Debug, PartialEq, Eq)]
pub enum Constant {
    Unusable,
    Utf8(Vec<u16>),
    Integer(u32),
    Float(u32),
    Long(u64),
    Double(u64),
    Class(u16),
    String(u16),
    Fieldref { class: u16, name_and_type: u16 },
    Methodref { class: u16, name_and_type: u16 },
    InterfaceMethodref { class: u16, name_and_type: u16 },
    NameAndType { name: u16, descriptor: u16 },
    MethodHandle { kind: u8, reference: u16 },
    MethodType(u16),
    Dynamic { bootstrap: u16, name_and_type: u16 },
    InvokeDynamic { bootstrap: u16, name_and_type: u16 },
    Module(u16),
    Package(u16),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Attribute<'a> {
    pub name: u16,
    pub bytes: &'a [u8],
}
#[derive(Debug, PartialEq, Eq)]
pub struct Member<'a> {
    pub access_flags: u16,
    pub name: u16,
    pub descriptor: u16,
    pub attributes: Vec<Attribute<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ClassFile<'a> {
    pub minor: u16,
    pub major: u16,
    pub constants: Vec<Constant>,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<Member<'a>>,
    pub methods: Vec<Member<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}
impl ClassFile<'_> {
    /// Modified UTF-8 decoded losslessly, including unpaired UTF-16 surrogates.
    pub fn utf16(&self, index: u16) -> Result<&[u16]> {
        utf16(&self.constants, index)
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    records: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(len).ok_or(Error::Truncated)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn count(&mut self) -> Result<usize> {
        let n = usize::from(self.u16()?);
        self.records += n;
        if self.records > MAX_RECORDS {
            return Err(Error::Limit("records"));
        }
        Ok(n)
    }
    fn attributes(&mut self, cp: &[Constant]) -> Result<Vec<Attribute<'a>>> {
        let n = self.count()?;
        let mut out = Vec::new();
        for _ in 0..n {
            let name = self.u16()?;
            utf16(cp, name)?;
            let len = self.u32()? as usize;
            out.push(Attribute {
                name,
                bytes: self.take(len)?,
            });
        }
        Ok(out)
    }
    fn members(
        &mut self,
        cp: &[Constant],
        method: bool,
        major: u16,
        work: &mut usize,
    ) -> Result<Vec<Member<'a>>> {
        let n = self.count()?;
        let mut out = Vec::new();
        for _ in 0..n {
            let access_flags = self.u16()?;
            let name = self.u16()?;
            let descriptor = self.u16()?;
            let text = charged(cp, name, work)?;
            name_valid(text, method)?;
            let desc = charged(cp, descriptor, work)?;
            descriptor_valid(
                desc,
                method,
                method && access_flags & 0x0008 == 0 && !eq(text, "<clinit>"),
            )?;
            if method
                && (eq(text, "<init>") && desc.last() != Some(&86)
                    || eq(text, "<clinit>")
                        && (desc.last() != Some(&86)
                            || major >= 51 && (!eq(desc, "()V") || access_flags & 8 == 0)))
            {
                return Err(Error::Malformed("special method"));
            }
            let attributes = self.attributes(cp)?;
            out.push(Member {
                access_flags,
                name,
                descriptor,
                attributes,
            });
        }
        Ok(out)
    }
}

/// Reads a supported structural subset. Success does not establish JVM verification.
pub fn parse_class(bytes: &[u8]) -> Result<ClassFile<'_>> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Limit("bytes"));
    }
    let mut r = Reader {
        bytes,
        at: 0,
        records: 0,
    };
    if r.u32()? != 0xcafebabe {
        return Err(Error::Malformed("magic"));
    }
    let minor = r.u16()?;
    let major = r.u16()?;
    if !(major == 45 && minor <= 3 || (46..=67).contains(&major) && minor == 0) {
        return Err(Error::UnsupportedVersion { major, minor });
    }
    let n = r.count()?;
    if n == 0 {
        return Err(Error::Malformed("constant pool count"));
    }
    let mut cp = vec![Constant::Unusable];
    while cp.len() < n {
        let tag = r.u8()?;
        let minimum = match tag {
            15 | 16 | 18 => 51,
            19 | 20 => 53,
            17 => 55,
            _ => 45,
        };
        if major < minimum {
            return Err(Error::Malformed("constant tag version"));
        }
        let c = match tag {
            1 => {
                let len = usize::from(r.u16()?);
                Constant::Utf8(mutf8(r.take(len)?)?)
            }
            3 => Constant::Integer(r.u32()?),
            4 => Constant::Float(r.u32()?),
            5 => Constant::Long(r.u64()?),
            6 => Constant::Double(r.u64()?),
            7 => Constant::Class(r.u16()?),
            8 => Constant::String(r.u16()?),
            9 => Constant::Fieldref {
                class: r.u16()?,
                name_and_type: r.u16()?,
            },
            10 => Constant::Methodref {
                class: r.u16()?,
                name_and_type: r.u16()?,
            },
            11 => Constant::InterfaceMethodref {
                class: r.u16()?,
                name_and_type: r.u16()?,
            },
            12 => Constant::NameAndType {
                name: r.u16()?,
                descriptor: r.u16()?,
            },
            15 => Constant::MethodHandle {
                kind: r.u8()?,
                reference: r.u16()?,
            },
            16 => Constant::MethodType(r.u16()?),
            17 => Constant::Dynamic {
                bootstrap: r.u16()?,
                name_and_type: r.u16()?,
            },
            18 => Constant::InvokeDynamic {
                bootstrap: r.u16()?,
                name_and_type: r.u16()?,
            },
            19 => Constant::Module(r.u16()?),
            20 => Constant::Package(r.u16()?),
            _ => return Err(Error::UnsupportedTag(tag)),
        };
        let wide = matches!(c, Constant::Long(_) | Constant::Double(_));
        cp.push(c);
        if wide {
            if cp.len() == n {
                return Err(Error::Malformed("wide constant slot"));
            }
            cp.push(Constant::Unusable);
        }
    }
    let mut work = 0;
    validate_pool(&cp, major, &mut work)?;
    let access_flags = r.u16()?;
    let this_class = r.u16()?;
    class_name(&cp, this_class, &mut work, false)?;
    let super_class = r.u16()?;
    if super_class != 0 {
        class_name(&cp, super_class, &mut work, false)?;
    } else if !eq(
        class_name(&cp, this_class, &mut work, false)?,
        "java/lang/Object",
    ) && access_flags & 0x8000 == 0
    {
        return Err(Error::Malformed("zero superclass"));
    }
    let n = r.count()?;
    let mut interfaces = Vec::new();
    for _ in 0..n {
        let index = r.u16()?;
        class_name(&cp, index, &mut work, false)?;
        interfaces.push(index);
    }
    let fields = r.members(&cp, false, major, &mut work)?;
    let methods = r.members(&cp, true, major, &mut work)?;
    let attributes = r.attributes(&cp)?;
    if r.at != bytes.len() {
        return Err(Error::Malformed("trailing bytes"));
    }
    Ok(ClassFile {
        minor,
        major,
        constants: cp,
        access_flags,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        attributes,
    })
}
fn constant(cp: &[Constant], i: u16) -> Result<&Constant> {
    match cp.get(usize::from(i)) {
        None | Some(Constant::Unusable) => Err(Error::Malformed("constant index")),
        Some(c) => Ok(c),
    }
}
fn utf16(cp: &[Constant], i: u16) -> Result<&[u16]> {
    match constant(cp, i)? {
        Constant::Utf8(s) => Ok(s),
        _ => Err(Error::Malformed("UTF8 index")),
    }
}
fn charged<'a>(cp: &'a [Constant], i: u16, work: &mut usize) -> Result<&'a [u16]> {
    let s = utf16(cp, i)?;
    *work += s.len();
    if *work > MAX_VALIDATION_UNITS {
        return Err(Error::Limit("validation work"));
    }
    Ok(s)
}
fn eq(s: &[u16], ascii: &str) -> bool {
    s.iter().copied().eq(ascii.bytes().map(u16::from))
}
fn name_valid(s: &[u16], method: bool) -> Result<()> {
    if method && (eq(s, "<init>") || eq(s, "<clinit>")) {
        return Ok(());
    }
    if s.is_empty()
        || s.iter()
            .any(|c| [46, 59, 91, 47].contains(c) || method && [60, 62].contains(c))
    {
        return Err(Error::Malformed("member name"));
    }
    Ok(())
}
fn internal_name(s: &[u16]) -> Result<()> {
    for part in s.split(|c| *c == 47) {
        name_valid(part, false)?;
    }
    Ok(())
}
fn class_name<'a>(cp: &'a [Constant], i: u16, work: &mut usize, arrays: bool) -> Result<&'a [u16]> {
    let Constant::Class(name) = constant(cp, i)? else {
        return Err(Error::Malformed("class index"));
    };
    let s = charged(cp, *name, work)?;
    if arrays && s.first() == Some(&91) {
        descriptor_valid(s, false, false)?;
    } else {
        internal_name(s)?;
    }
    Ok(s)
}
fn nat(cp: &[Constant], i: u16) -> Result<(u16, u16)> {
    match constant(cp, i)? {
        Constant::NameAndType { name, descriptor } => Ok((*name, *descriptor)),
        _ => Err(Error::Malformed("name-and-type index")),
    }
}
fn validate_pool(cp: &[Constant], major: u16, work: &mut usize) -> Result<()> {
    for (i, c) in cp.iter().enumerate().skip(1) {
        match c {
            Constant::Class(_) => {
                class_name(cp, i as u16, work, true)?;
            }
            Constant::String(i) | Constant::Module(i) | Constant::Package(i) => {
                utf16(cp, *i)?;
            }
            Constant::NameAndType { name, descriptor } => {
                let d = charged(cp, *descriptor, work)?;
                let method = d.first() == Some(&40);
                descriptor_valid(d, method, false)?;
                let name = charged(cp, *name, work)?;
                name_valid(name, method)?;
                if method && eq(name, "<clinit>") {
                    return Err(Error::Malformed("name-and-type method name"));
                }
            }
            Constant::MethodType(i) => {
                descriptor_valid(charged(cp, *i, work)?, true, false)?;
            }
            Constant::Fieldref {
                class,
                name_and_type,
            }
            | Constant::Methodref {
                class,
                name_and_type,
            }
            | Constant::InterfaceMethodref {
                class,
                name_and_type,
            } => {
                if !matches!(constant(cp, *class)?, Constant::Class(_)) {
                    return Err(Error::Malformed("class index"));
                }
                let (name, d) = nat(cp, *name_and_type)?;
                let method = !matches!(c, Constant::Fieldref { .. });
                let d = charged(cp, d, work)?;
                descriptor_valid(d, method, false)?;
                let name = charged(cp, name, work)?;
                name_valid(name, method)?;
                if method
                    && (eq(name, "<clinit>")
                        || eq(name, "<init>")
                            && (matches!(c, Constant::InterfaceMethodref { .. })
                                || d.last() != Some(&86)))
                {
                    return Err(Error::Malformed("reference method name"));
                }
            }
            Constant::Dynamic { name_and_type, .. }
            | Constant::InvokeDynamic { name_and_type, .. } => {
                let (name, d) = nat(cp, *name_and_type)?;
                let method = matches!(c, Constant::InvokeDynamic { .. });
                descriptor_valid(charged(cp, d, work)?, method, false)?;
                let name = charged(cp, name, work)?;
                name_valid(name, method)?;
                if method && (eq(name, "<init>") || eq(name, "<clinit>")) {
                    return Err(Error::Malformed("dynamic name"));
                }
            }
            Constant::MethodHandle { kind, reference } => {
                let target = constant(cp, *reference)?;
                let valid = match kind {
                    1..=4 => matches!(target, Constant::Fieldref { .. }),
                    5 | 8 => matches!(target, Constant::Methodref { .. }),
                    6 | 7 => {
                        matches!(target, Constant::Methodref { .. })
                            || major >= 52 && matches!(target, Constant::InterfaceMethodref { .. })
                    }
                    9 => matches!(target, Constant::InterfaceMethodref { .. }),
                    _ => false,
                };
                if !valid {
                    return Err(Error::Malformed("method handle target"));
                }
                if let Constant::Methodref { name_and_type, .. }
                | Constant::InterfaceMethodref { name_and_type, .. } = target
                {
                    let (name, _) = nat(cp, *name_and_type)?;
                    let name = charged(cp, name, work)?;
                    if (*kind == 8) != eq(name, "<init>") || eq(name, "<clinit>") {
                        return Err(Error::Malformed("method handle name"));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn field_type(s: &[u16], at: &mut usize) -> Result<usize> {
    let start = *at;
    while s.get(*at) == Some(&91) {
        *at += 1;
    }
    let arrays = *at - start;
    if arrays > 255 {
        return Err(Error::Malformed("array depth"));
    }
    let c = *s.get(*at).ok_or(Error::Malformed("descriptor"))?;
    *at += 1;
    let slots = match c {
        66 | 67 | 70 | 73 | 83 | 90 => 1,
        68 | 74 => 2,
        76 => {
            let begin = *at;
            while s.get(*at).is_some_and(|c| *c != 59) {
                *at += 1;
            }
            if s.get(*at) != Some(&59) {
                return Err(Error::Malformed("object descriptor"));
            }
            internal_name(&s[begin..*at])?;
            *at += 1;
            1
        }
        _ => return Err(Error::Malformed("descriptor type")),
    };
    Ok(if arrays > 0 { 1 } else { slots })
}
fn descriptor_valid(s: &[u16], method: bool, receiver: bool) -> Result<()> {
    let mut at = 0;
    if method {
        if s.first() != Some(&40) {
            return Err(Error::Malformed("method descriptor"));
        }
        at += 1;
        let mut slots = usize::from(receiver);
        while s.get(at).is_some_and(|c| *c != 41) {
            slots += field_type(s, &mut at)?;
            if slots > 255 {
                return Err(Error::Malformed("parameter slots"));
            }
        }
        if s.get(at) != Some(&41) {
            return Err(Error::Malformed("method descriptor"));
        }
        at += 1;
        if s.get(at) == Some(&86) {
            at += 1;
        } else {
            field_type(s, &mut at)?;
        }
    } else {
        field_type(s, &mut at)?;
    }
    if at != s.len() {
        return Err(Error::Malformed("descriptor trailing data"));
    }
    Ok(())
}
fn mutf8(bytes: &[u8]) -> Result<Vec<u16>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let a = bytes[at];
        at += 1;
        let unit = match a {
            1..=127 => u16::from(a),
            0xc0..=0xdf => {
                let b = *bytes.get(at).ok_or(Error::Malformed("modified UTF8"))?;
                at += 1;
                let u = (u16::from(a & 31) << 6) | u16::from(b & 63);
                if b & 0xc0 != 0x80 || u < 128 && !(a == 0xc0 && b == 0x80) {
                    return Err(Error::Malformed("modified UTF8"));
                }
                u
            }
            0xe0..=0xef => {
                let b = *bytes.get(at).ok_or(Error::Malformed("modified UTF8"))?;
                let c = *bytes.get(at + 1).ok_or(Error::Malformed("modified UTF8"))?;
                at += 2;
                let u = (u16::from(a & 15) << 12) | (u16::from(b & 63) << 6) | u16::from(c & 63);
                if b & 0xc0 != 0x80 || c & 0xc0 != 0x80 || u < 2048 {
                    return Err(Error::Malformed("modified UTF8"));
                }
                u
            }
            _ => return Err(Error::Malformed("modified UTF8")),
        };
        out.push(unit);
    }
    Ok(out)
}
