use ktlsp_binary::annotations::{self, MAX_DEPTH, Value, Visibility};
use ktlsp_binary::class_type_annotations::{ClassTypeTarget as Target, decode};
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, Member,
    parse_class,
};

fn text(s: &str) -> Constant {
    Constant::Utf8(s.encode_utf16().collect())
}

// Independently authored CP/payloads, frozen before product execution.
fn base<'a>() -> ClassFile<'a> {
    ClassFile {
        major: 52,
        minor: 0,
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
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
            text("Ignored"),
            text("Code"),
            text("Record"),
        ],
    }
}
fn attr(name: u16, bytes: &[u8]) -> Attribute<'_> {
    Attribute { name, bytes }
}
fn member(attributes: Vec<Attribute<'_>>) -> Member<'_> {
    Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes,
    }
}
fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
macro_rules! fixture {
    ($name:literal) => {
        hex(include_str!(concat!(
            "fixtures/class_type_annotations/authored/",
            $name,
            ".hex"
        )))
    };
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
fn root(info: &[u8], path: &[(u8, u8)], value: Option<&[u8]>) -> Vec<u8> {
    let mut out = info.to_vec();
    out.push(path.len() as u8);
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

#[test]
fn frozen_literal_successes_preserve_targets_paths_values_and_input() {
    let cases = [
        (fixture!("empty"), vec![]),
        (
            fixture!("parameter0"),
            vec![Target::TypeParameter { index: 0 }],
        ),
        (
            fixture!("parameter255"),
            vec![Target::TypeParameter { index: 255 }],
        ),
        (fixture!("supertype0"), vec![Target::Supertype { index: 0 }]),
        (
            fixture!("supertype256"),
            vec![Target::Supertype { index: 256 }],
        ),
        (
            fixture!("superclass"),
            vec![Target::Supertype { index: 65535 }],
        ),
        (
            fixture!("bound0"),
            vec![Target::TypeParameterBound {
                parameter_index: 0,
                bound_index: 0,
            }],
        ),
        (
            fixture!("bound255"),
            vec![Target::TypeParameterBound {
                parameter_index: 255,
                bound_index: 255,
            }],
        ),
        (
            fixture!("mixed"),
            vec![
                Target::TypeParameter { index: 255 },
                Target::Supertype { index: 65535 },
                Target::TypeParameterBound {
                    parameter_index: 0,
                    bound_index: 255,
                },
            ],
        ),
        (fixture!("all_steps"), vec![Target::Supertype { index: 1 }]),
        (
            fixture!("int_bits"),
            vec![Target::TypeParameterBound {
                parameter_index: 2,
                bound_index: 3,
            }],
        ),
        (fixture!("utf16"), vec![Target::TypeParameter { index: 1 }]),
    ];
    for (index, (bytes, targets)) in cases.iter().enumerate() {
        for name in [1, 2] {
            let mut c = base();
            c.attributes = vec![attr(name, bytes)];
            let original = bytes.clone();
            let out = decode(&c).unwrap();
            assert_eq!(out.declarations, annotations::decode(&c).unwrap());
            assert_eq!(out.class.len(), 1);
            assert_eq!(
                out.class[0].visibility,
                if name == 1 {
                    Visibility::RuntimeVisible
                } else {
                    Visibility::RuntimeInvisible
                }
            );
            assert_eq!(out.class[0].annotations.len(), targets.len());
            for (a, target) in out.class[0].annotations.iter().zip(targets) {
                assert_eq!(&a.target, target);
                assert_eq!(a.annotation.descriptor, [76, 65, 59]);
                assert!(std::ptr::eq(a.annotation.descriptor, c.utf16(3).unwrap()));
                if index == 9 {
                    assert_eq!(
                        a.path
                            .iter()
                            .map(|p| (p.kind, p.argument_index))
                            .collect::<Vec<_>>(),
                        [(0, 0), (1, 0), (2, 0), (3, 255)]
                    );
                } else {
                    assert!(a.path.is_empty());
                }
                match index {
                    10 => {
                        assert_eq!(a.annotation.elements[0].name, [120]);
                        assert_eq!(a.annotation.elements[0].value, Value::Int(0x8000_0001));
                    }
                    11 => {
                        assert_eq!(a.annotation.elements[0].name, [115]);
                        assert_eq!(a.annotation.elements[0].value, Value::String(&[0, 0xd800]));
                    }
                    _ => assert!(a.annotation.elements.is_empty()),
                }
            }
            assert_eq!(&original, bytes);
            assert_eq!(c.attributes[0].bytes, original);
        }
    }
}

#[test]
fn frozen_literal_errors_have_exact_precedence_and_preserve_input() {
    let cases = [
        (fixture!("missing_parameter"), Error::Truncated),
        (fixture!("missing_supertype"), Error::Truncated),
        (fixture!("missing_bound"), Error::Truncated),
        (fixture!("missing_path_length"), Error::Truncated),
        (
            fixture!("invalid_kind"),
            Error::Malformed("type annotation path"),
        ),
        (
            fixture!("invalid_argument"),
            Error::Malformed("type annotation path"),
        ),
        (fixture!("invalid_kind_missing_index"), Error::Truncated),
        (
            fixture!("illegal_field"),
            Error::Malformed("class type annotation target"),
        ),
        (
            fixture!("illegal_method"),
            Error::Malformed("class type annotation target"),
        ),
        (
            fixture!("illegal_code"),
            Error::Malformed("class type annotation target"),
        ),
        (
            fixture!("legal_then_illegal"),
            Error::Malformed("class type annotation target"),
        ),
        (
            fixture!("illegal_then_legal"),
            Error::Malformed("class type annotation target"),
        ),
        (
            fixture!("zero_descriptor"),
            Error::Malformed("annotation constant index"),
        ),
        (
            fixture!("wrong_cp"),
            Error::Malformed("annotation constant type"),
        ),
        (fixture!("count_eof"), Error::Truncated),
        (
            fixture!("trailing"),
            Error::Malformed("type annotation trailing bytes"),
        ),
    ];
    for (bytes, expected) in cases {
        for name in [1, 2] {
            let mut c = base();
            c.attributes = vec![attr(name, &bytes)];
            let original = bytes.clone();
            assert!(annotations::decode(&c).is_ok());
            assert_eq!(decode(&c).unwrap_err(), expected);
            assert_eq!(c.attributes[0].bytes, original);
        }
    }
}

#[test]
fn every_tag_and_raw_index_and_target_order_is_class_scoped() {
    for tag in 0..=255 {
        for tail in [&[][..], &[255, 255, 0, 0, 3, 0, 0][..]] {
            let bytes = [vec![0, 1, tag], tail.to_vec()].concat();
            let mut c = base();
            c.attributes = vec![attr(1, &bytes)];
            if ![0, 0x10, 0x11].contains(&tag) {
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Malformed("class type annotation target")
                );
            } else if tail.is_empty() || tag == 0 {
                assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
            } else {
                assert!(decode(&c).is_ok());
            }
        }
    }
    for index in [0, 1, 255, 256, 65534, 65535] {
        let bytes = payload(&[root(&[0x10, (index >> 8) as u8, index as u8], &[], None)]);
        let mut c = base();
        c.attributes = vec![attr(1, &bytes)];
        assert_eq!(
            decode(&c).unwrap().class[0].annotations[0].target,
            Target::Supertype { index }
        );
    }
    for parameter_index in [0, 1, 255] {
        for bound_index in [0, 1, 255] {
            let bytes = payload(&[root(&[0x11, parameter_index, bound_index], &[], None)]);
            let mut c = base();
            c.attributes = vec![attr(2, &bytes)];
            assert_eq!(
                decode(&c).unwrap().class[0].annotations[0].target,
                Target::TypeParameterBound {
                    parameter_index,
                    bound_index
                }
            );
        }
    }
    let infos = [vec![0, 255], vec![0x10, 255, 255], vec![0x11, 255, 255]];
    let targets = [
        Target::TypeParameter { index: 255 },
        Target::Supertype { index: 65535 },
        Target::TypeParameterBound {
            parameter_index: 255,
            bound_index: 255,
        },
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
        [0, 0, 0],
        [1, 1, 1],
        [2, 2, 2],
    ] {
        let bytes = payload(&order.map(|i| root(&infos[i], &[], None)));
        let mut c = base();
        c.attributes = vec![attr(2, &bytes), attr(1, &bytes)];
        let out = decode(&c).unwrap();
        assert_eq!(out.class.len(), 2);
        assert_eq!(out.class[0].visibility, Visibility::RuntimeInvisible);
        assert_eq!(out.class[1].visibility, Visibility::RuntimeVisible);
        for a in &out.class {
            for (root, i) in a.annotations.iter().zip(order) {
                assert_eq!(root.target, targets[i]);
            }
        }
        c.attributes.reverse();
        assert_eq!(
            decode(&c).unwrap().class[0].visibility,
            Visibility::RuntimeVisible
        );
    }
}

#[test]
fn path_widths_every_prefix_and_trailing_data() {
    let nested = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), vec![b'[', 0, 0]]))),
    ]
    .concat();
    for info in [&[0, 255][..], &[0x10, 255, 255][..], &[0x11, 255, 255][..]] {
        for bytes in [
            payload(&[root(info, &[], None)]),
            payload(&[root(info, &[(3, 255); 255], None)]),
            payload(&[root(info, &[(0, 0)], Some(&nested))]),
            fixture!("mixed"),
        ] {
            for name in [1, 2] {
                for end in 0..bytes.len() {
                    let mut c = base();
                    c.attributes = vec![attr(name, &bytes[..end])];
                    assert_eq!(decode(&c).unwrap_err(), Error::Truncated, "{info:?}/{end}");
                }
                let mut c = base();
                c.attributes = vec![attr(name, &bytes)];
                let out = decode(&c).unwrap();
                if out.class[0].annotations[0].path.len() == 255 {
                    assert!(
                        out.class[0].annotations[0]
                            .path
                            .iter()
                            .all(|p| p.kind == 3 && p.argument_index == 255)
                    );
                }
                let mut trailing = bytes.clone();
                trailing.push(0);
                c.attributes = vec![attr(name, &trailing)];
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::Malformed("type annotation trailing bytes")
                );
            }
        }
        for kind in 0..=255 {
            for argument in [0, 1, 255] {
                let bytes = payload(&[root(info, &[(kind, argument)], None)]);
                let mut c = base();
                c.attributes = vec![attr(1, &bytes)];
                if kind <= 3 && (kind == 3 || argument == 0) {
                    assert!(decode(&c).is_ok());
                } else {
                    assert_eq!(
                        decode(&c).unwrap_err(),
                        Error::Malformed("type annotation path")
                    );
                }
            }
        }
    }
}

#[test]
fn versions_duplicates_opaque_contexts_and_declaration_precedence() {
    let bytes = fixture!("parameter0");
    for major in 45..=70 {
        for minor in [0, 1, 3, 65535] {
            let mut c = base();
            c.major = major;
            c.minor = minor;
            let supported = major == 45 && minor <= 3 || (46..=69).contains(&major) && minor == 0;
            if !supported {
                assert_eq!(
                    decode(&c).unwrap_err(),
                    Error::UnsupportedVersion { major, minor }
                );
                continue;
            }
            assert!(decode(&c).unwrap().class.is_empty());
            for raw in [&bytes[..], &[0, 0][..], &[][..]] {
                c.attributes = vec![attr(1, raw)];
                if major < 52 {
                    assert_eq!(
                        decode(&c).unwrap_err(),
                        Error::Malformed("type annotation version")
                    );
                } else if raw.is_empty() {
                    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
                } else {
                    assert!(decode(&c).is_ok());
                }
            }
        }
    }
    for name in [1, 2] {
        let mut c = base();
        c.constants.push(text(if name == 1 {
            "RuntimeVisibleTypeAnnotations"
        } else {
            "RuntimeInvisibleTypeAnnotations"
        }));
        c.attributes = vec![attr(name, &bytes), attr(21, &[])];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("duplicate type annotation attribute")
        );
        c.attributes.swap(0, 1);
        assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
        c.attributes = vec![attr(name, &[0, 0]), attr(21, &[0, 0])];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("duplicate type annotation attribute")
        );
        c.major = 51;
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("type annotation version")
        );
    }
    let mut c = base();
    c.major = 51;
    c.fields = vec![member(vec![attr(1, &[255]), attr(1, &[])])];
    c.methods = vec![member(vec![attr(2, &[255]), attr(2, &[])])];
    c.attributes = vec![attr(19, &[255]), attr(20, &[255])];
    assert!(decode(&c).is_ok());
    c.major = 52;
    c.attributes = vec![attr(1, &[0, 1, 0x13]), attr(17, &[])];
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
    c.attributes.pop();
    c.fields[0].attributes.push(attr(17, &[]));
    assert_eq!(decode(&c).unwrap_err(), Error::Truncated);
    c.fields[0].attributes.pop();
    c.methods[0].attributes.push(attr(0, &[]));
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant index")
    );
    c.methods[0].attributes.pop();
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("class type annotation target")
    );
    c.constants.clear();
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation constant pool size")
    );
    c.major = 70;
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::UnsupportedVersion {
            major: 70,
            minor: 0
        }
    );
}

#[test]
fn consumed_cp_elements_and_unconsumed_class_references() {
    let good = payload(&[root(&[0, 0], &[], Some(&scalar(b'I', 5)))]);
    for offset in [5, 9, 12] {
        for index in [0, 10, 12, 21, u16::MAX] {
            let mut bytes = good.clone();
            bytes[offset..offset + 2].copy_from_slice(&index.to_be_bytes());
            let mut c = base();
            c.attributes = vec![attr(1, &bytes)];
            assert!(decode(&c).is_err());
        }
    }
    for name in [0, 5, 10, 21, u16::MAX] {
        let mut c = base();
        c.attributes = vec![attr(name, &good)];
        assert!(decode(&c).is_err());
    }
    for descriptor in ["I", "[LA;", "L;", "LA", "LA;;"] {
        let mut c = base();
        c.constants[3] = text(descriptor);
        c.attributes = vec![attr(1, &good)];
        assert!(decode(&c).is_err());
    }
    for name in ["", "a/b", "<init>"] {
        let mut c = base();
        c.constants[4] = text(name);
        c.attributes = vec![attr(1, &good)];
        assert!(decode(&c).is_err());
    }
    let duplicate = [
        0, 1, 0, 0, 0, 0, 3, 0, 2, 0, 4, b'I', 0, 5, 0, 21, b'I', 0, 5,
    ];
    let mut c = base();
    c.constants.push(text("x"));
    c.attributes = vec![attr(1, &duplicate)];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("duplicate annotation element")
    );
    c.attributes = vec![attr(1, &good)];
    c.this_class = u16::MAX;
    c.super_class = u16::MAX;
    c.interfaces = vec![u16::MAX];
    c.fields = vec![member(vec![])];
    c.methods = vec![member(vec![])];
    c.constants[0] = text("forged slot zero");
    assert!(decode(&c).is_ok());
    c.constants[3] = Constant::String(7);
    assert!(decode(&c).is_err());
}

#[test]
fn values_numeric_bits_utf16_nesting_depth_and_invalid_tags() {
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
    for info in [&[0, 0][..], &[0x10, 255, 255][..], &[0x11, 0, 0][..]] {
        for (value, expected) in &cases {
            let bytes = payload(&[root(info, &[], Some(value))]);
            let mut c = base();
            c.attributes = vec![attr(1, &bytes)];
            assert_eq!(
                &decode(&c).unwrap().class[0].annotations[0]
                    .annotation
                    .elements[0]
                    .value,
                expected
            );
        }
        for tag in 0..=255 {
            if b"BCDFIJSZsec@[".contains(&tag) {
                continue;
            }
            let bytes = payload(&[root(info, &[], Some(&[tag]))]);
            let mut c = base();
            c.attributes = vec![attr(2, &bytes)];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("annotation value tag")
            );
        }
    }
    let nested = [
        vec![b'@'],
        annotation(Some(&array(&[scalar(b's', 7), vec![b'[', 0, 0]]))),
    ]
    .concat();
    let bytes = payload(&[root(&[0x11, 0, 0], &[], Some(&nested))]);
    let mut c = base();
    c.attributes = vec![attr(1, &bytes)];
    let out = decode(&c).unwrap();
    let Value::Annotation(nested) = &out.class[0].annotations[0].annotation.elements[0].value
    else {
        panic!()
    };
    assert_eq!(
        nested.elements[0].value,
        Value::Array(vec![Value::String(&[0, 0xd800]), Value::Array(vec![])])
    );
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
            let bytes = payload(&[root(&[0, 0], &[], Some(&value))]);
            let mut c = base();
            c.attributes = vec![attr(1, &bytes)];
            if depth == MAX_DEPTH {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation depth"));
            }
        }
    }
}

#[test]
fn aggregate_nodes_include_declarations_and_new_retained_capacity() {
    for count in [MAX_RECORDS - 3, MAX_RECORDS - 2] {
        let bytes = payload(&vec![root(&[0, 0], &[], None); count]);
        let mut c = base();
        c.attributes = vec![attr(1, &bytes)];
        match decode(&c) {
            Ok(out) => {
                assert_eq!(count, MAX_RECORDS - 3);
                assert!(
                    1 + out.class.capacity() + out.class[0].annotations.capacity() <= MAX_RECORDS
                );
                assert_eq!(out.class[0].annotations.len(), count);
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    let mut roots = vec![root(&[0, 0], &[], None); MAX_RECORDS - 3];
    roots[0] = root(&[0, 0], &[(3, 255)], None);
    let bytes = payload(&roots);
    let mut c = base();
    c.attributes = vec![attr(1, &bytes)];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    for extra in [0, 1] {
        let count = MAX_RECORDS - 8 + extra;
        let values = array(&vec![scalar(b'I', 5); count]);
        let declaration = [vec![0, 1], annotation(Some(&values))].concat();
        let types = fixture!("parameter0");
        let mut c = base();
        c.attributes = vec![attr(17, &declaration), attr(1, &types)];
        assert!(annotations::decode(&c).is_ok());
        match decode(&c) {
            Ok(out) => {
                assert_eq!(extra, 0);
                assert!(
                    5 + count + out.class.capacity() + out.class[0].annotations.capacity()
                        <= MAX_RECORDS
                );
            }
            Err(error) => assert_eq!(error, Error::Limit("annotation nodes")),
        }
    }
    let mut c = base();
    c.attributes = vec![attr(1, &[255, 255])];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    c.attributes.clear();
    c.fields = (0..=MAX_RECORDS).map(|_| member(vec![])).collect();
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
}

#[test]
fn aggregate_raw_bytes_and_work_are_shared_once() {
    for types in [
        fixture!("parameter0"),
        fixture!("superclass"),
        fixture!("bound0"),
    ] {
        for delta in [-1, 0, 1] {
            let ignored = vec![0; ((MAX_BYTES - types.len()) as isize + delta) as usize];
            let mut c = base();
            c.attributes = vec![attr(18, &ignored), attr(1, &types)];
            if delta <= 0 {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
            }
        }
    }
    for info in [&[0, 0][..], &[0x10, 255, 255][..], &[0x11, 0, 0][..]] {
        let types = payload(&[root(info, &[], Some(&scalar(b's', 7)))]);
        // 511 ignored names read twice; tune another name and the consumed string.
        let baseline = if info[0] == 0 { 70 } else { 71 };
        let remainder = MAX_VALIDATION_UNITS - 511 * 2 * 65536 - baseline;
        let filler = (remainder - 2) / 2;
        for delta in [-1, 0, 1] {
            let string_len = (remainder - 2 * filler) as isize + delta;
            let mut c = base();
            c.constants[18] = Constant::Utf8(vec![65; 65535]);
            c.constants.push(Constant::Utf8(vec![66; filler - 1]));
            c.constants[7] = Constant::Utf8(vec![67; string_len as usize]);
            c.attributes = (0..511).map(|_| attr(18, &[])).collect();
            c.attributes.extend([attr(21, &[]), attr(1, &types)]);
            if delta <= 0 {
                assert!(decode(&c).is_ok());
            } else {
                assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
            }
        }
    }
}

#[test]
fn finite_every_byte_mutations_return_controlled_results() {
    let nested = [vec![b'@'], annotation(Some(&array(&[scalar(b's', 7)])))].concat();
    for bytes in [
        fixture!("parameter0"),
        fixture!("superclass"),
        fixture!("bound0"),
        payload(&[root(&[0x11, 255, 255], &[(3, 255)], Some(&nested))]),
    ] {
        for at in 0..bytes.len() {
            for replacement in 0..=255 {
                let mut changed = bytes.clone();
                changed[at] = replacement;
                let mut c = base();
                c.attributes = vec![attr(1, &changed)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn existing_licensed_class_bound_fixture_matches_frozen_expectations() {
    let bytes = include_bytes!("fixtures/method_type_annotations/AbstractIterator.class.bin");
    assert_eq!(bytes.len(), 2361);
    assert_eq!(&bytes[2329..2335], [0, 63, 0, 0, 0, 10]);
    assert_eq!(&bytes[2335..2345], [0, 1, 0x11, 0, 0, 0, 0, 64, 0, 0]);
    let c = parse_class(bytes).unwrap();
    assert_eq!((c.major, c.minor), (52, 0));
    let out = decode(&c).unwrap();
    assert_eq!(out.declarations, annotations::decode(&c).unwrap());
    assert_eq!(out.class.len(), 1);
    assert_eq!(out.class[0].visibility, Visibility::RuntimeVisible);
    assert_eq!(out.class[0].annotations.len(), 1);
    let a = &out.class[0].annotations[0];
    assert_eq!(
        a.target,
        Target::TypeParameterBound {
            parameter_index: 0,
            bound_index: 0
        }
    );
    assert!(a.path.is_empty());
    assert!(a.annotation.elements.is_empty());
    assert_eq!(
        a.annotation.descriptor,
        "Lorg/jspecify/annotations/Nullable;"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    assert!(std::ptr::eq(a.annotation.descriptor, c.utf16(64).unwrap()));
}
