use ktlsp_binary::{
    MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS,
    metadata::{
        self, constructor,
        jvm_method_signature::{
            ErrorKind, Limits, MethodSignatureExtension, SignatureBody, decode,
        },
        name_resolver, payload, wire,
    },
};

const S: usize = size_of::<SignatureBody<'_>>();
const SELECTED: &str = include_str!("fixtures/metadata_jvm_method_signature/selected.hex");
const MERGED: &str = include_str!("fixtures/metadata_jvm_method_signature/merged.hex");
const PAIR: &str = include_str!("fixtures/metadata_jvm_method_signature/pair_parent.hex");

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|word| u8::from_str_radix(word, 16).unwrap())
        .collect()
}
fn check(
    out: &MethodSignatureExtension<'_>,
    bytes: &[u8],
    signature: Option<(Option<i32>, Option<i32>)>,
    spans: &[(usize, usize)],
) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(out.signature.as_ref().map(|s| (s.name, s.desc)), signature);
    assert_eq!(out.bodies.len(), spans.len());
    for (body, &(start, end)) in out.bodies.iter().zip(spans) {
        assert_eq!(body.span, start..end);
        assert_eq!(body.raw, &bytes[start..end]);
        assert_eq!(body.raw.as_ptr(), bytes[start..end].as_ptr());
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    let e = decode(bytes, limits).unwrap_err();
    assert_eq!((e.kind, e.offset), (kind, offset), "{bytes:02x?}");
}

#[test]
fn frozen_values_presence_merge_raw_spans_and_exact_budgets() {
    for (text, signature, spans, records, work, owned) in [
        ("", None, vec![], 0, 0, 0),
        ("a2 06 00", Some((None, None)), vec![(3, 3)], 1, 5 + S, S),
        (
            "a2 06 04 08 00 10 00",
            Some((Some(0), Some(0))),
            vec![(3, 7)],
            3,
            17 + S,
            S,
        ),
        (
            SELECTED,
            Some((Some(7), Some(8))),
            vec![(3, 7)],
            3,
            17 + S,
            S,
        ),
        (
            MERGED,
            Some((Some(7), Some(8))),
            vec![(3, 5), (8, 10), (13, 13)],
            5,
            27 + 4 * S,
            4 * S,
        ),
        (
            "a2 06 06 08 01 08 02 10 03 a2 06 02 08 04",
            Some((Some(4), Some(3))),
            vec![(3, 9), (12, 14)],
            6,
            34 + 2 * S,
            2 * S,
        ),
        ("a0 06 07", None, vec![], 1, 5, 0),
        (
            "a2 06 04 0a 00 10 00",
            Some((None, Some(0))),
            vec![(3, 7)],
            3,
            17 + S,
            S,
        ),
        (
            "a2 06 06 08 07 10 08 18 63",
            Some((Some(7), Some(8))),
            vec![(3, 9)],
            4,
            23 + S,
            S,
        ),
        (
            "a2 06 04 08 07 12 00 a0 06 01",
            Some((Some(7), None)),
            vec![(3, 7)],
            4,
            22 + S,
            S,
        ),
        ("aa 06 01 00", None, vec![], 1, 6, 0),
        (
            "a2 06 06 08 ff ff ff ff 0f",
            Some((Some(-1), None)),
            vec![(3, 9)],
            2,
            19 + S,
            S,
        ),
        (
            "a2 06 06 08 80 80 80 80 08",
            Some((Some(i32::MIN), None)),
            vec![(3, 9)],
            2,
            19 + S,
            S,
        ),
        (
            "a2 06 06 08 80 80 80 80 10",
            Some((Some(0), None)),
            vec![(3, 9)],
            2,
            19 + S,
            S,
        ),
        (
            "a2 06 03 08 87 00",
            Some((Some(7), None)),
            vec![(3, 6)],
            2,
            13 + S,
            S,
        ),
        (
            "a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00",
            Some((None, None)),
            vec![(3, 3), (6, 6), (9, 9), (12, 12), (15, 15)],
            5,
            25 + 8 * S,
            8 * S,
        ),
        (
            "a2 06 02 0a 00",
            Some((None, None)),
            vec![(3, 5)],
            2,
            11 + S,
            S,
        ),
        (
            "a2 06 03 1a 01 00",
            Some((None, None)),
            vec![(3, 6)],
            2,
            13 + S,
            S,
        ),
        (
            "a2 06 00 10 00",
            Some((None, None)),
            vec![(3, 3)],
            2,
            9 + S,
            S,
        ),
        (
            "a2 06 02 08 07 a2 06 02 0a 00",
            Some((Some(7), None)),
            vec![(3, 5), (8, 10)],
            4,
            22 + 2 * S,
            2 * S,
        ),
        (
            "a2 86 00 84 00 08 07 10 08",
            Some((Some(7), Some(8))),
            vec![(5, 9)],
            3,
            19 + S,
            S,
        ),
        (
            "a2 86 80 80 10 04 08 07 10 08",
            Some((Some(7), Some(8))),
            vec![(6, 10)],
            3,
            20 + S,
            S,
        ),
        (
            "a2 06 0b 08 ff ff ff ff ff ff ff ff ff 01",
            Some((Some(-1), None)),
            vec![(3, 14)],
            2,
            29 + S,
            S,
        ),
        (PAIR, Some((Some(7), Some(8))), vec![(21, 25)], 6, 41 + S, S),
    ] {
        let bytes = hex(text);
        let exact = Limits {
            bytes: bytes.len(),
            records,
            work,
            owned_bytes: owned,
        };
        for limits in [
            exact,
            Limits {
                bytes: usize::MAX,
                records: usize::MAX,
                work: usize::MAX,
                owned_bytes: usize::MAX,
            },
        ] {
            check(&decode(&bytes, limits).unwrap(), &bytes, signature, &spans);
        }
        if !bytes.is_empty() {
            error(
                &bytes,
                Limits {
                    bytes: bytes.len() - 1,
                    ..exact
                },
                ErrorKind::Limit("bytes"),
                0,
            );
        }
    }
}

#[test]
fn every_prefix_of_selected_merged_empty_and_genuine_parent() {
    for (text, boundaries) in [
        (
            SELECTED,
            vec![
                (0, None, vec![]),
                (7, Some((Some(7), Some(8))), vec![(3, 7)]),
            ],
        ),
        (
            MERGED,
            vec![
                (0, None, vec![]),
                (5, Some((Some(7), None)), vec![(3, 5)]),
                (10, Some((Some(7), Some(8))), vec![(3, 5), (8, 10)]),
                (
                    13,
                    Some((Some(7), Some(8))),
                    vec![(3, 5), (8, 10), (13, 13)],
                ),
            ],
        ),
        (
            PAIR,
            vec![
                (0, None, vec![]),
                (2, None, vec![]),
                (10, None, vec![]),
                (18, None, vec![]),
                (25, Some((Some(7), Some(8))), vec![(21, 25)]),
            ],
        ),
        (
            "a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00",
            vec![
                (0, None, vec![]),
                (3, Some((None, None)), vec![(3, 3)]),
                (6, Some((None, None)), vec![(3, 3), (6, 6)]),
                (9, Some((None, None)), vec![(3, 3), (6, 6), (9, 9)]),
                (
                    12,
                    Some((None, None)),
                    vec![(3, 3), (6, 6), (9, 9), (12, 12)],
                ),
                (
                    15,
                    Some((None, None)),
                    vec![(3, 3), (6, 6), (9, 9), (12, 12), (15, 15)],
                ),
            ],
        ),
    ] {
        let bytes = hex(text);
        for cut in 0..=bytes.len() {
            let prefix = &bytes[..cut];
            if let Some((_, signature, spans)) = boundaries.iter().find(|(end, _, _)| *end == cut) {
                check(
                    &decode(prefix, Limits::default()).unwrap(),
                    prefix,
                    *signature,
                    spans,
                );
            } else {
                error(prefix, Limits::default(), ErrorKind::Truncated, cut);
            }
        }
    }
}

#[test]
fn malformed_parent_and_children_fail_whole_call_with_parent_offsets() {
    for (text, kind, offset) in [
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("a2 06 01 00", ErrorKind::InvalidFieldNumber, 3),
        ("a2 06 01 08", ErrorKind::Truncated, 4),
        ("a2 06 02 08 80", ErrorKind::Truncated, 5),
        ("a2 06 01 0e", ErrorKind::InvalidWireType, 3),
        ("a2 06 01 0b", ErrorKind::UnsupportedGroup, 3),
        ("a2 06 03 0a 02 00 10 01", ErrorKind::Truncated, 6),
        ("a2 06 00 00", ErrorKind::InvalidFieldNumber, 3),
        ("a2 06 02 08 07 a2 06 01 08", ErrorKind::Truncated, 9),
        ("a2 06 ff ff ff ff 0f", ErrorKind::NegativeLength, 2),
        ("a2 06 06 0a ff ff ff ff 0f", ErrorKind::NegativeLength, 4),
        (
            "a2 06 0b 08 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            13,
        ),
    ] {
        error(&hex(text), Limits::default(), kind, offset);
    }
}

#[test]
fn exact_shared_resource_thresholds_and_error_precedence() {
    let selected = hex(SELECTED);
    for (limits, label, offset) in [
        (
            Limits {
                bytes: 6,
                ..Limits::default()
            },
            "bytes",
            0,
        ),
        (
            Limits {
                records: 0,
                ..Limits::default()
            },
            "records",
            0,
        ),
        (
            Limits {
                records: 1,
                ..Limits::default()
            },
            "records",
            3,
        ),
        (
            Limits {
                records: 2,
                ..Limits::default()
            },
            "records",
            5,
        ),
        (
            Limits {
                work: 0,
                ..Limits::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                work: 1,
                ..Limits::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                work: 3,
                ..Limits::default()
            },
            "work",
            2,
        ),
        (
            Limits {
                work: 7,
                ..Limits::default()
            },
            "work",
            3,
        ),
        (
            Limits {
                work: 8,
                ..Limits::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                work: 9,
                ..Limits::default()
            },
            "work",
            3,
        ),
        (
            Limits {
                work: 12,
                ..Limits::default()
            },
            "work",
            3,
        ),
        (
            Limits {
                work: 13,
                ..Limits::default()
            },
            "work",
            5,
        ),
        (
            Limits {
                work: 16,
                ..Limits::default()
            },
            "work",
            5,
        ),
        (
            Limits {
                work: 17 + S - 1,
                ..Limits::default()
            },
            "work",
            0,
        ),
        (
            Limits {
                owned_bytes: S - 1,
                ..Limits::default()
            },
            "owned_bytes",
            0,
        ),
        (
            Limits {
                work: 17,
                owned_bytes: S - 1,
                ..Limits::default()
            },
            "owned_bytes",
            0,
        ),
    ] {
        error(&selected, limits, ErrorKind::Limit(label), offset);
    }
    for (text, limits, label, offset) in [
        (
            MERGED,
            Limits {
                records: 4,
                ..Limits::default()
            },
            "records",
            10,
        ),
        (
            MERGED,
            Limits {
                owned_bytes: 2 * S,
                ..Limits::default()
            },
            "owned_bytes",
            10,
        ),
        (
            MERGED,
            Limits {
                owned_bytes: 4 * S - 1,
                ..Limits::default()
            },
            "owned_bytes",
            10,
        ),
        (
            MERGED,
            Limits {
                work: 27 + 4 * S - 1,
                ..Limits::default()
            },
            "work",
            10,
        ),
        (
            "a2 06 06 08 01 08 02 10 03 a2 06 02 08 04",
            Limits {
                records: 5,
                ..Limits::default()
            },
            "records",
            12,
        ),
        (
            "a2 06 06 08 01 08 02 10 03 a2 06 02 08 04",
            Limits {
                work: 34 + 2 * S - 1,
                ..Limits::default()
            },
            "work",
            9,
        ),
        (
            "a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00",
            Limits {
                records: 4,
                ..Limits::default()
            },
            "records",
            12,
        ),
        (
            "a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00",
            Limits {
                owned_bytes: 4 * S,
                ..Limits::default()
            },
            "owned_bytes",
            12,
        ),
        (
            "a2 06 00 a2 06 00 a2 06 00 a2 06 00 a2 06 00",
            Limits {
                work: 25 + 8 * S - 1,
                ..Limits::default()
            },
            "work",
            12,
        ),
        (
            "a2 06 02 08 07 a2 06 02 0a 00",
            Limits {
                records: 3,
                ..Limits::default()
            },
            "records",
            8,
        ),
        (
            "a2 06 02 08 07 a2 06 02 0a 00",
            Limits {
                owned_bytes: 2 * S - 1,
                ..Limits::default()
            },
            "owned_bytes",
            5,
        ),
        (
            "a2 06 02 08 07 a2 06 02 0a 00",
            Limits {
                work: 22 + 2 * S - 1,
                ..Limits::default()
            },
            "work",
            5,
        ),
        (
            PAIR,
            Limits {
                bytes: 24,
                ..Limits::default()
            },
            "bytes",
            0,
        ),
        (
            PAIR,
            Limits {
                records: 5,
                ..Limits::default()
            },
            "records",
            23,
        ),
        (
            PAIR,
            Limits {
                work: 41 + S - 1,
                ..Limits::default()
            },
            "work",
            18,
        ),
        (
            PAIR,
            Limits {
                owned_bytes: S - 1,
                ..Limits::default()
            },
            "owned_bytes",
            18,
        ),
        (
            "a2 06 01 00",
            Limits {
                bytes: 3,
                ..Limits::default()
            },
            "bytes",
            0,
        ),
        (
            "a2 06 01 00",
            Limits {
                records: 1,
                ..Limits::default()
            },
            "records",
            3,
        ),
        (
            "a2 06 01 08",
            Limits {
                work: 7,
                ..Limits::default()
            },
            "work",
            3,
        ),
        (
            "a2 06 01 08",
            Limits {
                work: 8,
                ..Limits::default()
            },
            "work",
            4,
        ),
    ] {
        error(&hex(text), limits, ErrorKind::Limit(label), offset);
    }
    error(
        &hex("a2 06 01 00"),
        Limits {
            owned_bytes: 0,
            ..Limits::default()
        },
        ErrorKind::InvalidFieldNumber,
        3,
    );
    error(
        &hex("a2 06 01 08"),
        Limits {
            work: 9,
            ..Limits::default()
        },
        ErrorKind::Truncated,
        4,
    );
}

#[test]
fn caller_limits_cannot_raise_hard_byte_or_record_caps() {
    let defaults = Limits::default();
    assert_eq!(
        (
            defaults.bytes,
            defaults.records,
            defaults.work,
            defaults.owned_bytes
        ),
        (MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, MAX_BYTES)
    );
    let raised = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    error(
        &vec![0; MAX_BYTES + 1],
        raised,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [0xa2, 6, 0].repeat(65_535);
    for limits in [
        raised,
        Limits {
            bytes: 196_605,
            records: 65_535,
            work: 327_675 + 65_536 * S,
            owned_bytes: 65_536 * S,
        },
    ] {
        let out = decode(&bytes, limits).unwrap();
        assert_eq!(
            out.signature.as_ref().map(|s| (s.name, s.desc)),
            Some((None, None))
        );
        assert_eq!(out.bodies.len(), 65_535);
        assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
        for (i, body) in out.bodies.iter().enumerate() {
            assert_eq!(body.span, 3 * i + 3..3 * i + 3);
            assert!(body.raw.is_empty());
            assert_eq!(body.raw.as_ptr(), bytes[body.span.clone()].as_ptr());
        }
    }
    bytes.extend([0xa2, 6, 0]);
    error(&bytes, raised, ErrorKind::Limit("records"), 196_605);
}

#[test]
fn genuine_explicit_pair_signature_resolves_to_encoded_method_descriptor() {
    let class_bytes = include_bytes!("fixtures/metadata_constructor/Pair.class.bin");
    let class = ktlsp_binary::parse_class(class_bytes).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata")
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let typed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(typed.string_table_span, 1..47);
    assert_eq!(typed.payload_span, 47..285);
    let flat = wire::decode(&bytes, Default::default()).unwrap();
    let constructors: Vec<_> = flat
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 8 && matches!(f.value, wire::Value::Bytes(_)))
        .collect();
    assert_eq!(constructors.len(), 1);
    let field = constructors[0];
    assert_eq!(field.span, 76..103);
    let wire::Value::Bytes(parent) = field.value else {
        unreachable!()
    };
    assert_eq!(parent, hex(PAIR));
    assert_eq!(parent.as_ptr(), bytes[78..103].as_ptr());
    let original = constructor::decode(parent, Default::default()).unwrap();
    assert_eq!(original.flags, Some(70));
    assert_eq!(original.raw.as_ptr(), parent.as_ptr());
    assert_eq!(
        original
            .value_parameters
            .iter()
            .map(|p| p.span.clone())
            .collect::<Vec<_>>(),
        vec![4..10, 12..18]
    );
    let out = decode(
        parent,
        Limits {
            bytes: 25,
            records: 6,
            work: 41 + S,
            owned_bytes: S,
        },
    )
    .unwrap();
    check(&out, parent, Some((Some(7), Some(8))), &[(21, 25)]);
    assert_eq!(out.bodies[0].raw, hex("08 07 10 08"));
    assert_eq!(out.bodies[0].raw.as_ptr(), bytes[99..103].as_ptr());
    let signature = out.signature.as_ref().unwrap();
    let mut resolver =
        name_resolver::Resolver::new(&typed.string_table, &header.d2, Default::default()).unwrap();
    let name = resolver.get_string(signature.name.unwrap()).unwrap();
    let desc = resolver.get_string(signature.desc.unwrap()).unwrap();
    assert_eq!(name, "<init>".encode_utf16().collect::<Vec<_>>());
    assert_eq!(
        desc,
        "(Ljava/lang/Object;Ljava/lang/Object;)V"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    let matched: Vec<_> = class
        .methods
        .iter()
        .filter(|m| {
            class.utf16(m.name).unwrap() == name && class.utf16(m.descriptor).unwrap() == desc
        })
        .collect();
    assert_eq!(matched.len(), 1);
    assert_eq!((matched[0].name, matched[0].descriptor), (8, 9));
    assert_eq!(&class_bytes[1925..1931], &[0, 1, 0, 8, 0, 9]);
    assert_eq!(&class_bytes[163..169], b"<init>");
    assert_eq!(
        &class_bytes[172..211],
        b"(Ljava/lang/Object;Ljava/lang/Object;)V"
    );
    assert_eq!(class.utf16(matched[0].name).unwrap(), name);
    assert_eq!(class.utf16(matched[0].descriptor).unwrap(), desc);
}

#[test]
fn deterministic_bounded_hostile_bytes() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
        owned_bytes: 1024,
    };
    let check = |bytes: &[u8]| {
        let out = decode(bytes, limits);
        assert_eq!(out, decode(bytes, limits));
        match out {
            Ok(out) => {
                assert_eq!(out.raw, bytes);
                assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
                assert_eq!(out.signature.is_none(), out.bodies.is_empty());
                for body in out.bodies {
                    assert!(body.span.start <= body.span.end && body.span.end <= bytes.len());
                    assert_eq!(body.raw, &bytes[body.span.clone()]);
                    assert_eq!(body.raw.as_ptr(), bytes[body.span].as_ptr());
                }
            }
            Err(e) => assert!(e.offset <= bytes.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            check(&[a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
