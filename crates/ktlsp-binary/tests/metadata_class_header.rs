use ktlsp_binary::metadata::{
    self,
    class_header::{ClassHeader, Error, ErrorKind, Limits, decode},
    name_resolver::Resolver,
    payload,
    wire::ErrorKind as WireKind,
};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check_raw(out: &ClassHeader<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn values(text: &str, flags: Option<i32>, fq_name: i32) {
    let bytes = hex(text);
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!((out.flags, out.fq_name), (flags, fq_name), "{text}");
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
fn frozen_authored_fixtures_and_exact_bounds() {
    for (text, flags, fields, work) in [
        (
            include_str!("fixtures/metadata_class_header/minimal.hex"),
            None,
            1,
            4,
        ),
        (
            include_str!("fixtures/metadata_class_header/explicit.hex"),
            Some(6),
            2,
            8,
        ),
        (
            include_str!("fixtures/metadata_class_header/duplicates.hex"),
            Some(6),
            6,
            26,
        ),
        (
            include_str!("fixtures/metadata_class_header/opaque.hex"),
            None,
            5,
            24,
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
            assert_eq!((out.flags, out.fq_name), (flags, 0));
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
    }
    for (text, records, work, offset) in [
        (
            include_str!("fixtures/metadata_class_header/duplicates.hex"),
            5,
            25,
            11,
        ),
        (
            include_str!("fixtures/metadata_class_header/opaque.hex"),
            4,
            23,
            10,
        ),
    ] {
        let bytes = hex(text);
        limited(
            &bytes,
            records,
            usize::MAX,
            ErrorKind::Wire(WireKind::Limit("records")),
            offset,
        );
        limited(
            &bytes,
            usize::MAX,
            work,
            ErrorKind::Wire(WireKind::Limit("work")),
            offset,
        );
    }
}

#[test]
fn presence_defaults_duplicates_and_wrong_wire() {
    for (text, flags, name) in [
        ("18 00", None, 0),
        ("08 00 18 00", Some(0), 0),
        ("08 06 18 00", Some(6), 0),
        ("18 ac 02 08 02", Some(2), 300),
        ("18 01 18 02", None, 2),
        ("08 01 08 02 18 00", Some(2), 0),
        ("1a 01 80 18 00", None, 0),
        ("18 01 1a 00", None, 1),
        ("0a 00 18 00", None, 0),
        ("08 01 0a 00 18 00", Some(1), 0),
        ("1d 00 00 00 00 18 00", None, 0),
        ("19 00 00 00 00 00 00 00 00 18 00", None, 0),
        ("98 00 80 00", None, 0),
        ("98 80 80 80 10 00", None, 0),
    ] {
        values(text, flags, name);
    }
    for text in ["", "08 06", "1a 00"] {
        let bytes = hex(text);
        error(
            &bytes,
            Limits::default(),
            ErrorKind::MissingFqName,
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
        ErrorKind::MissingFqName,
        0,
    );
}

#[test]
fn signed_low32_and_nonminimal_values_for_both_scalars() {
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
        values(&format!("18 {text}"), None, value);
        values(&format!("08 {text} 18 00"), Some(value), 0);
    }
}

#[test]
fn unknown_wires_and_nested_contents_stay_opaque() {
    for text in [
        "18 00 c0 a3 09 01",       // Reserved number19000 remains raw.
        "18 00 f8 ff ff ff 0f 01", // Maximum number536870911.
        "18 00 a0 06 01",
        "18 00 a1 06 01 02 03 04 05 06 07 08",
        "18 00 a2 06 00",
        "18 00 a2 06 03 ff 00 0b",
        "18 00 a5 06 01 02 03 04",
        "18 00 2a 00 4a 01 80 12 01 80 a2 06 01 00",
    ] {
        values(text, None, 0);
    }
    let mut bytes = hex("18 00 4a 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    check_raw(&decode(&bytes, Limits::default()).unwrap(), &bytes);
    limited(&bytes, 2, 1032, ErrorKind::Wire(WireKind::Limit("work")), 2);
    assert!(
        decode(
            &bytes,
            Limits {
                work: 1033,
                ..Limits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn malformed_top_level_fields_precede_missing_or_present_name() {
    for (text, kind, offset) in [
        ("18", WireKind::Truncated, 1),
        ("80", WireKind::Truncated, 1),
        ("00", WireKind::InvalidFieldNumber, 0),
        ("03", WireKind::InvalidFieldNumber, 0),
        ("07", WireKind::InvalidFieldNumber, 0),
        ("0b", WireKind::UnsupportedGroup, 0),
        ("0c", WireKind::UnsupportedGroup, 0),
        ("1b", WireKind::UnsupportedGroup, 0),
        ("1c", WireKind::UnsupportedGroup, 0),
        ("0e", WireKind::InvalidWireType, 0),
        ("0f", WireKind::InvalidWireType, 0),
        ("1a 02 ff", WireKind::Truncated, 3),
        ("0d 01 02 03", WireKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", WireKind::Truncated, 8),
        ("1a 80 80 80 80 08", WireKind::NegativeLength, 1),
        ("1a ff ff ff ff 07", WireKind::Truncated, 6),
        (
            "18 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            9,
        ),
        (
            "1a 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
    ] {
        wire_error(text, kind, offset);
        for prefix in ["18 00", "08 06"] {
            wire_error(&format!("{prefix} {text}"), kind, offset + 2);
        }
    }
}

#[test]
fn every_authored_fixture_cut_has_independent_expectation() {
    for (text, truncations, missing, successes) in [
        (
            include_str!("fixtures/metadata_class_header/minimal.hex"),
            vec![1],
            vec![0],
            vec![(2, None, 0)],
        ),
        (
            include_str!("fixtures/metadata_class_header/explicit.hex"),
            vec![1, 3],
            vec![0, 2],
            vec![(4, Some(6), 0)],
        ),
        (
            include_str!("fixtures/metadata_class_header/duplicates.hex"),
            vec![1, 3, 5, 7, 9, 10, 12, 13],
            vec![0, 2],
            vec![
                (4, Some(1), 1),
                (6, Some(6), 1),
                (8, Some(6), 0),
                (11, Some(6), 0),
                (14, Some(6), 0),
            ],
        ),
        (
            include_str!("fixtures/metadata_class_header/opaque.hex"),
            vec![1, 3, 5, 6, 8, 9, 11, 12, 13],
            vec![0],
            vec![
                (2, None, 0),
                (4, None, 0),
                (7, None, 0),
                (10, None, 0),
                (14, None, 0),
            ],
        ),
    ] {
        let bytes = hex(text);
        assert_eq!(
            truncations.len() + missing.len() + successes.len(),
            bytes.len() + 1
        );
        for n in truncations {
            error(
                &bytes[..n],
                Limits::default(),
                ErrorKind::Wire(WireKind::Truncated),
                n,
            );
        }
        for n in missing {
            error(&bytes[..n], Limits::default(), ErrorKind::MissingFqName, n);
        }
        for (n, flags, name) in successes {
            let out = decode(&bytes[..n], Limits::default()).unwrap();
            assert_eq!((out.flags, out.fq_name), (flags, name));
            check_raw(&out, &bytes[..n]);
        }
    }
    for n in 1..=10 {
        wire_error(
            &"80 ".repeat(n),
            if n == 10 {
                WireKind::MalformedVarint
            } else {
                WireKind::Truncated
            },
            if n == 10 { 9 } else { n },
        );
    }
}

#[test]
fn exact_work_records_and_failure_precedence() {
    for (text, work, kind, offset) in [
        ("18 00", 0, WireKind::Limit("work"), 0),
        ("18 00", 1, WireKind::Limit("work"), 0),
        ("18 00", 2, WireKind::Limit("work"), 1),
        ("18 00", 3, WireKind::Limit("work"), 0),
        ("18", 2, WireKind::Limit("work"), 1),
        ("18", 3, WireKind::Truncated, 1),
        ("18 00 80", 6, WireKind::Limit("work"), 3),
        ("18 00 80", 7, WireKind::Truncated, 3),
        ("18 00 00", 6, WireKind::InvalidFieldNumber, 2),
        ("1a 00", 3, WireKind::Limit("work"), 0),
        ("1a 02 ff", 4, WireKind::Limit("work"), 2),
        ("1a 02 ff", 5, WireKind::Truncated, 3),
        ("18 00 2a 01 80", 8, WireKind::Limit("work"), 2),
        ("18 00 2a 03 ff ff ff", 9, WireKind::Limit("work"), 4),
        ("18 00 2a 03 ff ff ff", 10, WireKind::Limit("work"), 2),
    ] {
        limited(&hex(text), usize::MAX, work, ErrorKind::Wire(kind), offset);
    }
    limited(
        &hex("18 00"),
        0,
        0,
        ErrorKind::Wire(WireKind::Limit("records")),
        0,
    );
    limited(
        &hex("18"),
        0,
        0,
        ErrorKind::Wire(WireKind::Limit("records")),
        0,
    );
    limited(
        &hex("18 00 80"),
        1,
        0,
        ErrorKind::Wire(WireKind::Limit("work")),
        0,
    );
    limited(
        &hex("18 00 80"),
        1,
        4,
        ErrorKind::Wire(WireKind::Limit("records")),
        2,
    );
    limited(&hex("1a 00"), 1, 4, ErrorKind::MissingFqName, 2);
    for (text, work) in [
        ("18 00", 4),
        ("18 00 2a 01 80", 9),
        ("18 00 2a 03 ff ff ff", 11),
    ] {
        assert!(
            decode(
                &hex(text),
                Limits {
                    work,
                    ..Limits::default()
                }
            )
            .is_ok()
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
    assert!(decode(&hex("18 00"), unlimited).is_ok());
    let oversized = vec![0; 16 * 1024 * 1024 + 1];
    error(
        &oversized,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    let mut bytes = hex("18 00");
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
    assert_eq!((out.flags, out.fq_name), (Some(0), 0));
    check_raw(&out, &bytes);
    bytes.extend_from_slice(&[8, 0]);
    error(
        &bytes,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("records")),
        131_070,
    );
}

#[test]
fn supported_unit_composes_under_separate_budgets() {
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
    assert_eq!(
        framed.payload,
        hex(
            "08 c6 02 18 00 32 02 30 01 42 07 08 42 a2 06 02 08 02 4a 0a 10 03 1a 02 30 04 48 96 80 04 a8 06 05"
        )
    );
    let selected = decode(
        framed.payload,
        Limits {
            bytes: 33,
            records: 6,
            work: 45,
        },
    )
    .unwrap();
    assert_eq!((selected.flags, selected.fq_name), (Some(326), 0));
    check_raw(&selected, &bytes[19..52]);
    let mut resolver = Resolver::new(&framed.string_table, &header.d2, Default::default()).unwrap();
    assert_eq!(
        resolver.get_string(selected.fq_name).unwrap(),
        "kotlin/Unit".encode_utf16().collect::<Vec<_>>()
    );
    let old = payload::decode(&[4, 10, 2, 8, 2, 8, 1], Default::default()).unwrap();
    error(old.payload, Limits::default(), ErrorKind::MissingFqName, 2);
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
            check(&[24, 0, a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
