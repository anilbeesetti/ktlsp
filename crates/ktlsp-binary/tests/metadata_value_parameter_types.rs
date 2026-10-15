use ktlsp_binary::metadata::{
    self, constructor,
    type_header::TypeHeader,
    value_parameter,
    value_parameter_types::{
        Error, ErrorKind, Limits, SelectedType, ValueParameterWithTypes, decode,
    },
    wire::{self, ErrorKind as WireKind, Value},
};
use std::mem::size_of;

const S: usize = size_of::<SelectedType<'_>>();
type Parent = (Option<i32>, i32, Option<i32>, Option<i32>);
type Child = (u32, usize, usize, [Option<i32>; 10]);

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn one(index: usize, value: i32) -> [Option<i32>; 10] {
    let mut out = [None; 10];
    out[index] = Some(value);
    out
}
fn scalars(out: &TypeHeader<'_>) -> [Option<i32>; 10] {
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
fn borrows(out: &ValueParameterWithTypes<'_>, bytes: &[u8]) {
    assert_eq!(out.parameter.raw, bytes);
    assert_eq!(out.parameter.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for child in &out.types {
        assert!(matches!(child.field_number, 3 | 4));
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
fn check(bytes: &[u8], limits: Limits, parent: Parent, types: &[Child]) {
    let out = decode(bytes, limits).unwrap();
    borrows(&out, bytes);
    let p = &out.parameter;
    assert_eq!(
        (p.flags, p.name, p.type_id, p.vararg_element_type_id),
        parent
    );
    assert_eq!(out.types.len(), types.len());
    for (child, &(number, start, end, expected)) in out.types.iter().zip(types) {
        assert_eq!(child.field_number, number);
        assert_eq!(child.span, start..end);
        assert_eq!(scalars(&child.header), expected);
    }
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
    types: Vec<Child>,
    records: usize,
    work: usize,
    owned: usize,
    last: usize,
    storage: usize,
    boundaries: &'static [usize],
}
fn fixtures() -> Vec<Fixture> {
    let mut nullable_class = one(3, 2);
    nullable_class[1] = Some(1);
    vec![
        Fixture {
            text: include_str!("fixtures/metadata_value_parameter_types/minimal.hex"),
            parent: (None, 0, None, None),
            types: vec![],
            records: 2,
            work: 9,
            owned: 0,
            last: 0,
            storage: 0,
            boundaries: &[2],
        },
        Fixture {
            text: include_str!("fixtures/metadata_value_parameter_types/selected.hex"),
            parent: (Some(0), 5, Some(7), Some(8)),
            types: vec![(3, 6, 10, nullable_class), (4, 12, 14, one(4, 3))],
            records: 15,
            work: 75 + 2 * S,
            owned: 2 * S,
            last: 16,
            storage: 10,
            boundaries: &[4, 10, 14, 16, 18],
        },
        Fixture {
            text: include_str!("fixtures/metadata_value_parameter_types/duplicates.hex"),
            parent: (None, 2, None, None),
            types: vec![
                (4, 4, 6, one(4, 1)),
                (3, 8, 10, one(3, 2)),
                (4, 12, 12, [None; 10]),
                (3, 14, 18, one(3, 4)),
            ],
            records: 16,
            work: 85 + 4 * S,
            owned: 4 * S,
            last: 18,
            storage: 10,
            boundaries: &[2, 6, 10, 12, 18, 20],
        },
        Fixture {
            text: include_str!("fixtures/metadata_value_parameter_types/opaque.hex"),
            parent: (None, 5, None, None),
            types: vec![(3, 4, 16, [None; 10])],
            records: 12,
            work: 82 + S,
            owned: S,
            last: 19,
            storage: 2,
            boundaries: &[2, 16, 19, 22],
        },
        Fixture {
            text: include_str!("fixtures/metadata_value_parameter_types/wrong_wire.hex"),
            parent: (None, 5, None, None),
            types: vec![],
            records: 12,
            work: 71,
            owned: 0,
            last: 20,
            storage: 0,
            boundaries: &[2, 4, 6, 11, 20, 23],
        },
        Fixture {
            text: "10 05 1a 02 38 00",
            parent: (None, 5, None, None),
            types: vec![(3, 4, 6, one(4, 0))],
            records: 5,
            work: 26 + S,
            owned: S,
            last: 4,
            storage: 2,
            boundaries: &[2, 6],
        },
        Fixture {
            text: "10 06 1a 02 38 01",
            parent: (None, 6, None, None),
            types: vec![(3, 4, 6, one(4, 1))],
            records: 5,
            work: 26 + S,
            owned: S,
            last: 4,
            storage: 2,
            boundaries: &[2, 6],
        },
    ]
}

#[test]
fn frozen_fixtures_and_all_aggregate_limits() {
    for f in fixtures() {
        let bytes = hex(f.text);
        for extra in [0, 1] {
            check(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: f.records + extra,
                    nodes: 1 + f.types.len() + extra,
                    depth: if f.types.is_empty() {
                        1 + extra
                    } else {
                        2 + extra
                    },
                    work: f.work + extra,
                    owned_bytes: f.owned + extra,
                },
                f.parent,
                &f.types,
            );
        }
        limited(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                nodes: 0,
                depth: 0,
                work: 0,
                owned_bytes: 0,
            },
            "bytes",
            0,
        );
        limited(
            &bytes,
            Limits {
                records: f.records - 1,
                ..Default::default()
            },
            "records",
            f.last,
        );
        let work_offset = if bytes.len() == 6 { 2 } else { f.last };
        limited(
            &bytes,
            Limits {
                work: f.work - 1,
                ..Default::default()
            },
            "work",
            work_offset,
        );
        if let Some(last) = f.types.last() {
            limited(
                &bytes,
                Limits {
                    nodes: f.types.len(),
                    ..Default::default()
                },
                "nodes",
                last.1,
            );
            limited(
                &bytes,
                Limits {
                    depth: 1,
                    ..Default::default()
                },
                "depth",
                f.types[0].1,
            );
            limited(
                &bytes,
                Limits {
                    owned_bytes: f.owned - 1,
                    ..Default::default()
                },
                "owned_bytes",
                f.storage,
            );
        } else {
            for (limits, label) in [
                (
                    Limits {
                        depth: 0,
                        ..Default::default()
                    },
                    "depth",
                ),
                (
                    Limits {
                        nodes: 0,
                        ..Default::default()
                    },
                    "nodes",
                ),
            ] {
                limited(&bytes, limits, label, 0);
            }
        }
    }
}

#[test]
fn all_104_frozen_prefix_cuts() {
    let mut cuts = 0;
    for (index, f) in fixtures().into_iter().enumerate() {
        let bytes = hex(f.text);
        for cut in 0..=bytes.len() {
            cuts += 1;
            if cut == 0 || (index == 1 && cut == 2) {
                error(
                    &bytes[..cut],
                    Default::default(),
                    ErrorKind::MissingName,
                    cut,
                );
            } else if f.boundaries.contains(&cut) {
                let mut parent = f.parent;
                if index == 1 {
                    parent.2 = if cut >= 16 { Some(7) } else { None };
                    parent.3 = if cut >= 18 { Some(8) } else { None };
                }
                if index == 2 && cut < 20 {
                    parent.1 = 1;
                }
                let types: Vec<_> = f.types.iter().copied().filter(|t| t.2 <= cut).collect();
                check(&bytes[..cut], Default::default(), parent, &types);
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
    assert_eq!(cuts, 104);
}

#[test]
fn both_fields_empty_duplicates_late_name_and_wrong_wires() {
    for (text, types, records, work, owned) in [
        (
            "10 00 22 02 38 01",
            vec![(4, 4, 6, one(4, 1))],
            5,
            26 + S,
            S,
        ),
        ("10 00 1a 00", vec![(3, 4, 4, [None; 10])], 4, 18 + S, S),
        ("10 00 22 00", vec![(4, 4, 4, [None; 10])], 4, 18 + S, S),
        (
            "10 00 1a 00 22 00",
            vec![(3, 4, 4, [None; 10]), (4, 6, 6, [None; 10])],
            6,
            27 + 2 * S,
            2 * S,
        ),
        (
            "10 00 1a 02 38 00 22 02 38 01",
            vec![(3, 4, 6, one(4, 0)), (4, 8, 10, one(4, 1))],
            8,
            43 + 2 * S,
            2 * S,
        ),
    ] {
        let bytes = hex(text);
        check(
            &bytes,
            Limits {
                bytes: bytes.len(),
                records,
                nodes: 1 + types.len(),
                depth: 2,
                work,
                owned_bytes: owned,
            },
            (None, 0, None, None),
            &types,
        );
    }
    check(
        &hex("1a 02 38 00 10 05"),
        Limits {
            bytes: 6,
            records: 5,
            nodes: 2,
            depth: 2,
            work: 26 + S,
            owned_bytes: S,
        },
        (None, 5, None, None),
        &[(3, 2, 4, one(4, 0))],
    );
    let baseline = hex("10 05 1a 02 38 00");
    for number in [3u8, 4] {
        for (wire, body) in [
            (0, "01"),
            (1, "00 00 00 00 00 00 00 00"),
            (5, "00 00 00 00"),
        ] {
            let wrong = hex(&format!("{:02x} {body}", (number << 3) | wire));
            check(
                &[hex("10 05"), wrong.clone()].concat(),
                Default::default(),
                (None, 5, None, None),
                &[],
            );
            for before in [true, false] {
                let bytes = if before {
                    [wrong.as_slice(), baseline.as_slice()].concat()
                } else {
                    [baseline.as_slice(), wrong.as_slice()].concat()
                };
                let shift = if before { wrong.len() } else { 0 };
                check(
                    &bytes,
                    Default::default(),
                    (None, 5, None, None),
                    &[(3, 4 + shift, 6 + shift, one(4, 0))],
                );
            }
        }
        let mut conflict = one(3, 3);
        conflict[4] = Some(2);
        let bytes = [vec![0x10, 0, number << 3 | 2, 6], hex("30 01 38 02 30 03")].concat();
        check(
            &bytes,
            Limits {
                bytes: 10,
                records: 7,
                nodes: 2,
                depth: 2,
                work: 42 + S,
                owned_bytes: S,
            },
            (None, 0, None, None),
            &[(number.into(), 4, 10, conflict)],
        );
        let bytes = vec![0x10, 0, number << 3 | 2, 0, number << 3 | 2, 2, 0x38, 1];
        check(
            &bytes,
            Default::default(),
            (None, 0, None, None),
            &[
                (number.into(), 4, 4, [None; 10]),
                (number.into(), 6, 8, one(4, 1)),
            ],
        );
    }
}

#[test]
fn signed_parent_ids_and_type_scalars_stay_uninterpreted() {
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
        for tag in [0x10, 0x28, 0x30] {
            let bytes = [hex("10 05 1a 02 38 00"), vec![tag], value_bytes.clone()].concat();
            let parent = match tag {
                0x10 => (None, value, None, None),
                0x28 => (None, 5, Some(value), None),
                _ => (None, 5, None, Some(value)),
            };
            check(&bytes, Default::default(), parent, &[(3, 4, 6, one(4, 0))]);
        }
        for (index, tag) in [8u8, 24, 32, 48, 56, 64, 72, 88, 96, 112]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != 1)
        {
            let body = [vec![tag], value_bytes.clone()].concat();
            for number in [3u8, 4] {
                let bytes = [
                    vec![0x10, 0, number << 3 | 2, body.len() as u8],
                    body.clone(),
                ]
                .concat();
                check(
                    &bytes,
                    Default::default(),
                    (None, 0, None, None),
                    &[(number.into(), 4, bytes.len(), one(index, value))],
                );
            }
        }
    }
    for number in [3u8, 4] {
        let bytes = [vec![0x10, 0, number << 3 | 2, 6], hex("18 80 80 80 80 10")].concat();
        check(
            &bytes,
            Default::default(),
            (None, 0, None, None),
            &[(number.into(), 4, 10, one(1, 1))],
        );
        let bytes = [
            vec![0x10, 0, number << 3 | 2, 12],
            hex("a0 06 01 f8 ff ff ff 0f 00 32 01 80"),
        ]
        .concat();
        check(
            &bytes,
            Default::default(),
            (None, 0, None, None),
            &[(number.into(), 4, 16, [None; 10])],
        );
    }
}

#[test]
fn parent_first_errors_and_child_offsets_never_bleed_or_double_rebase() {
    error(
        &hex("1a 01 80"),
        Default::default(),
        ErrorKind::MissingName,
        3,
    );
    for (text, kind, offset) in [
        ("10 05 1a 01 80 00", WireKind::InvalidFieldNumber, 5),
        ("10 05 1a 02 38 00 80", WireKind::Truncated, 7),
        ("10 05 1a 01 80", WireKind::Truncated, 5),
        ("10 05 1a 01 80 28 00", WireKind::Truncated, 5),
        ("10 05 1a 01 00", WireKind::InvalidFieldNumber, 4),
        ("10 05 1a 01 0b", WireKind::UnsupportedGroup, 4),
        ("10 05 1a 01 0e", WireKind::InvalidWireType, 4),
        ("10 05 1a 02 30 80", WireKind::Truncated, 6),
        ("10 05 1a 03 12 02 ff", WireKind::Truncated, 7),
        ("10 05 1a 06 12 80 80 80 80 08", WireKind::NegativeLength, 5),
        (
            "10 05 1a 0b 08 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            14,
        ),
        (
            "10 05 1a 0a 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            13,
        ),
        ("10 05 22 00 1a 01 80", WireKind::Truncated, 7),
        ("10 05 1a 02 ff", WireKind::Truncated, 5),
    ] {
        let original = hex(text);
        for number in [3u8, 4] {
            let mut bytes = original.clone();
            let tag = bytes.iter().position(|b| *b == 0x1a).unwrap();
            bytes[tag] = number << 3 | 2;
            for prefix in [vec![], hex("08 00"), hex("a0 06 00")] {
                error(
                    &[prefix.as_slice(), bytes.as_slice()].concat(),
                    Default::default(),
                    ErrorKind::Wire(kind),
                    offset + prefix.len(),
                );
            }
        }
    }
    error(&[], Default::default(), ErrorKind::MissingName, 0);
    limited(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            nodes: 0,
            depth: 0,
            work: 0,
            owned_bytes: 0,
        },
        "depth",
        0,
    );
    assert_eq!(
        value_parameter::decode(
            &[],
            value_parameter::Limits {
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
}

#[test]
fn exact_pair_work_trace_and_competing_limits() {
    let bytes = hex("10 05 1a 02 38 00");
    for (work, offset) in [
        (0, 0),
        (1, 0),
        (10, 2),
        (11, 0),
        (20, 2),
        (21, 4),
        (22, 4),
        (23, 4),
        (24, 5),
        (25, 4),
        (26, 2),
    ] {
        limited(
            &bytes,
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
                work: 26,
                owned_bytes: 0,
                ..Default::default()
            },
            "owned_bytes",
            2,
        ),
        (
            Limits {
                records: 0,
                work: 1,
                ..Default::default()
            },
            "records",
            0,
        ),
        (
            Limits {
                nodes: 0,
                depth: 0,
                ..Default::default()
            },
            "depth",
            0,
        ),
        (
            Limits {
                records: 4,
                nodes: 1,
                ..Default::default()
            },
            "nodes",
            4,
        ),
        (
            Limits {
                records: 4,
                work: 22,
                ..Default::default()
            },
            "records",
            4,
        ),
    ] {
        limited(&bytes, limits, label, offset);
    }
}

#[test]
fn both_fields_share_growth_and_hard_clamps() {
    for k in 1usize..=5 {
        let mut bytes = hex("10 00");
        let mut types = Vec::new();
        for i in 0..k {
            let number = if i % 2 == 0 { 3 } else { 4 };
            bytes.extend_from_slice(&[number << 3 | 2, 0]);
            types.push((number.into(), 4 + 2 * i, 4 + 2 * i, [None; 10]));
        }
        let q = k.next_power_of_two();
        let work = 9 + 9 * k + q * S;
        check(
            &bytes,
            Limits {
                bytes: bytes.len(),
                records: 2 + 2 * k,
                nodes: 1 + k,
                depth: 2,
                work,
                owned_bytes: q * S,
            },
            (None, 0, None, None),
            &types,
        );
        let growth_tag = 2 + q;
        let storage_offset = if k == 1 { 2 } else { growth_tag };
        limited(
            &bytes,
            Limits {
                owned_bytes: q * S - 1,
                ..Default::default()
            },
            "owned_bytes",
            storage_offset,
        );
        let work_offset = if k == 4 { 2 * k + 2 } else { 2 * k };
        limited(
            &bytes,
            Limits {
                work: work - 1,
                ..Default::default()
            },
            "work",
            work_offset,
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
    check(&hex("10 00"), max, (None, 0, None, None), &[]);
    limited(&vec![0; ktlsp_binary::MAX_BYTES + 1], max, "bytes", 0);
    let mut bytes = hex("10 00");
    for i in 0..32766 {
        bytes.extend_from_slice(&[if i % 2 == 0 { 0x1a } else { 0x22 }, 0]);
    }
    const { assert!(32768 * S <= ktlsp_binary::MAX_BYTES) };
    let out = decode(
        &bytes,
        Limits {
            bytes: 65534,
            records: 65534,
            nodes: 32767,
            depth: 2,
            work: 294903 + 32768 * S,
            owned_bytes: 32768 * S,
        },
    )
    .unwrap();
    assert_eq!(out.types.len(), 32766);
    borrows(&out, &bytes);
    bytes.extend_from_slice(&[0x1a, 0]);
    limited(&bytes, max, "records", 65534);
}

#[test]
fn genuine_pair_constructor_both_parameters_and_types() {
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
    let mut fields = framed
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 8 && matches!(f.value, Value::Bytes(_)));
    let field = fields.next().unwrap();
    assert!(fields.next().is_none());
    assert_eq!(field.span, 76..103);
    let Value::Bytes(body) = field.value else {
        unreachable!()
    };
    assert_eq!(body, &bytes[78..103]);
    assert_eq!(body.as_ptr(), bytes[78..103].as_ptr());
    let ctor = constructor::decode(body, Default::default()).unwrap();
    assert_eq!(ctor.flags, Some(70));
    assert!(ctor.version_requirements.is_empty());
    assert_eq!(ctor.value_parameters.len(), 2);
    for (index, (parameter, name, start, ctor_start, literal)) in [
        (&ctor.value_parameters[0], 5, 82, 4, "10 05 1a 02 38 00"),
        (&ctor.value_parameters[1], 6, 90, 12, "10 06 1a 02 38 01"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(parameter.span, ctor_start..ctor_start + 6);
        assert_eq!(parameter.raw, hex(literal));
        assert_eq!(parameter.raw.as_ptr(), bytes[start..start + 6].as_ptr());
        check(
            parameter.raw,
            Limits {
                bytes: 6,
                records: 5,
                nodes: 2,
                depth: 2,
                work: 26 + S,
                owned_bytes: S,
            },
            (None, name, None, None),
            &[(3, 4, 6, one(4, index as i32))],
        );
        let out = decode(parameter.raw, Default::default()).unwrap();
        assert_eq!(
            out.types[0].header.raw.as_ptr(),
            bytes[start + 4..start + 6].as_ptr()
        );
    }
}

#[test]
fn finite_hostile_parents_and_named_children_are_deterministic_and_bounded() {
    let limits = Limits {
        bytes: 32,
        records: 16,
        nodes: 8,
        depth: 2,
        work: 256,
        owned_bytes: 4096,
    };
    let probe = |bytes: &[u8], number: Option<u32>| {
        let a = decode(bytes, limits);
        let b = decode(bytes, limits);
        assert_eq!(a, b);
        match a {
            Ok(out) => {
                borrows(&out, bytes);
                if let Some(number) = number {
                    assert_eq!(out.parameter.name, 0);
                    assert_eq!(out.types.len(), 1);
                    assert_eq!(out.types[0].field_number, number);
                    assert_eq!(out.types[0].span, 4..6);
                }
            }
            Err(error) => {
                assert!(error.offset <= bytes.len());
                if number.is_some() {
                    assert!(
                        (4..=6).contains(&error.offset)
                            || error
                                == Error {
                                    offset: 2,
                                    kind: ErrorKind::Wire(WireKind::Limit("allocation"))
                                }
                    );
                }
            }
        }
    };
    for value in 0u32..=65535 {
        let a = (value >> 8) as u8;
        let b = value as u8;
        probe(&[a, b], None);
        for number in [3u8, 4] {
            probe(&[0x10, 0, number << 3 | 2, 2, a, b], Some(number.into()));
        }
    }
    for byte in 0u8..=255 {
        for len in 0..=15 {
            probe(&vec![byte; len], None);
        }
    }
}
