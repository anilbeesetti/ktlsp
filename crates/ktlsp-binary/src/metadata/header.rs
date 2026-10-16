use crate::annotations::{Annotation, DeclarationAnnotations, Value, Visibility};
use crate::{MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS};

/// Caller limits can tighten, but cannot raise, the hard defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub code_units: usize,
    pub output_bytes: usize,
    pub fields: usize,
    pub work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            code_units: MAX_BYTES,
            output_bytes: MAX_BYTES,
            fields: MAX_RECORDS,
            work: MAX_VALIDATION_UNITS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(&'static str),
    Limit(&'static str),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Field,
    Version,
    Kind,
}

/// Raw UTF-16 is borrowed without normalization. `raw` also retains unknown fields.
/// Presence bits, in order: k/mv/bv/d1/d2/xs/pn/xi; omitted fields use schema defaults.
#[derive(Debug, PartialEq, Eq)]
pub struct Header<'a> {
    pub raw: &'a Annotation<'a>,
    pub visibility: &'a Visibility,
    pub present: u8,
    pub k: i32,
    pub mv: Vec<i32>,
    pub bv: Vec<i32>,
    pub d1: Vec<&'a [u16]>,
    pub d2: Vec<&'a [u16]>,
    pub xs: &'a [u16],
    pub pn: &'a [u16],
    pub xi: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome<'a> {
    Absent,
    Unsupported {
        header: Header<'a>,
        reason: Unsupported,
    },
    Decoded {
        header: Header<'a>,
        bytes: Vec<u8>,
    },
}

pub(super) struct Budget<'b> {
    limits: Limits,
    code_units: usize,
    fields: usize,
    work: usize,
    composite: Option<&'b mut super::string_table::Budget>,
}
fn charge(used: &mut usize, n: usize, limit: usize, label: &'static str) -> Result<(), Error> {
    *used = used.checked_add(n).ok_or(Error::Limit(label))?;
    if *used > limit {
        return Err(Error::Limit(label));
    }
    Ok(())
}
impl<'b> Budget<'b> {
    pub(super) fn new(limits: Limits) -> Self {
        let hard = Limits::default();
        Self {
            limits: Limits {
                code_units: limits.code_units.min(hard.code_units),
                output_bytes: limits.output_bytes.min(hard.output_bytes),
                fields: limits.fields.min(hard.fields),
                work: limits.work.min(hard.work),
            },
            code_units: 0,
            fields: 0,
            work: 0,
            composite: None,
        }
    }
    pub(super) fn composed(budget: &'b mut super::string_table::Budget) -> Self {
        Self { composite: Some(budget), ..Self::new(Limits::default()) }
    }
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        if let Some(budget) = &mut self.composite {
            return budget.push(values, value, 0).map_err(composite_error);
        }
        values.try_reserve(1).map_err(|_| Error::Limit("metadata allocation"))?;
        values.push(value);
        Ok(())
    }
    fn fields(&mut self, n: usize) -> Result<(), Error> {
        charge(&mut self.fields, n, self.limits.fields, "metadata fields")?;
        if let Some(budget) = &mut self.composite {
            budget.wire.records(n).map_err(composite_error)?;
        }
        self.work(n)
    }
    fn text(&mut self, text: &[u16]) -> Result<(), Error> {
        charge(
            &mut self.code_units,
            text.len(),
            self.limits.code_units,
            "metadata code units",
        )?;
        self.work(text.len())
    }
    pub(super) fn work(&mut self, n: usize) -> Result<(), Error> {
        charge(&mut self.work, n, self.limits.work, "metadata work")?;
        if let Some(budget) = &mut self.composite {
            budget.wire.work(n, 0).map_err(composite_error)?;
        }
        Ok(())
    }
    pub(super) fn output(&mut self, n: usize) -> Result<(), Error> {
        if n > self.limits.output_bytes {
            return Err(Error::Limit("metadata output bytes"));
        }
        if let Some(budget) = &mut self.composite {
            budget.input(n).map_err(composite_error)?;
            budget.allocation(n).map_err(composite_error)?;
        }
        self.work(n)
    }
}

fn composite_error(error: super::wire::Error) -> Error {
    match error.kind {
        super::wire::ErrorKind::Limit(label) => Error::Limit(label),
        _ => Error::Malformed("metadata composite budget"),
    }
}

/// Decode already typed annotations from `annotations::decode`. Both visibility
/// containers are accepted as data; duplicate or member-level Metadata is malformed.
/// Nested annotations and unrelated annotation values are not searched/interpreted.
/// One aggregate budget charges visited containers/descriptors, header names/values,
/// array entries and decoding work before allocation. Unknown values remain opaque.
/// Only mv=[2,3,0], k=1/2/5 are byte payloads; no compiler compatibility is implied.
pub fn decode<'a>(
    input: &'a DeclarationAnnotations<'a>,
    limits: Limits,
) -> Result<Outcome<'a>, Error> {
    decode_with_budget(input, &mut Budget::new(limits))
}

pub(super) fn decode_with_budget<'a>(
    input: &'a DeclarationAnnotations<'a>,
    budget: &mut Budget<'_>,
) -> Result<Outcome<'a>, Error> {
    budget.fields(input.class.len())?;
    budget.fields(input.fields.len())?;
    budget.fields(input.methods.len())?;
    let mut found = None;
    for (attrs, class) in std::iter::once((&input.class, true)).chain(
        input
            .fields
            .iter()
            .chain(&input.methods)
            .map(|attrs| (attrs, false)),
    ) {
        if !class {
            budget.fields(attrs.len())?;
        }
        for attr in attrs {
            budget.fields(attr.annotations.len())?;
            for annotation in &attr.annotations {
                budget.text(annotation.descriptor)?;
                if crate::eq(annotation.descriptor, "Lkotlin/Metadata;") {
                    if !class {
                        return Err(Error::Malformed("metadata declaration context"));
                    }
                    if found.replace((annotation, &attr.visibility)).is_some() {
                        return Err(Error::Malformed("duplicate metadata annotation"));
                    }
                }
            }
        }
    }
    let Some((raw, visibility)) = found else {
        return Ok(Outcome::Absent);
    };
    let mut header = Header {
        raw,
        visibility,
        present: 0,
        k: 1,
        mv: vec![],
        bv: vec![],
        d1: vec![],
        d2: vec![],
        xs: &[],
        pn: &[],
        xi: 0,
    };
    for value in [1, 0, 3] {
        budget.push(&mut header.bv, value)?;
    }
    budget.fields(raw.elements.len())?;
    let mut unknown = false;
    for (i, element) in raw.elements.iter().enumerate() {
        budget.text(element.name)?;
        for previous in &raw.elements[..i] {
            budget.work(element.name.len().min(previous.name.len()) + 1)?;
            if element.name == previous.name {
                return Err(Error::Malformed("duplicate metadata field"));
            }
        }
        let slot = ["k", "mv", "bv", "d1", "d2", "xs", "pn", "xi"]
            .iter()
            .position(|name| crate::eq(element.name, name));
        let Some(slot) = slot else {
            unknown = true;
            continue;
        };
        header.present |= 1 << slot;
        match slot {
            0 => header.k = integer(&element.value)?,
            1 => header.mv = integers(&element.value, budget)?,
            2 => header.bv = integers(&element.value, budget)?,
            3 => header.d1 = strings(&element.value, budget)?,
            4 => header.d2 = strings(&element.value, budget)?,
            5 => header.xs = string(&element.value, budget)?,
            6 => header.pn = string(&element.value, budget)?,
            7 => header.xi = integer(&element.value)?,
            _ => unreachable!(),
        }
    }
    let reason = if unknown {
        Some(Unsupported::Field)
    } else if header.mv != [2, 3, 0] {
        Some(Unsupported::Version)
    } else if ![1, 2, 5].contains(&header.k) {
        Some(Unsupported::Kind)
    } else {
        None
    };
    if let Some(reason) = reason {
        return Ok(Outcome::Unsupported { header, reason });
    }
    let bytes = super::encoding::decode(&header.d1, budget)?;
    Ok(Outcome::Decoded { header, bytes })
}

fn integer(value: &Value<'_>) -> Result<i32, Error> {
    match value {
        Value::Int(n) => Ok(*n as i32),
        _ => Err(Error::Malformed("metadata integer type")),
    }
}
fn array<'a, 'b>(value: &'b Value<'a>, budget: &mut Budget) -> Result<&'b [Value<'a>], Error> {
    let Value::Array(values) = value else {
        return Err(Error::Malformed("metadata array type"));
    };
    budget.fields(values.len())?;
    Ok(values)
}
fn integers(value: &Value<'_>, budget: &mut Budget) -> Result<Vec<i32>, Error> {
    let mut out = Vec::new();
    for value in array(value, budget)? {
        let value = integer(value)?;
        budget.push(&mut out, value)?;
    }
    Ok(out)
}
fn string<'a>(value: &Value<'a>, budget: &mut Budget) -> Result<&'a [u16], Error> {
    let Value::String(text) = value else {
        return Err(Error::Malformed("metadata string type"));
    };
    budget.text(text)?;
    Ok(text)
}
fn strings<'a>(value: &Value<'a>, budget: &mut Budget) -> Result<Vec<&'a [u16]>, Error> {
    let mut out = Vec::new();
    for value in array(value, budget)? {
        let text = string(value, budget)?;
        budget.push(&mut out, text)?;
    }
    Ok(out)
}
