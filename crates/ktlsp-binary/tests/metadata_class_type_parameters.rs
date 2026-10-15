use ktlsp_binary::metadata::{
    self, class_header,
    class_type_parameters::{
        ClassWithTypeParameters, Error, ErrorKind, Limits, SelectedTypeParameter, decode,
    },
    type_parameter::{self, UpperBound, Variance},
    wire::ErrorKind as WireKind,
};
use std::mem::size_of;

const S: usize = size_of::<SelectedTypeParameter<'_>>();
const B: usize = size_of::<UpperBound<'_>>();
type Parent = (Option<i32>, i32);

macro_rules! literal {
    ($name:literal) => {
        include_str!(concat!(
            "fixtures/metadata_class_type_parameters/",
            $name,
            ".hex"
        ))
    };
}
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
#[derive(Clone, Debug)]
struct Child {
    start: usize,
    end: usize,
    id: i32,
    name: i32,
    reified: Option<bool>,
    variance: Option<Variance>,
    ids: &'static [i32],
    bounds: &'static [(usize, usize, &'static [u8])],
}
fn child(start: usize, end: usize, id: i32, name: i32) -> Child {
    Child {
        start,
        end,
        id,
        name,
        reified: None,
        variance: None,
        ids: &[],
        bounds: &[],
    }
}
fn check<'a>(
    bytes: &'a [u8],
    limits: Limits,
    parent: Parent,
    expected: &[Child],
) -> ClassWithTypeParameters<'a> {
    let out = decode(bytes, limits).unwrap();
    assert_eq!((out.header.flags, out.header.fq_name), parent);
    assert_eq!(out.header.raw, bytes);
    assert_eq!(out.header.raw.as_ptr(), bytes.as_ptr());
    assert_eq!(
        out.header,
        class_header::decode(bytes, Default::default()).unwrap()
    );
    assert_eq!(out.type_parameters.len(), expected.len());
    for (selected, e) in out.type_parameters.iter().zip(expected) {
        assert_eq!(selected.span, e.start..e.end);
        let p = &selected.parameter;
        assert_eq!(p.raw, &bytes[e.start..e.end]);
        assert_eq!(p.raw.as_ptr(), bytes[e.start..e.end].as_ptr());
        assert_eq!(
            (p.id, p.name, p.reified, p.variance),
            (e.id, e.name, e.reified, e.variance)
        );
        assert_eq!(p.upper_bound_id, e.ids);
        assert_eq!(p.upper_bound.len(), e.bounds.len());
        for (bound, &(start, end, raw)) in p.upper_bound.iter().zip(e.bounds) {
            assert_eq!(bound.span, start..end);
            assert_eq!(bound.raw, raw);
            assert_eq!(bound.raw, &p.raw[start..end]);
            assert_eq!(bound.raw.as_ptr(), p.raw[start..end].as_ptr());
        }
        assert_eq!(
            *p,
            type_parameter::decode(p.raw, Default::default()).unwrap()
        );
    }
    out
}
fn error(bytes: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert!(offset <= bytes.len());
    assert_eq!(
        decode(bytes, limits),
        Err(Error { offset, kind }),
        "{bytes:02x?}"
    );
    assert_eq!(
        decode(&hex(literal!("selected")), Default::default())
            .unwrap()
            .type_parameters
            .len(),
        1
    );
}
fn cw(kind: WireKind) -> ErrorKind {
    ErrorKind::Class(class_header::ErrorKind::Wire(kind))
}
fn tw(kind: WireKind) -> ErrorKind {
    ErrorKind::TypeParameter(type_parameter::ErrorKind::Wire(kind))
}
fn limit(bytes: &[u8], limits: Limits, label: &'static str, offset: usize) {
    error(
        bytes,
        limits,
        ErrorKind::Wire(WireKind::Limit(label)),
        offset,
    );
}
struct Fixture {
    text: &'static str,
    parent: Parent,
    children: Vec<Child>,
    exact: Limits,
    prefixes: &'static [usize],
    missing: &'static [usize],
}
fn exact(
    bytes: usize,
    records: usize,
    nodes: usize,
    depth: usize,
    work: usize,
    owned_bytes: usize,
) -> Limits {
    Limits {
        bytes,
        records,
        nodes,
        depth,
        work,
        owned_bytes,
    }
}
fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            text: literal!("minimal"),
            parent: (None, 0),
            children: vec![],
            exact: exact(2, 2, 1, 1, 9, 0),
            prefixes: &[2],
            missing: &[0],
        },
        Fixture {
            text: literal!("selected"),
            parent: (None, 0),
            children: vec![child(4, 8, 0, 0)],
            exact: exact(8, 6, 2, 2, 34 + S, S),
            prefixes: &[2, 8],
            missing: &[0],
        },
        Fixture {
            text: literal!("duplicates"),
            parent: (None, 0),
            children: vec![child(4, 8, 0, 0), child(10, 14, 0, 0)],
            exact: exact(14, 10, 3, 2, 59 + 2 * S, 2 * S),
            prefixes: &[2, 8, 14],
            missing: &[0],
        },
        Fixture {
            text: literal!("three"),
            parent: (None, 0),
            children: vec![child(4, 8, 0, 0), child(10, 14, 0, 0), child(16, 20, 0, 0)],
            exact: exact(20, 14, 4, 2, 84 + 4 * S, 4 * S),
            prefixes: &[2, 8, 14, 20],
            missing: &[0],
        },
        Fixture {
            text: literal!("scalars"),
            parent: (None, 0),
            children: vec![Child {
                reified: Some(false),
                variance: Some(Variance::Inv),
                ids: &[-1],
                ..child(4, 26, -1, -2)
            }],
            exact: exact(26, 9, 2, 2, 99 + S, 4 + S),
            prefixes: &[2, 26],
            missing: &[0],
        },
        Fixture {
            text: literal!("presence"),
            parent: (None, 0),
            children: vec![Child {
                reified: Some(false),
                variance: Some(Variance::Out),
                ..child(4, 18, 0, 0)
            }],
            exact: exact(18, 11, 2, 2, 74 + S, S),
            prefixes: &[2, 18],
            missing: &[0],
        },
        Fixture {
            text: literal!("wrong_wire"),
            parent: (None, 0),
            children: vec![],
            exact: exact(9, 6, 1, 1, 31, 0),
            prefixes: &[2, 4, 9],
            missing: &[0],
        },
        Fixture {
            text: literal!("opaque"),
            parent: (None, 0),
            children: vec![Child {
                ids: &[7, 8, 9, 10],
                bounds: &[(16, 16, &[]), (18, 20, &[0x0b, 0xff])],
                ..child(4, 24, 0, 0)
            }],
            exact: exact(24, 14, 2, 2, 118 + 2 * B + S, 16 + 2 * B + S),
            prefixes: &[2, 24],
            missing: &[0],
        },
        Fixture {
            text: literal!("nonminimal"),
            parent: (None, 0),
            children: vec![child(8, 16, 0, 1)],
            exact: exact(16, 6, 2, 2, 54 + S, S),
            prefixes: &[4, 16],
            missing: &[0],
        },
        Fixture {
            text: literal!("pair_class"),
            parent: (Some(1031), 0),
            children: vec![
                Child {
                    variance: Some(Variance::Out),
                    ..child(7, 13, 0, 1)
                },
                Child {
                    variance: Some(Variance::Out),
                    ..child(15, 21, 1, 2)
                },
            ],
            exact: exact(238, 36, 3, 2, 563 + 2 * S, 2 * S),
            prefixes: &[5, 13, 21, 29, 56, 68, 85, 102, 151, 173, 185, 210, 235, 238],
            missing: &[0, 3],
        },
    ]
}

#[test]
fn frozen_literals_exact_aggregate_limits_and_every_byte_prefix() {
    for f in fixtures() {
        let bytes = hex(f.text);
        let l = f.exact;
        assert_eq!(bytes.len(), l.bytes);
        for limits in [
            l,
            Limits {
                bytes: l.bytes + 1,
                ..l
            },
            Limits {
                records: l.records + 1,
                ..l
            },
            Limits {
                nodes: l.nodes + 1,
                ..l
            },
            Limits {
                depth: l.depth + 1,
                ..l
            },
            Limits {
                work: l.work + 1,
                ..l
            },
            Limits {
                owned_bytes: l.owned_bytes + 1,
                ..l
            },
        ] {
            check(&bytes, limits, f.parent, &f.children);
        }
        for n in 0..=bytes.len() {
            if f.prefixes.contains(&n) {
                let children: Vec<_> = f.children.iter().filter(|c| c.end <= n).cloned().collect();
                check(&bytes[..n], Default::default(), f.parent, &children);
            } else if f.missing.contains(&n) {
                error(
                    &bytes[..n],
                    Default::default(),
                    ErrorKind::Class(class_header::ErrorKind::MissingFqName),
                    n,
                );
            } else {
                error(&bytes[..n], Default::default(), cw(WireKind::Truncated), n);
            }
        }
    }
}

#[test]
fn parent_child_error_provenance_and_required_precedence() {
    use class_header::ErrorKind::MissingFqName;
    use type_parameter::ErrorKind::{MissingId, MissingName};
    for (text, kind, offset) in [
        ("", ErrorKind::Class(MissingFqName), 0),
        ("08 06", ErrorKind::Class(MissingFqName), 2),
        ("1a 00", ErrorKind::Class(MissingFqName), 2),
        ("2a 01 00", ErrorKind::Class(MissingFqName), 3),
        ("18 00 2a 00", ErrorKind::TypeParameter(MissingId), 4),
        (
            "18 00 2a 02 08 00",
            ErrorKind::TypeParameter(MissingName),
            6,
        ),
        ("18 00 2a 02 10 00", ErrorKind::TypeParameter(MissingId), 6),
        ("18 00 2a 01 00", tw(WireKind::InvalidFieldNumber), 4),
        ("18 00 2a 01 80", tw(WireKind::Truncated), 5),
        ("18 00 2a 01 0b", tw(WireKind::UnsupportedGroup), 4),
        ("18 00 2a 01 0e", tw(WireKind::InvalidWireType), 4),
        ("18 00 2a 01 00 00", cw(WireKind::InvalidFieldNumber), 5),
        ("18 00 2a 00 00", cw(WireKind::InvalidFieldNumber), 4),
        ("18 00 2b", cw(WireKind::UnsupportedGroup), 2),
        ("18 00 2e", cw(WireKind::InvalidWireType), 2),
        ("18 00 2a 80", cw(WireKind::Truncated), 4),
        ("18 00 2a ff ff ff ff 0f", cw(WireKind::NegativeLength), 3),
        (
            "18 00 2a 80 80 80 80 80 80 80 80 80 80",
            cw(WireKind::MalformedVarint),
            12,
        ),
        ("18 00 2a 02 30 07", ErrorKind::TypeParameter(MissingId), 6),
        (
            "18 00 2a 07 08 00 10 00 2a 00 00",
            tw(WireKind::InvalidFieldNumber),
            10,
        ),
    ] {
        let bytes = hex(text);
        error(&bytes, Default::default(), kind, offset);
        if matches!(kind, ErrorKind::Class(_)) {
            error(
                &bytes,
                Limits {
                    depth: 1,
                    nodes: 1,
                    owned_bytes: 0,
                    ..Default::default()
                },
                kind,
                offset,
            );
        }
    }
    let empty = hex("18 00 2a 00");
    for n in 0..=4 {
        let kind = match n {
            0 => ErrorKind::Class(MissingFqName),
            2 => {
                check(&empty[..n], Default::default(), (None, 0), &[]);
                continue;
            }
            4 => ErrorKind::TypeParameter(MissingId),
            _ => cw(WireKind::Truncated),
        };
        error(&empty[..n], Default::default(), kind, n);
    }
}

#[test]
fn whole_budget_one_less_and_failure_charge_order() {
    let bytes = hex(literal!("selected"));
    for (limits, kind, offset) in [
        (
            Limits {
                bytes: 7,
                records: 0,
                work: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("bytes")),
            0,
        ),
        (
            Limits {
                nodes: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("nodes")),
            0,
        ),
        (
            Limits {
                depth: 0,
                nodes: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("depth")),
            0,
        ),
        (
            Limits {
                work: 0,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("work")),
            0,
        ),
        (
            Limits {
                nodes: 1,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("nodes")),
            4,
        ),
        (
            Limits {
                depth: 1,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("depth")),
            4,
        ),
        (
            Limits {
                records: 5,
                ..Default::default()
            },
            tw(WireKind::Limit("records")),
            6,
        ),
        (
            Limits {
                owned_bytes: S - 1,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("owned_bytes")),
            2,
        ),
        (
            Limits {
                work: 33 + S,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("work")),
            2,
        ),
        (
            Limits {
                records: 0,
                work: 1,
                ..Default::default()
            },
            cw(WireKind::Limit("records")),
            0,
        ),
        (
            Limits {
                records: 6,
                work: 1,
                ..Default::default()
            },
            cw(WireKind::Limit("work")),
            0,
        ),
        (
            Limits {
                work: 25,
                ..Default::default()
            },
            ErrorKind::Wire(WireKind::Limit("work")),
            4,
        ),
    ] {
        error(&bytes, limits, kind, offset);
    }
    let empty = hex("18 00 2a 00");
    limit(
        &empty,
        Limits {
            work: 17,
            ..Default::default()
        },
        "work",
        4,
    );
    error(
        &empty,
        Limits {
            work: 18,
            ..Default::default()
        },
        ErrorKind::TypeParameter(type_parameter::ErrorKind::MissingId),
        4,
    );
    let charged_id = hex("18 00 2a 02 30 07");
    error(
        &charged_id,
        Limits {
            work: 31,
            owned_bytes: 4,
            ..Default::default()
        },
        ErrorKind::TypeParameter(type_parameter::ErrorKind::MissingId),
        6,
    );
    error(
        &charged_id,
        Limits {
            owned_bytes: 3,
            ..Default::default()
        },
        tw(WireKind::Limit("owned_bytes")),
        4,
    );
    let charged_bound = hex("18 00 2a 07 08 00 10 00 2a 00 00");
    error(
        &charged_bound,
        Limits {
            work: 46 + B,
            owned_bytes: B,
            ..Default::default()
        },
        tw(WireKind::InvalidFieldNumber),
        10,
    );
    error(
        &charged_bound,
        Limits {
            work: 45 + B,
            owned_bytes: B,
            ..Default::default()
        },
        tw(WireKind::Limit("work")),
        10,
    );
    for (text, owned, work, tag) in [
        (literal!("duplicates"), 2 * S, 59 + 2 * S, 8),
        (literal!("three"), 4 * S, 84 + 4 * S, 14),
        (literal!("opaque"), 16 + 2 * B + S, 118 + 2 * B + S, 2),
    ] {
        let bytes = hex(text);
        limit(
            &bytes,
            Limits {
                owned_bytes: owned - 1,
                ..Default::default()
            },
            "owned_bytes",
            tag,
        );
        limit(
            &bytes,
            Limits {
                work: work - 1,
                ..Default::default()
            },
            "work",
            tag,
        );
    }
}

#[test]
fn inline_wrong_wire_signed_opaque_controls_and_prefixes() {
    for (text, parent, children, prefixes, missing) in [
        (
            "18 00 2a 0e 08 00 10 00 0a 00 12 00 1a 00 22 00 28 01",
            (None, 0),
            vec![child(4, 18, 0, 0)],
            vec![2, 18],
            vec![0],
        ),
        (
            "08 ff ff ff ff 0f 18 fe ff ff ff 0f 2a 04 08 00 10 00",
            (Some(-1), -2),
            vec![child(14, 18, 0, 0)],
            vec![12, 18],
            vec![0, 6],
        ),
        (
            "18 00 2a 08 08 01 10 02 08 03 10 04",
            (None, 0),
            vec![child(4, 12, 3, 4)],
            vec![2, 12],
            vec![0],
        ),
        (
            "18 00 2a 06 08 00 10 00 32 00",
            (None, 0),
            vec![child(4, 10, 0, 0)],
            vec![2, 10],
            vec![0],
        ),
        (
            "18 00 32 01 ff 4a 01 80",
            (None, 0),
            vec![],
            vec![2, 5, 8],
            vec![0],
        ),
    ] {
        let bytes = hex(text);
        for n in 0..=bytes.len() {
            if prefixes.contains(&n) {
                let children: Vec<_> = children.iter().filter(|c| c.end <= n).cloned().collect();
                check(&bytes[..n], Default::default(), parent, &children);
            } else if missing.contains(&n) {
                error(
                    &bytes[..n],
                    Default::default(),
                    ErrorKind::Class(class_header::ErrorKind::MissingFqName),
                    n,
                );
            } else {
                error(&bytes[..n], Default::default(), cw(WireKind::Truncated), n);
            }
        }
    }
}

#[test]
fn finite_hostile_literals_and_hard_clamps() {
    for (text, kind, offset) in [
        ("00", cw(WireKind::InvalidFieldNumber), 0),
        ("0e", cw(WireKind::InvalidWireType), 0),
        ("0b", cw(WireKind::UnsupportedGroup), 0),
        ("18 00 2a 01 ff", tw(WireKind::Truncated), 5),
        ("18 00 2a 02 08 80", tw(WireKind::Truncated), 6),
    ] {
        error(&hex(text), Default::default(), kind, offset);
    }
    let huge = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    for f in fixtures() {
        check(&hex(f.text), huge, f.parent, &f.children);
    }
    let mut fields = hex("18 00");
    for _ in 0..32_766 {
        fields.extend([0x20, 0]);
    }
    check(&fields, huge, (None, 0), &[]);
    fields.extend([0x20, 0]);
    limit(&fields, huge, "records", 65_534);
    let oversized = vec![0; ktlsp_binary::MAX_BYTES + 1];
    limit(&oversized, huge, "bytes", 0);
}

#[test]
fn genuine_pair_pipeline_keeps_raw_indices_and_borrowed_spans() {
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
    assert_eq!(bytes.len(), 285);
    let payload = metadata::payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(payload.payload_span, 47..285);
    assert_eq!(payload.payload, hex(literal!("pair_class")));
    let children = [
        Child {
            variance: Some(Variance::Out),
            ..child(7, 13, 0, 1)
        },
        Child {
            variance: Some(Variance::Out),
            ..child(15, 21, 1, 2)
        },
    ];
    let l = exact(238, 36, 3, 2, 563 + 2 * S, 2 * S);
    let out = check(payload.payload, l, (Some(1031), 0), &children);
    assert_eq!(out.header.raw.as_ptr(), bytes[47..285].as_ptr());
    assert_eq!(
        out.type_parameters[0].parameter.raw,
        hex("08 00 10 01 20 01")
    );
    assert_eq!(
        out.type_parameters[1].parameter.raw,
        hex("08 01 10 02 20 01")
    );
    assert_eq!(
        out.type_parameters[0].parameter.raw.as_ptr(),
        bytes[54..60].as_ptr()
    );
    assert_eq!(
        out.type_parameters[1].parameter.raw.as_ptr(),
        bytes[62..68].as_ptr()
    );
    for (limits, label, offset) in [
        (Limits { records: 35, ..l }, "records", 235),
        (
            Limits {
                work: 562 + 2 * S,
                ..l
            },
            "work",
            235,
        ),
        (
            Limits {
                owned_bytes: 2 * S - 1,
                ..l
            },
            "owned_bytes",
            13,
        ),
        (Limits { nodes: 2, ..l }, "nodes", 15),
        (Limits { depth: 1, ..l }, "depth", 7),
    ] {
        limit(payload.payload, limits, label, offset);
    }
    for (n, records, nodes, work, owned) in [
        (5, 4, 1, 19, 0),
        (13, 9, 2, 52 + S, S),
        (21, 14, 3, 85 + 2 * S, 2 * S),
        (29, 16, 3, 105 + 2 * S, 2 * S),
        (56, 18, 3, 163 + 2 * S, 2 * S),
        (68, 20, 3, 191 + 2 * S, 2 * S),
        (85, 22, 3, 229 + 2 * S, 2 * S),
        (102, 24, 3, 267 + 2 * S, 2 * S),
        (151, 26, 3, 369 + 2 * S, 2 * S),
        (173, 28, 3, 417 + 2 * S, 2 * S),
        (185, 30, 3, 445 + 2 * S, 2 * S),
        (210, 32, 3, 499 + 2 * S, 2 * S),
        (235, 34, 3, 553 + 2 * S, 2 * S),
        (238, 36, 3, 563 + 2 * S, 2 * S),
    ] {
        let expected: Vec<_> = children.iter().filter(|c| c.end <= n).cloned().collect();
        check(
            &payload.payload[..n],
            exact(n, records, nodes, if n == 5 { 1 } else { 2 }, work, owned),
            (Some(1031), 0),
            &expected,
        );
    }
}
