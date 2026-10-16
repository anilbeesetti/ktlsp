#[path = "../../ktlsp-artifacts/tests/fixtures/jimage/authored.rs"]
mod authored;
use authored::{Record, image, verified, zip_layer};
use ktlsp_artifacts::{ChecksumStatus, JimageError, index_jimage};
use ktlsp_binary::{Constant, Error, JimageClassError as E, parse_class, parse_jimage_class};
use sha2::{Digest, Sha256};
const OBJECT: &[u8] =
    include_bytes!("../../ktlsp-artifacts/tests/fixtures/jimage/Object.class.bin");
const IMAGES: [(&[u8], bool, usize, u32); 4] = [
    (
        include_bytes!(
            "../../ktlsp-artifacts/tests/fixtures/jimage/non-compressed_little-endian.jimage"
        ),
        false,
        7109,
        63,
    ),
    (
        include_bytes!(
            "../../ktlsp-artifacts/tests/fixtures/jimage/non-compressed_big-endian.jimage"
        ),
        false,
        7109,
        63,
    ),
    (
        include_bytes!(
            "../../ktlsp-artifacts/tests/fixtures/jimage/compressed_little-endian.jimage"
        ),
        true,
        3142,
        67,
    ),
    (
        include_bytes!("../../ktlsp-artifacts/tests/fixtures/jimage/compressed_big-endian.jimage"),
        true,
        3142,
        67,
    ),
];

#[test]
fn all_four_genuine_selected_original_object_classes_retain_actual_origin() {
    for (bytes, compressed, start, location) in IMAGES {
        let artifact = verified(bytes.to_vec());
        let index = index_jimage(&artifact).unwrap();
        let resource = index
            .select_resource(b"java.base", b"java/lang/Object.class")
            .unwrap()
            .unwrap();
        let mut scratch = vec![0xaa; 13];
        assert_eq!(resource.payload(&mut scratch).unwrap(), OBJECT);
        assert_eq!(
            format!("{:x}", Sha256::digest(OBJECT)),
            "1467daa5037c5ca733b7042b3616630bde7413f75a9c281256ff5873ba25912d"
        );
        let scratch_pointer = scratch.as_ptr();
        let class = parse_jimage_class(&resource, &mut scratch).unwrap();
        assert_eq!((class.major, class.minor), (67, 0));
        assert_eq!(class, parse_class(OBJECT).unwrap());
        let Constant::Class(name) = class.constants[usize::from(class.this_class)] else {
            panic!("not a class")
        };
        assert_eq!(
            String::from_utf16(class.utf16(name).unwrap()).unwrap(),
            "java/lang/Object"
        );
        assert!(std::ptr::eq(resource.artifact(), &artifact));
        assert!(std::ptr::eq(
            resource.artifact().provenance(),
            artifact.provenance()
        ));
        assert_eq!(
            resource.artifact().provenance().checksum,
            ChecksumStatus::Unverified
        );
        assert_eq!(resource.artifact().digest(), artifact.digest());
        assert_eq!(
            (
                resource.table_slot(),
                resource.location_offset(),
                resource.stored_span().start
            ),
            (6, location, start)
        );
        assert_eq!(resource.is_compressed(), compressed);
        // Actual borrowed class attribute bytes, not a copied verified class artifact.
        let source = if compressed {
            scratch_pointer
        } else {
            artifact.bytes()[start..].as_ptr()
        } as usize;
        for attr in &class.attributes {
            let at = attr.bytes.as_ptr() as usize;
            assert!(at >= source && at + attr.bytes.len() <= source + OBJECT.len());
        }
        let selected = index
            .select_resource(b"java.base", b"java/lang/without_ext")
            .unwrap()
            .unwrap();
        assert_eq!(
            selected.payload(&mut scratch).unwrap(),
            b"I'm file without an extension"
        );
    }
}

fn failure(bytes: &[u8], path: &[u8], compressed: bool) -> E {
    let mut record = Record::stored(b"chosen", path, bytes);
    if compressed {
        record.data = zip_layer(bytes, 1, 1);
        record.compressed = true;
    }
    let built = image(&[record]);
    assert!(built.data_start > built.strings_start);
    assert_eq!(built.locations.len(), built.slots.len());
    let artifact = verified(built.bytes);
    let index = index_jimage(&artifact).unwrap();
    let resource = index.select_resource(b"chosen", path).unwrap().unwrap();
    let mut scratch = vec![0xaa; 19];
    let error = parse_jimage_class(&resource, &mut scratch).unwrap_err();
    assert!(scratch.is_empty(), "partial class output on {error:?}");
    error
}
#[test]
fn selected_class_magic_name_version_and_extension_failures_are_atomic() {
    for compressed in [false, true] {
        let mut bad = OBJECT.to_vec();
        bad[0] = 0;
        assert_eq!(
            failure(&bad, b"java/lang/Object.class", compressed),
            E::Class(Error::Malformed("magic"))
        );
        assert_eq!(
            failure(OBJECT, b"java/lang/Other.class", compressed),
            E::DeclaredName
        );
        assert_eq!(
            failure(OBJECT, b"java/lang/Object.txt", compressed),
            E::NotClassResource
        );
        let mut future = OBJECT.to_vec();
        future[6..8].copy_from_slice(&70u16.to_be_bytes());
        assert_eq!(
            failure(&future, b"java/lang/Object.class", compressed),
            E::Class(Error::UnsupportedVersion {
                major: 70,
                minor: 0
            })
        );
        let mut preview = OBJECT.to_vec();
        preview[4..6].copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(
            failure(&preview, b"java/lang/Object.class", compressed),
            E::Class(Error::UnsupportedVersion {
                major: 67,
                minor: u16::MAX
            })
        );
        assert_eq!(
            failure(b"not class bytes", b"java/lang/Object.class", compressed),
            E::Class(Error::Malformed("magic"))
        );
    }
    let mut bad = Record::stored(b"m", b"java/lang/Object.class", OBJECT);
    bad.data = zip_layer(&zip_layer(OBJECT, 1, 5), 1, 1);
    bad.compressed = true;
    let artifact = verified(image(&[bad]).bytes);
    let index = index_jimage(&artifact).unwrap();
    let resource = index
        .select_resource(b"m", b"java/lang/Object.class")
        .unwrap()
        .unwrap();
    let mut scratch = vec![1; 11];
    assert_eq!(
        parse_jimage_class(&resource, &mut scratch),
        Err(E::Payload(JimageError::UnsupportedCompression))
    );
    assert!(scratch.is_empty());
}

fn named_class(name: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(0xcafebabeu32.to_be_bytes());
    for v in [0u16, 67, 5] {
        b.extend(v.to_be_bytes());
    }
    b.push(7);
    b.extend(2u16.to_be_bytes());
    b.push(1);
    b.extend((name.len() as u16).to_be_bytes());
    b.extend(name);
    b.push(7);
    b.extend(4u16.to_be_bytes());
    b.push(1);
    b.extend(16u16.to_be_bytes());
    b.extend(b"java/lang/Object");
    for v in [1u16, 1, 3, 0, 0, 0, 0] {
        b.extend(v.to_be_bytes());
    }
    b
}
#[test]
fn declared_names_compare_lossless_mutf8_not_rust_utf8() {
    for name in [
        b"p/\xed\xa0\xbd\xed\xb8\x80".as_slice(),
        b"p/\xed\xa0\x80",
        b"p/\xc0\x80",
    ] {
        let class_bytes = named_class(name);
        for compressed in [false, true] {
            let mut path = name.to_vec();
            path.extend(b".class");
            let mut r = Record::stored(b"m", &path, &class_bytes);
            if compressed {
                r.data = zip_layer(&class_bytes, 0, 1);
                r.compressed = true;
            }
            let artifact = verified(image(&[r]).bytes);
            let index = index_jimage(&artifact).unwrap();
            let resource = index.select_resource(b"m", &path).unwrap().unwrap();
            assert_eq!(
                parse_jimage_class(&resource, &mut Vec::new()).unwrap(),
                parse_class(&class_bytes).unwrap()
            );
            assert_eq!(
                failure(&class_bytes, b"p/Other.class", compressed),
                E::DeclaredName
            );
        }
    }
}
