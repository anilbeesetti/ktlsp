use ktlsp_binary::metadata::wire::{Error, ErrorKind, Limits, Value, decode};

fn mixed() -> Vec<u8> {
    include_str!("fixtures/metadata_wire/mixed.hex")
        .split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn error(bytes: &[u8], kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, Limits::default()).unwrap_err(),
        Error { kind, offset }
    );
}

#[test]
fn independent_fields_order_duplicates_unknowns_endian_and_borrowed_spans() {
    let bytes = mixed();
    let result = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(result.prefix_span, 0..1);
    assert_eq!(result.prefix, [19]);
    assert!(std::ptr::eq(result.prefix.as_ptr(), bytes.as_ptr()));
    assert_eq!(result.string_table.span, 1..20);
    assert_eq!(result.payload.span, 20..47);
    let expected = [
        (1, 1..4, Value::Varint(300)),
        (2, 4..9, Value::Bytes(&[0xff, 0, 0x80])),
        (1, 9..11, Value::Varint(1)),
        (19000, 11..15, Value::Varint(7)),
        (9, 15..20, Value::Fixed32(0x12345678)),
        (1, 20..29, Value::Fixed64(0x0102030405060708)),
        (536870911, 29..38, Value::Fixed32(0x89abcdef)),
        (100, 38..41, Value::Varint(42)),
        (2, 41..43, Value::Bytes(&[])),
        (2, 43..47, Value::Bytes(&[0x0b, 0x0c])),
    ];
    let fields: Vec<_> = result
        .string_table
        .fields
        .iter()
        .chain(&result.payload.fields)
        .collect();
    assert_eq!(fields.len(), expected.len());
    for (field, (number, span, value)) in fields.iter().zip(expected) {
        assert_eq!(
            (field.number, &field.span, field.value),
            (number, &span, value)
        );
        assert_eq!(field.raw, &bytes[span.clone()]);
        assert!(std::ptr::eq(
            field.raw.as_ptr(),
            bytes[span.clone()].as_ptr()
        ));
        if let Value::Bytes(payload) = field.value {
            assert!(std::ptr::eq(
                payload.as_ptr(),
                bytes[span.end - payload.len()..].as_ptr()
            ));
        }
    }
    for section in [&result.string_table, &result.payload] {
        assert!(std::ptr::eq(
            section.raw.as_ptr(),
            bytes[section.span.clone()].as_ptr()
        ));
        assert_eq!(
            section.fields.iter().map(|f| f.raw.len()).sum::<usize>(),
            section.raw.len()
        );
    }
}

#[test]
fn empty_and_nonminimal_envelope_sections() {
    error(&[], ErrorKind::Truncated, 0);
    for bytes in [&[0][..], &[0x80, 0], &[0x80, 0x80, 0]] {
        let out = decode(bytes, Limits::default()).unwrap();
        assert_eq!(out.prefix, bytes);
        assert_eq!(out.prefix_span, 0..bytes.len());
        assert_eq!(out.string_table.span, bytes.len()..bytes.len());
        assert_eq!(out.payload.span, bytes.len()..bytes.len());
        assert!(out.string_table.fields.is_empty() && out.payload.fields.is_empty());
    }
    // Tags, lengths and the envelope all preserve nonminimal forms.
    let bytes = [0x85, 0, 0x8a, 0, 0x81, 0, 0xff, 0x88, 0, 0x81, 0];
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.string_table.fields[0].span, 2..7);
    assert_eq!(out.string_table.fields[0].value, Value::Bytes(&[0xff]));
    assert_eq!(out.payload.fields[0].span, 7..11);
    assert_eq!(out.payload.fields[0].value, Value::Varint(1));
}

#[test]
fn every_byte_truncation_respects_section_and_complete_field_boundaries() {
    let bytes = mixed();
    for n in 0..=bytes.len() {
        let out = decode(&bytes[..n], Limits::default());
        if [20, 29, 38, 41, 43, 47].contains(&n) {
            assert!(out.is_ok(), "complete field boundary {n}: {out:?}");
        } else {
            assert_eq!(
                out.unwrap_err(),
                Error {
                    kind: ErrorKind::Truncated,
                    offset: n
                },
                "prefix {n}"
            );
        }
    }
    // A field in the first section cannot borrow bytes from the remainder.
    error(&[1, 8, 1], ErrorKind::Truncated, 2);
    error(&[2, 0x12, 1, 0xff], ErrorKind::Truncated, 3);
    error(&[2, 0x0d, 1, 2, 3, 4], ErrorKind::Truncated, 3);
    error(&[2, 0x09, 1, 2, 3, 4, 5, 6, 7, 8], ErrorKind::Truncated, 3);
}

#[test]
fn varint_nonminimal_low_bits_and_overflow_policy() {
    for n in 1..=10 {
        // Nonminimal zero in envelope, tag (field1), length and value positions.
        let mut zero = vec![0x80; n];
        zero[n - 1] = 0;
        assert!(decode(&zero, Limits::default()).is_ok());
        let mut tag = zero.clone();
        tag[0] |= 8;
        let mut bytes = vec![0];
        bytes.extend(tag);
        bytes.extend(&zero);
        let out = decode(&bytes, Limits::default()).unwrap();
        assert_eq!(out.payload.fields[0].number, 1);
        assert_eq!(out.payload.fields[0].value, Value::Varint(0));
        let mut bytes = vec![0, 0x12];
        bytes.extend(zero);
        assert_eq!(
            decode(&bytes, Limits::default()).unwrap().payload.fields[0].value,
            Value::Bytes(&[])
        );
    }
    for last in [0, 1, 2, 0x7e, 0x7f] {
        let mut bytes = vec![0, 8];
        bytes.extend([0x80; 9]);
        bytes.push(last);
        let out = decode(&bytes, Limits::default()).unwrap();
        assert_eq!(
            out.payload.fields[0].value,
            Value::Varint(u64::from(last & 1) << 63)
        );
    }
    let mut maximum = vec![0, 8];
    maximum.extend([0xff; 9]);
    maximum.push(1);
    assert_eq!(
        decode(&maximum, Limits::default()).unwrap().payload.fields[0].value,
        Value::Varint(u64::MAX)
    );
    // Mathematical 2^32+8 truncates to tag8; 2^32 truncates to empty lengths.
    let bytes = [
        0x80, 0x80, 0x80, 0x80, 0x10, 0x88, 0x80, 0x80, 0x80, 0x10, 0,
    ];
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.payload.fields[0].number, 1);
    assert_eq!(out.payload.fields[0].value, Value::Varint(0));
    assert_eq!(
        decode(&[0, 0x12, 0x80, 0x80, 0x80, 0x80, 0x10], Limits::default())
            .unwrap()
            .payload
            .fields[0]
            .value,
        Value::Bytes(&[])
    );
    error(
        &[0, 0x80, 0x80, 0x80, 0x80, 0x10],
        ErrorKind::InvalidFieldNumber,
        1,
    );
}

#[test]
fn precise_malformed_tags_lengths_varints_and_offsets() {
    for tag in 0..8 {
        error(&[0, tag], ErrorKind::InvalidFieldNumber, 1);
    }
    for tag in [0x0b, 0x0c] {
        error(&[0, tag], ErrorKind::UnsupportedGroup, 1);
    }
    for tag in [0x0e, 0x0f] {
        error(&[0, tag], ErrorKind::InvalidWireType, 1);
    }
    error(&[0xff, 0xff, 0xff, 0xff, 0x07], ErrorKind::Truncated, 5);
    error(
        &[0x80, 0x80, 0x80, 0x80, 0x08],
        ErrorKind::NegativeLength,
        0,
    );
    error(
        &[0, 0x12, 0x80, 0x80, 0x80, 0x80, 0x08],
        ErrorKind::NegativeLength,
        2,
    );
    error(
        &[0, 0x12, 0xff, 0xff, 0xff, 0xff, 0x07],
        ErrorKind::Truncated,
        7,
    );
    for prefix in [&[][..], &[0], &[0, 8], &[0, 0x12]] {
        let mut bytes = prefix.to_vec();
        let start = bytes.len();
        bytes.extend([0x80; 10]);
        error(&bytes, ErrorKind::MalformedVarint, start + 9);
        for n in 1..10 {
            error(&bytes[..start + n], ErrorKind::Truncated, start + n);
        }
    }
    error(&[0, 8, 1, 0x0f], ErrorKind::InvalidWireType, 3);
    error(&[2, 8, 1, 0x0b], ErrorKind::UnsupportedGroup, 3);
}

#[test]
fn aggregate_exact_budgets_clean_eof_and_error_precedence() {
    let bytes = mixed();
    let limits = Limits {
        bytes: 47,
        records: 11,
        work: 58,
    };
    assert!(decode(&bytes, limits).is_ok());
    assert_eq!(
        decode(
            &bytes,
            Limits {
                bytes: 46,
                ..limits
            }
        )
        .unwrap_err(),
        Error {
            kind: ErrorKind::Limit("bytes"),
            offset: 0
        }
    );
    assert_eq!(
        decode(
            &bytes,
            Limits {
                records: 10,
                ..limits
            }
        )
        .unwrap_err(),
        Error {
            kind: ErrorKind::Limit("records"),
            offset: 43
        }
    );
    assert_eq!(
        decode(&bytes, Limits { work: 57, ..limits }).unwrap_err(),
        Error {
            kind: ErrorKind::Limit("work"),
            offset: 45
        }
    );
    assert!(
        decode(
            &[0],
            Limits {
                bytes: 1,
                records: 1,
                work: 2
            }
        )
        .is_ok()
    );
    // Independently proposed adversarial probe: neither counter resets at section EOF.
    for (records, work, expected) in [
        (
            2,
            7,
            Some(Error {
                kind: ErrorKind::Limit("records"),
                offset: 3,
            }),
        ),
        (
            3,
            7,
            Some(Error {
                kind: ErrorKind::Limit("work"),
                offset: 4,
            }),
        ),
        (3, 8, None),
    ] {
        assert_eq!(
            decode(
                &[2, 8, 1, 8, 2],
                Limits {
                    records,
                    work,
                    ..limits
                }
            )
            .err(),
            expected
        );
    }
    for (bytes, limits, expected) in [
        (
            &[][..],
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
            },
            Error {
                kind: ErrorKind::Limit("records"),
                offset: 0,
            },
        ),
        (
            &[][..],
            Limits {
                bytes: 0,
                records: 1,
                work: 0,
            },
            Error {
                kind: ErrorKind::Limit("work"),
                offset: 0,
            },
        ),
        (
            &[][..],
            Limits {
                bytes: 0,
                records: 1,
                work: 1,
            },
            Error {
                kind: ErrorKind::Limit("work"),
                offset: 0,
            },
        ),
        (
            &[][..],
            Limits {
                bytes: 0,
                records: 1,
                work: 2,
            },
            Error {
                kind: ErrorKind::Truncated,
                offset: 0,
            },
        ),
        (
            &[0, 0][..],
            Limits {
                bytes: 2,
                records: 1,
                work: 2,
            },
            Error {
                kind: ErrorKind::Limit("records"),
                offset: 1,
            },
        ),
        (
            &[0, 0][..],
            Limits {
                bytes: 2,
                records: 2,
                work: 3,
            },
            Error {
                kind: ErrorKind::Limit("work"),
                offset: 1,
            },
        ),
        (
            &[0, 0][..],
            Limits {
                bytes: 2,
                records: 2,
                work: 4,
            },
            Error {
                kind: ErrorKind::InvalidFieldNumber,
                offset: 1,
            },
        ),
    ] {
        assert_eq!(decode(bytes, limits).unwrap_err(), expected);
    }
    // Each fails after record/tag/prefix costs: available bytes plus one failed attempt.
    for bytes in [
        &[5, 1, 2][..],
        &[0, 0x0d, 1, 2],
        &[0, 0x09, 1, 2],
        &[0, 0x12, 5, 1, 2],
    ] {
        let exact = bytes.len() + if bytes[0] == 0 { 3 } else { 2 };
        assert_eq!(
            decode(
                bytes,
                Limits {
                    work: exact,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .kind,
            ErrorKind::Truncated
        );
        assert_eq!(
            decode(
                bytes,
                Limits {
                    work: exact - 1,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            Error {
                kind: ErrorKind::Limit("work"),
                offset: if bytes[0] != 0 {
                    1
                } else if bytes[1] == 0x12 {
                    3
                } else {
                    2
                }
            }
        );
    }
}

#[test]
fn hard_input_record_limits_cannot_be_raised() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
    };
    let oversized = vec![0; ktlsp_binary::MAX_BYTES + 1];
    assert_eq!(
        decode(&oversized, unlimited).unwrap_err(),
        Error {
            kind: ErrorKind::Limit("bytes"),
            offset: 0
        }
    );
    let mut bytes = vec![0];
    for _ in 0..ktlsp_binary::MAX_RECORDS - 1 {
        bytes.extend([8, 0]);
    }
    assert_eq!(
        decode(&bytes, unlimited).unwrap().payload.fields.len(),
        ktlsp_binary::MAX_RECORDS - 1
    );
    let start = bytes.len();
    bytes.extend([8, 0]);
    assert_eq!(
        decode(&bytes, unlimited).unwrap_err(),
        Error {
            kind: ErrorKind::Limit("records"),
            offset: start
        }
    );
}

#[test]
fn supported_header_outcome_composes_with_precompiled_unit() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let ktlsp_binary::metadata::Outcome::Decoded { header, bytes } =
        ktlsp_binary::metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported header");
    };
    assert_eq!(header.mv, [2, 3, 0]);
    assert_eq!(header.k, 1);
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.prefix, [18]);
    assert_eq!(out.string_table.span, 1..19);
    assert_eq!(
        out.string_table
            .fields
            .iter()
            .map(|f| f.value)
            .collect::<Vec<_>>(),
        [
            Value::Bytes(&[16, 2]),
            Value::Bytes(&[16, 0]),
            Value::Bytes(&[8, 2]),
            Value::Bytes(&[16, 14]),
            Value::Bytes(&[])
        ]
    );
    assert_eq!(out.payload.span, 19..52);
    assert_eq!(
        out.payload
            .fields
            .iter()
            .map(|f| (f.number, f.value))
            .collect::<Vec<_>>(),
        [
            (1, Value::Varint(326)),
            (3, Value::Varint(0)),
            (6, Value::Bytes(&[48, 1])),
            (8, Value::Bytes(&[8, 66, 162, 6, 2, 8, 2])),
            (9, Value::Bytes(&[16, 3, 26, 2, 48, 4, 72, 150, 128, 4])),
            (101, Value::Varint(5))
        ]
    );
}

#[test]
fn deterministic_hostile_bytes_with_tight_budgets() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 40,
    };
    for a in 0..=255 {
        for b in 0..=255 {
            let _ = decode(&[0, a, b], limits);
        }
        for n in 0..32 {
            let _ = decode(&vec![a; n], limits);
        }
    }
}
