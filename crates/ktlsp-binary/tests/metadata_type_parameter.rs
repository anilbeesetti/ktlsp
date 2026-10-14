use ktlsp_binary::metadata::{
    type_parameter::{Error, ErrorKind, Limits, TypeParameter, UpperBound, Variance, decode},
    wire::ErrorKind as WireKind,
};
use std::mem::size_of;

const BASE: &[u8] = &[8, 0, 16, 0];
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn tail(text: &str) -> Vec<u8> {
    let mut bytes = BASE.to_vec();
    bytes.extend(hex(text));
    bytes
}
fn borrowed(out: &TypeParameter<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    for bound in &out.upper_bound {
        assert!(bound.span.start <= bound.span.end && bound.span.end <= bytes.len());
        assert_eq!(bound.raw, &bytes[bound.span.clone()]);
        assert_eq!(bound.raw.as_ptr(), bytes[bound.span.clone()].as_ptr());
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { kind, offset }));
}
fn wire_error(bytes: &[u8], limits: Limits, kind: WireKind, offset: usize) {
    error(bytes, limits, ErrorKind::Wire(kind), offset);
}
fn minimal() -> Vec<u8> {
    hex(include_str!("fixtures/metadata_type_parameter/minimal.hex"))
}
fn mixed() -> Vec<u8> {
    hex(include_str!("fixtures/metadata_type_parameter/mixed.hex"))
}
fn scalars() -> Vec<u8> {
    hex(include_str!("fixtures/metadata_type_parameter/scalars.hex"))
}

#[test]
fn frozen_fixtures_preserve_presence_signed_values_and_borrowed_bounds() {
    let bytes = minimal();
    let out = decode(
        &bytes,
        Limits {
            bytes: 4,
            records: 2,
            work: 8,
            owned_bytes: 0,
        },
    )
    .unwrap();
    assert_eq!(
        (out.id, out.name, out.reified, out.variance),
        (0, 0, None, None)
    );
    assert!(out.upper_bound.is_empty() && out.upper_bound_id.is_empty());
    borrowed(&out, &bytes);
    let bytes = scalars();
    let out = decode(
        &bytes,
        Limits {
            bytes: 22,
            records: 5,
            work: 37,
            owned_bytes: 4,
        },
    )
    .unwrap();
    assert_eq!(
        (out.id, out.name, out.reified, out.variance),
        (-1, -2, Some(false), Some(Variance::Inv))
    );
    assert_eq!(out.upper_bound_id, [-1]);
    assert!(out.upper_bound.is_empty());
    borrowed(&out, &bytes);
    let bytes = mixed();
    let s = size_of::<UpperBound<'_>>();
    let out = decode(
        &bytes,
        Limits {
            bytes: 20,
            records: 10,
            work: 60 + 2 * s,
            owned_bytes: 16 + 2 * s,
        },
    )
    .unwrap();
    assert_eq!(
        (out.id, out.name, out.reified, out.variance),
        (0, 0, None, None)
    );
    assert_eq!(out.upper_bound_id, [7, 8, 9, 10]);
    assert_eq!(
        out.upper_bound,
        [
            UpperBound {
                raw: &[],
                span: 16..16
            },
            UpperBound {
                raw: &[11, 255],
                span: 18..20
            }
        ]
    );
    borrowed(&out, &bytes);
}

#[test]
fn scalar_duplicates_unknown_enums_and_wrong_wire_fields() {
    for (text, reified, variance) in [
        ("18 00 20 02", Some(false), Some(Variance::Inv)),
        ("18 02 20 00", Some(true), Some(Variance::In)),
        (
            "18 01 18 00 20 00 20 01 20 03",
            Some(false),
            Some(Variance::Out),
        ),
        ("20 03", None, None),
        ("20 ff ff ff ff 0f", None, None),
        ("0a 00 12 00 1a 00 22 00 28 01 35 00 00 00 00", None, None),
        ("18 80 80 80 80 10", Some(true), None),
        ("18 80 80 80 80 80 80 80 80 80 02", Some(false), None),
    ] {
        let bytes = tail(text);
        let out = decode(&bytes, Default::default()).unwrap();
        assert_eq!(
            (out.id, out.name, out.reified, out.variance),
            (0, 0, reified, variance)
        );
        assert!(out.upper_bound.is_empty() && out.upper_bound_id.is_empty());
        borrowed(&out, &bytes);
    }
    for (text, id, name) in [
        ("08 01 10 02 08 03 10 04", 3, 4),
        ("88 00 80 00 90 00 81 00", 0, 1),
        ("08 80 80 80 80 10 10 00", 0, 0),
    ] {
        let bytes = hex(text);
        let out = decode(&bytes, Default::default()).unwrap();
        assert_eq!(
            (out.id, out.name, out.reified, out.variance),
            (id, name, None, None)
        );
        borrowed(&out, &bytes);
    }
}

#[test]
fn bound_ids_append_across_packed_unpacked_empty_and_signed_segments() {
    for (text, ids) in [
        ("30 01 32 02 02 03 30 04 32 00", &[1, 2, 3, 4][..]),
        ("32 01 07 32 01 08", &[7, 8][..]),
        ("32 05 ff ff ff ff 0f", &[-1][..]),
        ("32 02 80 00", &[0][..]),
    ] {
        let bytes = tail(text);
        let out = decode(&bytes, Default::default()).unwrap();
        assert_eq!(out.upper_bound_id, ids);
        assert!(out.upper_bound.is_empty());
        borrowed(&out, &bytes);
    }
}

#[test]
fn selected_bounds_annotations_extensions_and_unknowns_stay_opaque() {
    let bytes = tail("2a 01 80 2a 00");
    let out = decode(&bytes, Default::default()).unwrap();
    assert_eq!(
        out.upper_bound,
        [
            UpperBound {
                raw: &[128],
                span: 6..7
            },
            UpperBound {
                raw: &[],
                span: 9..9
            }
        ]
    );
    borrowed(&out, &bytes);
    for text in [
        "a2 06 01 00 aa 06 01 80",
        "79 01 02 03 04 05 06 07 08 85 01 78 56 34 12",
    ] {
        let bytes = tail(text);
        let out = decode(&bytes, Default::default()).unwrap();
        assert!(out.upper_bound.is_empty() && out.upper_bound_id.is_empty());
        borrowed(&out, &bytes);
    }
}

#[test]
fn missing_required_fields_are_checked_after_the_entire_scan() {
    for (text, kind, offset) in [
        ("", ErrorKind::MissingId, 0),
        ("08 00", ErrorKind::MissingName, 2),
        ("10 00", ErrorKind::MissingId, 2),
        ("0a 00 10 00", ErrorKind::MissingId, 4),
        ("08 00 12 00", ErrorKind::MissingName, 4),
        ("08 00 80", ErrorKind::Wire(WireKind::Truncated), 3),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
    error(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            work: 0,
            owned_bytes: 0,
        },
        ErrorKind::MissingId,
        0,
    );
}

#[test]
fn malformed_tails_keep_wire_error_kinds_and_message_relative_offsets() {
    for (text, kind, offset) in [
        ("00", WireKind::InvalidFieldNumber, 4),
        ("07", WireKind::InvalidFieldNumber, 4),
        ("0b", WireKind::UnsupportedGroup, 4),
        ("0c", WireKind::UnsupportedGroup, 4),
        ("0e", WireKind::InvalidWireType, 4),
        ("0f", WireKind::InvalidWireType, 4),
        ("30", WireKind::Truncated, 5),
        ("32 02 01", WireKind::Truncated, 7),
        ("32 01 80", WireKind::Truncated, 7),
        ("32 01 80 30 01", WireKind::Truncated, 7),
        ("2a 02 01", WireKind::Truncated, 7),
        ("32 80 80 80 80 08", WireKind::NegativeLength, 5),
        (
            "30 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            14,
        ),
        (
            "32 0a 80 80 80 80 80 80 80 80 80 80",
            WireKind::MalformedVarint,
            15,
        ),
        ("79 00", WireKind::Truncated, 6),
    ] {
        wire_error(&tail(text), Default::default(), kind, offset);
    }
}

#[test]
fn fixture_prefixes_have_frozen_clean_eof_and_truncation_outcomes() {
    for (bytes, boundaries) in [
        (minimal(), &[4][..]),
        (mixed(), &[4, 6, 10, 12, 14, 16, 20][..]),
        (scalars(), &[12, 14, 16, 22][..]),
    ] {
        for n in 0..=bytes.len() {
            let prefix = &bytes[..n];
            if boundaries.contains(&n) {
                let out = decode(prefix, Default::default()).unwrap();
                borrowed(&out, prefix);
                if bytes.len() == 20 {
                    let ids: &[i32] = match n {
                        4 => &[],
                        6 => &[7],
                        10 | 12 => &[7, 8, 9],
                        _ => &[7, 8, 9, 10],
                    };
                    assert_eq!(out.upper_bound_id, ids);
                    let count = match n {
                        16 => 1,
                        20 => 2,
                        _ => 0,
                    };
                    assert_eq!(out.upper_bound.len(), count);
                } else if bytes.len() == 22 {
                    assert_eq!((out.id, out.name), (-1, -2));
                    assert_eq!(out.reified, (n >= 14).then_some(false));
                    assert_eq!(out.variance, (n >= 16).then_some(Variance::Inv));
                    assert_eq!(
                        out.upper_bound_id,
                        if n == 22 { &[-1][..] } else { &[][..] }
                    );
                }
            } else if n == 0 {
                error(prefix, Default::default(), ErrorKind::MissingId, n);
            } else if (bytes.len() != 22 && n == 2) || (bytes.len() == 22 && n == 6) {
                error(prefix, Default::default(), ErrorKind::MissingName, n);
            } else {
                wire_error(prefix, Default::default(), WireKind::Truncated, n);
            }
        }
    }
}

#[test]
fn exact_shared_limits_precharge_reservations_and_do_not_reset_at_packed_fields() {
    for (limits, kind, offset) in [
        (
            Limits {
                bytes: 3,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            Limits {
                records: 1,
                ..Default::default()
            },
            "records",
            2,
        ),
        (
            Limits {
                work: 0,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                work: 1,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                work: 7,
                ..Default::default()
            },
            "work",
            2,
        ),
    ] {
        wire_error(BASE, limits, WireKind::Limit(kind), offset);
    }
    let s = size_of::<UpperBound<'_>>();
    for (text, records, work, owned) in [
        ("30 07", 3, 17, 4),
        ("32 02 07 08", 5, 28, 8),
        ("32 01 07 32 01 08", 6, 32, 8),
        ("32 00", 3, 12, 0),
        ("2a 00", 3, 12 + s, s),
    ] {
        let bytes = tail(text);
        let out = decode(
            &bytes,
            Limits {
                bytes: bytes.len(),
                records,
                work,
                owned_bytes: owned,
            },
        )
        .unwrap();
        borrowed(&out, &bytes);
    }
    for (text, limits, kind, offset) in [
        (
            "30 07",
            Limits {
                owned_bytes: 3,
                ..Default::default()
            },
            "owned_bytes",
            4,
        ),
        (
            "30 07",
            Limits {
                work: 16,
                ..Default::default()
            },
            "work",
            4,
        ),
        (
            "32 02 07 08",
            Limits {
                records: 4,
                ..Default::default()
            },
            "records",
            7,
        ),
        (
            "32 02 07 08",
            Limits {
                owned_bytes: 7,
                ..Default::default()
            },
            "owned_bytes",
            7,
        ),
        (
            "32 02 07 08",
            Limits {
                work: 27,
                ..Default::default()
            },
            "work",
            7,
        ),
        (
            "32 01 07 32 01 08",
            Limits {
                records: 5,
                ..Default::default()
            },
            "records",
            9,
        ),
        (
            "32 01 07 32 01 08",
            Limits {
                owned_bytes: 7,
                ..Default::default()
            },
            "owned_bytes",
            9,
        ),
        (
            "32 01 07 32 01 08",
            Limits {
                work: 31,
                ..Default::default()
            },
            "work",
            9,
        ),
        (
            "2a 00",
            Limits {
                owned_bytes: s - 1,
                ..Default::default()
            },
            "owned_bytes",
            4,
        ),
        (
            "2a 00",
            Limits {
                work: 11 + s,
                ..Default::default()
            },
            "work",
            4,
        ),
        (
            "32 01 80 30 01",
            Limits {
                records: 3,
                ..Default::default()
            },
            "records",
            6,
        ),
        (
            "32 01 80 30 01",
            Limits {
                work: 14,
                ..Default::default()
            },
            "work",
            6,
        ),
    ] {
        wire_error(&tail(text), limits, WireKind::Limit(kind), offset);
    }
    wire_error(
        &mixed(),
        Limits {
            owned_bytes: 15 + 2 * s,
            ..Default::default()
        },
        WireKind::Limit("owned_bytes"),
        16,
    );
    wire_error(
        &mixed(),
        Limits {
            work: 59 + 2 * s,
            ..Default::default()
        },
        WireKind::Limit("work"),
        16,
    );
}

#[test]
fn caller_limits_cannot_raise_hard_byte_or_record_bounds() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    borrowed(&decode(BASE, unlimited).unwrap(), BASE);
    wire_error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        WireKind::Limit("bytes"),
        0,
    );
    let mut bytes = BASE.to_vec();
    for _ in 0..65_533 {
        bytes.extend_from_slice(&[56, 0]);
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
    borrowed(&out, &bytes);
    assert!(out.upper_bound.is_empty() && out.upper_bound_id.is_empty());
    bytes.extend_from_slice(&[56, 0]);
    wire_error(&bytes, unlimited, WireKind::Limit("records"), 131_070);
}

#[test]
fn deterministic_hostile_bytes_and_tails() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
        owned_bytes: 256,
    };
    let check = |bytes: &[u8]| {
        let out = decode(bytes, limits);
        assert_eq!(out, decode(bytes, limits));
        match out {
            Ok(out) => borrowed(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            check(&[a, b]);
            check(&[8, 0, 16, 0, a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
