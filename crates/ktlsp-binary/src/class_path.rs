//! Bounded ordered archive class lookup as data, not class loading or linkage.

use crate::{ClassFile, Constant, ZipClassError, parse_zip_class};
use ktlsp_artifacts::{JmodDirectoryIndex, ZipDirectoryIndex};
use std::collections::HashSet;

pub const MAX_ROOTS: usize = 64;
pub const MAX_NAME_BYTES: usize = 4096;
pub const MAX_ENTRIES: usize = 65_536;
pub const MAX_SCAN_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_MANIFEST_TOTAL: usize = 1024 * 1024;
const MAX_HEADERS: usize = 4096;
const MAX_VALUE: usize = 65_535;

/// Borrow already indexed roots in explicit precedence order. No indexing,
/// filesystem access, manifest Class-Path traversal, or module visibility policy.
#[derive(Clone, Copy, Debug)]
pub enum ClasspathRoot<'i, 'a> {
    Jar(&'i ZipDirectoryIndex<'a>),
    Jmod(&'i JmodDirectoryIndex<'a>),
}
impl<'i, 'a> ClasspathRoot<'i, 'a> {
    fn index(self) -> &'i ZipDirectoryIndex<'a> {
        match self {
            Self::Jar(index) => index,
            Self::Jmod(index) => index.zip_index(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassRank {
    Base,
    Version(u16),
    Jmod,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassOrigin {
    pub root: usize,
    pub ordinal: usize,
    pub rank: ClassRank,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ClasspathClass<'a> {
    pub origin: ClassOrigin,
    pub class: ClassFile<'a>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClasspathError {
    InvalidName,
    InvalidRelease,
    Limit(&'static str),
    AmbiguousManifest {
        root: usize,
    },
    Manifest {
        root: usize,
        reason: &'static str,
    },
    ManifestPayload {
        root: usize,
        error: ZipClassError,
    },
    AmbiguousClass {
        origin: ClassOrigin,
    },
    ClassPayload {
        origin: ClassOrigin,
        error: ZipClassError,
    },
    DeclaredName {
        origin: ClassOrigin,
    },
    IncompatibleVersion {
        origin: ClassOrigin,
        major: u16,
        maximum: u32,
    },
}
impl std::fmt::Display for ClasspathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "archive classpath: {self:?}")
    }
}
impl std::error::Error for ClasspathError {}

fn charge(
    total: &mut usize,
    amount: usize,
    cap: usize,
    field: &'static str,
) -> Result<(), ClasspathError> {
    *total = total
        .checked_add(amount)
        .filter(|&n| n <= cap)
        .ok_or(ClasspathError::Limit(field))?;
    Ok(())
}
fn validate_name(name: &str) -> Result<(), ClasspathError> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(ClasspathError::InvalidName);
    }
    if name.split('/').any(str::is_empty)
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '.' | ';' | '[' | '\\' | ':'))
    {
        return Err(ClasspathError::InvalidName);
    }
    Ok(())
}

/// Query a JVM internal name (UTF-8, no normalization) at an explicit Java
/// feature release >= 1. The first matching root wins. An ambiguous or invalid
/// winner is an error, never permission to try a lower rank/root. `None` alone
/// means absent. UTF-16 this_class identity is compared without lossy conversion.
///
/// All-root metadata/manifest budgets are checked before scratch allocation.
/// Selected payload/structural-output limits remain those of parse_zip_class.
/// Scratch is reused for manifests, then the selected class; retain it while
/// the result borrows it. No JVM/API-equivalence/signature verification occurs.
pub fn lookup_class<'a>(
    roots: &[ClasspathRoot<'_, 'a>],
    name: &str,
    java_release: u16,
    scratch: &'a mut Vec<u8>,
) -> Result<Option<ClasspathClass<'a>>, ClasspathError> {
    scratch.clear();
    validate_name(name)?;
    if java_release == 0 {
        return Err(ClasspathError::InvalidRelease);
    }
    preflight(roots)?;
    for (root, &source) in roots.iter().enumerate() {
        let index = source.index();
        let multi = match source {
            ClasspathRoot::Jar(_) => manifest_multi(index, root, scratch)?,
            ClasspathRoot::Jmod(_) => false,
        };
        let mut winner: Option<ClassOrigin> = None;
        let mut duplicate = false;
        for (ordinal, entry) in index.entries().iter().enumerate() {
            let raw = entry.raw_name();
            let rank = match source {
                ClasspathRoot::Jmod(_) => raw
                    .strip_prefix(b"classes/")
                    .filter(|rest| class_path_matches(rest, name))
                    .map(|_| ClassRank::Jmod),
                ClasspathRoot::Jar(_) => {
                    if class_path_matches(raw, name) {
                        Some(ClassRank::Base)
                    } else if multi && java_release >= 9 && !name.starts_with("META-INF/") {
                        versioned(raw, name, java_release).map(ClassRank::Version)
                    } else {
                        None
                    }
                }
            };
            let Some(rank) = rank else { continue };
            let origin = ClassOrigin {
                root,
                ordinal,
                rank,
            };
            match winner {
                Some(previous) if rank_number(previous.rank) > rank_number(rank) => {}
                Some(previous) if previous.rank == rank => duplicate = true,
                _ => {
                    winner = Some(origin);
                    duplicate = false;
                }
            }
        }
        let Some(origin) = winner else { continue };
        if duplicate {
            return Err(ClasspathError::AmbiguousClass { origin });
        }
        let class = parse_zip_class(index, origin.ordinal, scratch)
            .map_err(|error| ClasspathError::ClassPayload { origin, error })?;
        let Constant::Class(declared) = class.constants[usize::from(class.this_class)] else {
            return Err(ClasspathError::DeclaredName { origin });
        };
        if !class
            .utf16(declared)
            .map_err(|error| ClasspathError::ClassPayload {
                origin,
                error: ZipClassError::Class(error),
            })?
            .iter()
            .copied()
            .eq(name.encode_utf16())
        {
            return Err(ClasspathError::DeclaredName { origin });
        }
        let release = match origin.rank {
            ClassRank::Version(version) => version,
            _ => java_release,
        };
        let maximum = u32::from(release) + 44;
        if u32::from(class.major) > maximum {
            return Err(ClasspathError::IncompatibleVersion {
                origin,
                major: class.major,
                maximum,
            });
        }
        return Ok(Some(ClasspathClass { origin, class }));
    }
    scratch.clear();
    Ok(None)
}
fn rank_number(rank: ClassRank) -> u16 {
    match rank {
        ClassRank::Version(n) => n,
        _ => 0,
    }
}
fn class_path_matches(path: &[u8], name: &str) -> bool {
    path.strip_suffix(b".class") == Some(name.as_bytes())
}
fn versioned(path: &[u8], name: &str, release: u16) -> Option<u16> {
    let rest = path.strip_prefix(b"META-INF/versions/")?;
    let slash = rest.iter().position(|&b| b == b'/')?;
    let digits = &rest[..slash];
    if !matches!(digits.first(), Some(b'1'..=b'9')) || !class_path_matches(&rest[slash + 1..], name)
    {
        return None;
    }
    let mut version = 0u16;
    for &digit in digits {
        if !digit.is_ascii_digit() {
            return None;
        }
        version = version
            .checked_mul(10)?
            .checked_add(u16::from(digit - b'0'))?;
    }
    (version >= 9 && version <= release).then_some(version)
}
fn preflight(roots: &[ClasspathRoot<'_, '_>]) -> Result<(), ClasspathError> {
    if roots.len() > MAX_ROOTS {
        return Err(ClasspathError::Limit("roots"));
    }
    let (mut entries, mut names, mut manifests) = (0, 0, 0);
    for &root in roots {
        let index = root.index();
        charge(&mut entries, index.entries().len(), MAX_ENTRIES, "entries")?;
        for entry in index.entries() {
            // Three bounded directory passes, each comparing only borrowed names.
            charge(
                &mut names,
                entry.raw_name().len(),
                MAX_SCAN_BYTES,
                "scan bytes",
            )?;
            if matches!(root, ClasspathRoot::Jar(_))
                && entry
                    .raw_name()
                    .eq_ignore_ascii_case(b"META-INF/MANIFEST.MF")
            {
                for size in [
                    entry.claimed_compressed_size(),
                    entry.claimed_uncompressed_size(),
                ] {
                    if size as usize > MAX_MANIFEST_BYTES {
                        return Err(ClasspathError::Limit("manifest bytes"));
                    }
                    charge(
                        &mut manifests,
                        size as usize,
                        MAX_MANIFEST_TOTAL,
                        "manifest total",
                    )?;
                }
            }
        }
    }
    Ok(())
}
fn manifest_multi(
    index: &ZipDirectoryIndex<'_>,
    root: usize,
    scratch: &mut Vec<u8>,
) -> Result<bool, ClasspathError> {
    let mut selected = None;
    for (ordinal, entry) in index.entries().iter().enumerate() {
        if entry
            .raw_name()
            .eq_ignore_ascii_case(b"META-INF/MANIFEST.MF")
            && selected.replace(ordinal).is_some()
        {
            return Err(ClasspathError::AmbiguousManifest { root });
        }
    }
    let Some(ordinal) = selected else {
        return Ok(false);
    };
    let bytes = if index.entries()[ordinal].method() == 8 {
        index.deflated_payload(ordinal, scratch).map_err(|error| {
            ClasspathError::ManifestPayload {
                root,
                error: ZipClassError::Deflated(error),
            }
        })?
    } else {
        index
            .stored_payload(ordinal)
            .map_err(|error| ClasspathError::ManifestPayload {
                root,
                error: ZipClassError::Stored(error),
            })?
    };
    parse_manifest(bytes).map_err(|reason| ClasspathError::Manifest { root, reason })
}

// Only the actual main section is interpreted. Named sections are not policy.
fn parse_manifest(bytes: &[u8]) -> Result<bool, &'static str> {
    let mut headers = HashSet::new();
    headers
        .try_reserve(MAX_HEADERS.min(bytes.len() / 4))
        .map_err(|_| "header allocation")?;
    let mut value = Vec::new();
    value
        .try_reserve(MAX_VALUE.min(bytes.len()))
        .map_err(|_| "value allocation")?;
    let mut current: Option<&[u8]> = None;
    let mut multi = false;
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        while at < bytes.len() && !matches!(bytes[at], b'\r' | b'\n') {
            at += 1;
        }
        if at == bytes.len() {
            return Err("unterminated main line");
        }
        let line = &bytes[start..at];
        let cr = bytes[at] == b'\r';
        at += 1;
        if cr && bytes.get(at) == Some(&b'\n') {
            at += 1;
        }
        if line.len() > 72 || line.contains(&0) {
            return Err("invalid main line");
        }
        if line.first() == Some(&b' ') {
            if current.is_none() {
                return Err("orphan continuation");
            }
            append_value(&mut value, &line[1..])?;
            continue;
        }
        if let Some(header) = current {
            finish_header(header, &value, &mut multi)?;
        }
        if line.is_empty() {
            return if current.is_some() {
                Ok(multi)
            } else {
                Err("missing Manifest-Version")
            };
        }
        let colon = line
            .iter()
            .position(|&b| b == b':')
            .ok_or("missing header colon")?;
        let name = &line[..colon];
        if name.is_empty()
            || name.len() > 70
            || !name[0].is_ascii_alphanumeric()
            || !name
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || line.get(colon + 1) != Some(&b' ')
        {
            return Err("invalid header");
        }
        if current.is_none() && name != b"Manifest-Version" {
            return Err("missing initial Manifest-Version");
        }
        if name.eq_ignore_ascii_case(b"Name") {
            return Err("Name in main section");
        }
        if headers.len() == MAX_HEADERS {
            return Err("main header limit");
        }
        let mut key = [0; 70];
        for (out, &byte) in key.iter_mut().zip(name) {
            *out = byte.to_ascii_lowercase();
        }
        if !headers.insert(key) {
            return Err("duplicate main header");
        }
        current = Some(name);
        value.clear();
        append_value(&mut value, &line[colon + 2..])?;
    }
    Err("missing main section terminator")
}
fn append_value(value: &mut Vec<u8>, part: &[u8]) -> Result<(), &'static str> {
    if value.len() + part.len() > MAX_VALUE {
        return Err("main value limit");
    }
    value.extend_from_slice(part);
    Ok(())
}
fn finish_header(name: &[u8], value: &[u8], multi: &mut bool) -> Result<(), &'static str> {
    std::str::from_utf8(value).map_err(|_| "invalid header UTF-8")?;
    if name.eq_ignore_ascii_case(b"Manifest-Version")
        && (value.is_empty()
            || value
                .split(|&b| b == b'.')
                .any(|s| s.is_empty() || !s.iter().all(u8::is_ascii_digit)))
    {
        return Err("invalid Manifest-Version");
    }
    if name.eq_ignore_ascii_case(b"Multi-Release") {
        *multi = value.eq_ignore_ascii_case(b"true");
    }
    Ok(())
}
