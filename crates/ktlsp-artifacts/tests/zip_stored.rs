use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, IntegrityError, Provenance, VerifiedArtifact,
    ZipIndexError, ZipStoredError, index_zip_directory, verify_sha256,
};
use sha2::{Digest, Sha256};

#[derive(Debug)]
enum Expected {
    Payload {
        start: usize,
        end: usize,
        crc: u32,
        bytes: &'static [u8],
    },
    Index(ZipIndexError),
    Stored(ZipStoredError),
}

struct Case {
    name: &'static str,
    bytes: &'static [u8],
    digest: [u8; 32],
    ordinal: usize,
    expected: Expected,
}

include!("fixtures/zip_stored/cases.rs");

const VECTOR: &[u8] = include_bytes!("fixtures/zip_stored/stored-vector.bin");

fn artifact(bytes: Vec<u8>) -> Artifact {
    Artifact {
        provenance: Provenance {
            coordinate: Coordinate::new("g", "a", "1", None, "jar").unwrap(),
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

// Independent layout from the frozen vector; CRCs are external constants.
fn single(name: &[u8], extra: &[u8], payload: &[u8], crc: u32) -> Vec<u8> {
    let mut local = VECTOR[..30].to_vec();
    put16(&mut local, 26, name.len().try_into().unwrap());
    put16(&mut local, 28, extra.len().try_into().unwrap());
    put32(&mut local, 14, crc);
    put32(&mut local, 18, payload.len().try_into().unwrap());
    put32(&mut local, 22, payload.len().try_into().unwrap());
    local.extend_from_slice(name);
    local.extend_from_slice(extra);
    local.extend_from_slice(payload);
    let d = local.len();
    let mut central = VECTOR[40..86].to_vec();
    put16(&mut central, 28, name.len().try_into().unwrap());
    put32(&mut central, 16, crc);
    put32(&mut central, 20, payload.len().try_into().unwrap());
    put32(&mut central, 24, payload.len().try_into().unwrap());
    central.extend_from_slice(name);
    let mut end = VECTOR[87..].to_vec();
    put32(&mut end, 12, central.len().try_into().unwrap());
    put32(&mut end, 16, d.try_into().unwrap());
    local.extend(central);
    local.extend(end);
    local
}

fn stored_error(bytes: Vec<u8>) -> ZipStoredError {
    index_zip_directory(&verified(bytes))
        .unwrap()
        .stored_payload(0)
        .unwrap_err()
}

fn malformed(offset: usize, reason: &'static str) -> ZipStoredError {
    ZipStoredError::Structure(ZipIndexError::Malformed { offset, reason })
}

#[test]
fn frozen_108_exact_contract_cases_and_hashes() {
    assert_eq!(CASES.len(), 108);
    for case in CASES {
        let digest: [u8; 32] = Sha256::digest(case.bytes).into();
        assert_eq!(digest, case.digest, "{}", case.name);
        let artifact = verify_sha256(artifact(case.bytes.to_vec()), case.digest).unwrap();
        let provenance = artifact.provenance().clone();
        match &case.expected {
            Expected::Index(expected) => {
                assert_eq!(
                    index_zip_directory(&artifact).unwrap_err(),
                    *expected,
                    "{}",
                    case.name
                );
            }
            Expected::Stored(expected) => {
                let index = index_zip_directory(&artifact).unwrap();
                assert_eq!(
                    index.stored_payload(case.ordinal).unwrap_err(),
                    *expected,
                    "{}",
                    case.name
                );
            }
            Expected::Payload {
                start,
                end,
                crc,
                bytes,
            } => {
                let index = index_zip_directory(&artifact).unwrap();
                let payload = index.stored_payload(case.ordinal).unwrap();
                assert_eq!(
                    index.entries()[case.ordinal].claimed_crc32(),
                    *crc,
                    "{}",
                    case.name
                );
                drop(index);
                assert_eq!(payload, *bytes, "{}", case.name);
                assert_eq!(payload, &artifact.bytes()[*start..*end], "{}", case.name);
                assert_eq!(
                    payload.as_ptr(),
                    artifact.bytes()[*start..].as_ptr(),
                    "{}",
                    case.name
                );
            }
        }
        assert_eq!(artifact.digest(), &digest, "{}", case.name);
        assert_eq!(artifact.provenance(), &provenance, "{}", case.name);
    }
}

#[test]
fn every_proper_prefix_rejects_before_payload_access() {
    for bytes in [
        VECTOR,
        include_bytes!("fixtures/zip_stored/empty-at-directory.bin").as_slice(),
        include_bytes!("fixtures/zip_stored/different-unknown-repeated-extras-prefix.bin")
            .as_slice(),
    ] {
        assert_eq!(
            verify_sha256(artifact(vec![]), [0; 32]).unwrap_err(),
            IntegrityError::Empty
        );
        for len in 1..bytes.len() {
            let artifact = verified(bytes[..len].to_vec());
            assert_eq!(
                index_zip_directory(&artifact).unwrap_err(),
                ZipIndexError::Malformed {
                    offset: len.saturating_sub(22),
                    reason: "missing EOCD",
                },
                "prefix {len}/{}",
                bytes.len()
            );
        }
    }
}

#[test]
fn exact_payload_cap_and_over_cap_before_crc() {
    let zeros = vec![0; 8 * 1024 * 1024];
    let artifact = verified(single(b"a", &[], &zeros, CRC_ZEROS_8_MIB));
    let index = index_zip_directory(&artifact).unwrap();
    let payload = index.stored_payload(0).unwrap();
    assert_eq!(payload, zeros);
    assert_eq!(payload.as_ptr(), artifact.bytes()[31..].as_ptr());
    let over = vec![0; zeros.len() + 1];
    assert_eq!(
        stored_error(single(b"a", &[], &over, 0)),
        ZipStoredError::Structure(ZipIndexError::Limit {
            offset: 31,
            field: "stored payload bytes",
            limit: 8 * 1024 * 1024
        })
    );
}

#[test]
fn local_field_caps_and_extra_framing_precedence() {
    let name = vec![b'n'; 4096];
    let artifact = verified(single(&name, &[], &[], 0));
    assert_eq!(
        index_zip_directory(&artifact)
            .unwrap()
            .stored_payload(0)
            .unwrap(),
        b""
    );
    let mut bytes = single(&name, &[], &[], 0);
    put16(&mut bytes, 26, 4097);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::Limit {
            offset: 26,
            field: "local name bytes",
            limit: 4096,
        })
    );
    let mut bytes = single(&name, &[], &[], 0);
    bytes.drain(30..31);
    let len = bytes.len();
    put32(&mut bytes, len - 6, 4125);
    assert_eq!(stored_error(bytes), malformed(30, "truncated local name"));
    let mut extra = vec![0; 16_384];
    put16(&mut extra, 0, 0xcafe);
    put16(&mut extra, 2, 16_380);
    let artifact = verified(single(b"a", &extra, &[], 0));
    assert_eq!(
        index_zip_directory(&artifact)
            .unwrap()
            .stored_payload(0)
            .unwrap(),
        b""
    );
    let repeated = [0xfe, 0xca, 0, 0].repeat(4096);
    let artifact = verified(single(b"a", &repeated, &[], 0));
    assert_eq!(
        index_zip_directory(&artifact)
            .unwrap()
            .stored_payload(0)
            .unwrap(),
        b""
    );
    let mut bytes = single(b"a", &[], &[], 0);
    put16(&mut bytes, 28, 16_385);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::Limit {
            offset: 28,
            field: "local extra bytes",
            limit: 16_384,
        })
    );
    for (extra, expected) in [
        (
            vec![0xfe, 0xca, 0, 0, 1, 0, 0, 0],
            ZipStoredError::Structure(ZipIndexError::UnsupportedZip64 { offset: 35 }),
        ),
        (
            vec![0xfe, 0xca, 0, 0, 1, 0, 1, 0],
            malformed(35, "truncated extra body"),
        ),
        (
            vec![0xfe, 0xca, 1, 0, 0, 0],
            malformed(36, "truncated extra header"),
        ),
    ] {
        assert_eq!(stored_error(single(b"a", &extra, &[], 0)), expected);
    }
}

#[test]
fn independent_crc_vectors_and_corruption() {
    for (byte, &crc) in CRC_SINGLETONS.iter().enumerate() {
        let payload = [u8::try_from(byte).unwrap()];
        let artifact = verified(single(b"a", &[], &payload, crc));
        assert_eq!(
            index_zip_directory(&artifact)
                .unwrap()
                .stored_payload(0)
                .unwrap(),
            payload
        );
    }
    for &(len, crc) in CRC_LENGTHS {
        let payload: Vec<_> = (0..len)
            .map(|i| u8::try_from((i * 37 + 11) % 256).unwrap())
            .collect();
        let artifact = verified(single(b"a", &[], &payload, crc));
        assert_eq!(
            index_zip_directory(&artifact)
                .unwrap()
                .stored_payload(0)
                .unwrap(),
            payload
        );
    }
    for i in 31..40 {
        let mut bytes = VECTOR.to_vec();
        bytes[i] ^= 1;
        assert!(matches!(
            stored_error(bytes),
            ZipStoredError::CrcMismatch {
                offset: 31,
                expected: 0xcbf4_3926,
                ..
            }
        ));
    }
    assert_eq!(
        stored_error(single(b"a", &[], b"123456789", 0xcbf4_3926u32.swap_bytes())),
        ZipStoredError::CrcMismatch {
            offset: 31,
            expected: 0x2639_f4cb,
            actual: 0xcbf4_3926
        }
    );
}

#[test]
fn debug_keeps_metadata_and_omits_retained_payload() {
    let sentinel = [217, 218, 219, 220];
    let artifact = verified(single(b"a", &[], &sentinel, CRC_DEBUG_SENTINEL));
    let index = index_zip_directory(&artifact).unwrap();
    let debug = format!("{index:?}");
    assert!(debug.starts_with("ZipDirectoryIndex { entries:"));
    assert!(debug.contains("archive_comment:"));
    assert!(!debug.contains(&format!("{sentinel:?}")));
    assert!(!debug.contains("directory_start:"));
    assert!(!debug.contains("bytes:"));
    assert_eq!(index.stored_payload(0).unwrap(), sentinel);
}

#[test]
fn payload_uses_retained_bytes_after_provenance_path_replacement_and_deletion() {
    let path = std::env::temp_dir().join(format!("ktlsp-zip-stored-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, VECTOR).unwrap();
    drop(file);
    let mut input = artifact(VECTOR.to_vec());
    input.provenance.artifact_path = path.clone();
    let artifact = verify_sha256(input, Sha256::digest(VECTOR).into()).unwrap();
    let index = index_zip_directory(&artifact).unwrap();
    std::fs::write(&path, b"replacement").unwrap();
    assert_eq!(index.stored_payload(0).unwrap(), b"123456789");
    std::fs::remove_file(path).unwrap();
    assert_eq!(index.stored_payload(0).unwrap(), b"123456789");
    assert_eq!(
        index.stored_payload(0).unwrap().as_ptr(),
        artifact.bytes()[31..].as_ptr()
    );
}

#[test]
fn selected_and_competing_directory_bounds_precede_crc() {
    let mut bytes = VECTOR.to_vec();
    put32(&mut bytes, 40 + 42, 39);
    assert_eq!(stored_error(bytes), malformed(39, "truncated local header"));
    for offset in [40, 41, u32::MAX] {
        let mut bytes = VECTOR.to_vec();
        put32(&mut bytes, 40 + 42, offset);
        let artifact = verified(bytes);
        assert_eq!(
            index_zip_directory(&artifact).unwrap_err(),
            if offset == u32::MAX {
                ZipIndexError::UnsupportedZip64 { offset: 40 }
            } else {
                ZipIndexError::Malformed {
                    offset: 82,
                    reason: "claimed local header must precede directory",
                }
            }
        );
    }
    let mut bytes =
        include_bytes!("fixtures/zip_stored/competing-signature-before-crc.bin").to_vec();
    put32(&mut bytes, 127 + 42, 79);
    assert_eq!(
        stored_error(bytes),
        malformed(79, "truncated competing local header")
    );
}

#[test]
fn local_feature_order_and_utf8_name_agreement() {
    let mut bytes = VECTOR.to_vec();
    put32(&mut bytes, 18, u32::MAX);
    put16(&mut bytes, 8, 8);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::UnsupportedZip64 { offset: 0 })
    );
    let mut bytes = VECTOR.to_vec();
    put16(&mut bytes, 8, 8);
    put16(&mut bytes, 4, 21);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::UnsupportedMethod {
            offset: 8,
            method: 8
        })
    );
    let mut bytes = VECTOR.to_vec();
    put16(&mut bytes, 4, 21);
    put16(&mut bytes, 6, 1);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::UnsupportedVersion {
            offset: 4,
            version: 21
        })
    );
    let mut bytes = VECTOR.to_vec();
    put16(&mut bytes, 6, 1 | 8);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Structure(ZipIndexError::UnsupportedEncryption { offset: 6 })
    );
    let mut bytes = include_bytes!("fixtures/zip_stored/raw-name-c3a9.bin").to_vec();
    bytes[30] = 0xff;
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Disagreement {
            offset: 30,
            field: "name"
        }
    );
}

#[test]
fn overlap_in_names_and_extras_in_both_directions() {
    let inner = single(b"b", &[], b"B", CRC_SINGLETONS[66]);
    for extra_body in [false, true] {
        let mut variable = if extra_body {
            vec![0xfe, 0xca, 32, 0]
        } else {
            vec![b'A']
        };
        variable.extend_from_slice(&inner[..32]);
        let mut bytes = if extra_body {
            single(b"a", &variable, &[], 0)
        } else {
            single(&variable, &[], &[], 0)
        };
        let h = if extra_body { 35 } else { 31 };
        let d = if extra_body { 67 } else { 63 };
        let mut end = bytes.split_off(bytes.len() - 22);
        let mut central = inner[32..79].to_vec();
        put32(&mut central, 42, h);
        bytes.extend(central);
        put16(&mut end, 8, 2);
        put16(&mut end, 10, 2);
        put32(&mut end, 12, (bytes.len() - d).try_into().unwrap());
        bytes.extend(end);
        let artifact = verified(bytes);
        let index = index_zip_directory(&artifact).unwrap();
        assert_eq!(
            index.stored_payload(0).unwrap_err(),
            ZipStoredError::Overlap {
                offset: 0,
                other_ordinal: 1
            }
        );
        assert_eq!(
            index.stored_payload(1).unwrap_err(),
            ZipStoredError::Overlap {
                offset: h as usize,
                other_ordinal: 0
            }
        );
    }
}

#[test]
fn central_order_and_selected_agreement_choose_error_before_crc() {
    let mut bytes =
        include_bytes!("fixtures/zip_stored/unselected-aliases-disjoint-from-selected.bin")
            .to_vec();
    put32(&mut bytes, 190 + 42, 0);
    bytes[32] = 0;
    bytes[31] ^= 1;
    assert_eq!(
        stored_error(bytes.clone()),
        malformed(32, "competing local header signature")
    );
    bytes[143..237].rotate_left(47);
    assert_eq!(
        stored_error(bytes),
        ZipStoredError::Overlap {
            offset: 0,
            other_ordinal: 1
        }
    );

    let mut bytes =
        include_bytes!("fixtures/zip_stored/unselected-aliases-disjoint-from-selected.bin")
            .to_vec();
    bytes[190 + 46] = b'c';
    let artifact = verified(bytes);
    assert_eq!(
        index_zip_directory(&artifact)
            .unwrap()
            .stored_payload(2)
            .unwrap_err(),
        ZipStoredError::Disagreement {
            offset: 62,
            field: "name"
        }
    );
}
