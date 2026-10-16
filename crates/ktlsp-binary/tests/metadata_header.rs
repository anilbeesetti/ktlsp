use ktlsp_binary::annotations::{
    Annotation, AnnotationAttribute, DeclarationAnnotations, Element, Value, Visibility,
};
use ktlsp_binary::metadata::{Error, Limits, Outcome, Unsupported, decode};

const DESCRIPTOR: &[u16] = &[
    76, 107, 111, 116, 108, 105, 110, 47, 77, 101, 116, 97, 100, 97, 116, 97, 59,
];
fn name(s: &str) -> &'static [u16] {
    match s {
        "k" => &[107],
        "mv" => &[109, 118],
        "bv" => &[98, 118],
        "d1" => &[100, 49],
        "d2" => &[100, 50],
        "xs" => &[120, 115],
        "pn" => &[112, 110],
        "xi" => &[120, 105],
        "future" => &[102, 117, 116, 117, 114, 101],
        _ => panic!("test field"),
    }
}
fn field<'a>(s: &str, value: Value<'a>) -> Element<'a> {
    Element {
        name: name(s),
        value,
    }
}
fn version() -> Element<'static> {
    field(
        "mv",
        Value::Array(vec![Value::Int(2), Value::Int(3), Value::Int(0)]),
    )
}
fn strings<'a>(values: &[&'a [u16]]) -> Value<'a> {
    Value::Array(values.iter().map(|s| Value::String(s)).collect())
}
fn fixture(elements: Vec<Element<'_>>) -> DeclarationAnnotations<'_> {
    DeclarationAnnotations {
        class: vec![AnnotationAttribute {
            visibility: Visibility::RuntimeVisible,
            annotations: vec![Annotation {
                descriptor: DESCRIPTOR,
                elements,
            }],
        }],
        fields: vec![],
        methods: vec![],
    }
}
fn bytes(parts: &[&[u16]]) -> Vec<u8> {
    let input = fixture(vec![version(), field("d1", strings(parts))]);
    let Outcome::Decoded { bytes, .. } = decode(&input, Limits::default()).unwrap() else {
        panic!("decoded")
    };
    bytes
}

#[test]
fn absent_defaults_presence_and_exact_signed_header() {
    let absent = DeclarationAnnotations {
        class: vec![],
        fields: vec![],
        methods: vec![],
    };
    assert_eq!(decode(&absent, Limits::default()).unwrap(), Outcome::Absent);
    let omitted = fixture(vec![]);
    let Outcome::Unsupported { header, reason } = decode(&omitted, Limits::default()).unwrap()
    else {
        panic!("unsupported")
    };
    assert_eq!(reason, Unsupported::Version);
    assert_eq!((header.present, header.k, header.xi), (0, 1, 0));
    assert!(header.mv.is_empty() && header.d1.is_empty() && header.d2.is_empty());
    assert_eq!(header.bv, [1, 0, 3]);
    assert!(header.xs.is_empty() && header.pn.is_empty());
    let text = [0, 0xd800, 0xdc00, 0xffff];
    let complete = fixture(vec![
        field("k", Value::Int(5)),
        version(),
        field("bv", Value::Array(vec![Value::Int(u32::MAX)])),
        field("d1", strings(&[&[0, 255]])),
        field("d2", strings(&[&text, &[]])),
        field("xs", Value::String(&text)),
        field("pn", Value::String(&text)),
        field("xi", Value::Int(u32::MAX)),
    ]);
    let Outcome::Decoded { header, bytes } = decode(&complete, Limits::default()).unwrap() else {
        panic!("decoded")
    };
    assert_eq!(bytes, [255]);
    assert_eq!((header.present, header.k, header.xi), (255, 5, -1));
    assert_eq!(header.bv, [-1]);
    assert_eq!(header.d2, [&text[..], &[]]);
    assert_eq!(header.xs, text);
    assert_eq!(header.pn, text);
    assert!(std::ptr::eq(header.raw, &complete.class[0].annotations[0]));
    assert!(std::ptr::eq(header.xs.as_ptr(), text.as_ptr()));
}

#[test]
fn supported_kinds_and_explicit_unsupported_preserve_input() {
    for k in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, 6, i32::MAX] {
        let input = fixture(vec![
            version(),
            field("k", Value::Int(k as u32)),
            field("d1", strings(&[&[0, 10]])),
        ]);
        let result = decode(&input, Limits::default()).unwrap();
        if [1, 2, 5].contains(&k) {
            assert!(matches!(result, Outcome::Decoded { bytes, .. } if bytes == [10]));
        } else {
            assert!(
                matches!(result, Outcome::Unsupported { header, reason: Unsupported::Kind } if header.k == k)
            );
        }
    }
    for mv in [
        vec![],
        vec![2, 3],
        vec![2, 3, 0, 0],
        vec![1, 9, 0],
        vec![2, 4, 0],
        vec![u32::MAX, 3, 0],
    ] {
        let input = fixture(vec![field(
            "mv",
            Value::Array(mv.iter().copied().map(Value::Int).collect()),
        )]);
        assert!(
            matches!(decode(&input, Limits::default()).unwrap(), Outcome::Unsupported { header, reason: Unsupported::Version } if header.mv == mv.iter().map(|n| *n as i32).collect::<Vec<_>>())
        );
    }
    let unknown = fixture(vec![
        version(),
        field(
            "future",
            Value::Array(vec![Value::Annotation(Annotation {
                descriptor: DESCRIPTOR,
                elements: vec![],
            })]),
        ),
    ]);
    assert!(
        matches!(decode(&unknown, Limits::default()).unwrap(), Outcome::Unsupported { header, reason: Unsupported::Field } if header.raw.elements.len() == 2)
    );
}

#[test]
fn independent_byte_vectors_markers_multipart_and_unicode() {
    assert!(bytes(&[]).is_empty());
    assert!(bytes(&[&[]]).is_empty());
    assert!(bytes(&[&[0]]).is_empty());
    assert!(bytes(&[&[0xffff]]).is_empty());
    assert_eq!(
        bytes(&[&[0, 0, 1, 127, 128, 255, 256, 0xd800, 0xdcff, 0xffff]]),
        [0, 1, 127, 128, 255, 0, 0, 255, 255]
    );
    assert_eq!(bytes(&[&[0, 0], &[], &[1, 255]]), [0, 1, 255]);
    // Seven independent bytes 00 ff 80 01 7f 55 aa, split into LSB-first 7-bit chunks,
    // incremented modulo128: 01 7f 04 0d 71 30 16 56.
    let legacy = [1, 127, 4, 13, 113, 48, 22, 86];
    let expected = [0, 255, 128, 1, 127, 85, 170];
    assert_eq!(bytes(&[&legacy]), expected);
    let mut marked = vec![0xffff];
    marked.extend(legacy);
    assert_eq!(bytes(&[&marked]), expected);
    for split in 0..=legacy.len() {
        assert_eq!(bytes(&[&legacy[..split], &[], &legacy[split..]]), expected);
    }
    // Marker detection does not skip an empty first chunk: ffff becomes legacy 126.
    assert_eq!(bytes(&[&[], &[0xffff, 1]]), [126]);
    assert_eq!(bytes(&[&[], &[0, 1]]), [127]);
    assert_eq!(bytes(&[&[0xffff, 0x101, 0xd8ff]]), [0]);
    assert_eq!(bytes(&[&[0xffff, 0, 0xffff]]), [127]);
}

#[test]
fn legacy_truncation_discards_incomplete_bits_like_upstream() {
    let encoded = [1, 127, 4, 13, 113, 48, 22, 86];
    let expected = [0, 255, 128, 1, 127, 85, 170];
    for n in 0..=8 {
        assert_eq!(bytes(&[&encoded[..n]]), expected[..n * 7 / 8]);
    }
    // Two chunks encode one byte; the other six bits do not affect that byte.
    assert_eq!(bytes(&[&[1, 127]]), [0]);
    assert_eq!(bytes(&[&[1, 1]]), [0]);
}

#[test]
fn duplicates_wrong_types_and_contexts_are_malformed() {
    for s in ["k", "mv", "bv", "d1", "d2", "xs", "pn", "xi", "future"] {
        let value = || match s {
            "mv" | "bv" => Value::Array(vec![Value::Int(1)]),
            "d1" | "d2" => strings(&[&[]]),
            "xs" | "pn" => Value::String(&[]),
            _ => Value::Int(1),
        };
        let duplicate = fixture(vec![field(s, value()), field(s, value())]);
        assert_eq!(
            decode(&duplicate, Limits::default()).unwrap_err(),
            Error::Malformed("duplicate metadata field")
        );
    }
    let duplicate = fixture(vec![version(), version()]);
    assert_eq!(
        decode(&duplicate, Limits::default()).unwrap_err(),
        Error::Malformed("duplicate metadata field")
    );
    for s in ["k", "mv", "bv", "d1", "d2", "xs", "pn", "xi"] {
        for value in [
            Value::Byte(1),
            Value::Long(1),
            Value::String(&[]),
            Value::Int(1),
            Value::Array(vec![]),
            Value::Array(vec![Value::Int(1)]),
            Value::Array(vec![Value::String(&[])]),
        ] {
            let valid = matches!(
                (s, &value),
                ("k" | "xi", Value::Int(_)) | ("xs" | "pn", Value::String(_))
            ) || matches!((s, &value), ("mv" | "bv", Value::Array(v)) if v.iter().all(|v| matches!(v, Value::Int(_))))
                || matches!((s, &value), ("d1" | "d2", Value::Array(v)) if v.iter().all(|v| matches!(v, Value::String(_))));
            let input = fixture(vec![field(s, value)]);
            assert_eq!(decode(&input, Limits::default()).is_ok(), valid, "{s}");
        }
    }
    let mut duplicate = fixture(vec![version()]);
    duplicate.class.push(AnnotationAttribute {
        visibility: Visibility::RuntimeInvisible,
        annotations: vec![Annotation {
            descriptor: DESCRIPTOR,
            elements: vec![],
        }],
    });
    assert_eq!(
        decode(&duplicate, Limits::default()).unwrap_err(),
        Error::Malformed("duplicate metadata annotation")
    );
    let mut invisible = fixture(vec![version()]);
    invisible.class[0].visibility = Visibility::RuntimeInvisible;
    assert!(
        matches!(decode(&invisible, Limits::default()).unwrap(), Outcome::Decoded { header, .. } if *header.visibility == Visibility::RuntimeInvisible)
    );
    for method in [false, true] {
        let mut member = fixture(vec![version()]);
        let attrs = std::mem::take(&mut member.class);
        if method {
            member.methods.push(attrs);
        } else {
            member.fields.push(attrs);
        }
        assert_eq!(
            decode(&member, Limits::default()).unwrap_err(),
            Error::Malformed("metadata declaration context")
        );
    }
    let mut nested = fixture(vec![field(
        "future",
        Value::Annotation(Annotation {
            descriptor: DESCRIPTOR,
            elements: vec![version()],
        }),
    )]);
    nested.class[0].annotations[0].descriptor = &[76, 65, 59];
    assert_eq!(decode(&nested, Limits::default()).unwrap(), Outcome::Absent);
}

#[test]
fn aggregate_and_zero_budgets_preflight_allocation_and_work() {
    let data = [0, 10, 20, 30];
    let input = fixture(vec![
        version(),
        field("d1", strings(&[&data])),
        field("d2", strings(&[&[0, 0xd800], &[0, 0xd800]])),
    ]);
    let limits = Limits::default();
    assert_eq!(
        decode(
            &input,
            Limits {
                output_bytes: 2,
                ..limits
            }
        )
        .unwrap_err(),
        Error::Limit("metadata output bytes")
    );
    assert!(
        decode(
            &input,
            Limits {
                output_bytes: 3,
                ..limits
            }
        )
        .is_ok()
    );
    assert_eq!(
        decode(&input, Limits { work: 57, ..limits }).unwrap_err(),
        Error::Limit("metadata work")
    );
    assert!(decode(&input, Limits { work: 58, ..limits }).is_ok());
    for limits in [
        Limits {
            fields: 0,
            ..limits
        },
        Limits {
            code_units: 0,
            ..limits
        },
        Limits { work: 0, ..limits },
    ] {
        assert!(matches!(decode(&input, limits), Err(Error::Limit(_))));
    }
    // descriptor17 + names2+2+2 + d1 units4 + repeated d2 units4 =31 units.
    assert!(
        decode(
            &input,
            Limits {
                code_units: 31,
                ..limits
            }
        )
        .is_ok()
    );
    assert_eq!(
        decode(
            &input,
            Limits {
                code_units: 30,
                ..limits
            }
        )
        .unwrap_err(),
        Error::Limit("metadata code units")
    );
    let empties = fixture(vec![
        version(),
        field(
            "d1",
            Value::Array((0..100).map(|_| Value::String(&[])).collect()),
        ),
    ]);
    assert_eq!(
        decode(
            &empties,
            Limits {
                fields: 106,
                ..limits
            }
        )
        .unwrap_err(),
        Error::Limit("metadata fields")
    );
    assert!(
        decode(
            &empties,
            Limits {
                fields: 107,
                ..limits
            }
        )
        .is_ok()
    );
    let mut unrelated = fixture(vec![]);
    unrelated.class[0].annotations[0].descriptor = &[76, 65, 59];
    unrelated.methods = (0..10).map(|_| vec![]).collect();
    assert_eq!(
        decode(
            &unrelated,
            Limits {
                fields: 11,
                ..limits
            }
        )
        .unwrap_err(),
        Error::Limit("metadata fields")
    );
    assert_eq!(
        decode(
            &unrelated,
            Limits {
                fields: 12,
                ..limits
            }
        )
        .unwrap(),
        Outcome::Absent
    );
    let excessive = fixture(vec![
        version(),
        field(
            "d2",
            Value::Array(
                (0..=ktlsp_binary::MAX_RECORDS)
                    .map(|_| Value::String(&[]))
                    .collect(),
            ),
        ),
    ]);
    assert_eq!(
        decode(
            &excessive,
            Limits {
                fields: usize::MAX,
                ..limits
            }
        )
        .unwrap_err(),
        Error::Limit("metadata fields")
    );
}

#[test]
fn precompiled_unit_has_independent_exact_payload_and_preserved_names() {
    let class = ktlsp_binary::parse_class(include_bytes!("../fixtures/t017c2/Unit.class")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let Outcome::Decoded { header, bytes } = decode(&annotations, Limits::default()).unwrap()
    else {
        panic!("decoded")
    };
    // Independently transcribed from the fixture constant-pool d1 MUTF8, not encoder output.
    assert_eq!(
        bytes,
        [
            18, 10, 2, 16, 2, 10, 2, 16, 0, 10, 2, 8, 2, 10, 2, 16, 14, 10, 0, 8, 198, 2, 24, 0,
            50, 2, 48, 1, 66, 7, 8, 66, 162, 6, 2, 8, 2, 74, 10, 16, 3, 26, 2, 48, 4, 72, 150, 128,
            4, 168, 6, 5
        ]
    );
    assert_eq!(header.mv, [2, 3, 0]);
    assert_eq!(header.xi, 48);
    assert_eq!(header.d2[2], [60, 105, 110, 105, 116, 62]);
    assert_eq!(header.d2[3], [116, 111, 83, 116, 114, 105, 110, 103]);
    assert_eq!(header.d1[0][0], 0);
}

#[test]
fn deterministic_forged_units_and_multipart_inputs_never_panic() {
    for n in 0..32 {
        for unit in [0, 1, 127, 128, 255, 256, 0xd800, 0xffff] {
            let mut parts = vec![unit; n];
            for split in 0..=n {
                let _ = bytes(&[&parts[..split], &[], &parts[split..]]);
            }
            parts.insert(0, 0);
            assert_eq!(bytes(&[&parts]), vec![unit as u8; n]);
        }
    }
}
