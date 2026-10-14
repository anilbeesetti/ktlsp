use ktlsp_binary::metadata::{
    self, function_header,
    function_parameters::{
        Error, ErrorKind, FunctionWithParameters, Limits, SelectedParameter, decode,
    },
    value_parameter,
    wire::{self, ErrorKind as WireKind, Value},
};
use std::{mem::size_of, ops::Range};

const S: usize = size_of::<SelectedParameter<'_>>();
type Parent = (Option<i32>, Option<i32>, i32);
type Parameter = (Option<i32>, i32, Option<i32>, Option<i32>);
type Child = (usize, usize, Parameter);

struct Fixture {
    name: &'static str,
    text: &'static str,
    parent: Parent,
    children: &'static [Child],
    records: usize,
    work: usize,
    slots: usize,
    success_cuts: &'static [usize],
    missing_cuts: &'static [usize],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "minimal",
        text: include_str!("fixtures/metadata_function_parameters/minimal.hex"),
        parent: (None, None, 0),
        children: &[],
        records: 2,
        work: 9,
        slots: 0,
        success_cuts: &[2],
        missing_cuts: &[0],
    },
    Fixture {
        name: "selected",
        text: include_str!("fixtures/metadata_function_parameters/selected.hex"),
        parent: (None, None, 0),
        children: &[(4, 6, (None, 1, None, None))],
        records: 5,
        work: 26,
        slots: 1,
        success_cuts: &[2, 6],
        missing_cuts: &[0],
    },
    Fixture {
        name: "duplicates",
        text: include_str!("fixtures/metadata_function_parameters/duplicates.hex"),
        parent: (None, None, 4),
        children: &[
            (4, 6, (None, 1, None, None)),
            (10, 12, (None, 1, None, None)),
        ],
        records: 12,
        work: 59,
        slots: 2,
        success_cuts: &[2, 6, 8, 12, 14],
        missing_cuts: &[0],
    },
    Fixture {
        name: "wrong_wire",
        text: include_str!("fixtures/metadata_function_parameters/wrong_wire.hex"),
        parent: (None, None, 0),
        children: &[(20, 22, (None, 7, None, None))],
        records: 11,
        work: 70,
        slots: 1,
        success_cuts: &[2, 4, 13, 18, 22],
        missing_cuts: &[0],
    },
    Fixture {
        name: "opaque",
        text: include_str!("fixtures/metadata_function_parameters/opaque.hex"),
        parent: (None, None, 0),
        children: &[(7, 18, (None, 2, None, None))],
        records: 12,
        work: 79,
        slots: 1,
        success_cuts: &[2, 5, 18, 21],
        missing_cuts: &[0],
    },
    Fixture {
        name: "signed",
        text: include_str!("fixtures/metadata_function_parameters/signed.hex"),
        parent: (Some(0), Some(0), -1),
        children: &[(12, 32, (Some(-1), 0, Some(-2147483648), Some(-1)))],
        records: 12,
        work: 110,
        slots: 1,
        success_cuts: &[8, 10, 32],
        missing_cuts: &[0, 2],
    },
    Fixture {
        name: "nonminimal",
        text: include_str!("fixtures/metadata_function_parameters/nonminimal.hex"),
        parent: (None, None, 0),
        children: &[(8, 11, (None, 1, None, None))],
        records: 5,
        work: 37,
        slots: 1,
        success_cuts: &[4, 11],
        missing_cuts: &[0],
    },
    Fixture {
        name: "pair_copy",
        text: include_str!("fixtures/metadata_function_parameters/pair_copy.hex"),
        parent: (Some(65734), None, 17),
        children: &[
            (20, 28, (Some(2), 5, None, None)),
            (30, 38, (Some(2), 6, None, None)),
        ],
        records: 18,
        work: 149,
        slots: 2,
        success_cuts: &[2, 18, 28, 38, 42, 47],
        missing_cuts: &[0],
    },
    Fixture {
        name: "pair_equals",
        text: include_str!("fixtures/metadata_function_parameters/pair_equals.hex"),
        parent: (Some(66006), None, 19),
        children: &[(8, 16, (None, 21, None, None))],
        records: 10,
        work: 70,
        slots: 1,
        success_cuts: &[2, 6, 16, 20],
        missing_cuts: &[0],
    },
];

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}
fn parent(out: &FunctionWithParameters<'_>) -> Parent {
    (out.header.flags, out.header.old_flags, out.header.name)
}
fn scalars(parameter: &value_parameter::ValueParameter<'_>) -> Parameter {
    (
        parameter.flags,
        parameter.name,
        parameter.type_id,
        parameter.vararg_element_type_id,
    )
}
fn borrows(out: &FunctionWithParameters<'_>, bytes: &[u8]) {
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for child in &out.parameters {
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
fn check(bytes: &[u8], limits: Limits, expected: Parent, children: &[Child]) {
    let out = decode(bytes, limits).unwrap();
    borrows(&out, bytes);
    assert_eq!(parent(&out), expected);
    assert_eq!(out.parameters.len(), children.len());
    for (child, &(start, end, expected)) in out.parameters.iter().zip(children) {
        assert_eq!(child.span, start..end);
        assert_eq!(scalars(&child.parameter), expected);
    }
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn minimum(fixture: &Fixture, bytes: usize) -> Limits {
    Limits {
        bytes,
        records: fixture.records,
        nodes: 1 + fixture.children.len(),
        depth: if fixture.children.is_empty() { 1 } else { 2 },
        work: fixture.work + fixture.slots * S,
        owned_bytes: fixture.slots * S,
    }
}

#[test]
fn nine_literal_fixtures_preserve_scalar_presence_order_spans_and_minima() {
    for fixture in FIXTURES {
        let bytes = hex(fixture.text);
        check(&bytes, Default::default(), fixture.parent, fixture.children);
        check(
            &bytes,
            minimum(fixture, bytes.len()),
            fixture.parent,
            fixture.children,
        );
    }
}

#[test]
fn every_applicable_fixture_limit_has_a_frozen_one_less_failure() {
    let failures: &[&[(&str, ErrorKind, usize)]] = &[
        // minimal
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            ("records", ErrorKind::Wire(WireKind::Limit("records")), 0),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 0),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 0),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 0),
        ],
        // selected
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            (
                "records",
                ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
                4,
            ),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 2),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 4),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 4),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                2,
            ),
        ],
        // duplicates
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            ("records", ErrorKind::Wire(WireKind::Limit("records")), 12),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 12),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 10),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 4),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                8,
            ),
        ],
        // wrong_wire
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            (
                "records",
                ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
                20,
            ),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 18),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 20),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 20),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                18,
            ),
        ],
        // opaque
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            ("records", ErrorKind::Wire(WireKind::Limit("records")), 18),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 18),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 7),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 7),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                5,
            ),
        ],
        // signed
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            (
                "records",
                ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
                26,
            ),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 10),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 12),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 12),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                10,
            ),
        ],
        // nonminimal
        &[
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
            (
                "records",
                ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
                8,
            ),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 4),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 8),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 8),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                4,
            ),
        ],
        // pair_copy
        &[
            ("records", ErrorKind::Wire(WireKind::Limit("records")), 42),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 42),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 30),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                28,
            ),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 20),
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
        ],
        // pair_equals
        &[
            ("records", ErrorKind::Wire(WireKind::Limit("records")), 16),
            ("work", ErrorKind::Wire(WireKind::Limit("work")), 16),
            ("nodes", ErrorKind::Wire(WireKind::Limit("nodes")), 8),
            (
                "owned_bytes",
                ErrorKind::Wire(WireKind::Limit("owned_bytes")),
                6,
            ),
            ("depth", ErrorKind::Wire(WireKind::Limit("depth")), 8),
            ("bytes", ErrorKind::Wire(WireKind::Limit("bytes")), 0),
        ],
    ];
    assert_eq!(FIXTURES.len(), failures.len());
    for (fixture, failures) in FIXTURES.iter().zip(failures) {
        let bytes = hex(fixture.text);
        let exact = minimum(fixture, bytes.len());
        for &(label, kind, offset) in *failures {
            let mut limits = exact;
            match label {
                "bytes" => limits.bytes -= 1,
                "records" => limits.records -= 1,
                "work" => limits.work -= 1,
                "nodes" => limits.nodes -= 1,
                "depth" => limits.depth -= 1,
                "owned_bytes" => limits.owned_bytes -= 1,
                _ => unreachable!(),
            }
            error(&bytes, limits, kind, offset);
        }
    }
}

#[test]
fn all_184_byte_prefixes_have_independent_completion_and_error_expectations() {
    let mut count = 0;
    for fixture in FIXTURES {
        let bytes = hex(fixture.text);
        for cut in 0..=bytes.len() {
            count += 1;
            if fixture.success_cuts.contains(&cut) {
                let mut expected = fixture.parent;
                match fixture.name {
                    "duplicates" if cut < 14 => expected.2 = 3,
                    "signed" if cut < 10 => expected.0 = None,
                    "pair_copy" if cut < 42 => expected.0 = None,
                    "pair_equals" if cut < 20 => expected.0 = None,
                    _ => {}
                }
                let children: Vec<_> = fixture
                    .children
                    .iter()
                    .copied()
                    .filter(|c| c.1 <= cut)
                    .collect();
                check(&bytes[..cut], Default::default(), expected, &children);
            } else {
                let kind = if fixture.missing_cuts.contains(&cut) {
                    ErrorKind::Function(function_header::ErrorKind::MissingName)
                } else {
                    ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Truncated))
                };
                error(&bytes[..cut], Default::default(), kind, cut);
            }
        }
    }
    assert_eq!(count, 184);
}

#[test]
fn parent_validation_precedes_children_and_child_faults_rebase_exactly_once() {
    for (text, kind, offset) in [
        (
            "",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            0,
        ),
        (
            "32 01 80",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            3,
        ),
        (
            "0a 01 00 32 01 80",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            6,
        ),
        (
            "12 00 32 01 80",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            5,
        ),
        (
            "10 00 32 00",
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            4,
        ),
        (
            "10 00 32 01 80",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Truncated)),
            5,
        ),
        (
            "10 00 32 01 80 00",
            ErrorKind::Function(function_header::ErrorKind::Wire(
                WireKind::InvalidFieldNumber,
            )),
            5,
        ),
        (
            "10 00 32 01 80 80",
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Truncated)),
            6,
        ),
        (
            "10 00 32 01 80 10 01",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Truncated)),
            5,
        ),
        (
            "10 00 32 01 00",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(
                WireKind::InvalidFieldNumber,
            )),
            4,
        ),
        (
            "10 00 32 01 0b",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::UnsupportedGroup)),
            4,
        ),
        (
            "10 00 32 01 0e",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::InvalidWireType)),
            4,
        ),
        (
            "10 00 32 02 10 80",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Truncated)),
            6,
        ),
        (
            "10 00 32 02 08 00",
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            6,
        ),
        (
            "10 00 32 03 12 02 ff",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Truncated)),
            7,
        ),
        (
            "10 00 32 06 12 80 80 80 80 08",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::NegativeLength)),
            5,
        ),
        (
            "10 00 32 0b 08 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::MalformedVarint)),
            14,
        ),
        (
            "10 00 32 0a 80 80 80 80 80 80 80 80 80 80",
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::MalformedVarint)),
            13,
        ),
        (
            "10 00 32 02 ff",
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Truncated)),
            5,
        ),
        (
            "10 00 32 02 10 01 32 00",
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            8,
        ),
        (
            "10 00 32 01 80 33",
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::UnsupportedGroup)),
            5,
        ),
        (
            "10 00 32 01 80 36",
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::InvalidWireType)),
            5,
        ),
        (
            "10 00 1a 01 80 22 00 32 00 5a 01 80 a2 06 01 00",
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            9,
        ),
    ] {
        for prefix in ["", "08 00", "a0 06 00"] {
            let mut bytes = hex(prefix);
            let shift = bytes.len();
            bytes.extend_from_slice(&hex(text));
            error(&bytes, Default::default(), kind, offset + shift);
        }
    }
    error(
        &[],
        Limits {
            bytes: 0,
            records: 0,
            nodes: 0,
            depth: 0,
            work: 0,
            owned_bytes: 0,
        },
        ErrorKind::Wire(WireKind::Limit("depth")),
        0,
    );
    assert_eq!(
        function_header::decode(
            &[],
            function_header::Limits {
                bytes: 0,
                records: 0,
                work: 0
            }
        ),
        Err(function_header::Error {
            offset: 0,
            kind: function_header::ErrorKind::MissingName
        })
    );
    assert_eq!(
        value_parameter::decode(
            &[],
            value_parameter::Limits {
                bytes: 0,
                records: 0,
                work: 0
            }
        ),
        Err(value_parameter::Error {
            offset: 0,
            kind: value_parameter::ErrorKind::MissingName
        })
    );
    let old_opaque = hex("10 00 1a 01 80 22 00 32 00 5a 01 80 a2 06 01 00");
    let old = function_header::decode(&old_opaque, Default::default()).unwrap();
    assert_eq!(old.raw.as_ptr(), old_opaque.as_ptr());
    assert_eq!((old.flags, old.old_flags, old.name), (None, None, 0));
}

#[test]
fn work_trace_and_13_competing_failures_freeze_admission_order() {
    let bytes = hex("10 00 32 02 10 01");
    let offsets = [
        0, 0, 0, 1, 0, 2, 2, 3, 4, 4, 2, 0, 0, 1, 0, 2, 2, 3, 4, 4, 2, 4, 4, 4, 5, 4, 2,
    ];
    for (work, offset) in offsets.into_iter().enumerate() {
        let kind = match work {
            1..=10 => {
                ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Limit("work")))
            }
            22..=25 => {
                ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("work")))
            }
            _ => ErrorKind::Wire(WireKind::Limit("work")),
        };
        error(
            &bytes,
            Limits {
                work,
                ..Default::default()
            },
            kind,
            offset,
        );
    }
    for (text, limits, kind, offset) in [
        (
            "10 00 32 02 ff",
            Limits {
                work: 9,
                ..Default::default()
            },
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Limit("work"))),
            4,
        ),
        (
            "10 00 32 02 ff",
            Limits {
                work: 10,
                ..Default::default()
            },
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Truncated)),
            5,
        ),
        (
            "10 00 32 01 80",
            Limits {
                work: 22,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("work"))),
            5,
        ),
        (
            "10 00 32 01 80",
            Limits {
                work: 23,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Truncated)),
            5,
        ),
        (
            "10 00 32 02 08 00",
            Limits {
                work: 25,
                owned_bytes: 0,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("work"))),
            4,
        ),
        (
            "10 00 32 02 08 00",
            Limits {
                work: 26,
                owned_bytes: 0,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            6,
        ),
        (
            "10 00 32 00",
            Limits {
                work: 17,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("work")),
            4,
        ),
        (
            "10 00 32 00",
            Limits {
                work: 18,
                owned_bytes: 0,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::MissingName),
            4,
        ),
        (
            "10 00 32 02 10 01",
            Limits {
                records: 0,
                work: 1,
                ..Default::default()
            },
            ErrorKind::Function(function_header::ErrorKind::Wire(WireKind::Limit("records"))),
            0,
        ),
        (
            "10 00 32 02 10 01",
            Limits {
                records: 4,
                work: 22,
                ..Default::default()
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
            4,
        ),
        (
            "10 00 32 02 10 01",
            Limits {
                work: 26,
                owned_bytes: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("owned_bytes")),
            2,
        ),
        (
            "10 00 32 02 10 01",
            Limits {
                nodes: 1,
                records: 4,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("nodes")),
            4,
        ),
        (
            "10 00 32 02 10 01",
            Limits {
                nodes: 0,
                depth: 0,
                records: 0,
                work: 0,
                owned_bytes: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("depth")),
            0,
        ),
    ] {
        error(&hex(text), limits, kind, offset);
    }
}

#[test]
fn signed_scalars_keep_presence_and_last_recognized_values() {
    for (encoded, value) in [
        ("00", 0),
        ("01", 1),
        ("7f", 127),
        ("ff ff ff ff 07", 2147483647),
        ("80 80 80 80 08", -2147483648),
        ("ff ff ff ff 0f", -1),
        ("ff ff ff ff ff ff ff ff ff 01", -1),
        ("80 80 80 80 10", 0),
        ("81 00", 1),
    ] {
        let value_bytes = hex(encoded);
        for tag in [0x08, 0x10, 0x28, 0x30] {
            let mut body = hex("10 00");
            body.push(tag);
            body.extend_from_slice(&value_bytes);
            let mut bytes = vec![0x10, 0, 0x32, body.len() as u8];
            bytes.extend_from_slice(&body);
            let mut expected = (None, 0, None, None);
            match tag {
                0x08 => expected.0 = Some(value),
                0x10 => expected.1 = value,
                0x28 => expected.2 = Some(value),
                0x30 => expected.3 = Some(value),
                _ => unreachable!(),
            }
            check(
                &bytes,
                Default::default(),
                (None, None, 0),
                &[(4, bytes.len(), expected)],
            );
        }
        for tag in [0x08, 0x10, 0x48] {
            let mut bytes = vec![0x10, 0, tag];
            bytes.extend_from_slice(&value_bytes);
            let start = bytes.len() + 2;
            bytes.extend_from_slice(&hex("32 02 10 00"));
            let mut expected = (None, None, 0);
            match tag {
                0x08 => expected.1 = Some(value),
                0x10 => expected.2 = value,
                0x48 => expected.0 = Some(value),
                _ => unreachable!(),
            }
            check(
                &bytes,
                Default::default(),
                expected,
                &[(start, start + 2, (None, 0, None, None))],
            );
        }
    }
    check(
        &hex("10 00 32 14 08 00 08 01 10 00 10 01 28 00 28 01 30 00 30 01 0a 00 12 00"),
        Default::default(),
        (None, None, 0),
        &[(4, 24, (Some(1), 1, Some(1), Some(1)))],
    );
    check(
        &hex("10 00 b2 80 80 80 10 82 80 80 80 10 10 01"),
        Limits {
            bytes: 14,
            records: 5,
            nodes: 2,
            depth: 2,
            work: 42 + S,
            owned_bytes: S,
        },
        (None, None, 0),
        &[(12, 14, (None, 1, None, None))],
    );
}

#[test]
fn ordered_parameters_share_capacity_growth_and_hard_clamps() {
    for k in 1usize..=5 {
        let mut bytes = hex("10 00");
        let mut children = Vec::new();
        for i in 0..k {
            bytes.extend_from_slice(&hex("32 02 10 00"));
            children.push((4 + 4 * i, 6 + 4 * i, (None, 0, None, None)));
        }
        let q = k.next_power_of_two();
        let exact = Limits {
            bytes: bytes.len(),
            records: 2 + 3 * k,
            nodes: 1 + k,
            depth: 2,
            work: 9 + 17 * k + q * S,
            owned_bytes: q * S,
        };
        check(&bytes, exact, (None, None, 0), &children);
        error(
            &bytes,
            Limits {
                records: exact.records - 1,
                ..exact
            },
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("records"))),
            4 * k,
        );
        error(
            &bytes,
            Limits {
                nodes: exact.nodes - 1,
                ..exact
            },
            ErrorKind::Wire(WireKind::Limit("nodes")),
            4 * k,
        );
        error(
            &bytes,
            Limits { depth: 1, ..exact },
            ErrorKind::Wire(WireKind::Limit("depth")),
            4,
        );
        error(
            &bytes,
            Limits {
                bytes: bytes.len() - 1,
                ..exact
            },
            ErrorKind::Wire(WireKind::Limit("bytes")),
            0,
        );
        error(
            &bytes,
            Limits {
                owned_bytes: exact.owned_bytes - 1,
                ..exact
            },
            ErrorKind::Wire(WireKind::Limit("owned_bytes")),
            [2, 6, 10, 10, 18][k - 1],
        );
        let kind = if k == 4 {
            ErrorKind::Parameter(value_parameter::ErrorKind::Wire(WireKind::Limit("work")))
        } else {
            ErrorKind::Wire(WireKind::Limit("work"))
        };
        error(
            &bytes,
            Limits {
                work: exact.work - 1,
                ..exact
            },
            kind,
            [2, 6, 10, 16, 18][k - 1],
        );
    }
    let max = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    check(&hex("10 00"), max, (None, None, 0), &[]);
    error(
        &vec![0; ktlsp_binary::MAX_BYTES + 1],
        max,
        ErrorKind::Wire(WireKind::Limit("bytes")),
        0,
    );
    const {
        assert!(32768 * S <= ktlsp_binary::MAX_BYTES);
        assert!(371357 + 32768 * S <= ktlsp_binary::MAX_VALIDATION_UNITS);
    }
    let mut bytes = hex("10 00");
    for _ in 0..21844 {
        bytes.extend_from_slice(&hex("32 02 10 00"));
    }
    let out = decode(
        &bytes,
        Limits {
            bytes: 87378,
            records: 65534,
            nodes: 21845,
            depth: 2,
            work: 371357 + 32768 * S,
            owned_bytes: 32768 * S,
        },
    )
    .unwrap();
    assert_eq!(out.parameters.len(), 21844);
    borrows(&out, &bytes);
    for (i, parameter) in out.parameters.iter().enumerate() {
        assert_eq!(parameter.span, 4 + 4 * i..6 + 4 * i);
        assert_eq!(scalars(&parameter.parameter), (None, 0, None, None));
    }
    bytes.extend_from_slice(&hex("32 02 10 00"));
    error(
        &bytes,
        max,
        ErrorKind::Wire(WireKind::Limit("records")),
        87378,
    );
}

#[test]
fn genuine_pair_copy_and_equals_borrow_actual_decoded_pipeline_slices() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata")
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let framed = wire::decode(&bytes, Default::default()).unwrap();
    assert_eq!(framed.payload.span, 47..285);
    for (name, field_span, body_span) in [
        ("pair_copy", 149..198, 151..198),
        ("pair_equals", 198..220, 200..220),
    ] {
        let fixture = FIXTURES.iter().find(|f| f.name == name).unwrap();
        let field = framed
            .payload
            .fields
            .iter()
            .find(|f| f.span == field_span)
            .unwrap();
        assert_eq!(field.number, 9);
        let Value::Bytes(body) = field.value else {
            panic!("Function body")
        };
        assert_eq!(body, hex(fixture.text));
        assert_eq!(body, &bytes[body_span.clone()]);
        assert_eq!(body.as_ptr(), bytes[body_span.clone()].as_ptr());
        check(
            body,
            minimum(fixture, body.len()),
            fixture.parent,
            fixture.children,
        );
        let out = decode(body, Default::default()).unwrap();
        assert_eq!(out.header.raw.as_ptr(), bytes[body_span.clone()].as_ptr());
        for (child, &(start, end, _)) in out.parameters.iter().zip(fixture.children) {
            assert_eq!(
                child.parameter.raw.as_ptr(),
                bytes[body_span.start + start..body_span.start + end].as_ptr()
            );
        }
    }
}

#[test]
fn finite_hostile_parents_and_children_are_deterministic_and_bounded() {
    let limits = Limits {
        bytes: 32,
        records: 16,
        nodes: 8,
        depth: 2,
        work: 256,
        owned_bytes: 4096,
    };
    let probe = |bytes: &[u8], selected: Option<Range<usize>>| {
        let a = decode(bytes, limits);
        assert_eq!(a, decode(bytes, limits));
        match a {
            Ok(out) => {
                borrows(&out, bytes);
                if let Some(span) = selected {
                    assert_eq!(out.header.name, 0);
                    assert_eq!(out.parameters.len(), 1);
                    assert_eq!(out.parameters[0].span, span);
                }
            }
            Err(error) => {
                assert!(error.offset <= bytes.len());
                if selected.is_some() {
                    assert!(
                        (4..=bytes.len()).contains(&error.offset)
                            || error
                                == Error {
                                    offset: 2,
                                    kind: ErrorKind::Wire(WireKind::Limit("allocation"))
                                }
                    );
                }
            }
        }
    };
    let mut count = 0;
    for value in 0u32..=65535 {
        let a = (value >> 8) as u8;
        let b = value as u8;
        probe(&[a, b], None);
        probe(&[0x10, 0, 0x32, 2, a, b], Some(4..6));
        probe(&[0x10, 0, 0x32, 4, 0x10, 0, a, b], Some(4..8));
        count += 3;
    }
    for byte in 0u8..=255 {
        for len in 0..=15 {
            probe(&vec![byte; len], None);
            let mut bytes = vec![0x10, 0, 0x32, (2 + len) as u8, 0x10, 0];
            bytes.extend_from_slice(&vec![byte; len]);
            probe(&bytes, Some(4..6 + len));
            count += 2;
        }
    }
    assert_eq!(count, 204800);
}
