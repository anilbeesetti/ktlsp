use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, ZipDeflateError,
    ZipIndexError, ZipStoredError, index_zip_directory, verify_sha256,
};
use ktlsp_binary::source_attachment::{AttachmentError as E, MAX_PATH_BYTES, MAX_WORK, attach};
use ktlsp_binary::{Attribute, ClassFile, Constant, Error, Member, metadata, parse_zip_class};
use sha2::{Digest, Sha256};

const JAR: &[u8] = include_bytes!("../fixtures/t017c1/commons-lang3-3.17.0.jar.bin");
const SOURCES: &[u8] = include_bytes!("../fixtures/t017c1/commons-lang3-3.17.0-sources.jar.bin");
fn digest(hex: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn verified(bytes: Vec<u8>, expected: Option<[u8; 32]>) -> VerifiedArtifact {
    let expected = expected.unwrap_or_else(|| Sha256::digest(&bytes).into());
    verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: Coordinate::new(
                    "org.apache.commons",
                    "commons-lang3",
                    "3.17.0",
                    None,
                    "jar",
                )
                .unwrap(),
                repository_root: "never-opened".into(),
                artifact_path: "never-opened".into(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
            bytes,
        },
        expected,
    )
    .unwrap()
}
fn cp(class: &mut ClassFile<'_>, text: &str) -> u16 {
    let index = class.constants.len().try_into().unwrap();
    class
        .constants
        .push(Constant::Utf8(text.encode_utf16().collect()));
    index
}
fn class() -> ClassFile<'static> {
    ClassFile {
        minor: 0,
        major: 52,
        constants: vec![
            Constant::Unusable,
            Constant::Class(2),
            Constant::Utf8("p/C$Inner".encode_utf16().collect()),
            Constant::Utf8("SourceFile".encode_utf16().collect()),
            Constant::Utf8("C.java".encode_utf16().collect()),
        ],
        access_flags: 0,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes: vec![Attribute {
            name: 3,
            bytes: &[0, 4],
        }],
    }
}
fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { 0xedb8_8320 };
        }
    }
    !crc
}
// Independent small ZIP writer. Entries deliberately retain caller order/names.
fn zip(entries: &[(&[u8], &[u8])], method: u16, flags: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for &(name, data) in entries {
        let payload = if method == 8 {
            let len = u16::try_from(data.len()).unwrap();
            let mut out = vec![1];
            out.extend(len.to_le_bytes());
            out.extend((!len).to_le_bytes());
            out.extend(data);
            out
        } else {
            data.to_vec()
        };
        let offset = bytes.len();
        let mut local = [0; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        put16(&mut local, 4, 20);
        put16(&mut local, 6, flags);
        put16(&mut local, 8, method);
        put32(&mut local, 14, crc32(data));
        put32(&mut local, 18, payload.len() as u32);
        put32(&mut local, 22, data.len() as u32);
        put16(&mut local, 26, name.len() as u16);
        bytes.extend(local);
        bytes.extend(name);
        bytes.extend(payload);
        let mut central = [0; 46];
        central[..4].copy_from_slice(b"PK\x01\x02");
        put16(&mut central, 6, 20);
        put16(&mut central, 8, flags);
        put16(&mut central, 10, method);
        put32(&mut central, 16, crc32(data));
        put32(
            &mut central,
            20,
            (bytes.len() - offset - 30 - name.len()) as u32,
        );
        put32(&mut central, 24, data.len() as u32);
        put16(&mut central, 28, name.len() as u16);
        put32(&mut central, 42, offset as u32);
        directory.extend(central);
        directory.extend(name);
    }
    let start = bytes.len();
    let mut end = [0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    put16(&mut end, 8, entries.len() as u16);
    put16(&mut end, 10, entries.len() as u16);
    put32(&mut end, 12, directory.len() as u32);
    put32(&mut end, 16, start as u32);
    bytes.extend(directory);
    bytes.extend(end);
    bytes
}
fn refusal(class: &ClassFile<'_>, bytes: Vec<u8>) -> E {
    let artifact = verified(bytes, None);
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = vec![99; 20];
    let error = attach(class, &index, &mut scratch).unwrap_err();
    assert!(scratch.is_empty());
    error
}
fn normal_zip() -> Vec<u8> {
    zip(&[(b"p/C.java", b"source")], 0, 0)
}

#[test]
fn genuine_builder_attaches_unchanged_source_payload_offline() {
    let binary = verified(
        JAR.to_vec(),
        Some(digest(
            "6ee731df5c8e5a2976a1ca023b6bb320ea8d3539fbe64c8a1d5cb765127c33b4",
        )),
    );
    let source = verified(
        SOURCES.to_vec(),
        Some(digest(
            "5fdcac21ad329766054a95367d7583dfcdca737d221d5e01a5f2a198c04c6b18",
        )),
    );
    let binary_index = index_zip_directory(&binary).unwrap();
    let source_index = index_zip_directory(&source).unwrap();
    let mut class_scratch = Vec::new();
    let class = parse_zip_class(&binary_index, 31, &mut class_scratch).unwrap();
    let mut scratch = Vec::new();
    let result = attach(&class, &source_index, &mut scratch).unwrap();
    assert_eq!(result.ordinal, 27);
    assert!(std::ptr::eq(result.entry, &source_index.entries()[27]));
    assert_eq!(
        result.entry.raw_name(),
        b"org/apache/commons/lang3/AppendableJoiner.java"
    );
    assert_eq!(result.bytes.len(), 11798);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(result.bytes)),
        digest("bb5a9762fa8614c2a49e5ccb770f1378c60d5fdb7b8679039c85bdabe7f57315")
    );
    let mut independently_selected = Vec::new();
    assert_eq!(
        result.bytes,
        source_index
            .deflated_payload(27, &mut independently_selected)
            .unwrap()
    );
    assert_eq!(result.bytes.as_ptr(), scratch.as_ptr());
    let allocation = scratch.as_ptr();
    assert_eq!(
        attach(&class, &source_index, &mut scratch)
            .unwrap()
            .bytes
            .len(),
        11798
    );
    assert_eq!(allocation, scratch.as_ptr());
}

#[test]
fn exact_order_duplicates_no_suffix_or_traversal_winner() {
    let artifact = verified(
        zip(
            &[
                (b"../p/C.java", b"bad"),
                (b"other/C.java", b"bad"),
                (b"p/C.java", b"correct"),
                (b"p/./C.java", b"bad"),
            ],
            0,
            0,
        ),
        None,
    );
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = vec![9];
    let class = class();
    let result = attach(&class, &index, &mut scratch).unwrap();
    assert_eq!(result.ordinal, 2);
    assert_eq!(result.bytes, b"correct");
    assert_eq!(
        result.bytes.as_ptr(),
        index.stored_payload(2).unwrap().as_ptr()
    );
    assert!(scratch.is_empty());
    assert_eq!(
        refusal(
            &class,
            zip(&[(b"p/C.java", b"same"), (b"p/C.java", b"same")], 0, 0)
        ),
        E::Ambiguous {
            first: 0,
            second: 1
        }
    );
    assert_eq!(
        refusal(
            &class,
            zip(&[(b"p/../p/C.java", b"bad"), (b"C.java", b"bad")], 0, 0)
        ),
        E::MissingCandidate
    );
}

#[test]
fn public_class_sourcefile_indices_context_and_version_are_checked() {
    let mut c = class();
    c.attributes.clear();
    assert_eq!(refusal(&c, normal_zip()), E::MissingSourceFile);
    for bytes in [
        &[][..],
        &[0][..],
        &[0, 4, 0][..],
        &[0, 0][..],
        &[255, 255][..],
        &[0, 1][..],
    ] {
        let mut c = class();
        c.attributes[0].bytes = bytes;
        assert!(matches!(
            refusal(&c, normal_zip()),
            E::Class(Error::Malformed(_))
        ));
    }
    let mut c = class();
    c.attributes.push(Attribute {
        name: 3,
        bytes: &[0, 4],
    });
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::Malformed(_))
    ));
    let mut c = class();
    c.methods.push(Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes: vec![Attribute {
            name: 3,
            bytes: &[0, 4],
        }],
    });
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::Malformed(_))
    ));
    for index in [0, 4, u16::MAX] {
        let mut c = class();
        c.this_class = index;
        assert!(matches!(
            refusal(&c, normal_zip()),
            E::Class(Error::Malformed(_))
        ));
    }
    let mut c = class();
    c.attributes[0].name = 0;
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::Malformed(_))
    ));
    let mut c = class();
    c.constants[0] = Constant::Utf8("C.java".encode_utf16().collect());
    c.attributes[0].bytes = &[0, 0];
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::Malformed(_))
    ));
    let mut c = class();
    c.major = 70;
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::UnsupportedVersion { .. })
    ));
}

#[test]
fn strict_utf16_and_safe_paths_without_normalization() {
    for name in [
        "../C.java",
        "/C.java",
        "a\\C.java",
        "C:evil.java",
        ".",
        "..",
        "C\0.java",
        "",
    ] {
        let mut c = class();
        c.constants[4] = Constant::Utf8(name.encode_utf16().collect());
        assert_eq!(refusal(&c, normal_zip()), E::UnsafePath);
    }
    for name in ["/p/C", "p//C", "p/../C", "p\\C", "p:C", "p/C;", ""] {
        let mut c = class();
        c.constants[2] = Constant::Utf8(name.encode_utf16().collect());
        assert_eq!(refusal(&c, normal_zip()), E::UnsafePath);
    }
    for slot in [2, 4] {
        let mut c = class();
        c.constants[slot] = Constant::Utf8(vec![0xd800]);
        assert_eq!(refusal(&c, normal_zip()), E::InvalidUtf16);
    }
    let mut c = class();
    c.constants[2] = Constant::Utf8("C".encode_utf16().collect());
    c.constants[4] = Constant::Utf8("🦀.kt".encode_utf16().collect());
    let artifact = verified(zip(&[("🦀.kt".as_bytes(), b"unicode")], 8, 0x0800), None);
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = Vec::new();
    assert_eq!(attach(&c, &index, &mut scratch).unwrap().bytes, b"unicode");
    assert_eq!(
        refusal(
            &c,
            zip(&[("🦀.kt".as_bytes(), b"ambiguous encoding")], 0, 0)
        ),
        E::Unsupported("non-UTF8 ZIP name encoding")
    );
}

// Encoded real declaration-annotation consumer inputs, not header-model mocks.
fn metadata_attribute(c: &mut ClassFile<'_>, package: &str, version: u32) -> Vec<u8> {
    let desc = cp(c, "Lkotlin/Metadata;");
    let mv = cp(c, "mv");
    let pn = cp(c, "pn");
    let package = cp(c, package);
    let integers: Vec<u16> = [version, 3, 0]
        .into_iter()
        .map(|n| {
            let i = c.constants.len() as u16;
            c.constants.push(Constant::Integer(n));
            i
        })
        .collect();
    let mut bytes = Vec::new();
    bytes.extend(1u16.to_be_bytes());
    bytes.extend(desc.to_be_bytes());
    bytes.extend(2u16.to_be_bytes());
    bytes.extend(mv.to_be_bytes());
    bytes.push(b'[');
    bytes.extend(3u16.to_be_bytes());
    for i in integers {
        bytes.push(b'I');
        bytes.extend(i.to_be_bytes());
    }
    bytes.extend(pn.to_be_bytes());
    bytes.push(b's');
    bytes.extend(package.to_be_bytes());
    bytes
}
#[test]
fn pinned_metadata_package_override_and_unsupported_shapes_never_guess_java_package() {
    let mut c = class();
    let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
    let bytes = metadata_attribute(&mut c, "source.pkg", 2);
    c.attributes.push(Attribute {
        name: attr_name,
        bytes: &bytes,
    });
    let artifact = verified(
        zip(
            &[(b"p/C.java", b"wrong"), (b"source/pkg/C.java", b"right")],
            0,
            0,
        ),
        None,
    );
    let index = index_zip_directory(&artifact).unwrap();
    let mut scratch = Vec::new();
    let result = attach(&c, &index, &mut scratch).unwrap();
    assert_eq!(result.ordinal, 1);
    assert_eq!(result.bytes, b"right");
    assert_eq!(refusal(&c, normal_zip()), E::MissingCandidate);
    let mut c = class();
    let attr_name = cp(&mut c, "RuntimeVisibleAnnotations");
    let bytes = metadata_attribute(&mut c, "source.pkg", 1);
    c.attributes.push(Attribute {
        name: attr_name,
        bytes: &bytes,
    });
    assert_eq!(
        refusal(&c, normal_zip()),
        E::UnsupportedMetadata(metadata::Unsupported::Version)
    );
    for desc in [
        "Lkotlin/jvm/JvmPackageName;",
        "Lkotlin/jvm/KotlinJvmPackageName;",
    ] {
        let mut c = class();
        let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
        let desc = cp(&mut c, desc);
        let mut bytes = vec![0, 1];
        bytes.extend(desc.to_be_bytes());
        bytes.extend([0, 0]);
        c.attributes.push(Attribute {
            name: attr_name,
            bytes: &bytes,
        });
        assert_eq!(
            refusal(&c, normal_zip()),
            E::Unsupported("KotlinJvmPackageName")
        );
    }
}

#[test]
fn malformed_metadata_context_and_package_text_refuse_before_candidate() {
    let mut c = class();
    let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
    let mut bytes = metadata_attribute(&mut c, "source.pkg", 2);
    let tag_at = bytes.len() - 3;
    bytes[tag_at] = b'I';
    c.attributes.push(Attribute {
        name: attr_name,
        bytes: &bytes,
    });
    assert!(matches!(
        refusal(&c, normal_zip()),
        E::Class(Error::Malformed(_))
    ));
    let mut c = class();
    let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
    let bytes = metadata_attribute(&mut c, "source.pkg", 2);
    c.methods.push(Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes: vec![Attribute {
            name: attr_name,
            bytes: &bytes,
        }],
    });
    assert_eq!(
        refusal(&c, normal_zip()),
        E::Metadata(metadata::Error::Malformed("metadata declaration context"))
    );
    for package in [".source", "source..pkg", "source/pkg", "source\\pkg"] {
        let mut c = class();
        let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
        let bytes = metadata_attribute(&mut c, package, 2);
        c.attributes.push(Attribute {
            name: attr_name,
            bytes: &bytes,
        });
        assert_eq!(refusal(&c, normal_zip()), E::UnsafePath);
    }
    let mut c = class();
    let attr_name = cp(&mut c, "RuntimeInvisibleAnnotations");
    let bytes = metadata_attribute(&mut c, "source.pkg", 2);
    let index = u16::from_be_bytes([bytes[bytes.len() - 2], bytes[bytes.len() - 1]]);
    c.constants[usize::from(index)] = Constant::Utf8(vec![0xd800]);
    c.attributes.push(Attribute {
        name: attr_name,
        bytes: &bytes,
    });
    assert_eq!(refusal(&c, normal_zip()), E::InvalidUtf16);
}

#[test]
fn crc_and_resource_refusals_cross_actual_consumer_boundary() {
    for method in [0, 8] {
        let mut bytes = zip(&[(b"p/C.java", b"source")], method, 0);
        // Corrupt only the claimed CRC in local+central headers, retaining agreement.
        put32(&mut bytes, 14, 0);
        let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        put32(&mut bytes, central + 16, 0);
        let error = refusal(&class(), bytes);
        assert!(matches!(
            error,
            E::Stored(ZipStoredError::CrcMismatch { .. })
                | E::Deflated(ZipDeflateError::Payload(ZipStoredError::CrcMismatch { .. }))
        ));
    }
    let mut bytes = zip(&[(b"p/C.java", b"source")], 8, 0);
    put32(&mut bytes, 22, 8 * 1024 * 1024 + 1);
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    put32(&mut bytes, central + 24, 8 * 1024 * 1024 + 1);
    assert!(matches!(
        refusal(&class(), bytes),
        E::Deflated(ZipDeflateError::Structure(ZipIndexError::Limit { .. }))
    ));
    let mut c = class();
    c.constants[4] = Constant::Utf8(vec![u16::from(b'a'); MAX_PATH_BYTES + 1]);
    assert_eq!(refusal(&c, normal_zip()), E::Limit("path bytes"));
    let mut c = class();
    c.constants[3] = Constant::Utf8(vec![u16::from(b'a'); MAX_WORK + 1]);
    assert_eq!(refusal(&c, normal_zip()), E::Limit("work"));
    let huge = vec![0; ktlsp_binary::MAX_BYTES + 1];
    let mut c = class();
    c.attributes[0].bytes = &huge;
    assert_eq!(refusal(&c, normal_zip()), E::Limit("attribute bytes"));
    let mut c = class();
    c.attributes = (0..=ktlsp_binary::MAX_RECORDS)
        .map(|_| Attribute {
            name: 3,
            bytes: &[0, 4],
        })
        .collect();
    assert_eq!(refusal(&c, normal_zip()), E::Limit("nodes"));
}

#[test]
fn modern_authored_source_attachment_inherits_metadata_package_policy() {
    for major in [68, 69] {
        let mut c = class();
        c.major = major;
        let name = cp(&mut c, "RuntimeInvisibleAnnotations");
        let bytes = metadata_attribute(&mut c, "source.pkg", 2);
        c.attributes.push(Attribute { name, bytes: &bytes });
        let artifact = verified(zip(&[(b"p/C.java", b"wrong"), (b"source/pkg/C.java", b"right")], 0, 0), None);
        let index = index_zip_directory(&artifact).unwrap();
        let mut scratch = Vec::new();
        let result = attach(&c, &index, &mut scratch).unwrap();
        assert_eq!((result.ordinal, result.bytes), (1, b"right".as_slice()));
    }
}

#[test]
fn source_attachment_public_model_version_matrix_and_earlier_error_order() {
    let artifact = verified(normal_zip(), None);
    let index = index_zip_directory(&artifact).unwrap();
    for (major, minor, supported) in [
        (45, 0, true), (45, 1, true), (45, 2, true), (45, 3, true),
        (46, 0, true), (55, 0, true), (67, 0, true), (68, 0, true), (69, 0, true),
        (0, 0, false), (44, 0, false), (45, 4, false), (46, 1, false),
        (55, 1, false), (67, 1, false), (68, 1, false), (69, 1, false),
        (45, 65535, false), (55, 65535, false), (67, 65535, false),
        (68, 65535, false), (69, 65535, false), (70, 0, false), (65535, 65535, false),
    ] {
        let mut c = class();
        c.major = major;
        c.minor = minor;
        let mut scratch = vec![99];
        let result = attach(&c, &index, &mut scratch);
        if supported {
            let candidate = result.unwrap();
            assert_eq!((candidate.ordinal, candidate.bytes), (0, b"source".as_slice()));
            assert_eq!(candidate.bytes.as_ptr(), index.stored_payload(0).unwrap().as_ptr());
        } else {
            assert_eq!(result.unwrap_err(), E::Class(Error::UnsupportedVersion { major, minor }));
        }
        assert!(scratch.is_empty());
        // SourceFile framing and CP-size refusals precede the inherited version gate.
        c.attributes[0].bytes = &[];
        assert_eq!(attach(&c, &index, &mut scratch).unwrap_err(), E::Class(Error::Malformed("SourceFile attribute")));
        c.constants.clear();
        assert_eq!(attach(&c, &index, &mut scratch).unwrap_err(), E::Class(Error::Malformed("source constant pool size")));
    }
}
