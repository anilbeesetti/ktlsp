use ktlsp_binary::metadata::{
    self,
    constructor::{Constructor, Error, ErrorKind, Limits, ValueParameterBody, decode},
    value_parameter,
    wire::{self, Value},
};
use std::mem::size_of;
const S: usize = size_of::<ValueParameterBody<'_>>();
const I: usize = size_of::<i32>();
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn check(
    out: &Constructor<'_>,
    bytes: &[u8],
    flags: Option<i32>,
    spans: &[(usize, usize)],
    ids: &[i32],
) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(out.flags, flags);
    assert_eq!(out.version_requirements, ids);
    assert_eq!(out.value_parameters.len(), spans.len());
    for (body, &(start, end)) in out.value_parameters.iter().zip(spans) {
        assert_eq!(body.span, start..end);
        assert_eq!(body.raw, &bytes[start..end]);
        assert_eq!(body.raw.as_ptr(), bytes[start..end].as_ptr());
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn values(text: &str, flags: Option<i32>, ids: &[i32]) {
    let bytes = hex(text);
    check(
        &decode(&bytes, Limits::default()).unwrap(),
        &bytes,
        flags,
        &[],
        ids,
    );
}
#[test]
fn frozen_fixtures_and_exact_limits() {
    for (text, flags, spans, ids, records, work, owned, last, growth) in [
        (
            include_str!("fixtures/metadata_constructor/selected.hex"),
            Some(0),
            vec![(4, 6), (16, 16)],
            vec![1, 2, 3, 4],
            8,
            39 + 2 * S + 4 * I,
            2 * S + 4 * I,
            16,
            14,
        ),
        (
            include_str!("fixtures/metadata_constructor/duplicates.hex"),
            Some(0),
            vec![(4, 5), (15, 16)],
            vec![0, 127, 128],
            10,
            48 + 2 * S + 4 * I,
            2 * S + 4 * I,
            22,
            22,
        ),
        (
            include_str!("fixtures/metadata_constructor/opaque.hex"),
            None,
            vec![],
            vec![],
            6,
            34,
            0,
            18,
            0,
        ),
        (
            include_str!("fixtures/metadata_constructor/pair_constructor.hex"),
            Some(70),
            vec![(4, 10), (12, 18)],
            vec![],
            4,
            33 + 2 * S,
            2 * S,
            18,
            10,
        ),
    ] {
        let bytes = hex(text);
        for extra in [0, 1] {
            let limits = Limits {
                bytes: bytes.len() + extra,
                records: records + extra,
                work: work + extra,
                owned_bytes: owned + extra,
            };
            check(
                &decode(&bytes, limits).unwrap(),
                &bytes,
                flags,
                &spans,
                &ids,
            );
        }
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
        if owned > 0 {
            error(
                &bytes,
                Limits {
                    owned_bytes: owned - 1,
                    ..Limits::default()
                },
                ErrorKind::Limit("owned_bytes"),
                growth,
            );
        }
    }
    check(
        &decode(
            &[],
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
        )
        .unwrap(),
        &[],
        None,
        &[],
        &[],
    );
}
#[test]
fn all_ninety_four_frozen_prefix_cuts() {
    let mut cuts = 0;
    for (text, spans, ids, boundaries) in [
        (
            include_str!("fixtures/metadata_constructor/selected.hex"),
            vec![(4, 6), (16, 16)],
            vec![1, 2, 3, 4],
            vec![
                (0, None, 0, 0),
                (2, Some(0), 0, 0),
                (6, Some(0), 1, 0),
                (9, Some(0), 1, 1),
                (14, Some(0), 1, 3),
                (16, Some(0), 2, 3),
                (19, Some(0), 2, 4),
            ],
        ),
        (
            include_str!("fixtures/metadata_constructor/duplicates.hex"),
            vec![(4, 5), (15, 16)],
            vec![0, 127, 128],
            vec![
                (0, None, 0, 0),
                (2, Some(1), 0, 0),
                (5, Some(1), 1, 0),
                (8, Some(1), 1, 1),
                (10, Some(6), 1, 1),
                (13, Some(6), 1, 1),
                (16, Some(6), 2, 1),
                (18, Some(0), 2, 1),
                (24, Some(0), 2, 3),
            ],
        ),
        (
            include_str!("fixtures/metadata_constructor/opaque.hex"),
            vec![],
            vec![],
            vec![
                (0, None, 0, 0),
                (3, None, 0, 0),
                (5, None, 0, 0),
                (11, None, 0, 0),
                (14, None, 0, 0),
                (18, None, 0, 0),
                (22, None, 0, 0),
            ],
        ),
        (
            include_str!("fixtures/metadata_constructor/pair_constructor.hex"),
            vec![(4, 10), (12, 18)],
            vec![],
            vec![
                (0, None, 0, 0),
                (2, Some(70), 0, 0),
                (10, Some(70), 1, 0),
                (18, Some(70), 2, 0),
                (25, Some(70), 2, 0),
            ],
        ),
    ] {
        let bytes = hex(text);
        for n in 0..=bytes.len() {
            cuts += 1;
            if let Some(&(_, flags, p, v)) = boundaries.iter().find(|&&(end, _, _, _)| end == n) {
                check(
                    &decode(&bytes[..n], Limits::default()).unwrap(),
                    &bytes[..n],
                    flags,
                    &spans[..p],
                    &ids[..v],
                );
            } else {
                error(&bytes[..n], Limits::default(), ErrorKind::Truncated, n);
            }
        }
    }
    assert_eq!(cuts, 94);
}
#[test]
fn signed_low32_presence_and_mixed_packed_order() {
    values("", None, &[]);
    for (text, value) in [
        ("00", 0),
        ("06", 6),
        ("ac 02", 300),
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("80 80 80 80 08", i32::MIN),
        ("ff ff ff ff 07", i32::MAX),
        ("80 00", 0),
        ("80 80 80 80 10", 0),
    ] {
        values(&format!("08 {text}"), Some(value), &[]);
        values(&format!("08 01 08 {text}"), Some(value), &[]);
        values(&format!("f8 01 {text}"), None, &[value]);
        values(
            &format!("fa 01 {:02x} {text}", hex(text).len()),
            None,
            &[value],
        );
        values(
            &format!(
                "f8 01 01 fa 01 {:02x} {text} fa 01 00 f8 01 02",
                hex(text).len()
            ),
            None,
            &[1, value, 2],
        );
    }
    values("88 00 80 00", Some(0), &[]);
    values("88 80 80 80 10 00", Some(0), &[]);
    values("f8 81 80 80 10 00", None, &[0]);
    values("f8 01 01 fa 01 02 02 03 f8 01 04", None, &[1, 2, 3, 4]);
    values("fa 01 00", None, &[]);
}
#[test]
fn wrong_wire_and_nested_data_stay_opaque() {
    for wrong in [
        "09 00 00 00 00 00 00 00 00",
        "0a 01 80",
        "0d 00 00 00 00",
        "10 00",
        "11 00 00 00 00 00 00 00 00",
        "15 00 00 00 00",
        "f9 01 00 00 00 00 00 00 00 00",
        "fd 01 00 00 00 00",
    ] {
        values(wrong, None, &[]);
        for (text, start) in [
            (
                format!("{wrong} 08 06 12 02 10 05 f8 01 07"),
                hex(wrong).len() + 4,
            ),
            (format!("08 06 12 02 10 05 f8 01 07 {wrong}"), 4),
        ] {
            let bytes = hex(&text);
            check(
                &decode(&bytes, Limits::default()).unwrap(),
                &bytes,
                Some(6),
                &[(start, start + 2)],
                &[7],
            );
        }
    }
    for inner in ["", "00", "80", "0b", "ff"] {
        let bytes = hex(&format!("12 {:02x} {inner}", hex(inner).len()));
        let out = decode(&bytes, Limits::default()).unwrap();
        check(&out, &bytes, None, &[(2, bytes.len())], &[]);
        if inner.is_empty() {
            assert_eq!(
                value_parameter::decode(out.value_parameters[0].raw, Default::default())
                    .unwrap_err(),
                value_parameter::Error {
                    offset: 0,
                    kind: value_parameter::ErrorKind::MissingName
                }
            );
        }
    }
    for unknown in [
        "1a 01 00",
        "82 02 01 ff",
        "a2 06 01 80",
        "b8 a3 09 01",
        "c0 a3 09 01",
        "f8 ff ff ff 0f 01",
        "a1 06 00 00 00 00 00 00 00 00",
        "a5 06 00 00 00 00",
    ] {
        values(unknown, None, &[]);
    }
}
#[test]
fn structural_and_packed_errors_have_exact_offsets() {
    for (text, kind, offset) in [
        ("08", ErrorKind::Truncated, 1),
        ("12", ErrorKind::Truncated, 1),
        ("f8", ErrorKind::Truncated, 1),
        ("f8 01", ErrorKind::Truncated, 2),
        ("fa 01", ErrorKind::Truncated, 2),
        ("80", ErrorKind::Truncated, 1),
        ("00", ErrorKind::InvalidFieldNumber, 0),
        ("03", ErrorKind::InvalidFieldNumber, 0),
        ("07", ErrorKind::InvalidFieldNumber, 0),
        ("0b", ErrorKind::UnsupportedGroup, 0),
        ("0c", ErrorKind::UnsupportedGroup, 0),
        ("13", ErrorKind::UnsupportedGroup, 0),
        ("14", ErrorKind::UnsupportedGroup, 0),
        ("fb 01", ErrorKind::UnsupportedGroup, 0),
        ("fc 01", ErrorKind::UnsupportedGroup, 0),
        ("0e", ErrorKind::InvalidWireType, 0),
        ("0f", ErrorKind::InvalidWireType, 0),
        ("16", ErrorKind::InvalidWireType, 0),
        ("17", ErrorKind::InvalidWireType, 0),
        ("fe 01", ErrorKind::InvalidWireType, 0),
        ("ff 01", ErrorKind::InvalidWireType, 0),
        ("12 02 ff", ErrorKind::Truncated, 3),
        ("0d 01 02 03", ErrorKind::Truncated, 4),
        ("09 01 02 03 04 05 06 07", ErrorKind::Truncated, 8),
        ("12 80 80 80 80 08", ErrorKind::NegativeLength, 1),
        ("fa 01 80 80 80 80 08", ErrorKind::NegativeLength, 2),
        ("12 ff ff ff ff 07", ErrorKind::Truncated, 6),
        (
            "08 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            10,
        ),
        (
            "80 80 80 80 80 80 80 80 80 80",
            ErrorKind::MalformedVarint,
            9,
        ),
        ("fa 01 01 80 08 00", ErrorKind::Truncated, 4),
        ("fa 01 02 01 80 08 00", ErrorKind::Truncated, 5),
        (
            "fa 01 0a 80 80 80 80 80 80 80 80 80 80 08 00",
            ErrorKind::MalformedVarint,
            12,
        ),
    ] {
        for prefix in ["", "08 06", "12 02 10 05"] {
            error(
                &hex(&format!("{prefix} {text}")),
                Limits::default(),
                kind,
                offset + hex(prefix).len(),
            );
        }
    }
    for n in 1..=10 {
        error(
            &hex(&format!("fa 01 {n:02x} {} 08 00", "80 ".repeat(n))),
            Limits::default(),
            if n == 10 {
                ErrorKind::MalformedVarint
            } else {
                ErrorKind::Truncated
            },
            if n == 10 { 12 } else { n + 3 },
        );
    }
}
#[test]
fn exact_budget_precedence_and_shared_growth() {
    for (text, records, work, owned, kind, offset) in [
        ("08 00", 1, 0, 0, ErrorKind::Limit("work"), 0),
        ("08 00", 1, 1, 0, ErrorKind::Limit("work"), 0),
        ("08 00", 1, 2, 0, ErrorKind::Limit("work"), 1),
        ("08 00", 1, 3, 0, ErrorKind::Limit("work"), 0),
        ("08 00", 0, 0, 0, ErrorKind::Limit("records"), 0),
        ("08", 1, 2, 0, ErrorKind::Limit("work"), 1),
        ("08", 1, 3, 0, ErrorKind::Truncated, 1),
        ("12 00", 1, 3, S, ErrorKind::Limit("work"), 0),
        ("12 00", 1, 4, S, ErrorKind::Limit("work"), 0),
        ("12 00", 1, 4, 0, ErrorKind::Limit("owned_bytes"), 0),
        ("12 01 ff", 1, 4, S, ErrorKind::Limit("work"), 0),
        ("12 01 ff", 1, 5, S, ErrorKind::Limit("work"), 0),
        ("12 01 ff", 1, 5, 0, ErrorKind::Limit("owned_bytes"), 0),
        ("12 02 ff", 1, 3, S, ErrorKind::Limit("work"), 2),
        ("12 02 ff", 1, 4, S, ErrorKind::Limit("work"), 2),
        ("12 02 ff", 1, 5, S, ErrorKind::Truncated, 3),
        ("f8 01 00", 1, 4, I, ErrorKind::Limit("work"), 0),
        ("f8 01 00", 1, 5, 0, ErrorKind::Limit("work"), 0),
        ("f8 01 00", 1, 6, 0, ErrorKind::Limit("owned_bytes"), 0),
        ("f8 01 00", 1, 6, I, ErrorKind::Limit("work"), 0),
        ("fa 01 00", 1, 4, 0, ErrorKind::Limit("work"), 0),
        ("fa 01 01 00", 1, 100, I, ErrorKind::Limit("records"), 3),
        ("fa 01 01 00", 2, 5, I, ErrorKind::Limit("work"), 0),
        ("fa 01 01 00", 2, 6, I, ErrorKind::Limit("work"), 3),
        ("fa 01 01 00", 2, 7, I, ErrorKind::Limit("work"), 3),
        ("fa 01 01 00", 2, 8, I, ErrorKind::Limit("work"), 3),
        ("fa 01 01 00", 2, 9, 0, ErrorKind::Limit("owned_bytes"), 3),
        ("fa 01 01 00", 2, 9, I, ErrorKind::Limit("work"), 3),
        ("fa 01 01 80 08 00", 2, 8, 0, ErrorKind::Limit("work"), 4),
        ("fa 01 01 80 08 00", 2, 9, 0, ErrorKind::Truncated, 4),
        ("fa 01 01 80 08 00", 1, 6, 0, ErrorKind::Limit("records"), 3),
    ] {
        error(
            &hex(text),
            Limits {
                bytes: 32,
                records,
                work,
                owned_bytes: owned,
            },
            kind,
            offset,
        );
    }
    for (text, records, work, owned) in [
        ("08 00", 1, 4, 0),
        ("12 00", 1, 4 + S, S),
        ("12 01 ff", 1, 5 + S, S),
        ("f8 01 00", 1, 6 + I, I),
        ("fa 01 00", 1, 5, 0),
        ("fa 01 01 00", 2, 9 + I, I),
    ] {
        let bytes = hex(text);
        assert!(
            decode(
                &bytes,
                Limits {
                    bytes: bytes.len(),
                    records,
                    work,
                    owned_bytes: owned
                }
            )
            .is_ok()
        );
    }
    for k in 1usize..=5 {
        let capacity = k.next_power_of_two();
        let growth = if k == 1 { 0 } else { capacity / 2 };
        for (bytes, records, work, owned, growth_offset, last_work) in [
            (
                [0x12, 0].repeat(k),
                k,
                4 * k + capacity * S,
                capacity * S,
                2 * growth,
                if k == 4 { 6 } else { 2 * growth },
            ),
            (
                [0xf8, 1, 0].repeat(k),
                k,
                6 * k + capacity * I,
                capacity * I,
                3 * growth,
                if k == 4 { 9 } else { 3 * growth },
            ),
            (
                {
                    let mut v = vec![0xfa, 1, k as u8];
                    v.extend(vec![0; k]);
                    v
                },
                k + 1,
                5 + 4 * k + capacity * I,
                capacity * I,
                3 + growth,
                if k == 4 { 6 } else { 3 + growth },
            ),
        ] {
            let limits = Limits {
                bytes: bytes.len(),
                records,
                work,
                owned_bytes: owned,
            };
            assert!(decode(&bytes, limits).is_ok());
            error(
                &bytes,
                Limits {
                    owned_bytes: owned - 1,
                    ..limits
                },
                ErrorKind::Limit("owned_bytes"),
                growth_offset,
            );
            error(
                &bytes,
                Limits {
                    work: work - 1,
                    ..limits
                },
                ErrorKind::Limit("work"),
                last_work,
            );
        }
    }
    let bytes = hex("12 00 f8 01 00 12 00 fa 01 02 00 00");
    let limits = Limits {
        bytes: 12,
        records: 6,
        work: 27 + 2 * S + 4 * I,
        owned_bytes: 2 * S + 4 * I,
    };
    check(
        &decode(&bytes, limits).unwrap(),
        &bytes,
        None,
        &[(2, 2), (7, 7)],
        &[0, 0, 0],
    );
    error(
        &bytes,
        Limits {
            work: limits.work - 1,
            ..limits
        },
        ErrorKind::Limit("work"),
        11,
    );
    error(
        &bytes,
        Limits {
            owned_bytes: limits.owned_bytes - 1,
            ..limits
        },
        ErrorKind::Limit("owned_bytes"),
        11,
    );
}
#[test]
fn hard_limits_include_packed_records() {
    let unlimited = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    error(
        &vec![0; 16 * 1024 * 1024 + 1],
        unlimited,
        ErrorKind::Limit("bytes"),
        0,
    );
    let mut bytes = [8, 0].repeat(65_535);
    check(
        &decode(
            &bytes,
            Limits {
                bytes: 131_070,
                records: 65_535,
                work: 262_140,
                owned_bytes: 0,
            },
        )
        .unwrap(),
        &bytes,
        Some(0),
        &[],
        &[],
    );
    bytes.extend([8, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), 131_070);
    let mut bytes = hex("fa 01 fd ff 03");
    bytes.extend(vec![0; 65_533]);
    let out = decode(&bytes, unlimited).unwrap();
    assert_eq!(out.version_requirements, vec![0; 65_533]);
    bytes.extend([0xf8, 1, 0]);
    assert_eq!(
        decode(&bytes, unlimited)
            .unwrap()
            .version_requirements
            .len(),
        65_534
    );
    let end = bytes.len();
    bytes.extend([0xf8, 1, 0]);
    error(&bytes, unlimited, ErrorKind::Limit("records"), end);
}
#[test]
fn genuine_pair_constructor_and_both_parameters_use_separate_budgets() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload.span, 47..285);
    let mut fields = framed
        .payload
        .fields
        .iter()
        .filter(|field| field.number == 8 && matches!(field.value, Value::Bytes(_)));
    let field = fields.next().unwrap();
    assert!(fields.next().is_none());
    assert_eq!(field.span, 76..103);
    let Value::Bytes(body) = field.value else {
        unreachable!()
    };
    assert_eq!(
        body,
        hex(include_str!(
            "fixtures/metadata_constructor/pair_constructor.hex"
        ))
    );
    assert_eq!(body.as_ptr(), bytes[78..103].as_ptr());
    let out = decode(
        body,
        Limits {
            bytes: 25,
            records: 4,
            work: 33 + 2 * S,
            owned_bytes: 2 * S,
        },
    )
    .unwrap();
    check(&out, body, Some(70), &[(4, 10), (12, 18)], &[]);
    for (parameter, name, d1_start, literal) in [
        (&out.value_parameters[0], 5, 82, "10 05 1a 02 38 00"),
        (&out.value_parameters[1], 6, 90, "10 06 1a 02 38 01"),
    ] {
        assert_eq!(parameter.raw, hex(literal));
        assert_eq!(
            parameter.raw.as_ptr(),
            bytes[d1_start..d1_start + 6].as_ptr()
        );
        let selected = value_parameter::decode(
            parameter.raw,
            value_parameter::Limits {
                bytes: 6,
                records: 2,
                work: 10,
            },
        )
        .unwrap();
        assert_eq!(
            (
                selected.flags,
                selected.name,
                selected.type_id,
                selected.vararg_element_type_id
            ),
            (None, name, None, None)
        );
        assert_eq!(selected.raw.as_ptr(), parameter.raw.as_ptr());
    }
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
                for body in out.value_parameters {
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
