use ktlsp_binary::annotation_extensions;
use ktlsp_binary::annotations::{MAX_DEPTH, Value, Visibility};
use ktlsp_binary::type_annotations::decode;
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, Member, parse_class,
};

fn text(s: &str) -> Constant {
    Constant::Utf8(s.encode_utf16().collect())
}

// Independently authored CP/payloads; no compiler/JVM or decoder generated them.
fn base<'a>() -> ClassFile<'a> {
    ClassFile {
        major: 52,
        minor: 0,
        access_flags: 0x2200,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![field(vec![])],
        methods: vec![],
        attributes: vec![],
        constants: vec![
            Constant::Unusable,
            text("RuntimeVisibleTypeAnnotations"),
            text("RuntimeInvisibleTypeAnnotations"),
            text("LA;"),
            text("x"),
            Constant::Integer(0x8000_0001),
            text("s"),
            Constant::Utf8(vec![0, 0xd800]),
            Constant::Float(0x7fc0_1234),
            Constant::Long(u64::MAX),
            Constant::Unusable,
            Constant::Double(0xfff8_0000_0000_1234),
            Constant::Unusable,
            text("LE;"),
            text("ONE"),
            text("V"),
            text("[[I"),
            text("RuntimeVisibleAnnotations"),
            text("RuntimeVisibleParameterAnnotations"),
            text("AnnotationDefault"),
            text("value"),
            text("()[I"),
            text("Ignored"),
            text("Code"),
            text("Record"),
        ],
    }
}

fn attr(name: u16, bytes: &[u8]) -> Attribute<'_> {
    Attribute { name, bytes }
}

fn field(attributes: Vec<Attribute<'_>>) -> Member<'_> {
    Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes,
    }
}

fn scalar(tag: u8, index: u16) -> Vec<u8> {
    vec![tag, (index >> 8) as u8, index as u8]
}

fn annotation(value: Option<&[u8]>) -> Vec<u8> {
    let mut out = vec![0, 3, 0, u8::from(value.is_some())];
    if let Some(value) = value {
        out.extend([0, 4]);
        out.extend(value);
    }
    out
}

fn root(path: &[(u8, u8)], value: Option<&[u8]>) -> Vec<u8> {
    let mut out = vec![0x13, path.len() as u8];
    for &(kind, index) in path {
        out.extend([kind, index]);
    }
    out.extend(annotation(value));
    out
}

fn payload(roots: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (roots.len() as u16).to_be_bytes().to_vec();
    for root in roots {
        out.extend(root);
    }
    out
}

fn array(values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![b'['];
    out.extend((values.len() as u16).to_be_bytes());
    for value in values {
        out.extend(value);
    }
    out
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}

#[test]
fn frozen_authored_byte_outcomes() {
    let cases = [
        (
            include_str!("fixtures/type_annotations/authored-bytes/empty.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/minimal.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/arg255.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/all_steps.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/int_bits.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/utf16_string.hex"),
            None,
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/wrong_target_eof.hex"),
            Some(Error::Malformed("field type annotation target")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/invalid_kind_missing_index.hex"),
            Some(Error::Truncated),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/invalid_kind.hex"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/invalid_argument.hex"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/zero_descriptor.hex"),
            Some(Error::Malformed("annotation constant index")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/wrong_cp_type.hex"),
            Some(Error::Malformed("UTF8 index")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/excess_count_eof.hex"),
            Some(Error::Limit("annotation nodes")),
        ),
        (
            include_str!("fixtures/type_annotations/authored-bytes/empty_trailing.hex"),
            Some(Error::Malformed("type annotation trailing bytes")),
        ),
    ];
    for (encoded, expected) in cases {
        let bytes = hex(encoded);
        for name in [1, 2] {
            let mut c = base();
            c.fields[0].attributes = vec![attr(name, &bytes)];
            match expected {
                Some(error) => assert_eq!(decode(&c).unwrap_err(), error, "{encoded}"),
                None => {
                    let out = decode(&c).unwrap();
                    let a = &out.fields[0][0];
                    assert_eq!(
                        a.visibility,
                        if name == 1 {
                            Visibility::RuntimeVisible
                        } else {
                            Visibility::RuntimeInvisible
                        }
                    );
                    assert_eq!(a.annotations.len(), usize::from(bytes[1]));
                    if let Some(root) = a.annotations.first() {
                        assert_eq!(root.annotation.descriptor, [76, 65, 59]);
                        let length = usize::from(bytes[3]);
                        assert_eq!(root.path.len(), length);
                        for (i, step) in root.path.iter().enumerate() {
                            assert_eq!(
                                (step.kind, step.argument_index),
                                (bytes[4 + 2 * i], bytes[5 + 2 * i])
                            );
                        }
                        if encoded == cases[4].0 {
                            assert_eq!(root.annotation.elements[0].value, Value::Int(0x8000_0001));
                        } else if encoded == cases[5].0 {
                            assert_eq!(
                                root.annotation.elements[0].value,
                                Value::String(&[0, 0xd800])
                            );
                        } else {
                            assert!(root.annotation.elements.is_empty());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn order_alignment_absent_empty_and_old_endpoint_compatibility() {
    let empty = [0, 0];
    let roots = payload(&[root(&[(3, 255)], None), root(&[(3, 255)], None)]);
    let mut c = base();
    c.fields[0].attributes = vec![attr(2, &empty), attr(1, &roots)];
    c.fields.push(field(vec![]));
    c.fields.push(field(vec![attr(1, &empty)]));
    c.attributes = vec![attr(1, &[255]), attr(1, &[255]), attr(24, &[255])];
    c.methods.push(field(vec![
        attr(1, &[255]),
        attr(1, &[255]),
        attr(23, &[255]),
    ]));
    let out = decode(&c).unwrap();
    assert_eq!(out.extensions, annotation_extensions::decode(&c).unwrap());
    assert_eq!(out.fields.len(), 3);
    assert_eq!(out.fields[0][0].visibility, Visibility::RuntimeInvisible);
    assert!(out.fields[0][0].annotations.is_empty());
    assert_eq!(out.fields[0][1].visibility, Visibility::RuntimeVisible);
    assert_eq!(out.fields[0][1].annotations.len(), 2);
    assert_eq!(
        out.fields[0][1].annotations[0],
        out.fields[0][1].annotations[1]
    );
    assert!(out.fields[1].is_empty());
    assert_eq!(out.fields[2].len(), 1);
    assert!(out.fields[2][0].annotations.is_empty());
    assert_eq!(c.fields[0].attributes[1].bytes, roots);
    c.fields[0].attributes = vec![attr(1, &[255])];
    assert!(annotation_extensions::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
}

#[test]
fn target_path_and_version_boundaries_are_encoded_data_only() {
    for length in [0, 1, 255] {
        let path: Vec<_> = (0..length)
            .map(|i| ((i % 4) as u8, if i % 4 == 3 { 255 } else { 0 }))
            .collect();
        let bytes = payload(&[root(&path, None)]);
        let mut c = base();
        c.fields[0].attributes = vec![attr(1, &bytes)];
        let out = decode(&c).unwrap();
        let actual: Vec<_> = out.fields[0][0].annotations[0]
            .path
            .iter()
            .map(|s| (s.kind, s.argument_index))
            .collect();
        assert_eq!(actual, path);
    }
    for target in 0..=255 {
        if target == 0x13 {
            continue;
        }
        let bytes = [0, 1, target];
        let mut c = base();
        c.fields[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("field type annotation target")
        );
    }
    for kind in 0..=255 {
        for index in [0, 1, 255] {
            let bytes = payload(&[root(&[(kind, index)], None)]);
            let mut c = base();
            c.fields[0].attributes = vec![attr(1, &bytes)];
            if kind <= 3 && (kind == 3 || index == 0) {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Malformed("type annotation path")
                );
            }
        }
    }
    for (major, minor) in [
        (45, 3),
        (48, 0),
        (49, 0),
        (51, 0),
        (52, 0),
        (67, 0),
        (68, 0),
        (69, 0),
        (70, 0),
        (67, 1),
        (67, 65535),
    ] {
        let mut c = base();
        c.major = major;
        c.minor = minor;
        c.fields[0].attributes = vec![attr(1, &[0, 0])];
        if major > 69 || minor != 0 && major != 45 {
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::UnsupportedVersion { major, minor }
            );
        } else if major < 52 {
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("type annotation version")
            );
            c.fields[0].attributes.clear();
            c.attributes = vec![attr(1, &[255])];
            assert!(decode(&c).is_ok());
        } else {
            assert!(decode(&c).is_ok());
        }
    }
}

#[test]
fn every_prefix_trailing_duplicates_and_failure_precedence() {
    let nested = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), scalar(b'c', 16)]))),
    ]
    .concat();
    for bytes in [
        payload(&[root(&[], None)]),
        payload(&[root(&[(3, 255); 255], None)]),
        payload(&[root(&[(0, 0)], Some(&nested))]),
    ] {
        for name in [1, 2] {
            for end in 0..bytes.len() {
                let mut c = base();
                c.fields[0].attributes = vec![attr(name, &bytes[..end])];
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Truncated,
                    "name{name} prefix{end}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            let mut c = base();
            c.fields[0].attributes = vec![attr(name, &trailing)];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("type annotation trailing bytes")
            );
            let duplicate = c.constants.len() as u16;
            c.constants.push(text(if name == 1 {
                "RuntimeVisibleTypeAnnotations"
            } else {
                "RuntimeInvisibleTypeAnnotations"
            }));
            c.fields[0].attributes = vec![attr(name, &bytes), attr(duplicate, &[])];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("duplicate type annotation attribute")
            );
            c.fields[0].attributes.swap(0, 1);
            assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
        }
    }
    let mut c = base();
    c.fields[0].attributes = vec![attr(1, &[0, 1, 0x14])];
    c.attributes = vec![attr(17, &[])];
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
    c.attributes.clear();
    c.methods.push(field(vec![attr(18, &[0])]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant index")
    );
    c.methods.clear();
    c.fields[0].attributes.push(attr(18, &[0]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation extension context")
    );
    c.fields[0].attributes = vec![attr(1, &[255, 255])];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
}

#[test]
fn consumed_constant_pool_refs_and_elements_are_checked() {
    let good = payload(&[root(&[], Some(&scalar(b'I', 5)))]);
    for offset in [4, 8, 11] {
        for index in [0, 10, 12, 25, u16::MAX] {
            let mut bytes = good.clone();
            bytes[offset..offset + 2].copy_from_slice(&index.to_be_bytes());
            let mut c = base();
            c.fields[0].attributes = vec![attr(1, &bytes)];
            assert!(decode(&c).is_err(), "offset{offset} index{index}");
        }
    }
    for name in [0, 5, 10, 25, u16::MAX] {
        let mut c = base();
        c.fields[0].attributes = vec![attr(name, &good)];
        assert!(decode(&c).is_err());
    }
    for descriptor in ["I", "[LA;", "L;", "LA", "LA;;"] {
        let mut c = base();
        c.constants[3] = text(descriptor);
        c.fields[0].attributes = vec![attr(1, &good)];
        assert!(decode(&c).is_err());
    }
    let mut duplicate = vec![
        0, 1, 0x13, 0, 0, 3, 0, 2, 0, 4, b'I', 0, 5, 0, 4, b'I', 0, 5,
    ];
    let mut c = base();
    c.constants.push(text("x"));
    duplicate[14] = 25;
    c.fields[0].attributes = vec![attr(1, &duplicate)];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("duplicate annotation element")
    );
    c.fields[0].attributes = vec![attr(1, &good)];
    c.fields[0].name = u16::MAX;
    c.fields[0].descriptor = u16::MAX;
    c.constants[0] = text("forged slot zero");
    assert!(decode(&c).is_ok());
    let mut zero = good.clone();
    zero[4..6].copy_from_slice(&[0, 0]);
    c.fields[0].attributes = vec![attr(1, &zero)];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant index")
    );
}

#[test]
fn all_existing_values_bits_utf16_and_nested_values_are_preserved() {
    let cases = [
        (scalar(b'B', 5), Value::Byte(0x8000_0001)),
        (scalar(b'C', 5), Value::Char(0x8000_0001)),
        (scalar(b'I', 5), Value::Int(0x8000_0001)),
        (scalar(b'S', 5), Value::Short(0x8000_0001)),
        (scalar(b'Z', 5), Value::Boolean(0x8000_0001)),
        (scalar(b'F', 8), Value::Float(0x7fc0_1234)),
        (scalar(b'J', 9), Value::Long(u64::MAX)),
        (scalar(b'D', 11), Value::Double(0xfff8_0000_0000_1234)),
        (scalar(b's', 7), Value::String(&[0, 0xd800])),
        (scalar(b'c', 15), Value::Class(&[86])),
        (scalar(b'c', 16), Value::Class(&[91, 91, 73])),
        (
            vec![b'e', 0, 13, 0, 14],
            Value::Enum {
                descriptor: &[76, 69, 59],
                name: &[79, 78, 69],
            },
        ),
        (vec![b'[', 0, 0], Value::Array(vec![])),
    ];
    for (value, expected) in cases {
        let bytes = payload(&[root(&[], Some(&value))]);
        let mut c = base();
        c.fields[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap().fields[0][0].annotations[0]
                .annotation
                .elements[0]
                .value,
            expected
        );
    }
    let value = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), vec![b'[', 0, 0]]))),
    ]
    .concat();
    let bytes = payload(&[root(&[], Some(&value))]);
    let mut c = base();
    c.fields[0].attributes = vec![attr(2, &bytes)];
    let out = decode(&c).unwrap();
    let Value::Annotation(nested) = &out.fields[0][0].annotations[0].annotation.elements[0].value
    else {
        panic!()
    };
    assert_eq!(
        nested.elements[0].value,
        Value::Array(vec![Value::String(&[0, 0xd800]), Value::Array(vec![])])
    );
    for tag in 0..=255 {
        if b"BCDFIJSZsec@[".contains(&tag) {
            continue;
        }
        let bytes = payload(&[root(&[], Some(&[tag]))]);
        let mut c = base();
        c.fields[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("annotation value tag")
        );
    }
}

#[test]
fn shared_depth_boundaries_and_finite_byte_mutations() {
    for depth in [MAX_DEPTH, MAX_DEPTH + 1] {
        for alternating in [false, true] {
            let mut value = scalar(b's', 7);
            let mut current = 2;
            while current < depth {
                if alternating && depth - current >= 2 {
                    value = [vec![b'@'], annotation(Some(&value))].concat();
                    current += 2;
                } else {
                    value = array(&[value]);
                    current += 1;
                }
            }
            let bytes = payload(&[root(&[], Some(&value))]);
            let mut c = base();
            c.fields[0].attributes = vec![attr(1, &bytes)];
            if depth <= MAX_DEPTH {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation depth"));
            }
        }
    }
    for bytes in [
        payload(&[root(&[], None)]),
        payload(&[root(&[(3, 1)], Some(&array(&[scalar(b's', 7)])))]),
    ] {
        for at in 0..bytes.len() {
            for replacement in 0..=255 {
                let mut changed = bytes.clone();
                changed[at] = replacement;
                let mut c = base();
                c.fields[0].attributes = vec![attr(1, &changed)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn aggregate_node_boundaries_include_new_capacity_and_all_old_outputs() {
    for count in [MAX_RECORDS - 5, MAX_RECORDS - 4] {
        let bytes = payload(&vec![root(&[], None); count]);
        let mut c = base();
        c.fields[0].attributes = vec![attr(1, &bytes)];
        match decode(&c) {
            Ok(out) => {
                assert_eq!(count, MAX_RECORDS - 5);
                let used = 1
                    + 1
                    + out.fields.capacity()
                    + out.fields[0].capacity()
                    + out.fields[0][0].annotations.capacity();
                assert!(used <= MAX_RECORDS);
                assert_eq!(out.fields[0][0].annotations.len(), count);
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    for extra in [0, 1] {
        let count = MAX_RECORDS - 19 + extra;
        let default = array(&vec![scalar(b'I', 5); count]);
        let declaration = [vec![0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
        let parameter = [vec![1, 0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
        let types = payload(&[root(&[(3, 255)], None)]);
        let mut c = base();
        c.attributes = vec![attr(17, &declaration)];
        c.fields[0].attributes = vec![attr(1, &types)];
        c.methods.push(Member {
            access_flags: 0x401,
            name: 20,
            descriptor: 21,
            attributes: vec![attr(18, &parameter), attr(19, &default)],
        });
        assert!(annotation_extensions::decode(&c).is_ok());
        match decode(&c) {
            Ok(out) => {
                assert_eq!(extra, 0);
                let new_capacity = out.fields.capacity()
                    + out.fields[0].capacity()
                    + out.fields[0][0].annotations.capacity()
                    + out.fields[0][0].annotations[0].path.capacity();
                // Existing logical charges14+X, new requested slotsQ1+C2+T1+L1.
                assert!(14 + count + new_capacity <= MAX_RECORDS);
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    let mut c = base();
    c.fields = (0..=MAX_RECORDS / 2).map(|_| field(vec![])).collect();
    assert!(annotation_extensions::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
}

#[test]
fn payload_bytes_are_charged_once_and_field_names_share_exact_work() {
    let types = payload(&[root(&[], None)]);
    for extra in [0, 1] {
        let ignored = vec![0; MAX_BYTES - types.len() + extra];
        let mut c = base();
        c.attributes = vec![attr(22, &ignored)];
        c.fields[0].attributes = vec![attr(1, &types)];
        if extra == 0 {
            assert!(decode(&c).is_ok());
        } else {
            assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
        }
    }
    let types = payload(&[root(&[(0, 0), (1, 0)], Some(&scalar(b's', 7)))]);
    let mut c = base();
    c.constants[7] = Constant::Utf8(vec![]);
    c.constants[22] = Constant::Utf8(vec![65; 65535]);
    // Name3*30 + target1 + path5 + descriptor4 + element-name2 + string-ref1.
    let remaining = (ktlsp_binary::MAX_VALIDATION_UNITS - 103 - 340 * 3 * 65536) / 3;
    assert_eq!(
        3 * remaining + 103 + 340 * 3 * 65536,
        ktlsp_binary::MAX_VALIDATION_UNITS
    );
    c.constants.push(Constant::Utf8(vec![66; remaining - 1]));
    c.fields[0].attributes = (0..340).map(|_| attr(22, &[])).collect();
    c.fields[0]
        .attributes
        .extend([attr(25, &[]), attr(1, &types)]);
    assert!(decode(&c).is_ok());
    let Constant::Utf8(value) = &mut c.constants[7] else {
        panic!()
    };
    value.push(66);
    assert!(annotation_extensions::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
    let Constant::Utf8(value) = &mut c.constants[7] else {
        panic!()
    };
    value.clear();
    let Constant::Utf8(name) = &mut c.constants[25] else {
        panic!()
    };
    name.push(66);
    assert!(annotation_extensions::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
}

#[test]
fn licensed_precompiled_field_data_matches_frozen_byte_expectations() {
    let bytes = include_bytes!("fixtures/type_annotations/Suppliers-MemoizingSupplier.class.bin");
    assert_eq!(bytes.len(), 2893);
    assert_eq!(&bytes[0x80d..0x815], [0, 1, 0x13, 0, 0, 0x4a, 0, 0]);
    let c = parse_class(bytes).unwrap();
    assert_eq!((c.major, c.minor), (52, 0));
    assert_eq!(
        c.utf16(c.fields[3].name).unwrap(),
        "value".encode_utf16().collect::<Vec<_>>()
    );
    let out = decode(&c).unwrap();
    assert_eq!(out.extensions, annotation_extensions::decode(&c).unwrap());
    assert_eq!(out.fields.len(), 5);
    for (index, attrs) in out.fields.iter().enumerate() {
        if index != 3 {
            assert!(attrs.is_empty());
            continue;
        }
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0].visibility, Visibility::RuntimeVisible);
        assert_eq!(attrs[0].annotations.len(), 1);
        let a = &attrs[0].annotations[0];
        assert!(a.path.is_empty());
        assert!(a.annotation.elements.is_empty());
        assert_eq!(
            a.annotation.descriptor,
            "Lorg/jspecify/annotations/Nullable;"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn modern_nonempty_field_type_annotations() {
    let bytes = payload(&[root(&[(3, 255)], None)]);
    for major in [68, 69] {
        let mut c = base();
        c.major = major;
        c.fields[0].attributes = vec![attr(1, &bytes)];
        let out = decode(&c).unwrap();
        let annotation = &out.fields[0][0].annotations[0];
        assert_eq!(annotation.path[0].argument_index, 255);
        assert_eq!(annotation.annotation.descriptor, c.utf16(3).unwrap());
    }
}
