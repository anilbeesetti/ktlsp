use ktlsp_binary::metadata::{
    self, class_header,
    class_supertypes::{ClassWithSupertypes, Error, ErrorKind, Limits, SelectedSupertype, decode},
    type_header::{self, TypeHeader},
    wire::ErrorKind as WireKind,
};
use std::mem::size_of;

const S: usize = size_of::<SelectedSupertype<'_>>();
type Parent = (Option<i32>, i32);
type Child = (usize, usize, [Option<i32>; 10]);

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn scalars(header: &TypeHeader<'_>) -> [Option<i32>; 10] {
    [
        header.flags,
        header.nullable.map(i32::from),
        header.flexible_type_capabilities_id,
        header.class_name,
        header.type_parameter,
        header.flexible_upper_bound_id,
        header.type_parameter_name,
        header.outer_type_id,
        header.type_alias_name,
        header.abbreviated_type_id,
    ]
}
fn one(index: usize, value: i32) -> [Option<i32>; 10] {
    let mut out = [None; 10];
    out[index] = Some(value);
    out
}
fn check<'a>(
    bytes: &'a [u8],
    limits: Limits,
    parent: Parent,
    children: &[Child],
) -> ClassWithSupertypes<'a> {
    let out = decode(bytes, limits).unwrap();
    assert_eq!((out.header.flags, out.header.fq_name), parent);
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(out.supertypes.len(), children.len());
    for (actual, &(start, end, expected)) in out.supertypes.iter().zip(children) {
        assert_eq!(actual.span, start..end);
        assert_eq!(actual.header.raw, &bytes[start..end]);
        assert_eq!(actual.header.raw.as_ptr(), bytes[start..end].as_ptr());
        assert_eq!(scalars(&actual.header), expected);
    }
    out
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert!(offset <= bytes.len());
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
    let healthy = hex(include_str!(
        "fixtures/metadata_class_supertypes/unit_class.hex"
    ));
    check(
        &healthy,
        Limits::default(),
        (Some(326), 0),
        &[(7, 9, one(3, 1))],
    );
}
fn limited(bytes: &[u8], limits: Limits, label: &'static str, offset: usize) {
    error(
        bytes,
        limits,
        ErrorKind::Wire(WireKind::Limit(label)),
        offset,
    );
}
struct Fixture {
    text: &'static str,
    parent: Parent,
    children: Vec<Child>,
    exact: Limits,
    deficits: [usize; 4], // records, nodes, depth, work offsets
    storage: Option<usize>,
    prefixes: &'static [(usize, Parent)],
    missing: &'static [usize],
}
fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/minimal.hex"),
            parent: (None, 0),
            children: vec![],
            exact: Limits {
                bytes: 2,
                records: 2,
                nodes: 1,
                depth: 1,
                work: 9,
                owned_bytes: 0,
            },
            deficits: [0, 0, 0, 0],
            storage: None,
            prefixes: &[(2, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/empty.hex"),
            parent: (None, 0),
            children: vec![(4, 4, [None; 10])],
            exact: Limits {
                bytes: 4,
                records: 4,
                nodes: 2,
                depth: 2,
                work: 18 + S,
                owned_bytes: S,
            },
            deficits: [2, 4, 4, 2],
            storage: Some(2),
            prefixes: &[(2, (None, 0)), (4, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/unit_class.hex"),
            parent: (Some(326), 0),
            children: vec![(7, 9, one(3, 1))],
            exact: Limits {
                bytes: 33,
                records: 13,
                nodes: 2,
                depth: 2,
                work: 96 + S,
                owned_bytes: S,
            },
            deficits: [30, 7, 7, 30],
            storage: Some(5),
            prefixes: &[
                (5, (Some(326), 0)),
                (9, (Some(326), 0)),
                (18, (Some(326), 0)),
                (30, (Some(326), 0)),
                (33, (Some(326), 0)),
            ],
            missing: &[0, 3],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/duplicates.hex"),
            parent: (Some(7), 2),
            children: vec![(4, 6, one(3, 1)), (8, 8, [None; 10]), (10, 14, one(3, 3))],
            exact: Limits {
                bytes: 18,
                records: 15,
                nodes: 4,
                depth: 2,
                work: 76 + 4 * S,
                owned_bytes: 4 * S,
            },
            deficits: [16, 10, 4, 16],
            storage: Some(8),
            prefixes: &[
                (2, (None, 0)),
                (6, (None, 0)),
                (8, (None, 0)),
                (14, (None, 0)),
                (16, (Some(7), 0)),
                (18, (Some(7), 2)),
            ],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/scalars.hex"),
            parent: (None, 0),
            children: vec![(
                4,
                24,
                [
                    Some(0),
                    Some(0),
                    Some(1),
                    Some(4),
                    Some(2),
                    Some(3),
                    Some(5),
                    Some(6),
                    Some(7),
                    Some(8),
                ],
            )],
            exact: Limits {
                bytes: 24,
                records: 14,
                nodes: 2,
                depth: 2,
                work: 98 + S,
                owned_bytes: S,
            },
            deficits: [22, 4, 4, 2],
            storage: Some(2),
            prefixes: &[(2, (None, 0)), (24, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/type_duplicates.hex"),
            parent: (None, 0),
            children: vec![(
                4,
                15,
                [
                    None,
                    Some(0),
                    None,
                    Some(2),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            )],
            exact: Limits {
                bytes: 15,
                records: 9,
                nodes: 2,
                depth: 2,
                work: 61 + S,
                owned_bytes: S,
            },
            deficits: [12, 4, 4, 2],
            storage: Some(2),
            prefixes: &[(2, (None, 0)), (15, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/wrong_wire.hex"),
            parent: (None, 0),
            children: vec![(20, 20, [None; 10]), (22, 24, one(3, 1))],
            exact: Limits {
                bytes: 26,
                records: 15,
                nodes: 3,
                depth: 2,
                work: 87 + 2 * S,
                owned_bytes: 2 * S,
            },
            deficits: [24, 22, 20, 24],
            storage: Some(20),
            prefixes: &[
                (2, (None, 0)),
                (4, (None, 0)),
                (13, (None, 0)),
                (18, (None, 0)),
                (20, (None, 0)),
                (24, (None, 0)),
                (26, (None, 0)),
            ],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/ids.hex"),
            parent: (None, 0),
            children: vec![(11, 11, [None; 10])],
            exact: Limits {
                bytes: 11,
                records: 8,
                nodes: 2,
                depth: 2,
                work: 40 + S,
                owned_bytes: S,
            },
            deficits: [9, 11, 11, 9],
            storage: Some(9),
            prefixes: &[
                (2, (None, 0)),
                (4, (None, 0)),
                (9, (None, 0)),
                (11, (None, 0)),
            ],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/opaque.hex"),
            parent: (None, 0),
            children: vec![(4, 16, [None; 10])],
            exact: Limits {
                bytes: 25,
                records: 14,
                nodes: 2,
                depth: 2,
                work: 92 + S,
                owned_bytes: S,
            },
            deficits: [22, 4, 4, 22],
            storage: Some(2),
            prefixes: &[
                (2, (None, 0)),
                (16, (None, 0)),
                (19, (None, 0)),
                (22, (None, 0)),
                (25, (None, 0)),
            ],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/nonminimal.hex"),
            parent: (None, 0),
            children: vec![(8, 12, one(3, 1))],
            exact: Limits {
                bytes: 12,
                records: 5,
                nodes: 2,
                depth: 2,
                work: 40 + S,
                owned_bytes: S,
            },
            deficits: [8, 8, 8, 4],
            storage: Some(4),
            prefixes: &[(4, (None, 0)), (12, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/low32_alias.hex"),
            parent: (None, 0),
            children: vec![(12, 12, [None; 10])],
            exact: Limits {
                bytes: 12,
                records: 4,
                nodes: 2,
                depth: 2,
                work: 34 + S,
                owned_bytes: S,
            },
            deficits: [2, 12, 12, 2],
            storage: Some(2),
            prefixes: &[(2, (None, 0)), (12, (None, 0))],
            missing: &[0],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/late_fq.hex"),
            parent: (None, 0),
            children: vec![(2, 4, one(3, 1))],
            exact: Limits {
                bytes: 6,
                records: 5,
                nodes: 2,
                depth: 2,
                work: 26 + S,
                owned_bytes: S,
            },
            deficits: [4, 2, 2, 4],
            storage: Some(0),
            prefixes: &[(6, (None, 0))],
            missing: &[0, 4],
        },
        Fixture {
            text: include_str!("fixtures/metadata_class_supertypes/conflicts.hex"),
            parent: (None, 0),
            children: vec![(
                4,
                12,
                [
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
            )],
            exact: Limits {
                bytes: 12,
                records: 8,
                nodes: 2,
                depth: 2,
                work: 50 + S,
                owned_bytes: S,
            },
            deficits: [10, 4, 4, 2],
            storage: Some(2),
            prefixes: &[(2, (None, 0)), (12, (None, 0))],
            missing: &[0],
        },
    ]
}

#[test]
fn frozen_literals_prefixes_and_exact_aggregate_limits() {
    let mut prefixes = 0;
    for f in fixtures() {
        let bytes = hex(f.text);
        let l = f.exact;
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
            check(&bytes, limits, f.parent, &f.children);
        }
        for (label, limits, offset) in [
            (
                "bytes",
                Limits {
                    bytes: l.bytes - 1,
                    ..l
                },
                0,
            ),
            (
                "records",
                Limits {
                    records: l.records - 1,
                    ..l
                },
                f.deficits[0],
            ),
            (
                "nodes",
                Limits {
                    nodes: l.nodes - 1,
                    ..l
                },
                f.deficits[1],
            ),
            (
                "depth",
                Limits {
                    depth: l.depth - 1,
                    ..l
                },
                f.deficits[2],
            ),
            (
                "work",
                Limits {
                    work: l.work - 1,
                    ..l
                },
                f.deficits[3],
            ),
        ] {
            limited(&bytes, limits, label, offset);
        }
        if let Some(offset) = f.storage {
            limited(
                &bytes,
                Limits {
                    owned_bytes: l.owned_bytes - 1,
                    ..l
                },
                "owned_bytes",
                offset,
            );
        }
        for n in 0..=bytes.len() {
            prefixes += 1;
            if let Some(&(_, parent)) = f.prefixes.iter().find(|&&(end, _)| end == n) {
                let children: Vec<_> = f
                    .children
                    .iter()
                    .copied()
                    .filter(|&(_, end, _)| end <= n)
                    .collect();
                check(&bytes[..n], Limits::default(), parent, &children);
            } else {
                error(
                    &bytes[..n],
                    Limits::default(),
                    if f.missing.contains(&n) {
                        ErrorKind::MissingFqName
                    } else {
                        ErrorKind::Wire(WireKind::Truncated)
                    },
                    n,
                );
            }
        }
    }
    assert_eq!(prefixes, 213);
}

#[test]
fn frozen_malformed_and_parent_precedence() {
    for (text, kind, offset) in [
        ("", ErrorKind::MissingFqName, 0),
        ("1a 00", ErrorKind::MissingFqName, 2),
        ("32 01 00", ErrorKind::MissingFqName, 3),
        ("1a 00 32 01 00", ErrorKind::MissingFqName, 5),
        ("18 00 00", ErrorKind::Wire(WireKind::InvalidFieldNumber), 2),
        ("18 00 33", ErrorKind::Wire(WireKind::UnsupportedGroup), 2),
        ("18 00 36", ErrorKind::Wire(WireKind::InvalidWireType), 2),
        ("18 00 32 02 30", ErrorKind::Wire(WireKind::Truncated), 5),
        (
            "18 00 32 ff ff ff ff 0f",
            ErrorKind::Wire(WireKind::NegativeLength),
            3,
        ),
        (
            "18 00 32 01 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            4,
        ),
        (
            "18 00 32 01 0b",
            ErrorKind::Wire(WireKind::UnsupportedGroup),
            4,
        ),
        (
            "18 00 32 01 0e",
            ErrorKind::Wire(WireKind::InvalidWireType),
            4,
        ),
        ("18 00 32 01 30", ErrorKind::Wire(WireKind::Truncated), 5),
        (
            "18 00 32 03 12 02 ff",
            ErrorKind::Wire(WireKind::Truncated),
            7,
        ),
        (
            "18 00 32 06 12 ff ff ff ff 0f",
            ErrorKind::Wire(WireKind::NegativeLength),
            5,
        ),
        (
            "18 00 32 0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Wire(WireKind::MalformedVarint),
            13,
        ),
        (
            "18 00 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Wire(WireKind::MalformedVarint),
            11,
        ),
        (
            "18 00 32 01 00 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            5,
        ),
        (
            "32 01 00 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            3,
        ),
        (
            "18 00 32 00 32 01 30",
            ErrorKind::Wire(WireKind::Truncated),
            7,
        ),
        (
            "18 00 b2 00 81 00 30",
            ErrorKind::Wire(WireKind::Truncated),
            7,
        ),
        (
            "32 01 00 18 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            2,
        ),
    ] {
        error(&hex(text), Limits::default(), kind, offset);
    }
    for (body, kind, local) in [
        ("00", WireKind::InvalidFieldNumber, 0),
        ("0b", WireKind::UnsupportedGroup, 0),
        ("0e", WireKind::InvalidWireType, 0),
        ("30", WireKind::Truncated, 1),
        ("12 02 ff", WireKind::Truncated, 3),
        ("12 ff ff ff ff 0f", WireKind::NegativeLength, 1),
        (
            "80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            9,
        ),
    ] {
        let raw = hex(body);
        let mut bytes = vec![0x18, 0, 0x32, raw.len() as u8];
        bytes.extend(&raw);
        for prefix in [vec![], hex("18 00 32 00")] {
            let mut shifted = prefix;
            let shift = shifted.len();
            shifted.extend(&bytes);
            error(
                &shifted,
                Limits::default(),
                ErrorKind::Wire(kind),
                shift + 4 + local,
            );
            let end = shifted.len();
            shifted.push(0);
            error(
                &shifted,
                Limits::default(),
                ErrorKind::Wire(WireKind::InvalidFieldNumber),
                end,
            );
        }
        error(
            &bytes[2..],
            Limits::default(),
            ErrorKind::MissingFqName,
            bytes.len() - 2,
        );
        bytes[0] = 0x1a;
        error(
            &bytes,
            Limits::default(),
            ErrorKind::MissingFqName,
            bytes.len(),
        );
    }
}

#[test]
fn resource_order_and_vector_growth() {
    let bytes = hex("18 00 32 00");
    for (work, offset) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 1),
        (4, 0),
        (5, 2),
        (6, 2),
        (7, 3),
        (8, 2),
        (9, 0),
        (10, 0),
        (11, 1),
        (12, 0),
        (13, 2),
        (14, 2),
        (15, 3),
        (16, 2),
        (17, 4),
        (18, 2),
    ] {
        limited(
            &bytes,
            Limits {
                work,
                ..Limits::default()
            },
            "work",
            offset,
        );
    }
    for (records, offset) in [(0, 0), (1, 2), (2, 0), (3, 2)] {
        limited(
            &bytes,
            Limits {
                records,
                ..Limits::default()
            },
            "records",
            offset,
        );
    }
    limited(
        &bytes,
        Limits {
            records: 3,
            depth: 1,
            ..Limits::default()
        },
        "records",
        2,
    );
    limited(
        &bytes,
        Limits {
            work: 18,
            owned_bytes: S - 1,
            ..Limits::default()
        },
        "owned_bytes",
        2,
    );
    limited(
        &bytes,
        Limits {
            depth: 0,
            ..Limits::default()
        },
        "depth",
        0,
    );
    limited(
        &bytes,
        Limits {
            depth: 1,
            nodes: 0,
            ..Limits::default()
        },
        "nodes",
        0,
    );
    limited(
        &bytes,
        Limits {
            nodes: 1,
            ..Limits::default()
        },
        "nodes",
        4,
    );
    check(
        &bytes,
        Limits {
            bytes: 4,
            records: 4,
            nodes: 2,
            depth: 2,
            work: 18 + S,
            owned_bytes: S,
        },
        (None, 0),
        &[(4, 4, [None; 10])],
    );
    let zero = Limits {
        bytes: 0,
        records: 0,
        nodes: 0,
        depth: 0,
        work: 0,
        owned_bytes: 0,
    };
    limited(&bytes, Limits { bytes: 3, ..zero }, "bytes", 0);
    limited(&[], zero, "depth", 0);
    error(&[], Limits::default(), ErrorKind::MissingFqName, 0);
    let minimal = hex("18 00");
    limited(&minimal, Limits { bytes: 1, ..zero }, "bytes", 0);
    limited(&minimal, Limits { bytes: 2, ..zero }, "depth", 0);
    limited(
        &minimal,
        Limits {
            bytes: 2,
            depth: 1,
            ..zero
        },
        "nodes",
        0,
    );
    limited(
        &minimal,
        Limits {
            bytes: 2,
            depth: 1,
            nodes: 1,
            ..zero
        },
        "work",
        0,
    );
    let bad = hex("18 00 32 01 00");
    error(
        &bad,
        Limits {
            owned_bytes: 0,
            ..Limits::default()
        },
        ErrorKind::Wire(WireKind::InvalidFieldNumber),
        4,
    );
    limited(
        &bad,
        Limits {
            depth: 1,
            ..Limits::default()
        },
        "depth",
        4,
    );
    limited(
        &bad,
        Limits {
            nodes: 1,
            ..Limits::default()
        },
        "nodes",
        4,
    );
    for (records, offset) in [(3, 2), (4, 4)] {
        limited(
            &bad,
            Limits {
                records,
                ..Limits::default()
            },
            "records",
            offset,
        );
    }
    for work in [20, 21] {
        limited(
            &bad,
            Limits {
                work,
                ..Limits::default()
            },
            "work",
            4,
        );
    }
    error(
        &bad,
        Limits {
            work: 22,
            owned_bytes: 0,
            ..Limits::default()
        },
        ErrorKind::Wire(WireKind::InvalidFieldNumber),
        4,
    );
    error(
        &hex("18 00 32 01 00 00"),
        Limits {
            depth: 1,
            owned_bytes: 0,
            ..Limits::default()
        },
        ErrorKind::Wire(WireKind::InvalidFieldNumber),
        5,
    );
    for text in ["32 01 00", "1a 00 32 01 00"] {
        let input = hex(text);
        error(
            &input,
            Limits {
                depth: 1,
                owned_bytes: 0,
                ..Limits::default()
            },
            ErrorKind::MissingFqName,
            input.len(),
        );
    }
    let mut bytes = hex("18 00");
    for (k, capacity, storage, work_offset) in [
        (1, 1, 2, 2),
        (2, 2, 4, 4),
        (3, 4, 6, 6),
        (4, 4, 6, 10),
        (5, 8, 10, 10),
    ] {
        bytes.extend([0x32, 0]);
        let children: Vec<_> = (0..k).map(|i| (4 + 2 * i, 4 + 2 * i, [None; 10])).collect();
        let exact = Limits {
            bytes: 2 + 2 * k,
            records: 2 + 2 * k,
            nodes: 1 + k,
            depth: 2,
            owned_bytes: capacity * S,
            work: 9 + 9 * k + capacity * S,
        };
        check(&bytes, exact, (None, 0), &children);
        limited(
            &bytes,
            Limits {
                owned_bytes: capacity * S - 1,
                ..exact
            },
            "owned_bytes",
            storage,
        );
        limited(
            &bytes,
            Limits {
                work: exact.work - 1,
                ..exact
            },
            "work",
            work_offset,
        );
        limited(&bytes, Limits { nodes: k, ..exact }, "nodes", 2 + 2 * k);
        limited(
            &bytes,
            Limits {
                records: exact.records - 1,
                ..exact
            },
            "records",
            2 * k,
        );
        if k == 3 {
            limited(
                &bytes,
                Limits {
                    owned_bytes: 3 * S,
                    ..exact
                },
                "owned_bytes",
                6,
            );
        }
    }
}

#[test]
fn genuine_unit_pipeline_and_shared_budget() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    assert_eq!(
        bytes,
        hex(
            "12 0a 02 10 02 0a 02 10 00 0a 02 08 02 0a 02 10 0e 0a 00 08 c6 02 18 00 32 02 30 01 42 07 08 42 a2 06 02 08 02 4a 0a 10 03 1a 02 30 04 48 96 80 04 a8 06 05"
        )
    );
    let payload = metadata::payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(payload.payload_span, 19..52);
    assert_eq!(
        payload.payload,
        hex(include_str!(
            "fixtures/metadata_class_supertypes/unit_class.hex"
        ))
    );
    let exact = Limits {
        bytes: 33,
        records: 13,
        nodes: 2,
        depth: 2,
        owned_bytes: S,
        work: 96 + S,
    };
    let out = check(payload.payload, exact, (Some(326), 0), &[(7, 9, one(3, 1))]);
    assert_eq!(out.header.raw.as_ptr(), bytes[19..52].as_ptr());
    assert_eq!(out.supertypes[0].header.raw, hex("30 01"));
    assert_eq!(
        out.supertypes[0].header.raw.as_ptr(),
        bytes[26..28].as_ptr()
    );
    let standalone = class_header::decode(
        payload.payload,
        class_header::Limits {
            bytes: 33,
            records: 6,
            work: 45,
        },
    )
    .unwrap();
    assert_eq!(out.header, standalone);
    let child = type_header::decode(
        &payload.payload[7..9],
        type_header::Limits {
            bytes: 2,
            records: 1,
            work: 4,
        },
    )
    .unwrap();
    assert_eq!(out.supertypes[0].header, child);
    limited(
        payload.payload,
        Limits {
            records: 6,
            ..exact
        },
        "records",
        0,
    );
    limited(
        payload.payload,
        Limits {
            records: 12,
            ..exact
        },
        "records",
        30,
    );
    limited(
        payload.payload,
        Limits {
            work: 95 + S,
            ..exact
        },
        "work",
        30,
    );
    limited(
        payload.payload,
        Limits {
            owned_bytes: S - 1,
            ..exact
        },
        "owned_bytes",
        5,
    );
    limited(payload.payload, Limits { depth: 1, ..exact }, "depth", 7);
    limited(payload.payload, Limits { nodes: 1, ..exact }, "nodes", 7);
}

#[test]
fn scalar_presence_wrong_wires_and_signed_low_bits() {
    for (index, tag) in [
        (0, 0x08),
        (1, 0x18),
        (2, 0x20),
        (3, 0x30),
        (4, 0x38),
        (5, 0x40),
        (6, 0x48),
        (7, 0x58),
        (8, 0x60),
        (9, 0x70),
    ] {
        for value in [0, 1] {
            check(
                &[0x18, 0, 0x32, 2, tag, value],
                Limits::default(),
                (None, 0),
                &[(4, 6, one(index, value.into()))],
            );
        }
        check(
            &[0x18, 0, 0x32, 6, tag, 0, tag, 1, tag, 0],
            Limits::default(),
            (None, 0),
            &[(4, 10, one(index, 0))],
        );
        for wrong in [
            vec![tag | 1, 1, 2, 3, 4, 5, 6, 7, 8],
            vec![tag | 2, 1, 0x80],
            vec![tag | 5, 1, 2, 3, 4],
        ] {
            for (before, after, expected) in [
                (false, false, [None; 10]),
                (true, false, one(index, 1)),
                (false, true, one(index, 1)),
            ] {
                let mut body = Vec::new();
                if before {
                    body.extend([tag, 1]);
                }
                body.extend(&wrong);
                if after {
                    body.extend([tag, 1]);
                }
                let mut input = vec![0x18, 0, 0x32, body.len() as u8];
                input.extend(body);
                check(
                    &input,
                    Limits::default(),
                    (None, 0),
                    &[(4, input.len(), expected)],
                );
            }
        }
    }
    for (raw, integer, boolean) in [
        ("00", 0, 0),
        ("80 00", 0, 0),
        ("01", 1, 1),
        ("02", 2, 1),
        ("ff ff ff ff 0f", -1, 1),
        ("ff ff ff ff ff ff ff ff ff 01", -1, 1),
        ("ff ff ff ff ff ff ff ff ff 7f", -1, 1),
        ("80 80 80 80 08", i32::MIN, 1),
        ("ff ff ff ff 07", i32::MAX, 1),
        ("80 80 80 80 10", 0, 1),
        ("80 80 80 80 80 80 80 80 80 02", 0, 0),
    ] {
        let value = hex(raw);
        for (index, tag) in [
            (0, 0x08),
            (1, 0x18),
            (2, 0x20),
            (3, 0x30),
            (4, 0x38),
            (5, 0x40),
            (6, 0x48),
            (7, 0x58),
            (8, 0x60),
            (9, 0x70),
        ] {
            let mut input = vec![0x18, 0, 0x32, (1 + value.len()) as u8, tag];
            input.extend(&value);
            check(
                &input,
                Limits::default(),
                (None, 0),
                &[(
                    4,
                    input.len(),
                    one(index, if index == 1 { boolean } else { integer }),
                )],
            );
        }
        for tag in [0x08, 0x18] {
            let mut input = hex("18 00");
            input.push(tag);
            input.extend(&value);
            check(
                &input,
                Limits::default(),
                if tag == 8 {
                    (Some(integer), 0)
                } else {
                    (None, integer)
                },
                &[],
            );
        }
    }
    for wrong in [
        hex("08 09 0a 01 80 18 03 1a 01 80"),
        hex("0a 01 80 08 09 1a 01 80 18 03"),
    ] {
        check(&wrong, Limits::default(), (Some(9), 3), &[]);
    }
    for wrong in [
        hex("30 01"),
        hex("31 01 02 03 04 05 06 07 08"),
        hex("35 01 02 03 04"),
    ] {
        for prefix in [hex("18 00"), hex("18 00 32 00 32 02 30 01")] {
            let mut input = prefix;
            let had_children = input.len() > 2;
            input.extend(&wrong);
            if had_children {
                check(
                    &input,
                    Limits::default(),
                    (None, 0),
                    &[(4, 4, [None; 10]), (6, 8, one(3, 1))],
                );
            } else {
                input.extend(hex("32 00 32 02 30 01"));
                let start = 2 + wrong.len();
                check(
                    &input,
                    Limits::default(),
                    (None, 0),
                    &[
                        (start + 2, start + 2, [None; 10]),
                        (start + 4, start + 6, one(3, 1)),
                    ],
                );
            }
        }
    }
}

#[test]
fn opaque_bodies_unknowns_continuations_and_hostile_byte_sweep() {
    for body in [0, 0x80, 0x0b, 0xff] {
        for tag in [0x2a, 0x42, 0x4a, 0x12] {
            check(
                &[0x18, 0, tag, 1, body, 0x32, 0],
                Limits {
                    depth: 2,
                    ..Limits::default()
                },
                (None, 0),
                &[(7, 7, [None; 10])],
            );
        }
        for tag in [hex("12"), hex("2a"), hex("52"), hex("6a"), hex("a2 06")] {
            let mut child = tag;
            child.extend([1, body]);
            let mut input = vec![0x18, 0, 0x32, child.len() as u8];
            input.extend(child);
            check(
                &input,
                Limits {
                    depth: 2,
                    ..Limits::default()
                },
                (None, 0),
                &[(4, input.len(), [None; 10])],
            );
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
        for prefix in [hex("18 00"), hex("18 00 32 02 30 01")] {
            let mut input = prefix;
            let has_child = input.len() > 2;
            input.extend(&raw);
            let children = [(4, 6, one(3, 1))];
            check(
                &input,
                Limits::default(),
                (None, 0),
                if has_child { &children } else { &[] },
            );
        }
    }
    for n in 1..=10 {
        let mut parent = hex("18 00 32");
        parent.extend(vec![0x80; n]);
        error(
            &parent,
            Limits::default(),
            ErrorKind::Wire(if n == 10 {
                WireKind::MalformedVarint
            } else {
                WireKind::Truncated
            }),
            if n == 10 { 12 } else { parent.len() },
        );
        let mut child = vec![0x18, 0, 0x32, (1 + n) as u8, 0x30];
        child.extend(vec![0x80; n]);
        error(
            &child,
            Limits::default(),
            ErrorKind::Wire(if n == 10 {
                WireKind::MalformedVarint
            } else {
                WireKind::Truncated
            }),
            if n == 10 { 14 } else { child.len() },
        );
    }
    for b in 0..=255u8 {
        let kind = if b >= 128 {
            WireKind::Truncated
        } else if b >> 3 == 0 {
            WireKind::InvalidFieldNumber
        } else {
            match b & 7 {
                3 | 4 => WireKind::UnsupportedGroup,
                6 | 7 => WireKind::InvalidWireType,
                _ => WireKind::Truncated,
            }
        };
        let offset = if kind == WireKind::Truncated { 5 } else { 4 };
        for prefix in [hex("18 00"), hex("18 00 32 00")] {
            let shift = prefix.len() - 2;
            let mut input = prefix;
            input.extend([0x32, 1, b]);
            error(
                &input,
                Limits::default(),
                ErrorKind::Wire(kind),
                offset + shift,
            );
            let end = input.len();
            input.push(0);
            error(
                &input,
                Limits::default(),
                ErrorKind::Wire(WireKind::InvalidFieldNumber),
                end,
            );
        }
    }
}

#[test]
fn caller_limits_cannot_raise_hard_bounds() {
    let huge = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for f in fixtures() {
        check(&hex(f.text), huge, f.parent, &f.children);
    }
    limited(&hex("18 00"), Limits { nodes: 0, ..huge }, "nodes", 0);
    limited(&hex("18 00"), Limits { depth: 0, ..huge }, "depth", 0);
    let mut fields = hex("18 00");
    for _ in 0..32_766 {
        fields.extend([8, 0]);
    }
    check(&fields, huge, (Some(0), 0), &[]);
    fields.extend([8, 0]);
    limited(&fields, huge, "records", 65_534);
    let mut opaque = hex("18 00 2a f8 ff ff 07");
    opaque.resize(16 * 1024 * 1024 - 1, 0);
    check(&opaque, huge, (None, 0), &[]);
    check(&opaque, Limits::default(), (None, 0), &[]);
    opaque.extend([8, 0]);
    limited(&opaque, huge, "bytes", 0);
}
