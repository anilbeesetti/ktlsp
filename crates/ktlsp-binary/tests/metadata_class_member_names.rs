use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, class_header,
        class_member_names::{Error, ErrorKind, Kind, Limits, Member, decode},
        function_header, name_resolver, property_header, string_table, wire,
    },
};
use std::mem::size_of;

static RAW: Annotation<'static> = Annotation {
    descriptor: &[],
    elements: Vec::new(),
};
const D2: &[&[u16]] = &[&[67], &[102], &[112], &[102]];
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
// Synthetic decoded inputs exercise the consumer's own guard; genuine decoding is below.
fn input<'a>(bytes: Vec<u8>, d2: &[&'a [u16]]) -> metadata::Outcome<'a> {
    metadata::Outcome::Decoded {
        bytes,
        header: metadata::Header {
            raw: &RAW,
            visibility: &Visibility::RuntimeVisible,
            present: 0,
            k: 1,
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
        "fixtures/metadata_class_member_names/mixed.hex"
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
            kind: Kind::Function,
            ordinal: 3,
            span: 27..31,
            body_span: 29..31,
            name: vec![102],
            flags: None,
            old_flags: None,
            getter_flags: None,
            setter_flags: None,
        },
    ]
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
fn exact() -> Limits {
    let r = size_of::<string_table::Record<'_>>();
    let m = size_of::<Member>();
    Limits {
        bytes: 51,
        records: 28,
        nodes: 6,
        depth: 2,
        work: 190 + r + 4 * m,
        owned_bytes: r + 4 * m + 6,
        entries: 5,
        range_slots: 4,
        queries: 3,
        output_units: 3,
    }
}

#[test]
fn interleaved_names_flags_spans_and_one_operation_limits() {
    let input = input(mixed(), D2);
    let limits = exact();
    assert_eq!(decode(&input, limits).unwrap(), expected());
    assert_eq!(
        decode(
            &input,
            Limits {
                bytes: 52,
                records: 29,
                nodes: 7,
                depth: 3,
                work: limits.work + 1,
                owned_bytes: limits.owned_bytes + 1,
                entries: 6,
                range_slots: 5,
                queries: 4,
                output_units: 4
            }
        )
        .unwrap(),
        expected()
    );
    for (limits, kind, offset) in [
        (
            Limits {
                bytes: 50,
                records: 0,
                work: 0,
                ..limits
            },
            wire_limit("bytes"),
            0,
        ),
        (
            Limits {
                records: 27,
                ..limits
            },
            wire_limit("records"),
            39,
        ),
        (
            Limits {
                work: limits.work - 1,
                ..limits
            },
            wire_limit("work"),
            39,
        ),
        (
            Limits {
                owned_bytes: limits.owned_bytes - 1,
                ..limits
            },
            wire_limit("owned_bytes"),
            27,
        ),
        (Limits { nodes: 5, ..limits }, wire_limit("nodes"), 29),
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
                queries: 2,
                ..limits
            },
            name(name_resolver::ErrorKind::Limit("queries"), Some(3), None),
            29,
        ),
        (
            Limits {
                output_units: 2,
                ..limits
            },
            name(
                name_resolver::ErrorKind::Limit("output_units"),
                Some(3),
                Some(0),
            ),
            29,
        ),
    ] {
        error(&input, limits, kind, offset);
    }
    let r = size_of::<string_table::Record<'_>>();
    for (work, kind) in [
        (limits.work, "owned_bytes"),
        (r + 113, "work"),
        (r + 114, "owned_bytes"),
    ] {
        error(
            &input,
            Limits {
                owned_bytes: r + 1,
                work,
                ..limits
            },
            name(name_resolver::ErrorKind::Limit(kind), Some(1), Some(0)),
            9,
        );
    }
}

#[test]
fn every_frozen_byte_cut() {
    let bytes = mixed();
    let trunc = [
        6, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 28, 29, 30, 32,
        33, 35, 36, 38, 40, 41, 42,
    ];
    for n in 0..=43 {
        let input = input(bytes[..n].to_vec(), D2);
        if n <= 4 {
            error(
                &input,
                Limits::default(),
                ErrorKind::Wire(wire::ErrorKind::Truncated),
                n,
            );
        } else if n == 5 {
            error(
                &input,
                Limits::default(),
                ErrorKind::Class(class_header::ErrorKind::MissingFqName),
                5,
            );
        } else if trunc.contains(&n) {
            error(
                &input,
                Limits::default(),
                ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
                n,
            );
        } else {
            let count = match n {
                7 => 0,
                15 => 1,
                27 => 2,
                31 | 34 | 37 | 39 | 43 => 3,
                _ => panic!("unclassified cut {n}"),
            };
            assert_eq!(
                decode(&input, Limits::default()).unwrap(),
                expected()[..count]
            );
        }
    }
}

#[test]
fn required_class_and_empty_result_are_validated() {
    let limits = Limits {
        bytes: 3,
        records: 3,
        nodes: 2,
        depth: 1,
        work: 12,
        owned_bytes: 0,
        entries: 0,
        range_slots: 0,
        queries: 0,
        output_units: 0,
    };
    let bytes = hex(include_str!(
        "fixtures/metadata_class_member_names/minimal.hex"
    ));
    let input = input(bytes.clone(), &[]);
    assert_eq!(decode(&input, limits).unwrap(), []);
    for (limits, kind, offset) in [
        (Limits { work: 11, ..limits }, wire_limit("work"), 1),
        (
            Limits {
                records: 2,
                ..limits
            },
            wire_limit("records"),
            1,
        ),
        (Limits { bytes: 2, ..limits }, wire_limit("bytes"), 0),
        (Limits { nodes: 1, ..limits }, wire_limit("nodes"), 1),
    ] {
        error(&input, limits, kind, offset);
    }
    for (text, kind, offset) in [
        ("", ErrorKind::Wire(wire::ErrorKind::Truncated), 0),
        (
            "00",
            ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            1,
        ),
        (
            "00 18",
            ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
            2,
        ),
        (
            "00 08 06",
            ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            3,
        ),
        (
            "00 18 00 00",
            ErrorKind::Class(class_header::ErrorKind::Wire(
                wire::ErrorKind::InvalidFieldNumber,
            )),
            3,
        ),
    ] {
        error(
            &self::input(hex(text), &[]),
            Limits::default(),
            kind,
            offset,
        );
    }
    assert_eq!(
        decode(
            &self::input(hex("00 18 00 48 00 50 00"), &[]),
            Limits::default()
        )
        .unwrap(),
        []
    );
}

#[test]
fn selected_headers_and_names_fail_with_exact_context() {
    for (tail, kind, offset) in [
        (
            "18 00 4a 00",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            9,
        ),
        (
            "18 00 52 00",
            ErrorKind::Property(property_header::ErrorKind::MissingName),
            9,
        ),
        (
            "18 00 4a 01 80",
            ErrorKind::Function(function_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
            10,
        ),
        (
            "18 00 52 01 00",
            ErrorKind::Property(property_header::ErrorKind::Wire(
                wire::ErrorKind::InvalidFieldNumber,
            )),
            9,
        ),
        (
            "18 00 4a 02 10 04",
            name(name_resolver::ErrorKind::InvalidIndex, Some(4), None),
            9,
        ),
        (
            "18 00 52 02 10 04",
            name(name_resolver::ErrorKind::InvalidIndex, Some(4), None),
            9,
        ),
        (
            "18 00 4a 06 10 ff ff ff ff 0f",
            name(name_resolver::ErrorKind::InvalidIndex, Some(-1), None),
            9,
        ),
        (
            "18 00 52 06 10 ff ff ff ff 0f",
            name(name_resolver::ErrorKind::InvalidIndex, Some(-1), None),
            9,
        ),
        (
            "18 00 4a 04 10 01 0b",
            ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
            12,
        ),
        (
            "18 00 4a 03 10 01 0b",
            ErrorKind::Function(function_header::ErrorKind::Wire(
                wire::ErrorKind::UnsupportedGroup,
            )),
            11,
        ),
        (
            "18 00 52 03 10 01 0b",
            ErrorKind::Property(property_header::ErrorKind::Wire(
                wire::ErrorKind::UnsupportedGroup,
            )),
            11,
        ),
        (
            "18 00 4a 02 10 01 00",
            ErrorKind::Class(class_header::ErrorKind::Wire(
                wire::ErrorKind::InvalidFieldNumber,
            )),
            11,
        ),
        (
            "18 00 52 02 10 01 00",
            ErrorKind::Class(class_header::ErrorKind::Wire(
                wire::ErrorKind::InvalidFieldNumber,
            )),
            11,
        ),
    ] {
        let mut bytes = hex("04 0a 02 08 04");
        bytes.extend(hex(tail));
        error(&input(bytes, D2), Limits::default(), kind, offset);
    }
    error(
        &input(hex("05 0a 03 32 01 ff 18 00 4a 02 10 00"), &[]),
        Limits::default(),
        name(name_resolver::ErrorKind::UnsupportedUtf8, Some(0), Some(0)),
        10,
    );
    assert_eq!(
        decode(
            &input(hex("05 0a 03 32 01 ff 18 00"), &[]),
            Limits::default()
        )
        .unwrap(),
        []
    );
    error(
        &input(hex("04 0a 02 08 04 18 00 4a 02 10 01"), &[]),
        Limits::default(),
        name(name_resolver::ErrorKind::MissingString, Some(1), Some(0)),
        9,
    );
}

#[test]
fn raw_utf16_wrong_wires_duplicates_and_opaque_nested_bodies() {
    for units in [&[55296][..], &[0][..], &[][..]] {
        let out = decode(
            &input(hex("04 0a 02 08 01 18 00 4a 02 10 00"), &[units]),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(out[0].name, units);
    }
    // Reuse frozen header literals; nested malformed messages stay opaque.
    for (tag, body, flags, old, getter, setter) in [
        (
            0x4a,
            "08 01 48 02 10 01 08 06 48 00 10 00 0a 01 ff 4a 01 80 12 01 00",
            Some(0),
            Some(6),
            None,
            None,
        ),
        (
            0x4a,
            "10 00 1a 01 80 22 00 32 00 5a 01 80 a2 06 01 00",
            None,
            None,
            None,
            None,
        ),
        (
            0x52,
            "08 01 10 00 38 00 40 01 58 00 12 01 80",
            Some(0),
            Some(1),
            Some(0),
            Some(1),
        ),
    ] {
        let body = hex(body);
        let mut bytes = hex("04 0a 02 08 01 18 00");
        bytes.extend([tag, body.len() as u8]);
        bytes.extend(body);
        let out = decode(&input(bytes, &[&[102]]), Limits::default()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, [102]);
        assert_eq!(
            (
                out[0].flags,
                out[0].old_flags,
                out[0].getter_flags,
                out[0].setter_flags
            ),
            (flags, old, getter, setter)
        );
    }
    for (tag, value, want) in [
        (0x4a, "ff ff ff ff 0f", Some(-1)),
        (0x52, "80 80 80 80 08", Some(i32::MIN)),
    ] {
        let mut body = hex("90 00 80 00");
        body.push(if tag == 0x4a { 0x48 } else { 0x58 });
        body.extend(hex(value));
        let mut bytes = hex("04 0a 02 08 01 18 00");
        bytes.extend([tag, body.len() as u8]);
        bytes.extend(body);
        let out = decode(&input(bytes, &[&[102]]), Limits::default()).unwrap();
        assert_eq!((out[0].name.as_slice(), out[0].flags), (&[102][..], want));
    }
    for tail in [
        "42 01 ff",
        "5a 01 80",
        "2a 01 00",
        "4d 00 00 00 00",
        "51 00 00 00 00 00 00 00 00",
    ] {
        let mut bytes = hex("00 18 00");
        bytes.extend(hex(tail));
        assert_eq!(decode(&input(bytes, &[]), Limits::default()).unwrap(), []);
    }
}

#[test]
fn supported_container_and_hard_limits_cannot_be_forged() {
    error(
        &metadata::Outcome::Absent,
        Limits::default(),
        ErrorKind::UnsupportedContainer,
        0,
    );
    for (kind, version) in [
        (2, vec![2, 3, 0]),
        (5, vec![2, 3, 0]),
        (1, vec![2, 3]),
        (1, vec![2, 4, 0]),
    ] {
        let mut input = input(vec![], &[]);
        let metadata::Outcome::Decoded { header, .. } = &mut input else {
            unreachable!()
        };
        header.k = kind;
        header.mv = version;
        error(
            &input,
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
                ..Limits::default()
            },
            ErrorKind::UnsupportedContainer,
            0,
        );
    }
    let metadata::Outcome::Decoded { header, .. } = input(vec![], &[]) else {
        unreachable!()
    };
    error(
        &metadata::Outcome::Unsupported {
            header,
            reason: metadata::Unsupported::Version,
        },
        Limits::default(),
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
    error(
        &input(vec![0; ktlsp_binary::MAX_BYTES + 1], &[]),
        unlimited,
        wire_limit("bytes"),
        0,
    );
    error(
        &input(vec![], &vec![&[][..]; ktlsp_binary::MAX_RECORDS + 1]),
        unlimited,
        name(name_resolver::ErrorKind::Limit("entries"), None, None),
        0,
    );
    let mut bytes = hex("00 18 00");
    for _ in 0..32766 {
        bytes.extend([8, 0]);
    }
    assert_eq!(decode(&input(bytes.clone(), &[]), unlimited).unwrap(), []);
    bytes.extend([8, 0]);
    error(&input(bytes, &[]), unlimited, wire_limit("records"), 65533);
}

#[test]
fn genuine_unit_class_uses_new_composition_api() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    assert_eq!(
        decode(&input, Limits::default()).unwrap(),
        vec![Member {
            kind: Kind::Function,
            ordinal: 4,
            span: 37..49,
            body_span: 39..49,
            name: vec![116, 111, 83, 116, 114, 105, 110, 103],
            flags: Some(65558),
            old_flags: None,
            getter_flags: None,
            setter_flags: None,
        }]
    );
}

#[test]
fn bounded_deterministic_hostile_inputs() {
    let limits = Limits {
        bytes: 32,
        records: 16,
        nodes: 8,
        work: 1024,
        owned_bytes: 1024,
        ..Limits::default()
    };
    for a in 0..=255 {
        for b in 0..=255 {
            for bytes in [vec![a, b], vec![0, 0x18, 0, a, b]] {
                let input = input(bytes, &[]);
                let first = decode(&input, limits);
                assert_eq!(first, decode(&input, limits));
                if let Err(e) = first {
                    let metadata::Outcome::Decoded { bytes, .. } = &input else {
                        unreachable!()
                    };
                    assert!(e.offset <= bytes.len());
                }
            }
        }
        for n in 0..32 {
            let input = input(vec![a; n], &[]);
            assert_eq!(decode(&input, limits), decode(&input, limits));
        }
    }
}
