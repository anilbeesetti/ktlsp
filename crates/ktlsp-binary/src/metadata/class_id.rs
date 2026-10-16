//! Lossless Kotlin ClassId parts shared by finite metadata consumers.
use super::{name_resolver, string_table, wire};

/// Shares the resolver's one UTF-16 allocation; equality compares semantic parts.
#[derive(Debug)]
pub struct ClassId {
    text: Vec<u16>,
    package_end: usize,
    relative_start: usize,
    pub local: bool,
}
impl ClassId {
    pub fn package_name(&self) -> &[u16] {
        &self.text[..self.package_end]
    }
    pub fn relative_class_name(&self) -> &[u16] {
        &self.text[self.relative_start..]
    }
}
impl PartialEq for ClassId {
    fn eq(&self, other: &Self) -> bool {
        self.local == other.local
            && self.package_name() == other.package_name()
            && self.relative_class_name() == other.relative_class_name()
    }
}
impl Eq for ClassId {}

pub(super) enum Error {
    Wire(wire::Error),
    Name(name_resolver::Error),
}
impl From<wire::Error> for Error {
    fn from(error: wire::Error) -> Self {
        Self::Wire(error)
    }
}

pub(super) fn resolve(
    index: i32,
    offset: usize,
    resolver: &mut name_resolver::Resolver<'_, '_>,
    budget: &mut string_table::Budget,
) -> Result<ClassId, Error> {
    let mut text = resolver
        .get_string_with_budget(index, budget)
        .map_err(Error::Name)?;
    let local = resolver
        .is_local_class_name_with_budget(index, budget)
        .map_err(Error::Name)?;
    // Kotlin ClassId.fromString: last slash BEFORE first backtick, not last slash overall.
    // One bounded scan selects the package; one in-place pass preserves lone surrogates.
    budget.wire.work(text.len(), offset)?;
    let mut split = None;
    for (i, &unit) in text.iter().enumerate() {
        if unit == u16::from(b'`') {
            break;
        }
        if unit == u16::from(b'/') {
            split = Some(i);
        }
    }
    budget.wire.work(text.len(), offset)?;
    let relative_start = split.map_or(0, |i| i + 1);
    let package_end = split.unwrap_or(0);
    for unit in &mut text[..package_end] {
        if *unit == u16::from(b'/') {
            *unit = u16::from(b'.');
        }
    }
    let mut write = relative_start;
    for read in relative_start..text.len() {
        if text[read] != u16::from(b'`') {
            text[write] = text[read];
            write += 1;
        }
    }
    text.truncate(write);
    Ok(ClassId {
        text,
        package_end,
        relative_start,
        local,
    })
}
