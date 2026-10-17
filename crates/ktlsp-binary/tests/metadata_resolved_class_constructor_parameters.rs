use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, class_context,
        resolved_class_constructor_parameters::{Error, ErrorKind, Limits, decode},
        resolved_class_function_parameters::{self as values, Classifier},
        wire,
    },
};
use std::ops::Range;

static RAW: Annotation<'static> = Annotation { descriptor: &[], elements: Vec::new() };
const NAMES: &[&[u16]] = &[
    &[112, 107, 103, 47, 66, 111, 120], // pkg/Box
    &[65], &[66], &[99, 111, 112, 121],
    &[102, 105, 114, 115, 116], &[115, 101, 99, 111, 110, 100],
    &[107, 111, 116, 108, 105, 110, 47, 65, 110, 121],
];
fn input<'a>(bytes: Vec<u8>, names: &[&'a [u16]]) -> metadata::Outcome<'a> {
    metadata::Outcome::Decoded {
        bytes,
        header: metadata::Header {
            raw: &RAW, visibility: &Visibility::RuntimeVisible, present: 0,
            k: 1, mv: vec![2, 3, 0], bv: vec![], d1: vec![], d2: names.to_vec(),
            xs: &[], pn: &[], xi: 0,
        },
    }
}
fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while value >= 128 { out.push(value as u8 | 128); value >>= 7; }
    out.push(value as u8);
    out
}
fn scalar(number: u32, value: u64) -> Vec<u8> {
    let mut out = varint(u64::from(number) * 8);
    out.extend(varint(value)); out
}
fn message(number: u32, body: &[u8]) -> Vec<u8> {
    let mut out = varint(u64::from(number) * 8 + 2);
    out.extend(varint(body.len() as u64)); out.extend(body); out
}
fn declaration(id: i32, name: u8) -> Vec<u8> {
    let mut body = scalar(1, u64::from(id as u32));
    body.extend(scalar(2, u64::from(name))); message(5, &body)
}
fn leaf(id: i32) -> Vec<u8> { scalar(7, u64::from(id as u32)) }
fn value_body(name: u8, ty: &[u8]) -> Vec<u8> {
    let mut body = scalar(2, u64::from(name)); body.extend(message(3, ty)); body
}
fn value(name: u8, ty: &[u8]) -> Vec<u8> { message(2, &value_body(name, ty)) }
fn class_with_table(fields: &[u8], table: &[u8]) -> Vec<u8> {
    let mut out = varint(table.len() as u64); out.extend(table);
    out.extend([0x18, 0]); out.extend(fields); out
}
fn class(fields: &[u8]) -> Vec<u8> { class_with_table(fields, &[0x0a, 2, 8, 7]) }
fn fields(body: &[u8]) -> Vec<u8> {
    let mut out = declaration(0, 1); out.extend(declaration(1, 2));
    out.extend(message(8, body)); out
}
fn ordinary_body() -> Vec<u8> {
    let mut out = value(4, &leaf(0)); out.extend(value(5, &leaf(1))); out
}
fn ordinary() -> metadata::Outcome<'static> { input(class(&fields(&ordinary_body())), NAMES) }
fn error(body: &[u8]) -> Error {
    decode(&input(class(&fields(body)), NAMES), 3, Limits::default()).unwrap_err()
}
fn value_kind(body: &[u8]) -> values::ErrorKind {
    let ErrorKind::Value(kind) = error(body).kind else { panic!("value error") }; kind
}
fn type_kind(ty: &[u8]) -> values::ErrorKind { value_kind(&value(4, ty)) }
fn units(text: &str) -> Vec<u16> { text.encode_utf16().collect() }
fn borrowed(raw: &[u8], bytes: &[u8], span: Range<usize>) {
    assert_eq!(raw, &bytes[span.clone()]); assert_eq!(raw.as_ptr(), bytes[span].as_ptr());
}

#[test]
fn genuine_pair_constructor_keeps_every_original_identity_and_option() {
    // Unchanged licensed distribution data; producing compiler remains UNVERIFIED.
    let class = ktlsp_binary::parse_class(include_bytes!("fixtures/metadata_constructor/Pair.class.bin")).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { bytes, .. } = &input else { panic!("decoded Class") };
    let result = decode(&input, 5, Limits::default()).unwrap();
    assert_eq!(result.owner.body_span, 47..285);
    borrowed(result.owner.header.raw, bytes, 47..285);
    assert_eq!(result.owner.header.flags, Some(1031));
    assert_eq!(result.owner.header.fq_name, 0);
    assert_eq!(result.owner.class_id.package_name(), units("kotlin"));
    assert_eq!(result.owner.class_id.relative_class_name(), units("Pair"));
    assert!(!result.owner.class_id.local);
    assert_eq!(result.parameters.len(), 2);
    let c = &result.constructor;
    assert_eq!(c.ordinal, 5); assert_eq!(c.span, 76..103); assert_eq!(c.body_span, 78..103);
    borrowed(c.raw, bytes, 78..103);
    assert_eq!(c.flags, Some(70)); assert!(c.version_requirements.is_empty());
    assert_eq!(c.parameters.len(), 2);
    for (ordinal, (name, raw_name, span, body_span, type_outer, type_span, declaration_span)) in [
        ("first", 5, 80..88, 82..88, 84..88, 86..88, 54..60),
        ("second", 6, 88..96, 90..96, 92..96, 94..96, 62..68),
    ].into_iter().enumerate() {
        let v = &c.parameters[ordinal];
        assert_eq!(v.ordinal, ordinal); assert_eq!(v.span, span); assert_eq!(v.body_span, body_span);
        assert_eq!(v.type_span, type_span);
        borrowed(v.raw.raw, bytes, body_span.clone()); borrowed(v.header.raw, bytes, type_span.clone());
        assert_eq!(&bytes[type_outer.clone()], &[0x1a, 2, 0x38, ordinal as u8]);
        assert_eq!(type_outer.end, type_span.end);
        assert_eq!(v.raw.raw, [0x10, raw_name, 0x1a, 2, 0x38, ordinal as u8]);
        assert_eq!(v.raw.name, i32::from(raw_name)); assert_eq!(v.name, units(name));
        assert_eq!(v.raw.flags, None); assert_eq!(v.raw.type_id, None);
        assert_eq!(v.raw.vararg_element_type_id, None);
        assert!(!v.has_annotations()); assert!(!v.declares_default_value());
        assert!(!v.is_crossinline()); assert!(!v.is_noinline());
        assert_eq!(v.classifier, Classifier::ClassParameter(ordinal));
        assert_eq!(v.header.type_parameter, Some(ordinal as i32));
        assert_eq!(v.header.flags, None); assert_eq!(v.header.nullable, None);
        assert_eq!(v.header.class_name, None); assert_eq!(v.header.type_parameter_name, None);
        assert_eq!(v.header.type_alias_name, None); assert_eq!(v.header.outer_type_id, None);
        assert_eq!(v.header.abbreviated_type_id, None);
        assert_eq!(v.header.flexible_upper_bound_id, None);
        assert_eq!(v.header.flexible_type_capabilities_id, None);
        let p = &result.parameters[ordinal];
        assert_eq!(p.body_span, declaration_span); borrowed(p.parameter.raw, bytes, declaration_span);
        assert_eq!(p.parameter.id, ordinal as i32); assert_eq!(p.parameter.name, ordinal as i32 + 1);
        assert_eq!(p.name, units(if ordinal == 0 { "A" } else { "B" }));
        assert_eq!(p.parameter.reified, None);
        assert_eq!(p.parameter.variance, Some(metadata::type_parameter::Variance::Out));
        assert!(p.parameter.upper_bound.is_empty()); assert!(p.parameter.upper_bound_id.is_empty());
    }
    assert_eq!(decode(&input, 0, Limits::default()).unwrap_err().kind, ErrorKind::NotConstructor);
}

// Explicitly authored wire controls below, not compiler provenance evidence.
#[test]
fn authored_signed_reordered_duplicate_ids_and_names_do_not_replace_identity() {
    let mut body = value(4, &leaf(-7)); body.extend(value(4, &leaf(12345)));
    body.extend(value(4, &scalar(7, (-7i64) as u64)));
    let mut fields = message(8, &body);
    fields.extend(declaration(12345, 1)); fields.extend(declaration(-7, 1));
    let input = input(class(&fields), NAMES);
    let result = decode(&input, 1, Limits::default()).unwrap();
    assert_eq!(result.parameters[0].parameter.id, 12345);
    assert_eq!(result.parameters[1].parameter.id, -7);
    assert_eq!(result.parameters[0].name, result.parameters[1].name);
    for (ordinal, target) in [1, 0, 1].into_iter().enumerate() {
        let v = &result.constructor.parameters[ordinal];
        assert_eq!(v.ordinal, ordinal); assert_eq!(v.classifier, Classifier::ClassParameter(target));
        assert_eq!(v.name, units("first"));
    }
    assert_ne!(result.constructor.parameters[0].raw.raw.as_ptr(), result.constructor.parameters[2].raw.raw.as_ptr());
    let mut duplicate = declaration(0, 1); duplicate.extend(declaration(0, 2));
    duplicate.extend(message(8, &value(4, &leaf(99))));
    assert_eq!(decode(&input_for(&duplicate), 3, Limits::default()).unwrap_err().kind,
        ErrorKind::Context(class_context::ErrorKind::DuplicateParameterId));
    assert_eq!(type_kind(&leaf(99)), values::ErrorKind::UnknownParameterId);
    // NAME resolves A but the Constructor's immediate-own scope is EMPTY.
    assert_eq!(type_kind(&scalar(9, 1)), values::ErrorKind::UnknownParameterName);
    assert!(matches!(type_kind(&scalar(9, 99)), values::ErrorKind::Name(_)));
}
fn input_for(fields: &[u8]) -> metadata::Outcome<'static> { input(class(fields), NAMES) }

#[test]
fn authored_nonminimal_prefixes_unknown_fields_multiple_and_empty_constructors() {
    let mut body = message(100, &[0x80]);
    let value_body = value_body(4, &leaf(0));
    let value_start = body.len();
    body.extend([0x92, 0, 0x86, 0]); // Nonminimal field2 tag and six-byte body length.
    body.extend(&value_body);
    body.extend(scalar(1, 6)); body.extend(scalar(1, u64::from(u32::MAX)));
    body.extend(scalar(31, 7));
    let mut packed = varint((-7i64) as u64); packed.extend(varint(2));
    body.extend(message(31, &packed)); body.extend(scalar(31, 7));
    for n in [3, 32, 100] { body.extend(message(n, &[0x80])); }
    let mut fields = message(8, &[]); // First Constructor is genuinely empty.
    fields.extend(scalar(99, 0)); // ALL-field ordinal includes unknown fields.
    fields.extend([0xc2, 0, (body.len() as u8) | 0x80, 0]); // Nonminimal Class field8.
    fields.extend(&body); fields.extend(message(8, &[0x80])); // Unselected body opaque.
    fields.extend(declaration(0, 1));
    let input = input_for(&fields);
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    let empty = decode(&input, 1, Limits::default()).unwrap();
    assert!(empty.constructor.parameters.is_empty()); assert_eq!(empty.constructor.flags, None);
    assert!(empty.constructor.version_requirements.is_empty());
    let result = decode(&input, 3, Limits::default()).unwrap();
    let c = &result.constructor;
    let outer = class(&[]).len() + message(8, &[]).len() + scalar(99, 0).len();
    assert_eq!(c.span, outer..outer + 4 + body.len());
    assert_eq!(c.body_span, outer + 4..outer + 4 + body.len());
    borrowed(c.raw, bytes, c.body_span.clone());
    assert_eq!(c.flags, Some(-1)); assert_eq!(c.version_requirements, [7, -7, 2, 7]);
    let v = &c.parameters[0]; let start = c.body_span.start + value_start;
    assert_eq!(v.span, start..start + 4 + value_body.len());
    assert_eq!(v.body_span, start + 4..start + 4 + value_body.len());
    borrowed(v.raw.raw, bytes, v.body_span.clone());
    borrowed(v.header.raw, bytes, v.type_span.clone());
    assert_eq!(decode(&input, 2, Limits::default()).unwrap_err().kind, ErrorKind::NotConstructor);
    assert_eq!(decode(&input, usize::MAX, Limits::default()).unwrap_err().kind, ErrorKind::MissingOrdinal);
    assert_eq!(decode(&input_for(&scalar(8, 0)), 1, Limits::default()).unwrap_err().kind, ErrorKind::NotConstructor);
    assert!(decode(&input_for(&[]), 1, Limits::default()).is_err());
}

#[test]
fn authored_presence_inline_precedence_and_all_flag_bits_are_retained() {
    for nullable in [None, Some(false), Some(true)] {
        for flags in [None, Some(0), Some(1), Some(2), Some(4), Some(8), Some(-1), Some(16)] {
            let mut ty = leaf(0);
            if let Some(marking) = nullable { ty.extend(scalar(3, u64::from(marking))); }
            let mut body = value_body(4, &ty); body.extend(scalar(5, u64::from(u32::MAX)));
            if let Some(flags) = flags { body.extend(scalar(1, u64::from(flags as u32))); }
            let input = input_for(&fields(&message(2, &body)));
            let result = decode(&input, 3, Limits::default()).unwrap();
            let v = &result.constructor.parameters[0];
            assert_eq!(v.header.nullable, nullable); assert_eq!(v.raw.flags, flags);
            assert_eq!(v.raw.type_id, Some(-1)); assert_eq!(v.raw.vararg_element_type_id, None);
            let bits = flags.unwrap_or(0);
            assert_eq!(v.has_annotations(), bits & 1 != 0);
            assert_eq!(v.declares_default_value(), bits & 2 != 0);
            assert_eq!(v.is_crossinline(), bits & 4 != 0); assert_eq!(v.is_noinline(), bits & 8 != 0);
        }
    }
    let mut ty = leaf(0); ty.extend(scalar(1, 1)); ty.extend(scalar(1, 1u64 << 32));
    ty.extend(scalar(7, 1u64 << 32));
    let input = input_for(&fields(&value(4, &ty)));
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.constructor.parameters[0].header.flags, Some(0));
    assert_eq!(result.constructor.parameters[0].classifier, Classifier::ClassParameter(0));
}

#[test]
fn authored_nominal_operations_predefined_and_lossless_names_use_actual_resolver() {
    for (record, encoded) in [(vec![0x18, 1], units("a/b/Outer$Inner")), (vec![0x18, 2], units("La/b/Outer$Inner;"))] {
        let mut table = message(1, &[8, 6]); table.extend(message(1, &record)); table.extend(scalar(5, 6));
        let mut names = NAMES.to_vec(); names[6] = &encoded;
        let input = input(class_with_table(&fields(&value(4, &[0x30, 6])), &table), &names);
        let result = decode(&input, 3, Limits::default()).unwrap();
        let Classifier::Class(id) = &result.constructor.parameters[0].classifier else { panic!("nominal") };
        assert_eq!(id.package_name(), units("a.b")); assert_eq!(id.relative_class_name(), units("Outer.Inner"));
        assert!(id.local);
    }
    let mut table = message(1, &[8, 6]); table.extend(message(1, &[0x10, 0]));
    let predefined = input(class_with_table(&fields(&value(4, &[0x30, 6])), &table), NAMES);
    let result = decode(&predefined, 3, Limits::default()).unwrap();
    let Classifier::Class(id) = &result.constructor.parameters[0].classifier else { panic!("predefined") };
    assert_eq!(id.package_name(), units("kotlin")); assert_eq!(id.relative_class_name(), units("Any"));
    let name = [0xd800, 0, 0xdc00]; let mut names = NAMES.to_vec(); names[4] = &name;
    let input = input(class(&fields(&value(4, &leaf(0)))), &names);
    assert_eq!(decode(&input, 3, Limits::default()).unwrap().constructor.parameters[0].name, name);
    assert!(matches!(type_kind(&[0x30, 99]), values::ErrorKind::Name(_)));
}

#[test]
fn authored_finite_grammar_wrong_wires_varargs_and_late_values_refuse_atomically() {
    let mut duplicate = value_body(4, &leaf(0)); duplicate.extend(message(3, &leaf(1)));
    assert_eq!(value_kind(&message(2, &duplicate)), values::ErrorKind::DuplicateInlineRole);
    assert_eq!(value_kind(&message(2, &[0x10, 4])), values::ErrorKind::MissingRole);
    assert_eq!(value_kind(&message(2, &[0x10, 4, 0x28, 0])), values::ErrorKind::UnsupportedTableRole);
    for number in [4, 6] {
        for extra in [message(number, &[]), scalar(number, 0)] {
            let mut body = value_body(4, &leaf(0)); body.extend(extra);
            let mut roles = ordinary_body(); roles.extend(message(2, &body));
            assert_eq!(value_kind(&roles), values::ErrorKind::UnsupportedVararg);
        }
    }
    for number in [1, 2, 3, 5] {
        let mut body = value_body(4, &leaf(0));
        body.extend(if number == 3 { scalar(number, 0) } else { message(number, &[]) });
        assert_eq!(value_kind(&message(2, &body)), values::ErrorKind::UnsupportedParameter);
    }
    assert!(matches!(value_kind(&message(2, &[])), values::ErrorKind::Parameter(_)));
    for n in [1, 2, 31] {
        let mut body = ordinary_body();
        body.extend(if n == 2 { scalar(n, 0) } else { let mut b = varint(u64::from(n) * 8 + 5); b.extend([0; 4]); b });
        assert_eq!(error(&body).kind, ErrorKind::UnsupportedConstructor);
    }
    for extra in [
        scalar(6, 6), scalar(9, 1), scalar(12, 0), scalar(4, 0), scalar(8, 0), scalar(11, 0), scalar(14, 0),
        message(2, &[]), message(5, &[]), message(10, &[]), message(13, &[]),
        message(1, &[]), message(3, &[]), message(6, &[]), message(7, &[]), scalar(1, 1),
    ] {
        let mut ty = leaf(0); ty.extend(extra);
        assert_eq!(type_kind(&ty), values::ErrorKind::UnsupportedType);
    }
    assert_eq!(type_kind(&[]), values::ErrorKind::UnsupportedType);
    for n in 1..=14 {
        if matches!(n, 1 | 3 | 6) { continue; }
        let mut ty = vec![0x30, 6]; ty.extend(message(n, &[]));
        assert_eq!(type_kind(&ty), values::ErrorKind::UnsupportedType);
    }
    let mut late = ordinary_body(); late.extend(value(4, &leaf(99)));
    assert_eq!(value_kind(&late), values::ErrorKind::UnknownParameterId);
}

#[test]
fn authored_structural_parents_precede_semantics_and_offsets_rebase_once() {
    // A missing value name would fail first without complete raw-parent staging.
    let mut body = message(2, &[]); body.push(0x80);
    let err = error(&body);
    let input = input_for(&fields(&body));
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    assert_eq!(err, Error { offset: bytes.len(), kind: ErrorKind::Constructor(wire::ErrorKind::Truncated) });
    let mut body = message(2, &[]); body.extend(message(31, &[0x80]));
    let err = error(&body);
    let input = input_for(&fields(&body));
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    assert_eq!(err, Error { offset: bytes.len(), kind: ErrorKind::Constructor(wire::ErrorKind::Truncated) });
    let body = value(4, &[0x38]);
    let err = error(&body);
    let input = input_for(&fields(&body));
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    assert_eq!(err, Error { offset: bytes.len(), kind: ErrorKind::Value(values::ErrorKind::Wire(wire::ErrorKind::Truncated)) });
    let mut fields = fields(&message(2, &[])); fields.push(0x80);
    let input = input_for(&fields);
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    let err = decode(&input, 3, Limits::default()).unwrap_err();
    assert_eq!(err.offset, bytes.len()); assert!(matches!(err.kind, ErrorKind::Context(_)));
    // Unknown name precedes a later semantic wrong-wire role, unlike malformed framing.
    let mut body = value(99, &leaf(0)); body.extend(scalar(2, 0));
    assert!(matches!(error(&body).kind, ErrorKind::Value(values::ErrorKind::Name(_))));
}

#[test]
fn authored_limits_measure_this_operation_joint_exact_and_one_less_all_nine() {
    let input = ordinary();
    let metadata::Outcome::Decoded { header, bytes } = &input else { unreachable!() };
    let admission = bytes.len() + header.d2.iter().map(|s| s.len() * 2).sum::<usize>();
    type Set = fn(&mut Limits, usize);
    let dimensions: [(&str, Set, usize); 9] = [
        ("bytes", |l, n| l.bytes = n, admission),
        ("records", |l, n| l.records = n, 4096),
        ("nodes", |l, n| l.nodes = n, 128),
        ("work", |l, n| l.work = n, 100_000),
        ("owned_bytes", |l, n| l.owned_bytes = n, 100_000),
        ("entries", |l, n| l.entries = n, 128),
        ("range_slots", |l, n| l.range_slots = n, 128),
        ("queries", |l, n| l.queries = n, 128),
        ("output_units", |l, n| l.output_units = n, 1024),
    ];
    let mut exact = Limits { depth: 2, ..Default::default() };
    for (name, set, bound) in dimensions {
        let accepted = |n| { let mut l = Limits::default(); set(&mut l, n); decode(&input, 3, l).is_ok() };
        assert!(!accepted(0), "{name}"); assert!(accepted(bound), "{name}");
        let (mut low, mut high) = (0, bound);
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if accepted(middle) { high = middle; } else { low = middle; }
        }
        assert!(!accepted(high - 1), "{name}"); assert!(accepted(high), "{name}");
        set(&mut exact, high);
    }
    assert_eq!(decode(&input, 3, exact).unwrap(), decode(&input, 3, Limits::default()).unwrap());
    for (name, set, _) in dimensions {
        // Reduce the same dimension from the jointly exact result, not defaults.
        let value = match name {
            "bytes" => exact.bytes, "records" => exact.records, "nodes" => exact.nodes,
            "work" => exact.work, "owned_bytes" => exact.owned_bytes, "entries" => exact.entries,
            "range_slots" => exact.range_slots, "queries" => exact.queries, "output_units" => exact.output_units,
            _ => unreachable!(),
        };
        let mut one_less = exact; set(&mut one_less, value - 1);
        assert!(decode(&input, 3, one_less).is_err(), "{name}");
    }
    let mut body = ordinary_body(); body.extend(value(4, &leaf(0)));
    let larger = input_for(&fields(&body));
    assert!(decode(&larger, 3, Limits::default()).is_ok());
    for (set, value) in [
        ((|l: &mut Limits, n| l.records = n) as Set, exact.records),
        ((|l: &mut Limits, n| l.nodes = n) as Set, exact.nodes),
        ((|l: &mut Limits, n| l.work = n) as Set, exact.work),
        ((|l: &mut Limits, n| l.owned_bytes = n) as Set, exact.owned_bytes),
        ((|l: &mut Limits, n| l.queries = n) as Set, exact.queries),
        ((|l: &mut Limits, n| l.output_units = n) as Set, exact.output_units),
    ] {
        let mut l = Limits::default(); set(&mut l, value); assert!(decode(&larger, 3, l).is_err());
    }
    assert!(decode(&input, 3, Limits { depth: 1, ..Default::default() }).is_err());
    let huge = Limits {
        bytes: usize::MAX, records: usize::MAX, nodes: usize::MAX, depth: usize::MAX,
        work: usize::MAX, owned_bytes: usize::MAX, entries: usize::MAX, range_slots: usize::MAX,
        queries: usize::MAX, output_units: usize::MAX,
    };
    assert_eq!(decode(&input, 3, huge).unwrap(), decode(&input, 3, Limits::default()).unwrap());
    let mut malformed = vec![0x0a]; malformed.extend(varint(u64::MAX));
    assert!(decode(&input_for(&malformed), 3, huge).is_err());
}

#[test]
fn authored_raised_limits_cannot_bypass_public_admission_and_scan_hard_clamps() {
    let hard = Limits::default();
    let huge = Limits {
        bytes: usize::MAX, records: usize::MAX, nodes: usize::MAX, depth: usize::MAX,
        work: usize::MAX, owned_bytes: usize::MAX, entries: usize::MAX, range_slots: usize::MAX,
        queries: usize::MAX, output_units: usize::MAX,
    };
    let bytes = input(vec![0; hard.bytes + 1], NAMES);
    let err = decode(&bytes, 0, hard).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Context(class_context::ErrorKind::Wire(wire::ErrorKind::Limit("bytes"))));
    assert_eq!(decode(&bytes, 0, huge).unwrap_err(), err);
    let names = vec![&[][..]; hard.entries + 1];
    let entries = input(class(&[]), &names);
    let err = decode(&entries, 0, hard).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Context(class_context::ErrorKind::Name(
        metadata::name_resolver::Error { kind: metadata::name_resolver::ErrorKind::Limit("entries"), .. }
    ))));
    assert_eq!(decode(&entries, 0, huge).unwrap_err(), err);
    let table = message(1, &scalar(1, (hard.range_slots + 1) as u64));
    let ranges = input(class_with_table(&[], &table), NAMES);
    let err = decode(&ranges, 0, hard).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Context(class_context::ErrorKind::Name(
        metadata::name_resolver::Error { kind: metadata::name_resolver::ErrorKind::Limit("range_slots"), .. }
    ))));
    assert_eq!(decode(&ranges, 0, huge).unwrap_err(), err);
    let field = scalar(99, 0);
    let records = input_for(&field.repeat(hard.records + 1));
    let err = decode(&records, 0, hard).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Context(class_context::ErrorKind::Class(
        metadata::class_header::ErrorKind::Wire(wire::ErrorKind::Limit("records"))
    ))));
    assert_eq!(decode(&records, 0, huge).unwrap_err(), err);
}

#[test]
fn authored_wrong_container_versions_and_hostile_truncations_refuse() {
    for (kind, version) in [(2, vec![2, 3, 0]), (1, vec![2, 2, 0])] {
        let mut input = ordinary();
        let metadata::Outcome::Decoded { header, .. } = &mut input else { unreachable!() };
        header.k = kind; header.mv = version;
        assert_eq!(decode(&input, 3, Limits::default()).unwrap_err().kind,
            ErrorKind::Context(class_context::ErrorKind::UnsupportedContainer));
    }
    let metadata::Outcome::Decoded { bytes, .. } = ordinary() else { unreachable!() };
    for end in 0..bytes.len() {
        assert!(decode(&input(bytes[..end].to_vec(), NAMES), 3, Limits::default()).is_err());
    }
    for ty in [vec![0x80], vec![0x38, 0x80], vec![0x0b], vec![0x30, 0x80], vec![0x12, 0xff]] {
        assert!(decode(&input_for(&fields(&value(4, &ty))), 3, Limits::default()).is_err());
    }
}
