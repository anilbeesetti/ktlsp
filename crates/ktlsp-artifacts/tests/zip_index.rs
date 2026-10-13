use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, ZipIndexError,
    index_zip_directory, verify_sha256,
};
use sha2::{Digest, Sha256};

// Independently authored byte layouts; offsets/expectations follow APPNOTE, not
// the production reader. Pretable byte 0 deliberately is not a local header.
fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn record(name: &[u8], extra: &[u8], comment: &[u8], method: u16, flags: u16) -> Vec<u8> {
    let mut bytes = vec![0; 46];
    bytes[..4].copy_from_slice(b"PK\x01\x02");
    put16(&mut bytes, 6, 20);
    put16(&mut bytes, 8, flags);
    put16(&mut bytes, 10, method);
    put32(&mut bytes, 16, 0x1234_5678);
    put32(&mut bytes, 20, 3);
    put32(&mut bytes, 24, 9);
    put16(&mut bytes, 28, name.len().try_into().unwrap());
    put16(&mut bytes, 30, extra.len().try_into().unwrap());
    put16(&mut bytes, 32, comment.len().try_into().unwrap());
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(extra);
    bytes.extend_from_slice(comment);
    bytes
}
fn tlv(tag: u16, length: usize) -> Vec<u8> {
    assert!(length >= 4);
    let mut bytes = vec![0; length];
    put16(&mut bytes, 0, tag);
    put16(&mut bytes, 2, (length - 4).try_into().unwrap());
    bytes
}
fn archive_with_prefix(prefix: &[u8], records: &[Vec<u8>], comment: &[u8]) -> Vec<u8> {
    let mut bytes = prefix.to_vec();
    for entry in records {
        bytes.extend_from_slice(entry);
    }
    let size = bytes.len() - prefix.len();
    let mut end = [0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    put16(&mut end, 8, records.len().try_into().unwrap());
    put16(&mut end, 10, records.len().try_into().unwrap());
    put32(&mut end, 12, size.try_into().unwrap());
    put32(&mut end, 16, prefix.len().try_into().unwrap());
    put16(&mut end, 20, comment.len().try_into().unwrap());
    bytes.extend_from_slice(&end);
    bytes.extend_from_slice(comment);
    bytes
}
fn archive(records: &[Vec<u8>], comment: &[u8]) -> Vec<u8> {
    archive_with_prefix(&[0], records, comment)
}
fn verified(bytes: Vec<u8>) -> VerifiedArtifact {
    let expected = Sha256::digest(&bytes).into();
    let artifact = Artifact {
        provenance: Provenance {
            coordinate: Coordinate::new("g", "a", "1", None, "jar").unwrap(),
            repository_root: "never-opened".into(),
            artifact_path: "never-opened\0path".into(),
            byte_length: bytes.len(),
            checksum: ChecksumStatus::Unverified,
        },
        bytes,
    };
    verify_sha256(artifact, expected).unwrap()
}
fn error(bytes: Vec<u8>) -> ZipIndexError {
    let artifact = verified(bytes);
    index_zip_directory(&artifact).unwrap_err()
}
fn is_limit(bytes: Vec<u8>, field: &str) {
    assert!(matches!(error(bytes), ZipIndexError::Limit { field: found, .. } if found == field));
}
fn is_malformed(bytes: Vec<u8>) {
    assert!(matches!(error(bytes), ZipIndexError::Malformed { .. }));
}

#[test]
fn borrowed_ordered_metadata_without_payload_or_path_access() {
    let extra = [tlv(0xcafe, 4), tlv(0x7075, 5), tlv(0x7075, 4)].concat();
    let records = [
        record(b"a.class", &extra, b"comment", 0, 0),
        record(b"a.class", &[], b"\xff", 8, 8 | 6),
    ];
    let bytes = archive(&records, b"\0\xffarchive");
    let artifact = verified(bytes);
    let digest = *artifact.digest();
    let provenance = artifact.provenance().clone();
    let index = index_zip_directory(&artifact).unwrap();
    assert_eq!(index.entries().len(), 2);
    assert_eq!(index.archive_comment(), b"\0\xffarchive");
    let first = &index.entries()[0];
    assert_eq!(first.raw_name(), b"a.class");
    assert_eq!(first.raw_name().as_ptr(), artifact.bytes()[47..].as_ptr());
    assert_eq!(first.raw_extra(), extra);
    assert_eq!(first.raw_extra().as_ptr(), artifact.bytes()[54..].as_ptr());
    assert_eq!(first.raw_comment(), b"comment");
    assert_eq!(first.method(), 0);
    assert_eq!(first.flags(), 0);
    assert_eq!(first.claimed_crc32(), 0x1234_5678);
    assert_eq!(first.claimed_compressed_size(), 3);
    assert_eq!(first.claimed_uncompressed_size(), 9);
    assert_eq!(first.claimed_local_header_offset(), 0);
    assert_eq!(index.entries()[1].raw_name(), first.raw_name());
    assert_eq!(index.entries()[1].method(), 8);
    assert_eq!(index.entries()[1].flags(), 14);
    assert_eq!(index.entries()[1].raw_comment(), b"\xff");
    assert_eq!(artifact.digest(), &digest);
    assert_eq!(artifact.provenance(), &provenance);
}

#[test]
fn retained_bytes_survive_provenance_path_replacement_and_deletion() {
    let path = std::env::temp_dir().join(format!("ktlsp-zip-index-{}", std::process::id()));
    let bytes = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, &bytes).unwrap();
    drop(file);
    let expected = Sha256::digest(&bytes).into();
    let artifact = verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: Coordinate::new("g", "a", "1", None, "jar").unwrap(),
                repository_root: std::env::temp_dir(),
                artifact_path: path.clone(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
            bytes,
        },
        expected,
    )
    .unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    assert_eq!(
        index_zip_directory(&artifact).unwrap().entries()[0].raw_name(),
        b"a"
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        index_zip_directory(&artifact).unwrap().entries()[0].raw_name(),
        b"a"
    );
}

#[test]
fn empty_and_maximum_raw_archive_comment() {
    for comment in [vec![], vec![0xff; 65_535]] {
        let artifact = verified(archive_with_prefix(&[], &[], &comment));
        let index = index_zip_directory(&artifact).unwrap();
        assert!(index.entries().is_empty());
        assert_eq!(index.archive_comment(), comment);
    }
}

#[test]
fn raw_paths_duplicates_and_claim_aliases_are_preserved() {
    let names: &[&[u8]] = &[
        b"../a",
        b"/a",
        b"C:\\a",
        b"a\\b",
        b"a\0b",
        b"a/",
        b"A",
        b"a",
        b"a",
        "é".as_bytes(),
        "e\u{301}".as_bytes(),
        b"\xff",
    ];
    let mut records: Vec<_> = names.iter().map(|n| record(n, &[], &[], 0, 0)).collect();
    put32(&mut records[0], 42, 2);
    put32(&mut records[1], 42, 1);
    let artifact = verified(archive_with_prefix(&[0; 3], &records, &[]));
    let index = index_zip_directory(&artifact).unwrap();
    for (entry, name) in index.entries().iter().zip(names) {
        assert_eq!(entry.raw_name(), *name);
    }
    assert_eq!(index.entries()[0].claimed_local_header_offset(), 2);
    assert_eq!(index.entries()[1].claimed_local_header_offset(), 1);
    assert_eq!(index.entries()[2].claimed_local_header_offset(), 0);
}

#[test]
fn encoding_checks_name_and_comment_only_when_flagged() {
    for (name, comment, field) in [
        (b"\xff".as_slice(), b"ok".as_slice(), "name"),
        (b"ok".as_slice(), b"\xff".as_slice(), "file comment"),
    ] {
        assert!(
            matches!(error(archive(&[record(name, &[], comment, 0, 1 << 11)], &[])), ZipIndexError::InvalidUtf8 { field: found, .. } if found == field)
        );
        let artifact = verified(archive(&[record(name, &[], comment, 0, 0)], &[]));
        assert!(index_zip_directory(&artifact).is_ok());
    }
    let artifact = verified(archive(
        &[record(
            "日本/é".as_bytes(),
            &tlv(0x6375, 4),
            "🦀".as_bytes(),
            8,
            1 << 11,
        )],
        b"\xff",
    ));
    assert!(index_zip_directory(&artifact).is_ok());
}

#[test]
fn each_flag_and_method_policy_is_explicit() {
    for method in [0, 8] {
        for bit in 0..16 {
            let flags = 1 << bit;
            let artifact = verified(archive(&[record(b"a", &[], &[], method, flags)], &[]));
            let result = index_zip_directory(&artifact);
            match bit {
                3 | 11 => assert!(result.is_ok()),
                1 | 2 if method == 8 => assert!(result.is_ok()),
                0 | 6 | 13 => assert!(matches!(
                    result,
                    Err(ZipIndexError::UnsupportedEncryption { .. })
                )),
                _ => assert!(matches!(
                    result,
                    Err(ZipIndexError::UnsupportedFlags { .. })
                )),
            }
        }
    }
    for flags in [0, 2, 4, 6, 8, 8 | 6 | (1 << 11)] {
        let artifact = verified(archive(&[record(b"a", &[], &[], 8, flags)], &[]));
        assert!(index_zip_directory(&artifact).is_ok());
    }
    for method in [1, 9, 99, u16::MAX] {
        for reverse in [false, true] {
            let mut records = vec![
                record(b"a", &[], &[], 0, 0),
                record(b"b", &[], &[], method, 0),
            ];
            if reverse {
                records.reverse();
            }
            assert!(
                matches!(error(archive(&records, &[])), ZipIndexError::UnsupportedMethod { method: found, .. } if found == method)
            );
        }
    }
}

#[test]
fn versions_disks_and_zip64_markers() {
    for version in [0, 9, 10, 11, 19, 20, 21, 45, 0x0114] {
        let mut rec = record(b"a", &[], &[], 0, 0);
        put16(&mut rec, 6, version);
        let artifact = verified(archive(&[rec], &[]));
        if (10..=20).contains(&version) {
            assert!(index_zip_directory(&artifact).is_ok());
        } else {
            assert!(matches!(
                index_zip_directory(&artifact),
                Err(ZipIndexError::UnsupportedVersion { .. })
            ));
        }
    }
    for field in [4, 6, 8, 10] {
        let mut bytes = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
        let eocd = bytes.len() - 22;
        put16(&mut bytes, eocd + field, u16::MAX);
        assert!(matches!(
            error(bytes),
            ZipIndexError::UnsupportedZip64 { .. }
        ));
    }
    for field in [12, 16] {
        let mut bytes = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
        let eocd = bytes.len() - 22;
        put32(&mut bytes, eocd + field, u32::MAX);
        assert!(matches!(
            error(bytes),
            ZipIndexError::UnsupportedZip64 { .. }
        ));
    }
    for field in [20, 24, 42] {
        let mut rec = record(b"a", &[], &[], 0, 0);
        put32(&mut rec, field, u32::MAX);
        assert!(matches!(
            error(archive(&[rec], &[])),
            ZipIndexError::UnsupportedZip64 { .. }
        ));
    }
    for disk in [1, u16::MAX] {
        let mut rec = record(b"a", &[], &[], 0, 0);
        put16(&mut rec, 34, disk);
        let err = error(archive(&[rec], &[]));
        if disk == 1 {
            assert!(matches!(err, ZipIndexError::UnsupportedMultidisk { .. }));
        } else {
            assert!(matches!(err, ZipIndexError::UnsupportedZip64 { .. }));
        }
    }
    for field in [4, 6] {
        let mut bytes = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
        let eocd = bytes.len() - 22;
        put16(&mut bytes, eocd + field, 1);
        assert!(matches!(
            error(bytes),
            ZipIndexError::UnsupportedMultidisk { .. }
        ));
    }
}

#[test]
fn extras_are_framed_but_not_interpreted() {
    for tag in [1, 0x0017, 0x0019, 0x9901] {
        let err = error(archive(&[record(b"a", &tlv(tag, 4), &[], 0, 0)], &[]));
        if tag == 1 {
            assert!(matches!(err, ZipIndexError::UnsupportedZip64 { .. }));
        } else {
            assert!(matches!(err, ZipIndexError::UnsupportedEncryption { .. }));
        }
    }
    for extra in [
        vec![0],
        vec![0; 2],
        vec![0; 3],
        vec![2, 0, 1, 0],
        [tlv(2, 4), vec![0]].concat(),
    ] {
        is_malformed(archive(&[record(b"a", &extra, &[], 0, 0)], &[]));
    }
    let extras = [
        tlv(0xcafe, 4),
        tlv(0xffff, 5),
        tlv(0x7075, 4),
        tlv(0x6375, 4),
    ]
    .concat();
    let artifact = verified(archive(&[record(b"a", &extras, &[], 0, 0)], &[]));
    assert_eq!(
        index_zip_directory(&artifact).unwrap().entries()[0].raw_extra(),
        extras
    );
}

#[test]
fn eocd_selection_does_not_retry_or_accept_trailing_garbage() {
    let good = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
    is_malformed(vec![0]);
    let mut trailing = good.clone();
    trailing.push(0);
    is_malformed(trailing);
    let mut bad_length = good.clone();
    let at = bad_length.len() - 2;
    put16(&mut bad_length, at, 1);
    is_malformed(bad_length);
    let comment = archive_with_prefix(&[], &[], &[]);
    assert_eq!(
        error(archive(&[record(b"a", &[], &[], 0, 0)], &comment)),
        ZipIndexError::AmbiguousEocd
    );
    let mut earlier = archive(
        &[record(b"a", &[], &[], 0, 0)],
        b"PK\x05\x06not a fixed header",
    );
    let eocd = good.len() - 22;
    put32(&mut earlier, eocd + 16, 0);
    is_malformed(earlier);
    let artifact = verified(archive_with_prefix(
        b"PK\x05\x06payload",
        &[record(b"a", &[], b"PK\x05\x06", 0, 0)],
        b"PK\x05\x06",
    ));
    assert!(index_zip_directory(&artifact).is_ok());
}

#[test]
fn locator_marker_is_conservative_and_position_specific() {
    for shift in [-1isize, 0, 1] {
        let mut comment = vec![0; 24];
        let at = (4 + shift) as usize;
        comment[at..at + 4].copy_from_slice(b"PK\x06\x07");
        let artifact = verified(archive(&[record(b"a", &[], &comment, 0, 0)], &[]));
        if shift == 0 {
            assert!(matches!(
                index_zip_directory(&artifact),
                Err(ZipIndexError::UnsupportedZip64 { .. })
            ));
        } else {
            assert!(index_zip_directory(&artifact).is_ok());
        }
    }
    let mut comment = vec![0; 24];
    comment[4..7].copy_from_slice(b"PK\x06");
    let artifact = verified(archive(&[record(b"a", &[], &comment, 0, 0)], &[]));
    assert!(index_zip_directory(&artifact).is_ok());
    let artifact = verified(archive_with_prefix(
        b"PK\x06\x07",
        &[record(b"a", &[], &[], 0, 0)],
        &[],
    ));
    assert!(index_zip_directory(&artifact).is_ok());
}

#[test]
fn dense_eocd_comment_candidates_do_not_amplify_directory_work() {
    let mut comment = vec![0; 65_535];
    for at in (0..=comment.len() - 22).step_by(22) {
        comment[at..at + 4].copy_from_slice(b"PK\x05\x06");
        put16(&mut comment, at + 20, u16::MAX);
    }
    let artifact = verified(archive(&[record(b"a", &[], &[], 0, 0)], &comment));
    assert_eq!(index_zip_directory(&artifact).unwrap().entries().len(), 1);
    // This candidate ends at EOF but has an impossible directory offset/size.
    // Ambiguity is rejected before either directory is parsed.
    let remaining = (comment.len() - 22) as u16;
    put16(&mut comment, 20, remaining);
    assert_eq!(
        error(archive(&[record(b"a", &[], &[], 0, 0)], &comment)),
        ZipIndexError::AmbiguousEocd
    );
}

#[test]
fn fixed_header_cannot_read_present_eocd_or_comment() {
    let mut bytes = archive(&[record(&[b'a'; 48], &[], &[], 0, 0)], &[0; 64]);
    let eocd = bytes.len() - 86;
    put16(&mut bytes, eocd + 8, 2);
    put16(&mut bytes, eocd + 10, 2);
    assert_eq!(
        error(bytes),
        ZipIndexError::Malformed {
            offset: 95,
            reason: "truncated central header"
        }
    );
    let mut first = record(b"a", &[], &[], 0, 0);
    put16(&mut first, 28, 3);
    assert_eq!(
        error(archive(&[first, record(b"b", &[], &[], 0, 0)], &[0; 64])),
        ZipIndexError::Malformed {
            offset: 50,
            reason: "truncated central header"
        }
    );
}

#[test]
fn framing_and_encryption_error_precedence_is_stable() {
    assert_eq!(
        error(archive(
            &[record(b"a", &[], &[], 0, (1 << 0) | (1 << 15))],
            &[]
        )),
        ZipIndexError::UnsupportedEncryption { offset: 9 }
    );
    assert_eq!(
        error(archive(
            &[record(b"\xff", &[2, 0, 1, 0], &[], 0, 1 << 11)],
            &[]
        )),
        ZipIndexError::Malformed {
            offset: 48,
            reason: "truncated extra body"
        }
    );
}

#[test]
fn count_and_span_claims_cannot_escape_directory() {
    let good = archive(&[record(b"a", &[], &[], 0, 0)], &[0; 100]);
    let eocd = good.len() - 122;
    for count in [0, 2, 16_384] {
        let mut bytes = good.clone();
        put16(&mut bytes, eocd + 8, count);
        put16(&mut bytes, eocd + 10, count);
        is_malformed(bytes);
    }
    let mut disagreement = good.clone();
    put16(&mut disagreement, eocd + 8, 0);
    is_malformed(disagreement);
    for (field, value) in [
        (12, 0),
        (12, 46),
        (12, 48),
        (12, u32::MAX - 1),
        (16, 0),
        (16, 2),
        (16, u32::MAX - 1),
    ] {
        let mut bytes = good.clone();
        put32(&mut bytes, eocd + field, value);
        is_malformed(bytes);
    }
    for field in [28, 30, 32] {
        let mut bytes = good.clone();
        put16(&mut bytes, 1 + field, 50);
        is_malformed(bytes);
    }
    let mut empty_name = good.clone();
    put16(&mut empty_name, 29, 0);
    is_malformed(empty_name);
    for offset in [1, 2, u32::MAX - 1] {
        let mut bytes = good.clone();
        put32(&mut bytes, 43, offset);
        is_malformed(bytes);
    }
    let mut bad_signature = good.clone();
    bad_signature[1] = 0;
    is_malformed(bad_signature);
    let mut short = archive(&[record(b"a", &[], &[], 0, 0)], &[]);
    short.remove(47);
    let at = short.len() - 22;
    put32(&mut short, at + 12, 46);
    is_malformed(short);
    let mut zero_size = archive_with_prefix(&[], &[], &[]);
    put16(&mut zero_size, 8, 1);
    put16(&mut zero_size, 10, 1);
    is_malformed(zero_size);
    let mut leftover = record(b"a", &[], &[], 0, 0);
    leftover.push(0);
    is_malformed(archive(&[leftover], &[]));
    for signature in [b"PK\x05\x05", b"PK\x06\x08", b"PK\x06\x06", b"PK\x06\x07"] {
        let mut rec = record(b"a", &[], &[], 0, 0);
        rec.extend_from_slice(signature);
        assert!(matches!(
            error(archive(&[rec], &[])),
            ZipIndexError::UnsupportedRecord { .. }
        ));
    }
}

#[test]
fn every_byte_truncation_of_single_eocd_fixtures_is_rejected() {
    assert!(matches!(
        verify_sha256(
            Artifact {
                bytes: vec![],
                provenance: Provenance {
                    coordinate: Coordinate::new("g", "a", "1", None, "jar").unwrap(),
                    repository_root: "unused".into(),
                    artifact_path: "unused".into(),
                    byte_length: 0,
                    checksum: ChecksumStatus::Unverified
                }
            },
            [0; 32]
        ),
        Err(ktlsp_artifacts::IntegrityError::Empty)
    ));
    let fixtures = [
        archive_with_prefix(&[], &[], &[]),
        archive(&[record(b"a", &[], &[], 0, 0)], &[]),
        archive(&[record(b"a", &tlv(2, 5), b"comment", 8, 8)], b"archive"),
    ];
    for bytes in fixtures {
        for length in 1..bytes.len() {
            let _ = error(bytes[..length].to_vec());
        }
        let artifact = verified(bytes);
        assert!(index_zip_directory(&artifact).is_ok());
    }
}

#[test]
fn individual_field_and_entry_count_boundaries() {
    for (field, cap) in [
        ("name bytes", 4096),
        ("extra bytes", 16_384),
        ("file comment bytes", 16_384),
    ] {
        for length in [cap, cap + 1] {
            let data = if field == "extra bytes" {
                tlv(2, length)
            } else {
                vec![b'a'; length]
            };
            let rec = match field {
                "name bytes" => record(&data, &[], &[], 0, 0),
                "extra bytes" => record(b"a", &data, &[], 0, 0),
                _ => record(b"a", &[], &data, 0, 0),
            };
            let bytes = archive(&[rec], &[]);
            if length == cap {
                assert!(index_zip_directory(&verified(bytes)).is_ok());
            } else {
                is_limit(bytes, field);
            }
        }
    }
    for count in [16_384, 16_385] {
        let bytes = archive(&vec![record(b"a", &[], &[], 0, 0); count], &[]);
        if count == 16_384 {
            assert!(index_zip_directory(&verified(bytes)).is_ok());
        } else {
            is_limit(bytes, "entries");
        }
    }
}

#[test]
fn aggregate_name_extra_comment_and_tlv_boundaries() {
    let name = vec![b'a'; 4096];
    let mut records = vec![record(&name, &[], &[], 0, 0); 1024];
    assert!(index_zip_directory(&verified(archive(&records, &[]))).is_ok());
    records.push(record(b"a", &[], &[], 0, 0));
    is_limit(archive(&records, &[]), "aggregate name bytes");
    let extra = tlv(2, 16_384);
    let mut records = vec![record(b"a", &extra, &[], 0, 0); 256];
    assert!(index_zip_directory(&verified(archive(&records, &[]))).is_ok());
    records[255] = record(b"a", &tlv(2, 16_381), &[], 0, 0);
    records.push(record(b"a", &tlv(2, 4), &[], 0, 0));
    is_limit(archive(&records, &[]), "aggregate extra bytes");
    let records = vec![record(b"a", &[], &vec![0; 16_384], 0, 0); 64];
    assert!(index_zip_directory(&verified(archive(&records, &[]))).is_ok());
    is_limit(archive(&records, &[0]), "aggregate comment bytes");
    let extras = tlv(2, 4).repeat(4096);
    let mut records = vec![record(b"a", &extras, &[], 0, 0); 16];
    assert!(index_zip_directory(&verified(archive(&records, &[]))).is_ok());
    records.push(record(b"a", &tlv(2, 4), &[], 0, 0));
    is_limit(archive(&records, &[]), "extra TLVs");
}

#[test]
fn directory_byte_boundary_and_extreme_unvalidated_claims() {
    let name = vec![b'a'; 4096];
    let mut records = vec![record(&name, &tlv(2, 4050), &[], 0, 0); 1024];
    let bytes = archive(&records, &[]);
    assert_eq!(bytes.len(), 1 + 8 * 1024 * 1024 + 22);
    assert!(index_zip_directory(&verified(bytes)).is_ok());
    records[1023] = record(&name, &tlv(2, 4051), &[], 0, 0);
    is_limit(archive(&records, &[]), "directory bytes");
    let mut rec = record(b"a", &[], &[], 0, 8);
    put32(&mut rec, 20, u32::MAX - 1);
    put32(&mut rec, 24, u32::MAX - 2);
    let artifact = verified(archive(&[rec], &[]));
    let index = index_zip_directory(&artifact).unwrap();
    assert_eq!(index.entries()[0].claimed_compressed_size(), u32::MAX - 1);
    assert_eq!(index.entries()[0].claimed_uncompressed_size(), u32::MAX - 2);
}

#[test]
fn bounded_deterministic_hostile_bytes_and_mutations() {
    let seed = archive(&[record(b"a", &tlv(2, 5), b"note", 8, 8)], b"comment");
    for at in 0..seed.len() {
        for value in [0, 1, 0x7f, 0xff] {
            let mut bytes = seed.clone();
            bytes[at] = value;
            let artifact = verified(bytes);
            if let Ok(index) = index_zip_directory(&artifact) {
                assert!(index.entries().len() <= 1);
                for entry in index.entries() {
                    assert!(!entry.raw_name().is_empty());
                    assert!(entry.raw_name().len() <= 4096);
                }
            }
        }
    }
    let mut state = 0x93ab_1140_u32;
    for length in 1..512 {
        let bytes: Vec<_> = (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        is_malformed(bytes);
    }
}
