use ktlsp_binary::metadata::{
    self, class_header, function_header,
    name_resolver::Resolver,
    payload,
    type_header::{Error, ErrorKind, Limits, TypeHeader, decode},
    wire::{self, Value},
};

const TAGS: [u8; 10] = [8, 24, 32, 48, 56, 64, 72, 88, 96, 112];
const SELECTED: [Option<i32>; 10] = [
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
const SELECTED_HEX: &str = include_str!("fixtures/metadata_type_header/selected.hex");
const OPAQUE_HEX: &str = include_str!("fixtures/metadata_type_header/opaque.hex");
const UNIT_HEX: &str = include_str!("fixtures/metadata_type_header/unit_type.hex");

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
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
fn check_raw(out: &TypeHeader<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn check(bytes: &[u8], limits: Limits, expected: [Option<i32>; 10]) {
    let out = decode(bytes, limits).unwrap();
    assert_eq!(scalars(&out), expected, "{bytes:02x?}");
    check_raw(&out, bytes);
}
fn one(index: usize, value: i32) -> [Option<i32>; 10] {
    let mut expected = [None; 10];
    expected[index] = Some(value);
    expected
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn limited(text: &str, records: usize, work: usize, kind: ErrorKind, offset: usize) {
    error(
        &hex(text),
        Limits {
            records,
            work,
            ..Default::default()
        },
        kind,
        offset,
    );
}

#[test]
fn frozen_fixtures_and_exact_limits() {
    for (text, expected, fields, work, last) in [
        (SELECTED_HEX, SELECTED, 10, 40, 18),
        (OPAQUE_HEX, [None; 10], 10, 47, 24),
        (UNIT_HEX, one(3, 4), 1, 4, 0),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            check(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: fields + extra,
                    work: work + extra,
                },
                expected,
            );
        }
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                work: 0,
            },
            ErrorKind::Limit("bytes"),
            0,
        );
        limited(
            text,
            fields - 1,
            usize::MAX,
            ErrorKind::Limit("records"),
            last,
        );
        limited(text, usize::MAX, work - 1, ErrorKind::Limit("work"), last);
    }
    check(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            work: 0,
        },
        [None; 10],
    );
}

#[test]
fn every_scalar_presence_duplicates_and_wrong_wires() {
    for (index, tag) in TAGS.into_iter().enumerate() {
        for value in [0, 1] {
            check(&[tag, value], Default::default(), one(index, value.into()));
        }
        check(&[tag, 0, tag, 1, tag, 0], Default::default(), one(index, 0));
        for (wire, body) in [
            (1, "01 02 03 04 05 06 07 08"),
            (2, "01 80"),
            (5, "01 02 03 04"),
        ] {
            let wrong = hex(&format!("{:02x} {body}", tag | wire));
            check(&wrong, Default::default(), [None; 10]);
            for bytes in [
                [wrong.as_slice(), hex(SELECTED_HEX).as_slice()].concat(),
                [hex(SELECTED_HEX).as_slice(), wrong.as_slice()].concat(),
            ] {
                check(&bytes, Default::default(), SELECTED);
            }
        }
    }
    check(&hex("b0 00 84 00"), Default::default(), one(3, 4));
    check(&hex("b0 80 80 80 10 04"), Default::default(), one(3, 4));
}

#[test]
fn signed_low32_for_all_nine_integer_fields() {
    for (text, value) in [
        ("00", 0),
        ("80 00", 0),
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("ff ff ff ff ff ff ff ff ff 7f", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 80 80 80 10", 0),
        ("80 80 80 80 80 80 80 80 80 02", 0),
    ] {
        for (index, tag) in TAGS.into_iter().enumerate().filter(|(i, _)| *i != 1) {
            let bytes = hex(&format!("{tag:02x} {text}"));
            check(&bytes, Default::default(), one(index, value));
            let mut duplicate = hex(SELECTED_HEX);
            duplicate.extend_from_slice(&bytes);
            let mut expected = SELECTED;
            expected[index] = Some(value);
            check(&duplicate, Default::default(), expected);
        }
    }
}

#[test]
fn nullable_uses_full_decoded_u64_not_low32_or_strict_boolean() {
    for (text, value) in [
        ("00", 0),
        ("80 00", 0),
        ("01", 1),
        ("02", 1),
        ("ff ff ff ff 0f", 1),
        ("80 80 80 80 08", 1),
        ("80 80 80 80 10", 1),
        ("ff ff ff ff ff ff ff ff ff 01", 1),
        ("ff ff ff ff ff ff ff ff ff 7f", 1),
        ("80 80 80 80 80 80 80 80 80 02", 0),
    ] {
        check(
            &hex(&format!("18 {text}")),
            Default::default(),
            one(1, value),
        );
    }
    check(&hex("18 01 18 00"), Default::default(), one(1, 0));
    check(&hex("18 00 18 01"), Default::default(), one(1, 1));
}

#[test]
fn unknown_fields_and_duplicate_nested_messages_stay_raw() {
    check(&hex(OPAQUE_HEX), Default::default(), [None; 10]);
    for text in [
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
    ] {
        check(&hex(text), Default::default(), [None; 10]);
        check(
            &hex(&format!("30 04 {text}")),
            Default::default(),
            one(3, 4),
        );
    }
    let mut bytes = hex("12 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    error(
        &bytes,
        Limits {
            work: 1028,
            ..Default::default()
        },
        ErrorKind::Limit("work"),
        0,
    );
    check(
        &bytes,
        Limits {
            bytes: 1027,
            records: 1,
            work: 1029,
        },
        [None; 10],
    );
}

#[test]
fn structural_errors_and_malformed_tails_never_return_partial_success() {
    for (text, kind, offset) in [
        ("30", ErrorKind::Truncated, 1),
        ("80", ErrorKind::Truncated, 1),
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("03", ErrorKind::InvalidFieldNumber, 0),
        ("07", ErrorKind::InvalidFieldNumber, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0c", ErrorKind::UnsupportedGroup, 0),
        ("1b", ErrorKind::UnsupportedGroup, 0),
        ("1c", ErrorKind::UnsupportedGroup, 0),
        ("33", ErrorKind::UnsupportedGroup, 0),
        ("34", ErrorKind::UnsupportedGroup, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
        ("1e", ErrorKind::InvalidWireType, 0),
        ("1f", ErrorKind::InvalidWireType, 0),
        ("36", ErrorKind::InvalidWireType, 0),
        ("37", ErrorKind::InvalidWireType, 0),
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
            "30 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        (
            "12 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
    ] {
        error(&hex(text), Default::default(), kind, offset);
        error(
            &hex(&format!("30 04 {text}")),
            Default::default(),
            kind,
            offset + 2,
        );
        let body = hex(text);
        let mut opaque = vec![18, body.len() as u8];
        opaque.extend_from_slice(&body);
        check(&opaque, Default::default(), [None; 10]);
    }
    for tag in TAGS {
        for n in 1..=10 {
            let mut bytes = vec![tag];
            bytes.extend(std::iter::repeat_n(0x80, n));
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
fn all_fifty_two_fixture_cuts_have_frozen_expectations() {
    let mut cuts = 0;
    for (text, truncated, successes, kind) in [
        (
            SELECTED_HEX,
            vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19],
            vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20],
            0,
        ),
        (
            OPAQUE_HEX,
            vec![1, 2, 4, 6, 7, 9, 11, 12, 14, 16, 17, 19, 21, 22, 23, 25, 26],
            vec![0, 3, 5, 8, 10, 13, 15, 18, 20, 24, 27],
            1,
        ),
        (UNIT_HEX, vec![1], vec![0, 2], 2),
    ] {
        let bytes = hex(text);
        let mut covered = vec![false; bytes.len() + 1];
        for n in truncated {
            assert!(!covered[n]);
            covered[n] = true;
            error(&bytes[..n], Default::default(), ErrorKind::Truncated, n);
        }
        for n in successes {
            assert!(!covered[n]);
            covered[n] = true;
            let mut expected = [None; 10];
            if kind == 0 {
                expected[..n / 2].copy_from_slice(&SELECTED[..n / 2]);
            }
            if kind == 2 && n == 2 {
                expected = one(3, 4);
            }
            check(&bytes[..n], Default::default(), expected);
        }
        assert!(covered.iter().all(|&value| value));
        cuts += covered.len();
    }
    assert_eq!(cuts, 52);
}

#[test]
fn exact_work_record_and_error_precedence() {
    for (text, work, kind, offset) in [
        ("30 04", 0, ErrorKind::Limit("work"), 0),
        ("30 04", 1, ErrorKind::Limit("work"), 0),
        ("30 04", 2, ErrorKind::Limit("work"), 1),
        ("30 04", 3, ErrorKind::Limit("work"), 0),
        ("30", 2, ErrorKind::Limit("work"), 1),
        ("30", 3, ErrorKind::Truncated, 1),
        ("30 04 80", 6, ErrorKind::Limit("work"), 3),
        ("30 04 80", 7, ErrorKind::Truncated, 3),
        ("30 04 00", 6, ErrorKind::InvalidFieldNumber, 2),
        ("12 00", 3, ErrorKind::Limit("work"), 0),
        ("12 02 ff", 4, ErrorKind::Limit("work"), 2),
        ("12 02 ff", 5, ErrorKind::Truncated, 3),
        ("30 04 12 01 80", 8, ErrorKind::Limit("work"), 2),
        ("30 04 12 03 ff ff ff", 9, ErrorKind::Limit("work"), 4),
        ("30 04 12 03 ff ff ff", 10, ErrorKind::Limit("work"), 2),
    ] {
        limited(text, usize::MAX, work, kind, offset);
    }
    limited("30 04", 0, 0, ErrorKind::Limit("records"), 0);
    limited("30 04 80", 1, 4, ErrorKind::Limit("records"), 2);
    check(
        &hex("12 00"),
        Limits {
            records: 1,
            work: 4,
            ..Default::default()
        },
        [None; 10],
    );
    for (text, work) in [
        ("30 04", 4),
        ("30 04 12 01 80", 9),
        ("30 04 12 03 ff ff ff", 11),
    ] {
        check(
            &hex(text),
            Limits {
                work,
                ..Default::default()
            },
            one(3, 4),
        );
    }
}

#[test]
fn caller_limits_cannot_raise_hard_bounds() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
    };
    check(&[48, 4], unlimited, one(3, 4));
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [48, 0].repeat(65_535);
    check(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140,
        },
        one(3, 0),
    );
    check(&bytes, unlimited, one(3, 0));
    bytes.extend_from_slice(&[48, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), 131_070);
}

#[test]
fn genuine_unit_inline_return_type_composes_under_separate_budgets() {
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
    let mut functions = fields
        .payload
        .fields
        .iter()
        .filter(|field| field.number == 9);
    let function = functions.next().unwrap();
    assert!(functions.next().is_none());
    assert_eq!(function.span, 37..49);
    let Value::Bytes(body) = function.value else {
        panic!("Function bytes");
    };
    assert_eq!(body, hex("10 03 1a 02 30 04 48 96 80 04"));
    assert_eq!(body, &selected_class.raw[20..30]);
    assert_eq!(body.as_ptr(), selected_class.raw[20..30].as_ptr());
    let selected_function = function_header::decode(body, Default::default()).unwrap();
    assert_eq!(
        (
            selected_function.name,
            selected_function.flags,
            selected_function.old_flags
        ),
        (3, Some(65558), None)
    );
    assert_eq!(selected_function.raw, body);
    assert_eq!(selected_function.raw.as_ptr(), bytes[39..49].as_ptr());
    assert_eq!(&selected_function.raw[2..4], &[0x1a, 2]);
    let inline_type = &selected_function.raw[4..6];
    assert_eq!(inline_type, hex(UNIT_HEX));
    assert_eq!(inline_type.as_ptr(), bytes[43..45].as_ptr());
    let selected = decode(
        inline_type,
        Limits {
            bytes: 2,
            records: 1,
            work: 4,
        },
    )
    .unwrap();
    assert_eq!(scalars(&selected), one(3, 4));
    check_raw(&selected, inline_type);
    let mut resolver = Resolver::new(&framed.string_table, &header.d2, Default::default()).unwrap();
    assert_eq!(
        resolver.get_string(selected.class_name.unwrap()).unwrap(),
        [
            107, 111, 116, 108, 105, 110, 47, 83, 116, 114, 105, 110, 103
        ]
    );
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
    };
    let probe = |bytes: &[u8]| {
        let result = decode(bytes, limits);
        assert_eq!(result, decode(bytes, limits));
        match result {
            Ok(out) => check_raw(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            probe(&[a, b]);
            probe(&[48, 4, a, b]);
        }
        for n in 0..32 {
            probe(&vec![a; n]);
        }
    }
}
