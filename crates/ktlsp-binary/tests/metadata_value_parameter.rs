use ktlsp_binary::metadata::{
    value_parameter::{Error, ErrorKind, Limits, ValueParameter, decode},
    wire::ErrorKind as WireKind,
};

type Selected = (Option<i32>, i32, Option<i32>, Option<i32>);
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn selected(out: &ValueParameter<'_>) -> Selected {
    (out.flags, out.name, out.type_id, out.vararg_element_type_id)
}
fn check(bytes: &[u8], limits: Limits, expected: Selected) {
    let out = decode(bytes, limits).unwrap();
    assert_eq!(selected(&out), expected, "{bytes:02x?}");
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn values(text: &str, expected: Selected) {
    check(&hex(text), Limits::default(), expected);
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn limited(text: &str, records: usize, work: usize, kind: WireKind, offset: usize) {
    error(
        &hex(text),
        Limits {
            records,
            work,
            ..Limits::default()
        },
        ErrorKind::Wire(kind),
        offset,
    );
}

#[test]
fn frozen_fixtures_and_exact_byte_record_work_limits() {
    for (text, expected, fields, work, last) in [
        (
            include_str!("fixtures/metadata_value_parameter/minimal.hex"),
            (None, 0, None, None),
            1,
            4,
            0,
        ),
        (
            include_str!("fixtures/metadata_value_parameter/explicit.hex"),
            (Some(0), 0, Some(0), Some(0)),
            4,
            16,
            6,
        ),
        (
            include_str!("fixtures/metadata_value_parameter/duplicates.hex"),
            (Some(0), 5, Some(6), Some(7)),
            12,
            52,
            25,
        ),
        (
            include_str!("fixtures/metadata_value_parameter/opaque.hex"),
            (None, 0, None, None),
            8,
            35,
            16,
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
            ErrorKind::Wire(WireKind::Limit("bytes")),
            0,
        );
        limited(
            text,
            fields - 1,
            usize::MAX,
            WireKind::Limit("records"),
            last,
        );
        limited(text, usize::MAX, work - 1, WireKind::Limit("work"), last);
    }
}

#[test]
fn independent_presence_signed_values_duplicates_and_wrong_wires() {
    values("10 00", (None, 0, None, None));
    values("08 00 10 00", (Some(0), 0, None, None));
    values("10 00 28 00", (None, 0, Some(0), None));
    values("10 00 30 00", (None, 0, None, Some(0)));
    values("30 04 28 03 10 02 08 01", (Some(1), 2, Some(3), Some(4)));
    for text in ["", "08 00 28 00 30 00", "12 00"] {
        let bytes = hex(text);
        error(
            &bytes,
            Limits::default(),
            ErrorKind::MissingName,
            bytes.len(),
        );
    }
    for number in [1_u8, 2, 5, 6] {
        let tag = number << 3;
        for (text, value) in [
            ("ac 02", 300),
            ("ff ff ff ff 0f", -1),
            ("ff ff ff ff ff ff ff ff ff 01", -1),
            ("80 80 80 80 08", i32::MIN),
            ("ff ff ff ff 07", i32::MAX),
            ("80 00", 0),
            ("80 80 80 80 10", 0),
        ] {
            let expected = match number {
                1 => (Some(value), 2, Some(3), Some(4)),
                2 => (Some(1), value, Some(3), Some(4)),
                5 => (Some(1), 2, Some(value), Some(4)),
                6 => (Some(1), 2, Some(3), Some(value)),
                _ => unreachable!(),
            };
            values(
                &format!("08 01 10 02 28 03 30 04 {tag:02x} {text}"),
                expected,
            );
            let isolated = match number {
                1 => (Some(value), 0, None, None),
                2 => (None, value, None, None),
                5 => (None, 0, Some(value), None),
                6 => (None, 0, None, Some(value)),
                _ => unreachable!(),
            };
            values(&format!("10 00 {tag:02x} {text}"), isolated);
        }
        for (wire, body) in [
            (1, "00 00 00 00 00 00 00 00"),
            (2, "01 80"),
            (5, "00 00 00 00"),
        ] {
            let wrong = format!("{:02x} {body}", tag | wire);
            if number == 2 {
                let bytes = hex(&wrong);
                error(
                    &bytes,
                    Limits::default(),
                    ErrorKind::MissingName,
                    bytes.len(),
                );
            } else {
                values(&format!("{wrong} 10 00"), (None, 0, None, None));
            }
            for text in [
                format!("{wrong} 08 01 10 02 28 03 30 04"),
                format!("08 01 10 02 28 03 30 04 {wrong}"),
            ] {
                values(&text, (Some(1), 2, Some(3), Some(4)));
            }
        }
    }
    values("90 00 80 00", (None, 0, None, None));
    values("90 80 80 80 10 00", (None, 0, None, None));
}

#[test]
fn nested_occurrences_unknown_fields_and_inline_ids_remain_raw() {
    for text in [
        "10 00 1a 00 28 03 22 00 30 04",
        "10 00 28 03 1a 00 30 04 22 00",
    ] {
        values(text, (None, 0, Some(3), Some(4)));
    }
    let repeated = hex("10 00 3a 00 3a 01 ff a2 06 01 00");
    check(
        &repeated,
        Limits {
            bytes: 11,
            records: 4,
            work: 19,
        },
        (None, 0, None, None),
    );
    for tail in [
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 00",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
    ] {
        values(&format!("10 00 {tail}"), (None, 0, None, None));
    }
    let mut bytes = hex("10 00 1a 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    error(
        &bytes,
        Limits {
            work: 1032,
            ..Limits::default()
        },
        ErrorKind::Wire(WireKind::Limit("work")),
        2,
    );
    check(
        &bytes,
        Limits {
            bytes: 1029,
            records: 2,
            work: 1033,
        },
        (None, 0, None, None),
    );
}

#[test]
fn structural_errors_precede_missing_or_present_name() {
    for (text, kind, offset) in [
        ("10", WireKind::Truncated, 1),
        ("80", WireKind::Truncated, 1),
        ("00", WireKind::InvalidFieldNumber, 0),
        ("03", WireKind::InvalidFieldNumber, 0),
        ("07", WireKind::InvalidFieldNumber, 0),
        ("12 02 ff", WireKind::Truncated, 3),
        ("0d 01 02 03", WireKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", WireKind::Truncated, 8),
        ("1a 80 80 80 80 08", WireKind::NegativeLength, 1),
        ("1a ff ff ff ff 07", WireKind::Truncated, 6),
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
        for prefix in ["", "10 00", "08 00", "08 01 10 02 28 03 30 04"] {
            error(
                &hex(&format!("{prefix} {text}")),
                Limits::default(),
                ErrorKind::Wire(kind),
                offset + hex(prefix).len(),
            );
        }
    }
    for number in [1_u8, 2, 5, 6] {
        for wire in [3, 4, 6, 7] {
            let kind = if wire < 6 {
                WireKind::UnsupportedGroup
            } else {
                WireKind::InvalidWireType
            };
            for prefix in ["", "10 00", "08 00", "08 01 10 02 28 03 30 04"] {
                error(
                    &hex(&format!("{prefix} {:02x}", (number << 3) | wire)),
                    Limits::default(),
                    ErrorKind::Wire(kind),
                    hex(prefix).len(),
                );
            }
        }
        for n in 1..=10 {
            let text = format!("{:02x} {}", number << 3, "80 ".repeat(n));
            let kind = if n == 10 {
                WireKind::MalformedVarint
            } else {
                WireKind::Truncated
            };
            for prefix in ["", "10 00", "08 00", "08 01 10 02 28 03 30 04"] {
                error(
                    &hex(&format!("{prefix} {text}")),
                    Limits::default(),
                    ErrorKind::Wire(kind),
                    if n == 10 { 10 } else { n + 1 } + hex(prefix).len(),
                );
            }
        }
    }
}

#[test]
fn all_sixty_one_frozen_fixture_cuts() {
    let mut cuts = 0;
    for (text, truncated, missing, successes) in [
        (
            include_str!("fixtures/metadata_value_parameter/minimal.hex"),
            vec![1],
            vec![0],
            vec![(2, (None, 0, None, None))],
        ),
        (
            include_str!("fixtures/metadata_value_parameter/explicit.hex"),
            vec![1, 3, 5, 7],
            vec![0, 2],
            vec![
                (4, (Some(0), 0, None, None)),
                (6, (Some(0), 0, Some(0), None)),
                (8, (Some(0), 0, Some(0), Some(0))),
            ],
        ),
        (
            include_str!("fixtures/metadata_value_parameter/duplicates.hex"),
            vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 18, 20, 21, 23, 24, 26, 27],
            vec![0, 2],
            vec![
                (4, (Some(1), 2, None, None)),
                (6, (Some(1), 2, Some(3), None)),
                (8, (Some(1), 2, Some(3), Some(4))),
                (10, (Some(0), 2, Some(3), Some(4))),
                (12, (Some(0), 5, Some(3), Some(4))),
                (14, (Some(0), 5, Some(6), Some(4))),
                (16, (Some(0), 5, Some(6), Some(7))),
                (19, (Some(0), 5, Some(6), Some(7))),
                (22, (Some(0), 5, Some(6), Some(7))),
                (25, (Some(0), 5, Some(6), Some(7))),
                (28, (Some(0), 5, Some(6), Some(7))),
            ],
        ),
        (
            include_str!("fixtures/metadata_value_parameter/opaque.hex"),
            vec![1, 3, 5, 6, 8, 10, 11, 13, 15, 17, 18],
            vec![0],
            vec![
                (2, (None, 0, None, None)),
                (4, (None, 0, None, None)),
                (7, (None, 0, None, None)),
                (9, (None, 0, None, None)),
                (12, (None, 0, None, None)),
                (14, (None, 0, None, None)),
                (16, (None, 0, None, None)),
                (19, (None, 0, None, None)),
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
            check(&bytes[..n], Limits::default(), expected);
        }
        assert!(covered.iter().all(|&value| value));
        cuts += covered.len();
    }
    assert_eq!(cuts, 61);
}

#[test]
fn exact_small_budget_offsets_precedence_and_hard_clamps() {
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
        ("10 00 1a 01 80", 8, WireKind::Limit("work"), 2),
        ("10 00 1a 03 ff ff ff", 9, WireKind::Limit("work"), 4),
        ("10 00 1a 03 ff ff ff", 10, WireKind::Limit("work"), 2),
    ] {
        limited(text, usize::MAX, work, kind, offset);
    }
    limited("10 00", 0, 0, WireKind::Limit("records"), 0);
    limited("10 00 80", 1, 4, WireKind::Limit("records"), 2);
    limited("10 00 80", 2, 6, WireKind::Limit("work"), 3);
    limited("10 00 80", 2, 7, WireKind::Truncated, 3);
    error(
        &hex("12 00"),
        Limits {
            records: 1,
            work: 4,
            ..Limits::default()
        },
        ErrorKind::MissingName,
        2,
    );
    for (text, work) in [
        ("10 00", 4),
        ("10 00 1a 01 80", 9),
        ("10 00 1a 03 ff ff ff", 11),
    ] {
        check(
            &hex(text),
            Limits {
                work,
                ..Limits::default()
            },
            (None, 0, None, None),
        );
    }
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
    };
    check(&[16, 0], unlimited, (None, 0, None, None));
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    let mut bytes = vec![16, 0];
    for _ in 0..65_534 {
        bytes.extend_from_slice(&[8, 0]);
    }
    check(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140,
        },
        (Some(0), 0, None, None),
    );
    bytes.extend_from_slice(&[40, 0]);
    error(
        &bytes,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("records")),
        131_070,
    );
}

#[test]
fn finite_hostile_inputs_are_deterministic_and_bounded() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
    };
    for a in 0..=255 {
        for b in 0..=255 {
            for bytes in [&[a, b][..], &[16, 0, a, b][..]] {
                let out = decode(bytes, limits);
                assert_eq!(out, decode(bytes, limits));
                match out {
                    Ok(out) => {
                        assert_eq!(out.raw, bytes);
                        assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
                    }
                    Err(error) => assert!(error.offset <= bytes.len()),
                }
            }
        }
    }
}
