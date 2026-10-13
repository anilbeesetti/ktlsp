//! Bounded JVM metadata names as lossless UTF-16; no declarations or semantics.
//! Selected malformed protobuf UTF-8 is an explicit unsupported compatibility gap.
//! Header, wire, schema and resolver budgets are separate; see METADATA_NAME_RESOLVER_SPEC.md.
use super::string_table::{Operation, Record, StringTableTypes};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

/// Caller limits can tighten, but cannot raise, these hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub entries: usize,
    pub input_bytes: usize,
    pub range_slots: usize,
    pub queries: usize,
    pub work: usize,
    /// Cumulative produced UTF-16 units, including reservations before later failures.
    pub output_units: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: MAX_RECORDS,
            input_bytes: MAX_BYTES,
            range_slots: MAX_RECORDS,
            queries: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
            output_units: MAX_BYTES / 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidIndex,
    MissingString,
    /// Java's malformed-byte replacement behavior is outside this finite subset.
    UnsupportedUtf8,
    Limit(&'static str),
}

/// Context uses the requested index and original compact record, never raw spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub index: Option<i32>,
    pub record: Option<usize>,
}
impl Error {
    fn limit(self, label: &'static str) -> Self {
        Self {
            kind: ErrorKind::Limit(label),
            ..self
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn debit(left: &mut usize, n: usize, error: Error) -> Result<(), Error> {
    *left = left.checked_sub(n).ok_or(error)?;
    Ok(())
}
fn add(total: &mut usize, n: usize, limit: usize, error: Error) -> Result<(), Error> {
    let sum = total.checked_add(n).ok_or(error)?;
    if sum > limit {
        return Err(error);
    }
    *total = sum;
    Ok(())
}
fn count(record: &Record<'_>) -> usize {
    record.range.unwrap_or(1).max(0) as usize
}

/// Borrows typed data; no expanded range table, cache or constructor allocation.
/// Raw/unknown fields are opaque. Public typed input is independently bounded.
/// One lifetime budget covers all calls and failures; failed debits keep their remainder.
#[derive(Debug)]
pub struct Resolver<'t, 'a> {
    table: &'t StringTableTypes<'a>,
    d2: &'t [&'a [u16]],
    work: usize,
    queries: usize,
    output_units: usize,
}
impl<'t, 'a> Resolver<'t, 'a> {
    pub fn new(
        table: &'t StringTableTypes<'a>,
        d2: &'t [&'a [u16]],
        limits: Limits,
    ) -> Result<Self, Error> {
        let hard = Limits::default();
        let error = Error {
            kind: ErrorKind::InvalidIndex,
            index: None,
            record: None,
        };
        let entry_limit = limits.entries.min(hard.entries);
        // Length-only preflight is separately hard-bounded, outside metered work.
        let mut entries = 0;
        for len in [table.records.len(), d2.len(), table.local_name.len()] {
            add(&mut entries, len, entry_limit, error.limit("entries"))?;
        }
        for record in &table.records {
            add(
                &mut entries,
                record.substring_index.len(),
                entry_limit,
                error.limit("entries"),
            )?;
            add(
                &mut entries,
                record.replace_char.len(),
                entry_limit,
                error.limit("entries"),
            )?;
        }
        let byte_limit = limits.input_bytes.min(hard.input_bytes);
        let mut bytes = 0;
        for record in &table.records {
            add(
                &mut bytes,
                record.string.map_or(0, <[u8]>::len),
                byte_limit,
                error.limit("input_bytes"),
            )?;
        }
        for text in d2 {
            let len = text
                .len()
                .checked_mul(2)
                .ok_or(error.limit("input_bytes"))?;
            add(&mut bytes, len, byte_limit, error.limit("input_bytes"))?;
        }
        let mut work = limits.work.min(hard.work);
        debit(&mut work, entries, error.limit("work"))?;
        debit(&mut work, bytes, error.limit("work"))?;
        let mut slots = 0;
        for (i, record) in table.records.iter().enumerate() {
            add(
                &mut slots,
                count(record),
                limits.range_slots.min(hard.range_slots),
                Error {
                    record: Some(i),
                    ..error
                }
                .limit("range_slots"),
            )?;
        }
        Ok(Self {
            table,
            d2,
            work,
            queries: limits.queries.min(hard.queries),
            output_units: limits.output_units.min(hard.output_units),
        })
    }

    fn query(&mut self, error: Error) -> Result<(), Error> {
        debit(&mut self.queries, 1, error.limit("queries"))?;
        debit(&mut self.work, 1, error.limit("work"))
    }

    /// Resolves getString/getQualifiedClassName units without name canonicality checks.
    /// A selected invalid protobuf UTF-8 string returns UnsupportedUtf8, never lossy text.
    pub fn get_string(&mut self, index: i32) -> Result<Vec<u16>, Error> {
        let error = Error {
            kind: ErrorKind::InvalidIndex,
            index: Some(index),
            record: None,
        };
        self.query(error)?;
        let mut remaining = usize::try_from(index).map_err(|_| error)?;
        let table = self.table;
        for (i, record) in table.records.iter().enumerate() {
            debit(&mut self.work, 1, error.limit("work"))?;
            let slots = count(record);
            if remaining >= slots {
                remaining -= slots;
                continue;
            }
            let error = Error {
                record: Some(i),
                ..error
            };
            if let Some(bytes) = record.string {
                debit(&mut self.work, bytes.len(), error.limit("work"))?;
                let text = std::str::from_utf8(bytes).map_err(|_| Error {
                    kind: ErrorKind::UnsupportedUtf8,
                    ..error
                })?;
                debit(&mut self.work, bytes.len(), error.limit("work"))?;
                let len = text.encode_utf16().count();
                return self.emit(record, text.encode_utf16(), len, error);
            }
            if let Some(text) = record
                .predefined_index
                .and_then(|n| usize::try_from(n).ok())
                .and_then(|n| PREDEFINED.get(n))
            {
                return self.emit(record, text.bytes().map(u16::from), text.len(), error);
            }
            let text = self.d2.get(index as usize).ok_or(Error {
                kind: ErrorKind::MissingString,
                ..error
            })?;
            return self.emit(record, text.iter().copied(), text.len(), error);
        }
        Err(error)
    }

    fn emit(
        &mut self,
        record: &Record<'_>,
        units: impl Iterator<Item = u16>,
        len: usize,
        error: Error,
    ) -> Result<Vec<u16>, Error> {
        debit(&mut self.work, len, error.limit("work"))?;
        let (mut begin, mut end) = (0, len);
        if let [a, b, ..] = record.substring_index.as_slice()
            && 0 <= *a
            && *a <= *b
            && (*b as usize) <= len
        {
            begin = *a as usize;
            end = *b as usize;
        }
        let operation = record.operation.unwrap_or(Operation::None);
        // Replacement keeps length, so descriptor stripping can select units before mapping.
        if operation == Operation::DescToClassId && end - begin >= 2 {
            begin += 1;
            end -= 1;
        }
        let length = end - begin;
        debit(&mut self.output_units, length, error.limit("output_units"))?;
        let bytes = length.checked_mul(2).ok_or(error.limit("work"))?;
        debit(&mut self.work, bytes, error.limit("work"))?;
        let mut out = Vec::new();
        out.try_reserve_exact(length)
            .map_err(|_| error.limit("allocation"))?;
        for mut unit in units.skip(begin).take(length) {
            if let [from, to, ..] = record.replace_char.as_slice()
                && unit == *from as u16
            {
                unit = *to as u16;
            }
            if operation != Operation::None && unit == u16::from(b'$') {
                unit = u16::from(b'.');
            }
            out.push(unit);
        }
        Ok(out)
    }

    /// Set membership for any i32, including negative/out-of-range indices.
    pub fn is_local_class_name(&mut self, index: i32) -> Result<bool, Error> {
        let error = Error {
            kind: ErrorKind::InvalidIndex,
            index: Some(index),
            record: None,
        };
        self.query(error)?;
        for n in &self.table.local_name {
            debit(&mut self.work, 1, error.limit("work"))?;
            if *n == index {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// Pinned Kotlin v2.3.20 order; Apache-2.0 provenance/notices are in the fixtures.
const PREDEFINED: [&str; 44] = [
    "kotlin/Any",
    "kotlin/Nothing",
    "kotlin/Unit",
    "kotlin/Throwable",
    "kotlin/Number",
    "kotlin/Byte",
    "kotlin/Double",
    "kotlin/Float",
    "kotlin/Int",
    "kotlin/Long",
    "kotlin/Short",
    "kotlin/Boolean",
    "kotlin/Char",
    "kotlin/CharSequence",
    "kotlin/String",
    "kotlin/Comparable",
    "kotlin/Enum",
    "kotlin/Array",
    "kotlin/ByteArray",
    "kotlin/DoubleArray",
    "kotlin/FloatArray",
    "kotlin/IntArray",
    "kotlin/LongArray",
    "kotlin/ShortArray",
    "kotlin/BooleanArray",
    "kotlin/CharArray",
    "kotlin/Cloneable",
    "kotlin/Annotation",
    "kotlin/collections/Iterable",
    "kotlin/collections/MutableIterable",
    "kotlin/collections/Collection",
    "kotlin/collections/MutableCollection",
    "kotlin/collections/List",
    "kotlin/collections/MutableList",
    "kotlin/collections/Set",
    "kotlin/collections/MutableSet",
    "kotlin/collections/Map",
    "kotlin/collections/MutableMap",
    "kotlin/collections/Map.Entry",
    "kotlin/collections/MutableMap.MutableEntry",
    "kotlin/collections/Iterator",
    "kotlin/collections/MutableIterator",
    "kotlin/collections/ListIterator",
    "kotlin/collections/MutableListIterator",
];
