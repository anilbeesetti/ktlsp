use ktlsp_binary::metadata::type_table::{Error, ErrorKind, Limits, TypeTable, decode};
use std::mem::size_of;

const S: usize = size_of::<&[u8]>();

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check(out: &TypeTable<'_>, bytes: &[u8], spans: &[(usize, usize)], value: Option<i32>) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(out.first_nullable, value);
    assert_eq!(out.types.len(), spans.len());
    for (body, &(start, end)) in out.types.iter().zip(spans) {
        assert_eq!(*body, &bytes[start..end]);
        assert_eq!(body.as_ptr(), bytes[start..end].as_ptr());
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn wire_error(text: &str, kind: ErrorKind, offset: usize) {
    error(&hex(text), Limits::default(), kind, offset);
}
fn scalar(text: &str, value: Option<i32>) {
    let bytes = hex(text);
    let out = decode(&bytes, Limits::default()).unwrap();
    check(&out, &bytes, &[], value);
}

#[test]
fn frozen_fixtures_and_exact_limits() {
    for (text, spans, value, records, work, capacity, last, growth) in [
        (
            include_str!("fixtures/metadata_type_table/empty.hex"),
            vec![],
            None,
            0,
            0,
            0,
            0,
            0,
        ),
        (
            include_str!("fixtures/metadata_type_table/ordered.hex"),
            vec![(2, 2), (4, 6), (8, 9)],
            Some(1),
            4,
            19 + 4 * S,
            4 * S,
            9,
            6,
        ),
        (
            include_str!("fixtures/metadata_type_table/duplicates.hex"),
            vec![(15, 15)],
            Some(2),
            5,
            25 + S,
            S,
            13,
            13,
        ),
        (
            include_str!("fixtures/metadata_type_table/opaque.hex"),
            vec![(23, 24)],
            None,
            5,
            34 + S,
            S,
            21,
            21,
        ),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            let out = decode(
                &bytes,
                Limits {
                    bytes: bytes.len() + extra,
                    records: records + extra,
                    work: work + extra,
                    owned_bytes: capacity + extra,
                },
            )
            .unwrap();
            check(&out, &bytes, &spans, value);
        }
        if !bytes.is_empty() {
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
            error(
                &bytes,
                Limits {
                    records: records - 1,
                    ..Limits::default()
                },
                ErrorKind::Limit("records"),
                last,
            );
            error(
                &bytes,
                Limits {
                    work: work - 1,
                    ..Limits::default()
                },
                ErrorKind::Limit("work"),
                last,
            );
            error(
                &bytes,
                Limits {
                    owned_bytes: capacity - 1,
                    ..Limits::default()
                },
                ErrorKind::Limit("owned_bytes"),
                growth,
            );
        }
    }
}

#[test]
fn all_fifty_four_fixture_cuts_have_frozen_expectations() {
    let mut count = 0;
    for (text, boundaries) in [
        (
            include_str!("fixtures/metadata_type_table/empty.hex"),
            vec![(0, 0, None)],
        ),
        (
            include_str!("fixtures/metadata_type_table/ordered.hex"),
            vec![
                (0, 0, None),
                (2, 1, None),
                (6, 2, None),
                (9, 3, None),
                (11, 3, Some(1)),
            ],
        ),
        (
            include_str!("fixtures/metadata_type_table/duplicates.hex"),
            vec![
                (0, 0, None),
                (2, 0, Some(0)),
                (5, 0, Some(0)),
                (11, 0, Some(-1)),
                (13, 0, Some(2)),
                (15, 1, Some(2)),
            ],
        ),
        (
            include_str!("fixtures/metadata_type_table/opaque.hex"),
            vec![
                (0, 0, None),
                (2, 0, None),
                (7, 0, None),
                (16, 0, None),
                (21, 0, None),
                (24, 1, None),
            ],
        ),
    ] {
        let bytes = hex(text);
        for n in 0..=bytes.len() {
            count += 1;
            if let Some(&(_, types, value)) = boundaries.iter().find(|&&(end, _, _)| end == n) {
                let out = decode(&bytes[..n], Limits::default()).unwrap();
                assert_eq!((out.types.len(), out.first_nullable), (types, value));
                assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
                assert_eq!(out.raw, &bytes[..n]);
            } else {
                error(&bytes[..n], Limits::default(), ErrorKind::Truncated, n);
            }
        }
    }
    assert_eq!(count, 54);
}

#[test]
fn scalar_presence_signed_low32_nonminimal_and_duplicate_order() {
    scalar("", None);
    for (text, value) in [
        ("00", 0),
        ("01", 1),
        ("02", 2),
        ("03", 3),
        ("ff ff ff ff 0f", -1),
        ("fe ff ff ff 0f", -2),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("ff ff ff ff ff ff ff ff ff 7f", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 80 80 80 10", 0),
        ("80 00", 0),
        ("80 80 80 80 80 80 80 80 80 02", 0),
    ] {
        scalar(&format!("10 {text}"), Some(value));
        scalar(&format!("10 01 10 {text}"), Some(value));
        let bytes = hex(&format!("0a 00 0a 00 10 {text}"));
        check(
            &decode(&bytes, Limits::default()).unwrap(),
            &bytes,
            &[(2, 2), (4, 4)],
            Some(value),
        );
    }
    scalar("10 01 10 02", Some(2));
    scalar("10 02 10 01", Some(1));
    scalar("90 00 80 00", Some(0));
    scalar("90 80 80 80 10 00", Some(0));
    for text in ["8a 00 80 00", "8a 80 80 80 10 00"] {
        let bytes = hex(text);
        check(
            &decode(&bytes, Limits::default()).unwrap(),
            &bytes,
            &[(bytes.len(), bytes.len())],
            None,
        );
    }
}

#[test]
fn wrong_wire_unknown_and_nested_contents_stay_opaque() {
    for wrong in ["11 00 00 00 00 00 00 00 00", "12 01 80", "15 00 00 00 00"] {
        scalar(wrong, None);
        scalar(&format!("{wrong} 10 01"), Some(1));
        scalar(&format!("10 01 {wrong}"), Some(1));
    }
    for wrong in ["08 01", "09 01 02 03 04 05 06 07 08", "0d 01 02 03 04"] {
        scalar(wrong, None);
        for bytes in [
            hex(&format!("{wrong} 0a 00")),
            hex(&format!("0a 00 {wrong}")),
        ] {
            let start = if bytes[0] == 10 { 2 } else { bytes.len() };
            check(
                &decode(&bytes, Limits::default()).unwrap(),
                &bytes,
                &[(start, start)],
                None,
            );
        }
    }
    for unknown in [
        "18 01",
        "19 01 02 03 04 05 06 07 08",
        "1a 03 ff 00 0b",
        "1d 01 02 03 04",
        "f8 ff ff ff 0f 01",
    ] {
        scalar(unknown, None);
    }
    for (text, spans) in [
        ("0a 00", vec![(2, 2)]),
        ("0a 01 80", vec![(2, 3)]),
        ("0a 03 ff 00 0b", vec![(2, 5)]),
        ("0a 01 80 0a 01 80", vec![(2, 3), (5, 6)]),
    ] {
        let bytes = hex(text);
        check(
            &decode(&bytes, Limits::default()).unwrap(),
            &bytes,
            &spans,
            None,
        );
    }
    let mut bytes = hex("0a 80 08");
    bytes.extend_from_slice(&[0xff; 1024]);
    error(
        &bytes,
        Limits {
            work: 1028 + S,
            ..Limits::default()
        },
        ErrorKind::Limit("work"),
        0,
    );
    check(
        &decode(
            &bytes,
            Limits {
                work: 1029 + S,
                ..Limits::default()
            },
        )
        .unwrap(),
        &bytes,
        &[(3, 1027)],
        None,
    );
}

#[test]
fn exact_structural_offsets_and_late_failures() {
    for (text, kind, offset) in [
        ("80", ErrorKind::Truncated, 1),
        ("0a", ErrorKind::Truncated, 1),
        ("10", ErrorKind::Truncated, 1),
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
        ("0a 02 ff", ErrorKind::Truncated, 3),
        ("0d 01 02 03", ErrorKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", ErrorKind::Truncated, 8),
        ("0a 80 80 80 80 08", ErrorKind::NegativeLength, 1),
        ("0a ff ff ff ff 07", ErrorKind::Truncated, 6),
        (
            "10 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            9,
        ),
        (
            "0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
    ] {
        wire_error(text, kind, offset);
        wire_error(&format!("0a 00 {text}"), kind, offset + 2);
    }
    for tag in ["10", "0a"] {
        for n in 1..=10 {
            wire_error(
                &format!("{tag} {}", "80 ".repeat(n)),
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
fn aggregate_capacity_growth_and_failure_precedence() {
    for (k, capacity, owned_offset, work_offset) in [
        (1, 1, 0, 0),
        (2, 2, 2, 2),
        (3, 4, 4, 4),
        (4, 4, 4, 6),
        (5, 8, 8, 8),
    ] {
        let bytes = [10, 0].repeat(k);
        let limits = Limits {
            bytes: 2 * k,
            records: k,
            work: 4 * k + capacity * S,
            owned_bytes: capacity * S,
        };
        let spans: Vec<_> = (1..=k).map(|n| (2 * n, 2 * n)).collect();
        check(&decode(&bytes, limits).unwrap(), &bytes, &spans, None);
        error(
            &bytes,
            Limits {
                owned_bytes: limits.owned_bytes - 1,
                ..limits
            },
            ErrorKind::Limit("owned_bytes"),
            owned_offset,
        );
        error(
            &bytes,
            Limits {
                work: limits.work - 1,
                ..limits
            },
            ErrorKind::Limit("work"),
            work_offset,
        );
    }
    for (text, records, work, owned_bytes, label, offset) in [
        ("0a 00", 1, 0, S, "work", 0),
        ("0a 00", 1, 1, S, "work", 0),
        ("0a 00", 1, 2, S, "work", 1),
        ("0a 00", 1, 3, S, "work", 0),
        ("0a 00", 1, 4, 0, "owned_bytes", 0),
        ("0a 00", 1, 4, S, "work", 0),
        ("0a 00", 0, 0, 0, "records", 0),
        ("0a 02 ff", 1, 4, S, "work", 2),
        ("10", 1, 2, S, "work", 1),
        ("0a 00 80", 1, 4 + S, S, "records", 2),
        ("0a 00 80", 2, 6 + S, S, "work", 3),
        ("0a 00 0a 00 18 00", 2, 12 + 2 * S, 2 * S, "records", 4),
        ("0a 00 0a 00 18 00", 3, 11 + 2 * S, 2 * S, "work", 4),
    ] {
        error(
            &hex(text),
            Limits {
                bytes: 32,
                records,
                work,
                owned_bytes,
            },
            ErrorKind::Limit(label),
            offset,
        );
    }
    for (text, work, kind, offset) in [
        ("0a 02 ff", 5, ErrorKind::Truncated, 3),
        ("10", 3, ErrorKind::Truncated, 1),
        ("0a 00 80", 7 + S, ErrorKind::Truncated, 3),
        ("0a 00 00", 6 + S, ErrorKind::InvalidFieldNumber, 2),
    ] {
        error(
            &hex(text),
            Limits {
                work,
                ..Limits::default()
            },
            kind,
            offset,
        );
    }
    let bytes = hex("0a 00 0a 00 18 00");
    check(
        &decode(
            &bytes,
            Limits {
                records: 3,
                work: 12 + 2 * S,
                owned_bytes: 2 * S,
                ..Limits::default()
            },
        )
        .unwrap(),
        &bytes,
        &[(2, 2), (4, 4)],
        None,
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
    check(&decode(&[], unlimited).unwrap(), &[], &[], None);
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [10, 0].repeat(65_535);
    let out = decode(
        &bytes,
        Limits {
            bytes: 131_070,
            records: 65_535,
            work: 262_140 + 65_536 * S,
            owned_bytes: 65_536 * S,
        },
    )
    .unwrap();
    assert_eq!((out.types.len(), out.first_nullable), (65_535, None));
    for (n, body) in out.types.iter().enumerate() {
        assert!(body.is_empty());
        assert_eq!(body.as_ptr(), bytes[2 * (n + 1)..].as_ptr());
    }
    bytes.extend_from_slice(&[10, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), 131_070);
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
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
                assert_eq!(out.raw, bytes);
                assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
                assert!(out.types.len() <= limits.records);
                let start = bytes.as_ptr() as usize;
                for body in out.types {
                    let pos = body.as_ptr() as usize - start;
                    assert!(pos <= bytes.len());
                    assert_eq!(body, &bytes[pos..pos + body.len()]);
                }
            }
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            check(&[a, b]);
            check(&[10, 0, a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
