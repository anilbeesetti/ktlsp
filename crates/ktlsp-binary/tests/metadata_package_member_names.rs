use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, function_header, name_resolver,
        package_member_names::{Error, ErrorKind, Kind, Limits, Member, decode},
        payload, property_header, string_table, type_alias, wire,
    },
};
use std::mem::size_of;

static RAW: Annotation<'static> = Annotation {
    descriptor: &[],
    elements: Vec::new(),
};
const D2: &[&[u16]] = &[&[102], &[112], &[97], &[102]];
const SELECTORS: [(u8, Kind); 3] = [
    (0x1a, Kind::Function),
    (0x22, Kind::Property),
    (0x2a, Kind::TypeAlias),
];
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn input<'a>(bytes: Vec<u8>, d2: &[&'a [u16]], kind: i32) -> metadata::Outcome<'a> {
    metadata::Outcome::Decoded {
        bytes,
        header: metadata::Header {
            raw: &RAW,
            visibility: &Visibility::RuntimeVisible,
            present: 0,
            k: kind,
            mv: vec![2, 3, 0],
            bv: vec![],
            d1: vec![],
            d2: d2.to_vec(),
            xs: &[],
            pn: &[],
            xi: 0,
        },
    }
}
fn mixed() -> Vec<u8> {
    hex(include_str!(
        "fixtures/metadata_package_member_names/mixed.hex"
    ))
}
fn expected() -> Vec<Member> {
    vec![
        Member {
            kind: Kind::Function,
            ordinal: 1,
            span: 7..15,
            body_span: 9..15,
            name: vec![102],
            flags: Some(6),
            old_flags: Some(0),
            getter_flags: None,
            setter_flags: None,
        },
        Member {
            kind: Kind::Property,
            ordinal: 2,
            span: 15..27,
            body_span: 17..27,
            name: vec![112],
            flags: Some(0),
            old_flags: Some(0),
            getter_flags: Some(0),
            setter_flags: Some(1),
        },
        Member {
            kind: Kind::TypeAlias,
            ordinal: 3,
            span: 27..41,
            body_span: 29..41,
            name: vec![97],
            flags: Some(0),
            old_flags: None,
            getter_flags: None,
            setter_flags: None,
        },
        Member {
            kind: Kind::Function,
            ordinal: 4,
            span: 41..45,
            body_span: 43..45,
            name: vec![102],
            flags: None,
            old_flags: None,
            getter_flags: None,
            setter_flags: None,
        },
    ]
}
fn exact() -> Limits {
    let r = size_of::<string_table::Record<'_>>();
    let m = size_of::<Member>();
    let s = size_of::<&[u8]>();
    Limits {
        bytes: 63,
        records: 25,
        nodes: 7,
        depth: 2,
        work: 176 + r + 3 * s + 4 * m,
        owned_bytes: r + 3 * s + 4 * m + 8,
        entries: 5,
        range_slots: 4,
        queries: 4,
        output_units: 4,
    }
}
fn error(input: &metadata::Outcome<'_>, limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(input, limits), Err(Error { kind, offset }));
}
fn name(kind: name_resolver::ErrorKind, index: Option<i32>, record: Option<usize>) -> ErrorKind {
    ErrorKind::Name(name_resolver::Error {
        kind,
        index,
        record,
    })
}
fn wire_limit(label: &'static str) -> ErrorKind {
    ErrorKind::Wire(wire::ErrorKind::Limit(label))
}
fn typed(kind: Kind, wire: Option<wire::ErrorKind>) -> ErrorKind {
    match kind {
        Kind::Function => ErrorKind::Function(wire.map_or(
            function_header::ErrorKind::MissingName,
            function_header::ErrorKind::Wire,
        )),
        Kind::Property => ErrorKind::Property(wire.map_or(
            property_header::ErrorKind::MissingName,
            property_header::ErrorKind::Wire,
        )),
        Kind::TypeAlias => ErrorKind::TypeAlias(wire.map_or(
            type_alias::ErrorKind::MissingName,
            type_alias::ErrorKind::Wire,
        )),
    }
}
fn selected(tag: u8, tail: &str) -> Vec<u8> {
    let mut bytes = hex("04 0a 02 08 04");
    bytes.push(tag);
    bytes.extend(hex(tail));
    bytes
}

#[test]
fn interleaved_names_flags_spans_and_one_operation_limits() {
    let limits = exact();
    for k in [2, 5] {
        let input = input(mixed(), D2, k);
        assert_eq!(decode(&input, limits).unwrap(), expected());
        for raised in [
            Limits {
                bytes: limits.bytes + 1,
                ..limits
            },
            Limits {
                records: limits.records + 1,
                ..limits
            },
            Limits {
                nodes: limits.nodes + 1,
                ..limits
            },
            Limits {
                depth: limits.depth + 1,
                ..limits
            },
            Limits {
                work: limits.work + 1,
                ..limits
            },
            Limits {
                owned_bytes: limits.owned_bytes + 1,
                ..limits
            },
            Limits {
                entries: limits.entries + 1,
                ..limits
            },
            Limits {
                range_slots: limits.range_slots + 1,
                ..limits
            },
            Limits {
                queries: limits.queries + 1,
                ..limits
            },
            Limits {
                output_units: limits.output_units + 1,
                ..limits
            },
        ] {
            assert_eq!(decode(&input, raised).unwrap(), expected());
        }
        for (limits, kind, offset) in [
            (
                Limits {
                    bytes: 62,
                    records: 0,
                    work: 0,
                    ..limits
                },
                wire_limit("bytes"),
                0,
            ),
            (
                Limits {
                    records: 24,
                    ..limits
                },
                wire_limit("records"),
                51,
            ),
            (
                Limits {
                    work: limits.work - 1,
                    ..limits
                },
                wire_limit("work"),
                51,
            ),
            (
                Limits {
                    owned_bytes: limits.owned_bytes - 1,
                    ..limits
                },
                name(
                    name_resolver::ErrorKind::Limit("owned_bytes"),
                    Some(3),
                    Some(0),
                ),
                43,
            ),
            (Limits { nodes: 6, ..limits }, wire_limit("nodes"), 43),
            (Limits { depth: 1, ..limits }, wire_limit("depth"), 3),
            (
                Limits {
                    entries: 4,
                    ..limits
                },
                name(name_resolver::ErrorKind::Limit("entries"), None, None),
                1,
            ),
            (
                Limits {
                    entries: 3,
                    bytes: 0,
                    records: 0,
                    work: 0,
                    ..limits
                },
                name(name_resolver::ErrorKind::Limit("entries"), None, None),
                0,
            ),
            (
                Limits {
                    range_slots: 3,
                    ..limits
                },
                name(
                    name_resolver::ErrorKind::Limit("range_slots"),
                    None,
                    Some(0),
                ),
                1,
            ),
            (
                Limits {
                    queries: 3,
                    ..limits
                },
                name(name_resolver::ErrorKind::Limit("queries"), Some(3), None),
                43,
            ),
            (
                Limits {
                    output_units: 3,
                    ..limits
                },
                name(
                    name_resolver::ErrorKind::Limit("output_units"),
                    Some(3),
                    Some(0),
                ),
                43,
            ),
        ] {
            error(&input, limits, kind, offset);
        }
        let r = size_of::<string_table::Record<'_>>();
        error(
            &input,
            Limits {
                owned_bytes: r + 3,
                ..limits
            },
            wire_limit("owned_bytes"),
            7,
        );
        for (work, label) in [
            (limits.work, "owned_bytes"),
            (r + 59, "work"),
            (r + 60, "owned_bytes"),
        ] {
            error(
                &input,
                Limits {
                    owned_bytes: r + 1,
                    work,
                    ..limits
                },
                name(name_resolver::ErrorKind::Limit(label), Some(0), Some(0)),
                9,
            );
        }
    }
}

#[test]
fn every_frozen_byte_cut_and_identical_duplicate_record() {
    let bytes = mixed();
    for k in [2, 5] {
        for n in 0..=55 {
            let input = input(bytes[..n].to_vec(), D2, k);
            let count = match n {
                5 | 7 => Some(0),
                15 => Some(1),
                27 => Some(2),
                41 => Some(3),
                45 | 47 | 51 | 55 => Some(4),
                _ => None,
            };
            if let Some(count) = count {
                assert_eq!(
                    decode(&input, Limits::default()).unwrap(),
                    expected()[..count]
                );
            } else {
                error(
                    &input,
                    Limits::default(),
                    ErrorKind::Wire(wire::ErrorKind::Truncated),
                    n,
                );
            }
        }
        assert_eq!(
            decode(
                &input(hex("04 0a 02 08 04 1a 02 10 00 1a 02 10 00"), D2, k),
                Limits::default()
            )
            .unwrap(),
            vec![
                Member {
                    kind: Kind::Function,
                    ordinal: 0,
                    span: 5..9,
                    body_span: 7..9,
                    name: vec![102],
                    flags: None,
                    old_flags: None,
                    getter_flags: None,
                    setter_flags: None
                },
                Member {
                    kind: Kind::Function,
                    ordinal: 1,
                    span: 9..13,
                    body_span: 11..13,
                    name: vec![102],
                    flags: None,
                    old_flags: None,
                    getter_flags: None,
                    setter_flags: None
                },
            ]
        );
    }
}

#[test]
fn empty_package_and_wrong_wire_selectors() {
    let limits = Limits {
        bytes: 1,
        records: 1,
        nodes: 2,
        depth: 1,
        work: 4,
        owned_bytes: 0,
        entries: 0,
        range_slots: 0,
        queries: 0,
        output_units: 0,
    };
    for k in [2, 5] {
        let input = input(
            hex(include_str!(
                "fixtures/metadata_package_member_names/minimal.hex"
            )),
            &[],
            k,
        );
        assert_eq!(decode(&input, limits).unwrap(), []);
        for (limits, kind, offset) in [
            (
                Limits {
                    records: 0,
                    ..limits
                },
                wire_limit("records"),
                0,
            ),
            (Limits { work: 3, ..limits }, wire_limit("work"), 1),
            (Limits { nodes: 1, ..limits }, wire_limit("nodes"), 1),
            (Limits { depth: 0, ..limits }, wire_limit("depth"), 1),
            (
                Limits {
                    bytes: 0,
                    records: 0,
                    work: 0,
                    nodes: 0,
                    ..limits
                },
                wire_limit("bytes"),
                0,
            ),
        ] {
            error(&input, limits, kind, offset);
        }
        for (bytes, kind, offset) in [
            ("", wire::ErrorKind::Truncated, 0),
            ("00 00", wire::ErrorKind::InvalidFieldNumber, 1),
            ("00 0b", wire::ErrorKind::UnsupportedGroup, 1),
        ] {
            error(
                &self::input(hex(bytes), &[], k),
                Limits::default(),
                ErrorKind::Wire(kind),
                offset,
            );
        }
        for bytes in [
            "00 18 00 20 00 28 00",
            "00 f2 01 01 ff 82 02 01 00 a2 06 01 80",
            "00 1d 00 00 00 00 21 00 00 00 00 00 00 00 00 28 00",
        ] {
            assert_eq!(
                decode(&self::input(hex(bytes), &[], k), Limits::default()).unwrap(),
                []
            );
        }
    }
}

#[test]
fn selected_header_and_name_error_order_is_explicit() {
    for (tag, kind) in SELECTORS {
        for (tail, want, offset) in [
            ("00", typed(kind, None), 7),
            ("01 80", typed(kind, Some(wire::ErrorKind::Truncated)), 8),
            (
                "01 00",
                typed(kind, Some(wire::ErrorKind::InvalidFieldNumber)),
                7,
            ),
            (
                "02 10 04",
                name(name_resolver::ErrorKind::InvalidIndex, Some(4), None),
                7,
            ),
            (
                "06 10 ff ff ff ff 0f",
                name(name_resolver::ErrorKind::InvalidIndex, Some(-1), None),
                7,
            ),
            (
                "0b 10 ff ff ff ff ff ff ff ff ff 01",
                name(name_resolver::ErrorKind::InvalidIndex, Some(-1), None),
                7,
            ),
            (
                "03 10 00 0b",
                typed(kind, Some(wire::ErrorKind::UnsupportedGroup)),
                9,
            ),
            (
                "03 10 00 00",
                typed(kind, Some(wire::ErrorKind::InvalidFieldNumber)),
                9,
            ),
            ("00 00", typed(kind, None), 7),
            (
                "02 10 04 00",
                name(name_resolver::ErrorKind::InvalidIndex, Some(4), None),
                7,
            ),
            (
                "02 10 00 00",
                ErrorKind::Wire(wire::ErrorKind::InvalidFieldNumber),
                9,
            ),
            (
                "04 10 00 1a 01",
                typed(kind, Some(wire::ErrorKind::Truncated)),
                11,
            ),
            ("02 12 00", typed(kind, None), 9),
        ] {
            for k in [2, 5] {
                error(
                    &input(selected(tag, tail), D2, k),
                    Limits::default(),
                    want,
                    offset,
                );
            }
        }
        let out = decode(
            &input(selected(tag, "04 10 00 1a 00"), D2, 2),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            (out.len(), out[0].kind, out[0].name.as_slice()),
            (1, kind, &[102][..])
        );
        error(
            &input(selected(tag, "02 10 00"), &[], 2),
            Limits::default(),
            name(name_resolver::ErrorKind::MissingString, Some(0), Some(0)),
            7,
        );
    }
}

#[test]
fn scalar_presence_signed_values_and_low_bits_are_preserved() {
    for (tag, kind) in SELECTORS {
        for body in [
            "10 03 10 00 08 01 08 06",
            "10 00 12 00",
            "10 80 80 80 80 10",
            "10 80 00",
        ] {
            let body = hex(body);
            let mut bytes = hex("04 0a 02 08 04");
            bytes.extend([tag, body.len() as u8]);
            bytes.extend(&body);
            let out = decode(&input(bytes, D2, 2), Limits::default()).unwrap();
            assert_eq!((out[0].kind, out[0].name.as_slice()), (kind, &[102][..]));
            let present = body.starts_with(&[0x10, 3]);
            assert_eq!(
                out[0].flags,
                if present && kind == Kind::TypeAlias {
                    Some(6)
                } else {
                    None
                }
            );
            assert_eq!(
                out[0].old_flags,
                if present && kind != Kind::TypeAlias {
                    Some(6)
                } else {
                    None
                }
            );
            assert_eq!((out[0].getter_flags, out[0].setter_flags), (None, None));
        }
        for (value, want) in [("ff ff ff ff 0f", -1), ("80 80 80 80 08", i32::MIN)] {
            let mut body = hex("90 00 80 00");
            body.push(match kind {
                Kind::Function => 0x48,
                Kind::Property => 0x58,
                Kind::TypeAlias => 0x08,
            });
            body.extend(hex(value));
            let mut bytes = hex("04 0a 02 08 04");
            bytes.extend([tag, body.len() as u8]);
            bytes.extend(body);
            let out = decode(&input(bytes, D2, 2), Limits::default()).unwrap();
            assert_eq!(
                (out[0].name.as_slice(), out[0].flags),
                (&[102][..], Some(want))
            );
        }
    }
}

#[test]
fn utf16_unsupported_utf8_and_unknown_enum_behavior() {
    for (tag, kind) in SELECTORS {
        for units in [&[][..], &[0][..], &[55296][..]] {
            let out = decode(
                &input(selected(tag, "02 10 00"), &[units], 2),
                Limits::default(),
            )
            .unwrap();
            assert_eq!((out[0].kind, out[0].name.as_slice()), (kind, units));
            let limits = Limits {
                output_units: units.len(),
                ..Limits::default()
            };
            assert_eq!(
                decode(&input(selected(tag, "02 10 00"), &[units], 5), limits).unwrap()[0].name,
                units
            );
            if !units.is_empty() {
                error(
                    &input(selected(tag, "02 10 00"), &[units], 2),
                    Limits {
                        output_units: 0,
                        ..limits
                    },
                    name(
                        name_resolver::ErrorKind::Limit("output_units"),
                        Some(0),
                        Some(0),
                    ),
                    7,
                );
            }
        }
        let mut bytes = hex("05 0a 03 32 01 ff");
        bytes.extend([tag, 2, 0x10, 0]);
        error(
            &input(bytes, &[], 2),
            Limits::default(),
            name(name_resolver::ErrorKind::UnsupportedUtf8, Some(0), Some(0)),
            8,
        );
        let mut bytes = hex("0a 0a 08 08 01 18 01 18 7f 32 00");
        bytes.extend([tag, 2, 0x10, 0]);
        let out = decode(&input(bytes, &[], 2), Limits::default()).unwrap();
        assert_eq!((out[0].kind, out[0].name.as_slice()), (kind, &[][..]));
    }
    assert_eq!(
        decode(&input(hex("05 0a 03 32 01 ff"), &[], 2), Limits::default()).unwrap(),
        []
    );
}

#[test]
fn container_guards_and_hard_limits() {
    let zero = Limits {
        bytes: 0,
        records: 0,
        nodes: 0,
        depth: 0,
        work: 0,
        owned_bytes: 0,
        entries: 0,
        range_slots: 0,
        queries: 0,
        output_units: 0,
    };
    error(
        &metadata::Outcome::Absent,
        zero,
        ErrorKind::UnsupportedContainer,
        0,
    );
    for (k, mv) in [
        (1, vec![2, 3, 0]),
        (3, vec![2, 3, 0]),
        (4, vec![2, 3, 0]),
        (-1, vec![2, 3, 0]),
        (2, vec![2, 3]),
        (5, vec![2, 4, 0]),
    ] {
        let mut input = input(vec![], &[], k);
        let metadata::Outcome::Decoded { header, .. } = &mut input else {
            unreachable!()
        };
        header.mv = mv;
        error(&input, zero, ErrorKind::UnsupportedContainer, 0);
    }
    let metadata::Outcome::Decoded { header, .. } = input(vec![], &[], 2) else {
        unreachable!()
    };
    error(
        &metadata::Outcome::Unsupported {
            header,
            reason: metadata::Unsupported::Version,
        },
        zero,
        ErrorKind::UnsupportedContainer,
        0,
    );
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
        entries: usize::MAX,
        range_slots: usize::MAX,
        queries: usize::MAX,
        output_units: usize::MAX,
    };
    assert_eq!(
        decode(&input(mixed(), D2, 2), unlimited).unwrap(),
        expected()
    );
    error(
        &input(vec![0; ktlsp_binary::MAX_BYTES + 1], &[], 2),
        unlimited,
        wire_limit("bytes"),
        0,
    );
    error(
        &input(vec![0], &vec![&[][..]; ktlsp_binary::MAX_RECORDS + 1], 2),
        unlimited,
        name(name_resolver::ErrorKind::Limit("entries"), None, None),
        0,
    );
    let units = vec![0; ktlsp_binary::MAX_BYTES / 2];
    error(
        &input(vec![0], &[&units], 2),
        unlimited,
        wire_limit("bytes"),
        0,
    );
}

#[test]
fn genuine_kind2_class_uses_new_composition_api() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_package_member_names/KTypeParameterBaseKt.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        panic!("genuine supported Package")
    };
    assert_eq!(
        (header.k, header.mv.as_slice(), header.xi, header.present),
        (2, &[2, 3, 0][..], 48, 0x9b)
    );
    assert_eq!(
        bytes,
        &hex(include_str!(
            "fixtures/metadata_package_member_names/genuine-d1.hex"
        ))
    );
    assert_eq!(header.d1[0][0], 0);
    assert_eq!(
        &header.d1[0][1..],
        bytes.iter().map(|b| u16::from(*b)).collect::<Vec<_>>()
    );
    let wanted = vec![
        84, 121, 112, 101, 80, 97, 114, 97, 109, 101, 116, 101, 114, 67, 111, 110, 116, 97, 105,
        110, 101, 114,
    ];
    assert_eq!(
        header.d2,
        vec![
            wanted.as_slice(),
            &[],
            &[
                107, 111, 116, 108, 105, 110, 45, 115, 116, 100, 108, 105, 98
            ]
        ]
    );
    assert_eq!(header.bv, [1, 0, 3]);
    assert!(header.xs.is_empty() && header.pn.is_empty());
    let framed = payload::decode(bytes, Default::default()).unwrap();
    assert_eq!(
        (
            framed.prefix_span,
            framed.string_table_span,
            framed.payload_span
        ),
        (0..1, 1..9, 9..26)
    );
    assert!(framed.string_table.local_name.is_empty() && framed.string_table.unknown.is_empty());
    assert_eq!(framed.string_table.records.len(), 3);
    for (i, span) in [2..2, 4..6, 8..8].into_iter().enumerate() {
        let r = &framed.string_table.records[i];
        assert_eq!(r.span, span);
        assert_eq!(r.range, None);
        assert_eq!(r.predefined_index, if i == 1 { Some(0) } else { None });
        assert!(
            r.operation.is_none()
                && r.string.is_none()
                && r.substring_index.is_empty()
                && r.replace_char.is_empty()
                && r.unknown.is_empty()
        );
    }
    let expected = vec![Member {
        kind: Kind::TypeAlias,
        ordinal: 0,
        span: 9..23,
        body_span: 11..23,
        name: wanted,
        flags: Some(0),
        old_flags: None,
        getter_flags: None,
        setter_flags: None,
    }];
    let r = size_of::<string_table::Record<'_>>();
    let s = size_of::<&[u8]>();
    let m = size_of::<Member>();
    let limits = Limits {
        bytes: 96,
        records: 11,
        nodes: 6,
        depth: 2,
        entries: 6,
        range_slots: 3,
        queries: 1,
        output_units: 22,
        owned_bytes: 4 * r + 2 * s + m + 44,
        work: 211 + 4 * r + 2 * s + m,
    };
    assert_eq!(decode(&input, limits).unwrap(), expected);
    assert_eq!(decode(&input, Limits::default()).unwrap(), expected);
    for (limits, label, offset) in [
        (
            Limits {
                work: limits.work - 1,
                ..limits
            },
            "work",
            23,
        ),
        (
            Limits {
                records: 10,
                ..limits
            },
            "records",
            23,
        ),
        (
            Limits {
                owned_bytes: limits.owned_bytes - 1,
                ..limits
            },
            "owned_bytes",
            9,
        ),
    ] {
        error(&input, limits, wire_limit(label), offset);
    }
    assert_eq!(&bytes[11..23], &hex("08 00 10 00 22 02 30 01 32 02 30 01"));
}

#[test]
fn bounded_deterministic_hostile_inputs() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        nodes: 4,
        depth: 2,
        work: 256,
        owned_bytes: 1024,
        ..Limits::default()
    };
    for a in 0..=255 {
        for b in 0..=255 {
            for k in [2, 5] {
                let input = input(vec![0, a, b], &[], k);
                let first = decode(&input, limits);
                assert_eq!(first, decode(&input, limits));
                if let Ok(members) = first {
                    for member in members {
                        assert!(
                            member.span.start <= member.body_span.start
                                && member.body_span.start <= member.body_span.end
                                && member.body_span.end == member.span.end
                                && member.span.end <= 3
                        );
                    }
                } else if let Err(e) = first {
                    assert!(e.offset <= 3);
                }
            }
        }
        for n in 0..32 {
            for k in [2, 5] {
                let input = input(vec![a; n], &[], k);
                assert_eq!(decode(&input, limits), decode(&input, limits));
            }
        }
    }
}
