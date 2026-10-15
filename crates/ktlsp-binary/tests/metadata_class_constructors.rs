use ktlsp_binary::metadata::{
    self,
    class_constructors::{
        ClassConstructors, ConstructorOccurrence, Error, ErrorKind, Limits, decode,
    },
    class_header, constructor, payload, value_parameter, wire,
};
use std::{mem::size_of, ops::Range};

const O: usize = size_of::<ConstructorOccurrence<'_>>();
const S: usize = size_of::<constructor::ValueParameterBody<'_>>();
const I: usize = size_of::<i32>();

struct ExpectedConstructor {
    span: Range<usize>,
    flags: Option<i32>,
    parameters: Vec<(Range<usize>, &'static str)>,
    versions: Vec<i32>,
}
struct Case {
    name: &'static str,
    raw: &'static str,
    flags: Option<i32>,
    fq_name: i32,
    constructors: Vec<ExpectedConstructor>,
    limits: Limits,
    records_error: Error,
    work_error: Error,
    owned_error: Option<Error>,
    success_cuts: &'static [usize],
    missing_cuts: &'static [usize],
}
fn hex(raw: &str) -> Vec<u8> {
    raw.split_whitespace()
        .map(|part| u8::from_str_radix(part, 16).unwrap())
        .collect()
}
fn fail(bytes: &[u8], limits: Limits, expected: Error) {
    assert_eq!(decode(bytes, limits).unwrap_err(), expected, "{bytes:02x?}");
}
fn check(
    out: &ClassConstructors<'_>,
    bytes: &[u8],
    flags: Option<i32>,
    fq_name: i32,
    expected: &[ExpectedConstructor],
) {
    assert_eq!((out.header.flags, out.header.fq_name), (flags, fq_name));
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(
        out.header,
        class_header::decode(bytes, Default::default()).unwrap()
    );
    let selected: Vec<_> = expected
        .iter()
        .filter(|c| c.span.end <= bytes.len())
        .collect();
    assert_eq!(out.constructors.len(), selected.len());
    for (occurrence, expected) in out.constructors.iter().zip(selected) {
        assert_eq!(occurrence.body_span, expected.span);
        assert!(occurrence.body_span.start <= occurrence.body_span.end);
        let body = &bytes[occurrence.body_span.clone()];
        let child = &occurrence.constructor;
        assert_eq!(child.raw, body);
        assert_eq!(child.raw.as_ptr(), body.as_ptr());
        assert_eq!(child.flags, expected.flags);
        assert_eq!(child.version_requirements, expected.versions);
        assert_eq!(child.value_parameters.len(), expected.parameters.len());
        assert_eq!(
            child,
            &constructor::decode(body, Default::default()).unwrap()
        );
        for (parameter, (span, raw)) in child.value_parameters.iter().zip(&expected.parameters) {
            assert_eq!(&parameter.span, span);
            assert!(span.start <= span.end && span.end <= body.len());
            assert_eq!(parameter.raw, hex(raw));
            assert_eq!(parameter.raw.as_ptr(), body[span.clone()].as_ptr());
            assert_eq!(
                parameter.raw.as_ptr(),
                bytes[occurrence.body_span.start + span.start..].as_ptr()
            );
        }
    }
}

// Literal tables transcribed from the sealed independent EXPECTATIONS.json.
fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "minimal",
            raw: include_str!("fixtures/metadata_class_constructors/minimal.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![],
            limits: Limits {
                bytes: 2,
                records: 2,
                work: 8,
                owned_bytes: 0,
            },
            records_error: Error {
                offset: 0,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 0,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: None,
            success_cuts: &[2],
            missing_cuts: &[],
        },
        Case {
            name: "empty_one",
            raw: include_str!("fixtures/metadata_class_constructors/empty_one.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 4..4,
                flags: None,
                parameters: vec![],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 4,
                records: 4,
                work: 16 + O,
                owned_bytes: O,
            },
            records_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 4],
            missing_cuts: &[],
        },
        Case {
            name: "explicit",
            raw: include_str!("fixtures/metadata_class_constructors/explicit.hex"),
            flags: Some(6),
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 6..8,
                flags: Some(6),
                parameters: vec![],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 8,
                records: 7,
                work: 32 + O,
                owned_bytes: O,
            },
            records_error: Error {
                offset: 6,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 4,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 4,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[4, 8],
            missing_cuts: &[2],
        },
        Case {
            name: "selected",
            raw: include_str!("fixtures/metadata_class_constructors/selected.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 4..23,
                flags: Some(0),
                parameters: vec![(4..6, "10 05"), (16..16, "")],
                versions: vec![1, 2, 3, 4],
            }],
            limits: Limits {
                bytes: 23,
                records: 12,
                work: 93 + O + 2 * S + 4 * I,
                owned_bytes: O + 2 * S + 4 * I,
            },
            records_error: Error {
                offset: 20,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 23],
            missing_cuts: &[],
        },
        Case {
            name: "repeated",
            raw: include_str!("fixtures/metadata_class_constructors/repeated.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![
                ExpectedConstructor {
                    span: 4..4,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 6..8,
                    flags: Some(6),
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 10..10,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
            ],
            limits: Limits {
                bytes: 10,
                records: 9,
                work: 40 + 4 * O,
                owned_bytes: 4 * O,
            },
            records_error: Error {
                offset: 8,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 8,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 8,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 4, 8, 10],
            missing_cuts: &[],
        },
        Case {
            name: "outer_five",
            raw: include_str!("fixtures/metadata_class_constructors/outer_five.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![
                ExpectedConstructor {
                    span: 4..4,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 6..6,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 8..8,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 10..10,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 12..12,
                    flags: None,
                    parameters: vec![],
                    versions: vec![],
                },
            ],
            limits: Limits {
                bytes: 12,
                records: 12,
                work: 48 + 8 * O,
                owned_bytes: 8 * O,
            },
            records_error: Error {
                offset: 10,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 10,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 10,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 4, 6, 8, 10, 12],
            missing_cuts: &[],
        },
        Case {
            name: "parameters_five",
            raw: include_str!("fixtures/metadata_class_constructors/parameters_five.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 4..14,
                flags: None,
                parameters: vec![(2..2, ""), (4..4, ""), (6..6, ""), (8..8, ""), (10..10, "")],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 14,
                records: 9,
                work: 56 + O + 8 * S,
                owned_bytes: O + 8 * S,
            },
            records_error: Error {
                offset: 12,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 14],
            missing_cuts: &[],
        },
        Case {
            name: "packed_five",
            raw: include_str!("fixtures/metadata_class_constructors/packed_five.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 4..12,
                flags: None,
                parameters: vec![],
                versions: vec![0, 0, 0, 0, 0],
            }],
            limits: Limits {
                bytes: 12,
                records: 10,
                work: 57 + O + 8 * I,
                owned_bytes: O + 8 * I,
            },
            records_error: Error {
                offset: 11,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 12],
            missing_cuts: &[],
        },
        Case {
            name: "opaque_class",
            raw: include_str!("fixtures/metadata_class_constructors/opaque_class.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![],
            limits: Limits {
                bytes: 24,
                records: 12,
                work: 72,
                owned_bytes: 0,
            },
            records_error: Error {
                offset: 21,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 21,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: None,
            success_cuts: &[2, 4, 9, 18, 21, 24],
            missing_cuts: &[],
        },
        Case {
            name: "opaque_child",
            raw: include_str!("fixtures/metadata_class_constructors/opaque_child.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 4..17,
                flags: None,
                parameters: vec![(2..3, "80")],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 17,
                records: 8,
                work: 63 + O + S,
                owned_bytes: O + S,
            },
            records_error: Error {
                offset: 13,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 17],
            missing_cuts: &[],
        },
        Case {
            name: "signed",
            raw: include_str!("fixtures/metadata_class_constructors/signed.hex"),
            flags: Some(-1),
            fq_name: -2147483648,
            constructors: vec![ExpectedConstructor {
                span: 14..41,
                flags: Some(-1),
                parameters: vec![],
                versions: vec![-1, 2147483647, 0],
            }],
            limits: Limits {
                bytes: 41,
                records: 11,
                work: 138 + O + 4 * I,
                owned_bytes: O + 4 * I,
            },
            records_error: Error {
                offset: 40,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 12,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 12,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[12, 41],
            missing_cuts: &[6],
        },
        Case {
            name: "nonminimal",
            raw: include_str!("fixtures/metadata_class_constructors/nonminimal.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 6..6,
                flags: None,
                parameters: vec![],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 6,
                records: 4,
                work: 20 + O,
                owned_bytes: O,
            },
            records_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 6],
            missing_cuts: &[],
        },
        Case {
            name: "aliased_tag",
            raw: include_str!("fixtures/metadata_class_constructors/aliased_tag.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 8..8,
                flags: None,
                parameters: vec![],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 8,
                records: 4,
                work: 24 + O,
                owned_bytes: O,
            },
            records_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 8],
            missing_cuts: &[],
        },
        Case {
            name: "unknown_then_empty",
            raw: include_str!("fixtures/metadata_class_constructors/unknown_then_empty.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 7..7,
                flags: None,
                parameters: vec![],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 7,
                records: 6,
                work: 26 + O,
                owned_bytes: O,
            },
            records_error: Error {
                offset: 5,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 5,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 5,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 5, 7],
            missing_cuts: &[],
        },
        Case {
            name: "pair_class",
            raw: include_str!("fixtures/metadata_class_constructors/pair_class.hex"),
            flags: Some(1031),
            fq_name: 0,
            constructors: vec![ExpectedConstructor {
                span: 31..56,
                flags: Some(70),
                parameters: vec![(4..10, "10 05 1a 02 38 00"), (12..18, "10 06 1a 02 38 01")],
                versions: vec![],
            }],
            limits: Limits {
                bytes: 238,
                records: 34,
                work: 569 + O + 2 * S,
                owned_bytes: O + 2 * S,
            },
            records_error: Error {
                offset: 235,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 235,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 29,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[5, 13, 21, 29, 56, 68, 85, 102, 151, 173, 185, 210, 235, 238],
            missing_cuts: &[3],
        },
        Case {
            name: "duplicate_scalars",
            raw: include_str!("fixtures/metadata_class_constructors/duplicate_scalars.hex"),
            flags: Some(6),
            fq_name: 0,
            constructors: vec![
                ExpectedConstructor {
                    span: 6..8,
                    flags: Some(1),
                    parameters: vec![],
                    versions: vec![],
                },
                ExpectedConstructor {
                    span: 14..18,
                    flags: Some(0),
                    parameters: vec![],
                    versions: vec![],
                },
            ],
            limits: Limits {
                bytes: 18,
                records: 15,
                work: 72 + 2 * O,
                owned_bytes: 2 * O,
            },
            records_error: Error {
                offset: 16,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 12,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 12,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[4, 8, 10, 12, 18],
            missing_cuts: &[2],
        },
        Case {
            name: "two_allocating",
            raw: include_str!("fixtures/metadata_class_constructors/two_allocating.hex"),
            flags: None,
            fq_name: 0,
            constructors: vec![
                ExpectedConstructor {
                    span: 4..9,
                    flags: None,
                    parameters: vec![(2..2, "")],
                    versions: vec![0],
                },
                ExpectedConstructor {
                    span: 11..16,
                    flags: None,
                    parameters: vec![(2..2, "")],
                    versions: vec![0],
                },
            ],
            limits: Limits {
                bytes: 16,
                records: 10,
                work: 64 + 2 * O + 2 * S + 2 * I,
                owned_bytes: 2 * O + 2 * S + 2 * I,
            },
            records_error: Error {
                offset: 13,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("records")),
            },
            work_error: Error {
                offset: 9,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("work")),
            },
            owned_error: Some(Error {
                offset: 9,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
            }),
            success_cuts: &[2, 9, 16],
            missing_cuts: &[],
        },
    ]
}
fn faults() -> Vec<(&'static str, Error)> {
    vec![
        (
            "",
            Error {
                offset: 0,
                kind: ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            },
        ),
        (
            "08 06",
            Error {
                offset: 2,
                kind: ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            },
        ),
        (
            "42 01 00",
            Error {
                offset: 3,
                kind: ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            },
        ),
        (
            "42 01 80 1a 00",
            Error {
                offset: 5,
                kind: ErrorKind::Class(class_header::ErrorKind::MissingFqName),
            },
        ),
        (
            "18 00 42 01 00",
            Error {
                offset: 4,
                kind: ErrorKind::Constructor(wire::ErrorKind::InvalidFieldNumber),
            },
        ),
        (
            "18 00 42 01 80",
            Error {
                offset: 5,
                kind: ErrorKind::Constructor(wire::ErrorKind::Truncated),
            },
        ),
        (
            "42 01 80 18 00",
            Error {
                offset: 3,
                kind: ErrorKind::Constructor(wire::ErrorKind::Truncated),
            },
        ),
        (
            "18 00 42 01 0b",
            Error {
                offset: 4,
                kind: ErrorKind::Constructor(wire::ErrorKind::UnsupportedGroup),
            },
        ),
        (
            "18 00 42 01 0e",
            Error {
                offset: 4,
                kind: ErrorKind::Constructor(wire::ErrorKind::InvalidWireType),
            },
        ),
        (
            "18 00 42 06 12 80 80 80 80 08",
            Error {
                offset: 5,
                kind: ErrorKind::Constructor(wire::ErrorKind::NegativeLength),
            },
        ),
        (
            "18 00 42 0b 08 80 80 80 80 80 80 80 80 80 80",
            Error {
                offset: 14,
                kind: ErrorKind::Constructor(wire::ErrorKind::MalformedVarint),
            },
        ),
        (
            "18 00 42 0a 80 80 80 80 80 80 80 80 80 80",
            Error {
                offset: 13,
                kind: ErrorKind::Constructor(wire::ErrorKind::MalformedVarint),
            },
        ),
        (
            "18 00 42 04 fa 01 01 80 08 06",
            Error {
                offset: 8,
                kind: ErrorKind::Constructor(wire::ErrorKind::Truncated),
            },
        ),
        (
            "18 00 42 01 00 00",
            Error {
                offset: 5,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(
                    wire::ErrorKind::InvalidFieldNumber,
                )),
            },
        ),
        (
            "18 00 42 01 80 08",
            Error {
                offset: 6,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
            },
        ),
        (
            "18 00 42 02 ff",
            Error {
                offset: 5,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)),
            },
        ),
        (
            "18 00 42 80 80 80 80 08",
            Error {
                offset: 3,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(
                    wire::ErrorKind::NegativeLength,
                )),
            },
        ),
        (
            "18 00 43",
            Error {
                offset: 2,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(
                    wire::ErrorKind::UnsupportedGroup,
                )),
            },
        ),
        (
            "18 00 46",
            Error {
                offset: 2,
                kind: ErrorKind::Class(class_header::ErrorKind::Wire(
                    wire::ErrorKind::InvalidWireType,
                )),
            },
        ),
        (
            "18 00 42 00 42 01 80",
            Error {
                offset: 7,
                kind: ErrorKind::Constructor(wire::ErrorKind::Truncated),
            },
        ),
        (
            "18 00 42 02 12 00 42 01 00",
            Error {
                offset: 8,
                kind: ErrorKind::Constructor(wire::ErrorKind::InvalidFieldNumber),
            },
        ),
        (
            "18 00 42 01 00 42 01 0b",
            Error {
                offset: 4,
                kind: ErrorKind::Constructor(wire::ErrorKind::InvalidFieldNumber),
            },
        ),
    ]
}
fn scalars() -> [(&'static str, i32); 10] {
    [
        ("00", 0),
        ("06", 6),
        ("ac 02", 300),
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("80 80 80 80 08", -2147483648),
        ("ff ff ff ff 07", 2147483647),
        ("80 00", 0),
        ("80 80 80 80 10", 0),
        ("80 80 80 80 80 80 80 80 80 02", 0),
    ]
}
#[test]
fn frozen_tuples_borrows_raw_api_and_exact_aggregate_minima() {
    for case in cases() {
        let bytes = hex(case.raw);
        assert_eq!(bytes.len(), case.limits.bytes, "{}", case.name);
        let out = decode(&bytes, case.limits).unwrap();
        check(&out, &bytes, case.flags, case.fq_name, &case.constructors);
        for limits in [
            Limits {
                bytes: case.limits.bytes + 1,
                ..case.limits
            },
            Limits {
                records: case.limits.records + 1,
                ..case.limits
            },
            Limits {
                work: case.limits.work + 1,
                ..case.limits
            },
            Limits {
                owned_bytes: case.limits.owned_bytes + 1,
                ..case.limits
            },
        ] {
            assert_eq!(decode(&bytes, limits).unwrap(), out);
        }
        fail(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
            Error {
                offset: 0,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit("bytes")),
            },
        );
        fail(
            &bytes,
            Limits {
                records: case.limits.records - 1,
                ..case.limits
            },
            case.records_error,
        );
        fail(
            &bytes,
            Limits {
                work: case.limits.work - 1,
                ..case.limits
            },
            case.work_error,
        );
        if let Some(error) = case.owned_error {
            fail(
                &bytes,
                Limits {
                    owned_bytes: case.limits.owned_bytes - 1,
                    ..case.limits
                },
                error,
            );
        }
    }
}

#[test]
fn all_477_frozen_prefix_cuts() {
    let mut count = 0;
    for case in cases() {
        let bytes = hex(case.raw);
        for cut in 0..=bytes.len() {
            count += 1;
            let prefix = &bytes[..cut];
            if case.success_cuts.contains(&cut) {
                let (flags, fq) = if case.name == "duplicate_scalars" {
                    (
                        Some(if cut < 10 { 1 } else { 6 }),
                        if cut < 12 { 1 } else { 0 },
                    )
                } else {
                    (case.flags, case.fq_name)
                };
                check(
                    &decode(prefix, Default::default()).unwrap(),
                    prefix,
                    flags,
                    fq,
                    &case.constructors,
                );
            } else {
                let kind = if cut == 0 || case.missing_cuts.contains(&cut) {
                    ErrorKind::Class(class_header::ErrorKind::MissingFqName)
                } else {
                    ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Truncated))
                };
                fail(prefix, Default::default(), Error { offset: cut, kind });
            }
        }
    }
    assert_eq!(count, 477);
}

#[test]
fn frozen_22_faults_and_parent_first_once_rebased_offsets() {
    for (raw, expected) in faults() {
        let bytes = hex(raw);
        fail(&bytes, Default::default(), expected);
        assert!(expected.offset <= bytes.len());
    }
}

fn wrap(body: &[u8]) -> Vec<u8> {
    assert!(body.len() < 128);
    let mut bytes = vec![0x18, 0, 0x42, body.len() as u8];
    bytes.extend(body);
    bytes
}
fn single(bytes: &[u8], flags: Option<i32>, fq: i32, expected: ExpectedConstructor) {
    check(
        &decode(bytes, Default::default()).unwrap(),
        bytes,
        flags,
        fq,
        &[expected],
    );
}

#[test]
fn frozen_signed_scalar_presence_and_packed_unpacked_controls() {
    for (raw, value) in scalars() {
        let scalar = hex(raw);
        let mut parent_flags = vec![8];
        parent_flags.extend(&scalar);
        parent_flags.extend([0x18, 0, 0x42, 0]);
        let end = parent_flags.len();
        single(
            &parent_flags,
            Some(value),
            0,
            ExpectedConstructor {
                span: end..end,
                flags: None,
                parameters: vec![],
                versions: vec![],
            },
        );
        let mut parent_fq = vec![0x18];
        parent_fq.extend(&scalar);
        parent_fq.extend([0x42, 0]);
        let end = parent_fq.len();
        single(
            &parent_fq,
            None,
            value,
            ExpectedConstructor {
                span: end..end,
                flags: None,
                parameters: vec![],
                versions: vec![],
            },
        );
        for tag in [vec![8], vec![0xf8, 1], vec![0xfa, 1]] {
            let mut body = tag.clone();
            if tag == [0xfa, 1] {
                body.push(scalar.len() as u8);
            }
            body.extend(&scalar);
            let bytes = wrap(&body);
            let is_flags = tag == [8];
            single(
                &bytes,
                None,
                0,
                ExpectedConstructor {
                    span: 4..bytes.len(),
                    flags: if is_flags { Some(value) } else { None },
                    parameters: vec![],
                    versions: if is_flags { vec![] } else { vec![value] },
                },
            );
        }
    }
}

#[test]
fn frozen_wrong_wire_interleaving_and_opaque_parameters() {
    let baseline = hex("08 06 12 02 10 05 f8 01 07");
    for raw in [
        "09 00 00 00 00 00 00 00 00",
        "0a 01 80",
        "0d 00 00 00 00",
        "10 00",
        "11 00 00 00 00 00 00 00 00",
        "15 00 00 00 00",
        "f9 01 00 00 00 00 00 00 00 00",
        "fd 01 00 00 00 00",
    ] {
        let wrong = hex(raw);
        let bytes = wrap(&wrong);
        single(
            &bytes,
            None,
            0,
            ExpectedConstructor {
                span: 4..bytes.len(),
                flags: None,
                parameters: vec![],
                versions: vec![],
            },
        );
        for before in [true, false] {
            let (body, shift) = if before {
                (
                    [wrong.as_slice(), baseline.as_slice()].concat(),
                    wrong.len(),
                )
            } else {
                ([baseline.as_slice(), wrong.as_slice()].concat(), 0)
            };
            let bytes = wrap(&body);
            single(
                &bytes,
                None,
                0,
                ExpectedConstructor {
                    span: 4..bytes.len(),
                    flags: Some(6),
                    parameters: vec![(shift + 4..shift + 6, "10 05")],
                    versions: vec![7],
                },
            );
        }
    }
    let bytes = hex("18 00 42 03 12 01 00");
    single(
        &bytes,
        None,
        0,
        ExpectedConstructor {
            span: 4..7,
            flags: None,
            parameters: vec![(2..3, "00")],
            versions: vec![],
        },
    );
    let bytes = hex("18 00 42 03 fa 01 00");
    let out = decode(
        &bytes,
        Limits {
            bytes: 7,
            records: 5,
            work: 27 + O,
            owned_bytes: O,
        },
    )
    .unwrap();
    assert!(
        out.constructors[0]
            .constructor
            .version_requirements
            .is_empty()
    );
}

#[test]
fn shared_failure_charges_and_empty_child_event_offsets() {
    let bytes = hex("18 00 42 00");
    let offsets = [0, 0, 1, 0, 2, 2, 3, 2, 0, 0, 1, 0, 2, 2, 3, 2];
    for (work, offset) in offsets.into_iter().enumerate() {
        let wire = wire::ErrorKind::Limit("work");
        let kind = if work < 8 {
            ErrorKind::Class(class_header::ErrorKind::Wire(wire))
        } else {
            ErrorKind::Wire(wire)
        };
        fail(
            &bytes,
            Limits {
                work,
                owned_bytes: O,
                ..Default::default()
            },
            Error { offset, kind },
        );
    }
    for (owned_bytes, kind) in [(0, "owned_bytes"), (O, "work")] {
        fail(
            &bytes,
            Limits {
                work: 16,
                owned_bytes,
                ..Default::default()
            },
            Error {
                offset: 2,
                kind: ErrorKind::Wire(wire::ErrorKind::Limit(kind)),
            },
        );
    }
    fail(
        &bytes,
        Limits {
            records: 0,
            work: 0,
            ..Default::default()
        },
        Error {
            offset: 0,
            kind: ErrorKind::Class(class_header::ErrorKind::Wire(wire::ErrorKind::Limit(
                "records",
            ))),
        },
    );
    fail(
        &bytes,
        Limits {
            records: 2,
            work: 8,
            ..Default::default()
        },
        Error {
            offset: 0,
            kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
        },
    );
    for (raw, records, work, owned, offset) in [
        ("18 00 42 04 fa 01 01 80 08 06", 7, 37, 0, 8),
        ("18 00 42 00 42 01 80", 7, 29 + O, O, 7),
        ("18 00 42 02 12 00 42 01 00", 8, 36 + S + O, S + O, 8),
    ] {
        let bytes = hex(raw);
        let limits = Limits {
            bytes: bytes.len(),
            records,
            work,
            owned_bytes: owned,
        };
        let kind = if offset == 8 && owned != 0 {
            wire::ErrorKind::InvalidFieldNumber
        } else {
            wire::ErrorKind::Truncated
        };
        fail(
            &bytes,
            limits,
            Error {
                offset,
                kind: ErrorKind::Constructor(kind),
            },
        );
        fail(
            &bytes,
            Limits {
                work: work - 1,
                ..limits
            },
            Error {
                offset,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit("work")),
            },
        );
    }
    let packed = hex("18 00 42 04 fa 01 01 80 08 06");
    for limits in [
        Limits {
            records: 6,
            work: 37,
            owned_bytes: 0,
            ..Default::default()
        },
        Limits {
            records: 7,
            work: 35,
            owned_bytes: 0,
            ..Default::default()
        },
    ] {
        let label = if limits.records == 6 {
            "records"
        } else {
            "work"
        };
        fail(
            &packed,
            limits,
            Error {
                offset: 7,
                kind: ErrorKind::Constructor(wire::ErrorKind::Limit(label)),
            },
        );
    }
    fail(
        &hex("18 00 42 00 42 01 80"),
        Limits {
            owned_bytes: O - 1,
            ..Default::default()
        },
        Error {
            offset: 2,
            kind: ErrorKind::Wire(wire::ErrorKind::Limit("owned_bytes")),
        },
    );
}

#[test]
fn caller_limits_cannot_raise_hard_caps() {
    let raised = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for case in cases() {
        let bytes = hex(case.raw);
        assert_eq!(decode(&bytes, raised), decode(&bytes, Default::default()));
    }
    let mut bytes = vec![0x18, 0];
    for _ in 0..32_766 {
        bytes.extend([0x40, 0]);
    }
    let out = decode(
        &bytes,
        Limits {
            bytes: 65_534,
            records: 65_534,
            work: 262_136,
            owned_bytes: 0,
        },
    )
    .unwrap();
    assert!(out.constructors.is_empty());
    bytes.extend([0x40, 0]);
    let error = Error {
        offset: 65_534,
        kind: ErrorKind::Wire(wire::ErrorKind::Limit("records")),
    };
    fail(&bytes, raised, error);
    fail(&bytes, Default::default(), error);
    let oversized = vec![0; ktlsp_binary::MAX_BYTES + 1];
    let error = Error {
        offset: 0,
        kind: ErrorKind::Wire(wire::ErrorKind::Limit("bytes")),
    };
    fail(&oversized, raised, error);
    fail(
        &oversized,
        Limits {
            records: 0,
            work: 0,
            owned_bytes: 0,
            ..Default::default()
        },
        error,
    );
}

#[test]
fn genuine_pair_class_pipeline_and_two_opaque_parameter_tuples() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported Class metadata");
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload_span, 47..285);
    assert_eq!(
        framed.payload,
        hex(include_str!(
            "fixtures/metadata_class_constructors/pair_class.hex"
        ))
    );
    let case = cases()
        .into_iter()
        .find(|case| case.name == "pair_class")
        .unwrap();
    let out = decode(framed.payload, case.limits).unwrap();
    check(&out, framed.payload, Some(1031), 0, &case.constructors);
    assert_eq!(out.header.raw.as_ptr(), bytes[47..].as_ptr());
    let child = &out.constructors[0];
    assert_eq!(child.body_span, 31..56);
    assert_eq!(child.constructor.raw.as_ptr(), bytes[78..103].as_ptr());
    for (parameter, name, start, raw) in [
        (
            &child.constructor.value_parameters[0],
            5,
            82,
            "10 05 1a 02 38 00",
        ),
        (
            &child.constructor.value_parameters[1],
            6,
            90,
            "10 06 1a 02 38 01",
        ),
    ] {
        assert_eq!(parameter.raw, hex(raw));
        assert_eq!(parameter.raw.as_ptr(), bytes[start..start + 6].as_ptr());
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
fn finite_hostile_inputs_are_deterministic_bounded_and_borrowed() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 1024,
        owned_bytes: 1024,
    };
    let probe = |bytes: &[u8]| {
        let result = decode(bytes, limits);
        assert_eq!(result, decode(bytes, limits));
        match result {
            Ok(out) => {
                assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
                for child in out.constructors {
                    assert!(
                        child.body_span.start <= child.body_span.end
                            && child.body_span.end <= bytes.len()
                    );
                    assert_eq!(
                        child.constructor.raw.as_ptr(),
                        bytes[child.body_span.clone()].as_ptr()
                    );
                    for parameter in child.constructor.value_parameters {
                        assert!(
                            parameter.span.start <= parameter.span.end
                                && parameter.span.end <= child.constructor.raw.len()
                        );
                        assert_eq!(
                            parameter.raw.as_ptr(),
                            child.constructor.raw[parameter.span].as_ptr()
                        );
                    }
                }
            }
            Err(error) => assert!(error.offset <= bytes.len()),
        }
    };
    for value in 0..=u16::MAX {
        probe(&value.to_be_bytes());
    }
    for byte in 0..=u8::MAX {
        for len in 0..32 {
            probe(&vec![byte; len]);
        }
    }
    for byte in 0..=u8::MAX {
        probe(&[0x18, 0, 0x42, 1, byte]);
        probe(&[0x18, 0, 0x42, 3, 0x12, 1, byte]);
    }
}
