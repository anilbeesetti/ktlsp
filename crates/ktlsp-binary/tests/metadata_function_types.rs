use ktlsp_binary::{
    MAX_BYTES,
    metadata::{
        self, class_header, function_header,
        function_types::{Error, ErrorKind, FunctionWithTypes, Limits, SelectedType, decode},
        payload,
        type_header::TypeHeader,
        wire::{self, Value},
    },
};
use std::{mem::size_of, ops::Range};

const S: usize = size_of::<SelectedType<'_>>();

struct Child {
    field: u32,
    span: Range<usize>,
    scalars: [Option<i32>; 10],
}
struct Case {
    text: &'static str,
    flags: &'static [(usize, i32)],
    old_flags: &'static [(usize, i32)],
    names: &'static [(usize, i32)],
    children: Vec<Child>,
    exact: Limits,
    deficits: [Option<usize>; 5],
    success: &'static [usize],
    missing: &'static [usize],
}

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

// Literal fields, budgets and offsets frozen before source; never decoder output.
fn cases() -> Vec<Case> {
    vec![
        Case {
            text: include_str!("fixtures/metadata_function_types/minimal.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![],
            exact: Limits {
                bytes: 2,
                records: 2,
                nodes: 1,
                depth: 1,
                work: 9,
                owned_bytes: 0,
            },
            deficits: [Some(0), Some(0), Some(0), Some(0), None],
            success: &[2],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/unit_function.hex"),
            flags: &[(10, 65558)],
            old_flags: &[],
            names: &[(2, 3)],
            children: vec![Child {
                field: 3,
                span: 4..6,
                scalars: [
                    None,
                    None,
                    None,
                    Some(4),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            }],
            exact: Limits {
                bytes: 10,
                records: 7,
                nodes: 2,
                depth: 2,
                work: 38 + S,
                owned_bytes: S,
            },
            deficits: [Some(6), Some(4), Some(4), Some(6), Some(2)],
            success: &[2, 6, 10],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/receiver.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![Child {
                field: 5,
                span: 4..8,
                scalars: [
                    None,
                    Some(1),
                    None,
                    Some(2),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            }],
            exact: Limits {
                bytes: 8,
                records: 6,
                nodes: 2,
                depth: 2,
                work: 34 + S,
                owned_bytes: S,
            },
            deficits: [Some(6), Some(4), Some(4), Some(2), Some(2)],
            success: &[2, 8],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/duplicates.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![
                Child {
                    field: 3,
                    span: 4..6,
                    scalars: [
                        None,
                        None,
                        None,
                        Some(1),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                },
                Child {
                    field: 5,
                    span: 8..8,
                    scalars: [None, None, None, None, None, None, None, None, None, None],
                },
                Child {
                    field: 3,
                    span: 10..12,
                    scalars: [
                        None,
                        Some(1),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                },
                Child {
                    field: 5,
                    span: 14..16,
                    scalars: [
                        None,
                        None,
                        None,
                        Some(2),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                },
            ],
            exact: Limits {
                bytes: 16,
                records: 13,
                nodes: 5,
                depth: 2,
                work: 69 + 4 * S,
                owned_bytes: 4 * S,
            },
            deficits: [Some(14), Some(14), Some(4), Some(14), Some(8)],
            success: &[2, 6, 8, 12, 16],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/scalars.hex"),
            flags: &[(6, 2), (18, 0)],
            old_flags: &[(2, 1), (10, 6)],
            names: &[(4, 1), (16, -1)],
            children: vec![
                Child {
                    field: 3,
                    span: 8..8,
                    scalars: [None, None, None, None, None, None, None, None, None, None],
                },
                Child {
                    field: 5,
                    span: 20..20,
                    scalars: [None, None, None, None, None, None, None, None, None, None],
                },
            ],
            exact: Limits {
                bytes: 20,
                records: 16,
                nodes: 3,
                depth: 2,
                work: 75 + 2 * S,
                owned_bytes: 2 * S,
            },
            deficits: [Some(18), Some(20), Some(8), Some(18), Some(18)],
            success: &[4, 6, 8, 10, 16, 18, 20],
            missing: &[0, 2],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/wrong_wire.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![
                Child {
                    field: 3,
                    span: 36..36,
                    scalars: [None, None, None, None, None, None, None, None, None, None],
                },
                Child {
                    field: 5,
                    span: 38..38,
                    scalars: [None, None, None, None, None, None, None, None, None, None],
                },
            ],
            exact: Limits {
                bytes: 38,
                records: 18,
                nodes: 3,
                depth: 2,
                work: 115 + 2 * S,
                owned_bytes: 2 * S,
            },
            deficits: [Some(36), Some(38), Some(36), Some(36), Some(36)],
            success: &[2, 4, 13, 18, 20, 29, 34, 36, 38],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/opaque.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![Child {
                field: 3,
                span: 4..16,
                scalars: [None, None, None, None, None, None, None, None, None, None],
            }],
            exact: Limits {
                bytes: 25,
                records: 14,
                nodes: 2,
                depth: 2,
                work: 92 + S,
                owned_bytes: S,
            },
            deficits: [Some(22), Some(4), Some(4), Some(22), Some(2)],
            success: &[2, 16, 19, 22, 25],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/nonminimal.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(4, 0)],
            children: vec![Child {
                field: 3,
                span: 8..12,
                scalars: [
                    None,
                    None,
                    None,
                    Some(4),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            }],
            exact: Limits {
                bytes: 12,
                records: 5,
                nodes: 2,
                depth: 2,
                work: 40 + S,
                owned_bytes: S,
            },
            deficits: [Some(8), Some(8), Some(8), Some(4), Some(4)],
            success: &[4, 12],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/ids.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![Child {
                field: 3,
                span: 8..8,
                scalars: [None, None, None, None, None, None, None, None, None, None],
            }],
            exact: Limits {
                bytes: 8,
                records: 8,
                nodes: 2,
                depth: 2,
                work: 34 + S,
                owned_bytes: S,
            },
            deficits: [Some(6), Some(8), Some(8), Some(6), Some(6)],
            success: &[2, 4, 6, 8],
            missing: &[0],
        },
        Case {
            text: include_str!("fixtures/metadata_function_types/conflicts.hex"),
            flags: &[],
            old_flags: &[],
            names: &[(2, 0)],
            children: vec![Child {
                field: 3,
                span: 4..12,
                scalars: [
                    None,
                    None,
                    None,
                    Some(1),
                    Some(2),
                    None,
                    Some(3),
                    None,
                    Some(4),
                    None,
                ],
            }],
            exact: Limits {
                bytes: 12,
                records: 8,
                nodes: 2,
                depth: 2,
                work: 50 + S,
                owned_bytes: S,
            },
            deficits: [Some(10), Some(4), Some(4), Some(2), Some(2)],
            success: &[2, 12],
            missing: &[0],
        },
    ]
}

fn scalars(h: &TypeHeader<'_>) -> [Option<i32>; 10] {
    [
        h.flags,
        h.nullable.map(i32::from),
        h.flexible_type_capabilities_id,
        h.class_name,
        h.type_parameter,
        h.flexible_upper_bound_id,
        h.type_parameter_name,
        h.outer_type_id,
        h.type_alias_name,
        h.abbreviated_type_id,
    ]
}

fn retained(out: &FunctionWithTypes<'_>, bytes: &[u8]) {
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for child in &out.types {
        assert!(end <= child.span.start && child.span.start <= child.span.end);
        assert!(child.span.end <= bytes.len());
        assert_eq!(child.header.raw, &bytes[child.span.clone()]);
        assert_eq!(
            child.header.raw.as_ptr(),
            bytes[child.span.clone()].as_ptr()
        );
        end = child.span.end;
    }
}

fn check(bytes: &[u8], limits: Limits, case: &Case) {
    let out = decode(bytes, limits).unwrap();
    retained(&out, bytes);
    let last = |values: &[(usize, i32)]| {
        values
            .iter()
            .rfind(|(end, _)| *end <= bytes.len())
            .map(|(_, v)| *v)
    };
    assert_eq!(out.header.flags, last(case.flags));
    assert_eq!(out.header.old_flags, last(case.old_flags));
    assert_eq!(out.header.name, last(case.names).unwrap());
    let children: Vec<_> = case
        .children
        .iter()
        .filter(|c| c.span.end <= bytes.len())
        .collect();
    assert_eq!(out.types.len(), children.len());
    for (actual, expected) in out.types.iter().zip(children) {
        assert_eq!(actual.field_number, expected.field);
        assert_eq!(actual.span, expected.span);
        assert_eq!(scalars(&actual.header), expected.scalars);
    }
}

fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn wire_error(bytes: &[u8], limits: Limits, kind: wire::ErrorKind, offset: usize) {
    error(bytes, limits, ErrorKind::Wire(kind), offset);
}

#[test]
fn frozen_literals_prefixes_and_exact_aggregate_limits() {
    for case in cases() {
        let bytes = hex(case.text);
        let l = case.exact;
        for limits in [
            l,
            Limits {
                bytes: l.bytes + 1,
                ..l
            },
            Limits {
                records: l.records + 1,
                ..l
            },
            Limits {
                nodes: l.nodes + 1,
                ..l
            },
            Limits {
                depth: l.depth + 1,
                ..l
            },
            Limits {
                work: l.work + 1,
                ..l
            },
            Limits {
                owned_bytes: l.owned_bytes + 1,
                ..l
            },
        ] {
            check(&bytes, limits, &case);
        }
        wire_error(
            &bytes,
            Limits {
                bytes: l.bytes - 1,
                ..l
            },
            wire::ErrorKind::Limit("bytes"),
            0,
        );
        for (limits, label, offset) in [
            (
                Limits {
                    records: l.records - 1,
                    ..l
                },
                "records",
                case.deficits[0],
            ),
            (
                Limits {
                    nodes: l.nodes - 1,
                    ..l
                },
                "nodes",
                case.deficits[1],
            ),
            (
                Limits {
                    depth: l.depth - 1,
                    ..l
                },
                "depth",
                case.deficits[2],
            ),
            (
                Limits {
                    work: l.work - 1,
                    ..l
                },
                "work",
                case.deficits[3],
            ),
            (
                Limits {
                    owned_bytes: l.owned_bytes.saturating_sub(1),
                    ..l
                },
                "owned_bytes",
                case.deficits[4],
            ),
        ] {
            if let Some(offset) = offset {
                wire_error(&bytes, limits, wire::ErrorKind::Limit(label), offset);
            }
        }
        for n in 0..=bytes.len() {
            if case.success.contains(&n) {
                check(&bytes[..n], Default::default(), &case);
            } else if case.missing.contains(&n) {
                error(&bytes[..n], Default::default(), ErrorKind::MissingName, n);
            } else {
                wire_error(
                    &bytes[..n],
                    Default::default(),
                    wire::ErrorKind::Truncated,
                    n,
                );
            }
        }
    }
}

#[test]
fn frozen_malformed_literals() {
    use ErrorKind::{MissingName, Wire};
    use wire::ErrorKind::*;
    for (text, kind, offset) in [
        ("", MissingName, 0),
        ("12 00", MissingName, 2),
        ("1a 01 00", MissingName, 3),
        ("12 00 1a 01 00", MissingName, 5),
        ("10 00 00", Wire(InvalidFieldNumber), 2),
        ("10 00 1b", Wire(UnsupportedGroup), 2),
        ("10 00 1e", Wire(InvalidWireType), 2),
        ("10 00 1a 02 30", Wire(Truncated), 5),
        ("10 00 1a ff ff ff ff 0f", Wire(NegativeLength), 3),
        ("10 00 1a 01 00", Wire(InvalidFieldNumber), 4),
        ("10 00 2a 01 0b", Wire(UnsupportedGroup), 4),
        ("10 00 2a 01 0e", Wire(InvalidWireType), 4),
        ("10 00 1a 01 30", Wire(Truncated), 5),
        ("10 00 1a 03 12 02 ff", Wire(Truncated), 7),
        ("10 00 1a 06 12 ff ff ff ff 0f", Wire(NegativeLength), 5),
        (
            "10 00 1a 0a 80 80 80 80 80 80 80 80 80 80",
            Wire(MalformedVarint),
            13,
        ),
        (
            "10 00 80 80 80 80 80 80 80 80 80 80",
            Wire(MalformedVarint),
            11,
        ),
        ("10 00 1a 01 00 00", Wire(InvalidFieldNumber), 5),
        ("1a 01 00 00", Wire(InvalidFieldNumber), 3),
        ("10 00 1a 00 2a 01 30", Wire(Truncated), 7),
        ("10 00 aa 00 81 00 30", Wire(Truncated), 7),
        ("1a 01 00 10 00", Wire(InvalidFieldNumber), 2),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
}

#[test]
fn resource_order_and_shared_growth() {
    use wire::ErrorKind::Limit;
    let b = hex("10 00 1a 00");
    let exact = Limits {
        bytes: 4,
        records: 4,
        nodes: 2,
        depth: 2,
        work: 18 + S,
        owned_bytes: S,
    };
    decode(&b, exact).unwrap();
    wire_error(
        &b,
        Limits {
            bytes: 3,
            records: 0,
            nodes: 0,
            depth: 0,
            work: 0,
            owned_bytes: 0,
        },
        Limit("bytes"),
        0,
    );
    for (limits, label, offset) in [
        (Limits { depth: 0, ..exact }, "depth", 0),
        (
            Limits {
                depth: 1,
                nodes: 0,
                ..exact
            },
            "nodes",
            0,
        ),
        (Limits { nodes: 1, ..exact }, "nodes", 4),
        (
            Limits {
                records: 0,
                ..exact
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 1,
                ..exact
            },
            "records",
            2,
        ),
        (
            Limits {
                records: 2,
                ..exact
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 3,
                ..exact
            },
            "records",
            2,
        ),
        (
            Limits {
                records: 3,
                depth: 1,
                ..exact
            },
            "records",
            2,
        ),
        (Limits { work: 18, ..exact }, "work", 2),
        (
            Limits {
                work: 18,
                owned_bytes: S - 1,
                ..exact
            },
            "owned_bytes",
            2,
        ),
    ] {
        wire_error(&b, limits, Limit(label), offset);
    }
    for (work, offset) in [0, 0, 0, 1, 0, 2, 2, 3, 2, 0, 0, 1, 0, 2, 2, 3, 2, 4]
        .into_iter()
        .enumerate()
    {
        wire_error(&b, Limits { work, ..exact }, Limit("work"), offset);
    }
    wire_error(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            nodes: 0,
            depth: 0,
            work: 0,
            owned_bytes: 0,
        },
        Limit("depth"),
        0,
    );
    error(&[], Default::default(), ErrorKind::MissingName, 0);
    for k in 1_usize..=5 {
        let mut b = vec![0x10, 0];
        for i in 0..k {
            b.extend_from_slice(&[if i % 2 == 0 { 0x1a } else { 0x2a }, 0]);
        }
        let capacity = k.next_power_of_two();
        let exact = Limits {
            bytes: 2 + 2 * k,
            records: 2 + 2 * k,
            nodes: 1 + k,
            depth: 2,
            work: 9 + 9 * k + capacity * S,
            owned_bytes: capacity * S,
        };
        let out = decode(&b, exact).unwrap();
        retained(&out, &b);
        assert_eq!(out.types.len(), k);
        for (i, t) in out.types.iter().enumerate() {
            assert_eq!(t.field_number, if i % 2 == 0 { 3 } else { 5 });
            assert_eq!(t.span, 4 + 2 * i..4 + 2 * i);
        }
        wire_error(
            &b,
            Limits {
                owned_bytes: exact.owned_bytes - 1,
                ..exact
            },
            Limit("owned_bytes"),
            [2, 4, 6, 6, 10][k - 1],
        );
        wire_error(
            &b,
            Limits {
                work: exact.work - 1,
                ..exact
            },
            Limit("work"),
            [2, 4, 6, 10, 10][k - 1],
        );
        wire_error(
            &b,
            Limits {
                records: exact.records - 1,
                ..exact
            },
            Limit("records"),
            2 * k,
        );
        wire_error(
            &b,
            Limits {
                nodes: exact.nodes - 1,
                ..exact
            },
            Limit("nodes"),
            2 + 2 * k,
        );
    }
    wire_error(
        &hex("10 00 1a 00 2a 00 1a 00"),
        Limits {
            owned_bytes: 3 * S,
            ..Default::default()
        },
        Limit("owned_bytes"),
        6,
    );
    let malformed = hex("10 00 1a 01 00");
    wire_error(
        &malformed,
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        wire::ErrorKind::InvalidFieldNumber,
        4,
    );
    wire_error(
        &malformed,
        Limits {
            depth: 1,
            ..Default::default()
        },
        Limit("depth"),
        4,
    );
    wire_error(
        &hex("10 00 1a 01 00 00"),
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        wire::ErrorKind::InvalidFieldNumber,
        5,
    );
    error(
        &hex("1a 01 00"),
        Limits {
            owned_bytes: 0,
            depth: 1,
            ..Default::default()
        },
        ErrorKind::MissingName,
        3,
    );
}

#[test]
fn both_selectors_rebase_once_and_validate_parent_first() {
    use wire::ErrorKind::*;
    for tag in [0x1a, 0x2a] {
        for (child, kind, relative) in [
            ("00", InvalidFieldNumber, 0),
            ("0b", UnsupportedGroup, 0),
            ("0e", InvalidWireType, 0),
            ("30", Truncated, 1),
            ("12 02 ff", Truncated, 3),
            ("12 ff ff ff ff 0f", NegativeLength, 1),
            ("80 80 80 80 80 80 80 80 80 80", MalformedVarint, 9),
        ] {
            let child = hex(child);
            let mut b = vec![0x10, 0, tag, child.len() as u8];
            b.extend_from_slice(&child);
            wire_error(&b, Default::default(), kind, 4 + relative);
            let mut prefixed = hex("10 00 1a 00");
            prefixed.extend_from_slice(&b);
            wire_error(&prefixed, Default::default(), kind, 8 + relative);
            let mut nonminimal = vec![0x10, 0, tag | 0x80, 0, child.len() as u8 | 0x80, 0];
            nonminimal.extend_from_slice(&child);
            wire_error(&nonminimal, Default::default(), kind, 6 + relative);
            error(
                &b[2..],
                Default::default(),
                ErrorKind::MissingName,
                b.len() - 2,
            );
            let mut wrong_name = b.clone();
            wrong_name[0] = 0x12;
            error(
                &wrong_name,
                Default::default(),
                ErrorKind::MissingName,
                b.len(),
            );
            let end = b.len();
            b.push(0);
            wire_error(&b, Default::default(), InvalidFieldNumber, end);
            wire_error(&b[2..], Default::default(), InvalidFieldNumber, end - 2);
        }
        for wrong in [
            vec![tag - 2, 1],
            vec![tag - 1, 1, 2, 3, 4, 5, 6, 7, 8],
            vec![tag + 3, 1, 2, 3, 4],
        ] {
            for child in [vec![], hex("30 02")] {
                for before in [false, true] {
                    let mut b = hex("10 00");
                    if before {
                        b.extend_from_slice(&wrong);
                    }
                    let start = b.len() + 2;
                    b.extend_from_slice(&[tag, child.len() as u8]);
                    b.extend_from_slice(&child);
                    if !before {
                        b.extend_from_slice(&wrong);
                    }
                    let out = decode(&b, Default::default()).unwrap();
                    retained(&out, &b);
                    assert_eq!(out.types.len(), 1);
                    assert_eq!(out.types[0].field_number, u32::from(tag >> 3));
                    assert_eq!(out.types[0].span, start..start + child.len());
                    assert_eq!(
                        out.types[0].header.class_name,
                        if child.is_empty() { None } else { Some(2) }
                    );
                }
            }
        }
    }
}

#[test]
fn low32_aliases_signed_scalars_and_unmerged_duplicates() {
    use wire::ErrorKind::Limit;
    for first in [0x9a, 0xaa] {
        let mut b = hex("10 00 9a 80 80 80 10 80 80 80 80 10");
        b[2] = first;
        let l = Limits {
            bytes: 12,
            records: 4,
            nodes: 2,
            depth: 2,
            work: 34 + S,
            owned_bytes: S,
        };
        let out = decode(&b, l).unwrap();
        retained(&out, &b);
        assert_eq!(out.types.len(), 1);
        assert_eq!(out.types[0].field_number, if first == 0x9a { 3 } else { 5 });
        assert_eq!(out.types[0].span, 12..12);
        wire_error(&b, Limits { records: 3, ..l }, Limit("records"), 2);
        wire_error(
            &b,
            Limits {
                owned_bytes: S - 1,
                ..l
            },
            Limit("owned_bytes"),
            2,
        );
        wire_error(&b, Limits { work: 33 + S, ..l }, Limit("work"), 2);
    }
    for tag in [0x1a, 0x2a] {
        for (body, class, nullable) in [
            ("30 ff ff ff ff 0f", Some(-1), None),
            ("30 ff ff ff ff ff ff ff ff ff 01", Some(-1), None),
            ("30 80 80 80 80 08", Some(i32::MIN), None),
            ("30 ff ff ff ff 07", Some(i32::MAX), None),
            ("30 80 80 80 80 10", Some(0), None),
            ("18 80 80 80 80 10", None, Some(true)),
            ("18 80 80 80 80 80 80 80 80 80 02", None, Some(false)),
            ("30 01 30 02 18 00 18 01", Some(2), Some(true)),
        ] {
            let body = hex(body);
            let mut b = vec![0x10, 0, tag, body.len() as u8];
            b.extend_from_slice(&body);
            b.extend_from_slice(&[tag, 0]);
            let out = decode(&b, Default::default()).unwrap();
            retained(&out, &b);
            assert_eq!(out.types.len(), 2);
            assert_eq!(out.types[0].header.class_name, class);
            assert_eq!(out.types[0].header.nullable, nullable);
            assert_eq!(scalars(&out.types[1].header), [None; 10]);
        }
    }
}

#[test]
fn deterministic_one_byte_hostile_children_and_healthy_followup() {
    use wire::ErrorKind::*;
    let healthy = hex(include_str!(
        "fixtures/metadata_function_types/unit_function.hex"
    ));
    for tag in [0x1a, 0x2a] {
        for b in 0..=u8::MAX {
            let (kind, offset) = if b >= 128 {
                (Truncated, 5)
            } else if b >> 3 == 0 {
                (InvalidFieldNumber, 4)
            } else {
                match b & 7 {
                    3 | 4 => (UnsupportedGroup, 4),
                    6 | 7 => (InvalidWireType, 4),
                    _ => (Truncated, 5),
                }
            };
            wire_error(&[0x10, 0, tag, 1, b], Default::default(), kind, offset);
            let out = decode(&healthy, Default::default()).unwrap();
            retained(&out, &healthy);
            assert_eq!(out.types.len(), 1);
            assert_eq!(out.types[0].header.class_name, Some(4));
        }
    }
}

#[test]
fn caller_limits_cannot_raise_hard_caps() {
    use wire::ErrorKind::Limit;
    let huge = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for c in cases() {
        check(&hex(c.text), huge, &c);
    }
    wire_error(
        &hex("10 00"),
        Limits { depth: 0, ..huge },
        Limit("depth"),
        0,
    );
    wire_error(
        &hex("10 00"),
        Limits { nodes: 0, ..huge },
        Limit("nodes"),
        0,
    );
    let mut b = hex("10 00");
    for _ in 0..32766 {
        b.extend_from_slice(&[0x18, 0]);
    }
    decode(&b, huge).unwrap();
    b.extend_from_slice(&[0x18, 0]);
    wire_error(&b, huge, Limit("records"), 65534);
    // Unselected field6 body is borrowed/skipped; bytes ceiling is checked once.
    let mut b = hex("10 00 32 f8 ff ff 07");
    b.resize(MAX_BYTES - 1, 0);
    let out = decode(&b, huge).unwrap();
    retained(&out, &b);
    assert!(out.types.is_empty());
    b.extend_from_slice(&[0x18, 0]);
    wire_error(&b, huge, Limit("bytes"), 0);
}

#[test]
fn genuine_unit_function_types_use_actual_pipeline_and_one_budget() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload_span, 19..52);
    let selected_class = class_header::decode(framed.payload, Default::default()).unwrap();
    assert_eq!(
        (selected_class.flags, selected_class.fq_name),
        (Some(326), 0)
    );
    assert_eq!(selected_class.raw, &bytes[19..52]);
    assert_eq!(selected_class.raw.as_ptr(), bytes[19..52].as_ptr());
    let fields = wire::decode(&bytes, Default::default()).unwrap();
    let mut functions = fields.payload.fields.iter().filter(|f| f.number == 9);
    let function = functions.next().unwrap();
    assert!(functions.next().is_none());
    assert_eq!(function.span, 37..49);
    let Value::Bytes(body) = function.value else {
        panic!("Function bytes");
    };
    assert_eq!(
        body,
        hex(include_str!(
            "fixtures/metadata_function_types/unit_function.hex"
        ))
    );
    assert_eq!(body.as_ptr(), selected_class.raw[20..30].as_ptr());
    let l = Limits {
        bytes: 10,
        records: 7,
        nodes: 2,
        depth: 2,
        work: 38 + S,
        owned_bytes: S,
    };
    let out = decode(body, l).unwrap();
    retained(&out, body);
    assert_eq!(
        (out.header.name, out.header.flags, out.header.old_flags),
        (3, Some(65558), None)
    );
    assert_eq!(out.header.raw.as_ptr(), bytes[39..49].as_ptr());
    assert_eq!(out.types.len(), 1);
    assert_eq!(out.types[0].field_number, 3);
    assert_eq!(out.types[0].span, 4..6);
    assert_eq!(out.types[0].header.raw, &[0x30, 4]);
    assert_eq!(out.types[0].header.raw.as_ptr(), bytes[43..45].as_ptr());
    assert_eq!(
        scalars(&out.types[0].header),
        [
            None,
            None,
            None,
            Some(4),
            None,
            None,
            None,
            None,
            None,
            None
        ]
    );
    function_header::decode(
        body,
        function_header::Limits {
            bytes: 10,
            records: 3,
            work: 16,
        },
    )
    .unwrap();
    wire_error(
        body,
        Limits { records: 3, ..l },
        wire::ErrorKind::Limit("records"),
        0,
    );
    wire_error(
        body,
        Limits { records: 6, ..l },
        wire::ErrorKind::Limit("records"),
        6,
    );
    wire_error(
        body,
        Limits { work: 37 + S, ..l },
        wire::ErrorKind::Limit("work"),
        6,
    );
    wire_error(
        body,
        Limits {
            owned_bytes: S - 1,
            ..l
        },
        wire::ErrorKind::Limit("owned_bytes"),
        2,
    );
}
