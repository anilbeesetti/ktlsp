//! Exact path-based source candidates, not compilation provenance or line mapping.

use crate::{ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, annotations, metadata};
use ktlsp_artifacts::{ZipDeflateError, ZipDirectoryEntry, ZipDirectoryIndex, ZipStoredError};

pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_WORK: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    Class(Error),
    MissingSourceFile,
    MissingCandidate,
    Ambiguous { first: usize, second: usize },
    InvalidUtf16,
    UnsafePath,
    Unsupported(&'static str),
    Metadata(metadata::Error),
    UnsupportedMetadata(metadata::Unsupported),
    Limit(&'static str),
    Stored(ZipStoredError),
    Deflated(ZipDeflateError),
}
impl std::fmt::Display for AttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source attachment: {self:?}")
    }
}
impl std::error::Error for AttachmentError {}

#[derive(Debug)]
pub struct SourceCandidate<'index, 'data> {
    pub ordinal: usize,
    pub entry: &'index ZipDirectoryEntry<'data>,
    pub bytes: &'data [u8],
}

type Result<T, E = AttachmentError> = std::result::Result<T, E>;

fn charge(used: &mut usize, n: usize, max: usize, label: &'static str) -> Result<()> {
    *used = used.checked_add(n).ok_or(AttachmentError::Limit(label))?;
    if *used > max {
        return Err(AttachmentError::Limit(label));
    }
    Ok(())
}
fn text<'a>(class: &'a ClassFile<'_>, index: u16, work: &mut usize) -> Result<&'a [u16]> {
    if index == 0 {
        return Err(AttachmentError::Class(Error::Malformed(
            "source constant index",
        )));
    }
    let text = class.utf16(index).map_err(AttachmentError::Class)?;
    charge(work, text.len(), MAX_WORK, "work")?;
    Ok(text)
}

// Validate without allocating a lossy string or a second path/name buffer.
fn path_bytes(text: &[u16], separator: Option<char>) -> Result<usize> {
    let mut bytes = 0;
    let mut segment_len = 0;
    let mut dots_only = true;
    for decoded in char::decode_utf16(text.iter().copied()) {
        let c = decoded.map_err(|_| AttachmentError::InvalidUtf16)?;
        if Some(c) == separator {
            if segment_len == 0 || dots_only {
                return Err(AttachmentError::UnsafePath);
            }
            segment_len = 0;
            dots_only = true;
        } else {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | ';' | '[') {
                return Err(AttachmentError::UnsafePath);
            }
            segment_len += 1;
            dots_only &= c == '.';
            if separator.is_some() && c == '.' {
                return Err(AttachmentError::UnsafePath);
            }
        }
        charge(&mut bytes, c.len_utf8(), MAX_PATH_BYTES, "path bytes")?;
    }
    if segment_len == 0 || dots_only {
        return Err(AttachmentError::UnsafePath);
    }
    Ok(bytes)
}

fn matches_path(mut raw: &[u8], package: &[u16], separator: char, file: &[u16]) -> bool {
    for (text, package_part) in [(package, true), (file, false)] {
        for decoded in char::decode_utf16(text.iter().copied()) {
            let Ok(mut c) = decoded else { return false };
            if package_part && c == separator {
                c = '/';
            }
            let mut buffer = [0; 4];
            let encoded = c.encode_utf8(&mut buffer).as_bytes();
            let Some(rest) = raw.strip_prefix(encoded) else {
                return false;
            };
            raw = rest;
        }
        if package_part && !package.is_empty() {
            let Some(rest) = raw.strip_prefix(b"/") else {
                return false;
            };
            raw = rest;
        }
    }
    raw.is_empty()
}

/// Attach a single exact central-name candidate using a unique two-byte SourceFile
/// attribute and the class package (or supported pinned Metadata.pn override).
/// Duplicate exact names are ambiguous, even if their payloads are identical.
/// No normalization, suffix search, archive order winner or filesystem access.
/// Method 0 borrows the verified artifact; method 8 borrows reusable scratch.
/// Scratch is cleared on entry, including every refusal and the stored path.
///
/// Public/changed ClassFile inputs are checked at consumed references, container
/// sizes and versions. Unrelated class structure is not reverified. Attachment
/// scans have 65,535 nodes, 16 MiB attribute bytes/work and 4,096 path bytes;
/// annotation/metadata/payload decoders additionally enforce their own ceilings.
/// KotlinJvmPackageName is explicitly unsupported rather than guessing its path.
/// Success is only a source path candidate, not proof of matching compiled bytes,
/// resolved declaration identity, line mapping, or Kotlin/Java semantic analysis.
pub fn attach<'index, 'data>(
    class: &ClassFile<'_>,
    index: &'index ZipDirectoryIndex<'data>,
    scratch: &'data mut Vec<u8>,
) -> Result<SourceCandidate<'index, 'data>> {
    scratch.clear();
    if class.constants.is_empty() || class.constants.len() > MAX_RECORDS {
        return Err(AttachmentError::Class(Error::Malformed(
            "source constant pool size",
        )));
    }
    let mut nodes = 0;
    let mut work = 0;
    let mut bytes = 0;
    charge(&mut nodes, class.fields.len(), MAX_RECORDS, "nodes")?;
    charge(&mut nodes, class.methods.len(), MAX_RECORDS, "nodes")?;
    let mut source = None;
    for (attrs, is_class) in std::iter::once((&class.attributes, true)).chain(
        class
            .fields
            .iter()
            .chain(&class.methods)
            .map(|m| (&m.attributes, false)),
    ) {
        charge(&mut nodes, attrs.len(), MAX_RECORDS, "nodes")?;
        for attr in attrs {
            charge(&mut bytes, attr.bytes.len(), MAX_BYTES, "attribute bytes")?;
            let name = text(class, attr.name, &mut work)?;
            if crate::eq(name, "SourceFile") {
                if !is_class || source.is_some() || attr.bytes.len() != 2 {
                    return Err(AttachmentError::Class(Error::Malformed(
                        "SourceFile attribute",
                    )));
                }
                let cp = u16::from_be_bytes([attr.bytes[0], attr.bytes[1]]);
                source = Some(text(class, cp, &mut work)?);
            }
        }
    }
    let annotations = annotations::decode(class).map_err(AttachmentError::Class)?;
    for attrs in std::iter::once(&annotations.class)
        .chain(annotations.fields.iter())
        .chain(annotations.methods.iter())
    {
        for attr in attrs {
            for annotation in &attr.annotations {
                charge(&mut work, annotation.descriptor.len(), MAX_WORK, "work")?;
                if crate::eq(annotation.descriptor, "Lkotlin/jvm/JvmPackageName;")
                    || crate::eq(annotation.descriptor, "Lkotlin/jvm/KotlinJvmPackageName;")
                {
                    return Err(AttachmentError::Unsupported("KotlinJvmPackageName"));
                }
            }
        }
    }
    let source = source.ok_or(AttachmentError::MissingSourceFile)?;
    let file_bytes = path_bytes(source, None)?;
    let name_index = match class.constants.get(usize::from(class.this_class)) {
        Some(Constant::Class(name)) if class.this_class != 0 => *name,
        _ => {
            return Err(AttachmentError::Class(Error::Malformed(
                "source this_class",
            )));
        }
    };
    let name = text(class, name_index, &mut work)?;
    path_bytes(name, Some('/'))?;
    let package_end = name
        .iter()
        .rposition(|&c| c == u16::from(b'/'))
        .unwrap_or(0);
    let mut package = &name[..package_end];
    let mut separator = '/';
    match metadata::decode(&annotations, metadata::Limits::default())
        .map_err(AttachmentError::Metadata)?
    {
        metadata::Outcome::Absent => {}
        metadata::Outcome::Unsupported { reason, .. } => {
            return Err(AttachmentError::UnsupportedMetadata(reason));
        }
        metadata::Outcome::Decoded { header, .. } => {
            if !header.pn.is_empty() {
                package = header.pn;
                separator = '.';
            }
        }
    }
    let package_bytes = if package.is_empty() {
        0
    } else {
        path_bytes(package, Some(separator))? + 1
    };
    if package_bytes + file_bytes > MAX_PATH_BYTES {
        return Err(AttachmentError::Limit("path bytes"));
    }
    let mut found = None;
    for (ordinal, entry) in index.entries().iter().enumerate() {
        let raw = entry.raw_name();
        charge(&mut work, raw.len() + 1, MAX_WORK, "work")?;
        if raw.len() == package_bytes + file_bytes && matches_path(raw, package, separator, source)
        {
            if let Some(first) = found {
                return Err(AttachmentError::Ambiguous {
                    first,
                    second: ordinal,
                });
            }
            found = Some(ordinal);
        }
    }
    let ordinal = found.ok_or(AttachmentError::MissingCandidate)?;
    let entry = &index.entries()[ordinal];
    if entry.flags() & 0x0800 == 0 && !entry.raw_name().is_ascii() {
        return Err(AttachmentError::Unsupported("non-UTF8 ZIP name encoding"));
    }
    let bytes = if entry.method() == 8 {
        index
            .deflated_payload(ordinal, scratch)
            .map_err(AttachmentError::Deflated)?
    } else {
        index
            .stored_payload(ordinal)
            .map_err(AttachmentError::Stored)?
    };
    Ok(SourceCandidate {
        ordinal,
        entry,
        bytes,
    })
}
