use ktlsp_binary::metadata::{
    property_header::{Error, ErrorKind, Limits, PropertyHeader, decode},
    wire::ErrorKind as WireKind,
};

type Values = (Option<i32>, Option<i32>, Option<i32>, Option<i32>, i32);

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check(out: &PropertyHeader<'_>, bytes: &[u8], expected: Values) {
    assert_eq!(
        (
            out.flags,
            out.old_flags,
            out.getter_flags,
            out.setter_flags,
            out.name
        ),
        expected,
        "{bytes:02x?}"
    );
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn values(text: &str, expected: Values) {
    let bytes = hex(text);
    check(
        &decode(&bytes, Limits::default()).unwrap(),
        &bytes,
        expected,
    );
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { kind, offset }));
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
    for (text, expected, fields, work, last) in [
        (
            include_str!("fixtures/metadata_property_header/minimal.hex"),
            (None, None, None, None, 0),
            1,
            4,
            0,
        ),
        (
            include_str!("fixtures/metadata_property_header/explicit.hex"),
            (Some(518), Some(2054), Some(0), Some(0), 0),
            5,
            22,
            9,
        ),
        (
            include_str!("fixtures/metadata_property_header/duplicates.hex"),
            (Some(0), Some(6), Some(0), Some(6), 0),
            15,
            63,
            31,
        ),
        (
            include_str!("fixtures/metadata_property_header/opaque.hex"),
            (None, None, None, None, 0),
            8,
            37,
            17,
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
            check(&out, &bytes, expected);
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
fn flags_preserve_independent_presence_defaults_and_order() {
    values("10 00", (None, None, None, None, 0));
    for (tag, default, expected_index) in [
        ("08", "86 10", 1),
        ("38", "00", 2),
        ("40", "00", 3),
        ("58", "86 04", 0),
    ] {
        let default_value = match tag {
            "08" => 2054,
            "58" => 518,
            _ => 0,
        };
        for (body, value) in [
            ("00", 0),
            (default, default_value),
            ("01", 1),
            ("01 TAG 02", 2),
            ("02 TAG 01", 1),
        ] {
            let body = body.replace("TAG", tag);
            let mut flags = [None; 4];
            flags[expected_index] = Some(value);
            for text in [format!("{tag} {body} 10 00"), format!("10 00 {tag} {body}")] {
                values(&text, (flags[0], flags[1], flags[2], flags[3], 0));
            }
        }
        let bytes = hex(&format!("{tag} {default}"));
        error(
            &bytes,
            Limits::default(),
            ErrorKind::MissingName,
            bytes.len(),
        );
    }
    values("10 01 10 02", (None, None, None, None, 2));
    values("10 02 10 01", (None, None, None, None, 1));
    values("48 01 50 02 10 00", (None, None, None, None, 0));
    values("90 00 80 00", (None, None, None, None, 0));
    values("90 80 80 80 10 00", (None, None, None, None, 0));
    for text in ["", "12 00"] {
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
    for tag in [8_u8, 16, 56, 64, 88] {
        for (wire, body) in [
            (1, "00 00 00 00 00 00 00 00"),
            (2, "00"),
            (2, "04 80 ff 00 0b"),
            (5, "00 00 00 00"),
        ] {
            let wrong = format!("{:02x} {body}", tag | wire);
            let bytes = hex(&wrong);
            error(
                &bytes,
                Limits::default(),
                ErrorKind::MissingName,
                bytes.len(),
            );
            if tag != 16 {
                values(&format!("{wrong} 10 00"), (None, None, None, None, 0));
                values(&format!("10 00 {wrong}"), (None, None, None, None, 0));
            }
            for text in [
                format!("{wrong} 08 01 10 01 38 01 40 01 58 01"),
                format!("08 01 10 01 38 01 40 01 58 01 {wrong}"),
            ] {
                values(&text, (Some(1), Some(1), Some(1), Some(1), 1));
            }
        }
    }
}

#[test]
fn signed_low32_and_nonminimal_values_for_all_five_fields() {
    for (body, value) in [
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("ff ff ff ff ff ff ff ff ff 7f", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 80 80 80 10", 0),
        ("80 80 80 80 80 80 80 80 80 02", 0),
        ("80 00", 0),
    ] {
        for (tag, index) in [("58", 0), ("08", 1), ("38", 2), ("40", 3), ("10", 4)] {
            let mut flags = [None; 4];
            let name = if index == 4 {
                value
            } else {
                flags[index] = Some(value);
                0
            };
            let expected = (flags[0], flags[1], flags[2], flags[3], name);
            let prefix = if index == 4 { "" } else { "10 00" };
            values(&format!("{prefix} {tag} {body}"), expected);
            values(&format!("{prefix} {tag} 01 {tag} {body}"), expected);
            let name = if index == 4 {
                1
            } else {
                flags[index] = Some(1);
                0
            };
            values(
                &format!("{prefix} {tag} {body} {tag} 01"),
                (flags[0], flags[1], flags[2], flags[3], name),
            );
        }
    }
}

#[test]
fn unknown_wires_and_hostile_nested_contents_stay_opaque() {
    for tail in [
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 00",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
    ] {
        values(&format!("10 00 {tail}"), (None, None, None, None, 0));
    }
    let mut bytes = hex("10 00 32 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    limited(&bytes, 2, 1032, ErrorKind::Wire(WireKind::Limit("work")), 2);
    check(
        &decode(
            &bytes,
            Limits {
                bytes: 1029,
                records: 2,
                work: 1033,
            },
        )
        .unwrap(),
        &bytes,
        (None, None, None, None, 0),
    );
}

#[test]
fn structural_errors_precede_absent_or_complete_selected_header() {
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
        ("3b", WireKind::UnsupportedGroup, 0),
        ("3c", WireKind::UnsupportedGroup, 0),
        ("43", WireKind::UnsupportedGroup, 0),
        ("44", WireKind::UnsupportedGroup, 0),
        ("5b", WireKind::UnsupportedGroup, 0),
        ("5c", WireKind::UnsupportedGroup, 0),
        ("0e", WireKind::InvalidWireType, 0),
        ("0f", WireKind::InvalidWireType, 0),
        ("16", WireKind::InvalidWireType, 0),
        ("17", WireKind::InvalidWireType, 0),
        ("3e", WireKind::InvalidWireType, 0),
        ("3f", WireKind::InvalidWireType, 0),
        ("46", WireKind::InvalidWireType, 0),
        ("47", WireKind::InvalidWireType, 0),
        ("5e", WireKind::InvalidWireType, 0),
        ("5f", WireKind::InvalidWireType, 0),
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
            "12 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            9,
        ),
    ] {
        wire_error(text, kind, offset);
        for prefix in [
            "10 00",
            "08 86 10",
            "38 00",
            "40 00",
            "58 86 04",
            include_str!("fixtures/metadata_property_header/explicit.hex"),
        ] {
            wire_error(
                &format!("{prefix} {text}"),
                kind,
                offset + hex(prefix).len(),
            );
        }
    }
    for tag in ["08", "10", "38", "40", "58"] {
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
fn all_seventy_two_fixture_cuts_have_frozen_exact_expectations() {
    let mut cuts = 0;
    for (text, truncated, missing, successes) in [
        (
            include_str!("fixtures/metadata_property_header/minimal.hex"),
            vec![1],
            vec![0],
            vec![(2, (None, None, None, None, 0))],
        ),
        (
            include_str!("fixtures/metadata_property_header/explicit.hex"),
            vec![1, 2, 4, 6, 8, 10, 11],
            vec![0, 3],
            vec![
                (5, (None, Some(2054), None, None, 0)),
                (7, (None, Some(2054), Some(0), None, 0)),
                (9, (None, Some(2054), Some(0), Some(0), 0)),
                (12, (Some(518), Some(2054), Some(0), Some(0), 0)),
            ],
        ),
        (
            include_str!("fixtures/metadata_property_header/duplicates.hex"),
            vec![
                1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 22, 24, 25, 27, 28, 30, 32,
            ],
            vec![0, 2, 4, 6, 8],
            vec![
                (10, (Some(4), Some(1), Some(2), Some(3), 1)),
                (12, (Some(4), Some(6), Some(2), Some(3), 1)),
                (14, (Some(4), Some(6), Some(0), Some(3), 1)),
                (16, (Some(4), Some(6), Some(0), Some(6), 1)),
                (18, (Some(0), Some(6), Some(0), Some(6), 1)),
                (20, (Some(0), Some(6), Some(0), Some(6), 0)),
                (23, (Some(0), Some(6), Some(0), Some(6), 0)),
                (26, (Some(0), Some(6), Some(0), Some(6), 0)),
                (29, (Some(0), Some(6), Some(0), Some(6), 0)),
                (31, (Some(0), Some(6), Some(0), Some(6), 0)),
                (33, (Some(0), Some(6), Some(0), Some(6), 0)),
            ],
        ),
        (
            include_str!("fixtures/metadata_property_header/opaque.hex"),
            vec![1, 3, 4, 6, 8, 10, 12, 13, 15, 16, 18, 19, 20],
            vec![0],
            vec![
                (2, (None, None, None, None, 0)),
                (5, (None, None, None, None, 0)),
                (7, (None, None, None, None, 0)),
                (9, (None, None, None, None, 0)),
                (11, (None, None, None, None, 0)),
                (14, (None, None, None, None, 0)),
                (17, (None, None, None, None, 0)),
                (21, (None, None, None, None, 0)),
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
        for (n, expected) in successes {
            assert!(!covered[n]);
            covered[n] = true;
            check(
                &decode(&bytes[..n], Limits::default()).unwrap(),
                &bytes[..n],
                expected,
            );
        }
        assert!(covered.iter().all(|&v| v));
        cuts += covered.len();
    }
    assert_eq!(cuts, 72);
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
        check(
            &decode(
                &bytes,
                Limits {
                    work,
                    ..Limits::default()
                },
            )
            .unwrap(),
            &bytes,
            (None, None, None, None, 0),
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
    let minimal = [16, 0];
    check(
        &decode(&minimal, unlimited).unwrap(),
        &minimal,
        (None, None, None, None, 0),
    );
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    let mut bytes = hex("10 00");
    for _ in 0..65_534 {
        bytes.extend_from_slice(&[56, 0]);
    }
    check(
        &decode(
            &bytes,
            Limits {
                bytes: 131_070,
                records: 65_535,
                work: 262_140,
            },
        )
        .unwrap(),
        &bytes,
        (None, None, Some(0), None, 0),
    );
    bytes.extend_from_slice(&[64, 0]);
    error(
        &bytes,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("records")),
        131_070,
    );
}

#[test]
fn finite_hostile_inputs_are_deterministic_and_keep_relative_offsets() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
    };
    let probe = |bytes: &[u8]| {
        let first = decode(bytes, limits);
        assert_eq!(first, decode(bytes, limits));
        match first {
            Ok(out) => {
                assert_eq!(out.raw, bytes);
                assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
            }
            Err(err) => assert!(err.offset <= bytes.len()),
        }
    };
    let mut inputs = 0;
    for a in 0..=255_u8 {
        for b in 0..=255_u8 {
            probe(&[a, b]);
            probe(&[16, 0, a, b]);
            inputs += 2;
        }
        for n in 0..32 {
            probe(&vec![a; n]);
            inputs += 1;
        }
    }
    assert_eq!(inputs, 139_264);
}
