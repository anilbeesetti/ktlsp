use ktlsp_binary::metadata::{
    string_table::{Limits, Operation, Record, decode},
    wire::{Error, ErrorKind, Field, Value},
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
fn defaults(record: &Record<'_>) {
    assert_eq!(record.range, None);
    assert_eq!(record.predefined_index, None);
    assert_eq!(record.operation, None);
    assert_eq!(record.string, None);
    assert!(record.substring_index.is_empty());
    assert!(record.replace_char.is_empty());
    assert!(record.unknown.is_empty());
}

#[test]
fn empty_presence_defaults_distinct_records_and_borrowed_raw() {
    let empty = decode(&[], Limits::default()).unwrap();
    assert!(empty.records.is_empty() && empty.local_name.is_empty() && empty.unknown.is_empty());
    let bytes = hex("0a000a00");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records.len(), 2);
    for (record, span) in out.records.iter().zip([2..2, 4..4]) {
        defaults(record);
        assert_eq!(record.span, span);
        assert_eq!(record.raw, &bytes[span.clone()]);
        assert!(std::ptr::eq(record.raw.as_ptr(), bytes[span].as_ptr()));
    }
    assert!(std::ptr::eq(out.raw.as_ptr(), bytes.as_ptr()));
    let bytes = hex("0a080801100018003200");
    let out = decode(&bytes, Limits::default()).unwrap();
    let record = &out.records[0];
    assert_eq!(record.range, Some(1));
    assert_eq!(record.predefined_index, Some(0));
    assert_eq!(record.operation, Some(Operation::None));
    assert_eq!(record.string, Some(&[][..]));
}

#[test]
fn singular_duplicates_enum_unknowns_and_invalid_utf8_are_data() {
    let bytes = hex("0a0c080208001801180732002003");
    let out = decode(&bytes, Limits::default()).unwrap();
    let r = &out.records[0];
    assert_eq!(r.range, Some(0));
    assert_eq!(r.operation, Some(Operation::InternalToClassId));
    assert_eq!(r.string, Some(&[][..]));
    assert_eq!(r.substring_index, [3]);
    assert_eq!(r.unknown[0].raw, hex("1807"));
    assert_eq!(r.raw, &bytes[2..]);
    for (input, operation, unknown) in [
        ("0a041802187f", Some(Operation::DescToClassId), "187f"),
        ("0a02187f", None, "187f"),
        ("0a0418011800", Some(Operation::None), ""),
        ("0a04187f1802", Some(Operation::DescToClassId), "187f"),
        ("0a0418011a00", Some(Operation::InternalToClassId), "1a00"),
    ] {
        let bytes = hex(input);
        let out = decode(&bytes, Limits::default()).unwrap();
        assert_eq!(out.records[0].operation, operation);
        let raw: Vec<_> = out.records[0]
            .unknown
            .iter()
            .flat_map(|f| f.raw)
            .copied()
            .collect();
        assert_eq!(raw, hex(unknown));
    }
    let bytes = hex("0a09100110003201613200");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records[0].predefined_index, Some(0));
    assert_eq!(out.records[0].string, Some(&[][..]));
    assert_eq!(out.records[0].raw, &bytes[2..]);
    let bytes = hex("0a033201ff");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records[0].string, Some(&[0xff][..]));
    assert!(std::ptr::eq(
        out.records[0].string.unwrap().as_ptr(),
        bytes[4..].as_ptr()
    ));
    let bytes = hex("0a0608ffffffff07");
    assert_eq!(
        decode(&bytes, Limits::default()).unwrap().records[0].range,
        Some(i32::MAX)
    );
}

#[test]
fn mixed_packed_unpacked_lists_and_signed_low32_values() {
    for (input, expected) in [
        ("28012a0202032804", vec![1, 2, 3, 4]),
        ("2a0028012a00", vec![1]),
        ("28ffffffff0f", vec![-1]),
        ("28ffffffffffffffffff01", vec![-1]),
        ("288080808010", vec![0]),
        ("288080808008", vec![i32::MIN]),
        ("288000", vec![0]),
        ("2a0cffffffff0f80808080108000", vec![-1, 0, 0]),
        ("2a0affffffffffffffffff01", vec![-1]),
    ] {
        let bytes = hex(input);
        assert_eq!(
            decode(&bytes, Limits::default()).unwrap().local_name,
            expected
        );
    }
    let bytes = hex("0a0a20012202020320042805");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records[0].substring_index, [1, 2, 3, 4]);
    assert_eq!(out.records[0].replace_char, [5]);
    let bytes = hex("0a0828012a0202032804");
    assert_eq!(
        decode(&bytes, Limits::default()).unwrap().records[0].replace_char,
        [1, 2, 3, 4]
    );
    // Unknown enum low32 -1 remains raw, known low32 zero sets explicit NONE.
    let bytes = hex("0a0c18ffffffff0f188080808010");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records[0].operation, Some(Operation::None));
    assert_eq!(
        out.records[0].unknown[0].value,
        Value::Varint(u32::MAX.into())
    );
}

#[test]
fn wrong_wire_fields_unknown_order_and_opaque_payloads() {
    let bytes = hex("08010a093801187f3d010203041200");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(
        out.unknown.iter().map(|f| f.raw).collect::<Vec<_>>(),
        [&bytes[0..2], &bytes[13..15]]
    );
    let r = &out.records[0];
    assert_eq!(r.operation, None);
    assert_eq!(r.unknown.len(), 3);
    let spans = [4..6, 6..8, 8..13];
    for (field, span) in r.unknown.iter().zip(spans) {
        assert_eq!(field.span, span);
        assert_eq!(field.raw, &bytes[span.clone()]);
        assert!(std::ptr::eq(field.raw.as_ptr(), bytes[span].as_ptr()));
    }
    for input in ["0a020a00", "0a040a020b0c"] {
        let bytes = hex(input);
        let out = decode(&bytes, Limits::default()).unwrap();
        assert_eq!(out.records[0].range, None);
        assert_eq!(out.records[0].unknown.len(), 1);
        assert!(matches!(out.records[0].unknown[0].value, Value::Bytes(_)));
    }
    // Every legal wrong wire stays unknown; fixed and opaque forms are not traversed.
    let bytes = hex("0a1c0d010203041100000000000000001a002d0102030430013d05060708");
    let out = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(out.records[0].unknown.len(), 6);
    assert_eq!(out.records[0].range, None);
    assert_eq!(out.records[0].predefined_index, None);
    assert_eq!(out.records[0].operation, None);
    assert_eq!(out.records[0].string, None);
}

#[test]
fn malformed_nested_packed_lengths_varints_groups_and_sibling_boundaries() {
    for (input, kind, offset) in [
        ("0a0108", ErrorKind::Truncated, 3),
        ("0a022280", ErrorKind::Truncated, 4),
        ("0a03220180", ErrorKind::Truncated, 5),
        ("0a020b0c", ErrorKind::UnsupportedGroup, 2),
        ("0a020e00", ErrorKind::InvalidWireType, 2),
        ("0a020000", ErrorKind::InvalidFieldNumber, 2),
        ("2a8080808008", ErrorKind::NegativeLength, 1),
        ("0a01082801", ErrorKind::Truncated, 3),
        ("0a0318011800", ErrorKind::Truncated, 5),
        ("0a022a01", ErrorKind::Truncated, 4),
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
    ] {
        error(&hex(input), Limits::default(), kind, offset);
    }
    let mut bytes = vec![0x2a, 10];
    bytes.extend([0x80; 10]);
    error(&bytes, Limits::default(), ErrorKind::MalformedVarint, 11);
    for n in 1..10 {
        let mut bytes = vec![0x2a, n];
        bytes.extend(vec![0x80; n as usize]);
        error(
            &bytes,
            Limits::default(),
            ErrorKind::Truncated,
            n as usize + 2,
        );
    }
    // A complete prior Record cannot supply the next one's missing value.
    error(
        &hex("0a0208010a0108"),
        Limits::default(),
        ErrorKind::Truncated,
        7,
    );
    let bytes = hex("0a020801");
    assert_eq!(
        decode(&bytes, Limits::default()).unwrap().records[0].range,
        Some(1)
    );
}

#[test]
fn exact_aggregate_budgets_are_shared_across_records_and_packed_entries() {
    let bytes = hex(include_str!("fixtures/metadata_string_table/aggregate.hex"));
    let backing = 2 * size_of::<Record<'_>>() + 4 * size_of::<i32>();
    let limits = Limits {
        bytes: 12,
        records: 7,
        nodes: 3,
        depth: 2,
        work: 34 + backing,
        owned_bytes: backing,
    };
    let out = decode(&bytes, limits).unwrap();
    assert_eq!(out.records.len(), 2);
    assert_eq!(out.records[0].range, Some(2));
    defaults(&out.records[1]);
    assert_eq!(out.local_name, [1, 2, 3]);
    for (limits, label, offset) in [
        (
            Limits {
                bytes: 11,
                ..limits
            },
            "bytes",
            0,
        ),
        (Limits { nodes: 2, ..limits }, "nodes", 6),
        (
            Limits {
                records: 6,
                ..limits
            },
            "records",
            11,
        ),
        (
            Limits {
                work: 33 + backing,
                ..limits
            },
            "work",
            11,
        ),
        (
            Limits {
                owned_bytes: backing - 1,
                ..limits
            },
            "owned_bytes",
            11,
        ),
        (Limits { depth: 0, ..limits }, "depth", 0),
        (Limits { depth: 1, ..limits }, "depth", 2),
    ] {
        error(&bytes, limits, ErrorKind::Limit(label), offset);
    }
    error(
        &[],
        Limits {
            nodes: 0,
            depth: 0,
            work: 0,
            ..Limits::default()
        },
        ErrorKind::Limit("depth"),
        0,
    );
    error(
        &[],
        Limits {
            nodes: 0,
            work: 0,
            ..Limits::default()
        },
        ErrorKind::Limit("nodes"),
        0,
    );
    error(
        &[],
        Limits {
            work: 0,
            ..Limits::default()
        },
        ErrorKind::Limit("work"),
        0,
    );
    assert!(
        decode(
            &[],
            Limits {
                work: 1,
                owned_bytes: 0,
                records: 0,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    for (input, offset) in [("0801", 0), ("0a00", 2)] {
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
    // Unknown enum dispatch is charged although it does not change the recognized value.
    let bytes = hex("0a041801187f");
    let backing = size_of::<Field<'_>>() + size_of::<Record<'_>>();
    assert!(
        decode(
            &bytes,
            Limits {
                work: 18 + backing,
                owned_bytes: backing,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    error(
        &bytes,
        Limits {
            work: 17 + backing,
            ..Limits::default()
        },
        ErrorKind::Limit("work"),
        2,
    );
    // All three packed lists in two Records share the same record budget.
    let bytes = hex("0a04220201020a042a0203042a020506");
    let out = decode(
        &bytes,
        Limits {
            records: 11,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(out.records[0].substring_index, [1, 2]);
    assert_eq!(out.records[1].replace_char, [3, 4]);
    assert_eq!(out.local_name, [5, 6]);
    error(
        &bytes,
        Limits {
            records: 10,
            ..Limits::default()
        },
        ErrorKind::Limit("records"),
        15,
    );
    error(
        &hex("0a03220180"),
        Limits {
            work: 16,
            ..Limits::default()
        },
        ErrorKind::Limit("work"),
        5,
    );
    error(
        &hex("0a03220180"),
        Limits {
            work: 17,
            ..Limits::default()
        },
        ErrorKind::Truncated,
        5,
    );
    // Failed bulk read charges available bytes + failed attempt before truncation.
    error(
        &hex("0a02ff"),
        Limits {
            work: 5,
            ..Limits::default()
        },
        ErrorKind::Limit("work"),
        2,
    );
    error(
        &hex("0a02ff"),
        Limits {
            work: 6,
            ..Limits::default()
        },
        ErrorKind::Truncated,
        3,
    );
}

#[test]
fn hard_input_record_node_limits_cannot_be_raised() {
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
    let mut bytes = Vec::new();
    for _ in 0..ktlsp_binary::MAX_RECORDS {
        bytes.extend([0x28, 0]);
    }
    assert_eq!(
        decode(&bytes, unlimited).unwrap().local_name.len(),
        ktlsp_binary::MAX_RECORDS
    );
    let end = bytes.len();
    bytes.extend([0x28, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), end);
    let mut bytes = Vec::new();
    for _ in 0..ktlsp_binary::MAX_RECORDS - 1 {
        bytes.extend([0x0a, 0]);
    }
    assert_eq!(
        decode(&bytes, unlimited).unwrap().records.len(),
        ktlsp_binary::MAX_RECORDS - 1
    );
    let end = bytes.len();
    bytes.extend([0x0a, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("nodes"), end + 2);
}

#[test]
fn supported_header_and_wire_compose_with_data_only_unit_class() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let ktlsp_binary::metadata::Outcome::Decoded { bytes, .. } =
        ktlsp_binary::metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported header");
    };
    let sections = ktlsp_binary::metadata::wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(
        sections.string_table.raw,
        hex("0a0210020a0210000a0208020a02100e0a00")
    );
    let out = decode(sections.string_table.raw, Limits::default()).unwrap();
    assert_eq!(out.records.len(), 5);
    for (r, (range, index)) in out.records.iter().zip([
        (None, Some(2)),
        (None, Some(0)),
        (Some(2), None),
        (None, Some(14)),
        (None, None),
    ]) {
        assert_eq!(r.range, range);
        assert_eq!(r.predefined_index, index);
        assert_eq!(r.operation, None);
        assert_eq!(r.string, None);
        assert!(r.substring_index.is_empty() && r.replace_char.is_empty() && r.unknown.is_empty());
    }
    assert!(out.local_name.is_empty() && out.unknown.is_empty());
}

#[test]
fn deterministic_hostile_bytes_and_every_byte_nested_truncation() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        nodes: 4,
        work: 256,
        owned_bytes: 1024,
        ..Limits::default()
    };
    for a in 0..=255 {
        for b in 0..=255 {
            let _ = decode(&[0x0a, 2, a, b], limits);
        }
        for n in 0..32 {
            let _ = decode(&vec![a; n], limits);
        }
    }
    let bytes = hex("0a0c080208001801180732002003");
    for n in 1..bytes.len() {
        error(&bytes[..n], Limits::default(), ErrorKind::Truncated, n);
    }
    assert!(decode(&bytes, Limits::default()).is_ok());
}
