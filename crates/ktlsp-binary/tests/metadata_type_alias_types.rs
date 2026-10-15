use ktlsp_binary::{
    MAX_BYTES, MAX_RECORDS,
    metadata::{
        type_alias,
        type_alias_types::{Error, ErrorKind, Limits, SelectedType, TypeAliasWithTypes, decode},
        type_header::TypeHeader,
        wire::ErrorKind as WireKind,
    },
};
use std::{mem::size_of, ops::Range};

const P: usize = size_of::<&[u8]>();
const S: usize = size_of::<SelectedType<'_>>();
type Scalars = (Option<i32>, i32, Option<i32>, Option<i32>);
type Header = [Option<i32>; 10];
type Occurrence = (u32, Range<usize>, Header);
type Fixture = (Scalars, Vec<Occurrence>, Limits, [usize; 5]);
const Z: Header = [None; 10];
const EMPTY: Scalars = (None, 0, None, None);
const HEX: [&str; 10] = [
    include_str!("fixtures/metadata_type_alias_types/minimal.hex"),
    include_str!("fixtures/metadata_type_alias_types/explicit.hex"),
    include_str!("fixtures/metadata_type_alias_types/selected.hex"),
    include_str!("fixtures/metadata_type_alias_types/interleaved.hex"),
    include_str!("fixtures/metadata_type_alias_types/empties.hex"),
    include_str!("fixtures/metadata_type_alias_types/opaque.hex"),
    include_str!("fixtures/metadata_type_alias_types/wrong_wire.hex"),
    include_str!("fixtures/metadata_type_alias_types/nonminimal.hex"),
    include_str!("fixtures/metadata_type_alias_types/all_scalars.hex"),
    include_str!("fixtures/metadata_type_alias_types/signed_parent.hex"),
];

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}
fn one(index: usize, value: i32) -> Header {
    let mut header = Z;
    header[index] = Some(value);
    header
}
fn scalars(out: &TypeAliasWithTypes<'_>) -> Scalars {
    (
        out.flags,
        out.name,
        out.underlying_type_id,
        out.expanded_type_id,
    )
}
fn header(out: &TypeHeader<'_>) -> Header {
    [
        out.flags,
        out.nullable.map(i32::from),
        out.flexible_type_capabilities_id,
        out.class_name,
        out.type_parameter,
        out.flexible_upper_bound_id,
        out.type_parameter_name,
        out.outer_type_id,
        out.type_alias_name,
        out.abbreviated_type_id,
    ]
}
fn borrowed(bytes: &[u8], out: &TypeAliasWithTypes<'_>) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    let mut previous = 0;
    for item in &out.types {
        assert!(matches!(item.field_number, 4 | 6));
        assert!(item.span.start >= previous);
        assert!(item.span.start <= item.span.end && item.span.end <= bytes.len());
        assert_eq!(item.header.raw, &bytes[item.span.clone()]);
        assert_eq!(item.header.raw.as_ptr(), bytes[item.span.clone()].as_ptr());
        previous = item.span.end;
    }
}
fn check(bytes: &[u8], limits: Limits, parent: Scalars, types: &[Occurrence]) {
    let out = decode(bytes, limits).unwrap();
    assert_eq!(scalars(&out), parent, "{bytes:02x?}");
    borrowed(bytes, &out);
    assert_eq!(out.types.len(), types.len(), "{bytes:02x?}");
    for (actual, (field, span, expected)) in out.types.iter().zip(types) {
        assert_eq!(actual.field_number, *field);
        assert_eq!(&actual.span, span);
        assert_eq!(header(&actual.header), *expected);
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn wire_error(bytes: &[u8], limits: Limits, kind: WireKind, offset: usize) {
    error(bytes, limits, ErrorKind::Wire(kind), offset);
}
fn limited(bytes: &[u8], limits: Limits, label: &'static str, offset: usize) {
    wire_error(bytes, limits, WireKind::Limit(label), offset);
}
fn fixtures() -> Vec<Fixture> {
    let mut first = one(1, 1);
    first[3] = Some(2);
    let all = [
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
    ];
    // Last array: records-, nodes-, depth-, work-, owned-minus-one offsets.
    vec![
        (EMPTY, vec![], exact(2, 1, 1, 1, 5, 0), [0, 0, 0, 0, 0]),
        (
            (Some(6), 0, Some(0), Some(0)),
            vec![],
            exact(8, 4, 1, 1, 17, 0),
            [6, 0, 0, 0, 0],
        ),
        (
            (Some(0), 5, Some(7), Some(8)),
            vec![(4, 6..10, first), (6, 12..14, one(4, 3))],
            exact(18, 9, 3, 2, 48 + 2 * P + 2 * S, 2 * P + 2 * S),
            [12, 12, 6, 12, 12],
        ),
        (
            (None, 2, None, None),
            vec![
                (6, 4..6, one(4, 1)),
                (4, 8..10, one(3, 2)),
                (6, 12..12, Z),
                (4, 14..18, one(3, 4)),
            ],
            exact(20, 10, 5, 2, 60 + 4 * P + 4 * S, 4 * P + 4 * S),
            [16, 14, 4, 16, 12],
        ),
        (
            EMPTY,
            vec![
                (4, 4..4, Z),
                (6, 6..6, Z),
                (4, 8..8, Z),
                (6, 10..10, Z),
                (4, 12..12, Z),
            ],
            exact(12, 6, 6, 2, 39 + 6 * P + 8 * S, 6 * P + 8 * S),
            [10, 12, 4, 12, 12],
        ),
        (
            (None, 5, None, None),
            vec![(4, 4..16, Z)],
            exact(22, 8, 2, 2, 53 + P + S, P + S),
            [13, 4, 4, 4, 4],
        ),
        (
            (None, 5, None, None),
            vec![],
            exact(23, 6, 1, 1, 36, 0),
            [20, 0, 0, 0, 0],
        ),
        (
            EMPTY,
            vec![(6, 6..6, Z), (4, 13..15, one(3, 4))],
            exact(15, 4, 3, 2, 31 + 2 * P + 2 * S, 2 * P + 2 * S),
            [13, 13, 6, 13, 13],
        ),
        (
            EMPTY,
            vec![(4, 4..24, all)],
            exact(24, 12, 2, 2, 71 + P + S, P + S),
            [22, 4, 4, 4, 4],
        ),
        (
            (Some(-1), i32::MIN, Some(i32::MAX), Some(0)),
            vec![],
            exact(24, 4, 1, 1, 33, 0),
            [18, 0, 0, 0, 0],
        ),
    ]
}
fn exact(
    bytes: usize,
    records: usize,
    nodes: usize,
    depth: usize,
    work: usize,
    owned_bytes: usize,
) -> Limits {
    Limits {
        bytes,
        records,
        nodes,
        depth,
        work,
        owned_bytes,
    }
}

#[test]
fn literal_tuples_borrows_and_exact_aggregate_minima() {
    for (text, (parent, types, minimum, offsets)) in HEX.iter().zip(fixtures()) {
        let bytes = hex(text);
        assert_eq!(bytes.len(), minimum.bytes);
        check(&bytes, minimum, parent, &types);
        check(
            &bytes,
            exact(
                minimum.bytes + 1,
                minimum.records + 1,
                minimum.nodes + 1,
                minimum.depth + 1,
                minimum.work + 1,
                minimum.owned_bytes + 1,
            ),
            parent,
            &types,
        );
        check(
            &bytes,
            exact(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            ),
            parent,
            &types,
        );
        for (label, value, offset) in [
            ("records", minimum.records, offsets[0]),
            ("nodes", minimum.nodes, offsets[1]),
            ("depth", minimum.depth, offsets[2]),
            ("work", minimum.work, offsets[3]),
            ("owned_bytes", minimum.owned_bytes, offsets[4]),
        ] {
            let mut limits = Limits::default();
            match label {
                "records" => limits.records = value,
                "nodes" => limits.nodes = value,
                "depth" => limits.depth = value,
                "work" => limits.work = value,
                "owned_bytes" => limits.owned_bytes = value,
                _ => unreachable!(),
            }
            check(&bytes, limits, parent, &types);
            if value > 0 {
                match label {
                    "records" => limits.records -= 1,
                    "nodes" => limits.nodes -= 1,
                    "depth" => limits.depth -= 1,
                    "work" => limits.work -= 1,
                    "owned_bytes" => limits.owned_bytes -= 1,
                    _ => unreachable!(),
                }
                limited(&bytes, limits, label, offset);
            }
        }
        limited(&bytes, exact(minimum.bytes - 1, 0, 0, 0, 0, 0), "bytes", 0);
    }
}

#[test]
fn every_literal_byte_cut() {
    const BOUNDARIES: [&[usize]; 10] = [
        &[0, 2],
        &[0, 2, 4, 6, 8],
        &[0, 2, 4, 10, 14, 16, 18],
        &[0, 2, 6, 10, 12, 18, 20],
        &[0, 2, 4, 6, 8, 10, 12],
        &[0, 2, 16, 19, 22],
        &[0, 2, 4, 6, 11, 20, 23],
        &[0, 2, 6, 15],
        &[0, 2, 24],
        &[0, 6, 12, 18, 24],
    ];
    for (index, (text, (full, types, _, _))) in HEX.iter().zip(fixtures()).enumerate() {
        let bytes = hex(text);
        for cut in 0..=bytes.len() {
            let prefix = &bytes[..cut];
            if !BOUNDARIES[index].contains(&cut) {
                wire_error(prefix, Limits::default(), WireKind::Truncated, cut);
            } else if cut == 0 || matches!((index, cut), (1 | 2, 2) | (9, 6)) {
                error(prefix, Limits::default(), ErrorKind::MissingName, cut);
            } else {
                let parent = match index {
                    1 => (Some(6), 0, (cut >= 6).then_some(0), (cut >= 8).then_some(0)),
                    2 => (
                        Some(0),
                        5,
                        (cut >= 16).then_some(7),
                        (cut >= 18).then_some(8),
                    ),
                    3 => (None, if cut == 20 { 2 } else { 1 }, None, None),
                    9 => (
                        Some(-1),
                        i32::MIN,
                        (cut >= 18).then_some(i32::MAX),
                        (cut >= 24).then_some(0),
                    ),
                    _ => full,
                };
                let retained: Vec<_> = types
                    .iter()
                    .filter(|(_, span, _)| span.end <= cut)
                    .cloned()
                    .collect();
                check(prefix, Limits::default(), parent, &retained);
            }
        }
    }
}

fn wrap(tag: u8, body: &[u8]) -> Vec<u8> {
    assert!(body.len() < 128);
    let mut bytes = vec![0x10, 0, tag, body.len() as u8];
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn parent_first_and_cross_field_child_error_precedence() {
    error(&[], exact(0, 0, 0, 0, 0, 0), ErrorKind::MissingName, 0);
    error(
        &hex("22 01 80"),
        Limits {
            nodes: 0,
            depth: 0,
            ..Limits::default()
        },
        ErrorKind::MissingName,
        3,
    );
    error(&hex("12 00"), Limits::default(), ErrorKind::MissingName, 2);
    limited(
        &hex("22 00"),
        Limits {
            owned_bytes: 0,
            ..Limits::default()
        },
        "owned_bytes",
        0,
    );
    limited(&hex("10 00"), exact(2, 0, 0, 0, 0, 0), "records", 0);
    for (text, kind, offset) in [
        ("10 00 22 01 80 00", WireKind::InvalidFieldNumber, 5),
        ("22 01 80 00", WireKind::InvalidFieldNumber, 3),
        ("10 00 22 02 30 04 80", WireKind::Truncated, 7),
        ("10 00 22 02 ff", WireKind::Truncated, 5),
    ] {
        wire_error(
            &hex(text),
            Limits {
                nodes: 0,
                depth: 0,
                ..Limits::default()
            },
            kind,
            offset,
        );
    }
    for (text, kind, offset) in [
        ("10 00 22 01 80", WireKind::Truncated, 5),
        ("10 00 32 01 80", WireKind::Truncated, 5),
        ("10 00 22 01 80 28 00", WireKind::Truncated, 5),
        ("10 00 32 01 80 28 00", WireKind::Truncated, 5),
        ("10 00 32 01 00 22 01 80", WireKind::InvalidFieldNumber, 4),
        ("10 00 22 01 00 32 01 80", WireKind::InvalidFieldNumber, 4),
        ("10 00 22 01 80 32 01 00", WireKind::Truncated, 5),
        ("10 00 32 01 80 22 01 00", WireKind::Truncated, 5),
        ("10 00 22 02 30 04 32 01 80", WireKind::Truncated, 9),
        ("10 00 32 02 30 04 22 01 80", WireKind::Truncated, 9),
        ("10 00 22 00 32 01 80", WireKind::Truncated, 7),
        ("10 00 32 00 22 01 80", WireKind::Truncated, 7),
    ] {
        let bytes = hex(text);
        wire_error(&bytes, Limits::default(), kind, offset);
        for prefix in [hex("08 00"), hex("a0 06 00")] {
            let mut shifted = prefix.clone();
            shifted.extend_from_slice(&bytes);
            wire_error(&shifted, Limits::default(), kind, offset + prefix.len());
        }
    }
    // Child validation precedes its typed reservation; staging still precedes child validation.
    wire_error(
        &hex("10 00 22 01 80"),
        Limits {
            owned_bytes: P,
            ..Limits::default()
        },
        WireKind::Truncated,
        5,
    );
    limited(
        &hex("10 00 22 01 80"),
        Limits {
            owned_bytes: P - 1,
            ..Limits::default()
        },
        "owned_bytes",
        2,
    );
}

#[test]
fn child_structural_errors_rebase_once_and_parent_tails_stay_absolute() {
    let mut cases = vec![
        (hex("30"), WireKind::Truncated, 1),
        (hex("80"), WireKind::Truncated, 1),
        (hex("30 80"), WireKind::Truncated, 2),
        (hex("12 02 ff"), WireKind::Truncated, 3),
        (hex("0d 01 02 03"), WireKind::Truncated, 4),
        (hex("09 01 02 03 04 05 06 07"), WireKind::Truncated, 8),
        (hex("12 80 80 80 80 08"), WireKind::NegativeLength, 1),
        (hex("12 ff ff ff ff 07"), WireKind::Truncated, 6),
        (vec![0x80; 10], WireKind::MalformedVarint, 9),
    ];
    for byte in [0x00, 0x03, 0x07] {
        cases.push((vec![byte], WireKind::InvalidFieldNumber, 0));
    }
    for byte in [0x0b, 0x0c, 0x23, 0x24, 0x33, 0x34] {
        cases.push((vec![byte], WireKind::UnsupportedGroup, 0));
    }
    for byte in [0x0e, 0x0f, 0x26, 0x27, 0x36, 0x37] {
        cases.push((vec![byte], WireKind::InvalidWireType, 0));
    }
    for tag in [0x30, 0x12] {
        let mut body = vec![tag];
        body.extend_from_slice(&[0x80; 10]);
        cases.push((body, WireKind::MalformedVarint, 10));
    }
    for (body, kind, offset) in cases {
        let mut tail = vec![0x10, 0];
        tail.extend_from_slice(&body);
        wire_error(&tail, Limits::default(), kind, offset + 2);
        for tag in [0x22, 0x32] {
            let bytes = wrap(tag, &body);
            wire_error(&bytes, Limits::default(), kind, offset + 4);
            for prefix in [hex("08 00"), hex("a0 06 00")] {
                let mut shifted = prefix.clone();
                shifted.extend_from_slice(&bytes);
                wire_error(&shifted, Limits::default(), kind, offset + 4 + prefix.len());
            }
        }
    }
    for tag in [0x08, 0x18, 0x20, 0x30, 0x38, 0x40, 0x48, 0x58, 0x60, 0x70] {
        for n in 1..=10 {
            let mut body = vec![tag];
            body.extend(std::iter::repeat_n(0x80, n));
            for outer in [0x22, 0x32] {
                wire_error(
                    &wrap(outer, &body),
                    Limits::default(),
                    if n == 10 {
                        WireKind::MalformedVarint
                    } else {
                        WireKind::Truncated
                    },
                    if n == 10 { 14 } else { 5 + n },
                );
            }
        }
    }
}

#[test]
fn raw_parent_remains_opaque_and_occurrences_remain_unmerged() {
    let bytes = hex(include_str!("fixtures/metadata_type_alias/inline.hex"));
    let parent = type_alias::decode(&bytes, type_alias::Limits::default()).unwrap();
    assert_eq!(
        (
            parent.flags,
            parent.name,
            parent.underlying_type_id,
            parent.expanded_type_id
        ),
        EMPTY
    );
    assert_eq!(parent.raw, bytes);
    assert_eq!(parent.raw.as_ptr(), bytes.as_ptr());
    for (actual, ranges) in [
        (&parent.underlying_types, vec![4..6, 11..11, 13..14]),
        (&parent.expanded_types, vec![8..9, 16..16]),
    ] {
        assert_eq!(actual.len(), ranges.len());
        for (raw, span) in actual.iter().zip(ranges) {
            assert_eq!(*raw, &bytes[span.clone()]);
            assert_eq!(raw.as_ptr(), bytes[span].as_ptr());
        }
    }
    wire_error(&bytes, Limits::default(), WireKind::Truncated, 9);
    for text in [
        "10 00 22 01 80",
        "10 00 32 01 80",
        "10 00 32 01 00 22 01 80",
    ] {
        let bytes = hex(text);
        let parent = type_alias::decode(&bytes, type_alias::Limits::default()).unwrap();
        assert_eq!(parent.name, 0);
        assert_eq!(parent.raw.as_ptr(), bytes.as_ptr());
    }
    check(
        &hex("10 00 28 01 22 00 38 02 32 00"),
        Limits::default(),
        (None, 0, Some(1), Some(2)),
        &[(4, 6..6, Z), (6, 10..10, Z)],
    );
    check(
        &hex("10 00 32 00 38 02 22 00 28 01"),
        Limits::default(),
        (None, 0, Some(1), Some(2)),
        &[(6, 4..4, Z), (4, 8..8, Z)],
    );
    check(
        &hex("10 00 22 02 30 04 32 02 30 04 22 02 30 04"),
        Limits::default(),
        EMPTY,
        &[
            (4, 4..6, one(3, 4)),
            (6, 8..10, one(3, 4)),
            (4, 12..14, one(3, 4)),
        ],
    );
}

const VALUES: [(&str, i32, i32); 8] = [
    ("00", 0, 0),
    ("80 00", 0, 0),
    ("ff ff ff ff 0f", -1, 1),
    ("ff ff ff ff ff ff ff ff ff 01", -1, 1),
    ("ff ff ff ff ff ff ff ff ff 7f", -1, 1),
    ("80 80 80 80 08", i32::MIN, 1),
    ("ff ff ff ff 07", i32::MAX, 1),
    ("80 80 80 80 10", 0, 1),
];
fn parent_value(tag: u8, value: i32) -> Scalars {
    match tag {
        0x08 => (Some(value), 0, None, None),
        0x10 => (None, value, None, None),
        0x28 => (None, 0, Some(value), None),
        0x38 => (None, 0, None, Some(value)),
        _ => unreachable!(),
    }
}
#[test]
fn parent_signed_presence_raw64_and_duplicate_scalars() {
    for tag in [0x08, 0x10, 0x28, 0x38] {
        for (text, expected, _) in
            VALUES
                .into_iter()
                .chain([("80 80 80 80 80 80 80 80 80 02", 0, 0)])
        {
            let mut bytes = vec![0x10, 0, tag];
            bytes.extend(hex(text));
            check(&bytes, Limits::default(), parent_value(tag, expected), &[]);
        }
        check(
            &[0x10, 0, tag, 0, tag, 1, tag, 0],
            Limits::default(),
            parent_value(tag, 0),
            &[],
        );
    }
}

#[test]
fn child_scalar_matrix_bool_raw64_and_nonminimal_tags() {
    for (index, tag) in [0x08, 0x18, 0x20, 0x30, 0x38, 0x40, 0x48, 0x58, 0x60, 0x70]
        .into_iter()
        .enumerate()
    {
        for (text, integer, boolean) in VALUES
            .into_iter()
            .chain([("80 80 80 80 80 80 80 80 80 02", 0, 0), ("02", 2, 1)])
        {
            let mut body = vec![tag];
            body.extend(hex(text));
            for outer in [0x22, 0x32] {
                check(
                    &wrap(outer, &body),
                    Limits::default(),
                    EMPTY,
                    &[(
                        u32::from(outer >> 3),
                        4..4 + body.len(),
                        one(index, if index == 1 { boolean } else { integer }),
                    )],
                );
            }
        }
        for outer in [0x22, 0x32] {
            check(
                &wrap(outer, &[tag, 0, tag, 1, tag, 0]),
                Limits::default(),
                EMPTY,
                &[(u32::from(outer >> 3), 4..10, one(index, 0))],
            );
        }
    }
    for body in [hex("b0 00 84 00"), hex("b0 80 80 80 10 04")] {
        for outer in [0x22, 0x32] {
            check(
                &wrap(outer, &body),
                Limits::default(),
                EMPTY,
                &[(u32::from(outer >> 3), 4..4 + body.len(), one(3, 4))],
            );
        }
    }
    let mut all = Z;
    all[3] = Some(1);
    all[4] = Some(2);
    all[6] = Some(3);
    all[8] = Some(4);
    check(
        &wrap(0x22, &hex("30 01 38 02 48 03 60 04")),
        Limits::default(),
        EMPTY,
        &[(4, 4..12, all)],
    );
}

fn wrong_wire(tag: u8, wire: u8) -> Vec<u8> {
    let mut bytes = vec![tag | wire];
    match wire {
        0 => bytes.push(1),
        1 => bytes.extend_from_slice(&[0; 8]),
        2 => bytes.extend_from_slice(&[1, 0x80]),
        5 => bytes.extend_from_slice(&[0; 4]),
        _ => unreachable!(),
    }
    bytes
}
#[test]
fn wrong_wire_and_unknown_fields_stay_opaque() {
    for tag in [0x08, 0x10, 0x28, 0x38] {
        for wire in [1, 2, 5] {
            let wrong = wrong_wire(tag, wire);
            if tag == 0x10 {
                error(
                    &wrong,
                    Limits::default(),
                    ErrorKind::MissingName,
                    wrong.len(),
                );
            }
            for after in [false, true] {
                let mut bytes = vec![0x10, 0];
                if !after {
                    bytes.extend_from_slice(&wrong);
                }
                bytes.extend_from_slice(&[tag, 7]);
                if after {
                    bytes.extend_from_slice(&wrong);
                }
                check(&bytes, Limits::default(), parent_value(tag, 7), &[]);
            }
        }
    }
    for tag in [0x20, 0x30] {
        for wire in [0, 1, 5] {
            let wrong = wrong_wire(tag, wire);
            let mut alone = vec![0x10, 0];
            alone.extend_from_slice(&wrong);
            check(&alone, Limits::default(), EMPTY, &[]);
            for after in [false, true] {
                let mut bytes = vec![0x10, 0];
                if !after {
                    bytes.extend_from_slice(&wrong);
                }
                let start = bytes.len() + 2;
                bytes.extend_from_slice(&[tag | 2, 2, 0x30, 4]);
                if after {
                    bytes.extend_from_slice(&wrong);
                }
                check(
                    &bytes,
                    Limits::default(),
                    EMPTY,
                    &[(u32::from(tag >> 3), start..start + 2, one(3, 4))],
                );
            }
        }
    }
    for (index, tag) in [0x08, 0x18, 0x20, 0x30, 0x38, 0x40, 0x48, 0x58, 0x60, 0x70]
        .into_iter()
        .enumerate()
    {
        for wire in [1, 2, 5] {
            let wrong = wrong_wire(tag, wire);
            for outer in [0x22, 0x32] {
                check(
                    &wrap(outer, &wrong),
                    Limits::default(),
                    EMPTY,
                    &[(u32::from(outer >> 3), 4..4 + wrong.len(), Z)],
                );
                for after in [false, true] {
                    let mut body = Vec::new();
                    if !after {
                        body.extend_from_slice(&wrong);
                    }
                    body.extend_from_slice(&[tag, 1]);
                    if after {
                        body.extend_from_slice(&wrong);
                    }
                    check(
                        &wrap(outer, &body),
                        Limits::default(),
                        EMPTY,
                        &[(u32::from(outer >> 3), 4..4 + body.len(), one(index, 1))],
                    );
                }
            }
        }
    }
    for raw in [
        hex("a0 06 01"),
        hex("a1 06 01 02 03 04 05 06 07 08"),
        hex("a2 06 03 ff 00 0b"),
        hex("a5 06 01 02 03 04"),
        hex("c0 a3 09 01"),
        hex("f8 ff ff ff 0f 01"),
    ] {
        let mut parent = vec![0x10, 0];
        parent.extend_from_slice(&raw);
        check(&parent, Limits::default(), EMPTY, &[]);
        for outer in [0x22, 0x32] {
            check(
                &wrap(outer, &raw),
                Limits::default(),
                EMPTY,
                &[(u32::from(outer >> 3), 4..4 + raw.len(), Z)],
            );
        }
    }
}

#[test]
fn tight_charge_order_staging_overlap_and_body_start_offsets() {
    let bytes = hex("10 00 22 00");
    check(
        &bytes,
        exact(4, 2, 2, 2, 11 + P + S, P + S),
        EMPTY,
        &[(4, 4..4, Z)],
    );
    for (work, offset) in [(8 + P, 0), (9 + P, 4), (10 + P, 4), (11 + P, 4)] {
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
    for (limits, label, offset) in [
        (
            Limits {
                owned_bytes: P - 1,
                ..Limits::default()
            },
            "owned_bytes",
            2,
        ),
        (
            Limits {
                owned_bytes: P,
                ..Limits::default()
            },
            "owned_bytes",
            4,
        ),
        (
            Limits {
                owned_bytes: P,
                work: 11 + P,
                ..Limits::default()
            },
            "owned_bytes",
            4,
        ),
        (
            Limits {
                records: 1,
                ..Limits::default()
            },
            "records",
            2,
        ),
        (
            Limits {
                nodes: 0,
                ..Limits::default()
            },
            "nodes",
            0,
        ),
        (
            Limits {
                nodes: 1,
                ..Limits::default()
            },
            "nodes",
            4,
        ),
        (
            Limits {
                depth: 0,
                ..Limits::default()
            },
            "depth",
            0,
        ),
        (
            Limits {
                depth: 1,
                ..Limits::default()
            },
            "depth",
            4,
        ),
        (
            Limits {
                records: 2,
                nodes: 1,
                ..Limits::default()
            },
            "nodes",
            4,
        ),
    ] {
        limited(&bytes, limits, label, offset);
    }
    let both = hex("10 00 22 00 32 00");
    check(
        &both,
        exact(6, 3, 3, 2, 18 + 2 * P + 2 * S, 2 * P + 2 * S),
        EMPTY,
        &[(4, 4..4, Z), (6, 6..6, Z)],
    );
    for work in [13 + 2 * P, 14 + 2 * P, 15 + 2 * P, 16 + 2 * P] {
        limited(
            &both,
            Limits {
                work,
                ..Limits::default()
            },
            "work",
            4,
        );
    }
    limited(
        &both,
        Limits {
            owned_bytes: 2 * P + S,
            ..Limits::default()
        },
        "owned_bytes",
        6,
    );
    check(
        &hex("10 00 22 00 22 00"),
        exact(6, 3, 3, 2, 17 + 2 * P + 2 * S, 2 * P + 2 * S),
        EMPTY,
        &[(4, 4..4, Z), (4, 6..6, Z)],
    );
    for (work, offset) in [(0, 0), (1, 0), (2, 1), (3, 0), (4, 0)] {
        limited(
            &hex("10 00"),
            Limits {
                work,
                ..Limits::default()
            },
            "work",
            offset,
        );
    }
    limited(
        &hex("10 00"),
        Limits {
            depth: 0,
            nodes: 0,
            ..Limits::default()
        },
        "depth",
        0,
    );
    // Parent reading and its staging reservation precede every composition charge.
    limited(
        &bytes,
        Limits {
            owned_bytes: P - 1,
            nodes: 0,
            depth: 0,
            work: 8,
            ..Limits::default()
        },
        "owned_bytes",
        2,
    );
    limited(
        &bytes,
        Limits {
            owned_bytes: P,
            nodes: 0,
            depth: 0,
            work: 8,
            ..Limits::default()
        },
        "work",
        2,
    );
    wire_error(
        &hex("10 00 22 02 ff"),
        Limits {
            work: 8,
            ..Limits::default()
        },
        WireKind::Limit("work"),
        4,
    );
    wire_error(
        &hex("10 00 22 02 ff"),
        Limits {
            work: 9,
            ..Limits::default()
        },
        WireKind::Truncated,
        5,
    );
}
fn q(n: usize) -> usize {
    if n == 0 { 0 } else { n.next_power_of_two() }
}
fn empty_sequence(tags: &[u8], comparisons: usize, owned: usize, work: usize) {
    let k = tags.len();
    let mut bytes = vec![0x10, 0];
    let mut expected = Vec::new();
    for (index, &tag) in tags.iter().enumerate() {
        assert!(matches!(tag, 0x22 | 0x32));
        bytes.extend_from_slice(&[tag, 0]);
        expected.push((u32::from(tag >> 3), 4 + 2 * index..4 + 2 * index, Z));
    }
    assert_eq!(work, 5 + 6 * k + comparisons + owned);
    let minimum = exact(2 + 2 * k, 1 + k, 1 + k, 2, work, owned);
    check(&bytes, minimum, EMPTY, &expected);
    check(
        &bytes,
        exact(
            minimum.bytes + 1,
            minimum.records + 1,
            minimum.nodes + 1,
            3,
            work + 1,
            owned + 1,
        ),
        EMPTY,
        &expected,
    );
    for (limits, label, offset) in [
        (
            Limits {
                records: k,
                ..Limits::default()
            },
            "records",
            2 * k,
        ),
        (
            Limits {
                nodes: k,
                ..Limits::default()
            },
            "nodes",
            2 + 2 * k,
        ),
        (
            Limits {
                depth: 1,
                ..Limits::default()
            },
            "depth",
            4,
        ),
        (
            Limits {
                work: work - 1,
                ..Limits::default()
            },
            "work",
            2 + 2 * k,
        ),
        (
            Limits {
                owned_bytes: owned - 1,
                ..Limits::default()
            },
            "owned_bytes",
            if k == 1 { 4 } else { 4 + q(k) },
        ),
    ] {
        limited(&bytes, limits, label, offset);
    }
}
#[test]
fn monotonic_growth_across_each_staging_list_and_one_typed_vector() {
    for k in [1, 2, 3, 4, 5, 8, 9] {
        for tag in [0x22, 0x32] {
            let owned = q(k) * (P + S);
            let tags = vec![tag; k];
            empty_sequence(&tags, 0, owned, 5 + 6 * k + owned);
            let mut bytes = vec![0x10, 0];
            for _ in 0..k {
                bytes.extend_from_slice(&[tag, 0]);
            }
            limited(
                &bytes,
                Limits {
                    owned_bytes: q(k) * P - 1,
                    ..Limits::default()
                },
                "owned_bytes",
                if k == 1 { 2 } else { 2 + q(k) },
            );
            limited(
                &bytes,
                Limits {
                    owned_bytes: q(k) * P,
                    ..Limits::default()
                },
                "owned_bytes",
                4,
            );
        }
        for reverse in [false, true] {
            let (first, second) = if reverse { (0x32, 0x22) } else { (0x22, 0x32) };
            let mut tags = vec![first; k];
            tags.extend(std::iter::repeat_n(second, k));
            let owned = 2 * q(k) * P + q(2 * k) * S;
            empty_sequence(&tags, k, owned, 5 + 13 * k + owned);
        }
    }
    let mut unequal = vec![0x22; 9];
    unequal.extend_from_slice(&[0x32; 5]);
    empty_sequence(&unequal, 9, 24 * P + 16 * S, 98 + 24 * P + 16 * S);
    unequal.reverse();
    empty_sequence(&unequal, 5, 24 * P + 16 * S, 94 + 24 * P + 16 * S);
    for k in [1usize, 2, 3, 4, 5, 8, 9, 17, 18] {
        let tags: Vec<_> = (0..k)
            .map(|i| if i % 2 == 0 { 0x22 } else { 0x32 })
            .collect();
        let owned = (q(k.div_ceil(2)) + q(k / 2)) * P + q(k) * S;
        empty_sequence(&tags, k - 1, owned, 5 + 6 * k + (k - 1) + owned);
    }
}

#[test]
fn hard_byte_and_record_caps_cannot_be_raised() {
    let unlimited = exact(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    limited(&vec![0; MAX_BYTES + 1], unlimited, "bytes", 0);
    limited(
        &vec![0; MAX_BYTES + 1],
        exact(usize::MAX, 0, 0, 0, 0, 0),
        "bytes",
        0,
    );
    let mut bytes = [0x10, 0].repeat(MAX_RECORDS);
    check(&bytes, exact(131070, 65535, 1, 1, 262141, 0), EMPTY, &[]);
    check(&bytes, unlimited, EMPTY, &[]);
    bytes.extend_from_slice(&[0x10, 0]);
    limited(&bytes, unlimited, "records", 131070);
}

fn hostile(bytes: &[u8], wrapped: bool) {
    let limits = exact(32, 16, 8, 2, 256, 4096);
    let first = decode(bytes, limits);
    assert_eq!(first, decode(bytes, limits), "{bytes:02x?}");
    match first {
        Ok(out) => {
            borrowed(bytes, &out);
            if wrapped {
                assert_eq!(scalars(&out), EMPTY);
                assert_eq!(out.types.len(), 1);
                assert_eq!(out.types[0].field_number, u32::from(bytes[2] >> 3));
                assert_eq!(out.types[0].span, 4..6);
            }
        }
        Err(error) => {
            assert!(error.offset <= bytes.len());
            if wrapped {
                if error.kind == ErrorKind::Wire(WireKind::Limit("allocation")) {
                    assert!(matches!(error.offset, 2 | 4));
                } else {
                    assert!((4..=6).contains(&error.offset));
                }
            }
        }
    }
}
#[test]
fn finite_hostile_parent_and_wrapped_child_inputs() {
    let mut inputs = 0;
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            hostile(&[a, b], false);
            inputs += 1;
            hostile(&[0x10, 0, 0x22, 2, a, b], true);
            inputs += 1;
            hostile(&[0x10, 0, 0x32, 2, a, b], true);
            inputs += 1;
        }
        for n in 0..16 {
            hostile(&vec![a; n], false);
            inputs += 1;
        }
    }
    assert_eq!(inputs, 200704);
}
