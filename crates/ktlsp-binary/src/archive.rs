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
