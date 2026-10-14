use ktlsp_binary::metadata::{
    self, constructor,
    constructor_parameters::{
        ConstructorWithParameters, Error, ErrorKind, Limits, SelectedValueParameter, decode,
    },
    payload,
    wire::{self, ErrorKind as WireKind, Value},
};
use std::mem::size_of;

const B: usize = size_of::<constructor::ValueParameterBody<'_>>();
const T: usize = size_of::<SelectedValueParameter<'_>>();
const I: usize = size_of::<i32>();
type Scalars = (Option<i32>, i32, Option<i32>, Option<i32>);
type Parameter = (usize, usize, Scalars);
type Success = (usize, Option<i32>, &'static [i32], &'static [Parameter]);
type Positive = (
    &'static str,
    Option<i32>,
    &'static [i32],
    &'static [Parameter],
);

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn borrows(out: &ConstructorWithParameters<'_>, bytes: &[u8]) {
    assert_eq!(out.raw, bytes);
    assert_eq!(out.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for child in &out.value_parameters {
        assert!(end <= child.span.start && child.span.start <= child.span.end);
        assert!(child.span.end <= bytes.len());
        assert_eq!(child.parameter.raw, &bytes[child.span.clone()]);
        assert_eq!(
            child.parameter.raw.as_ptr(),
            bytes[child.span.clone()].as_ptr()
        );
        end = child.span.end;
    }
}
fn check(
    out: &ConstructorWithParameters<'_>,
    bytes: &[u8],
    flags: Option<i32>,
    ids: &[i32],
    parameters: &[Parameter],
) {
    borrows(out, bytes);
    assert_eq!(out.flags, flags);
    assert_eq!(out.version_requirements, ids);
    assert_eq!(out.value_parameters.len(), parameters.len());
    for (child, &(start, end, expected)) in out.value_parameters.iter().zip(parameters) {
        assert_eq!(child.span, start..end);
        let p = &child.parameter;
        assert_eq!(
            (p.flags, p.name, p.type_id, p.vararg_element_type_id),
            expected
        );
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
}
fn limited(bytes: &[u8], limits: Limits, label: &'static str, offset: usize) {
    error(
        bytes,
        limits,
        ErrorKind::Wire(WireKind::Limit(label)),
        offset,
    );
}
struct Fixture {
    text: &'static str,
    cuts: &'static [Success],
    records: usize,
    owned: usize,
    work: usize,
    last_record: usize,
    last_storage: usize,
}
// Literal expectations transcribed from the independently frozen EXPECTATIONS.json.
fn fixtures() -> [Fixture; 6] {
    [
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/empty.hex"),
            cuts: &[(0, None, &[], &[])],
            records: 0,
            owned: 0,
            work: 0,
            last_record: 0,
            last_storage: 0,
        },
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/selected.hex"),
            cuts: &[
                (0, None, &[], &[]),
                (2, Some(6), &[], &[]),
                (12, Some(6), &[], &[(4, 12, (Some(0), 5, Some(7), Some(8)))]),
                (
                    18,
                    Some(6),
                    &[1, 128],
                    &[(4, 12, (Some(0), 5, Some(7), Some(8)))],
                ),
                (
                    21,
                    Some(6),
                    &[1, 128, 2],
                    &[(4, 12, (Some(0), 5, Some(7), Some(8)))],
                ),
            ],
            records: 10,
            owned: B + 4 * I + T,
            work: 53 + B + 4 * I + T,
            last_record: 10,
            last_storage: 4,
        },
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/duplicates.hex"),
            cuts: &[
                (0, None, &[], &[]),
                (2, Some(0), &[], &[]),
                (4, Some(7), &[], &[]),
                (7, Some(7), &[], &[]),
                (23, Some(7), &[], &[(9, 23, (Some(0), 2, Some(4), Some(0)))]),
                (
                    31,
                    Some(7),
                    &[],
                    &[
                        (9, 23, (Some(0), 2, Some(4), Some(0))),
                        (25, 31, (None, 2, None, None)),
                    ],
                ),
                (
                    36,
                    Some(7),
                    &[1, 1],
                    &[
                        (9, 23, (Some(0), 2, Some(4), Some(0))),
                        (25, 31, (None, 2, None, None)),
                    ],
                ),
                (
                    39,
                    Some(7),
                    &[1, 1, 2],
                    &[
                        (9, 23, (Some(0), 2, Some(4), Some(0))),
                        (25, 31, (None, 2, None, None)),
                    ],
                ),
            ],
            records: 18,
            owned: 2 * B + 4 * I + 2 * T,
            work: 98 + 2 * B + 4 * I + 2 * T,
            last_record: 27,
            last_storage: 25,
        },
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/opaque.hex"),
            cuts: &[
                (0, None, &[], &[]),
                (12, None, &[], &[(2, 12, (None, 0, None, None))]),
            ],
            records: 5,
            owned: B + T,
            work: 32 + B + T,
            last_record: 10,
            last_storage: 2,
        },
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/three.hex"),
            cuts: &[
                (0, None, &[], &[]),
                (4, None, &[], &[(2, 4, (None, 0, None, None))]),
                (
                    8,
                    None,
                    &[],
                    &[(2, 4, (None, 0, None, None)), (6, 8, (None, 1, None, None))],
                ),
                (
                    12,
                    None,
                    &[],
                    &[
                        (2, 4, (None, 0, None, None)),
                        (6, 8, (None, 1, None, None)),
                        (10, 12, (None, 2, None, None)),
                    ],
                ),
            ],
            records: 6,
            owned: 4 * B + 4 * T,
            work: 30 + 4 * B + 4 * T,
            last_record: 10,
            last_storage: 10,
        },
        Fixture {
            text: include_str!("fixtures/metadata_constructor_parameters/pair_constructor.hex"),
            cuts: &[
                (0, None, &[], &[]),
                (2, Some(70), &[], &[]),
                (10, Some(70), &[], &[(4, 10, (None, 5, None, None))]),
                (
                    18,
                    Some(70),
                    &[],
                    &[
                        (4, 10, (None, 5, None, None)),
                        (12, 18, (None, 6, None, None)),
                    ],
                ),
                (
                    25,
                    Some(70),
                    &[],
                    &[
                        (4, 10, (None, 5, None, None)),
                        (12, 18, (None, 6, None, None)),
                    ],
                ),
            ],
            records: 8,
            owned: 2 * B + 2 * T,
            work: 53 + 2 * B + 2 * T,
            last_record: 14,
            last_storage: 12,
        },
    ]
}

#[test]
fn frozen_tuples_borrows_and_exact_aggregate_minima() {
    for f in fixtures() {
        let bytes = hex(f.text);
        let &(_, flags, ids, parameters) = f.cuts.last().unwrap();
        let limits = Limits {
            bytes: bytes.len(),
            records: f.records,
            work: f.work,
            owned_bytes: f.owned,
        };
        check(
            &decode(&bytes, limits).unwrap(),
            &bytes,
            flags,
            ids,
            parameters,
        );
        let original = constructor::decode(&bytes, Default::default()).unwrap();
        assert_eq!(original.raw.as_ptr(), bytes.as_ptr());
        assert_eq!(
            (original.flags, original.version_requirements.as_slice()),
            (flags, ids)
        );
        assert_eq!(original.value_parameters.len(), parameters.len());
        for (body, &(start, end, _)) in original.value_parameters.iter().zip(parameters) {
            assert_eq!(body.span, start..end);
            assert_eq!(body.raw.as_ptr(), bytes[start..end].as_ptr());
        }
        if bytes.is_empty() {
            continue;
        }
        limited(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                ..limits
            },
            "bytes",
            0,
        );
        limited(
            &bytes,
            Limits {
                records: f.records - 1,
                ..limits
            },
            "records",
            f.last_record,
        );
        limited(
            &bytes,
            Limits {
                owned_bytes: f.owned - 1,
                ..limits
            },
            "owned_bytes",
            f.last_storage,
        );
        limited(
            &bytes,
            Limits {
                work: f.work - 1,
                ..limits
            },
            "work",
            f.last_storage,
        );
    }
}

#[test]
fn all_115_frozen_prefix_cuts() {
    let mut count = 0;
    for f in fixtures() {
        let bytes = hex(f.text);
        for cut in 0..=bytes.len() {
            count += 1;
            let prefix = &bytes[..cut];
            match f.cuts.iter().find(|entry| entry.0 == cut) {
                Some(&(_, flags, ids, parameters)) => check(
                    &decode(prefix, Default::default()).unwrap(),
                    prefix,
                    flags,
                    ids,
                    parameters,
                ),
                None => error(
                    prefix,
                    Default::default(),
                    ErrorKind::Wire(WireKind::Truncated),
                    cut,
                ),
            }
        }
    }
    assert_eq!(count, 115);
}

#[test]
fn frozen_18_faults_and_parent_first_child_offsets() {
    for (text, kind, offset) in [
        ("12 00", ErrorKind::MissingName, 2),
        ("12 02 08 00", ErrorKind::MissingName, 4),
        ("12 02 12 00", ErrorKind::MissingName, 4),
        ("12 01 00", ErrorKind::Wire(WireKind::InvalidFieldNumber), 2),
        ("12 01 08", ErrorKind::Wire(WireKind::Truncated), 3),
        ("12 01 0b", ErrorKind::Wire(WireKind::UnsupportedGroup), 2),
        ("12 01 0e", ErrorKind::Wire(WireKind::InvalidWireType), 2),
        ("12 00 00", ErrorKind::Wire(WireKind::InvalidFieldNumber), 2),
        ("12 01 00 08", ErrorKind::Wire(WireKind::Truncated), 4),
        ("12 00 fa 01 01 80", ErrorKind::Wire(WireKind::Truncated), 6),
        ("12 02 10 00 12 00", ErrorKind::MissingName, 6),
        (
            "12 ff ff ff ff 0f",
            ErrorKind::Wire(WireKind::NegativeLength),
            1,
        ),
        (
            "12 06 1a ff ff ff ff 0f",
            ErrorKind::Wire(WireKind::NegativeLength),
            3,
        ),
        (
            "12 0b 10 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Wire(WireKind::MalformedVarint),
            12,
        ),
        ("0b", ErrorKind::Wire(WireKind::UnsupportedGroup), 0),
        ("0e", ErrorKind::Wire(WireKind::InvalidWireType), 0),
        ("00", ErrorKind::Wire(WireKind::InvalidFieldNumber), 0),
        ("12 00 08 00", ErrorKind::MissingName, 2),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
}

#[test]
fn frozen_signed_wrong_wire_and_repeated_positives() {
    let cases: [Positive; 4] = [
        (
            "12 04 10 00 12 00",
            None,
            &[],
            &[(2, 6, (None, 0, None, None))],
        ),
        (
            "12 0c 10 ff ff ff ff 0f 28 80 80 80 80 08",
            None,
            &[],
            &[(2, 14, (None, -1, Some(-2147483648), None))],
        ),
        (
            "08 ff ff ff ff 0f f8 01 ff ff ff ff 0f",
            Some(-1),
            &[-1],
            &[],
        ),
        (
            "12 02 10 00 12 02 10 00",
            None,
            &[],
            &[(2, 4, (None, 0, None, None)), (6, 8, (None, 0, None, None))],
        ),
    ];
    for (text, flags, ids, parameters) in cases {
        let bytes = hex(text);
        check(
            &decode(&bytes, Default::default()).unwrap(),
            &bytes,
            flags,
            ids,
            parameters,
        );
    }
}

#[test]
fn shared_budget_growth_and_exact_event_offsets() {
    let simple = hex("12 02 10 00");
    for (records, owned_bytes, work, label, offset) in [
        (0, B + T, 10 + B + T, "records", 0),
        (1, B + T, 10 + B + T, "records", 2),
        (2, 0, 10 + B + T, "owned_bytes", 0),
        (2, B, 10 + B + T, "owned_bytes", 2),
        (2, B + T, 6 + B, "work", 2),
        (2, B + T, 7 + B, "work", 2),
        (2, B + T, 8 + B, "work", 3),
        (2, B + T, 9 + B, "work", 2),
        (2, B + T, 10 + B, "work", 2),
    ] {
        limited(
            &simple,
            Limits {
                bytes: 4,
                records,
                owned_bytes,
                work,
            },
            label,
            offset,
        );
    }
    let selected = hex(include_str!(
        "fixtures/metadata_constructor_parameters/selected.hex"
    ));
    limited(
        &selected,
        Limits {
            owned_bytes: B + 4 * I - 1,
            ..Default::default()
        },
        "owned_bytes",
        18,
    );
    limited(
        &selected,
        Limits {
            owned_bytes: B + 4 * I,
            ..Default::default()
        },
        "owned_bytes",
        4,
    );
    limited(
        &selected,
        Limits {
            work: 37 + B + 4 * I,
            ..Default::default()
        },
        "work",
        4,
    );
    limited(
        &selected,
        Limits {
            records: 6,
            ..Default::default()
        },
        "records",
        4,
    );
    let pair = hex(include_str!(
        "fixtures/metadata_constructor_parameters/pair_constructor.hex"
    ));
    limited(
        &pair,
        Limits {
            records: 6,
            ..Default::default()
        },
        "records",
        12,
    );
    limited(
        &pair,
        Limits {
            records: 7,
            ..Default::default()
        },
        "records",
        14,
    );
    let three = hex(include_str!(
        "fixtures/metadata_constructor_parameters/three.hex"
    ));
    for owned_bytes in [4 * B + 2 * T, 4 * B + 4 * T - 1] {
        limited(
            &three,
            Limits {
                owned_bytes,
                ..Default::default()
            },
            "owned_bytes",
            10,
        );
    }
}

#[test]
fn frozen_missing_name_and_limit_precedence() {
    error(
        &hex("12 00"),
        Limits {
            owned_bytes: B,
            work: 4 + B,
            ..Default::default()
        },
        ErrorKind::MissingName,
        2,
    );
    error(
        &hex("12 01 00"),
        Limits {
            owned_bytes: B,
            ..Default::default()
        },
        ErrorKind::Wire(WireKind::InvalidFieldNumber),
        2,
    );
    limited(
        &hex("12 00 08 00"),
        Limits {
            records: 1,
            ..Default::default()
        },
        "records",
        2,
    );
    error(
        &hex("12 02 10 00 12 00"),
        Limits {
            owned_bytes: 2 * B + T,
            ..Default::default()
        },
        ErrorKind::MissingName,
        6,
    );
    for (limits, label) in [
        (
            Limits {
                bytes: 1,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            "bytes",
        ),
        (
            Limits {
                bytes: 2,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            "records",
        ),
        (
            Limits {
                bytes: 2,
                records: 1,
                work: 0,
                owned_bytes: 0,
            },
            "work",
        ),
    ] {
        limited(&hex("12 00"), limits, label, 0);
    }
}

#[test]
fn caller_limits_cannot_raise_hard_caps() {
    let raised = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    let too_long = vec![0; ktlsp_binary::MAX_BYTES + 1];
    limited(&too_long, raised, "bytes", 0);
    for f in fixtures() {
        let bytes = hex(f.text);
        assert_eq!(decode(&bytes, raised), decode(&bytes, Default::default()));
    }
    let mut scalars = [0x08, 0].repeat(65_535);
    check(
        &decode(
            &scalars,
            Limits {
                bytes: 131_070,
                records: 65_535,
                work: 262_140,
                owned_bytes: 0,
            },
        )
        .unwrap(),
        &scalars,
        Some(0),
        &[],
        &[],
    );
    scalars.extend([0x08, 0]);
    limited(&scalars, raised, "records", 131_070);
    assert_eq!(
        decode(&scalars, raised),
        decode(&scalars, Default::default())
    );
    let mut parameters = [0x12, 2, 0x10, 0].repeat(32_767);
    let out = decode(
        &parameters,
        Limits {
            bytes: 131_068,
            records: 65_534,
            work: 327_670 + 32_768 * (B + T),
            owned_bytes: 32_768 * (B + T),
        },
    )
    .unwrap();
    assert_eq!(out.flags, None);
    assert!(out.version_requirements.is_empty());
    assert_eq!(out.value_parameters.len(), 32_767);
    borrows(&out, &parameters);
    for (i, child) in out.value_parameters.iter().enumerate() {
        assert_eq!(child.span, 4 * i + 2..4 * i + 4);
        let p = &child.parameter;
        assert_eq!(
            (p.flags, p.name, p.type_id, p.vararg_element_type_id),
            (None, 0, None, None)
        );
    }
    drop(out);
    parameters.extend([0x12, 2, 0x10, 0]);
    limited(&parameters, raised, "records", 131_070);
    assert_eq!(
        decode(&parameters, raised),
        decode(&parameters, Default::default())
    );
}

#[test]
fn genuine_pair_class_metadata_payload_constructor_and_both_parameter_tuples() {
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
    let framed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.string_table_span, 1..47);
    assert_eq!(framed.payload_span, 47..285);
    assert_eq!(framed.payload.as_ptr(), bytes[47..285].as_ptr());
    let flat = wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(flat.payload.span, 47..285);
    assert_eq!(flat.payload.raw.as_ptr(), framed.payload.as_ptr());
    let mut constructors = flat
        .payload
        .fields
        .iter()
        .filter(|field| field.number == 8 && matches!(field.value, Value::Bytes(_)));
    let field = constructors.next().unwrap();
    assert!(constructors.next().is_none());
    assert_eq!(field.span, 76..103);
    let Value::Bytes(body) = field.value else {
        unreachable!()
    };
    assert_eq!(
        body,
        hex(include_str!(
            "fixtures/metadata_constructor_parameters/pair_constructor.hex"
        ))
    );
    assert_eq!(body.as_ptr(), bytes[78..103].as_ptr());
    let out = decode(
        body,
        Limits {
            bytes: 25,
            records: 8,
            work: 53 + 2 * B + 2 * T,
            owned_bytes: 2 * B + 2 * T,
        },
    )
    .unwrap();
    check(
        &out,
        body,
        Some(70),
        &[],
        &[
            (4, 10, (None, 5, None, None)),
            (12, 18, (None, 6, None, None)),
        ],
    );
    for (child, d1_start, literal) in [
        (&out.value_parameters[0], 82, "10 05 1a 02 38 00"),
        (&out.value_parameters[1], 90, "10 06 1a 02 38 01"),
    ] {
        assert_eq!(child.parameter.raw, hex(literal));
        assert_eq!(
            child.parameter.raw.as_ptr(),
            bytes[d1_start..d1_start + 6].as_ptr()
        );
    }
}

#[test]
fn finite_hostile_parents_and_children_are_deterministic_and_borrowed() {
    let limits = Limits {
        bytes: 32,
        records: 16,
        work: 4096,
        owned_bytes: 4096,
    };
    let probe = |bytes: &[u8]| {
        let result = decode(bytes, limits);
        assert_eq!(result, decode(bytes, limits));
        match result {
            Ok(out) => borrows(&out, bytes),
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for value in 0u32..=65_535 {
        let a = (value >> 8) as u8;
        let b = value as u8;
        probe(&[a, b]);
        probe(&[0x12, 2, a, b]);
    }
    let mut state = 1u32;
    for len in 0..=32 {
        for _ in 0..256 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (state >> 24) as u8
                })
                .collect();
            probe(&bytes);
        }
    }
}
