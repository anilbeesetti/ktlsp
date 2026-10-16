use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, IntegrityError, JmodIndexError, MAX_ARTIFACT_BYTES,
    Provenance, VerifiedArtifact, ZipIndexError, ZipStoredError, index_jmod_directory,
    index_zip_directory, verify_sha256,
};
use sha2::{Digest, Sha256};

const VALUE: &[u8] = b"123456789";
const CRC: u32 = 0xcbf4_3926;

fn artifact(bytes: Vec<u8>) -> Artifact {
    Artifact {
        provenance: Provenance {
            coordinate: Coordinate::new("test", "jmod", "1", None, "jmod").unwrap(),
            repository_root: "never-opened".into(),
            artifact_path: "never-opened\0path".into(),
            byte_length: bytes.len(),
            checksum: ChecksumStatus::Unverified,
        },
        bytes,
    }
}

fn verified(bytes: Vec<u8>) -> VerifiedArtifact {
    let digest = Sha256::digest(&bytes).into();
    verify_sha256(artifact(bytes), digest).unwrap()
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn jmod(zip: &[u8]) -> Vec<u8> {
    let mut bytes = b"JM\x01\x00".to_vec();
    bytes.extend_from_slice(zip);
    bytes
}

// Independently authored ZIP framing: stored plus raw DEFLATE with a final
// uncompressed block and signed descriptor, as data only. No archive/JDK tool.
fn zip() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    let mut deflated = vec![1, 9, 0, 0xf6, 0xff];
    deflated.extend_from_slice(VALUE);
    for (name, method, payload) in [
        (&b"classes/A.class"[..], 0u16, VALUE),
        (&b"classes/B.class"[..], 8u16, deflated.as_slice()),
    ] {
        let start = bytes.len() as u32;
        let flags = if method == 8 { 8 } else { 0 };
        let size = payload.len() as u32;
        let mut local = [0u8; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        put16(&mut local, 4, 20);
        put16(&mut local, 6, flags);
        put16(&mut local, 8, method);
        if flags == 0 {
            put32(&mut local, 14, CRC);
            put32(&mut local, 18, size);
            put32(&mut local, 22, VALUE.len() as u32);
        }
        put16(&mut local, 26, name.len() as u16);
        bytes.extend_from_slice(&local);
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(payload);
        if flags != 0 {
            bytes.extend_from_slice(b"PK\x07\x08");
            bytes.extend_from_slice(&CRC.to_le_bytes());
            bytes.extend_from_slice(&size.to_le_bytes());
            bytes.extend_from_slice(&(VALUE.len() as u32).to_le_bytes());
        }
        let mut central = [0u8; 46];
        central[..4].copy_from_slice(b"PK\x01\x02");
        put16(&mut central, 6, 20);
        put16(&mut central, 8, flags);
        put16(&mut central, 10, method);
        put32(&mut central, 16, CRC);
        put32(&mut central, 20, size);
        put32(&mut central, 24, VALUE.len() as u32);
        put16(&mut central, 28, name.len() as u16);
        put32(&mut central, 42, start);
        directory.extend_from_slice(&central);
        directory.extend_from_slice(name);
    }
    let mut end = [0u8; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    put16(&mut end, 8, 2);
    put16(&mut end, 10, 2);
    put32(&mut end, 12, directory.len() as u32);
    put32(&mut end, 16, bytes.len() as u32);
    put16(&mut end, 20, 3);
    bytes.extend(directory);
    bytes.extend_from_slice(&end);
    bytes.extend_from_slice(b"\0\xffc");
    bytes
}

#[test]
fn stored_and_deflated_borrow_correct_embedded_offsets_without_changing_zip() {
    let zip = zip();
    let plain = verified(zip.clone());
    let wrapped = verified(jmod(&zip));
    let provenance = wrapped.provenance().clone();
    let index = index_jmod_directory(&wrapped).unwrap();
    let embedded = index.zip_index();
    let ordinary = index_zip_directory(&plain).unwrap();
    assert_eq!(embedded.entries().len(), 2);
    assert_eq!(embedded.archive_comment(), b"\0\xffc");
    assert_eq!(embedded.entries()[0].claimed_local_header_offset(), 0);
    assert_eq!(embedded.entries()[1].claimed_local_header_offset(), 54);
    let payload = embedded.stored_payload(0).unwrap();
    assert_eq!(payload, VALUE);
    assert_eq!(payload.as_ptr(), wrapped.bytes()[49..].as_ptr());
    assert_eq!(
        embedded.entries()[0].raw_name().as_ptr(),
        wrapped.bytes()[179..].as_ptr()
    );
    let mut scratch = vec![0xff; 100];
    assert_eq!(embedded.deflated_payload(1, &mut scratch).unwrap(), VALUE);
    assert_eq!(scratch, VALUE);
    assert_eq!(ordinary.stored_payload(0).unwrap(), payload);
    assert_eq!(ordinary.deflated_payload(1, &mut scratch).unwrap(), VALUE);
    assert_eq!(format!("{embedded:?}"), format!("{ordinary:?}"));
    assert_eq!(wrapped.provenance(), &provenance);
    // Ordinary ZIP has no implicit JMOD/SFX offset correction.
    assert!(index_zip_directory(&wrapped).is_err());
}

#[test]
fn header_versions_prefixes_and_every_truncation_are_refused() {
    let bytes = jmod(&zip());
    for len in 1..bytes.len() {
        let input = verified(bytes[..len].to_vec());
        assert!(index_jmod_directory(&input).is_err(), "prefix {len}");
        if len < 4 {
            assert_eq!(
                index_jmod_directory(&input).unwrap_err(),
                JmodIndexError::TruncatedHeader
            );
        }
    }
    for (major, minor) in [(0, 0), (1, 1), (2, 0), (255, 255)] {
        let mut changed = bytes.clone();
        changed[2] = major;
        changed[3] = minor;
        assert_eq!(
            index_jmod_directory(&verified(changed)).unwrap_err(),
            JmodIndexError::UnsupportedVersion { major, minor }
        );
    }
    for prefix in [&b"x"[..], &b"PK\x03\x04"[..], &b"JM\x01\x00"[..]] {
        let mut changed = prefix.to_vec();
        changed.extend_from_slice(&bytes);
        assert!(index_jmod_directory(&verified(changed)).is_err());
    }
    for position in [0, 1] {
        let mut changed = bytes.clone();
        changed[position] ^= 0xff;
        assert_eq!(
            index_jmod_directory(&verified(changed)).unwrap_err(),
            JmodIndexError::InvalidMagic
        );
    }
    assert_eq!(
        index_jmod_directory(&verified(zip())).unwrap_err(),
        JmodIndexError::InvalidMagic
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(index_jmod_directory(&verified(trailing)).is_err());
    let mut concatenated = bytes.clone();
    concatenated.extend_from_slice(&bytes);
    assert!(index_jmod_directory(&verified(concatenated)).is_err());
}

#[test]
fn shared_index_refusals_and_payload_corruption_keep_zip_relative_offsets() {
    let original = zip();
    let eocd = original.len() - 25;
    let central = 129;
    // Disk, ZIP64, method, EOCD directory offset, and central local offset.
    for (at, replacement) in [
        (eocd + 4, vec![1, 0]),
        (eocd + 10, vec![255, 255]),
        (central + 10, vec![99, 0]),
        (eocd + 16, vec![0, 0, 0, 0]),
        (central + 42, vec![255, 255, 255, 255]),
    ] {
        let mut changed = original.clone();
        changed[at..at + replacement.len()].copy_from_slice(&replacement);
        let ordinary = index_zip_directory(&verified(changed.clone())).unwrap_err();
        assert_eq!(
            index_jmod_directory(&verified(jmod(&changed))).unwrap_err(),
            JmodIndexError::Structure(ordinary)
        );
    }
    for at in [45, 14, 99, 117] {
        let mut changed = original.clone();
        changed[at] ^= 1;
        let plain = verified(changed.clone());
        let wrapped = verified(jmod(&changed));
        let ordinary = index_zip_directory(&plain).unwrap();
        let index = index_jmod_directory(&wrapped).unwrap();
        if at < 54 {
            assert_eq!(
                index.zip_index().stored_payload(0).unwrap_err(),
                ordinary.stored_payload(0).unwrap_err()
            );
        } else {
            assert_eq!(
                index
                    .zip_index()
                    .deflated_payload(1, &mut Vec::new())
                    .unwrap_err(),
                ordinary.deflated_payload(1, &mut Vec::new()).unwrap_err()
            );
        }
    }
    let mut changed = original.clone();
    changed[45] ^= 1;
    assert!(matches!(
        index_jmod_directory(&verified(jmod(&changed)))
            .unwrap()
            .zip_index()
            .stored_payload(0),
        Err(ZipStoredError::CrcMismatch { offset: 45, .. })
    ));
    // Encoding offsets from the full file instead of the embedded ZIP is invalid.
    let mut changed = original;
    put32(&mut changed, eocd + 16, (central + 4) as u32);
    assert!(index_jmod_directory(&verified(jmod(&changed))).is_err());
}

fn empty_zip(total_len: usize) -> Vec<u8> {
    let mut bytes = vec![0; total_len];
    let end = total_len - 22;
    bytes[end..end + 4].copy_from_slice(b"PK\x05\x06");
    put32(&mut bytes, end + 16, end as u32);
    bytes
}

#[test]
fn empty_comments_and_full_artifact_boundary_preserve_existing_caps() {
    let empty = verified(jmod(&empty_zip(22)));
    assert!(
        index_jmod_directory(&empty)
            .unwrap()
            .zip_index()
            .entries()
            .is_empty()
    );
    let mut comment = empty_zip(22);
    put16(&mut comment, 20, u16::MAX);
    comment.resize(22 + u16::MAX as usize, b'x');
    let comment = verified(jmod(&comment));
    assert_eq!(
        index_jmod_directory(&comment)
            .unwrap()
            .zip_index()
            .archive_comment()
            .len(),
        65_535
    );
    let at_cap = verified(jmod(&empty_zip(MAX_ARTIFACT_BYTES - 4)));
    assert_eq!(at_cap.bytes().len(), MAX_ARTIFACT_BYTES);
    assert!(
        index_jmod_directory(&at_cap)
            .unwrap()
            .zip_index()
            .entries()
            .is_empty()
    );
    let mut over = artifact(Vec::from(at_cap.bytes()));
    over.bytes.push(0);
    over.provenance.byte_length += 1;
    assert!(matches!(
        verify_sha256(over, [0; 32]),
        Err(IntegrityError::TooLarge {
            limit: MAX_ARTIFACT_BYTES,
            ..
        })
    ));
}

#[test]
fn directory_entry_and_ambiguity_limits_are_shared() {
    for count in [16_384u16, 16_385] {
        let mut bytes = vec![0];
        let mut record = vec![0; 47];
        record[..4].copy_from_slice(b"PK\x01\x02");
        put16(&mut record, 6, 20);
        put16(&mut record, 28, 1);
        record[46] = b'a';
        for _ in 0..count {
            bytes.extend_from_slice(&record);
        }
        let mut end = [0; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        put16(&mut end, 8, count);
        put16(&mut end, 10, count);
        put32(&mut end, 12, u32::from(count) * 47);
        put32(&mut end, 16, 1);
        bytes.extend_from_slice(&end);
        let wrapped = verified(jmod(&bytes));
        let result = index_jmod_directory(&wrapped);
        if count == 16_384 {
            assert_eq!(
                result.unwrap().zip_index().entries().len(),
                usize::from(count)
            );
        } else {
            assert!(matches!(
                result,
                Err(JmodIndexError::Structure(ZipIndexError::Limit {
                    field: "entries",
                    limit: 16_384,
                    ..
                }))
            ));
        }
    }
    for size in [8 * 1024 * 1024, 8 * 1024 * 1024 + 1] {
        let mut bytes = vec![0; size + 23];
        let eocd = size + 1;
        bytes[eocd..eocd + 4].copy_from_slice(b"PK\x05\x06");
        put16(&mut bytes, eocd + 8, 1);
        put16(&mut bytes, eocd + 10, 1);
        put32(&mut bytes, eocd + 12, size as u32);
        put32(&mut bytes, eocd + 16, 1);
        let error = index_jmod_directory(&verified(jmod(&bytes))).unwrap_err();
        if size == 8 * 1024 * 1024 {
            assert!(matches!(
                error,
                JmodIndexError::Structure(ZipIndexError::Malformed { offset: 1, .. })
            ));
        } else {
            assert!(matches!(
                error,
                JmodIndexError::Structure(ZipIndexError::Limit {
                    field: "directory bytes",
                    ..
                })
            ));
        }
    }
    let mut ambiguous = empty_zip(22);
    put16(&mut ambiguous, 20, 22);
    ambiguous.extend_from_slice(&empty_zip(22));
    assert_eq!(
        index_jmod_directory(&verified(jmod(&ambiguous))).unwrap_err(),
        JmodIndexError::Structure(ZipIndexError::AmbiguousEocd)
    );
}
