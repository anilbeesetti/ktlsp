use ktlsp_binary::metadata::{
    self,
    type_header::TypeHeader,
    type_parameter,
    type_parameter::Variance,
    type_parameter_bounds::{
        Error, ErrorKind, Limits, SelectedUpperBound, TypeParameterWithBounds, decode,
    },
    wire::{self, ErrorKind as WireKind, Value},
};
use std::mem::size_of;

const BASE: &[u8] = &[8, 0, 16, 0];
const S: usize = size_of::<type_parameter::UpperBound<'_>>();
const T: usize = size_of::<SelectedUpperBound<'_>>();
const Z: [Option<i32>; 10] = [None; 10];
type Parent = (i32, i32, Option<bool>, Option<Variance>);
type Child = (usize, usize, [Option<i32>; 10]);
const P: Parent = (0, 0, None, None);

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn tail(text: &str) -> Vec<u8> {
    [BASE, &hex(text)].concat()
}
fn one(index: usize, value: i32) -> [Option<i32>; 10] {
    let mut out = Z;
    out[index] = Some(value);
    out
}
fn borrows(out: &TypeParameterWithBounds<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for bound in &out.upper_bound {
        assert!(end <= bound.span.start && bound.span.start <= bound.span.end);
        assert!(bound.span.end <= bytes.len());
        assert_eq!(bound.header.raw, &bytes[bound.span.clone()]);
        assert_eq!(
            bound.header.raw.as_ptr(),
            bytes[bound.span.clone()].as_ptr()
        );
        end = bound.span.end;
    }
}
fn check(bytes: &[u8], limits: Limits, parent: Parent, ids: &[i32], bounds: &[Child]) {
    let out = decode(bytes, limits).unwrap();
    borrows(&out, bytes);
    let expected = TypeParameterWithBounds {
        raw: bytes,
        id: parent.0,
        name: parent.1,
        reified: parent.2,
        variance: parent.3,
        upper_bound_id: ids.to_vec(),
        upper_bound: bounds
            .iter()
            .map(|&(start, end, values)| SelectedUpperBound {
                span: start..end,
                header: TypeHeader {
                    raw: &bytes[start..end],
                    flags: values[0],
                    nullable: values[1].map(|v| v != 0),
                    flexible_type_capabilities_id: values[2],
                    class_name: values[3],
                    type_parameter: values[4],
                    flexible_upper_bound_id: values[5],
                    type_parameter_name: values[6],
                    outer_type_id: values[7],
                    type_alias_name: values[8],
                    abbreviated_type_id: values[9],
                },
            })
            .collect(),
    };
    assert_eq!(out, expected);
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
    text: &'static str,
    parent: Parent,
    ids: &'static [i32],
    bounds: Vec<Child>,
    records: usize,
    work: usize,
    owned: usize,
}
fn fixtures() -> Vec<Fixture> {
    let mut duplicate = one(3, 2);
    duplicate[1] = Some(0);
    vec![
        Fixture {
            text: include_str!("fixtures/metadata_type_parameter_bounds/minimal.hex"),
            parent: P,
            ids: &[],
            bounds: vec![],
            records: 2,
            work: 9,
            owned: 0,
        },
        Fixture {
            text: include_str!("fixtures/metadata_type_parameter_bounds/selected.hex"),
            parent: (-1, -2, Some(true), Some(Variance::Out)),
            ids: &[-1, 7, 0],
            bounds: vec![
                (
                    18,
                    46,
                    [
                        Some(-1),
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
                ),
                (48, 48, Z),
            ],
            records: 20,
            work: 134 + 2 * S + 2 * T + 16,
            owned: 2 * S + 2 * T + 16,
        },
        Fixture {
            text: include_str!("fixtures/metadata_type_parameter_bounds/duplicates.hex"),
            parent: (1, 2, Some(false), Some(Variance::Out)),
            ids: &[],
            bounds: vec![(16, 24, duplicate), (26, 28, one(3, 3))],
            records: 14,
            work: 69 + 2 * S + 2 * T,
            owned: 2 * S + 2 * T,
        },
        Fixture {
            text: include_str!("fixtures/metadata_type_parameter_bounds/opaque.hex"),
            parent: P,
            ids: &[],
            bounds: vec![(6, 18, Z)],
            records: 7,
            work: 46 + S + T,
            owned: S + T,
        },
        Fixture {
            text: include_str!("fixtures/metadata_type_parameter_bounds/wrong_wire.hex"),
            parent: P,
            ids: &[],
            bounds: vec![(21, 25, Z)],
            records: 11,
            work: 53 + S + T,
            owned: S + T,
        },
    ]
}

#[test]
fn frozen_full_tuples_pointers_and_aggregate_minima() {
    for f in fixtures() {
        let bytes = hex(f.text);
        for extra in [0, 1] {
            check(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: f.records + extra,
                    nodes: 1 + f.bounds.len() + extra,
                    depth: if f.bounds.is_empty() {
                        1 + extra
                    } else {
                        2 + extra
                    },
                    work: f.work + extra,
                    owned_bytes: f.owned + extra,
                },
                f.parent,
                f.ids,
                &f.bounds,
            );
        }
    }
}

#[test]
fn frozen_scalar_id_wrong_wire_and_opaque_controls() {
    for (text, ids) in [
        ("28 01", &[][..]),
        ("30 01 32 02 02 03 30 04 32 00", &[1, 2, 3, 4][..]),
        ("32 05 ff ff ff ff 0f", &[-1][..]),
    ] {
        check(&tail(text), Default::default(), P, ids, &[]);
    }
    check(
        &tail("2a 03 12 01 80"),
        Default::default(),
        P,
        &[],
        &[(6, 9, Z)],
    );
    check(
        &tail("2a 02 30 01"),
        Limits {
            bytes: 8,
            records: 4,
            nodes: 2,
            depth: 2,
            work: 20 + S + T,
            owned_bytes: S + T,
        },
        P,
        &[],
        &[(6, 8, one(3, 1))],
    );
    check(
        &hex("08 80 80 80 80 10 10 00"),
        Default::default(),
        P,
        &[],
        &[],
    );
    check(
        &hex("88 00 80 00 90 00 81 00"),
        Default::default(),
        (0, 1, None, None),
        &[],
        &[],
    );
    for (text, value) in [
        ("18 80 80 80 80 10", true),
        ("18 80 80 80 80 80 80 80 80 80 02", false),
    ] {
        check(
            &tail(text),
            Default::default(),
            (0, 0, Some(value), None),
            &[],
            &[],
        );
    }
    check(
        &tail("2a 0b 18 80 80 80 80 80 80 80 80 80 02"),
        Default::default(),
        P,
        &[],
        &[(6, 17, one(1, 0))],
    );
}

#[test]
fn frozen_parent_first_required_and_once_rebased_child_errors() {
    for (text, kind, offset) in [
        ("", ErrorKind::MissingId, 0),
        ("08 00", ErrorKind::MissingName, 2),
        ("10 00", ErrorKind::MissingId, 2),
        ("2a 01 80", ErrorKind::MissingId, 3),
        ("08 00 2a 01 80", ErrorKind::MissingName, 5),
        ("10 00 2a 01 80", ErrorKind::MissingId, 5),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
    for (text, kind, offset) in [
        ("08 00 10 00 2a 01 80", WireKind::Truncated, 7),
        ("08 00 10 00 2a 02 30 01 2a 01 80", WireKind::Truncated, 11),
        ("08 00 10 00 2a 01 80 00", WireKind::InvalidFieldNumber, 7),
        ("08 00 10 00 2a 01 80 30", WireKind::Truncated, 8),
        ("08 00 10 00 2a 01 80 32 01 80", WireKind::Truncated, 10),
        ("08 00 10 00 2a 01 80 0b", WireKind::UnsupportedGroup, 7),
        ("08 00 10 00 2a 02 01", WireKind::Truncated, 7),
        ("08 00 10 00 2a 01 00", WireKind::InvalidFieldNumber, 6),
        ("08 00 10 00 2a 01 07", WireKind::InvalidFieldNumber, 6),
        ("08 00 10 00 2a 01 0b", WireKind::UnsupportedGroup, 6),
        ("08 00 10 00 2a 01 0c", WireKind::UnsupportedGroup, 6),
        ("08 00 10 00 2a 01 0e", WireKind::InvalidWireType, 6),
        ("08 00 10 00 2a 01 0f", WireKind::InvalidWireType, 6),
        ("08 00 10 00 2a 01 30", WireKind::Truncated, 7),
        ("08 00 10 00 2a 03 32 02 01", WireKind::Truncated, 9),
        (
            "08 00 10 00 2a 06 32 80 80 80 80 08",
            WireKind::NegativeLength,
            7,
        ),
        (
            "08 00 10 00 2a 0b 30 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            16,
        ),
        ("08 00 10 00 2a 01 80 2a 01 00", WireKind::Truncated, 7),
    ] {
        error(
            &hex(text),
            Default::default(),
            ErrorKind::Wire(kind),
            offset,
        );
    }
    let bytes = tail("2a 01 80");
    let raw = type_parameter::decode(&bytes, Default::default()).unwrap();
    assert_eq!(
        (raw.id, raw.name, raw.reified, raw.variance),
        (0, 0, None, None)
    );
    assert!(raw.upper_bound_id.is_empty());
    assert_eq!(raw.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(
        raw.upper_bound,
        [type_parameter::UpperBound {
            raw: &bytes[6..7],
            span: 6..7
        }]
    );
    assert_eq!(raw.upper_bound[0].raw.as_ptr(), bytes[6..7].as_ptr());
}

#[test]
fn frozen_minimal_empty_bound_and_malformed_prefixes() {
    for (text, outcomes) in [
        ("08 00 10 00", &[0u8, 1, 2, 1, 3][..]),
        ("08 00 10 00 2a 00 2a 00", &[0, 1, 2, 1, 3, 1, 3, 1, 3][..]),
        ("08 00 10 00 2a 01 80", &[0, 1, 2, 1, 3, 1, 1, 1][..]),
    ] {
        let bytes = hex(text);
        for (cut, &outcome) in outcomes.iter().enumerate() {
            let prefix = &bytes[..cut];
            match outcome {
                0 => error(prefix, Default::default(), ErrorKind::MissingId, cut),
                1 => error(
                    prefix,
                    Default::default(),
                    ErrorKind::Wire(WireKind::Truncated),
                    cut,
                ),
                2 => error(prefix, Default::default(), ErrorKind::MissingName, cut),
                3 => {
                    let bounds: &[Child] = match cut {
                        6 => &[(6, 6, Z)],
                        8 => &[(6, 6, Z), (8, 8, Z)],
                        _ => &[],
                    };
                    check(prefix, Default::default(), P, &[], bounds);
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn frozen_one_less_and_competing_limits() {
    let single = tail("2a 00");
    let typed = tail("2a 02 30 01");
    let f = fixtures();
    let selected = hex(f[1].text);
    let duplicates = hex(f[2].text);
    let opaque = hex(f[3].text);
    let wrong = hex(f[4].text);
    let five = tail("2a 00 2a 00 2a 00 2a 00 2a 00");
    let ids = tail("30 00 30 00 30 00 30 00 30 00");
    for (bytes, limits, label, offset) in [
        (
            BASE,
            Limits {
                bytes: 3,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            BASE,
            Limits {
                records: 1,
                ..Default::default()
            },
            "records",
            2,
        ),
        (
            BASE,
            Limits {
                nodes: 0,
                ..Default::default()
            },
            "nodes",
            0,
        ),
        (
            BASE,
            Limits {
                depth: 0,
                ..Default::default()
            },
            "depth",
            0,
        ),
        (
            BASE,
            Limits {
                work: 8,
                ..Default::default()
            },
            "work",
            2,
        ),
        (
            BASE,
            Limits {
                work: 0,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            &single,
            Limits {
                records: 2,
                ..Default::default()
            },
            "records",
            4,
        ),
        (
            &single,
            Limits {
                nodes: 1,
                ..Default::default()
            },
            "nodes",
            6,
        ),
        (
            &single,
            Limits {
                depth: 1,
                ..Default::default()
            },
            "depth",
            6,
        ),
        (
            &single,
            Limits {
                owned_bytes: S - 1,
                ..Default::default()
            },
            "owned_bytes",
            4,
        ),
        (
            &single,
            Limits {
                owned_bytes: S + T - 1,
                ..Default::default()
            },
            "owned_bytes",
            6,
        ),
        (
            &single,
            Limits {
                work: 13 + S + T,
                ..Default::default()
            },
            "work",
            6,
        ),
        (
            &typed,
            Limits {
                records: 3,
                ..Default::default()
            },
            "records",
            6,
        ),
        (
            &selected,
            Limits {
                bytes: 58,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            &selected,
            Limits {
                records: 19,
                ..Default::default()
            },
            "records",
            44,
        ),
        (
            &selected,
            Limits {
                nodes: 2,
                ..Default::default()
            },
            "nodes",
            48,
        ),
        (
            &selected,
            Limits {
                depth: 1,
                ..Default::default()
            },
            "depth",
            18,
        ),
        (
            &selected,
            Limits {
                owned_bytes: 2 * S + 2 * T + 15,
                ..Default::default()
            },
            "owned_bytes",
            48,
        ),
        (
            &selected,
            Limits {
                work: 133 + 2 * S + 2 * T + 16,
                ..Default::default()
            },
            "work",
            48,
        ),
        (
            &duplicates,
            Limits {
                records: 13,
                ..Default::default()
            },
            "records",
            26,
        ),
        (
            &duplicates,
            Limits {
                nodes: 2,
                ..Default::default()
            },
            "nodes",
            26,
        ),
        (
            &duplicates,
            Limits {
                owned_bytes: 2 * S + 2 * T - 1,
                ..Default::default()
            },
            "owned_bytes",
            26,
        ),
        (
            &duplicates,
            Limits {
                work: 68 + 2 * S + 2 * T,
                ..Default::default()
            },
            "work",
            26,
        ),
        (
            &opaque,
            Limits {
                records: 6,
                ..Default::default()
            },
            "records",
            15,
        ),
        (
            &opaque,
            Limits {
                work: 45 + S + T,
                ..Default::default()
            },
            "work",
            6,
        ),
        (
            &opaque,
            Limits {
                owned_bytes: S + T - 1,
                ..Default::default()
            },
            "owned_bytes",
            6,
        ),
        (
            &wrong,
            Limits {
                records: 10,
                ..Default::default()
            },
            "records",
            23,
        ),
        (
            &wrong,
            Limits {
                work: 52 + S + T,
                ..Default::default()
            },
            "work",
            21,
        ),
        (
            &wrong,
            Limits {
                owned_bytes: S + T - 1,
                ..Default::default()
            },
            "owned_bytes",
            21,
        ),
        (
            &five,
            Limits {
                records: 6,
                ..Default::default()
            },
            "records",
            12,
        ),
        (
            &five,
            Limits {
                nodes: 5,
                ..Default::default()
            },
            "nodes",
            14,
        ),
        (
            &five,
            Limits {
                owned_bytes: 8 * S + 8 * T - 1,
                ..Default::default()
            },
            "owned_bytes",
            14,
        ),
        (
            &five,
            Limits {
                work: 33 + 8 * S + 8 * T,
                ..Default::default()
            },
            "work",
            14,
        ),
        (
            &ids,
            Limits {
                owned_bytes: 31,
                ..Default::default()
            },
            "owned_bytes",
            12,
        ),
        (
            &ids,
            Limits {
                work: 65,
                ..Default::default()
            },
            "work",
            12,
        ),
        (
            &single,
            Limits {
                depth: 1,
                nodes: 1,
                ..Default::default()
            },
            "depth",
            6,
        ),
        (
            &[],
            Limits {
                nodes: 0,
                ..Default::default()
            },
            "nodes",
            0,
        ),
        (
            BASE,
            Limits {
                bytes: 0,
                depth: 0,
                nodes: 0,
                work: 0,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            &tail("2a 01 80"),
            Limits {
                owned_bytes: S - 1,
                ..Default::default()
            },
            "owned_bytes",
            4,
        ),
    ] {
        limited(bytes, limits, label, offset);
    }
}

#[test]
fn frozen_growth_and_hard_clamps() {
    check(
        &tail("2a 00"),
        Limits {
            bytes: 6,
            records: 3,
            nodes: 2,
            depth: 2,
            work: 14 + S + T,
            owned_bytes: S + T,
        },
        P,
        &[],
        &[(6, 6, Z)],
    );
    for (n, capacity, base_work) in [(2, 2, 19), (3, 4, 24), (5, 8, 34)] {
        let mut bytes = BASE.to_vec();
        let mut bounds = Vec::new();
        for i in 0..n {
            bytes.extend_from_slice(&[0x2a, 0]);
            bounds.push((6 + 2 * i, 6 + 2 * i, Z));
        }
        let owned = capacity * (S + T);
        check(
            &bytes,
            Limits {
                bytes: bytes.len(),
                records: 2 + n,
                nodes: 1 + n,
                depth: 2,
                work: base_work + owned,
                owned_bytes: owned,
            },
            P,
            &[],
            &bounds,
        );
        let mut bytes = BASE.to_vec();
        for _ in 0..n {
            bytes.extend_from_slice(&[0x30, 0]);
        }
        check(
            &bytes,
            Limits {
                bytes: bytes.len(),
                records: 2 + n,
                nodes: 1,
                depth: 1,
                work: 9 + 5 * n + capacity * 4,
                owned_bytes: capacity * 4,
            },
            P,
            &vec![0; n],
            &[],
        );
    }
    let max = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for index in [0, 1] {
        let f = &fixtures()[index];
        check(&hex(f.text), max, f.parent, f.ids, &f.bounds);
    }
    limited(&vec![0; ktlsp_binary::MAX_BYTES + 1], max, "bytes", 0);
    let mut bytes = BASE.to_vec();
    for _ in 0..65_533 {
        bytes.extend_from_slice(&[0x38, 0]);
    }
    check(
        &bytes,
        Limits {
            bytes: 131070,
            records: 65535,
            nodes: 1,
            depth: 1,
            work: 262141,
            owned_bytes: 0,
        },
        P,
        &[],
        &[],
    );
    bytes.extend_from_slice(&[0x38, 0]);
    limited(&bytes, max, "records", 131070);
}

#[test]
fn genuine_pair_pipeline_is_a_zero_inline_bound_control_only() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata")
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload.span, 47..285);
    let parameters: Vec<_> = framed
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 5 && matches!(f.value, Value::Bytes(_)))
        .collect();
    assert_eq!(parameters.len(), 2);
    for (field, start, id, name, literal) in [
        (parameters[0], 54, 0, 1, "08 00 10 01 20 01"),
        (parameters[1], 62, 1, 2, "08 01 10 02 20 01"),
    ] {
        assert_eq!(field.span, start - 2..start + 6);
        let Value::Bytes(body) = field.value else {
            unreachable!()
        };
        assert_eq!(body, hex(literal));
        assert_eq!(body.as_ptr(), bytes[start..start + 6].as_ptr());
        check(
            body,
            Limits {
                bytes: 6,
                records: 3,
                nodes: 1,
                depth: 1,
                work: 13,
                owned_bytes: 0,
            },
            (id, name, None, Some(Variance::Out)),
            &[],
            &[],
        );
    }
}

#[test]
fn finite_hostile_inputs_are_deterministic_and_borrow_bounded_spans() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        nodes: 4,
        depth: 2,
        work: 256,
        owned_bytes: 512,
    };
    let probe = |bytes: &[u8]| {
        let out = decode(bytes, limits);
        assert_eq!(out, decode(bytes, limits));
        match out {
            Ok(out) => borrows(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0u8..=255 {
        for b in 0u8..=255 {
            probe(&[a, b]);
            probe(&[8, 0, 16, 0, a, b]);
        }
        for n in 0..32 {
            probe(&vec![a; n]);
        }
    }
}
