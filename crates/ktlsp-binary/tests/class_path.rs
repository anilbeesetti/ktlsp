use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, index_jmod_directory,
    index_zip_directory, verify_sha256,
};
use ktlsp_binary::{
    Constant, Error, ZipClassError,
    class_path::{
        ClassRank, ClasspathError, ClasspathRoot, MAX_ENTRIES, MAX_MANIFEST_BYTES, MAX_NAME_BYTES,
        MAX_ROOTS, MAX_SCAN_BYTES, lookup_class,
    },
};
use sha2::{Digest, Sha256};

const JAR: &[u8] = include_bytes!("../fixtures/t017c1/commons-lang3-3.17.0.jar.bin");
const BUILDER: &str = "org/apache/commons/lang3/AppendableJoiner$Builder";
const MANIFEST: &[u8] = b"Manifest-Version: 1.0\r\nMulti-Release: true\r\n\r\n";

fn verified(bytes: Vec<u8>) -> VerifiedArtifact {
    let hash = Sha256::digest(&bytes).into();
    verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: Coordinate::new("example", "authored", "1", None, "jar").unwrap(),
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
fn p16(bytes: &mut [u8], at: usize, n: u16) {
    bytes[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn p32(bytes: &mut [u8], at: usize, n: u32) {
    bytes[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
// Independently authored ZIP data; offset claims are relative to embedded ZIP,
// including when prefixed with the actual JMOD magic below.
fn zip(entries: &[(&[u8], &[u8])], deflate: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for &(name, data) in entries {
        let offset = bytes.len();
        let mut compressed = Vec::new();
        let payload = if deflate {
            let n = u16::try_from(data.len()).unwrap();
            compressed.push(1);
            compressed.extend(n.to_le_bytes());
            compressed.extend((!n).to_le_bytes());
            compressed.extend(data);
            &compressed[..]
        } else {
            data
        };
        let method = if deflate { 8 } else { 0 };
        let mut local = [0; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        p16(&mut local, 4, 20);
        p16(&mut local, 6, 0x800);
        p16(&mut local, 8, method);
        p32(&mut local, 14, crc32(data));
        p32(&mut local, 18, payload.len() as u32);
        p32(&mut local, 22, data.len() as u32);
        p16(&mut local, 26, name.len() as u16);
        bytes.extend(local);
        bytes.extend(name);
        bytes.extend(payload);
        let mut central = [0; 46];
        central[..4].copy_from_slice(b"PK\x01\x02");
        p16(&mut central, 4, 20);
        p16(&mut central, 6, 20);
        p16(&mut central, 8, 0x800);
        p16(&mut central, 10, method);
        p32(&mut central, 16, crc32(data));
        p32(&mut central, 20, payload.len() as u32);
        p32(&mut central, 24, data.len() as u32);
        p16(&mut central, 28, name.len() as u16);
        p32(&mut central, 42, offset as u32);
        directory.extend(central);
        directory.extend(name);
    }
    let mut end = [0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    p16(&mut end, 8, entries.len() as u16);
    p16(&mut end, 10, entries.len() as u16);
    p32(&mut end, 12, directory.len() as u32);
    p32(&mut end, 16, bytes.len() as u32);
    bytes.extend(directory);
    bytes.extend(end);
    bytes
}
fn class_utf16(name: &[u16], major: u16) -> Vec<u8> {
    let mut encoded = Vec::new();
    for &c in name {
        if (1..=0x7f).contains(&c) {
            encoded.push(c as u8);
        } else if c <= 0x7ff {
            encoded.extend([0xc0 | (c >> 6) as u8, 0x80 | (c & 63) as u8]);
        } else {
            encoded.extend([
                0xe0 | (c >> 12) as u8,
                0x80 | ((c >> 6) & 63) as u8,
                0x80 | (c & 63) as u8,
            ]);
        }
    }
    let mut out = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0];
    out.extend(major.to_be_bytes());
    out.extend(5u16.to_be_bytes());
    out.push(1);
    out.extend((encoded.len() as u16).to_be_bytes());
    out.extend(encoded);
    out.extend([7, 0, 1, 1, 0, 16]);
    out.extend(b"java/lang/Object");
    out.extend([7, 0, 3]);
    out.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
    out
}
fn class(name: &str, major: u16) -> Vec<u8> {
    class_utf16(&name.encode_utf16().collect::<Vec<_>>(), major)
}
fn rank(entries: &[(&[u8], &[u8])], release: u16) -> Result<Option<ClassRank>, ClasspathError> {
    let artifact = verified(zip(entries, false));
    let index = index_zip_directory(&artifact).unwrap();
    Ok(lookup_class(
        &[ClasspathRoot::Jar(&index)],
        "p/C",
        release,
        &mut Vec::new(),
    )?
    .map(|c| c.origin.rank))
}

#[test]
fn genuine_commons_public_api_smoke_without_ordinal_and_real_mr_module() {
    let artifact = verified(JAR.to_vec());
    let index = index_zip_directory(&artifact).unwrap();
    let roots = [ClasspathRoot::Jar(&index)];
    let mut scratch = Vec::new();
    assert_eq!(
        format!("{:x}", Sha256::digest(JAR)),
        "6ee731df5c8e5a2976a1ca023b6bb320ea8d3539fbe64c8a1d5cb765127c33b4"
    );
    let found = lookup_class(&roots, BUILDER, 8, &mut scratch)
        .unwrap()
        .unwrap();
    assert_eq!(found.origin.root, 0);
    assert_eq!(found.origin.rank, ClassRank::Base);
    assert_eq!(
        index.entries()[found.origin.ordinal].raw_name(),
        format!("{BUILDER}.class").as_bytes()
    );
    assert_eq!(
        (
            found.class.major,
            found.class.access_flags,
            found.class.fields.len(),
            found.class.methods.len()
        ),
        (52, 49, 4, 7)
    );
    let Constant::Class(name) = found.class.constants[usize::from(found.class.this_class)] else {
        panic!()
    };
    assert!(
        found
            .class
            .utf16(name)
            .unwrap()
            .iter()
            .copied()
            .eq(BUILDER.encode_utf16())
    );
    assert_eq!(scratch.len(), 2510);
    assert!(
        lookup_class(&roots, "module-info", 8, &mut scratch)
            .unwrap()
            .is_none()
    );
    let module = lookup_class(&roots, "module-info", 9, &mut scratch)
        .unwrap()
        .unwrap();
    assert_eq!(module.origin.rank, ClassRank::Version(9));
    assert_eq!(module.class.major, 53);
    assert_eq!(module.class.access_flags, 0x8000);
    assert!(
        lookup_class(&roots, "missing/Type", 23, &mut scratch)
            .unwrap()
            .is_none()
    );
}

#[test]
fn ordered_roots_shadow_without_global_rank_competition() {
    let old = class("p/C", 52);
    let new = class("p/C", 55);
    let a = verified(zip(&[(b"p/C.class", &old)], false));
    let ai = index_zip_directory(&a).unwrap();
    let b = verified(zip(
        &[
            (b"META-INF/MANIFEST.MF", MANIFEST),
            (b"META-INF/versions/11/p/C.class", &new),
        ],
        false,
    ));
    let bi = index_zip_directory(&b).unwrap();
    let empty = verified(zip(&[], false));
    let ei = index_zip_directory(&empty).unwrap();
    let mut scratch = Vec::new();
    let found = lookup_class(
        &[
            ClasspathRoot::Jar(&ei),
            ClasspathRoot::Jar(&ai),
            ClasspathRoot::Jar(&bi),
        ],
        "p/C",
        17,
        &mut scratch,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        (found.origin.root, found.origin.rank, found.class.major),
        (1, ClassRank::Base, 52)
    );
    let found = lookup_class(
        &[ClasspathRoot::Jar(&bi), ClasspathRoot::Jar(&ai)],
        "p/C",
        17,
        &mut scratch,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        (found.origin.root, found.origin.rank, found.class.major),
        (0, ClassRank::Version(11), 55)
    );
}

#[test]
fn mr_highest_base_disabled_future_and_noncanonical_directories() {
    let base = class("p/C", 52);
    let nine = class("p/C", 53);
    let eleven = class("p/C", 55);
    let entries: &[(&[u8], &[u8])] = &[
        (b"META-INF/MANIFEST.MF", MANIFEST),
        (b"p/C.class", &base),
        (b"META-INF/versions/9/p/C.class", &nine),
        (b"META-INF/versions/11/p/C.class", &eleven),
        (b"META-INF/versions/99/p/C.class", b"bad"),
        (b"META-INF/versions/09/p/C.class", b"bad"),
        (b"META-INF/versions/8/p/C.class", b"bad"),
        (b"META-INF/versions/+9/p/C.class", b"bad"),
        (b"META-INF/versions/999999999999/p/C.class", b"bad"),
    ];
    assert_eq!(rank(entries, 8).unwrap(), Some(ClassRank::Base));
    assert_eq!(rank(entries, 10).unwrap(), Some(ClassRank::Version(9)));
    assert_eq!(rank(entries, 17).unwrap(), Some(ClassRank::Version(11)));
    for manifest in [
        b"Manifest-Version: 1.0\nMulti-Release: false\n\n".as_slice(),
        b"Manifest-Version: 1.0\nMulti-Release: true \n\n",
        b"Manifest-Version: 1.0\nMulti-Release:  true\n\n",
        b"Manifest-Version: 1.0\n\nName: x\nMulti-Release: true\n\n",
    ] {
        assert_eq!(
            rank(
                &[
                    (b"META-INF/MANIFEST.MF", manifest),
                    (b"p/C.class", &base),
                    (b"META-INF/versions/9/p/C.class", &nine)
                ],
                17
            )
            .unwrap(),
            Some(ClassRank::Base)
        );
    }
    assert_eq!(
        rank(&[(b"META-INF/versions/9/p/C.class", &nine)], 17).unwrap(),
        None
    );
}

#[test]
fn meta_inf_classes_are_never_versioned() {
    let data = class("META-INF/p/C", 52);
    for include_base in [false, true] {
        let mut entries: Vec<(&[u8], &[u8])> = vec![
            (b"META-INF/MANIFEST.MF", MANIFEST),
            (b"META-INF/versions/9/META-INF/p/C.class", b"bad"),
        ];
        if include_base {
            entries.push((b"META-INF/p/C.class", &data));
        }
        let a = verified(zip(&entries, false));
        let i = index_zip_directory(&a).unwrap();
        let mut scratch = Vec::new();
        let found =
            lookup_class(&[ClasspathRoot::Jar(&i)], "META-INF/p/C", 17, &mut scratch).unwrap();
        assert_eq!(
            found.map(|c| c.origin.rank),
            include_base.then_some(ClassRank::Base)
        );
    }
}

#[test]
fn manifest_case_continuations_line_endings_and_payload_methods() {
    let c = class("p/C", 53);
    for ending in ["\n", "\r", "\r\n"] {
        let manifest = format!(
            "Manifest-Version: 1.0{ending}mUlTi-ReLeAsE: tR{ending} uE{ending}X-UTF8: é😀{ending}{ending}"
        );
        for deflate in [false, true] {
            let a = verified(zip(
                &[
                    (b"meta-inf/manifest.mf", manifest.as_bytes()),
                    (b"META-INF/versions/9/p/C.class", &c),
                ],
                deflate,
            ));
            let i = index_zip_directory(&a).unwrap();
            assert_eq!(
                lookup_class(&[ClasspathRoot::Jar(&i)], "p/C", 9, &mut Vec::new())
                    .unwrap()
                    .unwrap()
                    .origin
                    .rank,
                ClassRank::Version(9)
            );
        }
    }
    // UTF-8 may be split between physical continuation lines.
    assert_eq!(
        rank(
            &[
                (
                    b"META-INF/MANIFEST.MF",
                    b"Manifest-Version: 1.0\nX: \xc3\n \xa9\nMulti-Release: true\n\n"
                ),
                (b"META-INF/versions/9/p/C.class", &c)
            ],
            9
        )
        .unwrap(),
        Some(ClassRank::Version(9))
    );
}

#[test]
fn malformed_or_ambiguous_manifest_is_not_a_silent_base_fallback() {
    let c = class("p/C", 52);
    for manifest in [
        b"Multi-Release: true\n\n".as_slice(),
        b"Manifest-Version: x\n\n",
        b"Manifest-Version: 1..0\n\n",
        b"Manifest-Version: 1.0\nMulti-Release:true\n\n",
        b"Manifest-Version: 1.0\nMulti-Release: true\nMULTI-RELEASE: false\n\n",
        b" orphan\n\n",
        b"Manifest-Version: 1.0\nName: x\n\n",
        b"Manifest-Version: 1.0\nX: \xff\n\n",
        b"Manifest-Version: 1.0\nX: \0\n\n",
        b"Manifest-Version: 1.0\n",
        b"Manifest-Version: 1.0",
        b"\n",
    ] {
        assert!(matches!(
            rank(
                &[(b"META-INF/MANIFEST.MF", manifest), (b"p/C.class", &c)],
                17
            ),
            Err(ClasspathError::Manifest { root: 0, .. })
        ));
    }
    let mut long = b"Manifest-Version: 1.0\nX: ".to_vec();
    long.extend([b'a'; 70]);
    long.extend(b"\n\n");
    assert!(matches!(
        rank(&[(b"META-INF/MANIFEST.MF", &long)], 17),
        Err(ClasspathError::Manifest { .. })
    ));
    assert!(matches!(
        rank(
            &[
                (b"META-INF/MANIFEST.MF", MANIFEST),
                (b"meta-inf/manifest.mf", MANIFEST),
                (b"p/C.class", &c)
            ],
            17
        ),
        Err(ClasspathError::AmbiguousManifest { root: 0 })
    ));
}

#[test]
fn manifest_header_and_continued_value_budgets_are_explicit() {
    let mut manifest = b"Manifest-Version: 1.0\nX: ".to_vec();
    let value = vec![b'a'; 65_535];
    for (n, chunk) in value.chunks(69).enumerate() {
        if n != 0 {
            manifest.push(b' ');
        }
        manifest.extend(chunk);
        manifest.push(b'\n');
    }
    manifest.push(b'\n');
    assert_eq!(
        rank(&[(b"META-INF/MANIFEST.MF", &manifest)], 17).unwrap(),
        None
    );
    manifest.pop();
    manifest.extend(b" a\n\n");
    assert!(matches!(
        rank(&[(b"META-INF/MANIFEST.MF", &manifest)], 17),
        Err(ClasspathError::Manifest {
            reason: "main value limit",
            ..
        })
    ));
    let mut manifest = b"Manifest-Version: 1.0\n".to_vec();
    for n in 0..4095 {
        manifest.extend(format!("X-{n}: a\n").bytes());
    }
    manifest.push(b'\n');
    assert_eq!(
        rank(&[(b"META-INF/MANIFEST.MF", &manifest)], 17).unwrap(),
        None
    );
    manifest.pop();
    manifest.extend(b"X-extra: a\n\n");
    assert!(matches!(
        rank(&[(b"META-INF/MANIFEST.MF", &manifest)], 17),
        Err(ClasspathError::Manifest {
            reason: "main header limit",
            ..
        })
    ));
}

#[test]
fn duplicate_winning_rank_only_is_ambiguous() {
    let base = class("p/C", 52);
    let nine = class("p/C", 53);
    assert!(matches!(
        rank(&[(b"p/C.class", &base), (b"p/C.class", &base)], 17),
        Err(ClasspathError::AmbiguousClass { .. })
    ));
    let entries: &[(&[u8], &[u8])] = &[
        (b"META-INF/MANIFEST.MF", MANIFEST),
        (b"p/C.class", &base),
        (b"p/C.class", &base),
        (b"META-INF/versions/9/p/C.class", &nine),
    ];
    assert_eq!(rank(entries, 9).unwrap(), Some(ClassRank::Version(9)));
    assert!(matches!(
        rank(&[entries[0], entries[3], entries[3], entries[1]], 17),
        Err(ClasspathError::AmbiguousClass { .. })
    ));
    assert_eq!(
        rank(
            &[
                entries[0],
                entries[3],
                (b"META-INF/versions/99/p/C.class", &nine),
                (b"META-INF/versions/99/p/C.class", &nine)
            ],
            17
        )
        .unwrap(),
        Some(ClassRank::Version(9))
    );
}

#[test]
fn selected_payload_identity_version_failures_never_fall_through() {
    let good = class("p/C", 52);
    let wrong = class("p/D", 52);
    let newer = class("p/C", 55);
    let unsupported = class("p/C", 68);
    let lower = verified(zip(&[(b"p/C.class", &good)], false));
    let li = index_zip_directory(&lower).unwrap();
    for (bytes, expected) in [
        (b"bad".as_slice(), 0),
        (&wrong, 1),
        (&newer, 2),
        (&unsupported, 3),
    ] {
        let a = verified(zip(&[(b"p/C.class", bytes)], false));
        let i = index_zip_directory(&a).unwrap();
        let error = lookup_class(
            &[ClasspathRoot::Jar(&i), ClasspathRoot::Jar(&li)],
            "p/C",
            8,
            &mut Vec::new(),
        )
        .unwrap_err();
        match expected {
            0 => assert!(matches!(error, ClasspathError::ClassPayload { .. })),
            1 => assert!(matches!(error, ClasspathError::DeclaredName { .. })),
            2 => assert!(matches!(
                error,
                ClasspathError::IncompatibleVersion { maximum: 52, .. }
            )),
            _ => assert!(matches!(
                error,
                ClasspathError::ClassPayload {
                    error: ZipClassError::Class(Error::UnsupportedVersion { major: 68, .. }),
                    ..
                }
            )),
        }
    }
    assert!(matches!(
        rank(
            &[
                (b"META-INF/MANIFEST.MF", MANIFEST),
                (b"p/C.class", &good),
                (b"META-INF/versions/9/p/C.class", b"bad")
            ],
            17
        ),
        Err(ClasspathError::ClassPayload { .. })
    ));
    assert!(matches!(
        rank(
            &[
                (b"META-INF/MANIFEST.MF", MANIFEST),
                (b"p/C.class", &good),
                (b"META-INF/versions/9/p/C.class", &newer)
            ],
            17
        ),
        Err(ClasspathError::IncompatibleVersion { maximum: 53, .. })
    ));
    let mut corrupted = zip(&[(b"p/C.class", &good)], false);
    corrupted[30 + b"p/C.class".len()] ^= 1;
    let a = verified(corrupted);
    let i = index_zip_directory(&a).unwrap();
    assert!(matches!(
        lookup_class(
            &[ClasspathRoot::Jar(&i), ClasspathRoot::Jar(&li)],
            "p/C",
            17,
            &mut Vec::new()
        ),
        Err(ClasspathError::ClassPayload {
            error: ZipClassError::Stored(_),
            ..
        })
    ));
    for deflate in [false, true] {
        let mut corrupted = zip(
            &[(b"META-INF/MANIFEST.MF", MANIFEST), (b"p/C.class", &good)],
            deflate,
        );
        corrupted[30 + b"META-INF/MANIFEST.MF".len() + if deflate { 5 } else { 0 }] ^= 1;
        let a = verified(corrupted);
        let i = index_zip_directory(&a).unwrap();
        assert!(matches!(
            lookup_class(
                &[ClasspathRoot::Jar(&i), ClasspathRoot::Jar(&li)],
                "p/C",
                17,
                &mut Vec::new()
            ),
            Err(ClasspathError::ManifestPayload { root: 0, .. })
        ));
        let mut corrupted = zip(&[(b"p/C.class", &good)], deflate);
        corrupted[30 + b"p/C.class".len() + if deflate { 5 } else { 0 }] ^= 1;
        let a = verified(corrupted);
        let i = index_zip_directory(&a).unwrap();
        assert!(matches!(
            lookup_class(
                &[ClasspathRoot::Jar(&i), ClasspathRoot::Jar(&li)],
                "p/C",
                17,
                &mut Vec::new()
            ),
            Err(ClasspathError::ClassPayload { .. })
        ));
    }
}

#[test]
fn name_safety_unicode_exactness_and_release_input() {
    let a = verified(zip(&[], false));
    let i = index_zip_directory(&a).unwrap();
    for name in [
        "",
        "/p/C",
        "p//C",
        "p/C/",
        "../C",
        "p.C",
        "p/C.class",
        "[Lp/C;",
        "p\\C",
        "C:evil",
        "p/\0",
        "p/\n",
        "p/\u{85}",
    ] {
        assert_eq!(
            lookup_class(&[ClasspathRoot::Jar(&i)], name, 17, &mut Vec::new()).unwrap_err(),
            ClasspathError::InvalidName
        );
    }
    assert_eq!(
        lookup_class(&[], &"x".repeat(MAX_NAME_BYTES + 1), 17, &mut Vec::new()).unwrap_err(),
        ClasspathError::InvalidName
    );
    assert_eq!(
        lookup_class(&[], "p/C", 0, &mut Vec::new()).unwrap_err(),
        ClasspathError::InvalidRelease
    );
    for name in ["p/é😀", "p/e\u{301}"] {
        let data = class(name, 52);
        let path = format!("{name}.class");
        let a = verified(zip(&[(path.as_bytes(), &data)], false));
        let i = index_zip_directory(&a).unwrap();
        assert!(
            lookup_class(&[ClasspathRoot::Jar(&i)], name, 8, &mut Vec::new())
                .unwrap()
                .is_some()
        );
    }
    let surrogate = class_utf16(&[b'p' as u16, b'/' as u16, 0xd800], 52);
    let a = verified(zip(&[("p/�.class".as_bytes(), &surrogate)], false));
    let i = index_zip_directory(&a).unwrap();
    assert!(matches!(
        lookup_class(&[ClasspathRoot::Jar(&i)], "p/�", 8, &mut Vec::new()),
        Err(ClasspathError::DeclaredName { .. })
    ));
    let decomposed = class("p/e\u{301}", 52);
    let a = verified(zip(&[("p/é.class".as_bytes(), &decomposed)], false));
    let i = index_zip_directory(&a).unwrap();
    assert!(matches!(
        lookup_class(&[ClasspathRoot::Jar(&i)], "p/é", 8, &mut Vec::new()),
        Err(ClasspathError::DeclaredName { .. })
    ));
    let longest = "x".repeat(MAX_NAME_BYTES);
    assert!(
        lookup_class(&[], &longest, u16::MAX, &mut Vec::new())
            .unwrap()
            .is_none()
    );
}

#[test]
fn jmod_classes_prefix_no_manifest_or_multi_release_and_order() {
    let data = class("p/C", 53);
    let mut bytes = b"JM\x01\x00".to_vec();
    bytes.extend(zip(
        &[
            (b"p/C.class", b"bad"),
            (b"classes/p/C.class", &data),
            (b"classes/META-INF/MANIFEST.MF", b"bad"),
            (b"classes/META-INF/versions/9/p/C.class", b"bad"),
        ],
        true,
    ));
    let a = verified(bytes);
    let i = index_jmod_directory(&a).unwrap();
    let mut scratch = Vec::new();
    let found = lookup_class(&[ClasspathRoot::Jmod(&i)], "p/C", 9, &mut scratch)
        .unwrap()
        .unwrap();
    assert_eq!(
        (found.origin.rank, found.origin.ordinal, found.class.major),
        (ClassRank::Jmod, 1, 53)
    );
    let jar = verified(zip(&[(b"p/C.class", &class("p/C", 52))], false));
    let ji = index_zip_directory(&jar).unwrap();
    assert_eq!(
        lookup_class(
            &[ClasspathRoot::Jar(&ji), ClasspathRoot::Jmod(&i)],
            "p/C",
            9,
            &mut scratch
        )
        .unwrap()
        .unwrap()
        .origin
        .rank,
        ClassRank::Base
    );
    assert_eq!(
        lookup_class(
            &[ClasspathRoot::Jmod(&i), ClasspathRoot::Jar(&ji)],
            "p/C",
            9,
            &mut scratch
        )
        .unwrap()
        .unwrap()
        .origin
        .rank,
        ClassRank::Jmod
    );
}

#[test]
fn whole_query_budgets_precede_scratch_growth_even_for_later_roots() {
    let c = class("p/C", 52);
    let a = verified(zip(&[(b"p/C.class", &c)], false));
    let i = index_zip_directory(&a).unwrap();
    let mut scratch = Vec::new();
    assert!(
        lookup_class(
            &vec![ClasspathRoot::Jar(&i); MAX_ROOTS],
            "p/C",
            8,
            &mut scratch
        )
        .unwrap()
        .is_some()
    );
    assert_eq!(
        lookup_class(
            &vec![ClasspathRoot::Jar(&i); MAX_ROOTS + 1],
            "p/C",
            8,
            &mut Vec::new()
        )
        .unwrap_err(),
        ClasspathError::Limit("roots")
    );
    let payload = vec![b'x'; MAX_MANIFEST_BYTES + 1];
    let huge = verified(zip(&[(b"META-INF/MANIFEST.MF", &payload)], false));
    let hi = index_zip_directory(&huge).unwrap();
    let mut fresh = Vec::new();
    assert_eq!(
        lookup_class(
            &[ClasspathRoot::Jar(&i), ClasspathRoot::Jar(&hi)],
            "p/C",
            8,
            &mut fresh
        )
        .unwrap_err(),
        ClasspathError::Limit("manifest bytes")
    );
    assert_eq!(fresh.capacity(), 0);
    let boundary = vec![b'\n'; MAX_MANIFEST_BYTES];
    let a = verified(zip(&[(b"META-INF/MANIFEST.MF", &boundary)], false));
    let bi = index_zip_directory(&a).unwrap();
    assert!(matches!(
        lookup_class(&[ClasspathRoot::Jar(&bi); 2], "p/C", 8, &mut fresh),
        Err(ClasspathError::Manifest { .. })
    ));
    assert_eq!(
        lookup_class(&[ClasspathRoot::Jar(&bi); 3], "p/C", 8, &mut Vec::new()).unwrap_err(),
        ClasspathError::Limit("manifest total")
    );
    let entries = vec![(b"x".as_slice(), b"".as_slice()); 16_384];
    let a = verified(zip(&entries, false));
    let many = index_zip_directory(&a).unwrap();
    assert_eq!(MAX_ENTRIES, 4 * many.entries().len());
    assert!(
        lookup_class(&[ClasspathRoot::Jar(&many); 4], "p/C", 8, &mut fresh)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        lookup_class(&[ClasspathRoot::Jar(&many); 5], "p/C", 8, &mut fresh).unwrap_err(),
        ClasspathError::Limit("entries")
    );
    let name = vec![b'x'; 4096];
    let entries = vec![(name.as_slice(), b"".as_slice()); 1024];
    let a = verified(zip(&entries, false));
    let names = index_zip_directory(&a).unwrap();
    assert_eq!(MAX_SCAN_BYTES, 4 * 4096 * 1024);
    assert!(
        lookup_class(&[ClasspathRoot::Jar(&names); 4], "p/C", 8, &mut fresh)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        lookup_class(&[ClasspathRoot::Jar(&names); 5], "p/C", 8, &mut fresh).unwrap_err(),
        ClasspathError::Limit("scan bytes")
    );
}
