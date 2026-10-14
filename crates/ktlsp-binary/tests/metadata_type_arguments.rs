use ktlsp_binary::metadata::{
    type_argument::{self, InlineType, Projection, TypeArgument},
    type_arguments::{Error, ErrorKind, Limits, SelectedArgument, TypeWithArguments, decode},
    type_header::{self, TypeHeader},
};
use std::mem::size_of;

const A: usize = size_of::<SelectedArgument<'_>>();
const S: usize = size_of::<InlineType<'_>>();

struct Case {
    text: &'static str,
    flags: Option<(usize, i32)>,
    class_name: Option<(usize, i32)>,
    arguments: Vec<SelectedArgument<'static>>,
    exact: Limits,
    deficits: [usize; 5],
    successful_prefixes: &'static [usize],
}

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

// These literals, counts, prefixes and failure offsets were frozen before source.
fn cases() -> [Case; 9] {
    [
        Case {
            text: include_str!("fixtures/metadata_type_arguments/selected.hex"),
            flags: Some((2, 1)),
            class_name: Some((14, 2)),
            arguments: vec![SelectedArgument {
                span: 4..12,
                argument: TypeArgument {
                    raw: &[0x08, 0x01, 0x12, 0x02, 0x38, 0x00, 0x18, 0x7f],
                    projection: Some(Projection::Out),
                    type_id: Some(127),
                    inline_types: vec![InlineType {
                        raw: &[0x38, 0x00],
                        span: 4..6,
                    }],
                },
            }],
            exact: Limits {
                bytes: 14,
                records: 9,
                nodes: 2,
                depth: 2,
                work: 56 + A + S,
                owned_bytes: A + S,
            },
            deficits: [12, 4, 4, 12, 2],
            successful_prefixes: &[0, 2, 12, 14],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/duplicates.hex"),
            flags: None,
            class_name: Some((22, 1)),
            arguments: vec![
                SelectedArgument {
                    span: 2..12,
                    argument: TypeArgument {
                        raw: &[0x08, 0x00, 0x08, 0x63, 0x12, 0x00, 0x12, 0x00, 0x18, 0x01],
                        projection: Some(Projection::In),
                        type_id: Some(1),
                        inline_types: vec![
                            InlineType {
                                raw: &[],
                                span: 6..6,
                            },
                            InlineType {
                                raw: &[],
                                span: 8..8,
                            },
                        ],
                    },
                },
                SelectedArgument {
                    span: 14..18,
                    argument: TypeArgument {
                        raw: &[0x08, 0x03, 0x18, 0x02],
                        projection: Some(Projection::Star),
                        type_id: Some(2),
                        inline_types: vec![],
                    },
                },
                SelectedArgument {
                    span: 20..20,
                    argument: TypeArgument {
                        raw: &[],
                        projection: None,
                        type_id: None,
                        inline_types: vec![],
                    },
                },
            ],
            exact: Limits {
                bytes: 22,
                records: 15,
                nodes: 4,
                depth: 2,
                work: 92 + 4 * A + 2 * S,
                owned_bytes: 4 * A + 2 * S,
            },
            deficits: [20, 20, 2, 20, 18],
            successful_prefixes: &[0, 12, 18, 20, 22],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/opaque.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 2..4,
                argument: TypeArgument {
                    raw: &[0x12, 0x00],
                    projection: None,
                    type_id: None,
                    inline_types: vec![InlineType {
                        raw: &[],
                        span: 2..2,
                    }],
                },
            }],
            exact: Limits {
                bytes: 15,
                records: 11,
                nodes: 2,
                depth: 2,
                work: 56 + A + S,
                owned_bytes: A + S,
            },
            deficits: [13, 2, 2, 13, 0],
            successful_prefixes: &[0, 4, 7, 10, 13, 15],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/enum.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 2..14,
                argument: TypeArgument {
                    raw: &[
                        0x08, 0x01, 0x08, 0xff, 0xff, 0xff, 0xff, 0x0f, 0x08, 0x63, 0x18, 0x00,
                    ],
                    projection: Some(Projection::Out),
                    type_id: Some(0),
                    inline_types: vec![],
                },
            }],
            exact: Limits {
                bytes: 14,
                records: 6,
                nodes: 2,
                depth: 2,
                work: 54 + A,
                owned_bytes: A,
            },
            deficits: [12, 2, 2, 0, 0],
            successful_prefixes: &[0, 14],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/signed.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 2..15,
                argument: TypeArgument {
                    raw: &[
                        0x18, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0x08,
                        0x03,
                    ],
                    projection: Some(Projection::Star),
                    type_id: Some(-1),
                    inline_types: vec![],
                },
            }],
            exact: Limits {
                bytes: 15,
                records: 4,
                nodes: 2,
                depth: 2,
                work: 53 + A,
                owned_bytes: A,
            },
            deficits: [13, 2, 2, 0, 0],
            successful_prefixes: &[0, 15],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/nonminimal.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 4..8,
                argument: TypeArgument {
                    raw: &[0x88, 0x00, 0x81, 0x00],
                    projection: Some(Projection::Out),
                    type_id: None,
                    inline_types: vec![],
                },
            }],
            exact: Limits {
                bytes: 8,
                records: 3,
                nodes: 2,
                depth: 2,
                work: 28 + A,
                owned_bytes: A,
            },
            deficits: [4, 4, 4, 0, 0],
            successful_prefixes: &[0, 8],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/wrong_wire.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 2..11,
                argument: TypeArgument {
                    raw: &[0x0a, 0x01, 0xff, 0x10, 0x00, 0x1a, 0x00, 0x20, 0x01],
                    projection: None,
                    type_id: None,
                    inline_types: vec![],
                },
            }],
            exact: Limits {
                bytes: 11,
                records: 6,
                nodes: 2,
                depth: 2,
                work: 45 + A,
                owned_bytes: A,
            },
            deficits: [9, 2, 2, 0, 0],
            successful_prefixes: &[0, 11],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/opaque_inline.hex"),
            flags: None,
            class_name: None,
            arguments: vec![SelectedArgument {
                span: 2..5,
                argument: TypeArgument {
                    raw: &[0x12, 0x01, 0x00],
                    projection: None,
                    type_id: None,
                    inline_types: vec![InlineType {
                        raw: &[0x00],
                        span: 2..3,
                    }],
                },
            }],
            exact: Limits {
                bytes: 5,
                records: 3,
                nodes: 2,
                depth: 2,
                work: 21 + A + S,
                owned_bytes: A + S,
            },
            deficits: [2, 2, 2, 0, 0],
            successful_prefixes: &[0, 5],
        },
        Case {
            text: include_str!("fixtures/metadata_type_arguments/pair_type.hex"),
            flags: None,
            class_name: Some((14, 0)),
            arguments: vec![
                SelectedArgument {
                    span: 2..6,
                    argument: TypeArgument {
                        raw: &[0x12, 0x02, 0x38, 0x00],
                        projection: None,
                        type_id: None,
                        inline_types: vec![InlineType {
                            raw: &[0x38, 0x00],
                            span: 2..4,
                        }],
                    },
                },
                SelectedArgument {
                    span: 8..12,
                    argument: TypeArgument {
                        raw: &[0x12, 0x02, 0x38, 0x01],
                        projection: None,
                        type_id: None,
                        inline_types: vec![InlineType {
                            raw: &[0x38, 0x01],
                            span: 2..4,
                        }],
                    },
                },
            ],
            exact: Limits {
                bytes: 14,
                records: 8,
                nodes: 3,
                depth: 2,
                work: 55 + 2 * A + 2 * S,
                owned_bytes: 2 * A + 2 * S,
            },
            deficits: [12, 8, 2, 12, 6],
            successful_prefixes: &[0, 6, 12, 14],
        },
    ]
}

fn header(bytes: &[u8], flags: Option<i32>, class_name: Option<i32>) -> TypeHeader<'_> {
    TypeHeader {
        raw: bytes,
        flags,
        nullable: None,
        flexible_type_capabilities_id: None,
        class_name,
        type_parameter: None,
        flexible_upper_bound_id: None,
        type_parameter_name: None,
        outer_type_id: None,
        type_alias_name: None,
        abbreviated_type_id: None,
    }
}

fn retained(out: &TypeWithArguments<'_>, bytes: &[u8]) {
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for selected in &out.arguments {
        assert!(end <= selected.span.start && selected.span.start <= selected.span.end);
        assert!(selected.span.end <= bytes.len());
        let raw = selected.argument.raw;
        assert_eq!(raw, &bytes[selected.span.clone()]);
        assert_eq!(raw.as_ptr(), bytes[selected.span.clone()].as_ptr());
        let mut inline_end = 0;
        for inline in &selected.argument.inline_types {
            assert!(inline_end <= inline.span.start && inline.span.start <= inline.span.end);
            assert!(inline.span.end <= raw.len());
            assert_eq!(inline.raw, &raw[inline.span.clone()]);
            assert_eq!(inline.raw.as_ptr(), raw[inline.span.clone()].as_ptr());
            inline_end = inline.span.end;
        }
        end = selected.span.end;
    }
}

fn check(bytes: &[u8], limits: Limits, case: &Case) {
    let out = decode(bytes, limits).unwrap();
    retained(&out, bytes);
    let present = |field: Option<(usize, i32)>| {
        field
            .filter(|(end, _)| *end <= bytes.len())
            .map(|(_, value)| value)
    };
    assert_eq!(
        out.header,
        header(bytes, present(case.flags), present(case.class_name))
    );
    let expected: Vec<_> = case
        .arguments
        .iter()
        .filter(|a| a.span.end <= bytes.len())
        .collect();
    assert_eq!(out.arguments.iter().collect::<Vec<_>>(), expected);
}

fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}

#[test]
fn frozen_fixtures_and_exact_aggregate_deficits() {
    for case in cases() {
        let bytes = hex(case.text);
        let exact = case.exact;
        for limits in [
            exact,
            Limits {
                bytes: exact.bytes + 1,
                ..exact
            },
            Limits {
                records: exact.records + 1,
                ..exact
            },
            Limits {
                nodes: exact.nodes + 1,
                ..exact
            },
            Limits {
                depth: exact.depth + 1,
                ..exact
            },
            Limits {
                work: exact.work + 1,
                ..exact
            },
            Limits {
                owned_bytes: exact.owned_bytes + 1,
                ..exact
            },
        ] {
            check(&bytes, limits, &case);
        }
        for (label, offset, limits) in [
            (
                "records",
                case.deficits[0],
                Limits {
                    records: exact.records - 1,
                    ..exact
                },
            ),
            (
                "nodes",
                case.deficits[1],
                Limits {
                    nodes: exact.nodes - 1,
                    ..exact
                },
            ),
            ("depth", case.deficits[2], Limits { depth: 1, ..exact }),
            (
                "work",
                case.deficits[3],
                Limits {
                    work: exact.work - 1,
                    ..exact
                },
            ),
            (
                "owned_bytes",
                case.deficits[4],
                Limits {
                    owned_bytes: exact.owned_bytes - 1,
                    ..exact
                },
            ),
        ] {
            error(&bytes, limits, ErrorKind::Limit(label), offset);
        }
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                depth: 0,
                nodes: 0,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            ErrorKind::Limit("bytes"),
            0,
        );
    }
}

#[test]
fn all_frozen_prefixes_and_malformed_arguments() {
    let mut prefixes = 0;
    for case in cases() {
        let bytes = hex(case.text);
        for end in 0..=bytes.len() {
            prefixes += 1;
            if case.successful_prefixes.contains(&end) {
                check(&bytes[..end], Default::default(), &case);
            } else {
                error(&bytes[..end], Default::default(), ErrorKind::Truncated, end);
            }
        }
    }
    assert_eq!(prefixes, 127);
    for (text, kind, offset) in [
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("23", ErrorKind::UnsupportedGroup, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("12 00 12 01 18", ErrorKind::Truncated, 5),
        ("92 00 81 00 18", ErrorKind::Truncated, 5),
        ("12 01 00", ErrorKind::InvalidFieldNumber, 2),
        ("12 01 18", ErrorKind::Truncated, 3),
        ("12 02 18", ErrorKind::Truncated, 3),
        ("12 01 00 00", ErrorKind::InvalidFieldNumber, 3),
        ("12 01 23", ErrorKind::UnsupportedGroup, 2),
        ("12 01 0e", ErrorKind::InvalidWireType, 2),
        ("12 03 12 02 ff", ErrorKind::Truncated, 5),
        ("12 06 12 ff ff ff ff 0f", ErrorKind::NegativeLength, 3),
        (
            "12 0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            11,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            9,
        ),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
}

#[test]
fn admission_nodes_and_empty_argument_charge_order() {
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
    let out = decode(
        &[],
        Limits {
            depth: 1,
            nodes: 1,
            work: 1,
            ..zero
        },
    )
    .unwrap();
    assert_eq!(out.header, header(&[], None, None));
    assert!(out.arguments.is_empty());
    let bytes = [0x12, 0];
    let exact = Limits {
        bytes: 2,
        records: 2,
        nodes: 2,
        depth: 2,
        work: 10 + A,
        owned_bytes: A,
    };
    for (work, offset) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 1),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 1),
        (8, 0),
        (9, 2),
        (10, 0),
    ] {
        error(
            &bytes,
            Limits { work, ..exact },
            ErrorKind::Limit("work"),
            offset,
        );
    }
    for records in [0, 1] {
        error(
            &bytes,
            Limits { records, ..exact },
            ErrorKind::Limit("records"),
            0,
        );
    }
    error(
        &bytes,
        Limits { bytes: 1, ..zero },
        ErrorKind::Limit("bytes"),
        0,
    );
    error(
        &bytes,
        Limits { depth: 1, ..exact },
        ErrorKind::Limit("depth"),
        2,
    );
    error(
        &bytes,
        Limits { nodes: 1, ..exact },
        ErrorKind::Limit("nodes"),
        2,
    );
    error(
        &bytes,
        Limits {
            records: 1,
            depth: 1,
            ..exact
        },
        ErrorKind::Limit("records"),
        0,
    );
    error(
        &bytes,
        Limits {
            records: 0,
            depth: 0,
            ..exact
        },
        ErrorKind::Limit("depth"),
        0,
    );
    error(
        &bytes,
        Limits {
            work: 10,
            owned_bytes: A - 1,
            ..exact
        },
        ErrorKind::Limit("owned_bytes"),
        0,
    );
    let out = decode(&bytes, exact).unwrap();
    retained(&out, &bytes);
    assert_eq!(out.header, header(&bytes, None, None));
    assert_eq!(
        out.arguments,
        vec![SelectedArgument {
            span: 2..2,
            argument: TypeArgument {
                raw: &bytes[2..2],
                projection: None,
                type_id: None,
                inline_types: Vec::new()
            },
        }]
    );
}

#[test]
fn child_storage_and_parent_validation_precedence() {
    let bytes = [0x12, 2, 0x12, 0];
    let exact = Limits {
        bytes: 4,
        records: 3,
        nodes: 2,
        depth: 2,
        work: 18 + A + S,
        owned_bytes: A + S,
    };
    for (owned_bytes, offset) in [(0, 2), (S, 0)] {
        error(
            &bytes,
            Limits {
                owned_bytes,
                ..exact
            },
            ErrorKind::Limit("owned_bytes"),
            offset,
        );
    }
    error(
        &bytes,
        Limits {
            work: 18,
            owned_bytes: S - 1,
            ..exact
        },
        ErrorKind::Limit("owned_bytes"),
        2,
    );
    error(
        &bytes,
        Limits {
            work: 17,
            owned_bytes: 0,
            ..exact
        },
        ErrorKind::Limit("work"),
        2,
    );
    error(
        &[0x12, 1, 0],
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        ErrorKind::InvalidFieldNumber,
        2,
    );
    error(
        &[0x12, 1, 0],
        Limits {
            depth: 1,
            owned_bytes: 0,
            ..Default::default()
        },
        ErrorKind::Limit("depth"),
        2,
    );
    error(
        &[0x12, 1, 0, 0],
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        ErrorKind::InvalidFieldNumber,
        3,
    );
}

#[test]
fn both_levels_share_retained_capacity_growth() {
    for (k, capacity, owned_offset, work_offset) in [
        (1, 1, 0, 0),
        (2, 2, 2, 2),
        (3, 4, 4, 4),
        (4, 4, 4, 8),
        (5, 8, 8, 8),
    ] {
        let bytes = [0x12, 0].repeat(k);
        let owned = capacity * A;
        let exact = Limits {
            bytes: 2 * k,
            records: 2 * k,
            nodes: 1 + k,
            depth: 2,
            work: 1 + 9 * k + owned,
            owned_bytes: owned,
        };
        let out = decode(&bytes, exact).unwrap();
        retained(&out, &bytes);
        assert_eq!(out.arguments.len(), k);
        for (i, selected) in out.arguments.iter().enumerate() {
            assert_eq!(selected.span, 2 * i + 2..2 * i + 2);
            assert_eq!(selected.argument.raw, []);
            assert_eq!(
                (selected.argument.projection, selected.argument.type_id),
                (None, None)
            );
            assert!(selected.argument.inline_types.is_empty());
        }
        error(
            &bytes,
            Limits {
                owned_bytes: owned - 1,
                ..exact
            },
            ErrorKind::Limit("owned_bytes"),
            owned_offset,
        );
        error(
            &bytes,
            Limits {
                work: exact.work - 1,
                ..exact
            },
            ErrorKind::Limit("work"),
            work_offset,
        );
        let out = decode(
            &bytes,
            Limits {
                work: exact.work + 1,
                owned_bytes: owned + 1,
                ..exact
            },
        )
        .unwrap();
        retained(&out, &bytes);
    }
    for (j, capacity, child_growth) in [(1, 1, 2), (2, 2, 4), (3, 4, 6), (4, 4, 6), (5, 8, 10)] {
        let mut bytes = vec![0x12, (2 * j) as u8];
        bytes.extend_from_slice(&[0x12, 0].repeat(j));
        let owned = A + capacity * S;
        let exact = Limits {
            bytes: bytes.len(),
            records: 2 + j,
            nodes: 2,
            depth: 2,
            work: 10 + 8 * j + owned,
            owned_bytes: owned,
        };
        let out = decode(&bytes, exact).unwrap();
        retained(&out, &bytes);
        assert_eq!(out.arguments.len(), 1);
        assert_eq!(out.arguments[0].span, 2..bytes.len());
        assert_eq!(out.arguments[0].argument.inline_types.len(), j);
        for (i, inline) in out.arguments[0].argument.inline_types.iter().enumerate() {
            assert_eq!(inline.span, 2 * i + 2..2 * i + 2);
            assert_eq!(inline.raw, []);
        }
        error(
            &bytes,
            Limits {
                owned_bytes: owned - 1,
                ..exact
            },
            ErrorKind::Limit("owned_bytes"),
            0,
        );
        error(
            &bytes,
            Limits {
                work: exact.work - 1,
                ..exact
            },
            ErrorKind::Limit("work"),
            0,
        );
        error(
            &bytes,
            Limits {
                owned_bytes: capacity * S - 1,
                ..exact
            },
            ErrorKind::Limit("owned_bytes"),
            child_growth,
        );
        let out = decode(
            &bytes,
            Limits {
                work: exact.work + 1,
                owned_bytes: owned + 1,
                ..exact
            },
        )
        .unwrap();
        retained(&out, &bytes);
    }
}

#[test]
fn aggregate_limits_do_not_reset_for_parent_or_children() {
    let bytes = hex(include_str!(
        "fixtures/metadata_type_arguments/pair_type.hex"
    ));
    let standalone_header = type_header::decode(
        &bytes,
        type_header::Limits {
            bytes: 14,
            records: 3,
            work: 20,
        },
    )
    .unwrap();
    assert_eq!(standalone_header, header(&bytes, None, Some(0)));
    for raw in [hex("12 02 38 00"), hex("12 02 38 01")] {
        let child = type_argument::decode(
            &raw,
            type_argument::Limits {
                bytes: 4,
                records: 1,
                work: 6 + S,
                owned_bytes: S,
            },
        )
        .unwrap();
        assert_eq!((child.projection, child.type_id), (None, None));
        assert_eq!(child.inline_types.len(), 1);
    }
    error(
        &bytes,
        Limits {
            records: 3,
            ..Default::default()
        },
        ErrorKind::Limit("records"),
        0,
    );
    let case = cases().into_iter().last().unwrap();
    check(&bytes, case.exact, &case);
}

#[test]
fn hard_limits_clamp_raised_caller_limits() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for case in cases() {
        check(&hex(case.text), unlimited, &case);
    }
    error(
        &vec![0; ktlsp_binary::MAX_BYTES + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let bytes = [0x10, 0].repeat(ktlsp_binary::MAX_RECORDS);
    let parent = type_header::decode(
        &bytes,
        type_header::Limits {
            bytes: usize::MAX,
            records: usize::MAX,
            work: usize::MAX,
        },
    )
    .unwrap();
    assert_eq!(parent, header(&bytes, None, None));
    error(&bytes, unlimited, ErrorKind::Limit("records"), 0);
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
