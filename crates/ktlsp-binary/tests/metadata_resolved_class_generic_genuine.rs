// Original Kotlin stdlib 2.3.20 data, not compiler-generated test doubles.
// Receipt: ktlsp-next-original-generic-class-data.json, 64360 bytes, SHA256
// 77f572ca59838e2a39011627ec4d922e9346b2b5aa7d98ccae1555bf3e7610fe.
// Archive SHA256: 0ae12504a5040ebaf37703908483420d1a5624dd1d93f357665f8c77c848a01e.
// ClosedRange: 1033 bytes, ZIP ordinal658/method8/CRCfd5d0861, SHA256
// 50c6eeaad95ce574993ee94a1cc490660ea3af6c3d6abe5db0914640cffe080a.
// ClosedFloatingPointRange: 1194 bytes, ordinal656/method8/CRCbfffc1aa, SHA256
// 7ed8a4ea14de1c471c3a37cdf69b215a699dc6e15835825419424698b59266dc.
// Existing Kotlin Apache-2.0 LICENSE/NOTICE: ../fixtures/t017c2/.
// Existing protobuf notice: fixtures/metadata_wire/PROTOBUF_LICENSE.txt.
// These assertions are source-derived expectations, not observed API outputs.
// Exact compiler/vendor authenticity and selected genuine TypeTable IDs are unknown.
use ktlsp_binary::{
    annotations::{self, Value, Visibility},
    metadata::{
        self, class_context, name_resolver,
        resolved_class_function_parameters::{self as classifier, Classifier},
        resolved_class_function_signature::Argument,
        resolved_class_supertypes::{ClassSupertypes, Error, ErrorKind, Limits, Supertype, decode},
        type_header::TypeHeader,
        wire,
    },
};
use std::ops::Range;

const RANGE: &[u8] = include_bytes!("fixtures/metadata_class_bounds/ClosedRange.class.bin");
const FLOATING: &[u8] =
    include_bytes!("fixtures/metadata_class_bounds/ClosedFloatingPointRange.class.bin");
const RANGE_D1: &[u8] = &[
    26, 10, 2, 24, 2, 10, 0, 10, 2, 16, 15, 10, 2, 16, 0, 10, 2, 8, 6, 10, 2, 16,
    11, 10, 2, 8, 4, 8, 102, 24, 0, 42, 14, 8, 0, 16, 1, 42, 8, 18, 4, 18, 2, 72,
    1, 48, 2, 50, 2, 48, 3, 74, 23, 16, 9, 26, 2, 48, 10, 50, 6, 16, 11, 26, 2, 56,
    0, 72, 150, 130, 4, 162, 6, 2, 16, 12, 74, 10, 16, 13, 26, 2, 48, 10, 72, 150,
    128, 4, 82, 19, 16, 4, 26, 2, 56, 0, 88, 166, 132, 8, 162, 6, 6, 26, 4, 8, 5,
    16, 6, 82, 19, 16, 7, 26, 2, 56, 0, 88, 166, 132, 8, 162, 6, 6, 26, 4, 8, 8,
    16, 6, 168, 6, 14,
];
const FLOATING_D1: &[u8] = &[
    24, 10, 2, 24, 2, 10, 0, 10, 2, 16, 15, 10, 2, 24, 2, 10, 0, 10, 2, 16, 11, 10,
    2, 8, 8, 8, 103, 24, 0, 42, 14, 8, 0, 16, 1, 42, 8, 18, 4, 18, 2, 72, 1, 48,
    2, 50, 8, 18, 4, 18, 2, 72, 1, 48, 3, 74, 23, 16, 4, 26, 2, 48, 5, 50, 6, 16,
    6, 26, 2, 56, 0, 72, 150, 130, 4, 162, 6, 2, 16, 7, 74, 10, 16, 8, 26, 2, 48,
    5, 72, 150, 128, 4, 74, 31, 16, 9, 26, 2, 48, 5, 50, 6, 16, 10, 26, 2, 56, 0,
    50, 6, 16, 11, 26, 2, 56, 0, 72, 166, 128, 4, 162, 6, 2, 16, 12, 168, 6, 13,
];

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn bytes<'a>(input: &'a metadata::Outcome<'_>) -> &'a [u8] {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("supported original Class")
    };
    bytes
}
fn bytes_mut<'a>(input: &'a mut metadata::Outcome<'_>) -> &'a mut Vec<u8> {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("supported original Class")
    };
    bytes
}
fn borrowed(raw: &[u8], input: &[u8], span: Range<usize>) {
    assert_eq!(raw, &input[span.clone()]);
    assert_eq!(raw.as_ptr(), input[span.clone()].as_ptr());
    println!("actual borrowed span={span:?} raw={raw:?} ptr={:p} retained={:p}",
        raw.as_ptr(), input[span].as_ptr());
}
fn identity(id: &metadata::class_id::ClassId, package: &str, relative: &str) {
    assert_eq!(id.package_name(), units(package));
    assert_eq!(id.relative_class_name(), units(relative));
    assert!(!id.local);
}
fn header(raw: &[u8], class_name: Option<i32>, name: Option<i32>) -> TypeHeader<'_> {
    TypeHeader {
        raw,
        flags: None,
        nullable: None,
        flexible_type_capabilities_id: None,
        class_name,
        type_parameter: None,
        flexible_upper_bound_id: None,
        type_parameter_name: name,
        outer_type_id: None,
        type_alias_name: None,
        abbreviated_type_id: None,
    }
}
fn argument(arg: &Argument<'_>, input: &[u8], body: Range<usize>) {
    assert_eq!(arg.body_span, body);
    assert_eq!(arg.type_span, body.start + 2..body.end);
    borrowed(arg.raw.raw, input, body.clone());
    assert_eq!(arg.raw.raw, [0x12, 2, 0x48, 1]);
    assert_eq!(arg.raw.projection, None); // Absent, NOT encoded IN/INV.
    assert_eq!(arg.raw.type_id, None);
    assert_eq!(arg.raw.inline_types.len(), 1);
    let leaf = &arg.raw.inline_types[0];
    assert_eq!(leaf.span, 2..4); // This one span is Argument-relative.
    borrowed(leaf.raw, arg.raw.raw, leaf.span.clone());
    borrowed(leaf.raw, input, arg.type_span.clone());
    borrowed(arg.header.raw, input, arg.type_span.clone());
    assert_eq!(arg.header, header(&[0x48, 1], None, Some(1)));
    // Encoded NAME1 binds declaration ordinal0, never encoded ID1 or Function scope.
    assert_eq!(arg.classifier, Classifier::ClassParameter(0));
}
fn supertype(
    ty: &Supertype<'_>, input: &[u8], ordinal: usize, outer: Range<usize>,
    index: i32, target: (&str, &str), generic: bool,
) {
    assert_eq!(ty.ordinal, ordinal);
    assert_eq!(ty.span, outer);
    assert_eq!(ty.body_span, outer.start + 2..outer.end);
    borrowed(ty.header.raw, input, ty.body_span.clone());
    let raw: &[u8] = if generic {
        if index == 2 { &[0x12, 4, 0x12, 2, 0x48, 1, 0x30, 2] }
        else { &[0x12, 4, 0x12, 2, 0x48, 1, 0x30, 3] }
    } else { &[0x30, 3] };
    assert_eq!(ty.header, header(raw, Some(index), None));
    assert_eq!(input[outer.start], if index == 2 { 0x2a } else { 0x32 });
    assert_eq!(usize::from(input[outer.start + 1]), raw.len());
    identity(&ty.class_id, target.0, target.1);
    assert_eq!(ty.arguments.len(), usize::from(generic));
    if generic {
        argument(&ty.arguments[0], input, ty.body_span.start + 2..ty.body_span.start + 6);
    }
}
fn complete(out: &ClassSupertypes<'_>, input: &metadata::Outcome<'_>, floating: bool) {
    let data = bytes(input);
    let (start, end, owner, flags) = if floating {
        (25, 128, "ClosedFloatingPointRange", 103)
    } else { (27, 133, "ClosedRange", 102) };
    identity(&out.owner.class_id, "kotlin.ranges", owner);
    assert_eq!(out.owner.body_span, start..end);
    assert_eq!(out.owner.header.fq_name, 0);
    assert_eq!(out.owner.header.flags, Some(flags));
    borrowed(out.owner.header.raw, data, start..end);
    assert_eq!(out.parameters.len(), 1);
    let p = &out.parameters[0];
    assert_eq!(p.body_span, start + 6..start + 20);
    borrowed(p.parameter.raw, data, p.body_span.clone());
    assert_eq!(p.parameter.raw, [8, 0, 16, 1, 0x2a, 8, 0x12, 4, 0x12, 2, 0x48, 1, 0x30, 2]);
    assert_eq!(p.parameter.id, 0);
    assert_eq!(p.parameter.name, 1);
    assert_eq!(p.name, [84]);
    assert_eq!(p.parameter.reified, None);
    assert_eq!(p.parameter.variance, None);
    assert!(p.parameter.upper_bound_id.is_empty());
    assert_eq!(p.parameter.upper_bound.len(), 1);
    let raw_bound = &p.parameter.upper_bound[0];
    assert_eq!(raw_bound.span, 6..14); // TypeParameter-relative raw custody retained.
    borrowed(raw_bound.raw, p.parameter.raw, 6..14);
    assert_eq!(out.bounds.len(), 1);
    let bound = &out.bounds[0];
    assert_eq!(bound.parameter, 0);
    assert!(bound.parameter < out.parameters.len());
    supertype(&bound.ty, data, 2, start + 10..start + 20, 2, ("kotlin", "Comparable"), true);
    assert_eq!(raw_bound.raw.as_ptr(), bound.ty.header.raw.as_ptr());
    assert_eq!(raw_bound.raw, bound.ty.header.raw);
    assert_ne!(bound.ty.class_id, out.owner.class_id);
    assert_eq!(out.supertypes.len(), 1);
    let ty = &out.supertypes[0];
    supertype(ty, data, 3, start + 20..if floating { 55 } else { 51 }, 3,
        (if floating { "kotlin.ranges" } else { "kotlin" },
        if floating { "ClosedRange" } else { "Any" }), floating);
    assert_ne!(ty.class_id, out.owner.class_id);
    assert_ne!(ty.class_id, bound.ty.class_id);
    for arg in bound.ty.arguments.iter().chain(&ty.arguments) {
        let Classifier::ClassParameter(ordinal) = &arg.classifier else {
            panic!("immediate Class scope, not a Function or nominal leaf")
        };
        assert_eq!(*ordinal, bound.parameter);
        let declaration = &out.parameters[*ordinal];
        assert_eq!(declaration.parameter.id, 0);
        assert_eq!(declaration.name, [84]);
        assert_eq!(declaration.parameter.raw.as_ptr(), data[p.body_span.clone()].as_ptr());
    }
}

// Each dimension is discovered for THIS composed operation only, at future execution.
type Set = fn(&mut Limits, usize);
type Get = fn(Limits) -> usize;
const DIMENSIONS: [(&str, Set, Get); 10] = [
    ("bytes", |l, n| l.bytes = n, |l| l.bytes),
    ("records", |l, n| l.records = n, |l| l.records),
    ("nodes", |l, n| l.nodes = n, |l| l.nodes),
    ("depth", |l, n| l.depth = n, |l| l.depth),
    ("work", |l, n| l.work = n, |l| l.work),
    ("owned_bytes", |l, n| l.owned_bytes = n, |l| l.owned_bytes),
    ("entries", |l, n| l.entries = n, |l| l.entries),
    ("range_slots", |l, n| l.range_slots = n, |l| l.range_slots),
    ("queries", |l, n| l.queries = n, |l| l.queries),
    ("output_units", |l, n| l.output_units = n, |l| l.output_units),
];
fn minimum(input: &metadata::Outcome<'_>, set: Set, get: Get) -> usize {
    let accepted = |n| {
        let mut limits = Limits::default();
        set(&mut limits, n);
        decode(input, limits).is_ok()
    };
    let (mut low, mut high) = (0, get(Limits::default()));
    assert!(!accepted(low));
    assert!(accepted(high));
    while high - low > 1 {
        let mid = low + (high - low) / 2;
        if accepted(mid) { high = mid; } else { low = mid; }
    }
    high
}
fn assert_limit(error: Error, label: &str) {
    let found = match error.kind {
        ErrorKind::Wire(wire::ErrorKind::Limit(found))
        | ErrorKind::Context(class_context::ErrorKind::Wire(wire::ErrorKind::Limit(found)))
        | ErrorKind::Context(class_context::ErrorKind::Class(
            metadata::class_header::ErrorKind::Wire(wire::ErrorKind::Limit(found))))
        | ErrorKind::Context(class_context::ErrorKind::TypeParameter(
            metadata::type_parameter::ErrorKind::Wire(wire::ErrorKind::Limit(found))))
        | ErrorKind::Classifier(classifier::ErrorKind::Wire(wire::ErrorKind::Limit(found))) => found,
        ErrorKind::Context(class_context::ErrorKind::Name(e))
        | ErrorKind::Classifier(classifier::ErrorKind::Name(e)) => {
            let name_resolver::ErrorKind::Limit(found) = e.kind else {
                panic!("expected {label} limit, actual {error:?}")
            };
            found
        }
        _ => panic!("expected {label} limit, actual {error:?}"),
    };
    assert_eq!(found, label, "actual {error:?}");
}
fn resources(input: &metadata::Outcome<'_>, floating: bool) -> Limits {
    let mut exact = Limits::default();
    for (label, set, get) in DIMENSIONS {
        let n = minimum(input, set, get);
        println!("actual minimum {label}={n}");
        set(&mut exact, n);
    }
    let actual = decode(input, exact).unwrap();
    println!("actual joint exact limits={exact:?} output={actual:#?}");
    complete(&actual, input, floating);
    assert_eq!(actual, decode(input, Limits::default()).unwrap());
    for (label, set, get) in DIMENSIONS {
        let mut below = exact;
        set(&mut below, get(exact) - 1);
        let error = decode(input, below).unwrap_err();
        println!("actual joint one-short {label} limits={below:?} error={error:?}");
        assert_limit(error, label);
    }
    exact
}
fn resource_mutations(input: &mut metadata::Outcome<'_>, exact: Limits, floating: bool) {
    println!("MUTATION CONTROLS: not unchanged-carrier evidence");
    let original = bytes(input).to_vec();
    let start = if floating { 25 } else { 27 };
    let super_end = if floating { 55 } else { 51 };
    let super_field = original[start + 20..super_end].to_vec();
    // Both children independently fit the same query/output allowance; their union
    // must not reset the one resolver. Declaration and owner stay in BOTH controls.
    for (label, set, get) in [DIMENSIONS[8], DIMENSIONS[9]] {
        bytes_mut(input).drain(start + 20..super_end); // bound-only mutation
        let bound_minimum = minimum(input, set, get);
        let mut bound_limits = Limits::default();
        set(&mut bound_limits, bound_minimum);
        let bound_only = decode(input, bound_limits).unwrap();
        assert_eq!(bound_only.bounds.len(), 1);
        assert!(bound_only.supertypes.is_empty());
        println!("actual mutation bound-only {label} limits={bound_limits:?} output={bound_only:#?}");
        *bytes_mut(input) = original.clone();
        bytes_mut(input).drain(start + 10..start + 20); // supertype-only mutation
        bytes_mut(input)[start + 5] -= 10; // retain original ID0/name1 declaration
        let super_minimum = minimum(input, set, get);
        let mut shared = Limits::default();
        set(&mut shared, bound_minimum.max(super_minimum));
        let super_only = decode(input, shared).unwrap();
        assert!(super_only.bounds.is_empty());
        assert_eq!(super_only.supertypes.len(), 1);
        println!("actual mutation supertype-only {label} limits={shared:?} output={super_only:#?}");
        *bytes_mut(input) = original.clone();
        assert!(get(exact) > get(shared));
        let error = decode(input, shared).unwrap_err();
        println!("actual combined-vs-isolated {label} limits={shared:?} error={error:?}");
        assert_limit(error, label);
        assert!(error.offset >= start + 22, "failure must reach later supertype: {error:?}");
    }
    // A later repeated original selected child needs cumulative storage/work/name
    // resources, even though its classifier and parameter identity are unchanged.
    bytes_mut(input).extend(&super_field);
    let repeated = decode(input, Limits::default()).unwrap();
    println!("actual mutation appended original supertype output={repeated:#?}");
    assert_eq!(repeated.bounds.len(), 1);
    assert_eq!(repeated.supertypes.len(), 2);
    assert_eq!(repeated.supertypes[1].span, original.len()..original.len() + super_field.len());
    assert_eq!(repeated.supertypes[0].class_id, repeated.supertypes[1].class_id);
    for dimension in [1, 2, 4, 5, 8, 9] {
        let (label, set, get) = DIMENSIONS[dimension];
        let mut limited = Limits::default();
        set(&mut limited, get(exact));
        let error = decode(input, limited).unwrap_err();
        println!("actual cumulative later-child {label} limits={limited:?} error={error:?}");
        assert_limit(error, label);
        // Parent rescans can spend record/work first. Selected-node/name failures
        // must reach the appended child, proving earlier selected children fit.
        if matches!(dimension, 2 | 8 | 9) {
            assert!(error.offset >= original.len(), "{label}: {error:?}");
        }
        assert!(minimum(input, set, get) > get(exact));
    }
    *bytes_mut(input) = original.clone();
    // Repeated bounds use the SAME monotonic allowance too. The second root is
    // another original-field occurrence, not a recursively expanded parameter.
    let bound_field = original[start + 10..start + 20].to_vec();
    bytes_mut(input).splice(start + 20..start + 20, bound_field);
    bytes_mut(input)[start + 5] += 10;
    let repeated = decode(input, Limits::default()).unwrap();
    println!("actual mutation repeated inline bound output={repeated:#?}");
    assert_eq!(repeated.parameters.len(), 1);
    assert_eq!(repeated.bounds.len(), 2);
    assert_eq!(repeated.supertypes.len(), 1);
    for (ordinal, bound) in repeated.bounds.iter().enumerate() {
        assert_eq!(bound.parameter, 0);
        supertype(&bound.ty, bytes(input), 2 + ordinal,
            start + 10 + ordinal * 10..start + 20 + ordinal * 10,
            2, ("kotlin", "Comparable"), true);
    }
    for dimension in [1, 2, 4, 5, 8, 9] {
        let (label, set, get) = DIMENSIONS[dimension];
        let mut limited = Limits::default();
        set(&mut limited, get(exact));
        let error = decode(input, limited).unwrap_err();
        println!("actual cumulative repeated-bound {label} limits={limited:?} error={error:?}");
        assert_limit(error, label);
        if matches!(dimension, 8 | 9) {
            assert!(error.offset >= start + 22, "{label}: {error:?}");
        }
        assert!(minimum(input, set, get) > get(exact));
    }
    *bytes_mut(input) = original;
}

// Shared ONLY by this integration test and fixtures/metadata_class_bounds/public_consumer.rs.
// Every printed result below is an actual public API value, not an expected-value echo.
pub fn run() {
    for (raw, floating) in [(RANGE, false), (FLOATING, true)] {
        let name = if floating { "ClosedFloatingPointRange" } else { "ClosedRange" };
        println!("UNCHANGED ORIGINAL {name}");
        let class = ktlsp_binary::parse_class(raw).unwrap();
        println!("actual class={class:#?}");
        assert_eq!((class.major, class.minor), (52, 0));
        assert_eq!((class.access_flags, class.this_class, class.super_class), (1537, 2, 5));
        assert!(class.fields.is_empty());
        assert_eq!(class.methods.len(), if floating { 3 } else { 4 });
        assert_eq!(class.interfaces, if floating { vec![7] } else { vec![] });
        let attribute_spans: &[(u16, usize, usize)] = if floating {
            &[(46, 1060, 1070), (44, 1076, 1078), (47, 1084, 1086),
                (48, 1092, 1177), (49, 1183, 1194)]
        } else { &[(42, 913, 923), (39, 929, 931), (43, 937, 939), (44, 945, 1033)] };
        assert_eq!(class.attributes.len(), attribute_spans.len());
        for (attribute, &(index, start, end)) in class.attributes.iter().zip(attribute_spans) {
            assert_eq!(attribute.name, index);
            borrowed(attribute.bytes, raw, start..end);
        }
        let annotations = annotations::decode(&class).unwrap();
        println!("actual annotations={annotations:#?}");
        let mut input = metadata::decode(&annotations, Default::default()).unwrap();
        println!("actual metadata={input:#?}");
        let metadata::Outcome::Decoded { bytes: data, header: h } = &input else {
            panic!("supported original mv2.3.0 kind1")
        };
        assert_eq!(data, if floating { FLOATING_D1 } else { RANGE_D1 });
        assert_eq!(h.present, 0b1001_1011);
        assert_eq!(h.k, 1);
        assert_eq!(h.mv, [2, 3, 0]);
        assert_eq!(h.xi, 48);
        assert_eq!(h.bv, [1, 0, 3]); // Absent bv retains the existing schema default.
        assert!(h.xs.is_empty());
        assert!(h.pn.is_empty());
        assert_eq!(*h.visibility, Visibility::RuntimeVisible);
        assert!(std::ptr::eq(h.raw, &annotations.class[0].annotations[0]));
        assert!(std::ptr::eq(h.visibility, &annotations.class[0].visibility));
        assert_eq!(h.raw.descriptor, units("Lkotlin/Metadata;"));
        assert_eq!(h.raw.elements.len(), 5);
        for (element, expected) in h.raw.elements.iter().zip(["mv", "k", "xi", "d1", "d2"]) {
            assert_eq!(element.name, units(expected));
        }
        assert_eq!(h.raw.elements[0].value, Value::Array(vec![Value::Int(2), Value::Int(3), Value::Int(0)]));
        assert_eq!(h.raw.elements[1].value, Value::Int(1));
        assert_eq!(h.raw.elements[2].value, Value::Int(48));
        let cp_d1 = class.utf16(if floating { 30 } else { 26 }).unwrap();
        assert_eq!(h.d1, [cp_d1]);
        assert_eq!(h.d1[0].as_ptr(), cp_d1.as_ptr());
        let mut d1_units = vec![0];
        d1_units.extend(data.iter().map(|&byte| u16::from(byte)));
        assert_eq!(cp_d1, d1_units);
        assert_eq!(h.raw.elements[3].value, Value::Array(vec![Value::String(cp_d1)]));
        let indices: &[u16] = if floating {
            &[32, 33, 34, 35, 8, 34, 36, 9, 12, 14, 37, 38, 15, 39]
        } else { &[28, 29, 30, 30, 31, 6, 7, 32, 10, 11, 30, 33, 12, 14, 34] };
        let texts: &[&str] = if floating {
            &["Lkotlin/ranges/ClosedFloatingPointRange;", "T", "", "Lkotlin/ranges/ClosedRange;",
                "contains", "", "value", "(Ljava/lang/Comparable;)Z", "isEmpty", "lessThanOrEquals",
                "a", "b", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)Z", "kotlin-stdlib"]
        } else {
            &["Lkotlin/ranges/ClosedRange;", "T", "", "", "start", "getStart", "()Ljava/lang/Comparable;",
                "endInclusive", "getEndInclusive", "contains", "", "value", "(Ljava/lang/Comparable;)Z",
                "isEmpty", "kotlin-stdlib"]
        };
        assert_eq!(h.d2.len(), indices.len());
        for ((actual, &index), expected) in h.d2.iter().zip(indices).zip(texts) {
            let cp = class.utf16(index).unwrap();
            assert_eq!(*actual, cp);
            assert_eq!(actual.as_ptr(), cp.as_ptr());
            assert_eq!(*actual, units(expected));
            println!("actual d2 CP{index} units={actual:?} ptr={:p}", actual.as_ptr());
        }
        assert_eq!(h.raw.elements[4].value,
            Value::Array(h.d2.iter().map(|text| Value::String(text)).collect()));
        // Independently bounded public resolver controls use actual compact records,
        // not CP-name guessing (d2[2] is empty but record2 is predefined Comparable).
        let framed = metadata::payload::decode(data, Default::default()).unwrap();
        assert_eq!(framed.payload_span, if floating { 25..128 } else { 27..133 });
        assert_eq!(framed.string_table_span, if floating { 1..25 } else { 1..27 });
        assert!(framed.string_table.local_name.is_empty());
        let mut resolver = name_resolver::Resolver::new(&framed.string_table, &h.d2, Default::default()).unwrap();
        let resolved: Vec<_> = [0, 1, 2, 3].into_iter().map(|index| {
            (index, resolver.get_string(index).unwrap(), resolver.is_local_class_name(index).unwrap())
        }).collect();
        println!("actual independent resolver controls={resolved:?}");
        assert_eq!(resolved[0].1, units(&format!("kotlin/ranges/{name}")));
        assert_eq!(resolved[1].1, [84]);
        assert_eq!(resolved[2].1, units("kotlin/Comparable"));
        assert_eq!(resolved[3].1, units(if floating { "kotlin/ranges/ClosedRange" } else { "kotlin/Any" }));
        assert!(resolved.iter().all(|entry| !entry.2));
        let actual = decode(&input, Limits::default()).unwrap();
        println!("actual composed class bounds/supertypes={actual:#?}");
        complete(&actual, &input, floating);
        let exact = resources(&input, floating);
        resource_mutations(&mut input, exact, floating);
        assert_eq!(bytes(&input), if floating { FLOATING_D1 } else { RANGE_D1 });
    }
}

#[test]
fn unchanged_original_class_bounds_generic_supertypes_and_joint_allowances() {
    run();
}
