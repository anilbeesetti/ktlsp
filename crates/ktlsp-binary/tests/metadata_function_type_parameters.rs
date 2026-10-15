use ktlsp_binary::metadata::{
    self, function_header,
    function_type_parameters::{
        Error, ErrorKind, FunctionWithTypeParameters, Limits, SelectedTypeParameter, decode,
    },
    type_parameter::{self, UpperBound, Variance},
    wire::{self, ErrorKind as W, Value},
};
use std::mem::size_of;

const S: usize = size_of::<SelectedTypeParameter<'_>>();
const B: usize = size_of::<UpperBound<'_>>();

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(bytes, limits), Err(Error { offset, kind }));
}
fn borrows(out: &FunctionWithTypeParameters<'_>, bytes: &[u8]) {
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    let mut end = 0;
    for selected in &out.parameters {
        assert!(end <= selected.span.start && selected.span.end <= bytes.len());
        let child = &selected.parameter;
        assert_eq!(child.raw, &bytes[selected.span.clone()]);
        assert_eq!(child.raw.as_ptr(), bytes[selected.span.clone()].as_ptr());
        for bound in &child.upper_bound {
            assert!(bound.span.end <= child.raw.len());
            assert_eq!(bound.raw, &child.raw[bound.span.clone()]);
            assert_eq!(
                bound.raw.as_ptr(),
                bytes[selected.span.start + bound.span.start..selected.span.start + bound.span.end]
                    .as_ptr()
            );
        }
        end = selected.span.end;
    }
}

#[test]
fn authored_literals_preserve_selection_order_presence_and_exact_borrowing() {
    for (text, name, spans) in [
        (
            include_str!("fixtures/metadata_function_type_parameters/minimal.hex"),
            0,
            vec![],
        ),
        (
            include_str!("fixtures/metadata_function_type_parameters/selected.hex"),
            1,
            vec![(6, 20)],
        ),
        (
            include_str!("fixtures/metadata_function_type_parameters/duplicates.hex"),
            0,
            vec![(4, 8), (12, 16)],
        ),
        (
            include_str!("fixtures/metadata_function_type_parameters/wrong_wire.hex"),
            0,
            vec![],
        ),
        (
            include_str!("fixtures/metadata_function_type_parameters/opaque.hex"),
            0,
            vec![(14, 20)],
        ),
        (
            include_str!("fixtures/metadata_function_type_parameters/nonminimal.hex"),
            0,
            vec![(8, 13)],
        ),
    ] {
        let bytes = hex(text);
        let out = decode(&bytes, Default::default()).unwrap();
        borrows(&out, &bytes);
        assert_eq!(out.header.name, name);
        assert_eq!(
            out.parameters
                .iter()
                .map(|p| (p.span.start, p.span.end))
                .collect::<Vec<_>>(),
            spans
        );
    }
    let bytes = hex(include_str!(
        "fixtures/metadata_function_type_parameters/selected.hex"
    ));
    let out = decode(
        &bytes,
        Limits {
            bytes: 22,
            records: 14,
            nodes: 2,
            depth: 2,
            work: 93 + B + S,
            owned_bytes: B + 4 + S,
        },
    )
    .unwrap();
    assert_eq!((out.header.old_flags, out.header.flags), (Some(0), Some(6)));
    let child = &out.parameters[0].parameter;
    assert_eq!(
        (child.id, child.name, child.reified, child.variance),
        (2, 3, Some(true), Some(Variance::Out))
    );
    assert_eq!(child.upper_bound_id, [7]);
    assert_eq!(child.upper_bound[0].span, 10..12);
    assert_eq!(child.upper_bound[0].raw, [0x0b, 0xff]);
    borrows(&out, &bytes);
}

#[test]
fn parent_validation_precedes_selected_child_failures() {
    for (text, kind, offset) in [
        (
            "",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            0,
        ),
        (
            "22 00",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            2,
        ),
        (
            "10 00 22 00 80",
            ErrorKind::Function(function_header::ErrorKind::Wire(W::Truncated)),
            5,
        ),
        (
            "10 00 22 00 00",
            ErrorKind::Function(function_header::ErrorKind::Wire(W::InvalidFieldNumber)),
            4,
        ),
        (
            "10 00 22 00 23",
            ErrorKind::Function(function_header::ErrorKind::Wire(W::UnsupportedGroup)),
            4,
        ),
        (
            "10 00 22 00 26",
            ErrorKind::Function(function_header::ErrorKind::Wire(W::InvalidWireType)),
            4,
        ),
        (
            "10 00 22 02 ff",
            ErrorKind::Function(function_header::ErrorKind::Wire(W::Truncated)),
            5,
        ),
        (
            "12 00 22 00",
            ErrorKind::Function(function_header::ErrorKind::MissingName),
            4,
        ),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
}

#[test]
fn child_errors_rebase_once_and_stay_inside_packed_boundaries() {
    for (text, kind, offset) in [
        ("10 00 22 00", type_parameter::ErrorKind::MissingId, 4),
        ("10 00 22 02 10 00", type_parameter::ErrorKind::MissingId, 6),
        (
            "10 00 22 02 08 00",
            type_parameter::ErrorKind::MissingName,
            6,
        ),
        (
            "10 00 22 03 08 00 80",
            type_parameter::ErrorKind::Wire(W::Truncated),
            7,
        ),
        (
            "10 00 22 01 00",
            type_parameter::ErrorKind::Wire(W::InvalidFieldNumber),
            4,
        ),
        (
            "10 00 22 01 0b",
            type_parameter::ErrorKind::Wire(W::UnsupportedGroup),
            4,
        ),
        (
            "10 00 22 01 0e",
            type_parameter::ErrorKind::Wire(W::InvalidWireType),
            4,
        ),
        (
            "10 00 22 07 08 00 10 00 32 01 80 10 01",
            type_parameter::ErrorKind::Wire(W::Truncated),
            11,
        ),
        (
            "10 00 22 07 08 00 10 00 2a 02 ff",
            type_parameter::ErrorKind::Wire(W::Truncated),
            11,
        ),
        (
            "10 00 22 0a 08 00 10 00 32 80 80 80 80 08",
            type_parameter::ErrorKind::Wire(W::NegativeLength),
            9,
        ),
        (
            "10 00 22 10 08 00 10 00 32 0a 80 80 80 80 80 80 80 80 80 80",
            type_parameter::ErrorKind::Wire(W::MalformedVarint),
            19,
        ),
    ] {
        for prefix in ["", "08 00", "a0 06 00"] {
            let mut bytes = hex(prefix);
            let shift = bytes.len();
            bytes.extend(hex(text));
            error(
                &bytes,
                Default::default(),
                ErrorKind::TypeParameter(kind),
                offset + shift,
            );
        }
    }
}

#[test]
fn signed_scalars_unknown_enums_and_wrong_wire_retain_child_rules() {
    let bytes = hex(
        "08 01 08 02 10 01 10 00 48 01 48 02 22 1e 08 ff ff ff ff 0f 10 fe ff ff ff 0f 18 00 18 02 20 00 20 01 20 03 0a 00 12 00 30 01 32 00",
    );
    let out = decode(&bytes, Default::default()).unwrap();
    assert_eq!(
        (out.header.old_flags, out.header.name, out.header.flags),
        (Some(2), 0, Some(2))
    );
    let p = &out.parameters[0].parameter;
    assert_eq!(
        (p.id, p.name, p.reified, p.variance),
        (-1, -2, Some(true), Some(Variance::Out))
    );
    assert_eq!(p.upper_bound_id, [1]);
    borrows(&out, &bytes);
}

#[test]
fn minimal_siblings_share_all_admissions_and_capacity_growth() {
    for k in 1usize..=5 {
        let mut bytes = hex("10 00");
        for _ in 0..k {
            bytes.extend(hex("22 04 08 00 10 00"));
        }
        let q = k.next_power_of_two();
        let exact = Limits {
            bytes: bytes.len(),
            records: 2 + 4 * k,
            nodes: 1 + k,
            depth: 2,
            work: 9 + 25 * k + q * S,
            owned_bytes: q * S,
        };
        let out = decode(&bytes, exact).unwrap();
        borrows(&out, &bytes);
        assert_eq!(out.parameters.len(), k);
        for (i, p) in out.parameters.iter().enumerate() {
            assert_eq!(p.span, 4 + 6 * i..8 + 6 * i);
        }
        error(
            &bytes,
            Limits {
                bytes: exact.bytes - 1,
                ..exact
            },
            ErrorKind::Wire(W::Limit("bytes")),
            0,
        );
        error(
            &bytes,
            Limits {
                records: exact.records - 1,
                ..exact
            },
            ErrorKind::TypeParameter(type_parameter::ErrorKind::Wire(W::Limit("records"))),
            6 * k,
        );
        error(
            &bytes,
            Limits {
                nodes: exact.nodes - 1,
                ..exact
            },
            ErrorKind::Wire(W::Limit("nodes")),
            6 * k - 2,
        );
        error(
            &bytes,
            Limits { depth: 1, ..exact },
            ErrorKind::Wire(W::Limit("depth")),
            4,
        );
        error(
            &bytes,
            Limits {
                owned_bytes: exact.owned_bytes - 1,
                ..exact
            },
            ErrorKind::Wire(W::Limit("owned_bytes")),
            [2, 8, 14, 14, 26][k - 1],
        );
        assert!(matches!(
            decode(
                &bytes,
                Limits {
                    work: exact.work - 1,
                    ..exact
                }
            ),
            Err(Error {
                kind: ErrorKind::Wire(W::Limit("work"))
                    | ErrorKind::TypeParameter(type_parameter::ErrorKind::Wire(W::Limit("work"))),
                ..
            })
        ));
    }
}

#[test]
fn child_vectors_and_outer_vector_share_storage_and_work() {
    // Two children each retain a bound and one ID; no per-child reset.
    let bytes = hex("10 00 22 08 08 00 10 00 2a 00 30 07 22 08 08 00 10 00 2a 00 30 07");
    let owned = 2 * (B + 4 + S);
    let exact = Limits {
        bytes: 22,
        records: 14,
        nodes: 3,
        depth: 2,
        work: 93 + owned,
        owned_bytes: owned,
    };
    let out = decode(&bytes, exact).unwrap();
    borrows(&out, &bytes);
    assert_eq!(out.parameters.len(), 2);
    for p in &out.parameters {
        assert_eq!(p.parameter.upper_bound_id, [7]);
        assert_eq!(p.parameter.upper_bound[0].span, 6..6);
    }
    error(
        &bytes,
        Limits {
            owned_bytes: owned - 1,
            ..exact
        },
        ErrorKind::Wire(W::Limit("owned_bytes")),
        12,
    );
    error(
        &bytes,
        Limits {
            owned_bytes: B + 4 + S + B + 3,
            ..exact
        },
        ErrorKind::TypeParameter(type_parameter::ErrorKind::Wire(W::Limit("owned_bytes"))),
        20,
    );
    error(
        &bytes,
        Limits {
            work: exact.work - 1,
            ..exact
        },
        ErrorKind::Wire(W::Limit("work")),
        12,
    );
}

#[test]
fn packed_ids_and_opaque_bounds_share_budget_without_recursing() {
    let bytes = hex("10 00 22 12 08 00 10 00 30 01 32 02 02 03 32 00 30 04 2a 02 0b ff");
    let exact = Limits {
        bytes: 22,
        records: 13,
        nodes: 2,
        depth: 2,
        work: 94 + B + 16 + S,
        owned_bytes: B + 16 + S,
    };
    let out = decode(&bytes, exact).unwrap();
    borrows(&out, &bytes);
    let p = &out.parameters[0].parameter;
    assert_eq!(p.upper_bound_id, [1, 2, 3, 4]);
    assert_eq!(p.upper_bound[0].span, 16..18);
    error(
        &bytes,
        Limits {
            records: exact.records - 1,
            ..exact
        },
        ErrorKind::TypeParameter(type_parameter::ErrorKind::Wire(W::Limit("records"))),
        18,
    );
    error(
        &bytes,
        Limits {
            owned_bytes: exact.owned_bytes - 1,
            ..exact
        },
        ErrorKind::Wire(W::Limit("owned_bytes")),
        2,
    );
}

#[test]
fn empty_selection_and_admission_precedence_have_exact_limits() {
    let bytes = hex("10 00");
    let exact = Limits {
        bytes: 2,
        records: 2,
        nodes: 1,
        depth: 1,
        work: 9,
        owned_bytes: 0,
    };
    assert!(decode(&bytes, exact).unwrap().parameters.is_empty());
    error(
        &bytes,
        Limits { work: 8, ..exact },
        ErrorKind::Wire(W::Limit("work")),
        0,
    );
    error(
        &bytes,
        Limits {
            records: 1,
            ..exact
        },
        ErrorKind::Wire(W::Limit("records")),
        0,
    );
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
        ErrorKind::Wire(W::Limit("depth")),
        0,
    );
    error(
        &hex("10 00 22 00"),
        Limits {
            depth: 1,
            ..Default::default()
        },
        ErrorKind::Wire(W::Limit("depth")),
        4,
    );
    error(
        &hex("10 00 22 00"),
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        ErrorKind::TypeParameter(type_parameter::ErrorKind::MissingId),
        4,
    );
}

#[test]
fn hard_clamps_keep_caller_maxima_bounded() {
    let max = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    error(
        &vec![0; ktlsp_binary::MAX_BYTES + 1],
        max,
        ErrorKind::Wire(W::Limit("bytes")),
        0,
    );
    let mut bytes = hex("10 00");
    // Two parent scans: the second reaches the hard record ceiling.
    for _ in 0..32767 {
        bytes.extend(hex("38 00"));
    }
    error(&bytes, max, ErrorKind::Wire(W::Limit("records")), 65534);
}

#[test]
fn genuine_pair_copy_is_only_an_empty_selection_control() {
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
    assert_eq!(header.mv, [2, 3, 0]);
    let framed = wire::decode(&bytes, Default::default()).unwrap();
    let field = framed
        .payload
        .fields
        .iter()
        .find(|f| f.span == (149..198))
        .unwrap();
    assert_eq!(field.number, 9);
    let Value::Bytes(raw) = field.value else {
        panic!("Function")
    };
    assert_eq!(raw, &bytes[151..198]);
    let out = decode(raw, Default::default()).unwrap();
    borrows(&out, raw);
    assert_eq!(out.header.name, 17);
    assert!(out.parameters.is_empty());
}

#[test]
fn finite_hostile_inputs_are_deterministic_and_bounded() {
    let limits = Limits {
        bytes: 32,
        records: 16,
        nodes: 4,
        depth: 2,
        work: 256,
        owned_bytes: 4096,
    };
    let probe = |bytes: &[u8]| {
        let a = decode(bytes, limits);
        assert_eq!(a, decode(bytes, limits));
        match a {
            Ok(out) => borrows(&out, bytes),
            Err(e) => assert!(e.offset <= bytes.len()),
        }
    };
    for value in 0u32..=65535 {
        let a = (value >> 8) as u8;
        let b = value as u8;
        probe(&[a, b]);
        probe(&[0x10, 0, 0x22, 2, a, b]);
        probe(&[0x10, 0, 0x22, 6, 0x08, 0, 0x10, 0, a, b]);
    }
}
