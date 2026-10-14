use ktlsp_binary::annotation_extensions;
use ktlsp_binary::annotations::{MAX_DEPTH, Value, Visibility};
use ktlsp_binary::method_type_annotations::{MethodTargetInfo, decode};
use ktlsp_binary::type_annotations;
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
        fields: vec![],
        methods: vec![field(vec![])],
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
    let mut out = vec![0x14, path.len() as u8];
    for &(kind, index) in path {
        out.extend([kind, index]);
    }
    out.extend(annotation(value));
    out
}

fn parameter_root(index: u8, path: &[(u8, u8)], value: Option<&[u8]>) -> Vec<u8> {
    let mut out = root(path, value);
    out[0] = 0x16;
    out.insert(1, index);
    out
}

fn target_infos() -> [(u8, &'static [u8]); 6] {
    [
        (0x01, &[255]),
        (0x12, &[254, 253]),
        (0x14, &[]),
        (0x15, &[]),
        (0x16, &[255]),
        (0x17, &[1, 2]),
    ]
}

fn target_root(tag: u8, info: &[u8], path: &[(u8, u8)], value: Option<&[u8]>) -> Vec<u8> {
    let mut out = root(path, value);
    out[0] = tag;
    out.splice(1..1, info.iter().copied());
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
            include_str!("fixtures/method_type_annotations/authored-bytes/empty.hex"),
            None,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/minimal.hex"),
            None,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/arg255.hex"),
            None,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/all_steps.hex"),
            None,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/int_bits.hex"),
            None,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/utf16_string.hex"),
            None,
        ),
        (
            include_str!(
                "fixtures/method_type_annotations/authored-bytes/illegal_field_target.hex"
            ),
            Some(Error::Malformed("method type annotation target")),
        ),
        (
            include_str!(
                "fixtures/method_type_annotations/authored-bytes/invalid_kind_missing_index.hex"
            ),
            Some(Error::Truncated),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/invalid_kind.hex"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/invalid_argument.hex"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/zero_descriptor.hex"),
            Some(Error::Malformed("annotation constant index")),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/wrong_cp_type.hex"),
            Some(Error::Malformed("UTF8 index")),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/excess_count_eof.hex"),
            Some(Error::Limit("annotation nodes")),
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/empty_trailing.hex"),
            Some(Error::Malformed("type annotation trailing bytes")),
        ),
    ];
    for (encoded, expected) in cases {
        let bytes = hex(encoded);
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            match expected {
                Some(error) => assert_eq!(decode(&c).unwrap_err(), error, "{encoded}"),
                None => {
                    let out = decode(&c).unwrap();
                    let a = &out.methods[0][0];
                    assert_eq!(c.methods[0].attributes[0].bytes, bytes);
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
                        assert_eq!(root.target_type, 0x14);
                        assert_eq!(root.target_info, MethodTargetInfo::Empty);
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
fn historical_incomplete_targets_and_mixed_attributes_fail_without_partial_output() {
    for (encoded, expected) in [
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/unsupported_01.hex"),
            Error::Truncated,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/unsupported_12.hex"),
            Error::Truncated,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/unsupported_15.hex"),
            Error::Truncated,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/unsupported_16.hex"),
            Error::Truncated,
        ),
        (
            include_str!("fixtures/method_type_annotations/authored-bytes/unsupported_17.hex"),
            Error::Truncated,
        ),
        (
            include_str!(
                "fixtures/method_type_annotations/authored-bytes/mixed_return_parameter.hex"
            ),
            Error::Truncated,
        ),
        (
            include_str!(
                "fixtures/method_type_annotations/authored-bytes/mixed_parameter_return.hex"
            ),
            Error::Truncated,
        ),
    ] {
        let bytes = hex(encoded);
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            assert_eq!(decode(&c).unwrap_err(), expected);
            assert_eq!(c.methods[0].attributes[0].bytes, bytes);
            assert!(type_annotations::decode(&c).is_ok());
        }
    }
}

#[test]
fn consumed_constant_pool_refs_and_elements_are_checked() {
    let good = payload(&[root(&[], Some(&scalar(b'I', 5)))]);
    for offset in [4, 8, 11] {
        for index in [0, 10, 12, 25, u16::MAX] {
            let mut bytes = good.clone();
            bytes[offset..offset + 2].copy_from_slice(&index.to_be_bytes());
            let mut c = base();
            c.methods[0].attributes = vec![attr(1, &bytes)];
            assert!(decode(&c).is_err(), "offset{offset} index{index}");
        }
    }
    for name in [0, 5, 10, 25, u16::MAX] {
        let mut c = base();
        c.methods[0].attributes = vec![attr(name, &good)];
        assert!(decode(&c).is_err());
    }
    for descriptor in ["I", "[LA;", "L;", "LA", "LA;;"] {
        let mut c = base();
        c.constants[3] = text(descriptor);
        c.methods[0].attributes = vec![attr(1, &good)];
        assert!(decode(&c).is_err());
    }
    let mut duplicate = vec![
        0, 1, 0x14, 0, 0, 3, 0, 2, 0, 4, b'I', 0, 5, 0, 4, b'I', 0, 5,
    ];
    let mut c = base();
    c.constants.push(text("x"));
    duplicate[14] = 25;
    c.methods[0].attributes = vec![attr(1, &duplicate)];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("duplicate annotation element")
    );
    c.methods[0].attributes = vec![attr(1, &good)];
    c.methods[0].name = u16::MAX;
    c.methods[0].descriptor = u16::MAX;
    c.constants[0] = text("forged slot zero");
    assert!(decode(&c).is_ok());
    let mut zero = good.clone();
    zero[4..6].copy_from_slice(&[0, 0]);
    c.methods[0].attributes = vec![attr(1, &zero)];
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
        c.methods[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap().methods[0][0].annotations[0]
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
    c.methods[0].attributes = vec![attr(2, &bytes)];
    let out = decode(&c).unwrap();
    let Value::Annotation(nested) = &out.methods[0][0].annotations[0].annotation.elements[0].value
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
        c.methods[0].attributes = vec![attr(1, &bytes)];
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
            c.methods[0].attributes = vec![attr(1, &bytes)];
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
                c.methods[0].attributes = vec![attr(1, &changed)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn order_alignment_absent_empty_and_old_endpoint_compatibility() {
    let empty = [0, 0];
    let roots = payload(&[root(&[(3, 255)], None), root(&[(3, 255)], None)]);
    let field_bytes = [0, 1, 0x13, 0, 0, 3, 0, 0];
    let mut c = base();
    c.methods[0].attributes = vec![attr(2, &empty), attr(1, &roots), attr(23, &[255])];
    c.methods.push(field(vec![]));
    c.methods.push(field(vec![attr(1, &empty)]));
    c.fields.push(field(vec![attr(1, &field_bytes)]));
    c.attributes = vec![attr(1, &[255]), attr(1, &[255]), attr(24, &[255])];
    let out = decode(&c).unwrap();
    assert_eq!(out.field_types, type_annotations::decode(&c).unwrap());
    assert_eq!(
        out.field_types.extensions,
        annotation_extensions::decode(&c).unwrap()
    );
    assert_eq!(out.methods.len(), 3);
    assert_eq!(out.methods[0][0].visibility, Visibility::RuntimeInvisible);
    assert!(out.methods[0][0].annotations.is_empty());
    assert_eq!(out.methods[0][1].visibility, Visibility::RuntimeVisible);
    assert_eq!(out.methods[0][1].annotations.len(), 2);
    assert_eq!(
        out.methods[0][1].annotations[0],
        out.methods[0][1].annotations[1]
    );
    assert!(out.methods[1].is_empty());
    assert_eq!(out.methods[2].len(), 1);
    assert!(out.methods[2][0].annotations.is_empty());
    assert_eq!(c.methods[0].attributes[1].bytes, roots);
    c.methods[0].attributes = vec![attr(1, &[255])];
    assert!(type_annotations::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
    c.methods.clear();
    assert!(decode(&c).unwrap().methods.is_empty());
}

#[test]
fn all_target_dispositions_path_and_version_boundaries() {
    for target in 0..=255 {
        let bytes = [0, 1, target];
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            let expected = match target {
                0x01 | 0x12 | 0x14 | 0x15 | 0x16 | 0x17 => Error::Truncated,
                _ => Error::Malformed("method type annotation target"),
            };
            assert_eq!(decode(&c).unwrap_err(), expected);
        }
    }
    for length in [0, 1, 255] {
        let path: Vec<_> = (0..length)
            .map(|i| ((i % 4) as u8, if i % 4 == 3 { 255 } else { 0 }))
            .collect();
        let bytes = payload(&[root(&path, None)]);
        let mut c = base();
        c.methods[0].attributes = vec![attr(1, &bytes)];
        let out = decode(&c).unwrap();
        let actual: Vec<_> = out.methods[0][0].annotations[0]
            .path
            .iter()
            .map(|s| (s.kind, s.argument_index))
            .collect();
        assert_eq!(actual, path);
    }
    for kind in 0..=255 {
        for index in [0, 1, 255] {
            let bytes = payload(&[root(&[(kind, index)], None)]);
            let mut c = base();
            c.methods[0].attributes = vec![attr(1, &bytes)];
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
        (67, 1),
        (67, 65535),
    ] {
        let mut c = base();
        c.major = major;
        c.minor = minor;
        c.methods[0].attributes = vec![attr(1, &[0, 0])];
        if major > 67 || minor != 0 && major != 45 {
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::UnsupportedVersion { major, minor }
            );
        } else if major < 52 {
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("type annotation version")
            );
            c.methods[0].attributes.clear();
            c.attributes = vec![attr(1, &[255]), attr(1, &[255])];
            assert!(decode(&c).is_ok());
        } else {
            assert!(decode(&c).is_ok());
        }
    }
}

#[test]
fn every_prefix_trailing_and_duplicate_content_precedence() {
    let nested = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), scalar(b'c', 16)]))),
    ]
    .concat();
    for bytes in [
        payload(&[root(&[], None)]),
        payload(&[root(&[], Some(&scalar(b'I', 5)))]),
        payload(&[root(&[(0, 0), (1, 0), (2, 0), (3, 255)], None)]),
        payload(&[root(&[(3, 255); 255], None)]),
        payload(&[root(&[(0, 0)], Some(&nested))]),
    ] {
        for name in [1, 2] {
            for end in 0..bytes.len() {
                let mut c = base();
                c.methods[0].attributes = vec![attr(name, &bytes[..end])];
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Truncated,
                    "name{name} prefix{end}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &trailing)];
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
            c.methods[0].attributes = vec![attr(name, &bytes), attr(duplicate, &[])];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("duplicate type annotation attribute")
            );
            c.methods[0].attributes.swap(0, 1);
            assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
            c.methods[0].attributes = vec![attr(name, &[0, 1, 0x16]), attr(duplicate, &bytes)];
            assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
        }
    }
    let mut c = base();
    c.methods[0].attributes = vec![attr(1, &[0, 2, 0x14, 0, 0, 0, 0, 0, 0x16])];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant index")
    );
    c.methods[0].attributes = vec![attr(1, &[0, 2, 0x14])];
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
}

#[test]
fn inherited_errors_precede_methods_and_unconsumed_refs_stay_unconsumed() {
    let good = payload(&[root(&[], None)]);
    for (name, descriptor) in [
        ("f", "()V"),
        ("f", "()I"),
        ("f", "()Ljava/lang/Object;"),
        ("<init>", "()V"),
        ("<clinit>", "()V"),
        ("bad/name", "garbage"),
    ] {
        let mut c = base();
        c.constants.extend([text(name), text(descriptor)]);
        c.methods[0].name = 25;
        c.methods[0].descriptor = 26;
        c.methods[0].attributes = vec![attr(1, &good)];
        assert!(decode(&c).is_ok());
    }
    let mut c = base();
    c.methods[0].attributes = vec![attr(1, &[0, 1, 0x13])];
    c.attributes = vec![attr(17, &[])];
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
    c.attributes.clear();
    c.methods[0].attributes.push(attr(18, &[0]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant index")
    );
    c.methods[0].name = 20;
    c.methods[0].descriptor = 21;
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("method type annotation target")
    );
    c.methods[0].attributes.pop();
    c.fields.push(field(vec![attr(1, &[0, 1, 0x14])]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("field type annotation target")
    );
    c.fields[0].attributes.push(attr(18, &[0]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation extension context")
    );
    c.major = 68;
    c.constants.clear();
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::UnsupportedVersion {
            major: 68,
            minor: 0
        }
    );
    c.major = 52;
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant pool size")
    );
}

#[test]
fn aggregate_method_and_mixed_node_boundaries_share_every_phase() {
    for count in [MAX_RECORDS - 6, MAX_RECORDS - 5, MAX_RECORDS - 4] {
        let bytes = payload(&vec![root(&[], None); count]);
        let mut c = base();
        c.methods[0].attributes = vec![attr(1, &bytes)];
        match decode(&c) {
            Ok(out) => {
                assert!(count <= MAX_RECORDS - 5);
                let used = 2
                    + out.methods.capacity()
                    + out.methods[0].capacity()
                    + out.methods[0][0].annotations.capacity();
                assert!(used <= MAX_RECORDS);
                assert_eq!(out.methods[0][0].annotations.len(), count);
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    let mut roots = vec![root(&[], None); MAX_RECORDS - 5];
    roots[0] = root(&[(3, 1)], None);
    let bytes = payload(&roots);
    let mut c = base();
    c.methods[0].attributes = vec![attr(1, &bytes)];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    c.methods[0].attributes = vec![attr(1, &[255, 255])];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));

    for count in [MAX_RECORDS - 26, MAX_RECORDS - 25, MAX_RECORDS - 24] {
        let default = array(&vec![scalar(b'I', 5); count]);
        let declaration = [vec![0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
        let parameter = [vec![1, 0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
        let method_types = payload(&[root(&[(3, 255)], None)]);
        let mut field_types = method_types.clone();
        field_types[2] = 0x13;
        let mut c = base();
        c.attributes = vec![attr(17, &declaration)];
        c.fields.push(field(vec![attr(1, &field_types)]));
        c.methods[0] = Member {
            access_flags: 0x401,
            name: 20,
            descriptor: 21,
            attributes: vec![
                attr(18, &parameter),
                attr(19, &default),
                attr(1, &method_types),
            ],
        };
        assert!(annotation_extensions::decode(&c).is_ok());
        if let Err(error) = type_annotations::decode(&c) {
            assert_eq!(error, Error::Limit("annotation nodes"));
        }
        match decode(&c) {
            Ok(out) => {
                assert!(count <= MAX_RECORDS - 25);
                let f = &out.field_types.fields;
                let m = &out.methods;
                let capacity = f.capacity()
                    + f[0].capacity()
                    + f[0][0].annotations.capacity()
                    + f[0][0].annotations[0].path.capacity()
                    + m.capacity()
                    + m[0].capacity()
                    + m[0][0].annotations.capacity()
                    + m[0][0].annotations[0].path.capacity();
                assert!(15 + count + capacity <= MAX_RECORDS);
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    let mut c = base();
    c.methods = (0..=MAX_RECORDS / 2).map(|_| field(vec![])).collect();
    assert!(type_annotations::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
}

#[test]
fn payload_bytes_once_and_final_method_work_exact_minus_plus_one() {
    let types = payload(&[root(&[], None)]);
    for size in [
        MAX_BYTES - types.len() - 1,
        MAX_BYTES - types.len(),
        MAX_BYTES - types.len() + 1,
    ] {
        let ignored = vec![0; size];
        let mut c = base();
        c.attributes = vec![attr(22, &ignored)];
        c.methods[0].attributes = vec![attr(1, &types)];
        if size + types.len() <= MAX_BYTES {
            assert!(decode(&c).is_ok());
        } else {
            assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
        }
    }
    let types = payload(&[root(&[(0, 0), (1, 0)], Some(&scalar(b's', 7)))]);
    for (filler_length, string_length, expected_work) in [
        (87345, 2, ktlsp_binary::MAX_VALIDATION_UNITS - 1),
        (87346, 0, ktlsp_binary::MAX_VALIDATION_UNITS),
        (87346, 1, ktlsp_binary::MAX_VALIDATION_UNITS + 1),
        (87347, 0, ktlsp_binary::MAX_VALIDATION_UNITS + 3),
    ] {
        assert_eq!(
            340 * 3 * 65536 + 3 * (filler_length + 1) + 103 + string_length,
            expected_work
        );
        let mut c = base();
        c.constants[7] = Constant::Utf8(vec![66; string_length]);
        c.constants[22] = Constant::Utf8(vec![65; 65535]);
        c.constants.push(Constant::Utf8(vec![66; filler_length]));
        c.methods[0].attributes = (0..340).map(|_| attr(22, &[])).collect();
        c.methods[0]
            .attributes
            .extend([attr(25, &[]), attr(1, &types)]);
        assert!(type_annotations::decode(&c).is_ok());
        if expected_work <= ktlsp_binary::MAX_VALIDATION_UNITS {
            assert!(decode(&c).is_ok());
        } else {
            assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
        }
    }
}

#[test]
fn licensed_precompiled_method_data_matches_frozen_bytes() {
    let bytes = include_bytes!("fixtures/method_type_annotations/AbstractIterator.class.bin");
    assert_eq!(bytes.len(), 2361);
    assert_eq!(&bytes[1548..1556], [0, 1, 0x14, 0, 0, 64, 0, 0]);
    assert_eq!(&bytes[1669..1677], [0, 1, 0x14, 0, 0, 64, 0, 0]);
    let c = parse_class(bytes).unwrap();
    assert_eq!((c.major, c.minor), (52, 0));
    assert_eq!(c.fields.len(), 2);
    assert_eq!(c.methods.len(), 7);
    for (index, name) in [(1, "computeNext"), (2, "endOfData")] {
        assert_eq!(
            c.utf16(c.methods[index].name).unwrap(),
            name.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            c.utf16(c.methods[index].descriptor).unwrap(),
            "()Ljava/lang/Object;".encode_utf16().collect::<Vec<_>>()
        );
    }
    let out = decode(&c).unwrap();
    assert_eq!(out.field_types, type_annotations::decode(&c).unwrap());
    assert_eq!(out.methods.len(), 7);
    for (index, attrs) in out.methods.iter().enumerate() {
        if index != 1 && index != 2 {
            assert!(attrs.is_empty());
            continue;
        }
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0].visibility, Visibility::RuntimeVisible);
        assert_eq!(attrs[0].annotations.len(), 1);
        let a = &attrs[0].annotations[0];
        assert_eq!(a.target_type, 0x14);
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
fn field_failure_precedes_method_outer_reservation() {
    let mut c = base();
    c.methods = (0..=MAX_RECORDS / 2).map(|_| field(vec![])).collect();
    c.fields.push(field(vec![attr(1, &[0, 1, 0x14])]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("field type annotation target")
    );
}

#[test]
fn frozen_formal_parameter_bytes_preserve_indices_paths_and_first_errors() {
    macro_rules! frozen {
        ($name:literal) => {
            include_str!(concat!(
                "fixtures/method_type_annotations/formal_parameter/authored/",
                $name,
                ".hex"
            ))
        };
    }
    let cases = [
        (frozen!("parameter0"), None),
        (frozen!("parameter255"), None),
        (frozen!("all_steps"), None),
        (frozen!("int_bits"), None),
        (frozen!("utf16"), None),
        (frozen!("return_parameter"), None),
        (frozen!("parameter_return"), None),
        (frozen!("repeated_parameter"), None),
        (frozen!("missing_index"), Some(Error::Truncated)),
        (frozen!("missing_path_length"), Some(Error::Truncated)),
        (frozen!("missing_path_index"), Some(Error::Truncated)),
        (
            frozen!("invalid_kind"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            frozen!("invalid_argument"),
            Some(Error::Malformed("type annotation path")),
        ),
        (
            frozen!("zero_descriptor"),
            Some(Error::Malformed("annotation constant index")),
        ),
        (frozen!("wrong_cp"), Some(Error::Malformed("UTF8 index"))),
        (
            frozen!("trailing"),
            Some(Error::Malformed("type annotation trailing bytes")),
        ),
        (frozen!("count_eof"), Some(Error::Limit("annotation nodes"))),
        (
            frozen!("illegal_field"),
            Some(Error::Malformed("method type annotation target")),
        ),
        (frozen!("unsupported_receiver"), Some(Error::Truncated)),
        (
            frozen!("malformed_before_unsupported"),
            Some(Error::Malformed("annotation constant index")),
        ),
        (
            frozen!("unsupported_before_parameter"),
            Some(Error::Truncated),
        ),
        (
            frozen!("parameter_before_unsupported"),
            Some(Error::Truncated),
        ),
    ];
    for (case, (encoded, expected)) in cases.into_iter().enumerate() {
        let bytes = hex(encoded);
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            assert!(type_annotations::decode(&c).is_ok());
            match expected {
                Some(error) => assert_eq!(decode(&c).unwrap_err(), error),
                None => {
                    let out = decode(&c).unwrap();
                    let a = &out.methods[0][0];
                    assert_eq!(c.methods[0].attributes[0].bytes, bytes);
                    assert_eq!(
                        a.visibility,
                        if name == 1 {
                            Visibility::RuntimeVisible
                        } else {
                            Visibility::RuntimeInvisible
                        }
                    );
                    let indices: Vec<_> = a
                        .annotations
                        .iter()
                        .map(|r| {
                            (
                                r.target_type,
                                match r.target_info {
                                    MethodTargetInfo::FormalParameter(index) => Some(index),
                                    MethodTargetInfo::Empty => None,
                                    _ => panic!("unexpected target info"),
                                },
                            )
                        })
                        .collect();
                    let expected = match case {
                        0 => vec![(0x16, Some(0))],
                        1 | 4 => vec![(0x16, Some(255))],
                        2 => vec![(0x16, Some(1))],
                        3 => vec![(0x16, Some(2))],
                        5 => vec![(0x14, None), (0x16, Some(255))],
                        6 => vec![(0x16, Some(0)), (0x14, None)],
                        7 => vec![(0x16, Some(0)); 2],
                        _ => unreachable!(),
                    };
                    assert_eq!(indices, expected);
                    for root in &a.annotations {
                        assert_eq!(root.annotation.descriptor, [76, 65, 59]);
                        let path: Vec<_> = root
                            .path
                            .iter()
                            .map(|s| (s.kind, s.argument_index))
                            .collect();
                        assert_eq!(
                            path,
                            if case == 2 {
                                vec![(0, 0), (1, 0), (2, 0), (3, 255)]
                            } else {
                                vec![]
                            }
                        );
                        match case {
                            3 => assert_eq!(
                                root.annotation.elements[0].value,
                                Value::Int(0x8000_0001)
                            ),
                            4 => assert_eq!(
                                root.annotation.elements[0].value,
                                Value::String(&[0, 0xd800])
                            ),
                            _ => assert!(root.annotation.elements.is_empty()),
                        }
                    }
                }
            }
            assert_eq!(c.methods[0].attributes[0].bytes, bytes);
        }
    }
}

#[test]
fn formal_parameter_prefixes_ordinal_controls_and_duplicates() {
    let nested = [vec![b'@'], annotation(Some(&array(&[scalar(b's', 7)])))].concat();
    for bytes in [
        payload(&[parameter_root(0, &[], None)]),
        payload(&[parameter_root(255, &[(3, 255); 255], Some(&nested))]),
    ] {
        for name in [1, 2] {
            for end in 0..bytes.len() {
                let mut c = base();
                c.methods[0].attributes = vec![attr(name, &bytes[..end])];
                assert_eq!(decode(&c).unwrap_err(), Error::Truncated, "prefix{end}");
            }
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes), attr(name, &[])];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("duplicate type annotation attribute")
            );
            c.methods[0].attributes = vec![attr(name, &[]), attr(name, &bytes)];
            assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
        }
    }
    for index in [0, 1, 2, 255] {
        let bytes = payload(&[parameter_root(index, &[], None)]);
        for (name, descriptor, flags) in [
            ("f", "(JDLjava/lang/Object;)V", 0),
            ("<init>", "(LOuter;J)V", 0x1000),
            ("<clinit>", "()V", 0),
            ("bad/name", "garbage", 0),
        ] {
            let mut c = base();
            c.constants.extend([text(name), text(descriptor)]);
            c.methods[0].name = 25;
            c.methods[0].descriptor = 26;
            c.methods[0].access_flags = flags;
            c.methods[0].attributes = vec![attr(1, &bytes)];
            assert_eq!(
                decode(&c).unwrap().methods[0][0].annotations[0].target_info,
                MethodTargetInfo::FormalParameter(index)
            );
            c.methods[0].name = u16::MAX;
            c.methods[0].descriptor = u16::MAX;
            assert!(decode(&c).is_ok());
            c.methods[0].attributes.push(attr(18, &[0]));
            assert!(decode(&c).is_err());
        }
    }
}

#[test]
fn formal_parameter_work_bytes_and_node_boundaries() {
    let types = payload(&[parameter_root(
        255,
        &[(0, 0), (1, 0)],
        Some(&scalar(b's', 7)),
    )]);
    for (length, expected) in [(1, true), (2, true), (3, false)] {
        assert_eq!(
            340 * 3 * 65536 + 3 * 87346 + 104 + length,
            ktlsp_binary::MAX_VALIDATION_UNITS + length - 2
        );
        let mut c = base();
        c.constants[7] = Constant::Utf8(vec![66; length]);
        c.constants[22] = Constant::Utf8(vec![65; 65535]);
        c.constants.push(Constant::Utf8(vec![66; 87345]));
        c.methods[0].attributes = (0..340).map(|_| attr(22, &[])).collect();
        c.methods[0]
            .attributes
            .extend([attr(25, &[]), attr(1, &types)]);
        assert!(type_annotations::decode(&c).is_ok());
        if expected {
            assert!(decode(&c).is_ok());
        } else {
            assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
        }
    }
    let types = payload(&[parameter_root(0, &[], None)]);
    for size in [MAX_BYTES - 10, MAX_BYTES - 9, MAX_BYTES - 8] {
        let ignored = vec![0; size];
        let mut c = base();
        c.attributes = vec![attr(22, &ignored)];
        c.methods[0].attributes = vec![attr(1, &types)];
        if size + types.len() <= MAX_BYTES {
            assert!(decode(&c).is_ok());
        } else {
            assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
        }
    }
    for count in [MAX_RECORDS - 6, MAX_RECORDS - 5, MAX_RECORDS - 4] {
        let bytes = payload(&vec![parameter_root(255, &[], None); count]);
        let mut c = base();
        c.methods[0].attributes = vec![attr(1, &bytes)];
        match decode(&c) {
            Ok(out) => {
                assert!(count <= MAX_RECORDS - 5);
                assert_eq!(out.methods[0][0].annotations.len(), count);
                assert!(
                    2 + out.methods.capacity()
                        + out.methods[0].capacity()
                        + out.methods[0][0].annotations.capacity()
                        <= MAX_RECORDS
                );
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
}

#[test]
fn genuine_formal_parameter_fixture_matches_frozen_data() {
    let bytes =
        include_bytes!("fixtures/method_type_annotations/formal_parameter/fixture.class.bin");
    assert_eq!(bytes.len(), 484);
    let c = parse_class(bytes).unwrap();
    let out = decode(&c).unwrap();
    assert_eq!(c.methods.len(), 1);
    assert_eq!(
        c.methods[0].attributes[1].bytes,
        hex("00 01 16 02 00 00 0c 00 00")
    );
    assert_eq!(out.field_types, type_annotations::decode(&c).unwrap());
    assert!(out.field_types.fields.is_empty());
    assert_eq!(out.methods.len(), 1);
    assert_eq!(out.methods[0].len(), 1);
    let a = &out.methods[0][0];
    assert_eq!(a.visibility, Visibility::RuntimeVisible);
    assert_eq!(a.annotations.len(), 1);
    let root = &a.annotations[0];
    assert_eq!(root.target_type, 0x16);
    assert_eq!(root.target_info, MethodTargetInfo::FormalParameter(2));
    assert!(root.path.is_empty());
    assert!(root.annotation.elements.is_empty());
    assert_eq!(
        root.annotation.descriptor,
        "Lorg/jspecify/annotations/Nullable;"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
}

#[test]
fn all_method_targets_share_values_and_depth_with_formal_parameter_mutations() {
    let nested = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), vec![b'[', 0, 0]]))),
    ]
    .concat();
    for value in [
        scalar(b'B', 5),
        scalar(b'C', 5),
        scalar(b'I', 5),
        scalar(b'S', 5),
        scalar(b'Z', 5),
        scalar(b'F', 8),
        scalar(b'J', 9),
        scalar(b'D', 11),
        scalar(b's', 7),
        scalar(b'c', 15),
        scalar(b'c', 16),
        vec![b'e', 0, 13, 0, 14],
        vec![b'[', 0, 0],
        nested,
    ] {
        for (tag, info) in target_infos() {
            let parameter = payload(&[target_root(tag, info, &[], Some(&value))]);
            let returns = payload(&[root(&[], Some(&value))]);
            let mut c = base();
            c.methods[0].attributes = vec![attr(1, &parameter)];
            let mut r = base();
            r.methods[0].attributes = vec![attr(1, &returns)];
            assert_eq!(
                decode(&c).unwrap().methods[0][0].annotations[0].annotation,
                decode(&r).unwrap().methods[0][0].annotations[0].annotation
            );
        }
    }
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
            for (tag, info) in target_infos() {
                let bytes = payload(&[target_root(tag, info, &[], Some(&value))]);
                let mut c = base();
                c.methods[0].attributes = vec![attr(1, &bytes)];
                if depth <= MAX_DEPTH {
                    assert!(decode(&c).is_ok());
                } else {
                    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation depth"));
                }
            }
        }
    }
    for bytes in [
        payload(&[parameter_root(0, &[], None)]),
        payload(&[parameter_root(
            255,
            &[(3, 255)],
            Some(&array(&[scalar(b's', 7)])),
        )]),
    ] {
        for at in 0..bytes.len() {
            for replacement in 0..=255 {
                let mut changed = bytes.clone();
                changed[at] = replacement;
                let mut c = base();
                c.methods[0].attributes = vec![attr(1, &changed)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn remaining_targets_match_frozen_literals_and_every_success_prefix() {
    use MethodTargetInfo::{Empty, Throws, TypeParameter, TypeParameterBound};
    for line in include_str!("fixtures/method_type_annotations/remaining/authored.hex").lines() {
        let (label, encoded) = line.split_once('|').unwrap();
        let bytes = hex(encoded);
        let expected = match label {
            "type_parameter0" => Ok((0x01, TypeParameter(0), vec![])),
            "type_parameter255" => Ok((0x01, TypeParameter(255), vec![])),
            "bound0" => Ok((
                0x12,
                TypeParameterBound {
                    type_parameter_index: 0,
                    bound_index: 0,
                },
                vec![],
            )),
            "bound255" => Ok((
                0x12,
                TypeParameterBound {
                    type_parameter_index: 255,
                    bound_index: 255,
                },
                vec![],
            )),
            "receiver" => Ok((0x15, Empty, vec![])),
            "throws0" => Ok((0x17, Throws(0), vec![])),
            "throws65535" => Ok((0x17, Throws(65535), vec![])),
            "throws258" => Ok((0x17, Throws(258), vec![])),
            "all_steps" => Ok((
                0x12,
                TypeParameterBound {
                    type_parameter_index: 254,
                    bound_index: 253,
                },
                vec![(0, 0), (1, 0), (2, 0), (3, 255)],
            )),
            "invalid_kind_missing_index" => Err(Error::Truncated),
            "invalid_kind" | "invalid_argument" => Err(Error::Malformed("type annotation path")),
            "zero_descriptor" => Err(Error::Malformed("annotation constant index")),
            "wrong_cp" => Err(Error::Malformed("UTF8 index")),
            "trailing" => Err(Error::Malformed("type annotation trailing bytes")),
            "illegal_field" | "unknown" => Err(Error::Malformed("method type annotation target")),
            "count_eof" => Err(Error::Limit("annotation nodes")),
            _ => panic!("unexpected fixture {label}"),
        };
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            assert!(type_annotations::decode(&c).is_ok());
            match &expected {
                Err(error) => {
                    assert_eq!(decode(&c).unwrap_err(), *error, "{label}");
                    assert_eq!(c.methods[0].attributes[0].bytes, bytes);
                }
                Ok((tag, info, path)) => {
                    let out = decode(&c).unwrap();
                    let a = &out.methods[0][0];
                    assert_eq!(c.methods[0].attributes[0].bytes, bytes);
                    assert_eq!(
                        a.visibility,
                        if name == 1 {
                            Visibility::RuntimeVisible
                        } else {
                            Visibility::RuntimeInvisible
                        }
                    );
                    assert_eq!(a.annotations.len(), 1);
                    let r = &a.annotations[0];
                    assert_eq!(r.target_type, *tag);
                    assert_eq!(&r.target_info, info);
                    assert_eq!(
                        r.path
                            .iter()
                            .map(|p| (p.kind, p.argument_index))
                            .collect::<Vec<_>>(),
                        *path
                    );
                    assert_eq!(r.annotation.descriptor, [76, 65, 59]);
                    assert!(r.annotation.elements.is_empty());
                    for end in 0..bytes.len() {
                        c.methods[0].attributes = vec![attr(name, &bytes[..end])];
                        assert_eq!(
                            decode(&c).unwrap_err(),
                            Error::Truncated,
                            "{label} prefix{end}"
                        );
                    }
                }
            }
            assert_eq!(bytes, hex(encoded));
        }
    }
}

#[test]
fn six_target_mapping_order_raw_indices_and_first_errors() {
    for tag in 0..=255 {
        let info: &[u8] = match tag {
            0x01 | 0x16 => &[255],
            0x12 | 0x17 => &[255, 255],
            _ => &[],
        };
        let bytes = payload(&[target_root(tag, info, &[], None)]);
        for name in [1, 2] {
            let mut c = base();
            c.methods[0].attributes = vec![attr(name, &bytes)];
            match tag {
                0x01 | 0x12 | 0x14 | 0x15 | 0x16 | 0x17 => {
                    let out = decode(&c).unwrap();
                    let r = &out.methods[0][0].annotations[0];
                    let expected = match tag {
                        0x01 => MethodTargetInfo::TypeParameter(255),
                        0x12 => MethodTargetInfo::TypeParameterBound {
                            type_parameter_index: 255,
                            bound_index: 255,
                        },
                        0x14 | 0x15 => MethodTargetInfo::Empty,
                        0x16 => MethodTargetInfo::FormalParameter(255),
                        0x17 => MethodTargetInfo::Throws(65535),
                        _ => unreachable!(),
                    };
                    assert_eq!(r.target_type, tag);
                    assert_eq!(r.target_info, expected);
                }
                _ => assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Malformed("method type annotation target")
                ),
            }
        }
    }
    let roots: Vec<_> = target_infos()
        .iter()
        .map(|(t, i)| target_root(*t, i, &[], None))
        .collect();
    let mixed = payload(&[roots.clone(), roots.clone()].concat());
    let mut c = base();
    c.methods[0].attributes = vec![attr(2, &mixed), attr(1, &mixed)];
    let out = decode(&c).unwrap();
    for (a, visibility) in out.methods[0]
        .iter()
        .zip([Visibility::RuntimeInvisible, Visibility::RuntimeVisible])
    {
        assert_eq!(a.visibility, visibility);
        assert_eq!(
            a.annotations
                .iter()
                .map(|r| r.target_type)
                .collect::<Vec<_>>(),
            vec![1, 18, 20, 21, 22, 23, 1, 18, 20, 21, 22, 23]
        );
        assert_eq!(&a.annotations[..6], &a.annotations[6..]);
    }
    for (tag, info) in [
        (0x01, &[0][..]),
        (0x01, &[1]),
        (0x01, &[255]),
        (0x12, &[0, 0]),
        (0x12, &[0, 255]),
        (0x12, &[255, 0]),
        (0x12, &[255, 255]),
        (0x15, &[]),
        (0x17, &[0, 0]),
        (0x17, &[0, 1]),
        (0x17, &[1, 2]),
        (0x17, &[255, 255]),
    ] {
        let bytes = payload(&[target_root(tag, info, &[], None)]);
        let mut c = base();
        c.constants.extend([text("Signature"), text("Exceptions")]);
        c.methods[0].name = u16::MAX;
        c.methods[0].descriptor = u16::MAX;
        c.methods[0].access_flags = u16::MAX;
        c.methods[0].attributes = vec![attr(1, &bytes), attr(25, &[255]), attr(26, &[255])];
        assert!(decode(&c).is_ok());
        c.methods[0].attributes.truncate(1);
        assert!(decode(&c).is_ok());
        // An inherited parameter extension still validates method references first.
        c.methods[0].attributes.push(attr(18, &[0]));
        assert!(decode(&c).is_err());
    }
    for (tag, info) in target_infos() {
        let good = target_root(tag, info, &[], None);
        let mut bad = good.clone();
        let descriptor = 2 + info.len();
        bad[descriptor..descriptor + 2].copy_from_slice(&[0, 0]);
        for records in [
            vec![good.clone(), bad.clone()],
            vec![bad.clone(), good.clone()],
        ] {
            let bytes = payload(&records);
            let mut c = base();
            c.methods[0].attributes = vec![attr(1, &bytes)];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("annotation constant index")
            );
            assert_eq!(c.methods[0].attributes[0].bytes, bytes);
        }
        let bytes = payload(&[vec![0xff], bad.clone()]);
        let mut c = base();
        c.methods[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("method type annotation target")
        );
        let bytes = payload(&[bad, vec![0xff]]);
        c.methods[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("annotation constant index")
        );
    }
}

#[test]
fn remaining_targets_path_prefix_duplicates_and_finite_mutations() {
    for (tag, info) in target_infos() {
        let nested = [vec![b'@'], annotation(Some(&array(&[scalar(b's', 7)])))].concat();
        for path in [vec![], vec![(3, 255); 255]] {
            let bytes = payload(&[target_root(tag, info, &path, Some(&nested))]);
            for name in [1, 2] {
                for end in 0..bytes.len() {
                    let mut c = base();
                    c.methods[0].attributes = vec![attr(name, &bytes[..end])];
                    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
                }
                let mut c = base();
                c.constants.push(text(if name == 1 {
                    "RuntimeVisibleTypeAnnotations"
                } else {
                    "RuntimeInvisibleTypeAnnotations"
                }));
                c.methods[0].attributes = vec![attr(name, &bytes), attr(25, &[])];
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Malformed("duplicate type annotation attribute")
                );
                c.methods[0].attributes.swap(0, 1);
                assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
            }
        }
        let bytes = payload(&[target_root(
            tag,
            info,
            &[(3, 255)],
            Some(&array(&[scalar(b's', 7)])),
        )]);
        for at in 0..bytes.len() {
            for replacement in 0..=255 {
                let mut changed = bytes.clone();
                changed[at] = replacement;
                let mut c = base();
                c.methods[0].attributes = vec![attr(1, &changed)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn remaining_targets_keep_shared_work_byte_and_capacity_boundaries() {
    for (tag, info) in target_infos() {
        let width = info.len();
        let types = payload(&[target_root(
            tag,
            info,
            &[(0, 0), (1, 0)],
            Some(&scalar(b's', 7)),
        )]);
        for offset in 0..=2 {
            let length = 2 - width + offset;
            assert_eq!(
                340 * 3 * 65536 + 3 * 87346 + 103 + width + length,
                ktlsp_binary::MAX_VALIDATION_UNITS - 1 + offset
            );
            let mut c = base();
            c.constants[7] = Constant::Utf8(vec![66; length]);
            c.constants[22] = Constant::Utf8(vec![65; 65535]);
            c.constants.push(Constant::Utf8(vec![66; 87345]));
            c.methods[0].attributes = (0..340).map(|_| attr(22, &[])).collect();
            c.methods[0]
                .attributes
                .extend([attr(25, &[]), attr(1, &types)]);
            assert!(type_annotations::decode(&c).is_ok());
            if offset <= 1 {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
            }
        }
        let types = payload(&[target_root(tag, info, &[], None)]);
        assert_eq!(types.len(), 8 + width);
        for size in [
            MAX_BYTES - types.len() - 1,
            MAX_BYTES - types.len(),
            MAX_BYTES - types.len() + 1,
        ] {
            let ignored = vec![0; size];
            let mut c = base();
            c.attributes = vec![attr(22, &ignored)];
            c.methods[0].attributes = vec![attr(1, &types)];
            if size + types.len() <= MAX_BYTES {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
            }
        }
        for count in [MAX_RECORDS - 6, MAX_RECORDS - 5, MAX_RECORDS - 4] {
            let bytes = payload(&vec![target_root(tag, info, &[], None); count]);
            let mut c = base();
            c.methods[0].attributes = vec![attr(1, &bytes)];
            match decode(&c) {
                Ok(out) => {
                    assert!(count <= MAX_RECORDS - 5);
                    assert_eq!(out.methods[0][0].annotations.len(), count);
                    assert!(
                        2 + out.methods.capacity()
                            + out.methods[0].capacity()
                            + out.methods[0][0].annotations.capacity()
                            <= MAX_RECORDS
                    );
                }
                Err(e) => assert_eq!(e, Error::Limit("annotation nodes")),
            }
        }
    }
    // Earlier declaration/default/field passes and all six method targets share a call.
    let declaration = [vec![0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
    let parameter = [vec![1, 0, 1], annotation(Some(&scalar(b'I', 5)))].concat();
    let default = array(&[scalar(b'I', 5)]);
    let types = payload(
        &target_infos()
            .iter()
            .map(|(t, i)| target_root(*t, i, &[], None))
            .collect::<Vec<_>>(),
    );
    let mut field_types = payload(&[root(&[], None)]);
    field_types[2] = 0x13;
    let mut c = base();
    c.attributes = vec![attr(17, &declaration)];
    c.fields.push(field(vec![attr(1, &field_types)]));
    c.methods[0] = Member {
        access_flags: 0x401,
        name: 20,
        descriptor: 21,
        attributes: vec![attr(18, &parameter), attr(19, &default), attr(1, &types)],
    };
    let out = decode(&c).unwrap();
    assert_eq!(out.field_types, type_annotations::decode(&c).unwrap());
    assert_eq!(out.methods[0][0].annotations.len(), 6);
}

#[test]
fn genuine_bound_and_formal_parameter_fixture_matches_frozen_bytes() {
    let bytes =
        include_bytes!("fixtures/method_type_annotations/remaining/NullnessCasts.class.bin");
    assert_eq!(bytes.len(), 835);
    let c = parse_class(bytes).unwrap();
    let out = decode(&c).unwrap();
    assert_eq!(c.major, 52);
    assert_eq!(c.minor, 0);
    assert_eq!(c.constants.len(), 30);
    assert!(c.fields.is_empty());
    assert_eq!(c.methods.len(), 2);
    assert_eq!(
        c.utf16(c.methods[0].name).unwrap(),
        "uncheckedCastNullableTToT"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        c.utf16(c.methods[0].descriptor).unwrap(),
        "(Ljava/lang/Object;)Ljava/lang/Object;"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    assert_eq!(c.methods[0].access_flags, 8);
    assert!(
        c.methods[0].attributes.iter().any(|a| a.name == 23
            && a.bytes == hex("00 02 12 00 00 00 00 18 00 00 16 00 00 00 18 00 00"))
    );
    assert_eq!(out.field_types, type_annotations::decode(&c).unwrap());
    assert_eq!(out.methods.len(), 2);
    assert!(out.methods[1].is_empty());
    assert_eq!(out.methods[0].len(), 1);
    let a = &out.methods[0][0];
    assert_eq!(a.visibility, Visibility::RuntimeVisible);
    assert_eq!(a.annotations.len(), 2);
    assert_eq!(a.annotations[0].target_type, 0x12);
    assert_eq!(
        a.annotations[0].target_info,
        MethodTargetInfo::TypeParameterBound {
            type_parameter_index: 0,
            bound_index: 0
        }
    );
    assert_eq!(a.annotations[1].target_type, 0x16);
    assert_eq!(
        a.annotations[1].target_info,
        MethodTargetInfo::FormalParameter(0)
    );
    for root in &a.annotations {
        assert!(root.path.is_empty());
        assert!(root.annotation.elements.is_empty());
        assert_eq!(
            root.annotation.descriptor,
            "Lorg/jspecify/annotations/Nullable;"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
    }
}
