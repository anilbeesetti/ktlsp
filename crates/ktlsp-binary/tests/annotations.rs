use ktlsp_binary::annotations::{MAX_DEPTH, Value, Visibility, decode};
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, Member, parse_class,
};

// Independently authored bytes and expected values; no compiler/JVM generates them.
fn u2(out: &mut Vec<u8>, n: u16) {
    out.extend(n.to_be_bytes());
}
fn base() -> ClassFile<'static> {
    ClassFile {
        major: 67,
        minor: 0,
        constants: vec![Constant::Unusable],
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes: vec![],
    }
}
fn add(c: &mut ClassFile<'_>, value: Constant) -> u16 {
    let i = c.constants.len() as u16;
    c.constants.push(value);
    i
}
fn text(c: &mut ClassFile<'_>, s: &str) -> u16 {
    add(c, Constant::Utf8(s.encode_utf16().collect()))
}
fn payload(ty: u16, pairs: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut b = vec![0, 1];
    u2(&mut b, ty);
    u2(&mut b, pairs.len() as u16);
    for (name, value) in pairs {
        u2(&mut b, *name);
        b.extend(value);
    }
    b
}
fn scalar(tag: u8, i: u16) -> Vec<u8> {
    let mut b = vec![tag];
    u2(&mut b, i);
    b
}
fn member(attrs: Vec<Attribute<'_>>) -> Member<'_> {
    Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes: attrs,
    }
}
fn setup() -> (ClassFile<'static>, u16, u16, u16) {
    let mut c = base();
    let visible = text(&mut c, "RuntimeVisibleAnnotations");
    let ty = text(&mut c, "Lexample/Annotation;");
    let name = text(&mut c, "value");
    (c, visible, ty, name)
}

#[test]
fn every_tag_exact_bits_utf16_order_and_contexts() {
    for major in [67, 68, 69] {
    let (mut c, visible, ty, _) = setup();
    c.major = major;
    let invisible = text(&mut c, "RuntimeInvisibleAnnotations");
    let int = add(&mut c, Constant::Integer(0xffff_ffff));
    let float = add(&mut c, Constant::Float(0x7fc0_1234));
    let long = add(&mut c, Constant::Long(0xffff_ffff_ffff_ffff));
    add(&mut c, Constant::Unusable);
    let double = add(&mut c, Constant::Double(0xfff8_0000_0000_1234));
    add(&mut c, Constant::Unusable);
    let string = add(&mut c, Constant::Utf8(vec![0, 0xd800, 0x61, 0xdc00]));
    let enum_ty = text(&mut c, "Lexample/Enum;");
    let enum_name = text(&mut c, "FIRST");
    let void = text(&mut c, "V");
    let mut enum_value = scalar(b'e', enum_ty);
    u2(&mut enum_value, enum_name);
    let mut nested = vec![b'@'];
    u2(&mut nested, ty);
    u2(&mut nested, 0);
    let array = vec![b'[', 0, 2, b'I', (int >> 8) as u8, int as u8, b'[', 0, 0];
    let values = vec![
        scalar(b'B', int),
        scalar(b'C', int),
        scalar(b'D', double),
        scalar(b'F', float),
        scalar(b'I', int),
        scalar(b'J', long),
        scalar(b'S', int),
        scalar(b'Z', int),
        scalar(b's', string),
        enum_value,
        scalar(b'c', void),
        nested,
        array,
    ];
    let pairs: Vec<_> = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (text(&mut c, &format!("v{i}")), v))
        .collect();
    let bytes = payload(ty, &pairs);
    let empty = [0, 0];
    c.attributes = vec![
        Attribute {
            name: visible,
            bytes: &bytes,
        },
        Attribute {
            name: invisible,
            bytes: &empty,
        },
    ];
    c.fields = vec![
        member(vec![Attribute {
            name: invisible,
            bytes: &bytes,
        }]),
        member(vec![]),
    ];
    c.methods = vec![member(vec![Attribute {
        name: visible,
        bytes: &bytes,
    }])];
    let decoded = decode(&c).unwrap();
    assert_eq!(decoded.class.len(), 2);
    assert_eq!(decoded.class[0].visibility, Visibility::RuntimeVisible);
    assert_eq!(decoded.class[1].visibility, Visibility::RuntimeInvisible);
    assert!(decoded.class[1].annotations.is_empty());
    assert_eq!(decoded.fields.len(), 2);
    assert!(decoded.fields[1].is_empty());
    assert_eq!(
        decoded.fields[0][0].visibility,
        Visibility::RuntimeInvisible
    );
    assert_eq!(decoded.methods[0][0].visibility, Visibility::RuntimeVisible);
    let a = &decoded.class[0].annotations[0];
    assert_eq!(
        a.descriptor,
        "Lexample/Annotation;".encode_utf16().collect::<Vec<_>>()
    );
    let expected = [
        Value::Byte(u32::MAX),
        Value::Char(u32::MAX),
        Value::Double(0xfff8_0000_0000_1234),
        Value::Float(0x7fc0_1234),
        Value::Int(u32::MAX),
        Value::Long(u64::MAX),
        Value::Short(u32::MAX),
        Value::Boolean(u32::MAX),
        Value::String(&[0, 0xd800, 0x61, 0xdc00]),
    ];
    for (element, expected) in a.elements.iter().zip(expected) {
        assert_eq!(element.value, expected);
    }
    assert!(
        matches!(&a.elements[9].value, Value::Enum { descriptor, name } if *descriptor == c.utf16(enum_ty).unwrap() && *name == c.utf16(enum_name).unwrap())
    );
    assert_eq!(a.elements[10].value, Value::Class(&[86]));
    assert!(matches!(&a.elements[11].value, Value::Annotation(a) if a.elements.is_empty()));
    assert_eq!(
        a.elements[12].value,
        Value::Array(vec![Value::Int(u32::MAX), Value::Array(vec![])])
    );
    assert_eq!(c.attributes[0].bytes, bytes);
    }
}

#[test]
fn typed_constant_references_and_forged_zero_slot() {
    let tags = b"BCDFIJSZs";
    for tag in tags {
        for candidate in [
            Constant::Unusable,
            Constant::Integer(1),
            Constant::Float(1),
            Constant::Long(1),
            Constant::Double(1),
            Constant::Utf8(vec![65]),
            Constant::String(2),
        ] {
            let (mut c, attr, ty, name) = setup();
            let valid = matches!(
                (*tag, &candidate),
                (b'B' | b'C' | b'I' | b'S' | b'Z', Constant::Integer(_))
                    | (b'F', Constant::Float(_))
                    | (b'J', Constant::Long(_))
                    | (b'D', Constant::Double(_))
                    | (b's', Constant::Utf8(_))
            );
            let i = add(&mut c, candidate);
            let bytes = payload(ty, &[(name, scalar(*tag, i))]);
            c.attributes = vec![Attribute {
                name: attr,
                bytes: &bytes,
            }];
            assert_eq!(decode(&c).is_ok(), valid, "tag {tag}, index {i}");
        }
        for i in [0, u16::MAX] {
            let (mut c, attr, ty, name) = setup();
            c.constants[0] = Constant::Integer(0);
            let bytes = payload(ty, &[(name, scalar(*tag, i))]);
            c.attributes = vec![Attribute {
                name: attr,
                bytes: &bytes,
            }];
            assert!(decode(&c).is_err());
        }
    }
    // Every text-bearing position independently rejects index zero, even usable.
    for position in 0..6 {
        let (mut c, attr, ty, name) = setup();
        c.constants[0] = Constant::Utf8(vec![65]);
        let value = match position {
            3 => scalar(b's', 0),
            4 => vec![b'e', 0, 0, 0, name as u8],
            5 => scalar(b'c', 0),
            _ => scalar(b's', name),
        };
        let bytes = payload(
            if position == 1 { 0 } else { ty },
            &[(if position == 2 { 0 } else { name }, value)],
        );
        c.attributes = vec![Attribute {
            name: if position == 0 { 0 } else { attr },
            bytes: &bytes,
        }];
        assert!(decode(&c).is_err(), "position {position}");
    }
}

#[test]
fn descriptors_names_duplicates_versions_and_trailing_bytes() {
    for descriptor in [
        "", "V", "I", "[I", "L;", "La//b;", "La.b;", "Labc", "()V", "La;;",
    ] {
        for tag in *b"@e" {
            let (mut c, attr, _, name) = setup();
            let ty = text(&mut c, descriptor);
            let bytes = if tag == b'@' {
                payload(ty, &[])
            } else {
                payload(
                    text(&mut c, "LA;"),
                    &[(
                        name,
                        vec![
                            b'e',
                            (ty >> 8) as u8,
                            ty as u8,
                            (name >> 8) as u8,
                            name as u8,
                        ],
                    )],
                )
            };
            c.attributes = vec![Attribute {
                name: attr,
                bytes: &bytes,
            }];
            assert!(decode(&c).is_err(), "descriptor {descriptor}, tag {tag}");
        }
    }
    for (descriptor, ok) in [
        ("V", true),
        ("I", true),
        ("[I", true),
        ("Ljava/lang/Void;", true),
        ("[[LA;", true),
        ("[V", false),
        ("()V", false),
        ("VI", false),
    ] {
        let (mut c, attr, ty, name) = setup();
        let d = text(&mut c, descriptor);
        let bytes = payload(ty, &[(name, scalar(b'c', d))]);
        c.attributes = vec![Attribute {
            name: attr,
            bytes: &bytes,
        }];
        assert_eq!(decode(&c).is_ok(), ok, "class literal {descriptor}");
    }
    for n in ["", "a/b", "a.b", "a;", "a[", "<init>", "<clinit>", "a>b"] {
        let (mut c, attr, ty, _) = setup();
        let name = text(&mut c, n);
        let bytes = payload(ty, &[(name, scalar(b's', name))]);
        c.attributes = vec![Attribute {
            name: attr,
            bytes: &bytes,
        }];
        assert!(decode(&c).is_err(), "name {n}");
    }
    let (mut c, attr, ty, name) = setup();
    let duplicate = text(&mut c, "value");
    let bytes = payload(
        ty,
        &[(name, scalar(b's', name)), (duplicate, scalar(b's', name))],
    );
    c.attributes = vec![Attribute {
        name: attr,
        bytes: &bytes,
    }];
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("duplicate annotation element")
    );
    let empty = [0, 0];
    for context in 0..3 {
        let (mut c, attr, _, _) = setup();
        let duplicate_attr = text(&mut c, "RuntimeVisibleAnnotations");
        let attrs = vec![
            Attribute {
                name: attr,
                bytes: &empty,
            },
            Attribute {
                name: duplicate_attr,
                bytes: &empty,
            },
        ];
        match context {
            0 => c.attributes = attrs,
            1 => c.fields = vec![member(attrs)],
            _ => c.methods = vec![member(attrs)],
        }
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("duplicate annotation attribute")
        );
    }
    let (mut c, attr, ty, _) = setup();
    let bytes = payload(ty, &[]);
    c.attributes = vec![Attribute {
        name: attr,
        bytes: &bytes,
    }];
    for (major, minor, ok) in [
        (48, 0, false),
        (49, 0, true),
        (67, 0, true),
        (68, 0, true),
        (69, 0, true),
        (70, 0, false),
        (67, 65535, false),
    ] {
        c.major = major;
        c.minor = minor;
        assert_eq!(decode(&c).is_ok(), ok);
    }
    c.major = 67;
    c.minor = 0;
    let mut trailing = bytes.clone();
    trailing.push(0);
    c.attributes[0].bytes = &trailing;
    assert_eq!(
        decode(&c).unwrap_err(),
        Error::Malformed("annotation trailing bytes")
    );
}

#[test]
fn payload_every_byte_truncation_and_bad_tags() {
    let (mut c, attr, ty, name) = setup();
    let mut value = vec![b'[', 0, 2, b'@'];
    u2(&mut value, ty);
    u2(&mut value, 1);
    u2(&mut value, name);
    value.extend(scalar(b's', name));
    value.extend(scalar(b'c', ty));
    let bytes = payload(ty, &[(name, value)]);
    for end in 0..bytes.len() {
        c.attributes = vec![Attribute {
            name: attr,
            bytes: &bytes[..end],
        }];
        assert!(decode(&c).is_err(), "prefix {end}");
    }
    for tag in 0..=255 {
        if b"BCDFIJSZsec@[".contains(&tag) {
            continue;
        }
        let (mut c, attr, ty, name) = setup();
        let bytes = payload(ty, &[(name, vec![tag])]);
        c.attributes = vec![Attribute {
            name: attr,
            bytes: &bytes,
        }];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("annotation value tag")
        );
    }
}

#[test]
fn aggregate_nodes_bytes_work_and_depth() {
    let (mut c, attr, ty, name) = setup();
    for n in [MAX_DEPTH - 2, MAX_DEPTH - 1] {
        let (mut c, attr, ty, name) = setup();
        let mut v = Vec::new();
        for _ in 0..n {
            v.extend([b'[', 0, 1]);
        }
        v.extend(scalar(b's', name));
        let bytes = payload(ty, &[(name, v)]);
        c.attributes = vec![Attribute {
            name: attr,
            bytes: &bytes,
        }];
        assert_eq!(decode(&c).is_ok(), n == MAX_DEPTH - 2);
    }
    let large = vec![0; MAX_BYTES / 2 + 1];
    let unknown = text(&mut c, "Unknown");
    c.attributes = vec![Attribute {
        name: unknown,
        bytes: &large,
    }];
    c.fields = vec![member(vec![Attribute {
        name: unknown,
        bytes: &large,
    }])];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation bytes"));
    c.attributes.clear();
    c.fields = (0..=MAX_RECORDS).map(|_| member(vec![])).collect();
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    c.fields.clear();
    let mut values = vec![b'['];
    u2(&mut values, 33000);
    for _ in 0..33000 {
        values.extend(scalar(b's', name));
    }
    let bytes = payload(ty, &[(name, values)]);
    let invisible = text(&mut c, "RuntimeInvisibleAnnotations");
    c.attributes = vec![
        Attribute {
            name: attr,
            bytes: &bytes,
        },
        Attribute {
            name: invisible,
            bytes: &bytes,
        },
    ];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    c.attributes.pop();
    c.methods = vec![member(vec![Attribute {
        name: attr,
        bytes: &bytes,
    }])];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
    c.methods.clear();
    let long = add(&mut c, Constant::Utf8(vec![65; 65535]));
    let mut values = vec![b'['];
    u2(&mut values, 600);
    for _ in 0..600 {
        values.extend(scalar(b's', long));
    }
    let bytes = payload(ty, &[(name, values)]);
    c.attributes = vec![
        Attribute {
            name: attr,
            bytes: &bytes,
        },
        Attribute {
            name: invisible,
            bytes: &bytes,
        },
    ];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
}

#[test]
fn deterministic_mutations_of_nested_payload_never_panic() {
    let (_, _, ty, name) = setup();
    let mut v = vec![b'[', 0, 2, b'@'];
    u2(&mut v, ty);
    u2(&mut v, 0);
    v.extend(scalar(b's', name));
    let bytes = payload(ty, &[(name, v)]);
    for i in 0..bytes.len() {
        for value in 0..=255 {
            let (mut c, attr, _, _) = setup();
            let mut mutated = bytes.clone();
            mutated[i] = value;
            c.attributes = vec![Attribute {
                name: attr,
                bytes: &mutated,
            }];
            let _ = decode(&c);
        }
    }
}

#[test]
fn precompiled_kotlin_data_only_annotations() {
    for bytes in [
        include_bytes!("../fixtures/t017c2/Unit.class").as_slice(),
        include_bytes!("../fixtures/t017c2/Metadata.class").as_slice(),
    ] {
        let c = parse_class(bytes).unwrap();
        let annotations = decode(&c).unwrap();
        let metadata = annotations
            .class
            .iter()
            .flat_map(|a| &a.annotations)
            .find(|a| a.descriptor == "Lkotlin/Metadata;".encode_utf16().collect::<Vec<_>>())
            .unwrap();
        let d1 = metadata
            .elements
            .iter()
            .find(|e| e.name == [100, 49])
            .unwrap();
        assert!(
            matches!(&d1.value, Value::Array(v) if matches!(&v[0], Value::String(s) if s.first() == Some(&0)))
        );
        let mv = metadata
            .elements
            .iter()
            .find(|e| e.name == [109, 118])
            .unwrap();
        assert_eq!(
            mv.value,
            Value::Array(vec![Value::Int(2), Value::Int(3), Value::Int(0)])
        );
        assert_eq!(annotations.fields.len(), c.fields.len());
        assert_eq!(annotations.methods.len(), c.methods.len());
        for end in 0..bytes.len() {
            assert!(parse_class(&bytes[..end]).is_err());
        }
    }
}

#[test]
fn nested_annotation_and_alternating_depth_boundaries() {
    for alternating in [false, true] {
        for wrappers in 0..=MAX_DEPTH {
            let (mut c, attr, ty, name) = setup();
            let mut v = scalar(b's', name);
            let mut depth = 2; // Root annotation and its element_value.
            for i in 0..wrappers {
                let mut outer = if alternating && i % 2 == 0 {
                    depth += 1;
                    vec![b'[', 0, 1]
                } else {
                    depth += 2; // '@' element_value plus the nested annotation.
                    let mut outer = scalar(b'@', ty);
                    u2(&mut outer, 1);
                    u2(&mut outer, name);
                    outer
                };
                outer.extend(v);
                v = outer;
            }
            let bytes = payload(ty, &[(name, v)]);
            c.attributes = vec![Attribute {
                name: attr,
                bytes: &bytes,
            }];
            assert_eq!(decode(&c).is_ok(), depth <= MAX_DEPTH, "depth {depth}");
        }
    }
}

#[test]
fn duplicate_comparisons_exhaust_shared_work_budget() {
    let (mut c, attr, ty, _) = setup();
    let int = add(&mut c, Constant::Integer(1));
    let pairs: Vec<_> = (0..400)
        .map(|i| {
            let name = text(&mut c, &format!("{}{i}", "a".repeat(1024)));
            (name, scalar(b'I', int))
        })
        .collect();
    let bytes = payload(ty, &pairs);
    c.attributes = vec![Attribute {
        name: attr,
        bytes: &bytes,
    }];
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
}
