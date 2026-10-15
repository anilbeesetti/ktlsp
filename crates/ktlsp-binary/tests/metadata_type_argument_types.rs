use ktlsp_binary::metadata::{
    type_argument::{InlineType, Projection},
    type_argument_types::{Error, ErrorKind, Limits, SelectedType, TypeArgumentWithTypes, decode},
    type_header::TypeHeader,
};
use std::mem::size_of;

const S: usize = size_of::<InlineType<'_>>();
const T: usize = size_of::<SelectedType<'_>>();

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn header(raw: &[u8]) -> TypeHeader<'_> {
    TypeHeader {
        raw,
        flags: None,
        nullable: None,
        flexible_type_capabilities_id: None,
        class_name: None,
        type_parameter: None,
        flexible_upper_bound_id: None,
        type_parameter_name: None,
        outer_type_id: None,
        type_alias_name: None,
        abbreviated_type_id: None,
    }
}
fn retained(out: &TypeArgumentWithTypes<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for body in &out.inline_types {
        assert!(end <= body.span.start && body.span.start <= body.span.end);
        assert!(body.span.end <= bytes.len());
        assert_eq!(body.header.raw, &bytes[body.span.clone()]);
        assert_eq!(body.header.raw.as_ptr(), bytes[body.span.clone()].as_ptr());
        end = body.span.end;
    }
}
struct Prefix {
    cut: usize,
    projection: Option<Projection>,
    type_id: Option<i32>,
    types: Vec<SelectedType<'static>>,
}
struct Case {
    bytes: Vec<u8>,
    prefixes: Vec<Prefix>,
    limits: Limits,
    // First deficits: records, nodes, depth, owned, work.
    offsets: [usize; 5],
}
fn check(bytes: &[u8], limits: Limits, expected: &Prefix) {
    let out = decode(bytes, limits).unwrap();
    retained(&out, bytes);
    assert_eq!(
        (out.projection, out.type_id),
        (expected.projection, expected.type_id)
    );
    assert_eq!(out.inline_types, expected.types);
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn cases() -> Vec<Case> {
    vec![
        Case {
            bytes: vec![],
            prefixes: vec![Prefix {
                cut: 0,
                projection: None,
                type_id: None,
                types: vec![],
            }],
            limits: Limits {
                bytes: 0,
                records: 0,
                nodes: 1,
                depth: 1,
                owned_bytes: 0,
                work: 1,
            },
            offsets: [0, 0, 0, 0, 0],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/empty_inline.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 2,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..2,
                        header: header(&[]),
                    }],
                },
            ],
            limits: Limits {
                bytes: 2,
                records: 1,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 6 + (S + T),
            },
            offsets: [0, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/pair0.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 4,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..4,
                        header: TypeHeader {
                            type_parameter: Some(0),
                            ..header(&[0x38, 0x00])
                        },
                    }],
                },
            ],
            limits: Limits {
                bytes: 4,
                records: 2,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 12 + (S + T),
            },
            offsets: [2, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/pair1.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 4,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..4,
                        header: TypeHeader {
                            type_parameter: Some(1),
                            ..header(&[0x38, 0x01])
                        },
                    }],
                },
            ],
            limits: Limits {
                bytes: 4,
                records: 2,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 12 + (S + T),
            },
            offsets: [2, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/selected.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 2,
                    projection: Some(Projection::Inv),
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 6,
                    projection: Some(Projection::Inv),
                    type_id: None,
                    types: vec![SelectedType {
                        span: 4..6,
                        header: TypeHeader {
                            class_name: Some(4),
                            ..header(&[0x30, 0x04])
                        },
                    }],
                },
                Prefix {
                    cut: 8,
                    projection: Some(Projection::Inv),
                    type_id: Some(0),
                    types: vec![SelectedType {
                        span: 4..6,
                        header: TypeHeader {
                            class_name: Some(4),
                            ..header(&[0x30, 0x04])
                        },
                    }],
                },
            ],
            limits: Limits {
                bytes: 8,
                records: 4,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 20 + (S + T),
            },
            offsets: [4, 4, 4, 4, 4],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/duplicates.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 2,
                    projection: Some(Projection::In),
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 4,
                    projection: Some(Projection::In),
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 8,
                    projection: Some(Projection::In),
                    type_id: None,
                    types: vec![SelectedType {
                        span: 6..8,
                        header: TypeHeader {
                            class_name: Some(4),
                            ..header(&[0x30, 0x04])
                        },
                    }],
                },
                Prefix {
                    cut: 12,
                    projection: Some(Projection::In),
                    type_id: None,
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                    ],
                },
                Prefix {
                    cut: 14,
                    projection: Some(Projection::In),
                    type_id: None,
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                        SelectedType {
                            span: 14..14,
                            header: header(&[]),
                        },
                    ],
                },
                Prefix {
                    cut: 16,
                    projection: Some(Projection::In),
                    type_id: Some(1),
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                        SelectedType {
                            span: 14..14,
                            header: header(&[]),
                        },
                    ],
                },
                Prefix {
                    cut: 22,
                    projection: Some(Projection::In),
                    type_id: Some(-1),
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                        SelectedType {
                            span: 14..14,
                            header: header(&[]),
                        },
                    ],
                },
                Prefix {
                    cut: 24,
                    projection: Some(Projection::Star),
                    type_id: Some(-1),
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                        SelectedType {
                            span: 14..14,
                            header: header(&[]),
                        },
                    ],
                },
                Prefix {
                    cut: 30,
                    projection: Some(Projection::Star),
                    type_id: Some(-1),
                    types: vec![
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                class_name: Some(4),
                                ..header(&[0x30, 0x04])
                            },
                        },
                        SelectedType {
                            span: 10..12,
                            header: TypeHeader {
                                nullable: Some(true),
                                ..header(&[0x18, 0x01])
                            },
                        },
                        SelectedType {
                            span: 14..14,
                            header: header(&[]),
                        },
                    ],
                },
            ],
            limits: Limits {
                bytes: 30,
                records: 11,
                nodes: 4,
                depth: 2,
                owned_bytes: 4 * (S + T),
                work: 60 + 4 * (S + T),
            },
            offsets: [10, 14, 6, 14, 14],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/repeated.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 4,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..4,
                        header: TypeHeader {
                            type_parameter: Some(0),
                            ..header(&[0x38, 0x00])
                        },
                    }],
                },
                Prefix {
                    cut: 8,
                    projection: None,
                    type_id: None,
                    types: vec![
                        SelectedType {
                            span: 2..4,
                            header: TypeHeader {
                                type_parameter: Some(0),
                                ..header(&[0x38, 0x00])
                            },
                        },
                        SelectedType {
                            span: 6..8,
                            header: TypeHeader {
                                type_parameter: Some(1),
                                ..header(&[0x38, 0x01])
                            },
                        },
                    ],
                },
            ],
            limits: Limits {
                bytes: 8,
                records: 4,
                nodes: 3,
                depth: 2,
                owned_bytes: 2 * (S + T),
                work: 23 + 2 * (S + T),
            },
            offsets: [6, 6, 2, 6, 6],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/wrong_wire.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 3,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 5,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 8,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
            ],
            limits: Limits {
                bytes: 8,
                records: 3,
                nodes: 1,
                depth: 1,
                owned_bytes: 0,
                work: 15,
            },
            offsets: [5, 0, 0, 0, 5],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/opaque_nested.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 6,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..6,
                        header: header(&[0x12, 0x02, 0x00, 0x80]),
                    }],
                },
            ],
            limits: Limits {
                bytes: 6,
                records: 2,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 16 + (S + T),
            },
            offsets: [2, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/opaque_extension.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 7,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..7,
                        header: header(&[0xa2, 0x06, 0x02, 0x00, 0x80]),
                    }],
                },
            ],
            limits: Limits {
                bytes: 7,
                records: 2,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 18 + (S + T),
            },
            offsets: [2, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/child_wrong_wire.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 5,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 2..5,
                        header: header(&[0x3a, 0x01, 0x80]),
                    }],
                },
            ],
            limits: Limits {
                bytes: 5,
                records: 2,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 14 + (S + T),
            },
            offsets: [2, 2, 2, 2, 2],
        },
        Case {
            bytes: hex(include_str!(
                "fixtures/metadata_type_argument_types/nonminimal_empty.hex"
            )),
            prefixes: vec![
                Prefix {
                    cut: 0,
                    projection: None,
                    type_id: None,
                    types: vec![],
                },
                Prefix {
                    cut: 4,
                    projection: None,
                    type_id: None,
                    types: vec![SelectedType {
                        span: 4..4,
                        header: header(&[]),
                    }],
                },
            ],
            limits: Limits {
                bytes: 4,
                records: 1,
                nodes: 2,
                depth: 2,
                owned_bytes: (S + T),
                work: 8 + (S + T),
            },
            offsets: [0, 4, 4, 4, 4],
        },
    ]
}

#[test]
fn frozen_literals_every_prefix_and_exact_minima() {
    let mut cuts = 0;
    for case in cases() {
        let expected = case.prefixes.last().unwrap();
        check(&case.bytes, case.limits, expected);
        let l = case.limits;
        check(
            &case.bytes,
            Limits {
                bytes: l.bytes + 1,
                records: l.records + 1,
                nodes: l.nodes + 1,
                depth: l.depth + 1,
                work: l.work + 1,
                owned_bytes: l.owned_bytes + 1,
            },
            expected,
        );
        for cut in 0..=case.bytes.len() {
            cuts += 1;
            match case.prefixes.iter().find(|p| p.cut == cut) {
                Some(expected) => check(&case.bytes[..cut], Limits::default(), expected),
                None => error(
                    &case.bytes[..cut],
                    Limits::default(),
                    ErrorKind::Truncated,
                    cut,
                ),
            }
        }
        for (limits, label, offset) in [
            (
                Limits {
                    nodes: l.nodes - 1,
                    ..l
                },
                "nodes",
                case.offsets[1],
            ),
            (
                Limits {
                    depth: l.depth - 1,
                    ..l
                },
                "depth",
                case.offsets[2],
            ),
            (
                Limits {
                    work: l.work - 1,
                    ..l
                },
                "work",
                case.offsets[4],
            ),
        ] {
            error(&case.bytes, limits, ErrorKind::Limit(label), offset);
        }
        if l.bytes > 0 {
            error(
                &case.bytes,
                Limits {
                    bytes: l.bytes - 1,
                    ..l
                },
                ErrorKind::Limit("bytes"),
                0,
            );
        }
        if l.records > 0 {
            error(
                &case.bytes,
                Limits {
                    records: l.records - 1,
                    ..l
                },
                ErrorKind::Limit("records"),
                case.offsets[0],
            );
        }
        if l.owned_bytes > 0 {
            error(
                &case.bytes,
                Limits {
                    owned_bytes: l.owned_bytes - 1,
                    ..l
                },
                ErrorKind::Limit("owned_bytes"),
                case.offsets[3],
            );
        }
    }
    assert_eq!(cuts, 98);
}

#[test]
fn malformed_offsets_and_parent_validation_first() {
    for (text, kind, offset) in [
        ("12 01 80", ErrorKind::Truncated, 3),
        ("12 01 00", ErrorKind::InvalidFieldNumber, 2),
        ("12 01 0b", ErrorKind::UnsupportedGroup, 2),
        ("12 01 0e", ErrorKind::InvalidWireType, 2),
        ("12 01 80 00", ErrorKind::InvalidFieldNumber, 3),
        ("12 01 80 08", ErrorKind::Truncated, 4),
        ("12 00 12 01 80", ErrorKind::Truncated, 5),
        ("92 00 81 00 80", ErrorKind::Truncated, 5),
        ("12 02 38 00 12 02 08 80", ErrorKind::Truncated, 8),
        ("12 06 12 80 80 80 80 08", ErrorKind::NegativeLength, 3),
        (
            "12 0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            11,
        ),
        ("12 02 ff", ErrorKind::Truncated, 3),
        ("12 80 80 80 80 08", ErrorKind::NegativeLength, 1),
    ] {
        error(&hex(text), Limits::default(), kind, offset);
    }
    for (text, kind, offset) in [
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("03", ErrorKind::InvalidFieldNumber, 0),
        ("07", ErrorKind::InvalidFieldNumber, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0c", ErrorKind::UnsupportedGroup, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
        ("09 01 02 03 04 05 06 07", ErrorKind::Truncated, 8),
        ("0d 01 02 03", ErrorKind::Truncated, 4),
        (
            "80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            9,
        ),
    ] {
        let body = hex(text);
        error(&body, Limits::default(), kind, offset);
        let parent = [&hex("12 02 38 00")[..], &body].concat();
        error(&parent, Limits::default(), kind, offset + 4);
        error(&wrap(&body), Limits::default(), kind, offset + 2);
    }
    error(
        &hex("12 01 80 00"),
        Limits {
            owned_bytes: S,
            nodes: 1,
            depth: 1,
            ..Default::default()
        },
        ErrorKind::InvalidFieldNumber,
        3,
    );
}

fn wrap(body: &[u8]) -> Vec<u8> {
    assert!(body.len() < 128);
    [&[0x12, body.len() as u8][..], body].concat()
}

#[test]
fn presence_signed_scalars_and_boolean_rules() {
    for (value, expected) in [
        ("00", 0),
        ("80 00", 0),
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 80 80 80 10", 0),
    ] {
        let value = hex(value);
        let parent = [&[0x18][..], &value].concat();
        let out = decode(&parent, Limits::default()).unwrap();
        retained(&out, &parent);
        assert_eq!(out.type_id, Some(expected));
        assert_eq!(out.projection, None);
        assert!(out.inline_types.is_empty());
        for tag in [0x08, 0x20, 0x30, 0x38, 0x40, 0x48, 0x58, 0x60, 0x70] {
            let raw = [&[tag][..], &value].concat();
            let bytes = wrap(&raw);
            let out = decode(&bytes, Limits::default()).unwrap();
            retained(&out, &bytes);
            assert_eq!((out.projection, out.type_id), (None, None));
            let mut expected_header = header(&raw);
            match tag {
                0x08 => expected_header.flags = Some(expected),
                0x20 => expected_header.flexible_type_capabilities_id = Some(expected),
                0x30 => expected_header.class_name = Some(expected),
                0x38 => expected_header.type_parameter = Some(expected),
                0x40 => expected_header.flexible_upper_bound_id = Some(expected),
                0x48 => expected_header.type_parameter_name = Some(expected),
                0x58 => expected_header.outer_type_id = Some(expected),
                0x60 => expected_header.type_alias_name = Some(expected),
                0x70 => expected_header.abbreviated_type_id = Some(expected),
                _ => unreachable!(),
            }
            assert_eq!(
                out.inline_types,
                [SelectedType {
                    span: 2..bytes.len(),
                    header: expected_header
                }]
            );
        }
    }
    for (value, expected) in [
        ("00", false),
        ("80 00", false),
        ("01", true),
        ("02", true),
        ("80 80 80 80 10", true),
    ] {
        let raw = [&[0x18][..], &hex(value)].concat();
        let bytes = wrap(&raw);
        let out = decode(&bytes, Limits::default()).unwrap();
        retained(&out, &bytes);
        assert_eq!(
            out.inline_types[0].header,
            TypeHeader {
                nullable: Some(expected),
                ..header(&raw)
            }
        );
    }
    for (value, projection) in [
        (0, Projection::In),
        (1, Projection::Out),
        (2, Projection::Inv),
        (3, Projection::Star),
    ] {
        for unknown in ["04", "63", "ff ff ff ff 0f", "80 80 80 80 08"] {
            let unknown = [&[0x08][..], &hex(unknown)].concat();
            for bytes in [
                [&[8, value][..], &unknown, &[8, 2], &unknown].concat(),
                [&unknown[..], &[8, value], &unknown].concat(),
            ] {
                let out = decode(&bytes, Limits::default()).unwrap();
                retained(&out, &bytes);
                let expected = if bytes.starts_with(&[8, value]) {
                    Projection::Inv
                } else {
                    projection
                };
                assert_eq!(out.projection, Some(expected));
            }
        }
    }
    for text in ["08 02", "08 03 12 00 18 00"] {
        let bytes = hex(text);
        let out = decode(&bytes, Limits::default()).unwrap();
        retained(&out, &bytes);
        assert_eq!(
            out.projection,
            Some(if bytes[1] == 2 {
                Projection::Inv
            } else {
                Projection::Star
            })
        );
    }
    let bytes = hex("12 06 38 01 38 00 18 00 12 02 38 02");
    let out = decode(&bytes, Limits::default()).unwrap();
    retained(&out, &bytes);
    assert_eq!(
        out.inline_types[0].header,
        TypeHeader {
            type_parameter: Some(0),
            nullable: Some(false),
            ..header(&bytes[2..8])
        }
    );
    assert_eq!(
        out.inline_types[1].header,
        TypeHeader {
            type_parameter: Some(2),
            ..header(&bytes[10..12])
        }
    );
}

fn wrong_wires(number: u8, recognized_wire: u8) -> Vec<Vec<u8>> {
    [0, 1, 2, 5]
        .into_iter()
        .filter(|&wire| wire != recognized_wire)
        .map(|wire| {
            let payload = match wire {
                0 => vec![1],
                1 => vec![1, 2, 3, 4, 5, 6, 7, 8],
                2 => vec![1, 0x80],
                5 => vec![1, 2, 3, 4],
                _ => unreachable!(),
            };
            [&[number * 8 + wire][..], &payload].concat()
        })
        .collect()
}

#[test]
fn wrong_wires_never_set_or_erase_selected_presence() {
    for number in [1, 2, 3] {
        let known = match number {
            1 => hex("08 02"),
            2 => hex("12 02 38 00"),
            3 => hex("18 00"),
            _ => unreachable!(),
        };
        for wrong in wrong_wires(number, if number == 2 { 2 } else { 0 }) {
            let ignored = decode(&wrong, Limits::default()).unwrap();
            retained(&ignored, &wrong);
            assert_eq!((ignored.projection, ignored.type_id), (None, None));
            assert!(ignored.inline_types.is_empty());
            for bytes in [[&wrong[..], &known].concat(), [&known[..], &wrong].concat()] {
                let out = decode(&bytes, Limits::default()).unwrap();
                retained(&out, &bytes);
                assert_eq!(
                    out.projection,
                    if number == 1 {
                        Some(Projection::Inv)
                    } else {
                        None
                    }
                );
                assert_eq!(out.type_id, if number == 3 { Some(0) } else { None });
                assert_eq!(out.inline_types.len(), usize::from(number == 2));
                if number == 2 {
                    assert_eq!(out.inline_types[0].header.type_parameter, Some(0));
                }
            }
        }
    }
    for number in [1, 3, 4, 6, 7, 8, 9, 11, 12, 14] {
        let known = [number * 8, 0];
        for wrong in wrong_wires(number, 0) {
            let bytes = wrap(&wrong);
            let ignored = decode(&bytes, Limits::default()).unwrap();
            retained(&ignored, &bytes);
            assert_eq!(ignored.inline_types[0].header, header(&wrong));
            for raw in [[&wrong[..], &known].concat(), [&known[..], &wrong].concat()] {
                let bytes = wrap(&raw);
                let out = decode(&bytes, Limits::default()).unwrap();
                retained(&out, &bytes);
                let mut expected = header(&raw);
                match number {
                    1 => expected.flags = Some(0),
                    3 => expected.nullable = Some(false),
                    4 => expected.flexible_type_capabilities_id = Some(0),
                    6 => expected.class_name = Some(0),
                    7 => expected.type_parameter = Some(0),
                    8 => expected.flexible_upper_bound_id = Some(0),
                    9 => expected.type_parameter_name = Some(0),
                    11 => expected.outer_type_id = Some(0),
                    12 => expected.type_alias_name = Some(0),
                    14 => expected.abbreviated_type_id = Some(0),
                    _ => unreachable!(),
                }
                assert_eq!(out.inline_types[0].header, expected);
            }
        }
    }
}

#[test]
fn grandchildren_extensions_and_unknown_fields_stay_opaque() {
    for tag in ["12", "2a", "52", "6a", "a2 06", "aa 06", "ba 0c"] {
        for body in [0, 0x80, 0x0b] {
            let raw = [&hex(tag)[..], &[1, body]].concat();
            let bytes = wrap(&raw);
            let out = decode(
                &bytes,
                Limits {
                    bytes: bytes.len(),
                    records: 2,
                    nodes: 2,
                    depth: 2,
                    owned_bytes: S + T,
                    work: 8 + 2 * raw.len() + S + T,
                },
            )
            .unwrap();
            retained(&out, &bytes);
            assert_eq!(out.inline_types[0].header, header(&raw));
        }
    }
    for text in [
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
    ] {
        let raw = hex(text);
        for bytes in [raw.clone(), [&hex("08 02 18 00")[..], &raw].concat()] {
            let out = decode(&bytes, Limits::default()).unwrap();
            retained(&out, &bytes);
            assert!(out.inline_types.is_empty());
            assert_eq!(
                out.projection,
                if bytes == raw {
                    None
                } else {
                    Some(Projection::Inv)
                }
            );
            assert_eq!(out.type_id, if bytes == raw { None } else { Some(0) });
        }
        let bytes = wrap(&raw);
        let out = decode(&bytes, Limits::default()).unwrap();
        retained(&out, &bytes);
        assert_eq!(out.inline_types[0].header, header(&raw));
    }
    for text in ["98 00 80 00", "98 80 80 80 10 00"] {
        let bytes = hex(text);
        let out = decode(&bytes, Limits::default()).unwrap();
        retained(&out, &bytes);
        assert_eq!(out.type_id, Some(0));
    }
}

#[test]
fn entry_and_empty_child_charge_order() {
    let zero = Limits {
        bytes: 0,
        records: 0,
        nodes: 0,
        depth: 0,
        work: 0,
        owned_bytes: 0,
    };
    error(&[], zero, ErrorKind::Limit("depth"), 0);
    error(
        &[],
        Limits { depth: 1, ..zero },
        ErrorKind::Limit("nodes"),
        0,
    );
    error(
        &[],
        Limits {
            depth: 1,
            nodes: 1,
            ..zero
        },
        ErrorKind::Limit("work"),
        0,
    );
    assert!(
        decode(
            &[],
            Limits {
                depth: 1,
                nodes: 1,
                work: 1,
                ..zero
            }
        )
        .is_ok()
    );
    let bytes = hex("12 00");
    for (work, offset) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 1),
        (4, 0),
        (5, 0),
        (5 + S, 2),
        (6 + S, 2),
    ] {
        error(
            &bytes,
            Limits {
                work,
                ..Default::default()
            },
            ErrorKind::Limit("work"),
            offset,
        );
    }
    for (limits, label, offset) in [
        (
            Limits {
                owned_bytes: S - 1,
                work: 5,
                ..Default::default()
            },
            "owned_bytes",
            0,
        ),
        (
            Limits {
                owned_bytes: S,
                work: 6 + S,
                ..Default::default()
            },
            "owned_bytes",
            2,
        ),
        (
            Limits {
                records: 0,
                depth: 1,
                ..Default::default()
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 1,
                depth: 1,
                ..Default::default()
            },
            "depth",
            2,
        ),
        (
            Limits {
                records: 0,
                depth: 0,
                ..Default::default()
            },
            "depth",
            0,
        ),
        (zero, "bytes", 0),
    ] {
        error(&bytes, limits, ErrorKind::Limit(label), offset);
    }
}

#[test]
fn staging_and_result_growth_are_monotonic_and_never_refunded() {
    for (k, capacity, owned_offset, work_offset, staging_offset) in [
        (1, 1, 2, 2, 0),
        (2, 2, 4, 4, 2),
        (3, 4, 6, 6, 4),
        (4, 4, 6, 8, 4),
        (5, 8, 10, 10, 8),
    ] {
        let bytes = [0x12, 0].repeat(k);
        let limits = Limits {
            bytes: 2 * k,
            records: k,
            nodes: k + 1,
            depth: 2,
            owned_bytes: capacity * (S + T),
            work: 1 + 5 * k + capacity * (S + T),
        };
        for extra in [0, 1] {
            let out = decode(
                &bytes,
                Limits {
                    bytes: limits.bytes + extra,
                    records: k + extra,
                    nodes: k + 1 + extra,
                    depth: 2 + extra,
                    work: limits.work + extra,
                    owned_bytes: limits.owned_bytes + extra,
                },
            )
            .unwrap();
            retained(&out, &bytes);
            assert_eq!((out.projection, out.type_id), (None, None));
            assert_eq!(out.inline_types.len(), k);
            for (i, body) in out.inline_types.iter().enumerate() {
                assert_eq!(body.span, 2 * i + 2..2 * i + 2);
                assert_eq!(body.header, header(&[]));
            }
        }
        for (l, label, offset) in [
            (
                Limits {
                    owned_bytes: limits.owned_bytes - 1,
                    ..limits
                },
                "owned_bytes",
                owned_offset,
            ),
            (
                Limits {
                    work: limits.work - 1,
                    ..limits
                },
                "work",
                work_offset,
            ),
            (
                Limits {
                    owned_bytes: capacity * S - 1,
                    nodes: 1,
                    ..limits
                },
                "owned_bytes",
                staging_offset,
            ),
            (
                Limits {
                    owned_bytes: capacity * S,
                    ..limits
                },
                "owned_bytes",
                2,
            ),
        ] {
            error(&bytes, l, ErrorKind::Limit(label), offset);
        }
        assert!(
            decode(
                &bytes,
                Limits {
                    owned_bytes: capacity * T,
                    ..limits
                }
            )
            .is_err()
        );
    }
}

#[test]
fn caller_limits_cannot_raise_hard_bounds() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    let bytes = hex("12 02 38 00");
    let out = decode(&bytes, unlimited).unwrap();
    retained(&out, &bytes);
    assert_eq!(out.inline_types[0].header.type_parameter, Some(0));
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [0x18, 0].repeat(65_535);
    let out = decode(&bytes, unlimited).unwrap();
    retained(&out, &bytes);
    assert_eq!(out.type_id, Some(0));
    assert!(out.inline_types.is_empty());
    assert!(
        decode(
            &bytes,
            Limits {
                bytes: 131070,
                records: 65535,
                nodes: 1,
                depth: 1,
                work: 262141,
                owned_bytes: 0
            }
        )
        .is_ok()
    );
    bytes.extend_from_slice(&[0x18, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), 131070);
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 36,
        records: 8,
        nodes: 8,
        depth: 2,
        work: 1024,
        owned_bytes: 1024,
    };
    let probe = |bytes: &[u8]| {
        let result = decode(bytes, limits);
        assert_eq!(result, decode(bytes, limits));
        match result {
            Ok(out) => retained(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            probe(&[a, b]);
            probe(&[0x12, 2, a, b]);
        }
        for n in 0..32 {
            probe(&vec![a; n]);
        }
    }
}
