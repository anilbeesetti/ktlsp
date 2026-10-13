use crate::{MAX_ARTIFACT_BYTES, VerifiedArtifact};

const MAX_DIRECTORY: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;
const MAX_NAME: usize = 4096;
const MAX_EXTRA: usize = 16_384;
const MAX_COMMENT: usize = 16_384;
const MAX_NAMES: usize = 4 * 1024 * 1024;
const MAX_EXTRAS: usize = 4 * 1024 * 1024;
const MAX_COMMENTS: usize = 1024 * 1024;
const MAX_TLVS: usize = 65_536;
const MAX_METADATA: usize = 2 * 1024 * 1024;
const MAX_WORK: usize = 16 * 1024 * 1024;
const EOCD: &[u8] = b"PK\x05\x06";
const CENTRAL: &[u8] = b"PK\x01\x02";
const LOCATOR: &[u8] = b"PK\x06\x07";

/// Ordered directory metadata borrowing retained verified bytes, not validated
/// payloads or filesystem paths. See ZIP_INDEX.md for policies and hard limits.
///
/// ```compile_fail
/// use ktlsp_artifacts::index_zip_directory;
/// index_zip_directory(&b"arbitrary bytes"[..]);
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{Artifact, index_zip_directory};
/// fn unverified(artifact: &Artifact) { index_zip_directory(artifact); }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{VerifiedArtifact, index_zip_directory};
/// fn mutate(artifact: &VerifiedArtifact) {
///     let mut index = index_zip_directory(artifact).unwrap();
///     index.entries()[0].raw_name()[0] = b'x';
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::ZipDirectoryIndex;
/// let index = ZipDirectoryIndex { entries: vec![], archive_comment: &[] };
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{VerifiedArtifact, ZipDirectoryIndex, index_zip_directory};
/// fn escape(artifact: VerifiedArtifact) -> ZipDirectoryIndex<'static> {
///     index_zip_directory(&artifact).unwrap()
/// }
/// ```
#[derive(Debug)]
pub struct ZipDirectoryIndex<'a> {
    entries: Vec<ZipDirectoryEntry<'a>>,
    archive_comment: &'a [u8],
}

impl<'a> ZipDirectoryIndex<'a> {
    pub fn entries(&self) -> &[ZipDirectoryEntry<'a>] {
        &self.entries
    }

    pub fn archive_comment(&self) -> &'a [u8] {
        self.archive_comment
    }
}

/// Central-record claims only. Duplicate names and unsafe path spellings are
/// preserved verbatim; no payload range or duplicate winner is provided.
#[derive(Debug)]
pub struct ZipDirectoryEntry<'a> {
    raw_name: &'a [u8],
    raw_extra: &'a [u8],
    raw_comment: &'a [u8],
    flags: u16,
    method: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
}

impl<'a> ZipDirectoryEntry<'a> {
    pub fn raw_name(&self) -> &'a [u8] {
        self.raw_name
    }
    pub fn raw_extra(&self) -> &'a [u8] {
        self.raw_extra
    }
    pub fn raw_comment(&self) -> &'a [u8] {
        self.raw_comment
    }
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn method(&self) -> u16 {
        self.method
    }
    pub fn claimed_crc32(&self) -> u32 {
        self.crc32
    }
    pub fn claimed_compressed_size(&self) -> u32 {
        self.compressed_size
    }
    pub fn claimed_uncompressed_size(&self) -> u32 {
        self.uncompressed_size
    }
    pub fn claimed_local_header_offset(&self) -> u32 {
        self.local_header_offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZipIndexError {
    Malformed {
        offset: usize,
        reason: &'static str,
    },
    AmbiguousEocd,
    UnsupportedZip64 {
        offset: usize,
    },
    UnsupportedMultidisk {
        offset: usize,
    },
    UnsupportedRecord {
        offset: usize,
    },
    UnsupportedMethod {
        offset: usize,
        method: u16,
    },
    UnsupportedVersion {
        offset: usize,
        version: u16,
    },
    UnsupportedFlags {
        offset: usize,
        flags: u16,
    },
    UnsupportedEncryption {
        offset: usize,
    },
    InvalidUtf8 {
        offset: usize,
        field: &'static str,
    },
    Limit {
        offset: usize,
        field: &'static str,
        limit: usize,
    },
    Allocation,
}

impl std::fmt::Display for ZipIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZIP directory index: {self:?}")
    }
}

impl std::error::Error for ZipIndexError {}

fn malformed(offset: usize, reason: &'static str) -> ZipIndexError {
    ZipIndexError::Malformed { offset, reason }
}

fn limit(
    value: usize,
    cap: usize,
    offset: usize,
    field: &'static str,
) -> Result<(), ZipIndexError> {
    if value > cap {
        return Err(ZipIndexError::Limit {
            offset,
            field,
            limit: cap,
        });
    }
    Ok(())
}

fn charge(
    total: &mut usize,
    amount: usize,
    cap: usize,
    offset: usize,
    field: &'static str,
) -> Result<(), ZipIndexError> {
    let next = total.checked_add(amount).ok_or(ZipIndexError::Limit {
        offset,
        field,
        limit: cap,
    })?;
    limit(next, cap, offset, field)?;
    *total = next;
    Ok(())
}

// Callers first obtain a complete fixed header or TLV header from a bounded slice.
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn metadata_bytes(count: usize) -> Result<usize, ZipIndexError> {
    let size = count
        .checked_mul(std::mem::size_of::<ZipDirectoryEntry<'_>>())
        .ok_or(ZipIndexError::Limit {
            offset: 0,
            field: "metadata bytes",
            limit: MAX_METADATA,
        })?;
    limit(size, MAX_METADATA, 0, "metadata bytes")?;
    Ok(size)
}

/// Index the selected directory once; never read a path or interpret payloads.
/// Any unsupported entry rejects the whole index. Limits are fixed, and errors
/// never return a partial index or include untrusted names/comments.
pub fn index_zip_directory(
    artifact: &VerifiedArtifact,
) -> Result<ZipDirectoryIndex<'_>, ZipIndexError> {
    let bytes = artifact.bytes();
    limit(bytes.len(), MAX_ARTIFACT_BYTES, 0, "artifact bytes")?;
    let last = bytes
        .len()
        .checked_sub(22)
        .ok_or_else(|| malformed(0, "missing EOCD"))?;
    let mut work = 0;
    let mut selected = None;
    for at in bytes.len().saturating_sub(22 + 65_535)..=last {
        charge(&mut work, 1, MAX_WORK, at, "work units")?;
        if &bytes[at..at + 4] == EOCD
            && at
                .checked_add(22)
                .and_then(|n| n.checked_add(usize::from(u16_at(&bytes[at..at + 22], 20))))
                == Some(bytes.len())
            && selected.replace(at).is_some()
        {
            return Err(ZipIndexError::AmbiguousEocd);
        }
    }
    let eocd = selected.ok_or_else(|| malformed(last, "missing EOCD"))?;
    let end = &bytes[eocd..eocd + 22];
    let disk = u16_at(end, 4);
    let start_disk = u16_at(end, 6);
    let disk_count = u16_at(end, 8);
    let count = u16_at(end, 10);
    let size = u32_at(end, 12);
    let start = u32_at(end, 16);
    if [disk, start_disk, disk_count, count].contains(&u16::MAX)
        || size == u32::MAX
        || start == u32::MAX
        || eocd
            .checked_sub(20)
            .is_some_and(|at| &bytes[at..at + 4] == LOCATOR)
    {
        return Err(ZipIndexError::UnsupportedZip64 { offset: eocd });
    }
    if disk != 0 || start_disk != 0 {
        return Err(ZipIndexError::UnsupportedMultidisk { offset: eocd });
    }
    if disk_count != count {
        return Err(malformed(eocd + 8, "entry counts disagree"));
    }
    let count = usize::from(count);
    let size =
        usize::try_from(size).map_err(|_| malformed(eocd + 12, "directory size conversion"))?;
    let start =
        usize::try_from(start).map_err(|_| malformed(eocd + 16, "directory offset conversion"))?;
    if start.checked_add(size) != Some(eocd) {
        return Err(malformed(eocd + 16, "directory span must end at EOCD"));
    }
    limit(count, MAX_ENTRIES, eocd + 10, "entries")?;
    limit(size, MAX_DIRECTORY, eocd + 12, "directory bytes")?;
    if count.checked_mul(47).is_none_or(|minimum| minimum > size) || (count == 0) != (size == 0) {
        return Err(malformed(
            eocd + 10,
            "impossible entry count and directory size",
        ));
    }
    let table = bytes
        .get(start..eocd)
        .ok_or_else(|| malformed(start, "directory span"))?;
    charge(&mut work, size, MAX_WORK, start, "work units")?;
    metadata_bytes(count)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| ZipIndexError::Allocation)?;
    let mut cursor = 0usize;
    let mut names = 0;
    let mut extras = 0;
    let mut comments = bytes.len() - eocd - 22;
    let mut tlvs = 0;
    for _ in 0..count {
        let offset = start + cursor;
        let fixed_end = cursor
            .checked_add(46)
            .ok_or_else(|| malformed(offset, "central header overflow"))?;
        let fixed = table
            .get(cursor..fixed_end)
            .ok_or_else(|| malformed(offset, "truncated central header"))?;
        if &fixed[..4] != CENTRAL {
            return Err(
                if matches!(
                    &fixed[..4],
                    b"PK\x05\x05" | b"PK\x06\x08" | b"PK\x06\x06" | b"PK\x06\x07"
                ) {
                    ZipIndexError::UnsupportedRecord { offset }
                } else {
                    malformed(offset, "central header signature")
                },
            );
        }
        let version = u16_at(fixed, 6);
        let flags = u16_at(fixed, 8);
        let method = u16_at(fixed, 10);
        let compressed_size = u32_at(fixed, 20);
        let uncompressed_size = u32_at(fixed, 24);
        let entry_disk = u16_at(fixed, 34);
        let local_header_offset = u32_at(fixed, 42);
        if compressed_size == u32::MAX
            || uncompressed_size == u32::MAX
            || entry_disk == u16::MAX
            || local_header_offset == u32::MAX
        {
            return Err(ZipIndexError::UnsupportedZip64 { offset });
        }
        if entry_disk != 0 {
            return Err(ZipIndexError::UnsupportedMultidisk {
                offset: offset + 34,
            });
        }
        if !matches!(method, 0 | 8) {
            return Err(ZipIndexError::UnsupportedMethod {
                offset: offset + 10,
                method,
            });
        }
        if !(10..=20).contains(&version) {
            return Err(ZipIndexError::UnsupportedVersion {
                offset: offset + 6,
                version,
            });
        }
        if flags & ((1 << 0) | (1 << 6) | (1 << 13)) != 0 {
            return Err(ZipIndexError::UnsupportedEncryption { offset: offset + 8 });
        }
        let allowed = (1 << 3) | (1 << 11) | if method == 8 { (1 << 1) | (1 << 2) } else { 0 };
        if flags & !allowed != 0 {
            return Err(ZipIndexError::UnsupportedFlags {
                offset: offset + 8,
                flags,
            });
        }
        if u64::from(local_header_offset) >= start as u64 {
            return Err(malformed(
                offset + 42,
                "claimed local header must precede directory",
            ));
        }
        let name_len = usize::from(u16_at(fixed, 28));
        let extra_len = usize::from(u16_at(fixed, 30));
        let comment_len = usize::from(u16_at(fixed, 32));
        let record_end = fixed_end
            .checked_add(name_len)
            .and_then(|n| n.checked_add(extra_len))
            .and_then(|n| n.checked_add(comment_len))
            .ok_or_else(|| malformed(offset, "central record overflow"))?;
        let variable = table
            .get(fixed_end..record_end)
            .ok_or_else(|| malformed(offset, "truncated central variable fields"))?;
        if name_len == 0 {
            return Err(malformed(offset + 28, "empty name"));
        }
        limit(name_len, MAX_NAME, offset, "name bytes")?;
        limit(extra_len, MAX_EXTRA, offset, "extra bytes")?;
        limit(comment_len, MAX_COMMENT, offset, "file comment bytes")?;
        charge(
            &mut names,
            name_len,
            MAX_NAMES,
            offset,
            "aggregate name bytes",
        )?;
        charge(
            &mut extras,
            extra_len,
            MAX_EXTRAS,
            offset,
            "aggregate extra bytes",
        )?;
        charge(
            &mut comments,
            comment_len,
            MAX_COMMENTS,
            offset,
            "aggregate comment bytes",
        )?;
        let raw_name = &variable[..name_len];
        let raw_extra = &variable[name_len..name_len + extra_len];
        let raw_comment = &variable[name_len + extra_len..];
        let mut extra_cursor = 0usize;
        while extra_cursor < raw_extra.len() {
            let extra_offset = offset + 46 + name_len + extra_cursor;
            charge(&mut tlvs, 1, MAX_TLVS, extra_offset, "extra TLVs")?;
            charge(&mut work, 1, MAX_WORK, extra_offset, "work units")?;
            let header = raw_extra
                .get(extra_cursor..extra_cursor + 4)
                .ok_or_else(|| malformed(extra_offset, "truncated extra header"))?;
            let tag = u16_at(header, 0);
            let length = usize::from(u16_at(header, 2));
            let next = extra_cursor
                .checked_add(4)
                .and_then(|n| n.checked_add(length))
                .filter(|&n| n <= raw_extra.len())
                .ok_or_else(|| malformed(extra_offset, "truncated extra body"))?;
            if tag == 1 {
                return Err(ZipIndexError::UnsupportedZip64 {
                    offset: extra_offset,
                });
            }
            if matches!(tag, 0x0017 | 0x0019 | 0x9901) {
                return Err(ZipIndexError::UnsupportedEncryption {
                    offset: extra_offset,
                });
            }
            extra_cursor = next;
        }
        if flags & (1 << 11) != 0 {
            for (text, field, text_offset) in [
                (raw_name, "name", offset + 46),
                (
                    raw_comment,
                    "file comment",
                    offset + 46 + name_len + extra_len,
                ),
            ] {
                charge(&mut work, text.len(), MAX_WORK, text_offset, "work units")?;
                std::str::from_utf8(text).map_err(|_| ZipIndexError::InvalidUtf8 {
                    offset: text_offset,
                    field,
                })?;
            }
        }
        entries.push(ZipDirectoryEntry {
            raw_name,
            raw_extra,
            raw_comment,
            flags,
            method,
            crc32: u32_at(fixed, 16),
            compressed_size,
            uncompressed_size,
            local_header_offset,
        });
        cursor = record_end;
    }
    if cursor != table.len() {
        let offset = start + cursor;
        return Err(
            if table.get(cursor..cursor + 4).is_some_and(|signature| {
                matches!(
                    signature,
                    b"PK\x05\x05" | b"PK\x06\x08" | b"PK\x06\x06" | b"PK\x06\x07"
                )
            }) {
                ZipIndexError::UnsupportedRecord { offset }
            } else {
                malformed(offset, "unconsumed directory bytes")
            },
        );
    }
    Ok(ZipDirectoryIndex {
        entries,
        archive_comment: &bytes[eocd + 22..],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_accounting_boundaries_are_atomic() {
        for (cap, field) in [
            (MAX_WORK, "work units"),
            (MAX_NAMES, "aggregate name bytes"),
            (MAX_EXTRAS, "aggregate extra bytes"),
            (MAX_COMMENTS, "aggregate comment bytes"),
            (MAX_TLVS, "extra TLVs"),
        ] {
            let mut total = cap - 1;
            charge(&mut total, 1, cap, 7, field).unwrap();
            assert_eq!(total, cap);
            assert_eq!(
                charge(&mut total, 1, cap, 7, field),
                Err(ZipIndexError::Limit {
                    offset: 7,
                    field,
                    limit: cap
                })
            );
            assert_eq!(total, cap);
            assert!(charge(&mut total, usize::MAX, cap, 7, field).is_err());
            assert_eq!(total, cap);
        }
        let capacity = MAX_METADATA / std::mem::size_of::<ZipDirectoryEntry<'_>>();
        assert!(metadata_bytes(capacity).is_ok());
        assert!(metadata_bytes(capacity + 1).is_err());
        assert!(metadata_bytes(usize::MAX).is_err());
        assert!(metadata_bytes(MAX_ENTRIES).is_ok());
        const { assert!(65_536 + MAX_DIRECTORY + MAX_NAMES + MAX_COMMENTS + MAX_TLVS < MAX_WORK) };
    }
}
