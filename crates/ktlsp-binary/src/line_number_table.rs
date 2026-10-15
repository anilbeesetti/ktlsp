//! Bounded encoded LineNumberTable data from one caller-supplied Code body.
use crate::{Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, code::CodeBody};

/// Caller limits tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
    pub work: usize,
    /// Logical retained elements, excluding capacity surplus, allocator and RSS.
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
pub struct LineNumber {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LineNumberTable<'a> {
    pub name: u16,
    /// Complete selected payload, excluding its outer six-byte attribute header.
    pub raw: &'a [u8],
    pub entries: Vec<LineNumber>,
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
        charge(&mut self.bytes, n, self.limits.bytes, "line number bytes")
    }
    fn records(&mut self, n: usize) -> Result<()> {
        charge(
            &mut self.records,
            n,
            self.limits.records,
            "line number records",
        )
    }
    fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.work, n, self.limits.work, "line number work")
    }
    fn storage<T>(&mut self, n: usize) -> Result<()> {
        let bytes = n
            .checked_mul(size_of::<T>())
            .ok_or(Error::Limit("line number owned bytes"))?;
        charge(
            &mut self.owned_bytes,
            bytes,
            self.limits.owned_bytes,
            "line number owned bytes",
        )
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
}

/// Decode all selected tables in encounter order, preserving entry repetitions.
/// One budget covers every nested attribute (including ignored/aliased payloads),
/// name lookup and selected entry. Class parsing and Code framing are separate.
/// Forged public inputs have consumed references and bytecode length checked;
/// raw/body containment, exceptions, unrelated constants and versions are not
/// revalidated. Numeric PCs are checked, not instruction boundaries. No lookup,
/// source association, JVM verification or semantic claim follows. Errors return
/// no partial output, and all selected raw payloads borrow the original input.
pub fn decode<'a>(
    body: &'a CodeBody<'_>,
    cp: &[Constant],
    limits: Limits,
) -> Result<Vec<LineNumberTable<'a>>> {
    if cp.is_empty() || cp.len() > MAX_RECORDS {
        return Err(Error::Malformed("line number constant pool size"));
    }
    if !(1..=65_535).contains(&body.bytecode.len()) {
        return Err(Error::Malformed("line number code length"));
    }
    let mut budget = Budget::new(limits);
    budget.records(body.attributes.len())?;
    let mut tables = Vec::new();
    for attr in &body.attributes {
        budget.work(1)?;
        budget.bytes(attr.bytes.len())?;
        budget.work(attr.bytes.len())?;
        let name = budget.text(cp, attr.name)?;
        if !crate::eq(name, "LineNumberTable") {
            continue;
        }
        budget.storage::<LineNumberTable<'_>>(1)?;
        tables
            .try_reserve(1)
            .map_err(|_| Error::Limit("line number allocation"))?;
        let mut r = crate::Reader {
            bytes: attr.bytes,
            at: 0,
            records: 0,
        };
        let count = usize::from(r.u16()?);
        budget.records(count)?;
        budget.storage::<LineNumber>(count)?;
        let mut entries = Vec::new();
        if count != 0 {
            entries
                .try_reserve_exact(count)
                .map_err(|_| Error::Limit("line number allocation"))?;
        }
        for _ in 0..count {
            budget.work(1)?;
            let entry = LineNumber {
                start_pc: r.u16()?,
                line_number: r.u16()?,
            };
            if usize::from(entry.start_pc) >= body.bytecode.len() {
                return Err(Error::Malformed("line number start pc"));
            }
            entries.push(entry);
        }
        if r.at != attr.bytes.len() {
            return Err(Error::Malformed("line number trailing bytes"));
        }
        tables.push(LineNumberTable {
            name: attr.name,
            raw: attr.bytes,
            entries,
        });
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_budget_caps_and_checked_arithmetic() {
        for caller in [0, usize::MAX] {
            let budget = Budget::new(Limits {
                bytes: caller,
                records: caller,
                work: caller,
                owned_bytes: caller,
            });
            assert_eq!(budget.limits.bytes, caller.min(MAX_BYTES));
            assert_eq!(budget.limits.records, caller.min(MAX_RECORDS));
            assert_eq!(budget.limits.work, caller.min(MAX_VALIDATION_UNITS));
            assert_eq!(budget.limits.owned_bytes, caller.min(MAX_BYTES));
        }
        for (label, limit) in [
            ("line number bytes", MAX_BYTES),
            ("line number records", MAX_RECORDS),
            ("line number work", MAX_VALIDATION_UNITS),
            ("line number owned bytes", MAX_BYTES),
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
            budget.storage::<LineNumber>(usize::MAX),
            Err(Error::Limit("line number owned bytes"))
        );
        assert_eq!(budget.owned_bytes, 0);
        budget.bytes = usize::MAX;
        budget.records = usize::MAX;
        budget.work = usize::MAX;
        budget.owned_bytes = usize::MAX;
        assert_eq!(budget.bytes(1), Err(Error::Limit("line number bytes")));
        assert_eq!(budget.records(1), Err(Error::Limit("line number records")));
        assert_eq!(budget.work(1), Err(Error::Limit("line number work")));
        assert_eq!(
            budget.storage::<LineNumber>(1),
            Err(Error::Limit("line number owned bytes"))
        );
    }
}
