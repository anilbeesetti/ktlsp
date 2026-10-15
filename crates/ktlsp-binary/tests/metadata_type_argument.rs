use ktlsp_binary::metadata::{
    self, class_header, function_header,
    name_resolver::Resolver,
    payload,
    type_argument::{Error, ErrorKind, InlineType, Limits, Projection, TypeArgument, decode},
    type_argument_types, type_arguments, type_header,
    wire::{self, Value},
};
use std::{mem::size_of, ops::Range};

const S: usize = size_of::<InlineType<'_>>();
const SELECTED: &str = include_str!("fixtures/metadata_type_argument/selected.hex");
const DUPLICATES: &str = include_str!("fixtures/metadata_type_argument/duplicates.hex");
const OPAQUE: &str = include_str!("fixtures/metadata_type_argument/opaque.hex");
const PAIR: &str = include_str!("fixtures/metadata_type_argument/pair_argument.hex");

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn retained(out: &TypeArgument<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for body in &out.inline_types {
        assert!(end <= body.span.start && body.span.start <= body.span.end);
        assert!(body.span.end <= bytes.len());
        assert_eq!(body.raw, &bytes[body.span.clone()]);
        assert_eq!(body.raw.as_ptr(), bytes[body.span.clone()].as_ptr());
        end = body.span.end;
    }
}
fn check(
    bytes: &[u8],
    limits: Limits,
    projection: Option<Projection>,
    type_id: Option<i32>,
    bodies: &[(&[u8], Range<usize>)],
) {
    let out = decode(bytes, limits).unwrap();
    retained(&out, bytes);
    assert_eq!(
        (out.projection, out.type_id),
        (projection, type_id),
        "{bytes:02x?}"
    );
    assert_eq!(out.inline_types.len(), bodies.len());
    for (actual, (raw, span)) in out.inline_types.iter().zip(bodies) {
        assert_eq!((actual.raw, &actual.span), (*raw, span));
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn limited(
    text: &str,
    records: usize,
    work: usize,
    owned_bytes: usize,
    kind: ErrorKind,
    offset: usize,
) {
    error(
        &hex(text),
        Limits {
            records,
            work,
            owned_bytes,
            ..Default::default()
        },
        kind,
        offset,
    );
}

#[test]
fn frozen_fixtures_and_exact_aggregate_limits() {
    for (text, projection, id, bodies, fields, owned, work, last, growth) in [
        (
            SELECTED,
            Some(Projection::Inv),
            Some(0),
            vec![(&[0x30, 4][..], 4..6)],
            3,
            S,
            14 + S,
            6,
            2,
        ),
        (
            DUPLICATES,
            Some(Projection::Star),
            Some(-1),
            vec![
                (&[0x30, 4][..], 6..8),
                (&[0x18, 1][..], 10..12),
                (&[][..], 14..14),
            ],
            9,
            4 * S,
            48 + 4 * S,
            24,
            12,
        ),
        (
            OPAQUE,
            None,
            None,
            vec![(&[0x80][..], 14..15)],
            5,
            S,
            25 + S,
            12,
            12,
        ),
        (
            PAIR,
            None,
            None,
            vec![(&[0x38, 0][..], 2..4)],
            1,
            S,
            6 + S,
            0,
            0,
        ),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            check(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: fields + extra,
                    work: work + extra,
                    owned_bytes: owned + extra,
                },
                projection,
                id,
                &bodies,
            );
        }
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            ErrorKind::Limit("bytes"),
            0,
        );
        limited(
            text,
            fields - 1,
            usize::MAX,
            owned,
            ErrorKind::Limit("records"),
            last,
        );
        limited(
            text,
            fields,
            work - 1,
            owned,
            ErrorKind::Limit("work"),
            last,
        );
        limited(
            text,
            fields,
            work,
            owned - 1,
            ErrorKind::Limit("owned_bytes"),
            growth,
        );
    }
    check(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            work: 0,
            owned_bytes: 0,
        },
        None,
        None,
        &[],
    );
}

#[test]
fn projection_presence_defaults_unknown_enums_and_scalar_last_wins() {
    let values = [
        Projection::In,
        Projection::Out,
        Projection::Inv,
        Projection::Star,
    ];
    for (value, projection) in values.into_iter().enumerate() {
        check(
            &[8, value as u8],
            Default::default(),
            Some(projection),
            None,
            &[],
        );
        for text in [
            "04",
            "63",
            "ff ff ff ff 0f",
            "ff ff ff ff ff ff ff ff ff 01",
            "ff ff ff ff ff ff ff ff ff 7f",
            "80 80 80 80 08",
            "ff ff ff ff 07",
        ] {
            let unknown = hex(&format!("08 {text}"));
            check(&unknown, Default::default(), None, None, &[]);
            for bytes in [
                [unknown.as_slice(), &[8, value as u8]].concat(),
                [&[8, value as u8], unknown.as_slice()].concat(),
            ] {
                check(&bytes, Default::default(), Some(projection), None, &[]);
            }
        }
        for (next, next_projection) in values.into_iter().enumerate() {
            check(
                &[8, value as u8, 8, next as u8],
                Default::default(),
                Some(next_projection),
                None,
                &[],
            );
        }
    }
    check(
        &hex("08 00 08 63 08 02 08 ff ff ff ff 0f"),
        Default::default(),
        Some(Projection::Inv),
        None,
        &[],
    );
    check(
        &hex("12 00"),
        Default::default(),
        None,
        None,
        &[(&[], 2..2)],
    );
    check(
        &hex("08 03 12 00 18 00"),
        Default::default(),
        Some(Projection::Star),
        Some(0),
        &[(&[], 4..4)],
    );
    check(&hex("18 00"), Default::default(), None, Some(0), &[]);
    check(&hex("18 01 18 00"), Default::default(), None, Some(0), &[]);
}

#[test]
fn signed_low32_ids_enums_and_nonminimal_tags_lengths() {
    for (text, id, projection) in [
        ("00", 0, Some(Projection::In)),
        ("80 00", 0, Some(Projection::In)),
        ("01", 1, Some(Projection::Out)),
        ("02", 2, Some(Projection::Inv)),
        ("03", 3, Some(Projection::Star)),
        ("04", 4, None),
        ("63", 99, None),
        ("ff ff ff ff 0f", -1, None),
        ("ff ff ff ff ff ff ff ff ff 01", -1, None),
        ("ff ff ff ff ff ff ff ff ff 7f", -1, None),
        ("80 80 80 80 08", i32::MIN, None),
        ("ff ff ff ff 07", i32::MAX, None),
        ("80 80 80 80 10", 0, Some(Projection::In)),
        ("81 80 80 80 10", 1, Some(Projection::Out)),
        ("82 80 80 80 10", 2, Some(Projection::Inv)),
        ("83 80 80 80 10", 3, Some(Projection::Star)),
        ("80 80 80 80 80 80 80 80 80 02", 0, Some(Projection::In)),
    ] {
        check(
            &hex(&format!("18 {text}")),
            Default::default(),
            None,
            Some(id),
            &[],
        );
        check(
            &hex(&format!("18 01 18 {text}")),
            Default::default(),
            None,
            Some(id),
            &[],
        );
        check(
            &hex(&format!("08 {text}")),
            Default::default(),
            projection,
            None,
            &[],
        );
    }
    check(&hex("98 00 80 00"), Default::default(), None, Some(0), &[]);
    check(
        &hex("98 80 80 80 10 01"),
        Default::default(),
        None,
        Some(1),
        &[],
    );
    check(
        &hex("12 80 00"),
        Default::default(),
        None,
        None,
        &[(&[], 3..3)],
    );
    check(
        &hex("12 80 80 80 80 10"),
        Default::default(),
        None,
        None,
        &[(&[], 6..6)],
    );
}

#[test]
fn wrong_wires_never_supply_or_erase_selected_data() {
    for tag in [8, 16, 24] {
        let mut wrongs = vec![
            hex(&format!("{:02x} 01 02 03 04 05 06 07 08", tag | 1)),
            hex(&format!("{:02x} 01 02 03 04", tag | 5)),
        ];
        wrongs.push(if tag == 16 {
            vec![16, 1]
        } else {
            vec![tag | 2, 1, 0x80]
        });
        for wrong in wrongs {
            check(&wrong, Default::default(), None, None, &[]);
            let selected = hex(SELECTED);
            check(
                &[wrong.as_slice(), selected.as_slice()].concat(),
                Default::default(),
                Some(Projection::Inv),
                Some(0),
                &[(&[0x30, 4], wrong.len() + 4..wrong.len() + 6)],
            );
            check(
                &[selected.as_slice(), wrong.as_slice()].concat(),
                Default::default(),
                Some(Projection::Inv),
                Some(0),
                &[(&[0x30, 4], 4..6)],
            );
        }
    }
    check(&hex("1a 02 01 02"), Default::default(), None, None, &[]);
}

#[test]
fn all_inline_merge_inputs_unknown_fields_and_malformed_bodies_stay_borrowed() {
    check(
        &hex(DUPLICATES),
        Default::default(),
        Some(Projection::Star),
        Some(-1),
        &[(&[0x30, 4], 6..8), (&[0x18, 1], 10..12), (&[], 14..14)],
    );
    for text in [
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
    ] {
        check(&hex(text), Default::default(), None, None, &[]);
        check(
            &hex(&format!("{SELECTED} {text}")),
            Default::default(),
            Some(Projection::Inv),
            Some(0),
            &[(&[0x30, 4], 4..6)],
        );
    }
    for body in [
        vec![],
        hex("80"),
        hex("00"),
        hex("0b"),
        hex("ff"),
        hex("c0 af"),
    ] {
        let bytes = [&[18, body.len() as u8][..], body.as_slice()].concat();
        check(
            &bytes,
            Default::default(),
            None,
            None,
            &[(&body, 2..bytes.len())],
        );
    }
}

#[test]
fn structural_errors_and_malformed_tails_fail_without_partial_results() {
    for (text, kind, offset) in [
        ("08", ErrorKind::Truncated, 1),
        ("18", ErrorKind::Truncated, 1),
        ("80", ErrorKind::Truncated, 1),
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("03", ErrorKind::InvalidFieldNumber, 0),
        ("07", ErrorKind::InvalidFieldNumber, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0c", ErrorKind::UnsupportedGroup, 0),
        ("13", ErrorKind::UnsupportedGroup, 0),
        ("14", ErrorKind::UnsupportedGroup, 0),
        ("1b", ErrorKind::UnsupportedGroup, 0),
        ("1c", ErrorKind::UnsupportedGroup, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
        ("16", ErrorKind::InvalidWireType, 0),
        ("17", ErrorKind::InvalidWireType, 0),
        ("1e", ErrorKind::InvalidWireType, 0),
        ("1f", ErrorKind::InvalidWireType, 0),
        ("12 02 ff", ErrorKind::Truncated, 3),
        ("0d 01 02 03", ErrorKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", ErrorKind::Truncated, 8),
        ("12 80 80 80 80 08", ErrorKind::NegativeLength, 1),
        ("12 ff ff ff ff 07", ErrorKind::Truncated, 6),
        (
            "80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            9,
        ),
        (
            "08 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        (
            "18 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        (
            "12 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
    ] {
        let body = hex(text);
        error(&body, Default::default(), kind, offset);
        error(
            &hex(&format!("{SELECTED} {text}")),
            Default::default(),
            kind,
            offset + 8,
        );
        let bytes = [&[18, body.len() as u8][..], body.as_slice()].concat();
        check(
            &bytes,
            Default::default(),
            None,
            None,
            &[(&body, 2..bytes.len())],
        );
    }
    for tag in [8, 18, 24] {
        for n in 1..=10 {
            let bytes = [&[tag][..], vec![0x80; n].as_slice()].concat();
            error(
                &bytes,
                Default::default(),
                if n == 10 {
                    ErrorKind::MalformedVarint
                } else {
                    ErrorKind::Truncated
                },
                if n == 10 { 10 } else { n + 1 },
            );
        }
    }
}

#[test]
fn all_sixty_one_fixture_cuts_match_frozen_values_and_spans() {
    let mut cuts = 0;
    for (text, successes, kind) in [
        (SELECTED, vec![0, 2, 6, 8], 0),
        (DUPLICATES, vec![0, 2, 4, 8, 12, 14, 16, 22, 24, 30], 1),
        (OPAQUE, vec![0, 3, 5, 8, 12, 15], 2),
        (PAIR, vec![0, 4], 3),
    ] {
        let bytes = hex(text);
        for n in 0..=bytes.len() {
            cuts += 1;
            if !successes.contains(&n) {
                error(&bytes[..n], Default::default(), ErrorKind::Truncated, n);
                continue;
            }
            let mut projection = None;
            let mut id = None;
            let mut bodies: Vec<(&[u8], Range<usize>)> = Vec::new();
            match kind {
                0 => {
                    if n >= 2 {
                        projection = Some(Projection::Inv);
                    }
                    if n >= 6 {
                        bodies.push((&[0x30, 4], 4..6));
                    }
                    if n >= 8 {
                        id = Some(0);
                    }
                }
                1 => {
                    if n >= 2 {
                        projection = Some(Projection::In);
                    }
                    if n >= 8 {
                        bodies.push((&[0x30, 4], 6..8));
                    }
                    if n >= 12 {
                        bodies.push((&[0x18, 1], 10..12));
                    }
                    if n >= 14 {
                        bodies.push((&[], 14..14));
                    }
                    if n >= 16 {
                        id = Some(1);
                    }
                    if n >= 22 {
                        id = Some(-1);
                    }
                    if n >= 24 {
                        projection = Some(Projection::Star);
                    }
                }
                2 if n == 15 => bodies.push((&[0x80], 14..15)),
                3 if n == 4 => bodies.push((&[0x38, 0], 2..4)),
                _ => {}
            }
            check(&bytes[..n], Default::default(), projection, id, &bodies);
        }
    }
    assert_eq!(cuts, 61);
}

#[test]
fn exact_precedence_and_retained_capacity_growth() {
    for (text, work, kind, offset) in [
        ("12 00", 0, ErrorKind::Limit("work"), 0),
        ("12 00", 1, ErrorKind::Limit("work"), 0),
        ("12 00", 2, ErrorKind::Limit("work"), 1),
        ("12 00", 3, ErrorKind::Limit("work"), 0),
        ("12 00", 4, ErrorKind::Limit("work"), 0),
        ("18 00", 0, ErrorKind::Limit("work"), 0),
        ("18 00", 1, ErrorKind::Limit("work"), 0),
        ("18 00", 2, ErrorKind::Limit("work"), 1),
        ("18 00", 3, ErrorKind::Limit("work"), 0),
        ("18", 2, ErrorKind::Limit("work"), 1),
        ("18", 3, ErrorKind::Truncated, 1),
        ("12 02 ff", 4, ErrorKind::Limit("work"), 2),
        ("12 02 ff", 5, ErrorKind::Truncated, 3),
        ("18 00 80", 6, ErrorKind::Limit("work"), 3),
        ("18 00 80", 7, ErrorKind::Truncated, 3),
        ("18 00 00", 6, ErrorKind::InvalidFieldNumber, 2),
    ] {
        limited(text, usize::MAX, work, S, kind, offset);
    }
    limited("12 00", 0, 0, 0, ErrorKind::Limit("records"), 0);
    limited("18 00 80", 1, 4, 0, ErrorKind::Limit("records"), 2);
    limited("12 00", 1, 4, S - 1, ErrorKind::Limit("owned_bytes"), 0);
    limited("12 00", 1, 3, S - 1, ErrorKind::Limit("work"), 0);
    limited("12 00", 1, 4 + S, S - 1, ErrorKind::Limit("owned_bytes"), 0);
    check(
        &hex("18 00"),
        Limits {
            records: 1,
            work: 4,
            owned_bytes: 0,
            ..Default::default()
        },
        None,
        Some(0),
        &[],
    );
    for (k, capacity, growth, work_error) in [
        (1, 1, 0, 0),
        (2, 2, 2, 2),
        (3, 4, 4, 4),
        (4, 4, 4, 6),
        (5, 8, 8, 8),
    ] {
        let bytes = [18, 0].repeat(k);
        let bodies = (0..k)
            .map(|i| (&[][..], 2 * i + 2..2 * i + 2))
            .collect::<Vec<_>>();
        for extra in [0, 1] {
            check(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: k + extra,
                    work: 4 * k + capacity * S + extra,
                    owned_bytes: capacity * S + extra,
                },
                None,
                None,
                &bodies,
            );
        }
        error(
            &bytes,
            Limits {
                owned_bytes: capacity * S - 1,
                ..Default::default()
            },
            ErrorKind::Limit("owned_bytes"),
            growth,
        );
        error(
            &bytes,
            Limits {
                work: 4 * k + capacity * S - 1,
                ..Default::default()
            },
            ErrorKind::Limit("work"),
            work_error,
        );
    }
    let bytes = [&hex("12 80 08")[..], &[0xff; 1024]].concat();
    for work in [1028, 1029] {
        error(
            &bytes,
            Limits {
                work,
                ..Default::default()
            },
            ErrorKind::Limit("work"),
            0,
        );
    }
    check(
        &bytes,
        Limits {
            bytes: 1027,
            records: 1,
            work: 1029 + S,
            owned_bytes: S,
        },
        None,
        None,
        &[(&[0xff; 1024], 3..1027)],
    );
}

#[test]
fn caller_limits_cannot_raise_hard_bounds() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    check(&hex(PAIR), unlimited, None, None, &[(&[0x38, 0], 2..4)]);
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [24, 0].repeat(65_535);
    check(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140,
            owned_bytes: 0,
        },
        None,
        Some(0),
        &[],
    );
    check(&bytes, unlimited, None, Some(0), &[]);
    bytes.extend_from_slice(&[24, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), 131_070);
}

#[test]
fn genuine_pair_generic_return_composes_arguments_under_one_budget() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_type_argument/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    assert_eq!(bytes.len(), 285);
    let framed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.string_table_span, 1..47);
    assert_eq!(framed.payload_span, 47..285);
    let selected_class = class_header::decode(framed.payload, Default::default()).unwrap();
    assert_eq!(
        (selected_class.flags, selected_class.fq_name),
        (Some(1031), 0)
    );
    assert_eq!(selected_class.raw, &bytes[47..285]);
    assert_eq!(selected_class.raw.as_ptr(), bytes[47..285].as_ptr());
    let literal = hex(
        "10 11 1a 0e 12 04 12 02 38 00 12 04 12 02 38 01 30 00 32 08 08 02 10 05 1a 02 38 00 32 08 08 02 10 06 1a 02 38 01 48 c6 81 04 a2 06 02 10 12",
    );
    let fields = wire::decode(&bytes, Default::default()).unwrap();
    let mut functions = fields
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 9 && matches!(f.value, Value::Bytes(body) if body == literal));
    let function = functions.next().unwrap();
    assert!(functions.next().is_none());
    assert_eq!(function.span, 149..198);
    let Value::Bytes(body) = function.value else {
        panic!("Function body");
    };
    assert_eq!(body, &selected_class.raw[104..151]);
    assert_eq!(body.as_ptr(), bytes[151..198].as_ptr());
    let selected_function = function_header::decode(body, Default::default()).unwrap();
    assert_eq!(
        (
            selected_function.name,
            selected_function.flags,
            selected_function.old_flags
        ),
        (17, Some(65734), None)
    );
    assert_eq!(selected_function.raw, literal);
    assert_eq!(selected_function.raw.as_ptr(), body.as_ptr());
    assert_eq!(&selected_function.raw[2..4], &[0x1a, 14]);
    let inline = &selected_function.raw[4..18];
    assert_eq!(inline, hex("12 04 12 02 38 00 12 04 12 02 38 01 30 00"));
    assert_eq!(inline.as_ptr(), bytes[155..169].as_ptr());
    let a = size_of::<type_arguments::SelectedArgument<'_>>();
    let composed = type_arguments::decode(
        inline,
        type_arguments::Limits {
            bytes: 14,
            records: 8,
            nodes: 3,
            depth: 2,
            work: 55 + 2 * a + 2 * S,
            owned_bytes: 2 * a + 2 * S,
        },
    )
    .unwrap();
    let selected_type = &composed.header;
    assert_eq!(
        selected_type,
        &type_header::TypeHeader {
            raw: inline,
            class_name: Some(0),
            flags: None,
            nullable: None,
            flexible_type_capabilities_id: None,
            type_parameter: None,
            flexible_upper_bound_id: None,
            type_parameter_name: None,
            outer_type_id: None,
            type_alias_name: None,
            abbreviated_type_id: None
        }
    );
    assert_eq!(selected_type.raw.as_ptr(), inline.as_ptr());
    assert_eq!(composed.arguments.len(), 2);
    for (selected, (span, d1_span, id)) in composed
        .arguments
        .iter()
        .zip([(2..6, 157..161, 0), (8..12, 163..167, 1)])
    {
        assert_eq!(selected.span, span);
        let decoded = &selected.argument;
        let argument = decoded.raw;
        assert_eq!(argument, [18, 2, 56, id]);
        if id == 0 {
            assert_eq!(argument, hex(PAIR));
        }
        assert_eq!(argument.as_ptr(), bytes[d1_span].as_ptr());
        retained(decoded, &selected_type.raw[selected.span.clone()]);
        assert_eq!((decoded.projection, decoded.type_id), (None, None));
        assert_eq!(decoded.inline_types.len(), 1);
        let nested = &decoded.inline_types[0];
        assert_eq!(nested.span, 2..4);
        assert_eq!(nested.raw, [56, id]);
        let t = size_of::<type_argument_types::SelectedType<'_>>();
        let typed = type_argument_types::decode(
            argument,
            type_argument_types::Limits {
                bytes: 4,
                records: 2,
                nodes: 2,
                depth: 2,
                work: 12 + S + t,
                owned_bytes: S + t,
            },
        )
        .unwrap();
        assert_eq!(typed.raw, decoded.raw);
        assert_eq!(typed.raw.as_ptr(), decoded.raw.as_ptr());
        assert_eq!(
            (typed.projection, typed.type_id),
            (decoded.projection, decoded.type_id)
        );
        assert_eq!(typed.inline_types.len(), 1);
        assert_eq!(typed.inline_types[0].span, nested.span);
        let nested_type = &typed.inline_types[0].header;
        assert_eq!(
            nested_type,
            &type_header::TypeHeader {
                raw: nested.raw,
                class_name: None,
                flags: None,
                nullable: None,
                flexible_type_capabilities_id: None,
                type_parameter: Some(id.into()),
                flexible_upper_bound_id: None,
                type_parameter_name: None,
                outer_type_id: None,
                type_alias_name: None,
                abbreviated_type_id: None
            }
        );
        assert_eq!(nested_type.raw.as_ptr(), nested.raw.as_ptr());
    }
    let mut resolver = Resolver::new(&framed.string_table, &header.d2, Default::default()).unwrap();
    assert_eq!(
        resolver.get_string(0).unwrap(),
        [107, 111, 116, 108, 105, 110, 47, 80, 97, 105, 114]
    );
    assert_eq!(resolver.get_string(17).unwrap(), [99, 111, 112, 121]);
    assert_eq!(header.d2[17], [99, 111, 112, 121]);
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 512,
        owned_bytes: 256,
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
            probe(&[24, 0, a, b]);
        }
        for n in 0..32 {
            probe(&vec![a; n]);
        }
    }
}
