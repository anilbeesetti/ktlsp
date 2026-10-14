use ktlsp_binary::{
    MAX_BYTES, MAX_RECORDS,
    metadata::{
        version_requirement::{
            Error, ErrorKind, Level, Limits, VersionKind, VersionRequirement, decode, decode_table,
        },
        wire,
    },
};
use std::mem::size_of;

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
macro_rules! fixture {
    ($name:literal) => {
        hex(include_str!(concat!(
            "fixtures/metadata_version_requirement/",
            $name,
            ".hex"
        )))
    };
}
fn borrowed(out: &VersionRequirement<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, &bytes[out.span.clone()]);
    assert_eq!(out.raw.as_ptr(), bytes[out.span.clone()].as_ptr());
}
fn scalars(out: &VersionRequirement<'_>) -> [Option<i32>; 4] {
    [out.version, out.version_full, out.error_code, out.message]
}
fn error(bytes: &[u8], limits: wire::Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { kind, offset }));
}
fn table_error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode_table(bytes, limits), Err(Error { kind, offset }));
}

#[test]
fn frozen_requirement_projections_and_exact_budgets() {
    for (bytes, ints, level, kind, records, work) in [
        (fixture!("empty"), [None; 4], None, None, 0, 0),
        (
            fixture!("explicit"),
            [Some(127), Some(65536), Some(9), Some(7)],
            Some(Level::Hidden),
            Some(VersionKind::ApiVersion),
            6,
            26,
        ),
        (
            fixture!("duplicates"),
            [Some(2), Some(4), Some(6), Some(8)],
            Some(Level::Hidden),
            Some(VersionKind::ApiVersion),
            14,
            56,
        ),
        (fixture!("wrong_wire"), [None; 4], None, None, 7, 47),
        (
            fixture!("signed"),
            [Some(-1), Some(-1), Some(i32::MIN), Some(i32::MAX)],
            Some(Level::Warning),
            Some(VersionKind::ApiVersion),
            6,
            53,
        ),
        (
            fixture!("nonminimal"),
            [Some(1), None, None, None],
            Some(Level::Error),
            Some(VersionKind::LanguageVersion),
            4,
            30,
        ),
        (fixture!("unknown_enums"), [None; 4], None, None, 4, 29),
    ] {
        let limits = wire::Limits {
            bytes: bytes.len(),
            records,
            work,
        };
        let out = decode(&bytes, limits).unwrap();
        borrowed(&out, &bytes);
        assert_eq!(out.span, 0..bytes.len());
        assert_eq!(scalars(&out), ints);
        assert_eq!((out.level, out.version_kind), (level, kind));
        if work > 0 {
            assert!(matches!(
                decode(
                    &bytes,
                    wire::Limits {
                        work: work - 1,
                        ..limits
                    }
                ),
                Err(Error {
                    kind: ErrorKind::Limit("work"),
                    ..
                })
            ));
        }
    }
}

#[test]
fn explicit_defaults_and_independent_versions_keep_presence() {
    let bytes = hex("08 00 10 00 18 01 20 00 28 00 30 00");
    let out = decode(
        &bytes,
        wire::Limits {
            bytes: 12,
            records: 6,
            work: 24,
        },
    )
    .unwrap();
    assert_eq!(scalars(&out), [Some(0); 4]);
    assert_eq!(
        (out.level, out.version_kind),
        (Some(Level::Error), Some(VersionKind::LanguageVersion))
    );
    for (literal, version, full) in [
        ("08 01", Some(1), None),
        ("10 02", None, Some(2)),
        ("08 01 10 02", Some(1), Some(2)),
        ("08 80 80 80 80 10", Some(0), None),
        ("08 ff ff ff ff ff ff ff ff ff 7f", Some(-1), None),
        ("88 80 80 80 10 01", Some(1), None),
    ] {
        let bytes = hex(literal);
        let out = decode(&bytes, wire::Limits::default()).unwrap();
        assert_eq!(scalars(&out), [version, full, None, None]);
        borrowed(&out, &bytes);
    }
}

#[test]
fn enums_last_known_win_and_wrong_wire_stays_opaque() {
    for (literal, level, kind) in [
        (
            "18 00 30 01",
            Some(Level::Warning),
            Some(VersionKind::CompilerVersion),
        ),
        (
            "18 02 30 02",
            Some(Level::Hidden),
            Some(VersionKind::ApiVersion),
        ),
        (
            "18 01 18 03 30 00 30 03",
            Some(Level::Error),
            Some(VersionKind::LanguageVersion),
        ),
        (
            "18 03 18 00 30 03 30 01",
            Some(Level::Warning),
            Some(VersionKind::CompilerVersion),
        ),
        ("18 ff ff ff ff 07 30 80 80 80 80 08", None, None),
        (
            "18 80 80 80 80 10 30 81 80 80 80 10",
            Some(Level::Warning),
            Some(VersionKind::CompilerVersion),
        ),
        ("0a 01 80", None, None),
        ("1a 01 80 30 01", None, Some(VersionKind::CompilerVersion)),
        ("38 01 3d 78 56 34 12 3a 01 00", None, None),
        ("0a 81 80 80 80 10 80", None, None),
    ] {
        let bytes = hex(literal);
        let out = decode(&bytes, wire::Limits::default()).unwrap();
        assert_eq!(scalars(&out), [None; 4]);
        assert_eq!((out.level, out.version_kind), (level, kind));
        borrowed(&out, &bytes);
    }
}

#[test]
fn malformed_requirement_errors_keep_exact_offsets() {
    for (literal, kind, offset) in [
        (
            "08 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        ("08 80", ErrorKind::Truncated, 2),
        ("08", ErrorKind::Truncated, 1),
        ("80", ErrorKind::Truncated, 1),
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0c", ErrorKind::UnsupportedGroup, 0),
        ("09 00", ErrorKind::Truncated, 2),
        ("0d 00 00 00", ErrorKind::Truncated, 4),
        ("0a 02 00", ErrorKind::Truncated, 3),
        ("0a ff ff ff ff 0f", ErrorKind::NegativeLength, 1),
        ("08 01 00", ErrorKind::InvalidFieldNumber, 2),
    ] {
        let bytes = hex(literal);
        error(&bytes, wire::Limits::default(), kind, offset);
        let mut table = vec![10, bytes.len() as u8];
        table.extend(&bytes);
        table_error(&table, Limits::default(), kind, offset + 2);
    }
}

#[test]
fn ordered_empty_duplicate_table_entries_borrow_original_spans() {
    let bytes = fixture!("table_ordered");
    let s = size_of::<VersionRequirement<'_>>();
    let limits = Limits {
        bytes: 18,
        records: 9,
        work: 46 + 4 * s,
        owned_bytes: 4 * s,
    };
    let out = decode_table(&bytes, limits).unwrap();
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(out.raw, bytes);
    assert_eq!(out.requirements.len(), 4);
    for (entry, span) in out.requirements.iter().zip([2..2, 4..8, 10..14, 16..18]) {
        assert_eq!(entry.span, span);
        borrowed(entry, &bytes);
    }
    assert_eq!(scalars(&out.requirements[0]), [None; 4]);
    assert_eq!(
        (out.requirements[0].level, out.requirements[0].version_kind),
        (None, None)
    );
    for entry in &out.requirements[1..3] {
        assert_eq!(scalars(entry), [Some(1), None, None, None]);
        assert_eq!(
            (entry.level, entry.version_kind),
            (Some(Level::Warning), None)
        );
    }
    assert_eq!(scalars(&out.requirements[3]), [None; 4]);
    assert_eq!(
        (out.requirements[3].level, out.requirements[3].version_kind),
        (None, Some(VersionKind::ApiVersion))
    );
    table_error(
        &bytes,
        Limits {
            owned_bytes: 4 * s - 1,
            ..limits
        },
        ErrorKind::Limit("owned_bytes"),
        8,
    );
    table_error(
        &bytes,
        Limits {
            records: 8,
            ..limits
        },
        ErrorKind::Limit("records"),
        16,
    );
    let mixed = fixture!("table_mixed");
    let out = decode_table(
        &mixed,
        Limits {
            bytes: 17,
            records: 6,
            work: 31 + 2 * s,
            owned_bytes: 2 * s,
        },
    )
    .unwrap();
    assert_eq!(out.requirements.len(), 2);
    for (entry, span) in out.requirements.iter().zip([8..10, 17..17]) {
        assert_eq!(entry.span, span);
        assert_eq!(scalars(entry), [None; 4]);
        assert_eq!((entry.level, entry.version_kind), (None, None));
        borrowed(entry, &mixed);
    }
}

#[test]
fn table_wrong_wire_unknown_bodies_and_nonminimal_framing() {
    for (literal, records, work) in [
        ("08 01 0d 01 00 00 00 09 01 02 03 04 05 06 07 08", 3, 22),
        ("12 02 08 80", 1, 6),
    ] {
        let bytes = hex(literal);
        let out = decode_table(
            &bytes,
            Limits {
                records,
                work,
                owned_bytes: 0,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(out.raw, bytes);
        assert!(out.requirements.is_empty());
    }
    for (literal, span, version, work) in [
        ("0a 04 0a 02 00 ff", 2..6, None, 14),
        ("8a 80 80 80 10 82 80 80 80 10 08 01", 10..12, Some(1), 18),
    ] {
        let bytes = hex(literal);
        let s = size_of::<VersionRequirement<'_>>();
        let out = decode_table(
            &bytes,
            Limits {
                records: 2,
                work: work + s,
                owned_bytes: s,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(out.requirements.len(), 1);
        let entry = &out.requirements[0];
        assert_eq!(entry.span, span);
        assert_eq!(scalars(entry), [version, None, None, None]);
        borrowed(entry, &bytes);
    }
}

#[test]
fn malformed_table_children_cannot_consume_sibling_bytes() {
    for (literal, kind, offset) in [
        ("0a 02 08 80 0a 00", ErrorKind::Truncated, 4),
        ("0a 01 00", ErrorKind::InvalidFieldNumber, 2),
        ("0a 01 0e", ErrorKind::InvalidWireType, 2),
        ("0a 01 0b", ErrorKind::UnsupportedGroup, 2),
        ("0a 02 08 01 00", ErrorKind::InvalidFieldNumber, 4),
        ("0a 02 0a 80", ErrorKind::Truncated, 4),
        ("0a", ErrorKind::Truncated, 1),
        ("0a ff ff ff ff 0f", ErrorKind::NegativeLength, 1),
        ("00", ErrorKind::InvalidFieldNumber, 0),
    ] {
        table_error(&hex(literal), Limits::default(), kind, offset);
    }
    for bytes in [
        fixture!("table_malformed_child"),
        fixture!("table_truncated_parent"),
    ] {
        table_error(
            &bytes,
            Limits {
                owned_bytes: 0,
                ..Limits::default()
            },
            ErrorKind::Truncated,
            4,
        );
    }
    table_error(
        &hex("0a 02 08 80 0a 00"),
        Limits {
            owned_bytes: 0,
            ..Limits::default()
        },
        ErrorKind::Truncated,
        4,
    );
}

#[test]
fn scalar_budget_checks_preserve_read_dispatch_and_short_precedence() {
    let bytes = hex("08 01");
    let limits = wire::Limits {
        bytes: 2,
        records: 1,
        work: 4,
    };
    assert!(decode(&bytes, limits).is_ok());
    error(
        &bytes,
        wire::Limits {
            bytes: 1,
            records: 0,
            work: 0,
        },
        ErrorKind::Limit("bytes"),
        0,
    );
    error(
        &bytes,
        wire::Limits {
            records: 0,
            ..limits
        },
        ErrorKind::Limit("records"),
        0,
    );
    for (work, offset) in [(0, 0), (1, 0), (2, 1), (3, 0)] {
        error(
            &bytes,
            wire::Limits { work, ..limits },
            ErrorKind::Limit("work"),
            offset,
        );
    }
    for (work, kind) in [(3, ErrorKind::Limit("work")), (4, ErrorKind::Truncated)] {
        error(
            &hex("08 80"),
            wire::Limits {
                work,
                ..wire::Limits::default()
            },
            kind,
            2,
        );
    }
    for (work, kind, offset) in [
        (5, ErrorKind::Limit("work"), 2),
        (6, ErrorKind::Truncated, 4),
    ] {
        table_error(
            &hex("0a 03 08 01"),
            Limits {
                work,
                ..Limits::default()
            },
            kind,
            offset,
        );
    }
}

#[test]
fn aggregate_table_budget_is_not_reset_for_siblings() {
    let bytes = fixture!("table_aggregate");
    let s = size_of::<VersionRequirement<'_>>();
    let limits = Limits {
        bytes: 8,
        records: 4,
        work: 20 + 2 * s,
        owned_bytes: 2 * s,
    };
    let out = decode_table(&bytes, limits).unwrap();
    assert_eq!(out.requirements.len(), 2);
    assert_eq!(out.requirements[0].span, 2..4);
    assert_eq!(scalars(&out.requirements[0]), [Some(1), None, None, None]);
    assert_eq!(out.requirements[1].span, 6..8);
    assert_eq!(scalars(&out.requirements[1]), [None, Some(2), None, None]);
    for (limited, label, offset) in [
        (Limits { bytes: 7, ..limits }, "bytes", 0),
        (
            Limits {
                records: 3,
                ..limits
            },
            "records",
            6,
        ),
        (
            Limits {
                owned_bytes: 2 * s - 1,
                ..limits
            },
            "owned_bytes",
            4,
        ),
        (
            Limits {
                work: 19 + 2 * s,
                ..limits
            },
            "work",
            4,
        ),
    ] {
        table_error(&bytes, limited, ErrorKind::Limit(label), offset);
    }
    // The second parent/child reads precede the second reservation.
    for (work, offset) in [
        (10 + s, 4),
        (16 + s, 6),
        (17 + s, 6),
        (18 + s, 7),
        (19 + s, 6),
        (20 + s, 4),
    ] {
        table_error(
            &bytes,
            Limits { work, ..limits },
            ErrorKind::Limit("work"),
            offset,
        );
    }
}

#[test]
fn empty_entries_charge_exact_logical_capacity_growth() {
    let s = size_of::<VersionRequirement<'_>>();
    for (n, capacity) in [(1, 1), (2, 2), (3, 4), (5, 8)] {
        let bytes = [10, 0].repeat(n);
        let limits = Limits {
            bytes: bytes.len(),
            records: n,
            work: 4 * n + capacity * s,
            owned_bytes: capacity * s,
        };
        let out = decode_table(&bytes, limits).unwrap();
        assert_eq!(out.requirements.len(), n);
        for (i, entry) in out.requirements.iter().enumerate() {
            assert_eq!(entry.span, (2 * i + 2)..(2 * i + 2));
            assert_eq!(scalars(entry), [None; 4]);
            borrowed(entry, &bytes);
        }
        let last_growth_tag = if n == 1 { 0 } else { 2 * (capacity / 2) };
        table_error(
            &bytes,
            Limits {
                owned_bytes: capacity * s - 1,
                ..limits
            },
            ErrorKind::Limit("owned_bytes"),
            last_growth_tag,
        );
        table_error(
            &bytes,
            Limits {
                work: limits.work - 1,
                ..limits
            },
            ErrorKind::Limit("work"),
            last_growth_tag,
        );
    }
    let bytes = hex("0a 00");
    for (work, owned_bytes, label) in [(3, 0, "work"), (4, 0, "owned_bytes"), (3 + s, s, "work")] {
        table_error(
            &bytes,
            Limits {
                work,
                owned_bytes,
                ..Limits::default()
            },
            ErrorKind::Limit(label),
            0,
        );
    }
}

#[test]
fn hard_limits_clamp_and_empty_messages_need_no_budget() {
    let zero = wire::Limits {
        bytes: 0,
        records: 0,
        work: 0,
    };
    assert_eq!(scalars(&decode(&[], zero).unwrap()), [None; 4]);
    let zero_table = Limits {
        bytes: 0,
        records: 0,
        work: 0,
        owned_bytes: 0,
    };
    assert!(
        decode_table(&[], zero_table)
            .unwrap()
            .requirements
            .is_empty()
    );
    table_error(&[10, 0], zero_table, ErrorKind::Limit("bytes"), 0);
    let unlimited = wire::Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
    };
    let unlimited_table = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    let oversized = vec![0; MAX_BYTES + 1];
    error(&oversized, unlimited, ErrorKind::Limit("bytes"), 0);
    table_error(&oversized, unlimited_table, ErrorKind::Limit("bytes"), 0);
    let scalars = [8, 0].repeat(MAX_RECORDS);
    assert!(decode(&scalars, unlimited).is_ok());
    error(
        &[scalars, vec![8, 0]].concat(),
        unlimited,
        ErrorKind::Limit("records"),
        2 * MAX_RECORDS,
    );
    let empty = [10, 0].repeat(MAX_RECORDS);
    assert_eq!(
        decode_table(&empty, unlimited_table)
            .unwrap()
            .requirements
            .len(),
        MAX_RECORDS
    );
    table_error(
        &[empty, vec![10, 0]].concat(),
        unlimited_table,
        ErrorKind::Limit("records"),
        2 * MAX_RECORDS,
    );
    let mut mixed = [10, 2, 8, 1].repeat(32767);
    mixed.extend([10, 0]);
    assert_eq!(
        decode_table(&mixed, unlimited_table)
            .unwrap()
            .requirements
            .len(),
        32768
    );
    mixed.extend([10, 2, 8, 1]);
    table_error(&mixed, unlimited_table, ErrorKind::Limit("records"), 131070);
}

#[test]
fn exact_prefix_results_and_short_inputs_preserve_boundaries() {
    let bytes = fixture!("explicit");
    for n in 0..bytes.len() {
        if [0, 2, 6, 8, 10, 12].contains(&n) {
            let out = decode(&bytes[..n], wire::Limits::default()).unwrap();
            borrowed(&out, &bytes[..n]);
            assert_eq!(
                scalars(&out),
                [
                    (n >= 2).then_some(127),
                    (n >= 6).then_some(65536),
                    (n >= 10).then_some(9),
                    (n >= 12).then_some(7),
                ]
            );
            assert_eq!(out.level, (n >= 8).then_some(Level::Hidden));
            assert_eq!(out.version_kind, None);
        } else {
            error(
                &bytes[..n],
                wire::Limits::default(),
                ErrorKind::Truncated,
                n,
            );
        }
    }
    let bytes = fixture!("table_ordered");
    for n in 0..bytes.len() {
        if let Some(entries) = [0, 2, 8, 14].iter().position(|&boundary| boundary == n) {
            let out = decode_table(&bytes[..n], Limits::default()).unwrap();
            assert_eq!(out.raw, &bytes[..n]);
            assert_eq!(out.requirements.len(), entries);
            for (i, entry) in out.requirements.iter().enumerate() {
                assert_eq!(entry.span, [2..2, 4..8, 10..14][i]);
                borrowed(entry, &bytes[..n]);
                assert_eq!(
                    scalars(entry),
                    [if i == 0 { None } else { Some(1) }, None, None, None]
                );
                assert_eq!(
                    entry.level,
                    if i == 0 { None } else { Some(Level::Warning) }
                );
                assert_eq!(entry.version_kind, None);
            }
        } else {
            table_error(&bytes[..n], Limits::default(), ErrorKind::Truncated, n);
        }
    }
    for byte in 0..=255 {
        for n in 0..=10 {
            let bytes = vec![byte; n];
            let _ = decode(&bytes, wire::Limits::default());
            let _ = decode_table(&bytes, Limits::default());
        }
    }
}
