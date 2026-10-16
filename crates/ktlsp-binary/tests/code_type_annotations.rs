use ktlsp_binary::annotations::{Value, Visibility};
use ktlsp_binary::code::{self, CodeBody};
use ktlsp_binary::code_type_annotations::{CodeTargetInfo, Limits, decode};
use ktlsp_binary::{Attribute, ClassFile, Constant, Error, Member, parse_class};

fn fixture() -> Vec<u8> {
    include_str!("fixtures/code_type_annotations/all_targets.hex")
        .split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn raw(attrs: &[(u16, &[u8])]) -> Vec<u8> {
    let mut raw = vec![
        0, 1, 0, 2, 0, 0, 0, 4, 0, 0, 0, 0xb1, 0, 1, 0, 0, 0, 4, 0, 3, 0, 0,
    ];
    raw.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
    for (name, bytes) in attrs {
        raw.extend_from_slice(&name.to_be_bytes());
        raw.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        raw.extend_from_slice(bytes);
    }
    raw
}
fn make_class(raw: &[u8]) -> ClassFile<'_> {
    ClassFile {
        minor: 0,
        major: 52,
        constants: vec![
            Constant::Unusable,
            text("Code"),
            text("RuntimeVisibleTypeAnnotations"),
            text("RuntimeInvisibleTypeAnnotations"),
            text("LA;"),
            text("value"),
            Constant::Integer(0xfedcba98),
            text("Ignored"),
            text("RuntimeVisibleTypeAnnotations"),
            text("method"),
        ],
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        attributes: vec![],
        methods: vec![Member {
            access_flags: 0,
            name: 9,
            descriptor: 0,
            attributes: vec![Attribute {
                name: 1,
                bytes: raw,
            }],
        }],
    }
}
fn text(s: &str) -> Constant {
    Constant::Utf8(s.encode_utf16().collect())
}
fn with_code(attrs: &[(u16, &[u8])], check: impl FnOnce(&ClassFile<'_>, &mut CodeBody<'_>)) {
    let raw = raw(attrs);
    let class = make_class(&raw);
    let mut bodies = code::decode(&class, code::Limits::default()).unwrap();
    check(&class, bodies[0].as_mut().unwrap());
}
fn error(payload: &[u8], expected: Error) {
    with_code(&[(2, payload)], |class, body| {
        assert_eq!(decode(class, body, Limits::default()), Err(expected));
    });
}

#[test]
fn all_targets_paths_values_order_and_borrowed_spans() {
    let payload = fixture();
    with_code(
        &[(3, &payload), (7, &[0xff]), (2, &[0, 0])],
        |class, body| {
            let out = decode(class, body, Limits::default()).unwrap();
            assert_eq!(out.len(), 2);
            assert_eq!(out[0].visibility, Visibility::RuntimeInvisible);
            assert_eq!(out[1].visibility, Visibility::RuntimeVisible);
            assert!(out[1].annotations.is_empty());
            assert_eq!(out[0].raw.as_ptr(), body.attributes[0].bytes.as_ptr());
            let roots = &out[0].annotations;
            assert_eq!(
                roots.iter().map(|a| a.target_type).collect::<Vec<_>>(),
                (0x40..=0x4b).collect::<Vec<_>>()
            );
            let mut at = 2;
            for root in roots {
                assert_eq!(root.raw.as_ptr(), out[0].raw[at..].as_ptr());
                at += root.raw.len();
                assert_eq!(
                    root.annotation.descriptor.as_ptr(),
                    class.utf16(4).unwrap().as_ptr()
                );
            }
            assert_eq!(at, payload.len());
            let CodeTargetInfo::LocalVariable(rows) = &roots[0].target_info else {
                panic!()
            };
            assert_eq!(rows.len(), 2);
            assert_eq!((rows[0].start_pc, rows[0].length, rows[0].index), (0, 4, 0));
            assert_eq!((rows[1].start_pc, rows[1].length, rows[1].index), (3, 0, 1));
            let CodeTargetInfo::LocalVariable(rows) = &roots[1].target_info else {
                panic!()
            };
            assert_eq!((rows[0].start_pc, rows[0].length, rows[0].index), (0, 2, 1));
            assert_eq!(
                roots[2].target_info,
                CodeTargetInfo::Catch {
                    exception_table_index: 0
                }
            );
            for (root, offset) in roots[3..7].iter().zip([0, 3, 1, 2]) {
                assert_eq!(root.target_info, CodeTargetInfo::Offset { offset });
            }
            for (root, (offset, type_argument_index)) in
                roots[7..]
                    .iter()
                    .zip([(0, 0), (3, 255), (1, 1), (2, 2), (3, 255)])
            {
                assert_eq!(
                    root.target_info,
                    CodeTargetInfo::TypeArgument {
                        offset,
                        type_argument_index
                    }
                );
            }
            assert_eq!(
                roots[4]
                    .path
                    .iter()
                    .map(|p| (p.kind, p.argument_index))
                    .collect::<Vec<_>>(),
                [(0, 0), (1, 0), (2, 0), (3, 255)]
            );
            let Value::Array(values) = &roots[11].annotation.elements[0].value else {
                panic!()
            };
            assert_eq!(values[0], Value::Int(0xfedcba98));
            let Value::Annotation(nested) = &values[1] else {
                panic!()
            };
            assert!(nested.elements.is_empty());
        },
    );
    with_code(&[], |class, body| {
        assert!(decode(class, body, Limits::default()).unwrap().is_empty())
    });
    with_code(&[(7, &[])], |class, body| {
        assert!(decode(class, body, Limits::default()).unwrap().is_empty())
    });
    with_code(&[(2, &[0, 0]), (3, &[0, 0])], |class, body| {
        assert_eq!(
            decode(class, body, Limits::default())
                .unwrap()
                .iter()
                .map(|a| &a.visibility)
                .collect::<Vec<_>>(),
            [&Visibility::RuntimeVisible, &Visibility::RuntimeInvisible]
        );
    });
}

#[test]
fn prefixes_trailing_bytes_illegal_tags_and_paths() {
    let payload = fixture();
    for len in 0..payload.len() {
        error(&payload[..len], Error::Truncated);
    }
    let mut trailing = payload.clone();
    trailing.push(0);
    error(
        &trailing,
        Error::Malformed("type annotation trailing bytes"),
    );
    for tag in 0..=255 {
        if !(0x40..=0x4b).contains(&tag) {
            error(
                &[0, 1, tag],
                Error::Malformed("code type annotation target"),
            );
        }
    }
    for kind in 0..=255 {
        let expected = if kind <= 3 {
            Error::Truncated
        } else {
            Error::Malformed("type annotation path")
        };
        error(&[0, 1, 0x43, 0, 0, 1, kind, 0], expected);
        error(&[0, 1, 0x43, 0, 0, 1, kind], Error::Truncated);
    }
    for kind in 0..=2 {
        error(
            &[0, 1, 0x43, 0, 0, 1, kind, 1],
            Error::Malformed("type annotation path"),
        );
    }
}

#[test]
fn target_boundaries_and_annotation_validation() {
    for tag in 0x43..=0x4b {
        error(
            &[0, 1, tag, 0, 4],
            Error::Malformed("code type annotation offset"),
        );
        error(
            &[0, 1, tag, 0xff, 0xff],
            Error::Malformed("code type annotation offset"),
        );
    }
    for index in [1u16, 65535] {
        let [a, b] = index.to_be_bytes();
        error(
            &[0, 1, 0x42, a, b],
            Error::Malformed("code type annotation catch index"),
        );
    }
    for (start, len, index) in [
        (4u16, 0u16, 0u16),
        (0, 5, 0),
        (3, 2, 0),
        (0, 1, 2),
        (65535, 65535, 0),
    ] {
        let mut p = vec![0, 1, 0x40, 0, 1];
        for n in [start, len, index] {
            p.extend_from_slice(&n.to_be_bytes());
        }
        error(&p, Error::Malformed("code type annotation local range"));
    }
    with_code(&[(2, &[0, 1, 0x40, 0, 0, 0, 0, 4, 0, 0])], |class, body| {
        assert!(decode(class, body, Limits::default()).is_ok())
    });
    error(
        &[0, 1, 0x43, 0, 0, 0, 0, 0, 0, 0],
        Error::Malformed("annotation constant index"),
    );
    error(
        &[0, 1, 0x43, 0, 0, 0, 0, 6, 0, 0],
        Error::Malformed("UTF8 index"),
    );
    error(
        &[0, 1, 0x43, 0, 0, 0, 0, 4, 0, 1, 0, 5, b'I', 0, 4],
        Error::Malformed("annotation constant type"),
    );
    error(
        &[0, 1, 0x43, 0, 0, 0, 0, 4, 0, 1, 0, 5, 0],
        Error::Malformed("annotation value tag"),
    );
}

#[test]
fn duplicates_and_version_precedence() {
    with_code(&[(2, &[0, 0]), (8, &[])], |class, body| {
        assert_eq!(
            decode(class, body, Limits::default()),
            Err(Error::Malformed("duplicate type annotation attribute"))
        )
    });
    with_code(&[(2, &[]), (8, &[])], |class, body| {
        assert_eq!(
            decode(class, body, Limits::default()),
            Err(Error::Truncated)
        )
    });
    for (major, minor) in [
        (45, 0),
        (45, 3),
        (49, 0),
        (51, 0),
        (52, 0),
        (67, 0),
        (68, 0),
        (69, 0),
        (70, 0),
        (52, 1),
    ] {
        let raw = raw(&[(2, &[])]);
        let mut c = make_class(&raw);
        let base = make_class(&raw);
        let mut bodies = code::decode(&base, code::Limits::default()).unwrap();
        let body = bodies.remove(0).unwrap();
        c.major = major;
        c.minor = minor;
        let expected = if major == 70 || minor == 1 {
            Error::UnsupportedVersion { major, minor }
        } else if major < 52 {
            Error::Malformed("type annotation version")
        } else {
            Error::Truncated
        };
        assert_eq!(decode(&c, &body, Limits::default()), Err(expected));
    }
}

#[test]
fn forged_body_fields_and_cross_buffer_spans_are_rejected() {
    let payload = fixture();
    for mode in 0..8 {
        with_code(&[(2, &payload)], |class, body| {
            let raw_copy = body.raw.to_vec();
            let bytecode_copy = body.bytecode.to_vec();
            let payload_copy = body.attributes[0].bytes.to_vec();
            let mut forged = CodeBody {
                raw: if mode == 7 { &raw_copy } else { body.raw },
                max_stack: body.max_stack,
                max_locals: body.max_locals,
                bytecode: if mode == 2 {
                    &bytecode_copy
                } else {
                    body.bytecode
                },
                exceptions: body
                    .exceptions
                    .iter()
                    .map(|e| code::ExceptionHandler {
                        start_pc: e.start_pc,
                        end_pc: e.end_pc,
                        handler_pc: e.handler_pc,
                        catch_type: e.catch_type,
                    })
                    .collect(),
                attributes: vec![Attribute {
                    name: body.attributes[0].name,
                    bytes: if mode == 3 {
                        &payload_copy
                    } else {
                        body.attributes[0].bytes
                    },
                }],
            };
            match mode {
                0 => forged.max_stack += 1,
                1 => forged.max_locals += 1,
                4 => forged.attributes[0].name = 3,
                5 => forged.exceptions[0].handler_pc = 0,
                6 => forged.exceptions.clear(),
                _ => {}
            }
            assert_eq!(
                decode(class, &forged, Limits::default()),
                Err(Error::Malformed("code body mismatch"))
            );
        });
    }
    with_code(&[(2, &payload)], |class, body| {
        body.attributes.clear();
        assert_eq!(
            decode(class, body, Limits::default()),
            Err(Error::Malformed("code body mismatch"))
        );
    });
    with_code(&[(2, &payload)], |class, body| {
        let full = body.raw;
        for len in 0..full.len() {
            body.raw = &full[..len];
            assert_eq!(
                decode(class, body, Limits::default()),
                Err(Error::Truncated),
                "prefix{len}"
            );
        }
    });
}

#[test]
fn aggregate_limits_are_tightened_and_shared_across_visibilities() {
    let payload = fixture();
    with_code(&[(2, &payload), (3, &payload)], |class, body| {
        let unlimited = Limits {
            bytes: usize::MAX,
            nodes: usize::MAX,
            work: usize::MAX,
        };
        assert!(decode(class, body, unlimited).is_ok());
        assert!(
            decode(
                class,
                body,
                Limits {
                    bytes: body.raw.len(),
                    ..unlimited
                }
            )
            .is_ok()
        );
        assert_eq!(
            decode(
                class,
                body,
                Limits {
                    bytes: body.raw.len() - 1,
                    ..unlimited
                }
            ),
            Err(Error::Limit("code type annotation bytes"))
        );
        for (limits, label) in [
            (
                Limits {
                    bytes: 0,
                    ..unlimited
                },
                "code type annotation bytes",
            ),
            (
                Limits {
                    nodes: 0,
                    ..unlimited
                },
                "annotation nodes",
            ),
            (
                Limits {
                    work: 0,
                    ..unlimited
                },
                "annotation work",
            ),
        ] {
            assert_eq!(decode(class, body, limits), Err(Error::Limit(label)));
        }
        for nodes in [true, false] {
            let (mut low, mut high) = (1, 10000);
            while low < high {
                let cut = (low + high) / 2;
                let limits = if nodes {
                    Limits {
                        nodes: cut,
                        ..unlimited
                    }
                } else {
                    Limits {
                        work: cut,
                        ..unlimited
                    }
                };
                if decode(class, body, limits).is_ok() {
                    high = cut;
                } else {
                    low = cut + 1;
                }
            }
            let required = low;
            let exact = if nodes {
                Limits {
                    nodes: required,
                    ..unlimited
                }
            } else {
                Limits {
                    work: required,
                    ..unlimited
                }
            };
            assert!(decode(class, body, exact).is_ok());
            let limits = if nodes {
                Limits {
                    nodes: required - 1,
                    ..unlimited
                }
            } else {
                Limits {
                    work: required - 1,
                    ..unlimited
                }
            };
            assert_eq!(
                decode(class, body, limits),
                Err(Error::Limit(if nodes {
                    "annotation nodes"
                } else {
                    "annotation work"
                }))
            );
            let single_raw = raw(&[(2, &payload)]);
            let single = make_class(&single_raw);
            let bodies = code::decode(&single, code::Limits::default()).unwrap();
            assert!(decode(&single, bodies[0].as_ref().unwrap(), limits).is_ok());
        }
    });
}

#[test]
fn genuine_class_without_code_type_annotations_stays_empty() {
    let class = parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let bodies = code::decode(&class, code::Limits::default()).unwrap();
    assert_eq!(bodies.iter().flatten().count(), 3);
    for body in bodies.iter().flatten() {
        assert!(decode(&class, body, Limits::default()).unwrap().is_empty());
    }
}

// Fixed authored outer framing; deliberately bypasses code::decode validation.
fn direct_body(raw: &[u8], payload_end: usize) -> CodeBody<'_> {
    let u16_at = |at| u16::from_be_bytes([raw[at], raw[at + 1]]);
    CodeBody {
        raw,
        max_stack: 1,
        max_locals: 2,
        bytecode: &raw[8..12],
        exceptions: vec![code::ExceptionHandler {
            start_pc: u16_at(14),
            end_pc: u16_at(16),
            handler_pc: u16_at(18),
            catch_type: u16_at(20),
        }],
        attributes: vec![Attribute {
            name: u16_at(24),
            bytes: &raw[30..payload_end],
        }],
    }
}

#[test]
fn self_consistent_forged_body_trust_boundary() {
    for (mode, expected) in [
        (0, Error::Malformed("code length")),
        (1, Error::Malformed("code length")),
        (2, Error::Malformed("code exception range")),
        (3, Error::Malformed("code exception range")),
        (4, Error::Malformed("code exception range")),
        (5, Error::Malformed("class index")),
        (6, Error::Malformed("constant index")),
        (7, Error::Malformed("annotation constant index")),
        (8, Error::Malformed("UTF8 index")),
        (9, Error::Malformed("constant index")),
        (10, Error::Malformed("annotation constant pool size")),
        (11, Error::Malformed("annotation constant pool size")),
        (12, Error::Malformed("code trailing bytes")),
    ] {
        let mut bytes = raw(&[(2, &[0, 0])]);
        match mode {
            0 => bytes[4..8].copy_from_slice(&0u32.to_be_bytes()),
            1 => bytes[4..8].copy_from_slice(&65536u32.to_be_bytes()),
            2 => bytes[14..16].copy_from_slice(&4u16.to_be_bytes()),
            3 => bytes[16..18].copy_from_slice(&5u16.to_be_bytes()),
            4 => bytes[18..20].copy_from_slice(&4u16.to_be_bytes()),
            5 => bytes[20..22].copy_from_slice(&4u16.to_be_bytes()),
            6 => bytes[20..22].copy_from_slice(&65535u16.to_be_bytes()),
            7 => bytes[24..26].copy_from_slice(&0u16.to_be_bytes()),
            8 => bytes[24..26].copy_from_slice(&6u16.to_be_bytes()),
            9 => bytes[24..26].copy_from_slice(&65535u16.to_be_bytes()),
            12 => bytes.push(0),
            _ => {}
        }
        let mut class = make_class(&bytes);
        match mode {
            7 => class.constants[0] = text("RuntimeVisibleTypeAnnotations"),
            10 => class.constants.clear(),
            11 => {
                class.constants = (0..=ktlsp_binary::MAX_RECORDS)
                    .map(|_| Constant::Unusable)
                    .collect()
            }
            _ => {}
        }
        let body = direct_body(&bytes, 32);
        assert_eq!(
            decode(&class, &body, Limits::default()),
            Err(expected),
            "mode{mode}"
        );
    }
    let bytes = raw(&[(7, &[])]);
    let class = make_class(&bytes);
    let mut body = direct_body(&bytes, 30);
    assert!(decode(&class, &body, Limits::default()).is_ok());
    body.attributes[0].bytes = &[];
    assert_eq!(
        decode(&class, &body, Limits::default()),
        Err(Error::Malformed("code body mismatch"))
    );
}

#[test]
fn annotation_depth_duplicates_and_deterministic_mutations() {
    for (arrays, succeeds) in [(62, true), (63, false)] {
        let mut payload = vec![0, 1, 0x43, 0, 0, 0, 0, 4, 0, 1, 0, 5];
        for _ in 0..arrays {
            payload.extend_from_slice(&[b'[', 0, 1]);
        }
        payload.extend_from_slice(&[b'I', 0, 6]);
        with_code(&[(2, &payload)], |class, body| {
            let result = decode(class, body, Limits::default());
            if succeeds {
                assert!(result.is_ok());
            } else {
                assert_eq!(result, Err(Error::Limit("annotation depth")));
            }
        });
    }
    error(
        &[
            0, 1, 0x43, 0, 0, 0, 0, 4, 0, 2, 0, 5, b'I', 0, 6, 0, 5, b'I', 0, 6,
        ],
        Error::Malformed("duplicate annotation element"),
    );
    let original = fixture();
    for at in 0..original.len() {
        for byte in [0, 255] {
            let mut payload = original.clone();
            payload[at] = byte;
            with_code(&[(2, &payload)], |class, body| {
                let before = body.raw.to_vec();
                let _ = decode(class, body, Limits::default());
                assert_eq!(body.raw, before);
            });
        }
    }
}

#[test]
fn modern_authored_nonempty_code_type_annotations_and_tightened_bytes() {
    let payload = fixture();
    let raw = raw(&[(2, &payload)]);
    for major in [68, 69] {
        let mut c = make_class(&raw);
        c.major = major;
        let bodies = code::decode(&c, code::Limits::default()).unwrap();
        let body = bodies[0].as_ref().unwrap();
        let out = decode(&c, body, Limits::default()).unwrap();
        assert_eq!(out[0].annotations.iter().map(|a| a.target_type).collect::<Vec<_>>(), (0x40..=0x4b).collect::<Vec<_>>());
        assert_eq!(out[0].raw.as_ptr(), body.attributes[0].bytes.as_ptr());
        assert!(decode(&c, body, Limits { bytes: raw.len(), ..Limits::default() }).is_ok());
        assert_eq!(decode(&c, body, Limits { bytes: raw.len()-1, ..Limits::default() }).unwrap_err(), Error::Limit("code type annotation bytes"));
    }
}
