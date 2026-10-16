use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, index_zip_directory, verify_sha256,
};
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, annotation_extensions, annotations,
    class_path::{ClassRank, ClasspathError, ClasspathRoot, lookup_class},
    class_type_annotations, code, code_type_annotations, metadata, method_type_annotations,
    nesting, parse_class, parse_zip_class, signatures, source_attachment, type_annotations,
};
use sha2::{Digest, Sha256};

const JAR24: &[u8] = include_bytes!("../fixtures/modern-class-versions/jclMin24.jar.bin");
const JAR25: &[u8] = include_bytes!("../fixtures/modern-class-versions/jclMin25.jar.bin");
const SERIAL24: &[u8] = include_bytes!("../fixtures/modern-class-versions/24-Serializable.class.bin");
const SERIAL25: &[u8] = include_bytes!("../fixtures/modern-class-versions/25-Serializable.class.bin");
const OBJECT24: &[u8] = include_bytes!("../fixtures/modern-class-versions/24-Object.class.bin");
const OBJECT25: &[u8] = include_bytes!("../fixtures/modern-class-versions/25-Object.class.bin");

fn verified(bytes: &[u8], expected: &str, release: u16) -> ktlsp_artifacts::VerifiedArtifact {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
    verify_sha256(
        Artifact {
            bytes: bytes.to_vec(),
            provenance: Provenance {
                // Caller-supplied fixture identity, NOT an asserted Maven/SDK origin.
                coordinate: Coordinate::new("fixture", "eclipse-jdt-minimal-jcl", &release.to_string(), None, "jar").unwrap(),
                repository_root: "retained-fixture-data".into(),
                artifact_path: format!("jclMin{release}.jar.bin").into(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
        },
        Sha256::digest(bytes).into(),
    ).unwrap()
}

fn name(class: &ClassFile<'_>, index: u16) -> String {
    let Constant::Class(text) = class.constants[usize::from(index)] else { panic!("class reference") };
    String::from_utf16(class.utf16(text).unwrap()).unwrap()
}

#[test]
fn genuine_pinned_minimal_jcl_archives_and_inherited_consumers() {
    for (release, jar, expected, serial, object, count, object_ordinal, serial_hash, object_hash) in [
        (24, JAR24, "49335cdc8f8e0a2a7f529e8c5167181ee67acf334150e23a2f0dd3a03c7c57db", SERIAL24, OBJECT24, 74, 40, "8b4440d6d4fccddbf24f96007a4d5af28921307462fcd0f1ff8ad76b95de65a5", "9d2cb7b162d5e44ee196d1cc16dd1e88e886d71ebf13038d6f79689a5688cc6c"),
        (25, JAR25, "8eb40b1a47696cf92b4a83a70fd07c885bea858a5dd907edc962ed48b74585da", SERIAL25, OBJECT25, 78, 42, "3c3c1c52c1fce599f8d08c6cc22e4e4d5d09cb28cd69790a37c5c02b97189709", "dfd977f31a68312983afe5caf1705e9be0412c3add2b588e9817fc71cb4d5e0a"),
    ] {
        let artifact = verified(jar, expected, release);
        let index = index_zip_directory(&artifact).unwrap();
        assert_eq!(index.entries().len(), count);
        let entry_ptr = index.entries()[15].raw_name().as_ptr() as usize;
        let artifact_ptr = artifact.bytes().as_ptr() as usize;
        assert!((artifact_ptr..artifact_ptr + jar.len()).contains(&entry_ptr));
        assert_eq!(artifact.provenance().byte_length, jar.len());
        assert_eq!(index.entries()[15].raw_name(), b"java/io/Serializable.class");
        assert_eq!(index.entries()[object_ordinal].raw_name(), b"java/lang/Object.class");
        assert_eq!((index.entries()[15].method(), index.entries()[15].flags()), (8, 0x800));
        let mut scratch = Vec::new();
        let mut source_pointer = None;
        {
            let class = parse_zip_class(&index, 15, &mut scratch).unwrap();
            assert_eq!((class.major, class.minor, class.access_flags), (release + 44, 0, 0x601));
            assert_eq!(name(&class, class.this_class), "java/io/Serializable");
            assert_eq!(name(&class, class.super_class), "java/lang/Object");
            assert!(class.interfaces.is_empty() && class.fields.is_empty() && class.methods.is_empty());
            let sig = signatures::decode_signatures(&class).unwrap();
            assert!(sig.class.is_none() && sig.fields.is_empty() && sig.methods.is_empty());
            assert_eq!(nesting::decode(&class).unwrap(), nesting::Nesting::default());
            let declarations = annotations::decode(&class).unwrap();
            assert!(declarations.class.is_empty() && declarations.fields.is_empty() && declarations.methods.is_empty());
            assert!(matches!(metadata::decode(&declarations, metadata::Limits::default()).unwrap(), metadata::Outcome::Absent));
            assert!(annotation_extensions::decode(&class).unwrap().methods.is_empty());
            assert!(type_annotations::decode(&class).unwrap().fields.is_empty());
            assert!(method_type_annotations::decode(&class).unwrap().methods.is_empty());
            assert!(class_type_annotations::decode(&class).unwrap().class.is_empty());
            assert!(code::decode(&class, code::Limits::default()).unwrap().is_empty());
            let mut source_scratch = Vec::new();
            let attachment = source_attachment::attach(&class, &index, &mut source_scratch).unwrap_err();
            assert_eq!(attachment, if release == 24 { source_attachment::AttachmentError::MissingCandidate } else { source_attachment::AttachmentError::MissingSourceFile });
            for attr in &class.attributes {
                source_pointer = Some(attr.bytes.as_ptr());
                assert_eq!(String::from_utf16(class.utf16(attr.name).unwrap()).unwrap(), "SourceFile");
            }
        }
        assert_eq!(scratch, serial);
        if let Some(pointer) = source_pointer {
            assert_eq!(pointer, scratch[111..].as_ptr());
        }
        assert_eq!(format!("{:x}", Sha256::digest(&scratch)), serial_hash);
        let roots = [ClasspathRoot::Jar(&index)];
        {
            let selected = lookup_class(&roots, "java/io/Serializable", release, &mut scratch).unwrap().unwrap();
            assert_eq!((selected.origin.root, selected.origin.ordinal, selected.origin.rank), (0, 15, ClassRank::Base));
            assert_eq!(selected.class.major, release + 44);
        }
        assert_eq!(scratch, serial);
        if release == 25 {
            assert!(matches!(lookup_class(&roots, "java/io/Serializable", 24, &mut scratch), Err(ClasspathError::IncompatibleVersion { major: 69, maximum: 68, .. })));
        }
        let mut code_spans = Vec::new();
        {
            let class = parse_zip_class(&index, object_ordinal, &mut scratch).unwrap();
            assert_eq!(name(&class, class.this_class), "java/lang/Object");
            assert_eq!((class.major, class.minor, class.super_class, class.methods.len()), (release + 44, 0, 0, 12));
            assert_eq!(String::from_utf16(class.utf16(class.methods[0].name).unwrap()).unwrap(), "<init>");
            assert_eq!(String::from_utf16(class.utf16(class.methods[0].descriptor).unwrap()).unwrap(), "()V");
            assert!(signatures::decode_signatures(&class).unwrap().methods[4].is_some());
            let bodies = code::decode(&class, code::Limits::default()).unwrap();
            assert_eq!(bodies.len(), 12);
            for body in bodies.iter().flatten() {
                assert!(!body.bytecode.is_empty());
                code_spans.push((body.raw.as_ptr() as usize, body.raw.len()));
                assert_eq!(body.bytecode.as_ptr(), body.raw[8..].as_ptr());
                assert!(code_type_annotations::decode(&class, body, code_type_annotations::Limits::default()).unwrap().is_empty());
            }
            assert_eq!(bodies.iter().flatten().count(), 12);
        }
        assert_eq!(scratch, object);
        let start = scratch.as_ptr() as usize;
        for (pointer, length) in code_spans {
            assert!(pointer >= start && pointer + length <= start + scratch.len());
        }
        assert_eq!(format!("{:x}", Sha256::digest(&scratch)), object_hash);
    }
}

// Caller-authored model: no encoded class header is involved in the decoder matrix.
fn model(major: u16, minor: u16) -> ClassFile<'static> {
    ClassFile {
        major, minor,
        constants: vec![Constant::Unusable, Constant::Utf8("p/C".encode_utf16().collect()), Constant::Class(1)],
        access_flags: 0x21, this_class: 2, super_class: 0,
        interfaces: vec![], fields: vec![], methods: vec![], attributes: vec![],
    }
}

#[test]
fn direct_and_inherited_public_model_version_matrix() {
    let raw = [0, 0, 0, 0, 0, 0, 0, 1, 0xb1, 0, 0, 0, 0];
    let body = code::CodeBody { raw: &raw, max_stack: 0, max_locals: 0, bytecode: &raw[8..9], exceptions: vec![], attributes: vec![] };
    for (major, minor, supported) in [
        (45,0,true),(45,1,true),(45,2,true),(45,3,true),(46,0,true),(55,0,true),(67,0,true),(68,0,true),(69,0,true),
        (0,0,false),(44,0,false),(45,4,false),(46,1,false),(55,1,false),(67,1,false),(68,1,false),(69,1,false),
        (45,65535,false),(55,65535,false),(67,65535,false),(68,65535,false),(69,65535,false),(70,0,false),(65535,65535,false),
    ] {
        let class = model(major, minor);
        let results = [
            annotations::decode(&class).map(|_| ()),
            annotation_extensions::decode(&class).map(|_| ()),
            type_annotations::decode(&class).map(|_| ()),
            method_type_annotations::decode(&class).map(|_| ()),
            class_type_annotations::decode(&class).map(|_| ()),
            signatures::decode_signatures(&class).map(|_| ()),
            nesting::decode(&class).map(|_| ()),
            code::decode(&class, code::Limits::default()).map(|_| ()),
            code_type_annotations::decode(&class, &body, code_type_annotations::Limits::default()).map(|_| ()),
        ];
        for result in results {
            assert_eq!(result, if supported { Ok(()) } else { Err(Error::UnsupportedVersion { major, minor }) }, "{major}.{minor}");
        }
        let mut header = vec![0xca,0xfe,0xba,0xbe];
        header.extend(minor.to_be_bytes()); header.extend(major.to_be_bytes());
        assert_eq!(parse_class(&header).unwrap_err(), if supported { Error::Truncated } else { Error::UnsupportedVersion { major, minor } });
    }
}

#[test]
fn new_version_error_order_preserves_existing_container_and_budget_checks() {
    for major in [68,69] {
        let mut class = model(major, 0);
        class.constants.clear();
        assert_eq!(annotations::decode(&class).unwrap_err(), Error::Malformed("annotation constant pool size"));
        assert_eq!(code::decode(&class, code::Limits::default()).unwrap_err(), Error::Malformed("code constant pool size"));
        class.major = 70;
        assert_eq!(annotations::decode(&class).unwrap_err(), Error::UnsupportedVersion { major: 70, minor: 0 });
        let mut class = model(major, 0);
        class.constants.push(Constant::Utf8("Ignored".encode_utf16().collect()));
        class.attributes.push(Attribute { name: 3, bytes: &[0xff] });
        let limits = signatures::Limits { bytes: 0, ..signatures::Limits::default() };
        assert!(matches!(signatures::decode_signatures_with_limits(&class, limits), Err(Error::Limit(_))));
    }
}
