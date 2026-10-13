use ktlsp_binary::metadata::{
    payload::{Limits, Payload, decode},
    string_table::{self, Operation, Record},
    wire::{self, Error, ErrorKind, Field},
};
use std::mem::size_of;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits).unwrap_err(), Error { kind, offset });
}
fn borrowed(bytes: &[u8], out: &Payload<'_>) {
    for (raw, span) in [
        (out.prefix, out.prefix_span.clone()),
        (out.string_table.raw, out.string_table_span.clone()),
        (out.payload, out.payload_span.clone()),
    ] {
        assert_eq!(raw, &bytes[span.clone()]);
        assert!(std::ptr::eq(raw.as_ptr(), bytes[span].as_ptr()));
    }
    let table = out.string_table.raw;
    for record in &out.string_table.records {
        assert_eq!(record.raw, &table[record.span.clone()]);
        assert!(std::ptr::eq(
            record.raw.as_ptr(),
            table[record.span.clone()].as_ptr()
        ));
    }
    for field in out
        .string_table
        .unknown
        .iter()
        .chain(out.string_table.records.iter().flat_map(|r| &r.unknown))
    {
        assert_eq!(field.raw, &table[field.span.clone()]);
        assert!(std::ptr::eq(
            field.raw.as_ptr(),
            table[field.span.clone()].as_ptr()
        ));
    }
}

#[test]
fn authored_class_and_package_data_share_exact_budgets() {
    let bytes = hex(include_str!("fixtures/metadata_payload/class.hex"));
    let limits = Limits {
        bytes: 7,
        records: 3,
        nodes: 2,
        depth: 2,
        work: 14 + size_of::<Record<'_>>(),
        owned_bytes: size_of::<Record<'_>>(),
    };
    let out = decode(&bytes, limits).unwrap();
    assert_eq!(out.prefix_span, 0..1);
    assert_eq!(out.string_table_span, 1..5);
    assert_eq!(out.payload_span, 5..7);
    assert_eq!(out.payload, [8, 1]);
    assert_eq!(out.string_table.records[0].span, 2..4);
    assert_eq!(out.string_table.records[0].range, Some(2));
    borrowed(&bytes, &out);

    let bytes = hex(include_str!("fixtures/metadata_payload/package.hex"));
    let backing = 2 * size_of::<Record<'_>>() + 4 * size_of::<i32>();
    let limits = Limits {
        bytes: 15,
        records: 8,
        nodes: 3,
        depth: 2,
        work: 36 + backing,
        owned_bytes: backing,
    };
    let out = decode(&bytes, limits).unwrap();
    assert_eq!(out.string_table_span, 1..13);
    assert_eq!(out.payload_span, 13..15);
    assert_eq!(out.payload, [0x0b, 0xff]);
    assert_eq!(out.string_table.records.len(), 2);
    assert_eq!(out.string_table.records[0].range, Some(2));
    assert_eq!(out.string_table.records[1].range, None);
    assert_eq!(out.string_table.records[1].span, 6..6);
    assert_eq!(out.string_table.local_name, [1, 2, 3]);
    borrowed(&bytes, &out);
    for (limits, label, offset) in [
        (
            Limits {
                bytes: 14,
                ..limits
            },
            "bytes",
            0,
        ),
        (
            Limits {
                records: 7,
                ..limits
            },
            "records",
            12,
        ),
        (Limits { nodes: 2, ..limits }, "nodes", 7),
        (Limits { depth: 1, ..limits }, "depth", 3),
        (
            Limits {
                work: 35 + backing,
                ..limits
            },
            "work",
            12,
        ),
        (
            Limits {
                owned_bytes: backing - 1,
                ..limits
            },
            "owned_bytes",
            12,
        ),
    ] {
        error(&bytes, limits, ErrorKind::Limit(label), offset);
    }
    for raised in [
        Limits {
            bytes: 16,
            ..limits
        },
        Limits {
            records: 9,
            ..limits
        },
        Limits { nodes: 4, ..limits },
        Limits { depth: 3, ..limits },
        Limits {
            work: 37 + backing,
            ..limits
        },
        Limits {
            owned_bytes: backing + 1,
            ..limits
        },
    ] {
        assert_eq!(decode(&bytes, raised).unwrap(), out);
    }
    let mut longer = bytes.clone();
    longer.extend([0xff; 100]);
    let longer_out = decode(
        &longer,
        Limits {
            bytes: longer.len(),
            ..limits
        },
    )
    .unwrap();
    assert_eq!(longer_out.string_table, out.string_table);
    assert_eq!(longer_out.payload_span, 13..115);
    borrowed(&longer, &longer_out);
}

#[test]
fn prefix_rules_and_table_errors_are_rebased_once() {
    for (input, kind, offset) in [
        ("", ErrorKind::Truncated, 0),
        ("80", ErrorKind::Truncated, 1),
        ("01", ErrorKind::Truncated, 1),
        ("020a", ErrorKind::Truncated, 2),
        ("0100", ErrorKind::InvalidFieldNumber, 1),
        ("010b", ErrorKind::UnsupportedGroup, 1),
        ("010f", ErrorKind::InvalidWireType, 1),
        ("030a01082801", ErrorKind::Truncated, 4),
        ("050a03220180ff", ErrorKind::Truncated, 6),
        ("81000bff", ErrorKind::UnsupportedGroup, 2),
        ("ffffffff07", ErrorKind::Truncated, 5),
        ("8080808008", ErrorKind::NegativeLength, 0),
        ("80808080808080808080", ErrorKind::MalformedVarint, 9),
        (
            "0c2a0a80808080808080808080ff",
            ErrorKind::MalformedVarint,
            12,
        ),
    ] {
        error(&hex(input), Limits::default(), kind, offset);
    }
    for (input, prefix_len) in [
        ("00", 1),
        ("00000b0c0f80ff", 1),
        ("8000ff", 2),
        ("8080808010ff", 5),
        ("80808080808080808002", 10),
    ] {
        let bytes = hex(input);
        let out = decode(
            &bytes,
            Limits {
                owned_bytes: 0,
                work: prefix_len + 2,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(out.prefix_span, 0..prefix_len);
        assert_eq!(out.string_table_span, prefix_len..prefix_len);
        assert_eq!(out.payload_span, prefix_len..bytes.len());
        borrowed(&bytes, &out);
    }
}

#[test]
fn failed_reads_and_empty_root_preserve_precedence() {
    let defaults = Limits::default();
    let exact = Limits {
        bytes: 1,
        records: 1,
        nodes: 1,
        depth: 1,
        work: 3,
        owned_bytes: 0,
    };
    assert!(decode(&[0], exact).is_ok());
    for (limits, label, offset) in [
        (
            Limits {
                records: 0,
                ..exact
            },
            "records",
            0,
        ),
        (Limits { work: 0, ..exact }, "work", 0),
        (Limits { work: 1, ..exact }, "work", 0),
        (Limits { work: 2, ..exact }, "work", 1),
        (Limits { nodes: 0, ..exact }, "nodes", 1),
        (Limits { depth: 0, ..exact }, "depth", 1),
        (
            Limits {
                bytes: 0,
                records: 0,
                ..exact
            },
            "bytes",
            0,
        ),
        (
            Limits {
                depth: 0,
                nodes: 0,
                work: 2,
                ..exact
            },
            "depth",
            1,
        ),
        (
            Limits {
                nodes: 0,
                work: 2,
                ..exact
            },
            "nodes",
            1,
        ),
    ] {
        error(&[0], limits, ErrorKind::Limit(label), offset);
    }
    error(
        &[],
        Limits {
            records: 0,
            ..defaults
        },
        ErrorKind::Limit("records"),
        0,
    );
    for work in [0, 1] {
        error(
            &[],
            Limits { work, ..defaults },
            ErrorKind::Limit("work"),
            0,
        );
    }
    error(
        &[],
        Limits {
            work: 2,
            ..defaults
        },
        ErrorKind::Truncated,
        0,
    );
    for (input, work, kind, offset) in [
        ("01", 2, ErrorKind::Limit("work"), 1),
        ("01", 3, ErrorKind::Truncated, 1),
        ("020a", 3, ErrorKind::Limit("work"), 1),
        ("020a", 4, ErrorKind::Truncated, 2),
        ("050a03220180ff", 18, ErrorKind::Limit("work"), 6),
        ("050a03220180ff", 19, ErrorKind::Truncated, 6),
        ("8000", 3, ErrorKind::Limit("work"), 2),
    ] {
        error(&hex(input), Limits { work, ..defaults }, kind, offset);
    }
    for n in 1..=10 {
        let bytes = vec![0x80; n];
        let (kind, offset) = if n == 10 {
            (ErrorKind::MalformedVarint, 9)
        } else {
            (ErrorKind::Truncated, n)
        };
        error(&bytes, defaults, kind, offset);
    }
}

#[test]
fn unknown_fields_raw_strings_and_typed_spans_keep_table_coordinates() {
    let bytes = hex(include_str!("fixtures/metadata_payload/unknown.hex"));
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.string_table_span, 1..16);
    assert_eq!(out.payload, [0, 0xff]);
    assert_eq!(out.string_table.records[0].span, 4..13);
    assert_eq!(out.string_table.records[0].operation, None);
    assert_eq!(
        out.string_table
            .unknown
            .iter()
            .map(|f| f.span.clone())
            .collect::<Vec<_>>(),
        [0..2, 13..15]
    );
    assert_eq!(
        out.string_table.records[0]
            .unknown
            .iter()
            .map(|f| f.span.clone())
            .collect::<Vec<_>>(),
        [4..6, 6..8, 8..13]
    );
    borrowed(&bytes, &out);
    let bytes = hex("050a033201ffff");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.string_table.records[0].string, Some(&[0xff][..]));
    assert!(std::ptr::eq(
        out.string_table.records[0].string.unwrap().as_ptr(),
        bytes[5..].as_ptr()
    ));
    let bytes = hex("060a040a020b0c0b");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.string_table.records[0].range, None);
    assert_eq!(
        out.string_table.records[0].unknown[0].raw,
        [0x0a, 2, 0x0b, 0x0c]
    );
    assert_eq!(out.payload, [0x0b]);
    borrowed(&bytes, &out);

    for table in [
        "0a0c080208001801180732002003",
        "28012a0202032804",
        "28ffffffffffffffffff01",
        "288080808010",
        "0a041802187f",
        "0a0418011800",
        "0a0418011a00",
        "2a0028012a00",
    ] {
        let table = hex(table);
        let mut bytes = vec![u8::try_from(table.len()).unwrap()];
        bytes.extend(&table);
        bytes.extend([0, 0xff]);
        let out = decode(&bytes, Limits::default()).unwrap();
        assert_eq!(
            out.string_table,
            string_table::decode(&table, Limits::default()).unwrap()
        );
        borrowed(&bytes, &out);
    }
    let bytes = hex("060a041801187f");
    let backing = size_of::<Record<'_>>() + size_of::<Field<'_>>();
    let limits = Limits {
        work: 20 + backing,
        owned_bytes: backing,
        ..Limits::default()
    };
    assert_eq!(
        decode(&bytes, limits).unwrap().string_table.records[0].operation,
        Some(Operation::InternalToClassId)
    );
    error(
        &bytes,
        Limits {
            work: 19 + backing,
            ..limits
        },
        ErrorKind::Limit("work"),
        3,
    );
}

#[test]
fn all_truncation_cuts_distinguish_envelope_table_and_opaque_remainder() {
    for (input, table_end) in [
        (include_str!("fixtures/metadata_payload/class.hex"), 5),
        (include_str!("fixtures/metadata_payload/package.hex"), 13),
        (include_str!("fixtures/metadata_payload/unknown.hex"), 16),
    ] {
        let bytes = hex(input);
        for n in 0..table_end {
            error(&bytes[..n], Limits::default(), ErrorKind::Truncated, n);
        }
        let full = decode(&bytes, Limits::default()).unwrap();
        for n in table_end..=bytes.len() {
            let out = decode(&bytes[..n], Limits::default()).unwrap();
            assert_eq!(out.string_table, full.string_table);
            assert_eq!(out.payload_span, table_end..n);
            borrowed(&bytes[..n], &out);
        }
    }
    let table = hex("0a0c080208001801180732002003");
    for m in 0..=table.len() {
        let mut bytes = vec![u8::try_from(m).unwrap()];
        bytes.extend(&table[..m]);
        if m == 0 || m == table.len() {
            assert!(decode(&bytes, Limits::default()).is_ok());
        } else {
            error(&bytes, Limits::default(), ErrorKind::Truncated, m + 1);
        }
    }
}

#[test]
fn storage_growth_and_packed_records_use_the_same_budget() {
    for (input, backing, work, offset) in [
        ("06080108010801", 4 * size_of::<Field<'_>>(), 15, 5),
        ("06280128022803", 4 * size_of::<i32>(), 18, 5),
        ("0a08010801080108010801", 8 * size_of::<Field<'_>>(), 23, 9),
        ("0a28012802280328042805", size_of::<[i32; 8]>(), 28, 9),
        ("040a000a00", 2 * size_of::<Record<'_>>(), 13, 5),
    ] {
        let bytes = hex(input);
        let limits = Limits {
            work: work + backing,
            owned_bytes: backing,
            ..Limits::default()
        };
        assert!(decode(&bytes, limits).is_ok());
        error(
            &bytes,
            Limits {
                owned_bytes: backing - 1,
                ..limits
            },
            ErrorKind::Limit("owned_bytes"),
            offset,
        );
        error(
            &bytes,
            Limits {
                work: work + backing - 1,
                ..limits
            },
            ErrorKind::Limit("work"),
            offset,
        );
    }
    for (input, offset) in [("020801", 1), ("020a00", 3)] {
        error(
            &hex(input),
            Limits {
                owned_bytes: 0,
                ..Limits::default()
            },
            ErrorKind::Limit("owned_bytes"),
            offset,
        );
    }
    let bytes = hex("100a04220201020a042a0203042a020506");
    let limits = Limits {
        records: 12,
        ..Limits::default()
    };
    let out = decode(&bytes, limits).unwrap();
    assert_eq!(out.string_table.records[0].substring_index, [1, 2]);
    assert_eq!(out.string_table.records[1].replace_char, [3, 4]);
    assert_eq!(out.string_table.local_name, [5, 6]);
    error(
        &bytes,
        Limits {
            records: 11,
            ..limits
        },
        ErrorKind::Limit("records"),
        16,
    );
}

#[test]
fn hard_limits_cannot_be_raised_and_legacy_opaque_policy_is_separate() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    error(
        &vec![0; ktlsp_binary::MAX_BYTES + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = hex("fcff07");
    for _ in 0..ktlsp_binary::MAX_RECORDS - 1 {
        bytes.extend([0x28, 0]);
    }
    assert_eq!(
        decode(&bytes, unlimited)
            .unwrap()
            .string_table
            .local_name
            .len(),
        ktlsp_binary::MAX_RECORDS - 1
    );
    bytes[..3].copy_from_slice(&hex("feff07"));
    let offset = bytes.len();
    bytes.extend([0x28, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), offset);
    let bytes = [0, 0x0b];
    assert!(decode(&bytes, unlimited).is_ok());
    assert_eq!(
        wire::decode(&bytes, wire::Limits::default()).unwrap_err(),
        Error {
            kind: ErrorKind::UnsupportedGroup,
            offset: 1
        }
    );
    assert_eq!(
        string_table::decode(&[0x0b], Limits::default()).unwrap_err(),
        Error {
            kind: ErrorKind::UnsupportedGroup,
            offset: 0
        }
    );
}

#[test]
fn deterministic_bounded_malformed_probes_and_opaque_bytes() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        nodes: 4,
        depth: 2,
        work: 256,
        owned_bytes: 1024,
    };
    for a in 0..=255 {
        for b in 0..=255 {
            let bytes = [2, a, b];
            let first = decode(&bytes, limits);
            assert_eq!(first, decode(&bytes, limits));
            if let Ok(out) = first {
                borrowed(&bytes, &out);
            }
            let bytes = [0, a, b];
            let out = decode(
                &bytes,
                Limits {
                    records: 1,
                    nodes: 1,
                    depth: 1,
                    work: 3,
                    owned_bytes: 0,
                    ..limits
                },
            )
            .unwrap();
            assert_eq!(out, decode(&bytes, limits).unwrap());
            assert_eq!(out.payload, [a, b]);
            borrowed(&bytes, &out);
        }
        for n in 0..32 {
            let bytes = vec![a; n];
            let first = decode(&bytes, limits);
            assert_eq!(first, decode(&bytes, limits));
            if let Ok(out) = first {
                borrowed(&bytes, &out);
            }
        }
    }
}
