use ktlsp_binary::metadata::{
    self,
    function_header::{Error, ErrorKind, FunctionHeader, Limits, decode},
    name_resolver::Resolver,
    payload,
    wire::{self, ErrorKind as WireKind, Value},
};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check_raw(out: &FunctionHeader<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn values(text: &str, flags: Option<i32>, old_flags: Option<i32>, name: i32) {
    let bytes = hex(text);
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(
        (out.flags, out.old_flags, out.name),
        (flags, old_flags, name),
        "{text}"
    );
    check_raw(&out, &bytes);
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn wire_error(text: &str, kind: WireKind, offset: usize) {
    error(&hex(text), Limits::default(), ErrorKind::Wire(kind), offset);
}
fn limited(bytes: &[u8], records: usize, work: usize, kind: ErrorKind, offset: usize) {
    error(
        bytes,
        Limits {
            records,
            work,
            ..Limits::default()
        },
        kind,
        offset,
    );
}

#[test]
fn frozen_fixtures_and_exact_limits() {
    for (text, flags, old_flags, name, fields, work, last) in [
        (
            include_str!("fixtures/metadata_function_header/minimal.hex"),
            None,
            None,
            0,
            1,
            4,
            0,
        ),
        (
            include_str!("fixtures/metadata_function_header/explicit.hex"),
            Some(6),
            Some(6),
            0,
            3,
            12,
            4,
        ),
        (
            include_str!("fixtures/metadata_function_header/duplicates.hex"),
            Some(0),
            Some(6),
            0,
            9,
            39,
            18,
        ),
        (
            include_str!("fixtures/metadata_function_header/opaque.hex"),
            None,
            None,
            0,
            6,
            28,
            12,
        ),
        (
            include_str!("fixtures/metadata_function_header/unit_function.hex"),
            Some(65558),
            None,
            3,
            3,
            16,
            6,
        ),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            let out = decode(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: fields + extra,
                    work: work + extra,
                },
            )
            .unwrap();
            assert_eq!(
                (out.flags, out.old_flags, out.name),
                (flags, old_flags, name)
            );
            check_raw(&out, &bytes);
        }
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                work: 0,
            },
            ErrorKind::Wire(WireKind::Limit("bytes")),
            0,
        );
        limited(
            &bytes,
            fields - 1,
            usize::MAX,
            ErrorKind::Wire(WireKind::Limit("records")),
            last,
        );
        limited(
            &bytes,
            usize::MAX,
            work - 1,
            ErrorKind::Wire(WireKind::Limit("work")),
            last,
        );
    }
}

#[test]
fn presence_independent_flags_and_duplicate_order() {
    for (text, flags, old_flags, name) in [
        ("10 00", None, None, 0),
        ("08 00 10 00", None, Some(0), 0),
        ("48 00 10 00", Some(0), None, 0),
        ("08 06 10 00", None, Some(6), 0),
        ("48 06 10 00", Some(6), None, 0),
        ("48 02 08 01 10 ac 02", Some(2), Some(1), 300),
        ("08 01 10 00 48 02", Some(2), Some(1), 0),
        ("48 02 10 00 08 01", Some(2), Some(1), 0),
        ("10 01 10 02", None, None, 2),
        ("08 01 08 02 10 00", None, Some(2), 0),
        ("48 01 48 02 10 00", Some(2), None, 0),
        ("0a 01 80 10 00", None, None, 0),
        ("4a 01 80 10 00", None, None, 0),
        ("12 01 80 10 00", None, None, 0),
        ("08 01 0a 00 10 00", None, Some(1), 0),
        ("48 01 4a 00 10 00", Some(1), None, 0),
        ("10 01 12 00", None, None, 1),
        ("15 00 00 00 00 10 00", None, None, 0),
        ("11 00 00 00 00 00 00 00 00 10 00", None, None, 0),
        ("90 00 80 00", None, None, 0),
        ("90 80 80 80 10 00", None, None, 0),
    ] {
        values(text, flags, old_flags, name);
    }
    for text in ["", "08 06", "48 06", "08 06 48 06", "12 00"] {
        let bytes = hex(text);
        error(
            &bytes,
            Limits::default(),
            ErrorKind::MissingName,
            bytes.len(),
        );
    }
    error(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            work: 0,
        },
        ErrorKind::MissingName,
        0,
    );
}

#[test]
fn wrong_wire_never_sets_or_erases_selected_scalars() {
    for number in [1_u8, 2, 9] {
        for (wire, body) in [
            (1, "00 00 00 00 00 00 00 00"),
            (2, "01 80"),
            (5, "00 00 00 00"),
        ] {
            let wrong = format!("{:02x} {body}", (number << 3) | wire);
            let wrong_bytes = hex(&wrong);
            if number == 2 {
                error(
                    &wrong_bytes,
                    Limits::default(),
                    ErrorKind::MissingName,
                    wrong_bytes.len(),
                );
            } else {
                values(&format!("{wrong} 10 00"), None, None, 0);
            }
            for text in [
                format!("{wrong} 08 01 10 02 48 03"),
                format!("08 01 10 02 48 03 {wrong}"),
            ] {
                values(&text, Some(3), Some(1), 2);
            }
        }
    }
}

#[test]
fn signed_low32_and_nonminimal_values_for_all_three_fields() {
    for (text, value) in [
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("ff ff ff ff ff ff ff ff ff 7f", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 80 80 80 10", 0),
        ("80 80 80 80 80 80 80 80 80 02", 0),
        ("80 00", 0),
    ] {
        values(&format!("10 {text}"), None, None, value);
        values(&format!("08 {text} 10 00"), None, Some(value), 0);
        values(&format!("48 {text} 10 00"), Some(value), None, 0);
        values(
            &format!("08 01 10 02 48 03 08 {text}"),
            Some(3),
            Some(value),
            2,
        );
        values(
            &format!("08 01 10 02 48 03 10 {text}"),
            Some(3),
            Some(1),
            value,
        );
        values(
            &format!("08 01 10 02 48 03 48 {text}"),
            Some(value),
            Some(1),
            2,
        );
    }
}

#[test]
fn unknown_wires_and_nested_contents_stay_opaque() {
    for tail in [
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 00",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
    ] {
        values(&format!("10 00 {tail}"), None, None, 0);
    }
    let mut bytes = hex("10 00 32 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    limited(&bytes, 2, 1032, ErrorKind::Wire(WireKind::Limit("work")), 2);
    let out = decode(
        &bytes,
        Limits {
            work: 1033,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!((out.flags, out.old_flags, out.name), (None, None, 0));
    check_raw(&out, &bytes);
}

#[test]
fn structural_errors_precede_missing_or_present_name() {
    for (text, kind, offset) in [
        ("10", WireKind::Truncated, 1),
        ("80", WireKind::Truncated, 1),
        ("00", WireKind::InvalidFieldNumber, 0),
        ("03", WireKind::InvalidFieldNumber, 0),
        ("07", WireKind::InvalidFieldNumber, 0),
        ("0b", WireKind::UnsupportedGroup, 0),
        ("0c", WireKind::UnsupportedGroup, 0),
        ("13", WireKind::UnsupportedGroup, 0),
        ("14", WireKind::UnsupportedGroup, 0),
        ("4b", WireKind::UnsupportedGroup, 0),
        ("4c", WireKind::UnsupportedGroup, 0),
        ("0e", WireKind::InvalidWireType, 0),
        ("0f", WireKind::InvalidWireType, 0),
        ("16", WireKind::InvalidWireType, 0),
        ("17", WireKind::InvalidWireType, 0),
        ("4e", WireKind::InvalidWireType, 0),
        ("4f", WireKind::InvalidWireType, 0),
        ("12 02 ff", WireKind::Truncated, 3),
        ("0d 01 02 03", WireKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", WireKind::Truncated, 8),
        ("12 80 80 80 80 08", WireKind::NegativeLength, 1),
        ("12 ff ff ff ff 07", WireKind::Truncated, 6),
        (
            "10 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            9,
        ),
        (
            "12 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
    ] {
        wire_error(text, kind, offset);
        for prefix in ["10 00", "08 06", "48 06", "08 06 10 00 48 06"] {
            wire_error(
                &format!("{prefix} {text}"),
                kind,
                offset + hex(prefix).len(),
            );
        }
    }
    for tag in ["08", "10", "48"] {
        for n in 1..=10 {
            wire_error(
                &format!("{tag} {}", "80 ".repeat(n)),
                if n == 10 {
                    WireKind::MalformedVarint
                } else {
                    WireKind::Truncated
                },
                if n == 10 { 10 } else { n + 1 },
            );
        }
    }
}

#[test]
fn all_sixty_fixture_cuts_have_frozen_exact_expectations() {
    let mut cuts = 0;
    for (text, truncated, missing, successes) in [
        (
            include_str!("fixtures/metadata_function_header/minimal.hex"),
            vec![1],
            vec![0],
            vec![(2, None, None, 0)],
        ),
        (
            include_str!("fixtures/metadata_function_header/explicit.hex"),
            vec![1, 3, 5],
            vec![0, 2],
            vec![(4, None, Some(6), 0), (6, Some(6), Some(6), 0)],
        ),
        (
            include_str!("fixtures/metadata_function_header/duplicates.hex"),
            vec![1, 3, 5, 7, 9, 11, 13, 14, 16, 17, 19, 20],
            vec![0, 2, 4],
            vec![
                (6, Some(2), Some(1), 1),
                (8, Some(2), Some(6), 1),
                (10, Some(0), Some(6), 1),
                (12, Some(0), Some(6), 0),
                (15, Some(0), Some(6), 0),
                (18, Some(0), Some(6), 0),
                (21, Some(0), Some(6), 0),
            ],
        ),
        (
            include_str!("fixtures/metadata_function_header/opaque.hex"),
            vec![1, 3, 4, 6, 8, 10, 11, 13, 14, 15],
            vec![0],
            vec![
                (2, None, None, 0),
                (5, None, None, 0),
                (7, None, None, 0),
                (9, None, None, 0),
                (12, None, None, 0),
                (16, None, None, 0),
            ],
        ),
        (
            include_str!("fixtures/metadata_function_header/unit_function.hex"),
            vec![1, 3, 4, 5, 7, 8, 9],
            vec![0],
            vec![
                (2, None, None, 3),
                (6, None, None, 3),
                (10, Some(65558), None, 3),
            ],
        ),
    ] {
        let bytes = hex(text);
        let mut covered = vec![false; bytes.len() + 1];
        for n in truncated {
            assert!(!covered[n]);
            covered[n] = true;
            error(
                &bytes[..n],
                Limits::default(),
                ErrorKind::Wire(WireKind::Truncated),
                n,
            );
        }
        for n in missing {
            assert!(!covered[n]);
            covered[n] = true;
            error(&bytes[..n], Limits::default(), ErrorKind::MissingName, n);
        }
        for (n, flags, old_flags, name) in successes {
            assert!(!covered[n]);
            covered[n] = true;
            let out = decode(&bytes[..n], Limits::default()).unwrap();
            assert_eq!(
                (out.flags, out.old_flags, out.name),
                (flags, old_flags, name)
            );
            check_raw(&out, &bytes[..n]);
        }
        assert!(covered.iter().all(|&n| n));
        cuts += covered.len();
    }
    assert_eq!(cuts, 60);
}

#[test]
fn exact_work_records_and_failure_precedence() {
    for (text, work, kind, offset) in [
        ("10 00", 0, WireKind::Limit("work"), 0),
        ("10 00", 1, WireKind::Limit("work"), 0),
        ("10 00", 2, WireKind::Limit("work"), 1),
        ("10 00", 3, WireKind::Limit("work"), 0),
        ("10", 2, WireKind::Limit("work"), 1),
        ("10", 3, WireKind::Truncated, 1),
        ("10 00 80", 6, WireKind::Limit("work"), 3),
        ("10 00 80", 7, WireKind::Truncated, 3),
        ("10 00 00", 6, WireKind::InvalidFieldNumber, 2),
        ("12 00", 3, WireKind::Limit("work"), 0),
        ("12 02 ff", 4, WireKind::Limit("work"), 2),
        ("12 02 ff", 5, WireKind::Truncated, 3),
        ("10 00 32 01 80", 8, WireKind::Limit("work"), 2),
        ("10 00 32 03 ff ff ff", 9, WireKind::Limit("work"), 4),
        ("10 00 32 03 ff ff ff", 10, WireKind::Limit("work"), 2),
    ] {
        limited(&hex(text), usize::MAX, work, ErrorKind::Wire(kind), offset);
    }
    limited(
        &hex("10 00"),
        0,
        0,
        ErrorKind::Wire(WireKind::Limit("records")),
        0,
    );
    limited(
        &hex("10 00 80"),
        1,
        4,
        ErrorKind::Wire(WireKind::Limit("records")),
        2,
    );
    limited(&hex("12 00"), 1, 4, ErrorKind::MissingName, 2);
    for (text, work) in [
        ("10 00", 4),
        ("10 00 32 01 80", 9),
        ("10 00 32 03 ff ff ff", 11),
    ] {
        let bytes = hex(text);
        check_raw(
            &decode(
                &bytes,
                Limits {
                    work,
                    ..Limits::default()
                },
            )
            .unwrap(),
            &bytes,
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
    values("10 00", None, None, 0);
    let minimal = [16, 0];
    check_raw(&decode(&minimal, unlimited).unwrap(), &minimal);
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    let mut bytes = hex("10 00");
    for _ in 0..65_534 {
        bytes.extend_from_slice(&[8, 0]);
    }
    let out = decode(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140,
        },
    )
    .unwrap();
    assert_eq!((out.flags, out.old_flags, out.name), (None, Some(0), 0));
    check_raw(&out, &bytes);
    bytes.extend_from_slice(&[72, 0]);
    error(
        &bytes,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("records")),
        131_070,
    );
}

#[test]
fn genuine_unit_function_composes_under_separate_budgets() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported header");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload_span, 19..52);
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
    assert_eq!(
        body,
        hex(include_str!(
            "fixtures/metadata_function_header/unit_function.hex"
        ))
    );
    assert_eq!(body.as_ptr(), bytes[39..49].as_ptr());
    let selected = decode(
        body,
        Limits {
            bytes: 10,
            records: 3,
            work: 16,
        },
    )
    .unwrap();
    assert_eq!(
        (selected.flags, selected.old_flags, selected.name),
        (Some(65558), None, 3)
    );
    check_raw(&selected, &bytes[39..49]);
    assert_eq!(header.d2[3], [116, 111, 83, 116, 114, 105, 110, 103]);
    let mut resolver = Resolver::new(&framed.string_table, &header.d2, Default::default()).unwrap();
    assert_eq!(
        resolver.get_string(selected.name).unwrap(),
        [116, 111, 83, 116, 114, 105, 110, 103]
    );
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
    };
    let check = |bytes: &[u8]| {
        let out = decode(bytes, limits);
        assert_eq!(out, decode(bytes, limits));
        match out {
            Ok(out) => check_raw(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            check(&[a, b]);
            check(&[16, 0, a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
