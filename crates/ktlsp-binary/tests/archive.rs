use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, ZipDeflateError,
    ZipIndexError, ZipStoredError, index_zip_directory, lookup, verify_sha256,
};
use ktlsp_binary::{ClassFile, Constant, Error, ZipClassError, parse_class, parse_zip_class};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const CLASS: &[u8] = include_bytes!("../fixtures/t017c1/AppendableJoiner$Builder.class.bin");
const JAR: &[u8] = include_bytes!("../fixtures/t017c1/commons-lang3-3.17.0.jar.bin");
const MAX: usize = 8 * 1024 * 1024;

fn digest(hex: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn coordinate() -> Coordinate {
    Coordinate::new("org.apache.commons", "commons-lang3", "3.17.0", None, "jar").unwrap()
}
fn verified(bytes: Vec<u8>) -> VerifiedArtifact {
    let hash = Sha256::digest(&bytes).into();
    verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: coordinate(),
                repository_root: "never-opened".into(),
                artifact_path: "never-opened".into(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
            bytes,
        },
        hash,
    )
    .unwrap()
}
fn text(class: &ClassFile<'_>, index: u16) -> String {
    String::from_utf16(class.utf16(index).unwrap()).unwrap()
}
fn class_name(class: &ClassFile<'_>, index: u16) -> String {
    let Constant::Class(name) = class.constants[usize::from(index)] else {
        panic!("not a class")
    };
    text(class, name)
}
fn assert_builder(class: &ClassFile<'_>) {
    assert_eq!((class.major, class.minor, class.access_flags), (52, 0, 49));
    assert_eq!(
        class_name(class, class.this_class),
        "org/apache/commons/lang3/AppendableJoiner$Builder"
    );
    assert_eq!(class_name(class, class.super_class), "java/lang/Object");
    assert_eq!(
        class
            .interfaces
            .iter()
            .map(|&i| class_name(class, i))
            .collect::<Vec<_>>(),
        ["java/util/function/Supplier"]
    );
    assert_eq!(
        class
            .fields
            .iter()
            .map(|m| (
                m.access_flags,
                text(class, m.name),
                text(class, m.descriptor)
            ))
            .collect::<Vec<_>>(),
        [
            (2, "prefix".into(), "Ljava/lang/CharSequence;".into()),
            (2, "suffix".into(), "Ljava/lang/CharSequence;".into()),
            (2, "delimiter".into(), "Ljava/lang/CharSequence;".into()),
            (
                2,
                "appender".into(),
                "Lorg/apache/commons/lang3/function/FailableBiConsumer;".into()
            ),
        ]
    );
    assert_eq!(class.methods.iter().map(|m| (m.access_flags, text(class, m.name), text(class, m.descriptor))).collect::<Vec<_>>(), [
        (0, "<init>".into(), "()V".into()),
        (1, "get".into(), "()Lorg/apache/commons/lang3/AppendableJoiner;".into()),
        (1, "setDelimiter".into(), "(Ljava/lang/CharSequence;)Lorg/apache/commons/lang3/AppendableJoiner$Builder;".into()),
        (1, "setElementAppender".into(), "(Lorg/apache/commons/lang3/function/FailableBiConsumer;)Lorg/apache/commons/lang3/AppendableJoiner$Builder;".into()),
        (1, "setPrefix".into(), "(Ljava/lang/CharSequence;)Lorg/apache/commons/lang3/AppendableJoiner$Builder;".into()),
        (1, "setSuffix".into(), "(Ljava/lang/CharSequence;)Lorg/apache/commons/lang3/AppendableJoiner$Builder;".into()),
        (4161, "get".into(), "()Ljava/lang/Object;".into()),
    ]);
    assert_eq!(class, &parse_class(CLASS).unwrap());
}

struct Repository(PathBuf);
impl Drop for Repository {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn unchanged_commons_maven_lookup_digest_directory_ordinal_class() {
    let root = Repository(
        std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ktlsp-jar-class-{}", std::process::id())),
    );
    fs::create_dir(&root.0).unwrap();
    let path = root.0.join(coordinate().relative_path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, JAR).unwrap();
    let artifact = lookup(&coordinate(), std::slice::from_ref(&root.0)).unwrap();
    assert_eq!(artifact.provenance.checksum, ChecksumStatus::Unverified);
    let hash = digest("6ee731df5c8e5a2976a1ca023b6bb320ea8d3539fbe64c8a1d5cb765127c33b4");
    let artifact = verify_sha256(artifact, hash).unwrap();
    // Retained verified bytes, not a reopen, feed the production consumer.
    fs::remove_file(path).unwrap();
    let index = index_zip_directory(&artifact).unwrap();
    let entry = &index.entries()[31];
    assert_eq!(
        entry.raw_name(),
        b"org/apache/commons/lang3/AppendableJoiner$Builder.class"
    );
    assert_eq!((entry.method(), entry.flags()), (8, 0x0808));
    let mut scratch = Vec::new();
    assert_builder(&parse_zip_class(&index, 31, &mut scratch).unwrap());
    let decoded_hash: [u8; 32] = Sha256::digest(&scratch).into();
    assert_eq!(
        decoded_hash,
        digest("e288160745c56dff019f2af9393039558018308de89509bdd81a8535bfa9b03d")
    );
    assert_eq!(scratch, CLASS);
    let allocation = scratch.as_ptr();
    assert_builder(&parse_zip_class(&index, 31, &mut scratch).unwrap());
    assert_eq!(scratch.as_ptr(), allocation);
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { 0xedb8_8320 };
        }
    }
    !crc
}
fn raw_block(bytes: &[u8]) -> Vec<u8> {
    let len = u16::try_from(bytes.len()).unwrap();
    let mut out = vec![1]; // final DEFLATE stored block, not a ZIP stored entry
    out.extend(len.to_le_bytes());
    out.extend((!len).to_le_bytes());
    out.extend(bytes);
    out
}

struct Entry {
    method: u16,
    flags: u16,
    crc: u32,
    size: u32,
    payload: Vec<u8>,
    signed: bool,
}
impl Entry {
    fn new(method: u16, bytes: &[u8], flags: u16, signed: bool) -> Self {
        Self {
            method,
            flags,
            crc: crc32(bytes),
            size: bytes.len().try_into().unwrap(),
            payload: if method == 8 {
                raw_block(bytes)
            } else {
                bytes.to_vec()
            },
            signed,
        }
    }
}
// All entries deliberately use the same raw name: only the caller's ordinal wins.
fn archive(entries: &[Entry]) -> (Vec<u8>, usize) {
    let name = b"chosen.class";
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let h = bytes.len();
        let mut local = [0; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        put16(&mut local, 4, 20);
        put16(&mut local, 6, entry.flags);
        put16(&mut local, 8, entry.method);
        if entry.flags & 8 == 0 {
            put32(&mut local, 14, entry.crc);
            put32(&mut local, 18, entry.payload.len().try_into().unwrap());
            put32(&mut local, 22, entry.size);
        }
        put16(&mut local, 26, name.len().try_into().unwrap());
        bytes.extend(local);
        bytes.extend(name);
        bytes.extend(&entry.payload);
        if entry.flags & 8 != 0 {
            if entry.signed {
                bytes.extend(b"PK\x07\x08");
            }
            bytes.extend(entry.crc.to_le_bytes());
            bytes.extend(u32::try_from(entry.payload.len()).unwrap().to_le_bytes());
            bytes.extend(entry.size.to_le_bytes());
        }
        let mut record = [0; 46];
        record[..4].copy_from_slice(b"PK\x01\x02");
        put16(&mut record, 6, 20);
        put16(&mut record, 8, entry.flags);
        put16(&mut record, 10, entry.method);
        put32(&mut record, 16, entry.crc);
        put32(&mut record, 20, entry.payload.len().try_into().unwrap());
        put32(&mut record, 24, entry.size);
        put16(&mut record, 28, name.len().try_into().unwrap());
        put32(&mut record, 42, h.try_into().unwrap());
        central.extend(record);
        central.extend(name);
    }
    let d = bytes.len();
    let mut end = [0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    put16(&mut end, 8, entries.len().try_into().unwrap());
    put16(&mut end, 10, entries.len().try_into().unwrap());
    put32(&mut end, 12, central.len().try_into().unwrap());
    put32(&mut end, 16, d.try_into().unwrap());
    bytes.extend(central);
    bytes.extend(end);
    (bytes, d)
}
fn failure(bytes: Vec<u8>, ordinal: usize) -> ZipClassError {
    let artifact = verified(bytes);
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = vec![0xaa; 17];
    let error = parse_zip_class(&index, ordinal, &mut scratch).unwrap_err();
    if !matches!(error, ZipClassError::Class(_)) {
        assert!(scratch.is_empty());
    }
    error
}

#[test]
fn stored_and_deflated_signed_unsigned_descriptors_and_borrowing() {
    for method in [0, 8] {
        for flags in [0, 0x0800, 8, 0x0808] {
            for signed in [false, true] {
                let (bytes, _) = archive(&[Entry::new(method, CLASS, flags, signed)]);
                let artifact = verified(bytes);
                let index = index_zip_directory(&artifact).unwrap();
                let mut scratch = vec![0xaa; CLASS.len() * 2];
                let pointer = scratch.as_ptr();
                {
                    let class = parse_zip_class(&index, 0, &mut scratch).unwrap();
                    assert_builder(&class);
                    let attr = class.methods[0].attributes[0].bytes;
                    let origin = if method == 0 {
                        artifact.bytes().as_ptr()
                    } else {
                        pointer
                    };
                    assert!((attr.as_ptr() as usize) >= origin as usize);
                    assert!(
                        (attr.as_ptr() as usize)
                            < origin as usize
                                + if method == 0 {
                                    artifact.bytes().len()
                                } else {
                                    CLASS.len()
                                }
                    );
                }
                if method == 0 {
                    assert!(scratch.is_empty());
                } else {
                    assert_eq!(scratch, CLASS);
                }
            }
        }
    }
}

#[test]
fn exact_ordinal_and_class_errors_are_not_replaced_by_another_entry() {
    for method in [0, 8] {
        let (bytes, _) = archive(&[
            Entry::new(method, b"not a class", 0, false),
            Entry::new(8, CLASS, 0x0808, true),
        ]);
        assert_eq!(
            failure(bytes.clone(), 0),
            ZipClassError::Class(Error::Malformed("magic"))
        );
        let artifact = verified(bytes.clone());
        let index = index_zip_directory(&artifact).unwrap();
        assert_builder(&parse_zip_class(&index, 1, &mut Vec::new()).unwrap());
        assert_eq!(
            failure(bytes, 2),
            ZipClassError::Stored(ZipStoredError::InvalidOrdinal {
                ordinal: 2,
                entries: 2
            })
        );
    }
}

#[test]
fn malformed_truncated_trailing_streams_and_exact_output_lengths() {
    let mut invalid = Entry::new(8, CLASS, 0, false);
    invalid.payload[0] = 7; // reserved block type
    assert!(matches!(
        failure(archive(&[invalid]).0, 0),
        ZipClassError::Deflated(ZipDeflateError::InvalidStream { .. })
    ));
    let mut truncated = Entry::new(8, CLASS, 0, false);
    truncated.payload.pop();
    assert!(matches!(
        failure(archive(&[truncated]).0, 0),
        ZipClassError::Deflated(ZipDeflateError::InvalidStream { .. })
    ));
    let mut trailing = Entry::new(8, CLASS, 0, false);
    trailing.payload.extend([0, 0]);
    assert!(matches!(
        failure(archive(&[trailing]).0, 0),
        ZipClassError::Deflated(ZipDeflateError::TrailingInput { .. })
    ));
    for declared in [0, CLASS.len() - 1, CLASS.len() + 1] {
        let mut entry = Entry::new(8, CLASS, 0, false);
        entry.size = declared.try_into().unwrap();
        let error = failure(archive(&[entry]).0, 0);
        assert!(
            matches!(
                error,
                ZipClassError::Deflated(
                    ZipDeflateError::OutputTooLong { .. } | ZipDeflateError::SizeMismatch { .. }
                )
            ),
            "{error:?}"
        );
    }
    let empty = Entry::new(8, b"", 0, false);
    assert_eq!(
        failure(archive(&[empty]).0, 0),
        ZipClassError::Class(Error::Truncated)
    );
}

#[test]
fn crc_placeholders_descriptors_flags_and_preallocation_limits() {
    for method in [0, 8] {
        let mut entry = Entry::new(method, CLASS, 0x0808, true);
        entry.crc ^= 1;
        assert!(matches!(
            failure(archive(&[entry]).0, 0),
            ZipClassError::Stored(ZipStoredError::CrcMismatch { .. })
                | ZipClassError::Deflated(ZipDeflateError::Payload(
                    ZipStoredError::CrcMismatch { .. }
                ))
        ));
        for at in [14, 18, 22] {
            let (mut bytes, _) = archive(&[Entry::new(method, CLASS, 8, true)]);
            put32(&mut bytes, at, 1);
            assert!(matches!(
                failure(bytes, 0),
                ZipClassError::Stored(ZipStoredError::Disagreement { .. })
                    | ZipClassError::Deflated(ZipDeflateError::Payload(
                        ZipStoredError::Disagreement { .. }
                    ))
            ));
        }
        for signed in [false, true] {
            let (mut bytes, d) = archive(&[Entry::new(method, CLASS, 8, signed)]);
            bytes[d - 1] ^= 1;
            assert!(matches!(
                failure(bytes, 0),
                ZipClassError::Stored(ZipStoredError::Structure(ZipIndexError::Malformed { .. }))
                    | ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Structure(
                        ZipIndexError::Malformed { .. }
                    )))
            ));
        }
    }
    let mut entry = Entry::new(8, CLASS, 0, false);
    entry.size = (MAX + 1).try_into().unwrap();
    let artifact = verified(archive(&[entry]).0);
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = Vec::with_capacity(19);
    let capacity = scratch.capacity();
    assert!(matches!(
        parse_zip_class(&index, 0, &mut scratch),
        Err(ZipClassError::Deflated(ZipDeflateError::Structure(
            ZipIndexError::Limit {
                field: "decoded payload bytes",
                ..
            }
        )))
    ));
    assert!(scratch.is_empty());
    assert_eq!(scratch.capacity(), capacity);
    let mut entry = Entry::new(8, CLASS, 0, false);
    entry.payload = vec![0; MAX + 1];
    assert!(matches!(
        failure(archive(&[entry]).0, 0),
        ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Structure(
            ZipIndexError::Limit {
                field: "compressed payload bytes",
                ..
            }
        )))
    ));
    for flags in [2, 4] {
        assert!(matches!(
            failure(archive(&[Entry::new(8, CLASS, flags, false)]).0, 0),
            ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Structure(
                ZipIndexError::UnsupportedFlags { .. }
            )))
        ));
    }
}

#[test]
fn selected_and_competing_envelopes_include_descriptor_tails() {
    // Frozen independently authored descriptor overlaps the selected local header.
    let bytes = include_bytes!(
        "../../ktlsp-artifacts/tests/fixtures/zip_stored/descriptor-tail-overlaps-selected-header-gap.bin"
    );
    assert_eq!(
        failure(bytes.to_vec(), 1),
        ZipClassError::Stored(ZipStoredError::Overlap {
            offset: 51,
            other_ordinal: 0
        })
    );
    let mut selected = bytes.to_vec();
    for at in [14, 18, 22] {
        put32(&mut selected, at, 0);
    }
    assert_eq!(
        failure(selected, 0),
        ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Overlap {
            offset: 0,
            other_ordinal: 1
        }))
    );
    let (mut bytes, d) = archive(&[Entry::new(8, CLASS, 8, true), Entry::new(8, CLASS, 8, true)]);
    // Alias the second record's local offset with the selected descriptor entry.
    put32(&mut bytes, d + 58 + 42, 0);
    assert!(matches!(
        failure(bytes, 0),
        ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Overlap { .. }))
    ));
    // A descriptor cannot borrow bytes from the central directory.
    let (mut bytes, d) = archive(&[Entry::new(8, CLASS, 8, true)]);
    bytes.drain(d - 16..d);
    let end = bytes.len() - 22;
    put32(&mut bytes, end + 16, (d - 16).try_into().unwrap());
    assert!(matches!(
        failure(bytes, 0),
        ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Structure(
            ZipIndexError::Malformed { .. }
        )))
    ));
}

#[test]
fn chunked_exact_decoded_ceiling_and_expanding_stream_cannot_grow_past_it() {
    let exact = include_bytes!("../fixtures/zip-zero-8388608.deflate.bin");
    let over = include_bytes!("../fixtures/zip-zero-8388609.deflate.bin");
    for (input, hash) in [
        (
            exact.as_slice(),
            "11c267075b9c8783848d51618de3aa1f46ee79ed089538459a7383c7848b8218",
        ),
        (
            over.as_slice(),
            "3e46a1d86f76a8fdad7b606e977e0a5e30e26e02de550de8c7d077e4579ced49",
        ),
    ] {
        let actual: [u8; 32] = Sha256::digest(input).into();
        assert_eq!(actual, digest(hash));
        let entry = Entry {
            method: 8,
            flags: 0,
            crc: 0x1ad2_bc45,
            size: MAX.try_into().unwrap(),
            payload: input.to_vec(),
            signed: false,
        };
        let artifact = verified(archive(&[entry]).0);
        let index = index_zip_directory(&artifact).unwrap();
        let mut scratch = Vec::new();
        let error = parse_zip_class(&index, 0, &mut scratch).unwrap_err();
        if input == exact {
            assert_eq!(error, ZipClassError::Class(Error::Malformed("magic")));
            assert_eq!(scratch.len(), MAX);
            assert!(scratch.iter().all(|&b| b == 0));
        } else {
            assert_eq!(
                error,
                ZipClassError::Deflated(ZipDeflateError::OutputTooLong { expected: MAX })
            );
            assert!(scratch.is_empty());
        }
        assert_eq!(scratch.capacity(), MAX);
    }
}

#[test]
fn descriptor_signature_crc_zip64_forms_and_competing_flag_disagreement() {
    for signed in [false, true] {
        let mut entry = Entry::new(8, CLASS, 8, signed);
        entry.crc = 0x0807_4b50;
        // A CRC equal to the optional signature is not itself a signed descriptor.
        assert!(matches!(
            failure(archive(&[entry]).0, 0),
            ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::CrcMismatch { .. }))
        ));
        let (mut bytes, d) = archive(&[Entry::new(8, b"", 8, signed)]);
        let fields = d - 12;
        bytes.splice(fields + 12..fields + 12, [0; 4]);
        bytes.splice(fields + 8..fields + 8, [0; 4]);
        let end = bytes.len() - 22;
        put32(&mut bytes, end + 16, (d + 8).try_into().unwrap());
        assert!(matches!(
            failure(bytes, 0),
            ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Structure(
                ZipIndexError::UnsupportedZip64 { .. }
            )))
        ));
    }
    for method in [0, 8] {
        let (mut bytes, _) = archive(&[
            Entry::new(method, CLASS, 0, false),
            Entry::new(0, CLASS, 0, false),
        ]);
        let h = 42 + CLASS.len() + if method == 8 { 5 } else { 0 };
        put16(&mut bytes, h + 6, 8);
        let error = failure(bytes, 0);
        assert!(matches!(
            error,
            ZipClassError::Stored(ZipStoredError::Disagreement {
                field: "descriptor flag",
                ..
            }) | ZipClassError::Deflated(ZipDeflateError::Payload(ZipStoredError::Disagreement {
                field: "descriptor flag",
                ..
            }))
        ));
    }
}
