use super::{ZipDirectoryIndex, ZipIndexError, ZipStoredError, charge, limit};
use miniz_oxide::inflate::{
    TINFLStatus,
    core::{DecompressorOxide, decompress_with_limit, inflate_flags},
};

const MAX_DECODED: usize = 8 * 1024 * 1024;
const MAX_WORK: usize = 32 * 1024 * 1024;
const CHUNK: usize = 32 * 1024;

/// Selected method-8 errors never expose names or partial decoded bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZipDeflateError {
    Payload(ZipStoredError),
    Structure(ZipIndexError),
    InvalidStream { offset: usize },
    TrailingInput { offset: usize },
    SizeMismatch { expected: usize, actual: usize },
    OutputTooLong { expected: usize },
    Allocation,
}

impl std::fmt::Display for ZipDeflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZIP deflated payload: {self:?}")
    }
}
impl std::error::Error for ZipDeflateError {}

impl ZipDirectoryIndex<'_> {
    /// Inflate a selected ordinal into caller-reusable scratch, without extraction.
    /// Only flags 0/0x0800 and bit 3 are supported. Bit 3 requires zero local
    /// CRC/sizes and exactly one matching signed/unsigned non-ZIP64 descriptor.
    /// Compressed input and decoded output are independently capped at 8 MiB;
    /// work is capped per call before scratch growth. Every failure empties scratch.
    pub fn deflated_payload<'s>(
        &self,
        ordinal: usize,
        scratch: &'s mut Vec<u8>,
    ) -> Result<&'s [u8], ZipDeflateError> {
        scratch.clear();
        if let Err(error) = self.inflate_selected(ordinal, scratch) {
            scratch.clear();
            return Err(error);
        }
        Ok(scratch)
    }

    fn inflate_selected(
        &self,
        ordinal: usize,
        scratch: &mut Vec<u8>,
    ) -> Result<(), ZipDeflateError> {
        let (input, offset) = self
            .selected_payload(ordinal, 8)
            .map_err(ZipDeflateError::Payload)?;
        let expected = usize::try_from(self.entries[ordinal].uncompressed_size).map_err(|_| {
            ZipDeflateError::Structure(ZipIndexError::Limit {
                offset,
                field: "decoded payload bytes",
                limit: MAX_DECODED,
            })
        })?;
        limit(expected, MAX_DECODED, offset, "decoded payload bytes")
            .map_err(ZipDeflateError::Structure)?;
        // Input bounds limit empty-block/header work independently of output.
        // Reserve the full upper bound for input, output and its CRC before growth.
        let mut work = 47 * self.entries.len() + super::MAX_NAME + super::MAX_EXTRA;
        for amount in [input.len(), expected, expected] {
            charge(&mut work, amount, MAX_WORK, offset, "inflate work units")
                .map_err(ZipDeflateError::Structure)?;
        }
        scratch
            .try_reserve_exact(expected)
            .map_err(|_| ZipDeflateError::Allocation)?;
        scratch.resize(expected, 0);
        let mut state = DecompressorOxide::new();
        let mut consumed = 0;
        let mut produced = 0;
        loop {
            let end = (consumed + CHUNK).min(input.len());
            let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
                | if end < input.len() {
                    inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
                } else {
                    0
                };
            let (status, read, written) = decompress_with_limit(
                &mut state,
                &input[consumed..end],
                scratch,
                produced,
                CHUNK,
                flags,
            );
            consumed += read;
            produced += written;
            match status {
                TINFLStatus::Done => {
                    if consumed != input.len() {
                        return Err(ZipDeflateError::TrailingInput {
                            offset: offset + consumed,
                        });
                    }
                    if produced != expected {
                        return Err(ZipDeflateError::SizeMismatch {
                            expected,
                            actual: produced,
                        });
                    }
                    break;
                }
                TINFLStatus::HasMoreOutput | TINFLStatus::NeedsMoreInput => {
                    if read == 0 && written == 0 {
                        return Err(if produced == expected {
                            ZipDeflateError::OutputTooLong { expected }
                        } else {
                            ZipDeflateError::InvalidStream {
                                offset: offset + consumed,
                            }
                        });
                    }
                }
                _ => {
                    return Err(ZipDeflateError::InvalidStream {
                        offset: offset + consumed,
                    });
                }
            }
        }
        self.check_crc(ordinal, offset, scratch)
            .map_err(ZipDeflateError::Payload)
    }
}
