use ktlsp_binary::metadata::{
    self, payload, property_header,
    property_types::{Error, ErrorKind, Limits, PropertyWithTypes, SelectedType, decode},
    type_header::TypeHeader,
    wire::{self, ErrorKind as WireKind, Value},
};
use std::mem::size_of;
const S: usize = size_of::<SelectedType<'_>>();
type Parent = (i32, Option<i32>, Option<i32>, Option<i32>, Option<i32>);
type Child = (u32, usize, usize, [Option<i32>; 10]);
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}
fn one(i: usize, v: i32) -> [Option<i32>; 10] {
    let mut out = [None; 10];
    out[i] = Some(v);
    out
}
fn scalars(t: &TypeHeader<'_>) -> [Option<i32>; 10] {
    [
        t.flags,
        t.nullable.map(i32::from),
        t.flexible_type_capabilities_id,
        t.class_name,
        t.type_parameter,
        t.flexible_upper_bound_id,
        t.type_parameter_name,
        t.outer_type_id,
        t.type_alias_name,
        t.abbreviated_type_id,
    ]
}
fn check_out(out: &PropertyWithTypes<'_>, bytes: &[u8], parent: Parent, types: &[Child]) {
    let p = &out.property;
    assert_eq!(p.raw, bytes);
    assert_eq!(p.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(
        (p.name, p.flags, p.old_flags, p.getter_flags, p.setter_flags),
        parent
    );
    assert_eq!(out.types.len(), types.len());
    for (t, &(field, start, end, values)) in out.types.iter().zip(types) {
        assert_eq!(t.field_number, field);
        assert_eq!(t.span, start..end);
        assert_eq!(t.header.raw, &bytes[start..end]);
        assert_eq!(t.header.raw.as_ptr(), bytes[start..end].as_ptr());
        assert_eq!(scalars(&t.header), values);
    }
}
fn check(bytes: &[u8], limits: Limits, parent: Parent, types: &[Child]) {
    check_out(&decode(bytes, limits).unwrap(), bytes, parent, types);
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
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
    id: &'static str,
    text: &'static str,
    parent: Parent,
    types: &'static [Child],
    boundaries: &'static [usize],
    limits: Limits,
    cuts: [usize; 6],
}
fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            id: "minimal",
            text: include_str!("fixtures/metadata_property_types/minimal.hex"),
            parent: (0, None, None, None, None),
            types: &[],
            boundaries: &[2],
            limits: Limits {
                bytes: 2,
                records: 2,
                nodes: 1,
                depth: 1,
                owned_bytes: 0,
                work: 9,
            },
            cuts: [0, 0, 0, 0, 0, 0],
        },
        Fixture {
            id: "empty",
            text: include_str!("fixtures/metadata_property_types/empty.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                3,
                4,
                4,
                [None, None, None, None, None, None, None, None, None, None],
            )],
            boundaries: &[2, 4],
            limits: Limits {
                bytes: 4,
                records: 4,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 18 + S,
            },
            cuts: [0, 2, 4, 4, 2, 2],
        },
        Fixture {
            id: "receiver",
            text: include_str!("fixtures/metadata_property_types/receiver.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                5,
                4,
                6,
                [
                    None,
                    None,
                    None,
                    Some(0),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            )],
            boundaries: &[2, 6],
            limits: Limits {
                bytes: 6,
                records: 5,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 26 + S,
            },
            cuts: [0, 4, 4, 4, 2, 2],
        },
        Fixture {
            id: "interleaved",
            text: include_str!("fixtures/metadata_property_types/interleaved.hex"),
            parent: (0, None, None, None, None),
            types: &[
                (
                    3,
                    4,
                    6,
                    [
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
                ),
                (
                    5,
                    8,
                    8,
                    [None, None, None, None, None, None, None, None, None, None],
                ),
                (
                    3,
                    10,
                    12,
                    [
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
                ),
                (
                    5,
                    14,
                    16,
                    [
                        None,
                        None,
                        None,
                        None,
                        Some(2),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                ),
            ],
            boundaries: &[2, 6, 8, 12, 16],
            limits: Limits {
                bytes: 16,
                records: 13,
                nodes: 5,
                depth: 2,
                owned_bytes: 4 * S,
                work: 69 + 4 * S,
            },
            cuts: [0, 14, 14, 4, 8, 14],
        },
        Fixture {
            id: "duplicates",
            text: include_str!("fixtures/metadata_property_types/duplicates.hex"),
            parent: (0, None, None, None, None),
            types: &[
                (
                    3,
                    4,
                    6,
                    [
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
                ),
                (
                    3,
                    8,
                    10,
                    [
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
                ),
            ],
            boundaries: &[2, 6, 10],
            limits: Limits {
                bytes: 10,
                records: 8,
                nodes: 3,
                depth: 2,
                owned_bytes: 2 * S,
                work: 43 + 2 * S,
            },
            cuts: [0, 8, 8, 4, 6, 6],
        },
        Fixture {
            id: "scalars",
            text: include_str!("fixtures/metadata_property_types/scalars.hex"),
            parent: (2, Some(3), Some(1), Some(4), Some(5)),
            types: &[(
                3,
                22,
                42,
                [
                    Some(1),
                    Some(1),
                    Some(3),
                    Some(4),
                    Some(5),
                    Some(6),
                    Some(7),
                    Some(8),
                    Some(9),
                    Some(10),
                ],
            )],
            boundaries: &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 42],
            limits: Limits {
                bytes: 42,
                records: 32,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 170 + S,
            },
            cuts: [0, 40, 22, 22, 20, 20],
        },
        Fixture {
            id: "wrong_wire",
            text: include_str!("fixtures/metadata_property_types/wrong_wire.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                3,
                8,
                8,
                [None, None, None, None, None, None, None, None, None, None],
            )],
            boundaries: &[2, 4, 6, 8, 13, 22, 27, 36],
            limits: Limits {
                bytes: 36,
                records: 16,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 106 + S,
            },
            cuts: [0, 27, 8, 8, 6, 27],
        },
        Fixture {
            id: "opaque",
            text: include_str!("fixtures/metadata_property_types/opaque.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                3,
                4,
                16,
                [None, None, None, None, None, None, None, None, None, None],
            )],
            boundaries: &[2, 16, 19],
            limits: Limits {
                bytes: 19,
                records: 10,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 72 + S,
            },
            cuts: [0, 16, 4, 4, 2, 16],
        },
        Fixture {
            id: "ids",
            text: include_str!("fixtures/metadata_property_types/ids.hex"),
            parent: (0, None, None, None, None),
            types: &[
                (
                    3,
                    8,
                    10,
                    [
                        None,
                        None,
                        None,
                        Some(0),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                ),
                (
                    5,
                    12,
                    14,
                    [
                        None,
                        None,
                        None,
                        None,
                        Some(1),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ],
                ),
            ],
            boundaries: &[2, 4, 6, 10, 14],
            limits: Limits {
                bytes: 14,
                records: 12,
                nodes: 3,
                depth: 2,
                owned_bytes: 2 * S,
                work: 59 + 2 * S,
            },
            cuts: [0, 12, 12, 8, 10, 10],
        },
        Fixture {
            id: "nonminimal",
            text: include_str!("fixtures/metadata_property_types/nonminimal.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                3,
                12,
                12,
                [None, None, None, None, None, None, None, None, None, None],
            )],
            boundaries: &[2, 12],
            limits: Limits {
                bytes: 12,
                records: 4,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 34 + S,
            },
            cuts: [0, 2, 12, 12, 2, 2],
        },
        Fixture {
            id: "raw64_signed",
            text: include_str!("fixtures/metadata_property_types/raw64_signed.hex"),
            parent: (0, None, None, None, None),
            types: &[(
                3,
                4,
                23,
                [
                    Some(-1),
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
            )],
            boundaries: &[2, 23],
            limits: Limits {
                bytes: 23,
                records: 9,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 85 + S,
            },
            cuts: [0, 20, 4, 4, 2, 2],
        },
        Fixture {
            id: "pair_first",
            text: include_str!("fixtures/metadata_property_types/pair_first.hex"),
            parent: (5, Some(131590), None, None, None),
            types: &[(
                3,
                4,
                6,
                [
                    None,
                    None,
                    None,
                    None,
                    Some(0),
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            )],
            boundaries: &[2, 6, 10, 23],
            limits: Limits {
                bytes: 23,
                records: 9,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 68 + S,
            },
            cuts: [0, 10, 4, 4, 2, 10],
        },
        Fixture {
            id: "pair_second",
            text: include_str!("fixtures/metadata_property_types/pair_second.hex"),
            parent: (6, Some(131590), None, None, None),
            types: &[(
                3,
                4,
                6,
                [
                    None,
                    None,
                    None,
                    None,
                    Some(1),
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            )],
            boundaries: &[2, 6, 10, 23],
            limits: Limits {
                bytes: 23,
                records: 9,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 68 + S,
            },
            cuts: [0, 10, 4, 4, 2, 10],
        },
    ]
}
#[test]
fn frozen_full_tuples_borrows_and_exact_aggregate_deficits() {
    for f in fixtures() {
        let bytes = hex(f.text);
        check(&bytes, f.limits, f.parent, f.types);
        for (i, label) in ["bytes", "records", "nodes", "depth", "owned_bytes", "work"]
            .into_iter()
            .enumerate()
        {
            for increase in [true, false] {
                let mut l = f.limits;
                let v = match i {
                    0 => &mut l.bytes,
                    1 => &mut l.records,
                    2 => &mut l.nodes,
                    3 => &mut l.depth,
                    4 => &mut l.owned_bytes,
                    _ => &mut l.work,
                };
                if increase {
                    *v += 1;
                } else if *v == 0 {
                    continue;
                } else {
                    *v -= 1;
                }
                if increase {
                    check(&bytes, l, f.parent, f.types);
                } else {
                    limited(&bytes, l, label, f.cuts[i]);
                }
            }
        }
    }
}
#[test]
fn every_frozen_prefix_and_partial_parent_presence() {
    let mut count = 0;
    for f in fixtures() {
        let bytes = hex(f.text);
        for cut in 0..=bytes.len() {
            count += 1;
            if cut == 0 || (f.id == "scalars" && cut == 2) {
                error(
                    &bytes[..cut],
                    Default::default(),
                    ErrorKind::MissingName,
                    cut,
                );
            } else if f.boundaries.contains(&cut) {
                let mut p = f.parent;
                if f.id.starts_with("pair_") && cut < 10 {
                    p.1 = None;
                }
                if f.id == "scalars" {
                    p = (
                        if cut < 14 { 0 } else { 2 },
                        if cut < 10 {
                            None
                        } else if cut < 20 {
                            Some(0)
                        } else {
                            Some(3)
                        },
                        Some(if cut < 12 { 0 } else { 1 }),
                        if cut < 6 {
                            None
                        } else {
                            Some(if cut < 16 { 0 } else { 4 })
                        },
                        if cut < 8 {
                            None
                        } else {
                            Some(if cut < 18 { 0 } else { 5 })
                        },
                    );
                }
                let types: Vec<_> = f.types.iter().copied().filter(|t| t.2 <= cut).collect();
                check(&bytes[..cut], Default::default(), p, &types);
            } else {
                error(
                    &bytes[..cut],
                    Default::default(),
                    ErrorKind::Wire(WireKind::Truncated),
                    cut,
                );
            }
        }
    }
    assert_eq!(count, 243);
}
#[test]
fn all_nineteen_frozen_malformed_literals() {
    for (text, kind, offset) in [
        ("", ErrorKind::MissingName, 0),
        ("1a 00", ErrorKind::MissingName, 2),
        ("1a 01 00", ErrorKind::MissingName, 3),
        ("12 00 1a 01 00", ErrorKind::MissingName, 5),
        (
            "10 00 1a 01 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            4,
        ),
        (
            "10 00 2a 01 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            4,
        ),
        ("10 00 1a 01 80", ErrorKind::Wire(WireKind::Truncated), 5),
        ("10 00 2a 01 30", ErrorKind::Wire(WireKind::Truncated), 5),
        (
            "10 00 1a 01 0b",
            ErrorKind::Wire(WireKind::UnsupportedGroup),
            4,
        ),
        (
            "10 00 2a 01 0e",
            ErrorKind::Wire(WireKind::InvalidWireType),
            4,
        ),
        (
            "10 00 1a 01 00 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            5,
        ),
        (
            "1a 01 00 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            3,
        ),
        ("10 00 1a 02 00", ErrorKind::Wire(WireKind::Truncated), 5),
        (
            "10 00 1a 00 2a 01 00",
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            6,
        ),
        (
            "10 00 1a 06 32 ff ff ff ff 0f",
            ErrorKind::Wire(WireKind::NegativeLength),
            5,
        ),
        (
            "10 00 1a 0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Wire(WireKind::MalformedVarint),
            13,
        ),
        ("10 00 00", ErrorKind::Wire(WireKind::InvalidFieldNumber), 2),
        ("10 00 0b", ErrorKind::Wire(WireKind::UnsupportedGroup), 2),
        ("10 00 0e", ErrorKind::Wire(WireKind::InvalidWireType), 2),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
}
const PAIR_D1: &str = "2e 0a 02 18 02 0a 02 08 02 0a 02 18 02 0a 02 18 02 0a 02 08 09 0a 02 10 0e 0a 02 08 05 0a 02 10 0b 0a 00 0a 02 10 00 0a 00 0a 02 10 08 0a 00 08 87 08 18 00 2a 06 08 00 10 01 20 01 2a 06 08 01 10 02 20 01 32 06 30 03 6a 02 60 04 42 19 08 46 12 06 10 05 1a 02 38 00 12 06 10 06 1a 02 38 01 a2 06 04 08 07 10 08 4a 0a 10 0d 1a 02 30 0e 48 96 80 04 4a 0f 10 0f 1a 02 38 00 48 c6 83 04 a2 06 02 10 0a 4a 0f 10 10 1a 02 38 01 48 c6 83 04 a2 06 02 10 0a 4a 2f 10 11 1a 0e 12 04 12 02 38 00 12 04 12 02 38 01 30 00 32 08 08 02 10 05 1a 02 38 00 32 08 08 02 10 06 1a 02 38 01 48 c6 81 04 a2 06 02 10 12 4a 14 10 13 1a 02 30 14 32 08 10 15 1a 04 18 01 30 16 48 d6 83 04 4a 0a 10 17 1a 02 30 18 48 d6 81 04 52 17 10 05 1a 02 38 00 58 86 84 08 a2 06 0a 0a 02 10 0b 1a 04 08 09 10 0a 52 17 10 06 1a 02 38 01 58 86 84 08 a2 06 0a 0a 02 10 0b 1a 04 08 0c 10 0a a8 06 19";
#[test]
fn child_faults_parent_precedence_and_exactly_once_rebase() {
    for number in [3u8, 5] {
        for (body, kind, local) in [
            (hex("00"), WireKind::InvalidFieldNumber, 0),
            (hex("80"), WireKind::Truncated, 1),
            (hex("30"), WireKind::Truncated, 1),
            (hex("0b"), WireKind::UnsupportedGroup, 0),
            (hex("0c"), WireKind::UnsupportedGroup, 0),
            (hex("0e"), WireKind::InvalidWireType, 0),
            (hex("0f"), WireKind::InvalidWireType, 0),
            (hex("32 ff ff ff ff 0f"), WireKind::NegativeLength, 1),
            (vec![0x80; 10], WireKind::MalformedVarint, 9),
        ] {
            let raw = [vec![0x10, 0, number << 3 | 2, body.len() as u8], body].concat();
            for limits in [
                Limits::default(),
                Limits {
                    owned_bytes: 0,
                    ..Default::default()
                },
            ] {
                error(&raw, limits, ErrorKind::Wire(kind), 4 + local);
            }
            let mut shifted = hex("10 00 1a 00");
            shifted.extend_from_slice(&raw);
            error(
                &shifted,
                Default::default(),
                ErrorKind::Wire(kind),
                8 + local,
            );
            let mut late = raw.clone();
            late.push(0);
            error(
                &late,
                Default::default(),
                ErrorKind::Wire(WireKind::InvalidFieldNumber),
                raw.len(),
            );
            error(
                &raw[2..],
                Limits {
                    owned_bytes: 0,
                    depth: 1,
                    ..Default::default()
                },
                ErrorKind::MissingName,
                raw.len() - 2,
            );
        }
        error(
            &[0x10, 0, number << 3 | 2, 1, 0, 0x10, 0],
            Limits {
                records: 6,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::InvalidFieldNumber),
            4,
        );
        limited(
            &[0x10, 0, number << 3 | 2, 1, 0, 0x10, 0],
            Limits {
                records: 5,
                ..Default::default()
            },
            "records",
            4,
        );
        for (limits, label) in [
            (
                Limits {
                    depth: 1,
                    ..Default::default()
                },
                "depth",
            ),
            (
                Limits {
                    nodes: 1,
                    ..Default::default()
                },
                "nodes",
            ),
        ] {
            limited(&[0x10, 0, number << 3 | 2, 1, 0], limits, label, 4);
        }
        error(
            &[0x10, 0, (number << 3 | 2) | 0x80, 0, 0x81, 0, 0x30],
            Default::default(),
            ErrorKind::Wire(WireKind::Truncated),
            7,
        );
        check(
            &[
                0x10,
                0,
                (number << 3 | 2) | 0x80,
                0x80,
                0x80,
                0x80,
                0x10,
                0x80,
                0x80,
                0x80,
                0x80,
                0x10,
            ],
            Limits {
                bytes: 12,
                records: 4,
                nodes: 2,
                depth: 2,
                owned_bytes: S,
                work: 34 + S,
            },
            (0, None, None, None, None),
            &[(number.into(), 12, 12, [None; 10])],
        );
    }
    assert_eq!(
        property_header::decode(
            &[],
            property_header::Limits {
                bytes: 0,
                records: 0,
                work: 0
            }
        ),
        Err(Error {
            offset: 0,
            kind: ErrorKind::MissingName
        })
    );
    limited(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            nodes: 0,
            depth: 0,
            owned_bytes: 0,
            work: 0,
        },
        "depth",
        0,
    );
}
#[test]
fn every_small_work_cut_and_competing_resources() {
    let raw = hex("10 00 1a 00");
    for (work, offset) in [0, 0, 0, 1, 0, 2, 2, 3, 2, 0, 0, 1, 0, 2, 2, 3, 2, 4, 2]
        .into_iter()
        .enumerate()
    {
        limited(
            &raw,
            Limits {
                work,
                ..Default::default()
            },
            "work",
            offset,
        );
    }
    for (limits, label, offset) in [
        (
            Limits {
                bytes: 3,
                records: 0,
                nodes: 0,
                depth: 0,
                work: 0,
                owned_bytes: 0,
            },
            "bytes",
            0,
        ),
        (
            Limits {
                depth: 0,
                ..Default::default()
            },
            "depth",
            0,
        ),
        (
            Limits {
                depth: 1,
                nodes: 0,
                ..Default::default()
            },
            "nodes",
            0,
        ),
        (
            Limits {
                depth: 2,
                nodes: 1,
                ..Default::default()
            },
            "nodes",
            4,
        ),
        (
            Limits {
                records: 0,
                ..Default::default()
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 1,
                ..Default::default()
            },
            "records",
            2,
        ),
        (
            Limits {
                records: 2,
                ..Default::default()
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 3,
                depth: 1,
                ..Default::default()
            },
            "records",
            2,
        ),
        (
            Limits {
                work: 18,
                owned_bytes: S - 1,
                ..Default::default()
            },
            "owned_bytes",
            2,
        ),
    ] {
        limited(&raw, limits, label, offset);
    }
    // Separate parent and child success does not reset aggregate accounting.
    let raw = hex(include_str!(
        "fixtures/metadata_property_types/pair_first.hex"
    ));
    property_header::decode(
        &raw,
        property_header::Limits {
            bytes: 23,
            records: 4,
            work: 31,
        },
    )
    .unwrap();
    metadata::type_header::decode(
        &raw[4..6],
        metadata::type_header::Limits {
            bytes: 2,
            records: 1,
            work: 4,
        },
    )
    .unwrap();
    limited(
        &raw,
        Limits {
            records: 4,
            ..Default::default()
        },
        "records",
        0,
    );
}
#[test]
fn both_fields_share_growth_and_hard_limits_clamp() {
    for k in 1usize..=5 {
        let mut raw = hex("10 00");
        let mut types = Vec::new();
        for i in 0..k {
            let n = if i % 2 == 0 { 3u8 } else { 5 };
            raw.extend_from_slice(&[n << 3 | 2, 0]);
            types.push((n.into(), 4 + 2 * i, 4 + 2 * i, [None; 10]));
        }
        let q = k.next_power_of_two();
        let work = 9 + 9 * k + q * S;
        check(
            &raw,
            Limits {
                bytes: raw.len(),
                records: 2 + 2 * k,
                nodes: 1 + k,
                depth: 2,
                owned_bytes: q * S,
                work,
            },
            (0, None, None, None, None),
            &types,
        );
        for (limits, label, offset) in [
            (
                Limits {
                    owned_bytes: q * S - 1,
                    ..Default::default()
                },
                "owned_bytes",
                [2, 4, 6, 6, 10][k - 1],
            ),
            (
                Limits {
                    work: work - 1,
                    ..Default::default()
                },
                "work",
                [2, 4, 6, 10, 10][k - 1],
            ),
            (
                Limits {
                    nodes: k,
                    ..Default::default()
                },
                "nodes",
                2 + 2 * k,
            ),
            (
                Limits {
                    records: 1 + 2 * k,
                    ..Default::default()
                },
                "records",
                2 * k,
            ),
        ] {
            limited(&raw, limits, label, offset);
        }
        if k == 3 {
            limited(
                &raw,
                Limits {
                    owned_bytes: 3 * S,
                    ..Default::default()
                },
                "owned_bytes",
                6,
            );
        }
    }
    let max = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        owned_bytes: usize::MAX,
        work: usize::MAX,
    };
    for f in fixtures() {
        check(&hex(f.text), max, f.parent, f.types);
    }
    let mut raw = hex("10 00");
    for _ in 0..32766 {
        raw.extend_from_slice(&[0x18, 0]);
    }
    check(&raw, max, (0, None, None, None, None), &[]);
    raw.extend_from_slice(&[0x18, 0]);
    limited(&raw, max, "records", 65534);
    let mut raw = hex("10 00 32 f8 ff ff 07");
    raw.resize(ktlsp_binary::MAX_BYTES - 1, 0);
    assert_eq!(raw.len(), ktlsp_binary::MAX_BYTES - 1);
    check(&raw, max, (0, None, None, None, None), &[]);
    raw.extend_from_slice(&[0x18, 0]);
    limited(&raw, max, "bytes", 0);
}
#[test]
fn wrong_wires_scalar_presence_ids_and_deeper_bodies_stay_opaque() {
    for n in [3u8, 5] {
        let tag = n << 3 | 2;
        for (wire, body) in [(0, hex("01")), (1, vec![0; 8]), (5, vec![0; 4])] {
            let wrong = [vec![n << 3 | wire], body].concat();
            check(
                &[hex("10 00"), wrong.clone()].concat(),
                Limits {
                    nodes: 1,
                    depth: 1,
                    owned_bytes: 0,
                    ..Default::default()
                },
                (0, None, None, None, None),
                &[],
            );
            for selected in [vec![tag, 0], vec![tag, 2, 0x30, 1]] {
                for before in [true, false] {
                    let mut raw = hex("10 00");
                    if before {
                        raw.extend_from_slice(&wrong);
                    }
                    raw.extend_from_slice(&selected);
                    if !before {
                        raw.extend_from_slice(&wrong);
                    }
                    let start = 4 + if before { wrong.len() } else { 0 };
                    let end = start + selected.len() - 2;
                    check(
                        &raw,
                        Default::default(),
                        (0, None, None, None, None),
                        &[(
                            n.into(),
                            start,
                            end,
                            if end == start { [None; 10] } else { one(3, 1) },
                        )],
                    );
                }
            }
        }
        for body in [
            hex("12 01 00 2a 01 80 52 01 0b 6a 01 00 a2 06 01 80"),
            hex("a0 06 01 f8 ff ff ff 0f 00 32 01 80"),
        ] {
            let raw = [vec![0x10, 0, tag, body.len() as u8], body].concat();
            check(
                &raw,
                Limits {
                    depth: 2,
                    ..Default::default()
                },
                (0, None, None, None, None),
                &[(n.into(), 4, raw.len(), [None; 10])],
            );
        }
        let raw = vec![tag, 2, 0x38, 0, 0x10, 5];
        check(
            &raw,
            Default::default(),
            (5, None, None, None, None),
            &[(n.into(), 2, 4, one(4, 0))],
        );
        for body in [
            hex("08 ff ff ff ff 0f 18 80 80 80 80 10 30 01 30 02 32 01 00"),
            hex("b0 80 80 80 10 00"),
        ] {
            let mut expected = one(3, 0);
            if body.len() == 19 {
                expected = one(0, -1);
                expected[1] = Some(1);
                expected[3] = Some(2);
            }
            let raw = [vec![0x10, 0, tag, body.len() as u8], body].concat();
            check(
                &raw,
                Default::default(),
                (0, None, None, None, None),
                &[(n.into(), 4, raw.len(), expected)],
            );
        }
        for (text, value) in [
            ("00", 0),
            ("80 00", 0),
            ("ff ff ff ff 0f", -1),
            ("ff ff ff ff ff ff ff ff ff 01", -1),
            ("80 80 80 80 08", i32::MIN),
            ("ff ff ff ff 07", i32::MAX),
            ("80 80 80 80 10", 0),
        ] {
            let value_bytes = hex(text);
            for (index, scalar_tag) in [8u8, 24, 32, 48, 56, 64, 72, 88, 96, 112]
                .into_iter()
                .enumerate()
                .filter(|(i, _)| *i != 1)
            {
                let body = [vec![scalar_tag], value_bytes.clone()].concat();
                let raw = [vec![0x10, 0, tag, body.len() as u8], body].concat();
                check(
                    &raw,
                    Default::default(),
                    (0, None, None, None, None),
                    &[(n.into(), 4, raw.len(), one(index, value))],
                );
            }
        }
        for (value, expected) in [(hex("00"), 0), (hex("01"), 1), (hex("80 80 80 80 10"), 1)] {
            let body = [vec![0x18], value].concat();
            let raw = [vec![0x10, 0, tag, body.len() as u8], body].concat();
            check(
                &raw,
                Default::default(),
                (0, None, None, None, None),
                &[(n.into(), 4, raw.len(), one(1, expected))],
            );
        }
    }
    // Wrong-wire parent scalars neither replace nor clear existing values.
    for (tag, index) in [(0x10, 0usize), (0x58, 1), (0x08, 2), (0x38, 3), (0x40, 4)] {
        let mut raw = hex("10 00");
        raw.extend_from_slice(&[tag, 1, tag | 2, 1, 0]);
        let mut p = (0, None, None, None, None);
        match index {
            0 => p.0 = 1,
            1 => p.1 = Some(1),
            2 => p.2 = Some(1),
            3 => p.3 = Some(1),
            _ => p.4 = Some(1),
        }
        check(&raw, Default::default(), p, &[]);
    }
    for tag in [0x22u8, 0x32, 0x62, 0x72, 0x8a] {
        let raw = if tag == 0x8a {
            hex("10 00 8a 01 01 00")
        } else {
            vec![0x10, 0, tag, 1, 0]
        };
        check(&raw, Default::default(), (0, None, None, None, None), &[]);
    }
}
#[test]
fn genuine_pair_actual_metadata_payload_both_property_tuples_and_pointers() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported Metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    assert_eq!(bytes, hex(PAIR_D1));
    let typed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(typed.string_table_span, 1..47);
    assert_eq!(typed.payload_span, 47..285);
    assert_eq!(typed.payload.as_ptr(), bytes[47..].as_ptr());
    let framed = wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload.span, 47..285);
    assert_eq!(framed.payload.raw, typed.payload);
    assert_eq!(framed.payload.raw.as_ptr(), typed.payload.as_ptr());
    let properties: Vec<_> = framed
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 10 && matches!(f.value, Value::Bytes(_)))
        .collect();
    assert_eq!(properties.len(), 2);
    for (i, (field, f)) in properties
        .iter()
        .zip(fixtures().into_iter().filter(|f| f.id.starts_with("pair_")))
        .enumerate()
    {
        let field_start = 232 + 25 * i;
        let start = field_start + 2;
        assert_eq!(field.span, field_start..field_start + 25);
        let Value::Bytes(raw) = field.value else {
            unreachable!()
        };
        assert_eq!(raw, hex(f.text));
        assert_eq!(raw, &bytes[start..start + 23]);
        assert_eq!(raw.as_ptr(), bytes[start..start + 23].as_ptr());
        let out = decode(raw, f.limits).unwrap();
        check_out(&out, raw, f.parent, f.types);
        assert_eq!(
            out.types[0].header.raw.as_ptr(),
            bytes[start + 4..start + 6].as_ptr()
        );
        assert_eq!(out.types[0].header.type_parameter, Some(i as i32));
    }
}
#[test]
fn all_512_one_byte_child_faults_follow_frozen_wire_classes() {
    for byte in 0u8..=255 {
        for number in [3u8, 5] {
            let raw = [0x10, 0, number << 3 | 2, 1, byte];
            let (kind, offset) = if byte >= 128 {
                (WireKind::Truncated, 5)
            } else if byte >> 3 == 0 {
                (WireKind::InvalidFieldNumber, 4)
            } else {
                match byte & 7 {
                    3 | 4 => (WireKind::UnsupportedGroup, 4),
                    6 | 7 => (WireKind::InvalidWireType, 4),
                    _ => (WireKind::Truncated, 5),
                }
            };
            for limits in [
                Limits::default(),
                Limits {
                    bytes: usize::MAX,
                    records: usize::MAX,
                    nodes: usize::MAX,
                    depth: usize::MAX,
                    work: usize::MAX,
                    owned_bytes: usize::MAX,
                },
            ] {
                error(&raw, limits, ErrorKind::Wire(kind), offset);
                check(&hex("10 00"), limits, (0, None, None, None, None), &[]);
            }
        }
    }
}
