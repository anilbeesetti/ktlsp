//! Selected ordinal classfile consumption, not JAR name/version/signature policy.

use crate::{ClassFile, Error, parse_class};
use ktlsp_artifacts::{ZipDeflateError, ZipDirectoryIndex, ZipStoredError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZipClassError {
    Stored(ZipStoredError),
    Deflated(ZipDeflateError),
    Class(Error),
}

impl std::fmt::Display for ZipClassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZIP class: {self:?}")
    }
}
impl std::error::Error for ZipClassError {}

/// Parse exactly one caller-selected central ordinal. Method 0 borrows retained
/// archive bytes; method 8 borrows caller scratch. No entry-name winner, fallback
/// selection, extraction, network, multi-release or JAR signature interpretation.
/// Scratch is cleared even on the stored path; do not reuse it while the class
/// borrows it. A classfile error is returned unchanged inside `Class`.
pub fn parse_zip_class<'a>(
    index: &ZipDirectoryIndex<'a>,
    ordinal: usize,
    scratch: &'a mut Vec<u8>,
) -> Result<ClassFile<'a>, ZipClassError> {
    scratch.clear();
    let payload = if index
        .entries()
        .get(ordinal)
        .is_some_and(|entry| entry.method() == 8)
    {
        index
            .deflated_payload(ordinal, scratch)
            .map_err(ZipClassError::Deflated)?
    } else {
        index
            .stored_payload(ordinal)
            .map_err(ZipClassError::Stored)?
    };
    parse_class(payload).map_err(ZipClassError::Class)
}

/// Selected JImage class failures preserve image/codec and class-reader boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JimageClassError {
    Payload(ktlsp_artifacts::JimageError),
    NotClassResource,
    DeclaredName,
    Class(Error),
}
impl std::fmt::Display for JimageClassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JImage class: {self:?}")
    }
}
impl std::error::Error for JimageClassError {}

/// Parse exactly the origin-bound selected resource, comparing this_class to its
/// module-relative MUTF-8 path. No SDK/root resolution, fallback or second artifact.
/// Every failure clears scratch, including class/name failures after decompression.
pub fn parse_jimage_class<'s>(
    resource: &'s ktlsp_artifacts::JimageResource<'_, '_>,
    scratch: &'s mut Vec<u8>,
) -> Result<ClassFile<'s>, JimageClassError> {
    scratch.clear();
    let name = resource
        .resource_mutf8()
        .strip_suffix(b".class")
        .filter(|name| !name.is_empty())
        .ok_or(JimageClassError::NotClassResource)?;
    // A returned ClassFile borrows payload bytes. A normal &mut cleanup guard
    // would retain an exclusive borrow across that return. This private guard
    // only touches the Vec on failure, after all later-declared class/payload
    // borrows have ceased. It is disarmed before any borrowed result escapes.
    struct ClearOnFailure {
        scratch: *mut Vec<u8>,
        armed: bool,
    }
    impl Drop for ClearOnFailure {
        fn drop(&mut self) {
            if self.armed {
                // SAFETY: constructed from this call's live exclusive scratch;
                // on error no class or payload reference escapes, and locals
                // holding them are dropped before this guard. Success disarms it.
                unsafe {
                    (*self.scratch).clear();
                }
            }
        }
    }
    let mut cleanup = ClearOnFailure {
        scratch,
        armed: true,
    };
    // SAFETY: derive the sole payload reborrow from the saved pointer itself,
    // not a sibling reborrow of the original &mut (which could invalidate the
    // saved pointer's provenance). Caller scratch stays live for 's; cleanup
    // only writes after this borrow ceases on failure, never on success.
    let payload = resource
        .payload(unsafe { &mut *cleanup.scratch })
        .map_err(JimageClassError::Payload)?;
    let class = parse_class(payload).map_err(JimageClassError::Class)?;
    let crate::Constant::Class(declared) = class.constants[usize::from(class.this_class)] else {
        return Err(JimageClassError::DeclaredName);
    };
    let units = class.utf16(declared).map_err(JimageClassError::Class)?;
    // Encode existing lossless UTF-16 directly, with no UTF-8 normalization,
    // temporary allocation, replacement character or supplementary-name loss.
    let mut at = 0;
    for &unit in units {
        let (encoded, len) = if (1..=127).contains(&unit) {
            ([unit as u8, 0, 0], 1)
        } else if unit < 2048 {
            (
                [(0xc0 | (unit >> 6)) as u8, (0x80 | (unit & 63)) as u8, 0],
                2,
            )
        } else {
            (
                [
                    (0xe0 | (unit >> 12)) as u8,
                    (0x80 | ((unit >> 6) & 63)) as u8,
                    (0x80 | (unit & 63)) as u8,
                ],
                3,
            )
        };
        if name.get(at..at + len) != Some(&encoded[..len]) {
            return Err(JimageClassError::DeclaredName);
        }
        at += len;
    }
    if at != name.len() {
        return Err(JimageClassError::DeclaredName);
    }
    cleanup.armed = false;
    Ok(class)
}
