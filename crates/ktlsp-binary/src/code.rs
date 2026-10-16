//! Bounded Code-attribute framing; bytecode and nested attributes remain opaque.
use crate::{Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

/// Caller limits can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Requested retained element storage, excluding allocator surplus and RSS.
    pub owned_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            records: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
            owned_bytes: MAX_BYTES,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    /// Zero is catch-all; nonzero indices are checked only for CONSTANT_Class kind.
    pub catch_type: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CodeBody<'a> {
    /// Complete Code payload, excluding the outer six-byte attribute header.
    pub raw: &'a [u8],
    pub max_stack: u16,
    pub max_locals: u16,
    pub bytecode: &'a [u8],
    pub exceptions: Vec<ExceptionHandler>,
    /// Ordered raw attributes; known names are also uninterpreted.
    pub attributes: Vec<Attribute<'a>>,
}

type Result<T> = std::result::Result<T, Error>;

struct Budget {
    limits: Limits,
    bytes: usize,
    records: usize,
    work: usize,
    owned_bytes: usize,
}
fn charge(used: &mut usize, n: usize, limit: usize, label: &'static str) -> Result<()> {
    *used = used.checked_add(n).ok_or(Error::Limit(label))?;
    if *used > limit {
        return Err(Error::Limit(label));
    }
    Ok(())
}
impl Budget {
    fn new(limits: Limits) -> Self {
        Self {
            limits: Limits {
                bytes: limits.bytes.min(MAX_BYTES),
                records: limits.records.min(MAX_RECORDS),
                work: limits.work.min(MAX_VALIDATION_UNITS),
                owned_bytes: limits.owned_bytes.min(MAX_BYTES),
            },
            bytes: 0,
            records: 0,
            work: 0,
            owned_bytes: 0,
        }
    }
    fn bytes(&mut self, n: usize) -> Result<()> {
        charge(&mut self.bytes, n, self.limits.bytes, "code bytes")
    }
    fn records(&mut self, n: usize) -> Result<()> {
        charge(&mut self.records, n, self.limits.records, "code records")
    }
    fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.work, n, self.limits.work, "code work")
    }
    fn text<'a>(&mut self, cp: &'a [Constant], index: u16) -> Result<&'a [u16]> {
        self.work(1)?;
        if index == 0 {
            return Err(Error::Malformed("constant index"));
        }
        let text = crate::utf16(cp, index)?;
        self.work(text.len())?;
        Ok(text)
    }
    fn vector<T>(&mut self, n: usize) -> Result<Vec<T>> {
        let bytes = n
            .checked_mul(size_of::<T>())
            .ok_or(Error::Limit("code owned bytes"))?;
        charge(
            &mut self.owned_bytes,
            bytes,
            self.limits.owned_bytes,
            "code owned bytes",
        )?;
        let mut out = Vec::new();
        if n != 0 {
            out.try_reserve_exact(n)
                .map_err(|_| Error::Limit("code allocation"))?;
        }
        Ok(out)
    }
}

/// Frame all methods in input order, with None for ordinary abstract/native ones.
/// One aggregate byte/record/work/requested-storage budget covers the whole call,
/// including ignored outer attributes. Versions match parse_class (through 69.0).
/// Consumed names/indices are checked even for forged public ClassFile values;
/// unrelated class state, descriptors and flag combinations are not revalidated.
/// Exact <clinit> requires Code regardless of abstract/native bits. Errors return
/// no partial output. Success establishes no opcode, stack, instruction-boundary,
/// nested-attribute, Throwable, control-flow or JVM semantic verification.
pub fn decode<'a>(class: &'a ClassFile<'_>, limits: Limits) -> Result<Vec<Option<CodeBody<'a>>>> {
    if !crate::supported_class_version(class.major, class.minor) {
        return Err(Error::UnsupportedVersion {
            major: class.major,
            minor: class.minor,
        });
    }
    if class.constants.is_empty() || class.constants.len() > MAX_RECORDS {
        return Err(Error::Malformed("code constant pool size"));
    }
    let mut budget = Budget::new(limits);
    budget.records(class.methods.len())?;
    let mut methods = budget.vector(class.methods.len())?;
    for method in &class.methods {
        budget.work(1)?;
        let name = budget.text(&class.constants, method.name)?;
        let required = crate::eq(name, "<clinit>") || method.access_flags & 0x0500 == 0;
        budget.records(method.attributes.len())?;
        let mut raw = None;
        for attr in &method.attributes {
            budget.work(1)?;
            budget.bytes(attr.bytes.len())?;
            budget.work(attr.bytes.len())?;
            let name = budget.text(&class.constants, attr.name)?;
            if crate::eq(name, "Code") {
                if !required {
                    return Err(Error::Malformed("unexpected Code attribute"));
                }
                if raw.is_some() {
                    return Err(Error::Malformed("duplicate Code attribute"));
                }
                raw = Some(attr.bytes);
            }
        }
        methods.push(match raw {
            Some(raw) => Some(body(raw, &class.constants, &mut budget)?),
            None if required => return Err(Error::Malformed("missing Code attribute")),
            None => None,
        });
    }
    Ok(methods)
}

fn body<'a>(raw: &'a [u8], cp: &[Constant], budget: &mut Budget) -> Result<CodeBody<'a>> {
    let mut r = crate::Reader {
        bytes: raw,
        at: 0,
        records: 0,
    };
    let max_stack = r.u16()?;
    let max_locals = r.u16()?;
    let len = r.u32()?;
    if !(1..=65_535).contains(&len) {
        return Err(Error::Malformed("code length"));
    }
    let bytecode = r.take(usize::try_from(len).map_err(|_| Error::Truncated)?)?;
    let count = usize::from(r.u16()?);
    budget.records(count)?;
    let mut exceptions = budget.vector(count)?;
    for _ in 0..count {
        budget.work(1)?;
        let handler = ExceptionHandler {
            start_pc: r.u16()?,
            end_pc: r.u16()?,
            handler_pc: r.u16()?,
            catch_type: r.u16()?,
        };
        if handler.start_pc >= handler.end_pc
            || usize::from(handler.end_pc) > bytecode.len()
            || usize::from(handler.handler_pc) >= bytecode.len()
        {
            return Err(Error::Malformed("code exception range"));
        }
        if handler.catch_type != 0 {
            budget.work(1)?;
            if !matches!(crate::constant(cp, handler.catch_type)?, Constant::Class(_)) {
                return Err(Error::Malformed("class index"));
            }
        }
        exceptions.push(handler);
    }
    let count = usize::from(r.u16()?);
    budget.records(count)?;
    let mut attributes = budget.vector(count)?;
    for _ in 0..count {
        budget.work(1)?;
        let name = r.u16()?;
        budget.text(cp, name)?;
        let len = usize::try_from(r.u32()?).map_err(|_| Error::Truncated)?;
        attributes.push(Attribute {
            name,
            bytes: r.take(len)?,
        });
    }
    if r.at != raw.len() {
        return Err(Error::Malformed("code trailing bytes"));
    }
    Ok(CodeBody {
        raw,
        max_stack,
        max_locals,
        bytecode,
        exceptions,
        attributes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_counter_and_storage_arithmetic_is_checked() {
        for (caller, expected) in [
            (
                usize::MAX,
                (MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, MAX_BYTES),
            ),
            (0, (0, 0, 0, 0)),
        ] {
            let budget = Budget::new(Limits {
                bytes: caller,
                records: caller,
                work: caller,
                owned_bytes: caller,
            });
            assert_eq!(
                (
                    budget.limits.bytes,
                    budget.limits.records,
                    budget.limits.work,
                    budget.limits.owned_bytes
                ),
                expected
            );
        }
        for (label, limit) in [
            ("code bytes", MAX_BYTES),
            ("code records", MAX_RECORDS),
            ("code work", MAX_VALIDATION_UNITS),
            ("code owned bytes", MAX_BYTES),
        ] {
            let mut used = limit - 1;
            assert_eq!(charge(&mut used, 1, limit, label), Ok(()));
            assert_eq!(charge(&mut used, 1, limit, label), Err(Error::Limit(label)));
            let mut used = usize::MAX;
            assert_eq!(
                charge(&mut used, 1, usize::MAX, label),
                Err(Error::Limit(label))
            );
        }
        let mut budget = Budget::new(Limits::default());
        assert_eq!(
            budget.vector::<u16>(usize::MAX),
            Err(Error::Limit("code owned bytes"))
        );
        assert_eq!(budget.owned_bytes, 0);
        budget.bytes = usize::MAX;
        budget.records = usize::MAX;
        budget.work = usize::MAX;
        budget.owned_bytes = usize::MAX;
        assert_eq!(budget.bytes(1), Err(Error::Limit("code bytes")));
        assert_eq!(budget.records(1), Err(Error::Limit("code records")));
        assert_eq!(budget.work(1), Err(Error::Limit("code work")));
        assert_eq!(
            budget.vector::<u8>(1),
            Err(Error::Limit("code owned bytes"))
        );
    }
}
