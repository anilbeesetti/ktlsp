#![expect(
    clippy::single_range_in_vec_init,
    reason = "Fixture lists contain byte spans, not collected indexes."
)]

use ktlsp_binary::metadata::type_alias::{Error, ErrorKind, Limits, TypeAlias, decode};
use ktlsp_binary::metadata::wire::ErrorKind as WireKind;
use std::{mem::size_of, ops::Range};

const S: usize = size_of::<&[u8]>();
type Scalars = (Option<i32>, i32, Option<i32>, Option<i32>);
type Cut = (usize, Scalars, Vec<Range<usize>>, Vec<Range<usize>>);
const EMPTY: Scalars = (None, 0, None, None);
const MINIMAL: &str = include_str!("fixtures/metadata_type_alias/minimal.hex");
const EXPLICIT: &str = include_str!("fixtures/metadata_type_alias/explicit.hex");
const DUPLICATES: &str = include_str!("fixtures/metadata_type_alias/duplicates.hex");
const INLINE: &str = include_str!("fixtures/metadata_type_alias/inline.hex");
const OPAQUE: &str = include_str!("fixtures/metadata_type_alias/opaque.hex");

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check_raw(out: &TypeAlias<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
}
fn check_values(out: &TypeAlias<'_>, bytes: &[u8], expected: Scalars) {
    assert_eq!(
        (
            out.flags,
            out.name,
            out.underlying_type_id,
            out.expanded_type_id
        ),
        expected
    );
    check_raw(out, bytes);
}
fn check(bytes: &[u8], expected: Scalars, underlying: &[Range<usize>], expanded: &[Range<usize>]) {
    let out = decode(bytes, Limits::default()).unwrap();
    check_values(&out, bytes, expected);
    for (actual, ranges) in [
        (&out.underlying_types, underlying),
        (&out.expanded_types, expanded),
    ] {
        assert_eq!(actual.len(), ranges.len());
        for (body, span) in actual.iter().zip(ranges) {
            assert_eq!(*body, &bytes[span.clone()]);
            assert_eq!(body.as_ptr(), bytes[span.clone()].as_ptr());
        }
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn wire_error(text: &str, kind: WireKind, offset: usize) {
    error(&hex(text), Limits::default(), ErrorKind::Wire(kind), offset);
}
fn limited(text: &str, records: usize, work: usize, owned: usize, kind: WireKind, offset: usize) {
    error(
        &hex(text),
        Limits {
            records,
            work,
            owned_bytes: owned,
            ..Limits::default()
        },
        ErrorKind::Wire(kind),
        offset,
    );
}

#[test]
fn frozen_fixtures_and_exact_limits() {
    for (text, expected, fields, work, owned, last) in [
        (MINIMAL, EMPTY, 1, 4, 0, 0),
        (EXPLICIT, (Some(6), 0, Some(0), Some(0)), 4, 16, 0, 6),
        (DUPLICATES, (Some(6), 0, Some(0), Some(0)), 8, 32, 0, 14),
        (INLINE, EMPTY, 6, 28 + 6 * S, 6 * S, 14),
        (OPAQUE, EMPTY, 9, 41, 0, 21),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            let out = decode(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: fields + extra,
                    work: work + extra,
                    owned_bytes: owned + extra,
                },
            )
            .unwrap();
            check_values(&out, &bytes, expected);
        }
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            ErrorKind::Wire(WireKind::Limit("bytes")),
            0,
        );
        limited(
            text,
            fields - 1,
            usize::MAX,
            usize::MAX,
            WireKind::Limit("records"),
            last,
        );
        limited(
            text,
            usize::MAX,
            work - 1,
            usize::MAX,
            WireKind::Limit("work"),
            last,
        );
        if owned > 0 {
            limited(
                text,
                usize::MAX,
                usize::MAX,
                owned - 1,
                WireKind::Limit("owned_bytes"),
                last,
            );
        }
    }
}

#[test]
fn retained_inline_occurrences_and_id_coexistence() {
    check(
        &hex(INLINE),
        EMPTY,
        &[4..6, 11..11, 13..14],
        &[8..9, 16..16],
    );
    check(
        &hex("10 00 28 01 22 00 38 02 32 00"),
        (None, 0, Some(1), Some(2)),
        &[6..6],
        &[10..10],
    );
    check(
        &hex("10 00 32 00 38 02 22 00 28 01"),
        (None, 0, Some(1), Some(2)),
        &[8..8],
        &[4..4],
    );
    for text in [
        "10 00 28 01 22 00 38 02 32 00",
        "10 00 32 00 38 02 22 00 28 01",
    ] {
        let bytes = hex(text);
        let out = decode(
            &bytes,
            Limits {
                bytes: 10,
                records: 5,
                work: 20 + 2 * S,
                owned_bytes: 2 * S,
            },
        )
        .unwrap();
        check_values(&out, &bytes, (None, 0, Some(1), Some(2)));
        limited(text, 5, 19 + 2 * S, 2 * S, WireKind::Limit("work"), 8);
    }
    check(
        &hex("10 00 22 01 80 22 01 80 32 00 32 00"),
        EMPTY,
        &[4..5, 7..8],
        &[10..10, 12..12],
    );
}

#[test]
fn presence_signed_scalars_duplicates_and_nonminimal_wire() {
    for (text, expected) in [
        (MINIMAL, EMPTY),
        ("08 06 10 00", (Some(6), 0, None, None)),
        ("08 00 10 00", (Some(0), 0, None, None)),
        ("10 00 28 00", (None, 0, Some(0), None)),
        ("10 00 38 00", (None, 0, None, Some(0))),
        (
            "08 01 10 02 28 03 38 04 08 06 10 00 28 00 38 00",
            (Some(6), 0, Some(0), Some(0)),
        ),
        ("90 00 80 00", EMPTY),
        ("90 80 80 80 10 00", EMPTY),
    ] {
        check(&hex(text), expected, &[], &[]);
    }
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
        for (tag, expected) in [
            ("08", (Some(value), 2, Some(3), Some(4))),
            ("10", (Some(1), value, Some(3), Some(4))),
            ("28", (Some(1), 2, Some(value), Some(4))),
            ("38", (Some(1), 2, Some(3), Some(value))),
        ] {
            check(
                &hex(&format!("08 01 10 02 28 03 38 04 {tag} {body}")),
                expected,
                &[],
                &[],
            );
        }
    }
    for text in ["", "08 06", "28 00", "38 00", "12 00", "22 00 32 00"] {
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
            owned_bytes: 0,
        },
        ErrorKind::MissingName,
        0,
    );
}

#[test]
fn wrong_wire_never_sets_erases_or_retains_selected_values() {
    for number in [1_u8, 2, 5, 7] {
        for (wire, body) in [
            (1, "00 00 00 00 00 00 00 00"),
            (2, "01 80"),
            (5, "00 00 00 00"),
        ] {
            let wrong = format!("{:02x} {body}", (number << 3) | wire);
            if number == 2 {
                let bytes = hex(&wrong);
                error(
                    &bytes,
                    Limits::default(),
                    ErrorKind::MissingName,
                    bytes.len(),
                );
            } else {
                check(&hex(&format!("{wrong} 10 00")), EMPTY, &[], &[]);
            }
            for text in [
                format!("{wrong} 08 01 10 02 28 03 38 04"),
                format!("08 01 10 02 28 03 38 04 {wrong}"),
            ] {
                check(&hex(&text), (Some(1), 2, Some(3), Some(4)), &[], &[]);
            }
        }
    }
    for number in [4_u8, 6] {
        for (wire, body) in [
            (0, "01"),
            (1, "00 00 00 00 00 00 00 00"),
            (5, "00 00 00 00"),
        ] {
            let wrong = format!("{:02x} {body}", (number << 3) | wire);
            check(&hex(&format!("{wrong} 10 00")), EMPTY, &[], &[]);
            let shift = hex(&wrong).len();
            check(
                &hex(&format!("{wrong} 10 00 22 00 32 00")),
                EMPTY,
                &[shift + 4..shift + 4],
                &[shift + 6..shift + 6],
            );
            check(
                &hex(&format!("10 00 22 00 32 00 {wrong}")),
                EMPTY,
                &[4..4],
                &[6..6],
            );
        }
    }
}

#[test]
fn unselected_and_nested_fields_stay_opaque() {
    check(&hex(OPAQUE), EMPTY, &[], &[]);
    for tail in [
        "a0 06 01",
        "a1 06 01 02 03 04 05 06 07 08",
        "a2 06 00",
        "a2 06 03 ff 00 0b",
        "a5 06 01 02 03 04",
        "f8 ff ff ff 0f 01",
    ] {
        check(&hex(&format!("10 00 {tail}")), EMPTY, &[], &[]);
    }
    let mut bytes = hex("10 00 1a 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    let out = decode(
        &bytes,
        Limits {
            records: 2,
            work: 1033,
            owned_bytes: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    check_values(&out, &bytes, EMPTY);
    error(
        &bytes,
        Limits {
            work: 1032,
            ..Limits::default()
        },
        ErrorKind::Wire(WireKind::Limit("work")),
        2,
    );
}

#[test]
fn frozen_structural_errors_precede_missing_or_present_name() {
    for (text, kind, offset) in [
        ("10", WireKind::Truncated, 1),
        ("80", WireKind::Truncated, 1),
        ("00", WireKind::InvalidFieldNumber, 0),
        ("03", WireKind::InvalidFieldNumber, 0),
        ("07", WireKind::InvalidFieldNumber, 0),
        ("0b", WireKind::UnsupportedGroup, 0),
        ("0c", WireKind::UnsupportedGroup, 0),
        ("23", WireKind::UnsupportedGroup, 0),
        ("24", WireKind::UnsupportedGroup, 0),
        ("33", WireKind::UnsupportedGroup, 0),
        ("34", WireKind::UnsupportedGroup, 0),
        ("0e", WireKind::InvalidWireType, 0),
        ("0f", WireKind::InvalidWireType, 0),
        ("26", WireKind::InvalidWireType, 0),
        ("27", WireKind::InvalidWireType, 0),
        ("36", WireKind::InvalidWireType, 0),
        ("37", WireKind::InvalidWireType, 0),
        ("22 02 ff", WireKind::Truncated, 3),
        ("32 02 ff", WireKind::Truncated, 3),
        ("0d 01 02 03", WireKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", WireKind::Truncated, 8),
        ("22 80 80 80 80 08", WireKind::NegativeLength, 1),
        ("32 80 80 80 80 08", WireKind::NegativeLength, 1),
        ("22 ff ff ff ff 07", WireKind::Truncated, 6),
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
            "22 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            10,
        ),
    ] {
        wire_error(text, kind, offset);
        for prefix in ["10 00", "08 06"] {
            wire_error(&format!("{prefix} {text}"), kind, offset + 2);
        }
    }
    for tag in ["08", "10", "28", "38"] {
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
fn all_seventy_fixture_cuts_have_frozen_exact_expectations() {
    let scalar_explicit: Scalars = (Some(6), 0, Some(0), Some(0));
    let rows: [(&str, Vec<usize>, Vec<Cut>); 5] = [
        (MINIMAL, vec![0], vec![(2, EMPTY, vec![], vec![])]),
        (
            EXPLICIT,
            vec![0, 2],
            vec![
                (4, (Some(6), 0, None, None), vec![], vec![]),
                (6, (Some(6), 0, Some(0), None), vec![], vec![]),
                (8, scalar_explicit, vec![], vec![]),
            ],
        ),
        (
            DUPLICATES,
            vec![0, 2],
            vec![
                (4, (Some(1), 1, None, None), vec![], vec![]),
                (6, (Some(1), 1, Some(2), None), vec![], vec![]),
                (8, (Some(1), 1, Some(2), Some(3)), vec![], vec![]),
                (10, (Some(6), 1, Some(2), Some(3)), vec![], vec![]),
                (12, (Some(6), 0, Some(2), Some(3)), vec![], vec![]),
                (14, (Some(6), 0, Some(0), Some(3)), vec![], vec![]),
                (16, scalar_explicit, vec![], vec![]),
            ],
        ),
        (
            INLINE,
            vec![0],
            vec![
                (2, EMPTY, vec![], vec![]),
                (6, EMPTY, vec![4..6], vec![]),
                (9, EMPTY, vec![4..6], vec![8..9]),
                (11, EMPTY, vec![4..6, 11..11], vec![8..9]),
                (14, EMPTY, vec![4..6, 11..11, 13..14], vec![8..9]),
                (16, EMPTY, vec![4..6, 11..11, 13..14], vec![8..9, 16..16]),
            ],
        ),
        (
            OPAQUE,
            vec![0],
            [2, 5, 7, 11, 15, 17, 19, 21, 23]
                .into_iter()
                .map(|end| (end, EMPTY, vec![], vec![]))
                .collect(),
        ),
    ];
    let mut cuts = 0;
    for (text, missing, successes) in rows {
        let bytes = hex(text);
        let mut covered = vec![false; bytes.len() + 1];
        for n in missing {
            assert!(!covered[n]);
            covered[n] = true;
            error(&bytes[..n], Limits::default(), ErrorKind::MissingName, n);
        }
        for (n, expected, underlying, expanded) in successes {
            assert!(!covered[n]);
            covered[n] = true;
            check(&bytes[..n], expected, &underlying, &expanded);
        }
        for (n, seen) in covered.iter_mut().enumerate() {
            if !*seen {
                error(
                    &bytes[..n],
                    Limits::default(),
                    ErrorKind::Wire(WireKind::Truncated),
                    n,
                );
                *seen = true;
            }
        }
        assert!(covered.iter().all(|seen| *seen));
        cuts += covered.len();
    }
    assert_eq!(cuts, 70);
}

#[test]
fn exact_work_and_aggregate_allocation_precedence() {
    for (text, work, kind, offset) in [
        ("10 00", 0, WireKind::Limit("work"), 0),
        ("10 00", 1, WireKind::Limit("work"), 0),
        ("10 00", 2, WireKind::Limit("work"), 1),
        ("10 00", 3, WireKind::Limit("work"), 0),
        ("10", 2, WireKind::Limit("work"), 1),
        ("10", 3, WireKind::Truncated, 1),
        ("10 00 22 00", 7, WireKind::Limit("work"), 2),
        ("10 00 22 00", 8, WireKind::Limit("work"), 2),
        ("10 00 22 02 ff", 8, WireKind::Limit("work"), 4),
        ("10 00 22 02 ff", 9, WireKind::Truncated, 5),
    ] {
        limited(text, usize::MAX, work, usize::MAX, kind, offset);
    }
    limited("10 00", 0, 0, 0, WireKind::Limit("records"), 0);
    limited(
        "10 00 22 00",
        2,
        8,
        S - 1,
        WireKind::Limit("owned_bytes"),
        2,
    );
    limited(
        "10 00 22 00 32 00",
        3,
        usize::MAX,
        S,
        WireKind::Limit("owned_bytes"),
        4,
    );
    limited("22 00", 1, usize::MAX, 0, WireKind::Limit("owned_bytes"), 0);
    let bytes = hex("10 00 22 00");
    let out = decode(
        &bytes,
        Limits {
            bytes: 4,
            records: 2,
            work: 8 + S,
            owned_bytes: S,
        },
    )
    .unwrap();
    check_values(&out, &bytes, EMPTY);
    for (n, slots, growth) in [
        (1, 1, 0),
        (2, 2, 1),
        (3, 4, 2),
        (4, 4, 2),
        (5, 8, 4),
        (8, 8, 4),
        (9, 16, 8),
    ] {
        for (tag, expanded) in [(0x22, false), (0x32, true)] {
            let mut bytes = hex(MINIMAL);
            for _ in 0..n {
                bytes.extend_from_slice(&[tag, 0]);
            }
            let owned = slots * S;
            let work = bytes.len() + 2 * (n + 1) + owned;
            let out = decode(
                &bytes,
                Limits {
                    bytes: bytes.len(),
                    records: n + 1,
                    work,
                    owned_bytes: owned,
                },
            )
            .unwrap();
            check_values(&out, &bytes, EMPTY);
            assert_eq!(
                (out.underlying_types.len(), out.expanded_types.len()),
                if expanded { (0, n) } else { (n, 0) }
            );
            error(
                &bytes,
                Limits {
                    owned_bytes: owned - 1,
                    ..Limits::default()
                },
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                2 + 2 * growth,
            );
            error(
                &bytes,
                Limits {
                    work: work - 1,
                    ..Limits::default()
                },
                ErrorKind::Wire(WireKind::Limit("work")),
                2 * n,
            );
        }
    }
}

#[test]
fn callers_cannot_raise_hard_bounds() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    let bytes = hex(INLINE);
    check_raw(&decode(&bytes, unlimited).unwrap(), &bytes);
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    let mut bytes = hex(MINIMAL);
    for _ in 0..65_534 {
        bytes.extend_from_slice(&[8, 0]);
    }
    let out = decode(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140,
            owned_bytes: 0,
        },
    )
    .unwrap();
    check_values(&out, &bytes, (Some(0), 0, None, None));
    bytes.extend_from_slice(&[8, 0]);
    error(
        &bytes,
        unlimited,
        ErrorKind::Wire(WireKind::Limit("records")),
        131_070,
    );
}

#[test]
fn deterministic_finite_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
        owned_bytes: 128,
    };
    let check = |bytes: &[u8]| {
        let out = decode(bytes, limits);
        assert_eq!(out, decode(bytes, limits));
        match out {
            Ok(out) => {
                check_raw(&out, bytes);
                for body in out.underlying_types.iter().chain(&out.expanded_types) {
                    let start = body.as_ptr() as usize;
                    let base = bytes.as_ptr() as usize;
                    assert!(start >= base && start - base <= bytes.len());
                    assert!(body.len() <= bytes.len() - (start - base));
                    assert_eq!(*body, &bytes[start - base..start - base + body.len()]);
                }
            }
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
