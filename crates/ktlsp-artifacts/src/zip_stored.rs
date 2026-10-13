use super::{
    MAX_EXTRA, MAX_NAME, ZipDirectoryIndex, ZipIndexError, charge, limit, malformed, u16_at, u32_at,
};

const MAX_PAYLOAD: usize = 8 * 1024 * 1024;
const MAX_WORK: usize = 16 * 1024 * 1024;
const LOCAL: &[u8] = b"PK\x03\x04";

/// Selected-entry errors contain only numeric values and static labels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZipStoredError {
    InvalidOrdinal {
        ordinal: usize,
        entries: usize,
    },
    Structure(ZipIndexError),
    Disagreement {
        offset: usize,
        field: &'static str,
    },
    Overlap {
        offset: usize,
        other_ordinal: usize,
    },
    CrcMismatch {
        offset: usize,
        expected: u32,
        actual: u32,
    },
}

impl std::fmt::Display for ZipStoredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZIP stored payload: {self:?}")
    }
}

impl std::error::Error for ZipStoredError {}

impl<'a> ZipDirectoryIndex<'a> {
    /// Borrow one method-0 payload from this index's retained artifact, by
    /// central-record ordinal. Bounds, agreement and declared-envelope overlap
    /// checks precede ZIP CRC-32 verification. No names select a duplicate winner.
    ///
    /// Descriptor tails, unselected CRC/agreement and whole-archive validity
    /// remain unchecked. Limits apply to each call, not a session; see ZIP_STORED.md.
    ///
    /// ```compile_fail
    /// use ktlsp_artifacts::{ZipDirectoryIndex, VerifiedArtifact};
    /// fn mismatch(index: &ZipDirectoryIndex<'_>, other: &VerifiedArtifact) {
    ///     index.stored_payload(0, other);
    /// }
    /// ```
    /// ```compile_fail
    /// use ktlsp_artifacts::{ZipDirectoryIndex, ZipDirectoryEntry};
    /// fn arbitrary_entry(index: &ZipDirectoryIndex<'_>, entry: &ZipDirectoryEntry<'_>) {
    ///     index.stored_payload(entry);
    /// }
    /// ```
    /// ```compile_fail
    /// use ktlsp_artifacts::ZipDirectoryIndex;
    /// fn mutate(index: &ZipDirectoryIndex<'_>) {
    ///     index.stored_payload(0).unwrap()[0] = 1;
    /// }
    /// ```
    /// ```compile_fail
    /// use ktlsp_artifacts::{VerifiedArtifact, index_zip_directory};
    /// fn escape(artifact: VerifiedArtifact) -> &'static [u8] {
    ///     index_zip_directory(&artifact).unwrap().stored_payload(0).unwrap()
    /// }
    /// ```
    pub fn stored_payload(&self, ordinal: usize) -> Result<&'a [u8], ZipStoredError> {
        let entry = self
            .entries
            .get(ordinal)
            .ok_or(ZipStoredError::InvalidOrdinal {
                ordinal,
                entries: self.entries.len(),
            })?;
        let c = entry.central_offset;
        if entry.method != 0 {
            return Err(ZipStoredError::Structure(
                ZipIndexError::UnsupportedMethod {
                    offset: c + 10,
                    method: entry.method,
                },
            ));
        }
        if entry.flags & !0x0800 != 0 {
            return Err(ZipStoredError::Structure(ZipIndexError::UnsupportedFlags {
                offset: c + 8,
                flags: entry.flags,
            }));
        }
        let h = usize::try_from(entry.local_header_offset)
            .map_err(|_| ZipStoredError::Structure(malformed(c + 42, "local offset conversion")))?;
        let mut work = 0;
        let headers = self
            .entries
            .len()
            .checked_mul(31)
            .ok_or(ZipStoredError::Structure(ZipIndexError::Limit {
                offset: h,
                field: "stored work units",
                limit: MAX_WORK,
            }))?;
        charge(&mut work, headers, MAX_WORK, h, "stored work units")
            .map_err(ZipStoredError::Structure)?;
        let fixed_end = h
            .checked_add(30)
            .ok_or_else(|| ZipStoredError::Structure(malformed(h, "local span overflow")))?;
        let pretable = &self.bytes[..self.directory_start];
        let fixed = pretable
            .get(h..fixed_end)
            .ok_or_else(|| ZipStoredError::Structure(malformed(h, "truncated local header")))?;
        if &fixed[..4] != LOCAL {
            return Err(ZipStoredError::Structure(malformed(
                h,
                "local header signature",
            )));
        }
        let compressed = u32_at(fixed, 18);
        let uncompressed = u32_at(fixed, 22);
        if compressed == u32::MAX || uncompressed == u32::MAX {
            return Err(ZipStoredError::Structure(ZipIndexError::UnsupportedZip64 {
                offset: h,
            }));
        }
        let method = u16_at(fixed, 8);
        let version = u16_at(fixed, 4);
        let flags = u16_at(fixed, 6);
        if method != 0 {
            return Err(ZipStoredError::Structure(
                ZipIndexError::UnsupportedMethod {
                    offset: h + 8,
                    method,
                },
            ));
        }
        if !(10..=20).contains(&version) {
            return Err(ZipStoredError::Structure(
                ZipIndexError::UnsupportedVersion {
                    offset: h + 4,
                    version,
                },
            ));
        }
        if flags & ((1 << 0) | (1 << 6) | (1 << 13)) != 0 {
            return Err(ZipStoredError::Structure(
                ZipIndexError::UnsupportedEncryption { offset: h + 6 },
            ));
        }
        if flags & !0x0800 != 0 {
            return Err(ZipStoredError::Structure(ZipIndexError::UnsupportedFlags {
                offset: h + 6,
                flags,
            }));
        }
        for (at, field, local, central) in [
            (
                4,
                "version",
                u32::from(version),
                u32::from(entry.version_needed),
            ),
            (6, "flags", u32::from(flags), u32::from(entry.flags)),
            (8, "method", u32::from(method), u32::from(entry.method)),
            (14, "crc32", u32_at(fixed, 14), entry.crc32),
            (18, "compressed size", compressed, entry.compressed_size),
            (
                22,
                "uncompressed size",
                uncompressed,
                entry.uncompressed_size,
            ),
        ] {
            if local != central {
                return Err(ZipStoredError::Disagreement {
                    offset: h + at,
                    field,
                });
            }
        }
        if compressed != uncompressed {
            return Err(ZipStoredError::Structure(malformed(
                h + 22,
                "stored sizes differ",
            )));
        }
        let name_len = usize::from(u16_at(fixed, 26));
        let extra_len = usize::from(u16_at(fixed, 28));
        if name_len == 0 {
            return Err(ZipStoredError::Structure(malformed(h + 26, "empty name")));
        }
        limit(name_len, MAX_NAME, h + 26, "local name bytes").map_err(ZipStoredError::Structure)?;
        limit(extra_len, MAX_EXTRA, h + 28, "local extra bytes")
            .map_err(ZipStoredError::Structure)?;
        let name_end = fixed_end
            .checked_add(name_len)
            .ok_or_else(|| ZipStoredError::Structure(malformed(h, "local span overflow")))?;
        let name = pretable.get(fixed_end..name_end).ok_or_else(|| {
            ZipStoredError::Structure(malformed(fixed_end, "truncated local name"))
        })?;
        let p = name_end
            .checked_add(extra_len)
            .ok_or_else(|| ZipStoredError::Structure(malformed(h, "local span overflow")))?;
        let extra = pretable.get(name_end..p).ok_or_else(|| {
            ZipStoredError::Structure(malformed(name_end, "truncated local extra"))
        })?;
        if name_len != entry.raw_name.len() {
            return Err(ZipStoredError::Disagreement {
                offset: h + 26,
                field: "name length",
            });
        }
        charge(
            &mut work,
            name_len + extra_len,
            MAX_WORK,
            fixed_end,
            "stored work units",
        )
        .map_err(ZipStoredError::Structure)?;
        if name != entry.raw_name {
            return Err(ZipStoredError::Disagreement {
                offset: fixed_end,
                field: "name",
            });
        }
        let mut cursor = 0usize;
        while cursor < extra.len() {
            let at = name_end + cursor;
            let header = extra.get(cursor..cursor + 4).ok_or_else(|| {
                ZipStoredError::Structure(malformed(at, "truncated extra header"))
            })?;
            let tag = u16_at(header, 0);
            let end = cursor
                .checked_add(4)
                .and_then(|n| n.checked_add(usize::from(u16_at(header, 2))))
                .filter(|&n| n <= extra.len())
                .ok_or_else(|| ZipStoredError::Structure(malformed(at, "truncated extra body")))?;
            if tag == 1 {
                return Err(ZipStoredError::Structure(ZipIndexError::UnsupportedZip64 {
                    offset: at,
                }));
            }
            if matches!(tag, 0x0017 | 0x0019 | 0x9901) {
                return Err(ZipStoredError::Structure(
                    ZipIndexError::UnsupportedEncryption { offset: at },
                ));
            }
            cursor = end;
        }
        let size = usize::try_from(entry.compressed_size)
            .map_err(|_| ZipStoredError::Structure(malformed(p, "payload span overflow")))?;
        if size > self.directory_start - p {
            return Err(ZipStoredError::Structure(malformed(
                p,
                "payload crosses directory",
            )));
        }
        let q = p
            .checked_add(size)
            .ok_or_else(|| ZipStoredError::Structure(malformed(p, "payload span overflow")))?;
        limit(size, MAX_PAYLOAD, p, "stored payload bytes").map_err(ZipStoredError::Structure)?;
        for (j, other) in self.entries.iter().enumerate() {
            if j == ordinal {
                continue;
            }
            let hj = usize::try_from(other.local_header_offset).map_err(|_| {
                ZipStoredError::Structure(malformed(
                    other.central_offset + 42,
                    "competing local offset conversion",
                ))
            })?;
            let fixed_end = hj.checked_add(30).ok_or_else(|| {
                ZipStoredError::Structure(malformed(hj, "competing local span overflow"))
            })?;
            let fixed = pretable.get(hj..fixed_end).ok_or_else(|| {
                ZipStoredError::Structure(malformed(hj, "truncated competing local header"))
            })?;
            if &fixed[..4] != LOCAL {
                return Err(ZipStoredError::Structure(malformed(
                    hj,
                    "competing local header signature",
                )));
            }
            let bj = fixed_end
                .checked_add(usize::from(u16_at(fixed, 26)))
                .and_then(|n| n.checked_add(usize::from(u16_at(fixed, 28))))
                .ok_or_else(|| {
                    ZipStoredError::Structure(malformed(hj, "competing local span overflow"))
                })?;
            let size = usize::try_from(other.compressed_size).map_err(|_| {
                ZipStoredError::Structure(malformed(hj, "competing local span overflow"))
            })?;
            if bj > self.directory_start || size > self.directory_start - bj {
                return Err(ZipStoredError::Structure(malformed(
                    hj,
                    "competing local span crosses directory",
                )));
            }
            let ej = bj.checked_add(size).ok_or_else(|| {
                ZipStoredError::Structure(malformed(hj, "competing local span overflow"))
            })?;
            // shortcut: descriptor tails are outside these declared compressed envelopes; extend only with a separate descriptor contract.
            if h < ej && hj < q {
                return Err(ZipStoredError::Overlap {
                    offset: h,
                    other_ordinal: j,
                });
            }
        }
        charge(&mut work, size, MAX_WORK, p, "stored work units")
            .map_err(ZipStoredError::Structure)?;
        let payload = &self.bytes[p..q];
        let actual = crc32(payload);
        if actual != entry.crc32 {
            return Err(ZipStoredError::CrcMismatch {
                offset: p,
                expected: entry.crc32,
                actual,
            });
        }
        Ok(payload)
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_budget_is_atomic_and_overflow_checked() {
        let mut work = MAX_WORK - 1;
        charge(&mut work, 1, MAX_WORK, 7, "stored work units").unwrap();
        for amount in [1, usize::MAX] {
            assert_eq!(
                charge(&mut work, amount, MAX_WORK, 7, "stored work units"),
                Err(ZipIndexError::Limit {
                    offset: 7,
                    field: "stored work units",
                    limit: MAX_WORK
                })
            );
            assert_eq!(work, MAX_WORK);
        }
        const {
            assert!(
                31 * super::super::MAX_ENTRIES + MAX_NAME + MAX_EXTRA + MAX_PAYLOAD == 8_916_992
            );
        }
        const {
            assert!(8_916_992 < MAX_WORK);
        }
    }
}
