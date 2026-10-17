use ktlsp_binary::metadata::{
    self, class_context, name_resolver, resolved_class_function_parameters as classifier,
    resolved_class_supertypes::{Error, ErrorKind, Limits, decode},
    type_parameter::Variance,
    wire,
};
use std::ops::Range;

const UNIT: &[u8] = include_bytes!("../fixtures/t017c2/Unit.class");
const RESULT: &[u8] = include_bytes!("fixtures/metadata_constructor/Result$Companion.class.bin");
const PAIR: &[u8] = include_bytes!("fixtures/metadata_constructor/Pair.class.bin");
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn bytes<'a>(input: &'a metadata::Outcome<'_>) -> &'a [u8] {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("decoded Class")
    };
    bytes
}
fn bytes_mut<'a>(input: &'a mut metadata::Outcome<'_>) -> &'a mut Vec<u8> {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("decoded Class")
    };
    bytes
}
fn borrowed(raw: &[u8], input: &[u8], span: Range<usize>) {
    assert_eq!(raw, &input[span.clone()]);
    assert_eq!(raw.as_ptr(), input[span].as_ptr());
}
fn identity(id: &metadata::class_id::ClassId, package: &str, relative: &str, local: bool) {
    assert_eq!(id.package_name(), units(package));
    assert_eq!(id.relative_class_name(), units(relative));
    assert_eq!(id.local, local);
}
fn genuine(
    input: &metadata::Outcome<'_>,
    owner: &str,
    class_span: Range<usize>,
    outer: Range<usize>,
) {
    let out = decode(input, Limits::default()).unwrap();
    identity(&out.owner.class_id, "kotlin", owner, false);
    assert_eq!(out.owner.header.fq_name, 0);
    assert_eq!(out.owner.body_span, class_span);
    borrowed(out.owner.header.raw, bytes(input), class_span);
    assert!(out.parameters.is_empty());
    assert!(out.bounds.is_empty());
    assert_eq!(out.supertypes.len(), 1);
    let s = &out.supertypes[0];
    assert!(s.arguments.is_empty());
    assert_eq!(s.ordinal, 2);
    assert_eq!(s.span, outer);
    assert_eq!(s.body_span, outer.start + 2..outer.end);
    assert_eq!(&bytes(input)[s.span.clone()], &[0x32, 2, 0x30, 1]);
    borrowed(s.header.raw, bytes(input), s.body_span.clone());
    assert_eq!(s.header.raw, &[0x30, 1]);
    identity(&s.class_id, "kotlin", "Any", false);
    assert_eq!(
        s.header,
        metadata::type_header::TypeHeader {
            raw: &[0x30, 1],
            flags: None,
            nullable: None,
            flexible_type_capabilities_id: None,
            class_name: Some(1),
            type_parameter: None,
            flexible_upper_bound_id: None,
            type_parameter_name: None,
            outer_type_id: None,
            type_alias_name: None,
            abbreviated_type_id: None,
        }
    );
}

#[test]
fn original_genuine_unit_and_result_companion_are_interpreted_not_echoed() {
    // Existing licensed original data only; exact compiler provenance is unverified.
    // New-operation offsets/targets are source-derived expectations until execution.
    for (raw, owner, class_span, outer, flags) in [
        (UNIT, "Unit", 19..52, 24..28, Some(326)),
        (RESULT, "Result.Companion", 29..126, 34..38, Some(390)),
    ] {
        let class = ktlsp_binary::parse_class(raw).unwrap();
        let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
        let input = metadata::decode(&annotations, Default::default()).unwrap();
        genuine(&input, owner, class_span, outer);
        let out = decode(&input, Limits::default()).unwrap();
        assert_eq!(out.owner.header.flags, flags);
    }
}

fn pair_parameters(
    owner: &class_context::Owner<'_>,
    parameters: &[class_context::Parameter<'_>],
    input: &metadata::Outcome<'_>,
) {
    identity(&owner.class_id, "kotlin", "Pair", false);
    assert_eq!(parameters.len(), 2);
    for (ordinal, (name, span)) in [("A", 54..60), ("B", 62..68)].into_iter().enumerate() {
        let p = &parameters[ordinal];
        assert_eq!(p.body_span, span);
        borrowed(p.parameter.raw, bytes(input), span);
        assert_eq!(
            p.parameter.raw,
            &[8, ordinal as u8, 16, ordinal as u8 + 1, 32, 1]
        );
        assert_eq!(p.parameter.id, ordinal as i32);
        assert_eq!(p.parameter.name, ordinal as i32 + 1);
        assert_eq!(p.name, units(name));
        assert_eq!(p.parameter.reified, None);
        assert_eq!(p.parameter.variance, Some(Variance::Out));
        assert!(p.parameter.upper_bound.is_empty());
        assert!(p.parameter.upper_bound_id.is_empty());
    }
}

#[test]
fn original_pair_abbreviation_refuses_and_original_declarations_retain_custody() {
    let class = ktlsp_binary::parse_class(PAIR).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err(),
        Error {
            offset: 72,
            kind: ErrorKind::UnsupportedType,
        }
    );
    // An independently bounded EXISTING consumer retains original A/B declarations;
    // no failed new operation exposes a partial result or interpreted empty bounds.
    let old = metadata::resolved_class_properties::decode(&input, Limits::default()).unwrap();
    pair_parameters(&old.owner, &old.parameters, &input);
    drop(old);
    // Explicit mutation: strip only the original abbreviation. Not a genuine positive.
    bytes_mut(&mut input).splice(68..76, [0x32, 2, 0x30, 3]);
    let mutated = decode(&input, Limits::default()).unwrap();
    pair_parameters(&mutated.owner, &mutated.parameters, &input);
    assert!(mutated.bounds.is_empty());
    assert_eq!(mutated.supertypes[0].span, 68..72);
    identity(
        &mutated.supertypes[0].class_id,
        "java.io",
        "Serializable",
        false,
    );
}

fn replace_unit_type(input: &mut metadata::Outcome<'_>, ty: &[u8]) {
    assert!(ty.len() < 128);
    let mut outer = vec![0x32, ty.len() as u8];
    outer.extend(ty);
    let end = 26 + usize::from(bytes(input)[25]);
    bytes_mut(input).splice(24..end, outer);
}
fn name_error(
    label: &'static str,
    index: Option<i32>,
    record: Option<usize>,
) -> name_resolver::Error {
    name_resolver::Error {
        kind: name_resolver::ErrorKind::Limit(label),
        index,
        record,
    }
}

#[test]
fn mutations_resolve_real_indices_and_keep_repetitions_order_spans_and_options() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    // Actual original retained Unit data, with only the supertype index mutated.
    bytes_mut(&mut input)[27] = 0;
    let out = decode(&input, Limits::default()).unwrap();
    identity(&out.supertypes[0].class_id, "kotlin", "Unit", false);
    assert_eq!(out.supertypes[0].class_id, out.owner.class_id);
    drop(out); // This does NOT establish legal self-inheritance.
    bytes_mut(&mut input)[27] = 127;
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err(),
        Error {
            offset: 26,
            kind: ErrorKind::Classifier(classifier::ErrorKind::Name(name_resolver::Error {
                kind: name_resolver::ErrorKind::InvalidIndex,
                index: Some(127),
                record: None,
            })),
        }
    );
    bytes_mut(&mut input)[27] = 1;
    // Distinct followed by equal repeats, separated from the original by other fields.
    bytes_mut(&mut input).extend([0x32, 2, 0x30, 0, 0x32, 2, 0x30, 1]);
    let out = decode(&input, Limits::default()).unwrap();
    assert_eq!(out.supertypes.len(), 3);
    for (s, (ordinal, span, target, index)) in out.supertypes.iter().zip([
        (2, 24..28, "Any", 1),
        (6, 52..56, "Unit", 0),
        (7, 56..60, "Any", 1),
    ]) {
        assert_eq!(s.ordinal, ordinal);
        assert_eq!(s.span, span);
        assert_eq!(s.body_span, span.start + 2..span.end);
        borrowed(s.header.raw, bytes(&input), s.body_span.clone());
        assert_eq!(s.header.class_name, Some(index));
        identity(&s.class_id, "kotlin", target, false);
    }
    drop(out);
    // Reorder actual retained occurrences; do not deduplicate or sort by ClassId.
    bytes_mut(&mut input)[27] = 0;
    bytes_mut(&mut input)[55] = 1;
    let out = decode(&input, Limits::default()).unwrap();
    identity(&out.supertypes[0].class_id, "kotlin", "Unit", false);
    identity(&out.supertypes[1].class_id, "kotlin", "Any", false);
    assert_eq!(out.supertypes[2].span, 56..60);
    drop(out);
    for nullable in [None, Some(0), Some(1), Some(2)] {
        let mut ty = vec![0x30, 1, 8, 0];
        if let Some(value) = nullable {
            ty.extend([0x18, value]);
        }
        // Unknown extension remains opaque but fully borrowed and wire-bounded.
        ty.extend([0x7a, 1, 0xff]);
        replace_unit_type(&mut input, &ty);
        // Remove old appended occurrences whose offsets shifted with replacement.
        let len = bytes(&input).len();
        bytes_mut(&mut input).truncate(len - 8);
        let out = decode(&input, Limits::default()).unwrap();
        let s = &out.supertypes[0];
        assert_eq!(s.header.flags, Some(0));
        assert_eq!(s.header.nullable, nullable.map(|n| n != 0));
        borrowed(s.header.raw, bytes(&input), s.body_span.clone());
        assert_eq!(s.header.raw, ty);
        drop(out);
        // Restore original carrier for the next independently labelled mutation.
        input = metadata::decode(&annotations, Default::default()).unwrap();
        bytes_mut(&mut input).extend([0x32, 2, 0x30, 0, 0x32, 2, 0x30, 1]);
    }
}

#[test]
fn mutations_preserve_lossless_utf16_local_membership_without_function_scope_hacks() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    let target = [112, 47, 113, 47, 96, 88, 47, 0xd800, 96, 46, 0xdc00];
    let metadata::Outcome::Decoded {
        header,
        bytes: data,
    } = &mut input
    else {
        unreachable!()
    };
    header.d2[1] = &target;
    data[8] = 44; // Unknown predefined index falls back to this actual d2 entry.
    data.splice(19..19, [0x28, 1]); // Class starts two bytes later; local-name membership.
    data[0] += 2;
    let out = decode(&input, Limits::default()).unwrap();
    assert_eq!(out.supertypes[0].class_id.package_name(), &[112, 46, 113]);
    assert_eq!(
        out.supertypes[0].class_id.relative_class_name(),
        &[88, 47, 0xd800, 46, 0xdc00]
    );
    assert!(out.supertypes[0].class_id.local);
    assert!(!out.owner.class_id.local);
    assert_eq!(out.supertypes[0].body_span, 28..30);
    borrowed(out.supertypes[0].header.raw, bytes(&input), 28..30);
}

#[test]
fn mutations_refuse_every_significant_shape_wrong_wire_and_table_role_atomically() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    // Unsupported scalar/richer shapes; inline arguments now have a finite grammar.
    for number in [2u8, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        let mut ty = vec![0x30, 1];
        ty.extend([number * 8, 0]);
        replace_unit_type(&mut input, &ty);
        let expected = Error {
            offset: 28,
            kind: ErrorKind::UnsupportedType,
        };
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            expected,
            "field {number}"
        );
        let mut ty = vec![0x30, 1];
        ty.extend([number * 8 + 2, 0]);
        replace_unit_type(&mut input, &ty);
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: if number == 2 { 30 } else { 28 },
                kind: if number == 2 {
                    ErrorKind::MissingRole
                } else {
                    ErrorKind::UnsupportedType
                },
            },
            "inline/wrong-wire {number}"
        );
    }
    for number in [1u8, 3, 6] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        replace_unit_type(&mut input, &[0x30, 1, number * 8 + 2, 0]);
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: 28,
                kind: ErrorKind::UnsupportedType,
            }
        );
    }
    for ty in [&[8, 1, 0x30, 1][..], &[][..]] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        replace_unit_type(&mut input, ty);
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: 26,
                kind: ErrorKind::UnsupportedType,
            }
        );
    }
    for tail in [
        vec![0x10, 0],
        vec![0x12, 0],
        vec![0x12, 1, 0x80],
        vec![0x15, 0, 0, 0, 0],
        vec![0x32, 0, 0x12, 0],
    ] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        // Insert before the original supertype: every ID representation refuses,
        // even an empty packed list or malformed packed interior, with mixed roles.
        bytes_mut(&mut input).splice(24..24, tail);
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: if bytes(&input)[24] == 0x32 { 26 } else { 24 },
                kind: if bytes(&input)[24] == 0x32 {
                    ErrorKind::UnsupportedType
                } else {
                    ErrorKind::UnsupportedTableRole
                },
            }
        );
    }
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    bytes_mut(&mut input).splice(24..28, [0x30, 1]);
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err(),
        Error {
            offset: 24,
            kind: ErrorKind::UnsupportedClass,
        }
    );
}

#[test]
fn mutations_malformed_late_parent_and_unsupported_admission_never_return_prefixes() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    bytes_mut(&mut input).push(0x80);
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err(),
        Error {
            offset: 53,
            kind: ErrorKind::Context(class_context::ErrorKind::Class(
                metadata::class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)
            )),
        }
    );
    for (kind, version) in [(2, vec![2, 3, 0]), (1, vec![2, 2, 0]), (1, vec![2, 3])] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        let metadata::Outcome::Decoded { header, .. } = &mut input else {
            unreachable!()
        };
        header.k = kind;
        header.mv = version;
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: 0,
                kind: ErrorKind::Context(class_context::ErrorKind::UnsupportedContainer),
            }
        );
    }
}

type Set = fn(&mut Limits, usize);
const DIMENSIONS: [(&str, Set); 9] = [
    ("bytes", |l, n| l.bytes = n),
    ("records", |l, n| l.records = n),
    ("nodes", |l, n| l.nodes = n),
    ("work", |l, n| l.work = n),
    ("owned_bytes", |l, n| l.owned_bytes = n),
    ("queries", |l, n| l.queries = n),
    ("output_units", |l, n| l.output_units = n),
    ("entries", |l, n| l.entries = n),
    ("range_slots", |l, n| l.range_slots = n),
];
fn minimum(input: &metadata::Outcome<'_>, set: Set) -> usize {
    // Existing aggregate-boundary test convention; these minima are not copied from
    // another consumer or claimed observed before this test actually executes.
    let accepted = |n| {
        let mut l = Limits::default();
        set(&mut l, n);
        decode(input, l).is_ok()
    };
    let (mut low, mut high) = (0, 1_000_000);
    assert!(!accepted(low));
    assert!(accepted(high));
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if accepted(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn genuine_unit_uses_joint_exact_allowances_and_repeated_children_never_reset() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    let mut exact = Limits {
        depth: 2,
        ..Default::default()
    };
    let mut minima = Vec::new();
    for (_, set) in DIMENSIONS {
        let n = minimum(&input, set);
        minima.push(n);
        set(&mut exact, n);
    }
    assert_eq!(
        decode(&input, exact).unwrap(),
        decode(&input, Limits::default()).unwrap()
    );
    // These are this operation's dynamically observed boundaries, not historical
    // bare-only counts. Each joint one-short allowance must refuse atomically.
    for ((label, set), n) in DIMENSIONS.into_iter().zip(&minima) {
        let mut below = exact;
        set(&mut below, n - 1);
        let error = decode(&input, below).unwrap_err();
        match error.kind {
            ErrorKind::Wire(wire::ErrorKind::Limit(found)) => assert_eq!(found, label),
            ErrorKind::Context(class_context::ErrorKind::Wire(wire::ErrorKind::Limit(found))) => {
                assert_eq!(found, label);
            }
            ErrorKind::Context(class_context::ErrorKind::Name(e))
            | ErrorKind::Classifier(classifier::ErrorKind::Name(e)) => {
                assert_eq!(e.kind, name_resolver::ErrorKind::Limit(label));
            }
            other => panic!("unexpected joint one-short {label}: {other:?}"),
        }
    }
    assert_eq!(exact.queries, 4);
    assert_eq!(exact.output_units, 21);
    assert_eq!(
        decode(&input, Limits { depth: 1, ..exact }).unwrap_err(),
        Error {
            offset: 3,
            kind: ErrorKind::Wire(wire::ErrorKind::Limit("depth")),
        }
    );
    assert_eq!(
        decode(
            &input,
            Limits {
                depth: usize::MAX,
                ..exact
            }
        )
        .unwrap(),
        decode(&input, exact).unwrap()
    );
    // Owner-only mutation admits the original context with two queries. Adding the
    // selected target cannot obtain a new resolver's allowance or return a prefix.
    let late = Limits {
        queries: 2,
        ..Default::default()
    };
    assert_eq!(
        decode(&input, late).unwrap_err(),
        Error {
            offset: 26,
            kind: ErrorKind::Classifier(classifier::ErrorKind::Name(name_error(
                "queries",
                Some(1),
                None
            ))),
        }
    );
    bytes_mut(&mut input).drain(24..28);
    let owner = decode(&input, late).unwrap();
    identity(&owner.owner.class_id, "kotlin", "Unit", false);
    assert!(owner.supertypes.is_empty());
    drop(owner);
    input = metadata::decode(&annotations, Default::default()).unwrap();
    bytes_mut(&mut input).extend([0x32, 2, 0x30, 1]);
    let repeated = decode(&input, Limits::default()).unwrap();
    assert_eq!(repeated.supertypes.len(), 2);
    assert_eq!(repeated.supertypes[1].span, 52..56);
    drop(repeated);
    for (dimension, offset) in [(1, 37), (2, 54), (3, 37), (4, 54), (5, 54), (6, 54)] {
        let (label, set) = DIMENSIONS[dimension];
        let mut limited = Limits::default();
        set(&mut limited, minima[dimension]);
        let error = decode(&input, limited).unwrap_err();
        // The last-child query/output/node checks have exact target offsets. Record
        // and work boundaries may occur earlier in the charged repeated parent scan.
        if matches!(dimension, 2 | 4 | 5 | 6) {
            assert_eq!(error.offset, offset, "{label}");
        }
        match error.kind {
            ErrorKind::Wire(wire::ErrorKind::Limit(found)) => assert_eq!(found, label),
            ErrorKind::Classifier(classifier::ErrorKind::Name(e)) => {
                assert_eq!(e.kind, name_resolver::ErrorKind::Limit(label));
                assert_eq!(e.index, Some(1));
            }
            other => panic!(
                "unexpected repeated-child {label}: {other:?} at {}",
                error.offset
            ),
        }
    }
}

#[test]
fn explicitly_mutated_pair_bound_is_interpreted_or_the_whole_call_refuses() {
    let class = ktlsp_binary::parse_class(PAIR).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    // Mutation controls, NOT genuine nonempty-bound or bare-Pair evidence.
    bytes_mut(&mut input).splice(68..76, [0x32, 2, 0x30, 3]);
    let extra = [
        0x18, 0, 0x20, 99, 0x2a, 2, 0xff, 0x80, 0x30, 7, 0x32, 2, 8, 9, 0x32, 0,
    ];
    bytes_mut(&mut input).splice(60..60, extra);
    bytes_mut(&mut input)[53] += extra.len() as u8;
    assert!(decode(&input, Limits::default()).is_err());
    // The separately bounded existing property operation deliberately retains its
    // original opaque-bound contract; extending this consumer must not change it.
    let old = metadata::resolved_class_properties::decode(&input, Limits::default()).unwrap();
    assert_eq!(old.parameters.len(), 2);
    let p = &old.parameters[0];
    assert_eq!(p.body_span, 54..76);
    borrowed(p.parameter.raw, bytes(&input), 54..76);
    assert_eq!(p.parameter.id, 0);
    assert_eq!(p.parameter.name, 1);
    assert_eq!(p.name, units("A"));
    assert_eq!(p.parameter.reified, Some(false));
    assert_eq!(p.parameter.variance, Some(Variance::Out));
    assert_eq!(p.parameter.upper_bound.len(), 1);
    assert_eq!(p.parameter.upper_bound[0].span, 12..14);
    borrowed(p.parameter.upper_bound[0].raw, p.parameter.raw, 12..14);
    assert_eq!(p.parameter.upper_bound[0].raw, &[0xff, 0x80]);
    assert_eq!(p.parameter.upper_bound_id, [7, 8, 9]);
    assert_eq!(old.parameters[1].body_span, 78..84);
    borrowed(old.parameters[1].parameter.raw, bytes(&input), 78..84);
    assert_eq!(old.parameters[1].name, units("B"));
    assert_eq!(old.parameters[1].parameter.id, 1);
}

#[test]
fn late_id_role_and_bad_late_child_refuse_after_an_admitted_original_target() {
    let class = ktlsp_binary::parse_class(UNIT).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    for tail in [vec![0x10, 1], vec![0x12, 0], vec![0x12, 1, 0x80]] {
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        bytes_mut(&mut input).extend(tail);
        assert_eq!(
            decode(&input, Limits::default()).unwrap_err(),
            Error {
                offset: 52,
                kind: ErrorKind::UnsupportedTableRole,
            }
        );
    }
    let mut input = metadata::decode(&annotations, Default::default()).unwrap();
    bytes_mut(&mut input).extend([0x32, 1, 0x80]);
    assert_eq!(
        decode(&input, Limits::default()).unwrap_err(),
        Error {
            offset: 55,
            kind: ErrorKind::Wire(wire::ErrorKind::Truncated),
        }
    );
}
