use ktlsp_binary::annotation_extensions::decode;
use ktlsp_binary::annotations::{self, MAX_DEPTH, Value, Visibility};
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, Member, parse_class,
};

// Independently authored data; no compiler or JVM creates these fixtures.
fn base() -> ClassFile<'static> {
    ClassFile {
        major: 67,
        minor: 0,
        access_flags: 0x2200,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        attributes: vec![],
        methods: vec![Member {
            access_flags: 0x401,
            name: 8,
            descriptor: 9,
            attributes: vec![],
        }],
        constants: vec![
            Constant::Unusable,
            text("RuntimeVisibleParameterAnnotations"),
            text("RuntimeInvisibleParameterAnnotations"),
            text("AnnotationDefault"),
            text("RuntimeVisibleAnnotations"),
            text("RuntimeInvisibleAnnotations"),
            text("LA;"),
            text("element"),
            text("value"),
            text("()I"),
            Constant::Integer(u32::MAX),
            Constant::Float(0x7fc0_1234),
            Constant::Long(u64::MAX),
            Constant::Unusable,
            Constant::Double(0xfff8_0000_0000_1234),
            Constant::Unusable,
            Constant::Utf8(vec![0, 0xd800, 97, 0xdc00]),
            text("LE;"),
            text("ONE"),
            text("V"),
        ],
    }
}
fn text(s: &str) -> Constant {
    Constant::Utf8(s.encode_utf16().collect())
}
fn scalar(tag: u8, index: u16) -> Vec<u8> {
    vec![tag, (index >> 8) as u8, index as u8]
}
fn annotation(value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 6, 0, 1, 0, 7];
    bytes.extend(value);
    bytes
}
fn parameters(entries: &[Vec<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = vec![entries.len() as u8];
    for entry in entries {
        bytes.extend((entry.len() as u16).to_be_bytes());
        for annotation in entry {
            bytes.extend(annotation);
        }
    }
    bytes
}
fn array(values: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![b'['];
    bytes.extend((values.len() as u16).to_be_bytes());
    for value in values {
        bytes.extend(value);
    }
    bytes
}
fn attr(name: u16, bytes: &[u8]) -> Attribute<'_> {
    Attribute { name, bytes }
}
fn method_descriptor(c: &mut ClassFile<'_>, descriptor: &str) {
    c.constants[9] = text(descriptor);
}

#[test]
fn encoded_positions_visibility_order_empty_entries_and_methods() {
    let mut c = base();
    let a = annotation(&scalar(b's', 16));
    let visible = parameters(&[vec![a.clone(), a.clone()], vec![], vec![a.clone()]]);
    let invisible = parameters(&[vec![], vec![a]]);
    for descriptor in ["()V", "(I)V", "(Ljava/lang/Object;JD)V", "(IIIII)V"] {
        method_descriptor(&mut c, descriptor);
        c.methods[0].name = 8;
        c.methods[0].attributes = vec![attr(2, &invisible), attr(1, &visible)];
        let out = decode(&c).unwrap();
        assert_eq!(
            out.methods[0].parameters[0].visibility,
            Visibility::RuntimeInvisible
        );
        assert_eq!(
            out.methods[0].parameters[1].visibility,
            Visibility::RuntimeVisible
        );
        assert_eq!(out.methods[0].parameters[0].parameters.len(), 2);
        assert_eq!(out.methods[0].parameters[1].parameters.len(), 3);
        assert!(out.methods[0].parameters[1].parameters[1].is_empty());
        assert_eq!(out.methods[0].parameters[1].parameters[0].len(), 2);
        assert_eq!(
            out.methods[0].parameters[1].parameters[0][0].elements[0].value,
            Value::String(&[0, 0xd800, 97, 0xdc00])
        );
        assert_eq!(out.declarations, annotations::decode(&c).unwrap());
        assert_eq!(c.methods[0].attributes[0].bytes, invisible);
    }
    c.constants[8] = text("<init>");
    method_descriptor(&mut c, "(Ljava/lang/Object;I)V");
    assert_eq!(
        decode(&c).unwrap().methods[0].parameters[1].parameters[0].len(),
        2
    );
    c.methods[0].attributes.clear();
    c.methods.push(Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes: vec![],
    });
    let out = decode(&c).unwrap();
    assert_eq!(out.methods.len(), 2);
    assert!(
        out.methods
            .iter()
            .all(|m| m.parameters.is_empty() && m.default.is_none())
    );
    for count in [0, 1, 255] {
        let bytes = parameters(&vec![vec![]; count]);
        let mut c = base();
        c.methods[0].attributes = vec![attr(1, &bytes)];
        let out = decode(&c).unwrap();
        assert_eq!(out.methods[0].parameters.len(), 1);
        assert_eq!(out.methods[0].parameters[0].parameters.len(), count);
    }
}

#[test]
fn every_default_tag_exact_bits_arrays_and_structural_candidates() {
    let cases = [
        ("B", scalar(b'B', 10), Value::Byte(u32::MAX)),
        ("C", scalar(b'C', 10), Value::Char(u32::MAX)),
        ("D", scalar(b'D', 14), Value::Double(0xfff8_0000_0000_1234)),
        ("F", scalar(b'F', 11), Value::Float(0x7fc0_1234)),
        ("I", scalar(b'I', 10), Value::Int(u32::MAX)),
        ("J", scalar(b'J', 12), Value::Long(u64::MAX)),
        ("S", scalar(b'S', 10), Value::Short(u32::MAX)),
        ("Z", scalar(b'Z', 10), Value::Boolean(u32::MAX)),
        (
            "Ljava/lang/String;",
            scalar(b's', 16),
            Value::String(&[0, 0xd800, 97, 0xdc00]),
        ),
        ("Ljava/lang/Class;", scalar(b'c', 19), Value::Class(&[86])),
        (
            "LE;",
            vec![b'e', 0, 17, 0, 18],
            Value::Enum {
                descriptor: &[76, 69, 59],
                name: &[79, 78, 69],
            },
        ),
        (
            "LA;",
            vec![b'@', 0, 6, 0, 0],
            Value::Annotation(annotations::Annotation {
                descriptor: &[76, 65, 59],
                elements: vec![],
            }),
        ),
    ];
    for (ret, bytes, expected) in &cases {
        let mut c = base();
        method_descriptor(&mut c, &format!("(){ret}"));
        c.methods[0].attributes = vec![attr(3, bytes)];
        assert_eq!(
            decode(&c).unwrap().methods[0].default.as_ref(),
            Some(expected),
            "{ret}"
        );
        assert_eq!(c.methods[0].attributes[0].bytes, bytes);
        for (other, _, _) in &cases {
            method_descriptor(&mut c, &format!("(){other}"));
            assert_eq!(decode(&c).is_ok(), ret == other, "{ret} -> {other}");
        }
        for values in [vec![], vec![bytes.clone(), bytes.clone()]] {
            let data = array(&values);
            let mut c = base();
            method_descriptor(&mut c, &format!("()[{ret}"));
            c.methods[0].attributes = vec![attr(3, &data)];
            let out = decode(&c).unwrap();
            let Value::Array(v) = out.methods[0].default.as_ref().unwrap() else {
                panic!()
            };
            assert_eq!(v.len(), values.len());
            for value in v {
                assert_eq!(value, expected);
            }
        }
    }
    for literal in ["V", "I", "[I", "[[LA;", "Ljava/lang/Void;"] {
        let mut c = base();
        c.constants[19] = text(literal);
        method_descriptor(&mut c, "()Ljava/lang/Class;");
        let bytes = scalar(b'c', 19);
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(
            matches!(decode(&c).unwrap().methods[0].default, Some(Value::Class(s)) if s == literal.encode_utf16().collect::<Vec<_>>())
        );
    }
    let mut c = base();
    method_descriptor(&mut c, "()LA;");
    let nested = [vec![b'@'], annotation(&array(&[scalar(b's', 16)]))].concat();
    c.methods[0].attributes = vec![attr(3, &nested)];
    assert!(decode(&c).is_ok()); // LA is not resolved to an actual annotation class.
    for (ret, bytes) in [
        ("I", array(&[])),
        ("[I", scalar(b'I', 10)),
        ("[[I", array(&[])),
        ("[I", array(&[array(&[])])),
        ("V", scalar(b'I', 10)),
    ] {
        let mut c = base();
        method_descriptor(&mut c, &format!("(){ret}"));
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(decode(&c).is_err(), "{ret}");
    }
}

#[test]
fn default_context_method_flags_and_version_sensitive_strict() {
    let bytes = scalar(b'I', 10);
    for (major, minor, flags, ok) in [
        (48, 0, 0x401, false),
        (49, 0, 0x401, true),
        (60, 0, 0xc01, false),
        (61, 0, 0xc01, true),
        (67, 0, 0xc01, true),
        (68, 0, 0x401, true),
        (69, 0, 0x401, true),
        (70, 0, 0x401, false),
        (67, 65535, 0x401, false),
        (49, 0, 0xc01, false),
    ] {
        let mut c = base();
        c.major = major;
        c.minor = minor;
        c.methods[0].access_flags = flags;
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert_eq!(decode(&c).is_ok(), ok, "{major}.{minor} flags{flags:x}");
    }
    for forbidden in [2, 4, 8, 0x10, 0x20, 0x100] {
        let mut c = base();
        c.methods[0].access_flags |= forbidden;
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("annotation default method")
        );
    }
    for flags in [0, 0x400, 1] {
        let mut c = base();
        c.methods[0].access_flags = flags;
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(decode(&c).is_err());
    }
    for flags in [0, 0x200, 0x2000] {
        let mut c = base();
        c.access_flags = flags;
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(decode(&c).is_err());
    }
    for descriptor in ["(I)I", "()V", "()", "I", "()[V"] {
        let mut c = base();
        method_descriptor(&mut c, descriptor);
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(decode(&c).is_err());
    }
    for name in ["<init>", "<clinit>", "a>b", "", "a/b"] {
        let mut c = base();
        c.constants[8] = text(name);
        c.methods[0].attributes = vec![attr(3, &bytes)];
        assert!(decode(&c).is_err());
    }
    let mut c = base();
    c.methods[0].access_flags |= 0x8000;
    c.methods[0].attributes = vec![attr(3, &bytes)];
    assert!(decode(&c).is_ok());
}

#[test]
fn placements_duplicate_content_versions_and_opaque_interiors() {
    for (name, bytes) in [(1, vec![0]), (2, vec![0]), (3, scalar(b'I', 10))] {
        for context in 0..2 {
            let mut c = base();
            if context == 0 {
                c.attributes = vec![attr(name, &bytes)];
            } else {
                c.fields = vec![Member {
                    access_flags: 0,
                    name: 0,
                    descriptor: 0,
                    attributes: vec![attr(name, &bytes)],
                }];
            }
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("annotation extension context")
            );
        }
        let mut c = base();
        let duplicate = c.constants.len() as u16;
        let Constant::Utf8(s) = &c.constants[name as usize] else {
            panic!()
        };
        c.constants.push(Constant::Utf8(s.clone()));
        c.methods[0].attributes = vec![attr(name, &bytes), attr(duplicate, &bytes)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("duplicate annotation extension")
        );
        c.methods[0].attributes.pop();
        for (major, minor, ok) in [
            (45, 3, false),
            (48, 0, false),
            (49, 0, true),
            (67, 0, true),
            (68, 0, true),
            (69, 0, true),
            (70, 0, false),
            (67, 1, false),
            (67, 65535, false),
        ] {
            c.major = major;
            c.minor = minor;
            assert_eq!(decode(&c).is_ok(), ok, "{name} {major}.{minor}");
        }
    }
    let mut c = base();
    c.constants.push(text("Code"));
    c.constants.push(text("RuntimeVisibleTypeAnnotations"));
    c.methods[0].attributes = vec![attr(20, b"AnnotationDefault"), attr(21, &[255])];
    assert!(decode(&c).is_ok());
    let empty = [0];
    c.methods[0].attributes = vec![attr(1, &empty), attr(2, &empty)];
    assert_eq!(decode(&c).unwrap().methods[0].parameters.len(), 2);
}

#[test]
fn every_payload_prefix_count_boundary_and_trailing_bytes() {
    let nested = [
        vec![b'@'],
        annotation(&array(&[scalar(b's', 16), scalar(b'c', 19)])),
    ]
    .concat();
    let p = parameters(&[
        vec![annotation(&nested)],
        vec![],
        vec![annotation(&scalar(b'I', 10))],
    ]);
    for (name, bytes, descriptor) in [
        (1, p, "()I"),
        (2, parameters(&[vec![annotation(&nested)]]), "()I"),
        (3, nested, "()LA;"),
    ] {
        let mut c = base();
        method_descriptor(&mut c, descriptor);
        for end in 0..bytes.len() {
            c.methods[0].attributes = vec![attr(name, &bytes[..end])];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Truncated,
                "attr{name} payload-prefix{end}"
            );
        }
        c.methods[0].attributes = vec![attr(name, &bytes)];
        assert!(decode(&c).is_ok());
        let mut trailing = bytes.clone();
        trailing.push(0);
        c.methods[0].attributes = vec![attr(name, &trailing)];
        assert_eq!(
            decode(&c).unwrap_err(),
            Error::Malformed("annotation extension trailing bytes")
        );
    }
    for (name, bytes, expected) in [
        (1, vec![255], Error::Truncated),
        (1, vec![1, 255, 255], Error::Limit("annotation nodes")),
        (3, vec![b'[', 255, 255], Error::Limit("annotation nodes")),
    ] {
        let mut c = base();
        method_descriptor(&mut c, "()[I");
        c.methods[0].attributes = vec![attr(name, &bytes)];
        assert_eq!(decode(&c).unwrap_err(), expected);
    }
}

#[test]
fn consumed_method_boundaries_special_names_and_parameter_slots() {
    let bytes = [0];
    for (name, descriptor, flags, ok) in [
        ("<init>", "(I)V", 0, true),
        ("<init>", "()I", 0, false),
        ("<clinit>", "()V", 8, true),
        ("<clinit>", "()V", 0, false),
        ("<clinit>", "(I)V", 8, false),
        ("value", "(J[DLx;)V", 0, true),
        ("value", "(V)V", 0, false),
        ("value", "(L;)V", 0, false),
    ] {
        let mut c = base();
        c.constants[8] = text(name);
        method_descriptor(&mut c, descriptor);
        c.methods[0].access_flags = flags;
        c.methods[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(decode(&c).is_ok(), ok, "{name} {descriptor}");
    }
    for (count, flags, ok) in [
        (254, 0, true),
        (255, 0, false),
        (255, 8, true),
        (256, 8, false),
    ] {
        let mut c = base();
        method_descriptor(&mut c, &format!("({})V", "I".repeat(count)));
        c.methods[0].access_flags = flags;
        c.methods[0].attributes = vec![attr(1, &bytes)];
        assert_eq!(decode(&c).is_ok(), ok);
    }
}

#[test]
fn typed_cp_indices_forged_zero_holes_and_unknown_tags() {
    for position in 0..6 {
        for invalid in [0, 13, 15, 20, u16::MAX] {
            let mut c = base();
            c.constants[0] = text("usable-zero");
            let mut bytes = parameters(&[vec![annotation(&scalar(b's', 16))]]);
            match position {
                0 => (),
                1 => c.methods[0].name = invalid,
                2 => c.methods[0].descriptor = invalid,
                3 => bytes[3..5].copy_from_slice(&invalid.to_be_bytes()),
                4 => bytes[7..9].copy_from_slice(&invalid.to_be_bytes()),
                _ => bytes[10..12].copy_from_slice(&invalid.to_be_bytes()),
            }
            c.methods[0].attributes = vec![attr(if position == 0 { invalid } else { 1 }, &bytes)];
            assert!(decode(&c).is_err(), "position{position} index{invalid}");
        }
    }
    for tag in b"BCDFIJSZsc" {
        for candidate in [
            Constant::Unusable,
            Constant::Integer(1),
            Constant::Float(1),
            Constant::Long(1),
            Constant::Double(1),
            text("I"),
            Constant::String(16),
        ] {
            let mut c = base();
            let valid = matches!(
                (*tag, &candidate),
                (b'B' | b'C' | b'I' | b'S' | b'Z', Constant::Integer(_))
                    | (b'D', Constant::Double(_))
                    | (b'F', Constant::Float(_))
                    | (b'J', Constant::Long(_))
                    | (b's' | b'c', Constant::Utf8(_))
            );
            c.constants.push(candidate);
            let bytes = parameters(&[vec![annotation(&scalar(*tag, 20))]]);
            c.methods[0].attributes = vec![attr(1, &bytes)];
            assert_eq!(decode(&c).is_ok(), valid, "tag{tag}");
        }
    }
    for tag in 0..=255 {
        if b"BCDFIJSZsec@[".contains(&tag) {
            continue;
        }
        for name in [1, 3] {
            let mut c = base();
            let bytes = if name == 1 {
                parameters(&[vec![annotation(&[tag])]])
            } else {
                vec![tag]
            };
            c.methods[0].attributes = vec![attr(name, &bytes)];
            assert_eq!(
                decode(&c).unwrap_err(),
                Error::Malformed("annotation value tag")
            );
        }
    }
}

#[test]
fn declaration_errors_still_precede_extension_validation() {
    let mut c = base();
    let invalid = [0, 1, 0, 6, 0, 1, 0, 7, 0];
    c.attributes = vec![attr(1, &[0]), attr(4, &invalid)];
    let expected = Error::Malformed("annotation value tag");
    assert_eq!(annotations::decode(&c).unwrap_err(), expected);
    assert_eq!(decode(&c).unwrap_err(), expected);
}

#[test]
fn aggregate_nodes_exact_mixed_boundary_and_no_member_reset() {
    let a = annotation(&scalar(b'I', 10));
    let declaration = [vec![0, 1], a.clone()].concat();
    let p = parameters(&[vec![a]]);
    // F0 + M1 + A3 + P1 + R2 + E2 + V(3+X) + N0 = 12+X.
    for (count, extra_method, ok) in [
        (MAX_RECORDS - 12, false, true),
        (MAX_RECORDS - 11, false, false),
        (MAX_RECORDS - 12, true, false),
    ] {
        let mut c = base();
        method_descriptor(&mut c, "()[I");
        let d = array(&vec![scalar(b'I', 10); count]);
        c.attributes = vec![attr(4, &declaration)];
        c.methods[0].attributes = vec![attr(1, &p), attr(3, &d)];
        if extra_method {
            c.methods.push(Member {
                access_flags: 0,
                name: 0,
                descriptor: 0,
                attributes: vec![],
            });
        }
        let out = decode(&c);
        if ok {
            assert_eq!(out.unwrap().declarations, annotations::decode(&c).unwrap());
        } else {
            assert_eq!(out.unwrap_err(), Error::Limit("annotation nodes"));
        }
    }
    let mut c = base();
    c.fields = (0..MAX_RECORDS)
        .map(|_| Member {
            access_flags: 0,
            name: 0,
            descriptor: 0,
            attributes: vec![],
        })
        .collect();
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation nodes"));
}

#[test]
fn payload_bytes_once_and_combined_work_exhaustion() {
    let mut c = base();
    c.constants.push(text("Ignored"));
    let p = [0];
    for (size, ok) in [(MAX_BYTES - 1, true), (MAX_BYTES, false)] {
        let bytes = vec![0; size];
        let mut c = base();
        c.constants.push(text("Ignored"));
        c.attributes = vec![attr(20, &bytes)];
        c.methods[0].attributes = vec![attr(1, &p)];
        assert_eq!(decode(&c).is_ok(), ok);
    }
    c.constants[20] = Constant::Utf8(vec![65; 65535]);
    for (count, ok) in [(511, true), (512, false)] {
        c.attributes = (0..count).map(|_| attr(20, &[])).collect();
        c.methods[0].attributes = vec![attr(1, &p)];
        assert!(annotations::decode(&c).is_ok());
        let out = decode(&c);
        if ok {
            assert!(out.is_ok());
        } else {
            assert_eq!(out.unwrap_err(), Error::Limit("annotation work"));
        }
    }
}

#[test]
fn exact_shared_work_with_once_per_method_validation_and_default_comparison() {
    let mut c = base();
    method_descriptor(&mut c, "()LE;");
    let default = vec![b'e', 0, 17, 0, 18];
    let p = [0];
    // Two scans of both names, one method name/descriptor read, two enum text
    // reads, shape 1+3*3, and candidate comparison min(3,3)+1, per method.
    let per_method = 2
        * (1 + "RuntimeVisibleParameterAnnotations".len() + 1 + "AnnotationDefault".len())
        + 6
        + 6
        + 8
        + 10
        + 4;
    let remaining = (ktlsp_binary::MAX_VALIDATION_UNITS - 2 * per_method) / 2 - 511 * 65536;
    c.constants.push(Constant::Utf8(vec![65; 65535]));
    c.constants.push(Constant::Utf8(vec![65; remaining - 1]));
    c.attributes = (0..511).map(|_| attr(20, &[])).collect();
    c.attributes.push(attr(21, &[]));
    c.methods[0].attributes = vec![attr(1, &p), attr(3, &default)];
    c.methods.push(Member {
        access_flags: 0x401,
        name: 8,
        descriptor: 9,
        attributes: vec![attr(1, &p), attr(3, &default)],
    });
    assert!(decode(&c).is_ok());
    let Constant::Utf8(s) = &mut c.constants[21] else {
        panic!()
    };
    s.push(65);
    assert!(annotations::decode(&c).is_ok());
    assert_eq!(decode(&c).unwrap_err(), Error::Limit("annotation work"));
}

#[test]
fn default_and_parameter_nesting_depth_boundaries_and_mutations() {
    for default in [false, true] {
        for alternating in [false, true] {
            for wrappers in 0..=MAX_DEPTH {
                let mut value = scalar(b's', 16);
                let mut depth = if default { 1 } else { 2 };
                for i in 0..wrappers {
                    if alternating && i % 2 == 0 {
                        value = array(&[value]);
                        depth += 1;
                    } else {
                        value = [vec![b'@'], annotation(&value)].concat();
                        depth += 2;
                    }
                }
                // A default's top-level shape must be legal, independent of internal nesting.
                if default {
                    value = [vec![b'@'], annotation(&value)].concat();
                    depth += 2;
                }
                let bytes = if default {
                    value
                } else {
                    parameters(&[vec![annotation(&value)]])
                };
                let mut c = base();
                if default {
                    method_descriptor(&mut c, "()LA;");
                }
                c.methods[0].attributes = vec![attr(if default { 3 } else { 1 }, &bytes)];
                let out = decode(&c);
                if depth <= MAX_DEPTH {
                    assert!(out.is_ok(), "depth{depth}");
                } else {
                    assert_eq!(out.unwrap_err(), Error::Limit("annotation depth"));
                }
            }
        }
    }
    for (name, bytes) in [
        (
            1,
            parameters(&[vec![annotation(&array(&[scalar(b's', 16)]))]]),
        ),
        (
            3,
            [vec![b'@'], annotation(&array(&[scalar(b's', 16)]))].concat(),
        ),
    ] {
        for at in 0..bytes.len() {
            for byte in 0..=255 {
                let mut c = base();
                method_descriptor(&mut c, "()LA;");
                let mut mutated = bytes.clone();
                mutated[at] = byte;
                c.methods[0].attributes = vec![attr(name, &mutated)];
                let _ = decode(&c);
            }
        }
    }
}

#[test]
fn precompiled_parameter_annotations_and_defaults_data_only() {
    let bytes = include_bytes!("fixtures/annotation_extensions/ReadAfterEOFException.class.bin");
    let c = parse_class(bytes).unwrap();
    let out = decode(&c).unwrap();
    assert_eq!(out.declarations, annotations::decode(&c).unwrap());
    let index = c
        .methods
        .iter()
        .position(|m| c.utf16(m.name).unwrap() == "<init>".encode_utf16().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(
        c.utf16(c.methods[index].descriptor).unwrap(),
        "(Ljava/lang/String;)V".encode_utf16().collect::<Vec<_>>()
    );
    let p = &out.methods[index].parameters;
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].visibility, Visibility::RuntimeInvisible);
    assert_eq!(p[0].parameters.len(), 1);
    assert_eq!(p[0].parameters[0].len(), 1);
    assert_eq!(
        p[0].parameters[0][0].descriptor,
        "Lorg/jetbrains/annotations/Nullable;"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    assert!(p[0].parameters[0][0].elements.is_empty());
    for end in 0..bytes.len() {
        assert!(parse_class(&bytes[..end]).is_err(), "class-prefix{end}");
    }
    let bytes = include_bytes!("../fixtures/t017c2/Metadata.class");
    let c = parse_class(bytes).unwrap();
    let out = decode(&c).unwrap();
    let defaults: Vec<_> = c
        .methods
        .iter()
        .zip(&out.methods)
        .filter_map(|(m, e)| e.default.as_ref().map(|v| (c.utf16(m.name).unwrap(), v)))
        .collect();
    assert_eq!(defaults.len(), 8);
    for (expected_name, expected_value) in [
        ("k", Value::Int(1)),
        ("mv", Value::Array(vec![])),
        (
            "bv",
            Value::Array(vec![Value::Int(1), Value::Int(0), Value::Int(3)]),
        ),
        ("d1", Value::Array(vec![])),
        ("d2", Value::Array(vec![])),
        ("xs", Value::String(&[])),
        ("pn", Value::String(&[])),
        ("xi", Value::Int(0)),
    ] {
        let name: Vec<_> = expected_name.encode_utf16().collect();
        assert!(
            defaults
                .iter()
                .any(|(n, v)| *n == name && **v == expected_value),
            "{expected_name}"
        );
    }
}

#[test]
fn modern_nonempty_parameter_annotations_preserve_encoded_positions() {
    let a = annotation(&scalar(b's', 16));
    let bytes = parameters(&[vec![], vec![a]]);
    for major in [68, 69] {
        let mut c = base();
        c.major = major;
        method_descriptor(&mut c, "(I)V");
        c.methods[0].attributes = vec![attr(1, &bytes)];
        let out = decode(&c).unwrap();
        assert_eq!(out.methods[0].parameters[0].parameters.len(), 2);
        assert!(out.methods[0].parameters[0].parameters[0].is_empty());
        assert_eq!(out.methods[0].parameters[0].parameters[1][0].elements[0].value, Value::String(&[0, 0xd800, 97, 0xdc00]));
    }
}
