use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, class_context,
        resolved_class_function_parameters::{Classifier, ErrorKind, Limits, decode},
        wire,
    },
};

static RAW: Annotation<'static> = Annotation {
    descriptor: &[],
    elements: Vec::new(),
};
const NAMES: &[&[u16]] = &[
    &[112, 107, 103, 47, 66, 111, 120], // pkg/Box
    &[65],
    &[66],
    &[99, 111, 112, 121],
    &[102, 105, 114, 115, 116],
    &[115, 101, 99, 111, 110, 100],
    &[107, 111, 116, 108, 105, 110, 47, 65, 110, 121],
];
fn input<'a>(bytes: Vec<u8>, names: &[&'a [u16]]) -> metadata::Outcome<'a> {
    metadata::Outcome::Decoded {
        bytes,
        header: metadata::Header {
            raw: &RAW,
            visibility: &Visibility::RuntimeVisible,
            present: 0,
            k: 1,
            mv: vec![2, 3, 0],
            bv: vec![],
            d1: vec![],
            d2: names.to_vec(),
            xs: &[],
            pn: &[],
            xi: 0,
        },
    }
}
fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 128 {
        bytes.push(value as u8 | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}
fn scalar(number: u32, value: u64) -> Vec<u8> {
    let mut bytes = varint(u64::from(number) * 8);
    bytes.extend(varint(value));
    bytes
}
fn message(number: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = varint(u64::from(number) * 8 + 2);
    bytes.extend(varint(body.len() as u64));
    bytes.extend(body);
    bytes
}
fn declaration(id: i32, name: u8) -> Vec<u8> {
    let mut body = scalar(1, u64::from(id as u32));
    body.extend([0x10, name]);
    message(5, &body)
}
fn leaf(id: i32) -> Vec<u8> {
    scalar(7, u64::from(id as u32))
}
fn value_body(name: u8, ty: &[u8]) -> Vec<u8> {
    let mut body = vec![0x10, name];
    body.extend(message(3, ty));
    body
}
fn value(name: u8, ty: &[u8]) -> Vec<u8> {
    message(6, &value_body(name, ty))
}
fn function(roles: &[u8]) -> Vec<u8> {
    let mut body = vec![0x10, 3];
    body.extend(roles);
    message(9, &body)
}
fn class_with_table(fields: &[u8], table: &[u8]) -> Vec<u8> {
    let mut bytes = varint(table.len() as u64);
    bytes.extend(table);
    bytes.extend([0x18, 0]);
    bytes.extend(fields);
    bytes
}
fn class(fields: &[u8]) -> Vec<u8> {
    class_with_table(fields, &[0x0a, 2, 8, 7])
}
fn fields(roles: &[u8]) -> Vec<u8> {
    let mut bytes = declaration(0, 1);
    bytes.extend(declaration(1, 2));
    bytes.extend(function(roles));
    bytes
}
fn ordinary_roles() -> Vec<u8> {
    let mut roles = value(4, &leaf(0));
    roles.extend(value(5, &leaf(1)));
    roles
}
fn ordinary() -> metadata::Outcome<'static> {
    input(class(&fields(&ordinary_roles())), NAMES)
}
fn kind(roles: &[u8]) -> ErrorKind {
    decode(&input(class(&fields(roles)), NAMES), 3, Limits::default())
        .unwrap_err()
        .kind
}
fn type_kind(ty: &[u8]) -> ErrorKind {
    kind(&value(4, ty))
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn borrowed(raw: &[u8], bytes: &[u8], span: std::ops::Range<usize>) {
    assert_eq!(raw, &bytes[span.clone()]);
    assert_eq!(raw.as_ptr(), bytes[span].as_ptr());
}

#[test]
fn genuine_pair_copy_and_equals_preserve_same_outcome_original_identity() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        panic!("decoded Class")
    };
    let result = decode(&input, 9, Limits::default()).unwrap();
    assert_eq!(result.owner.body_span, 47..285);
    borrowed(result.owner.header.raw, bytes, 47..285);
    assert_eq!(result.owner.class_id.package_name(), units("kotlin"));
    assert_eq!(result.owner.class_id.relative_class_name(), units("Pair"));
    assert!(!result.owner.class_id.local);
    assert_eq!(result.parameters.len(), 2);
    let function = &result.function;
    assert_eq!(function.ordinal, 9);
    assert_eq!(function.span, 149..198);
    assert_eq!(function.body_span, 151..198);
    borrowed(function.header.raw, bytes, 151..198);
    assert_eq!(function.header.name, 17);
    assert_eq!(function.name, units("copy"));
    assert_eq!(function.parameters.len(), 2);
    for (ordinal, (name, span, body_span, type_span, declaration_span)) in [
        ("first", 169..179, 171..179, 177..179, 54..60),
        ("second", 179..189, 181..189, 187..189, 62..68),
    ]
    .into_iter()
    .enumerate()
    {
        let value = &function.parameters[ordinal];
        assert_eq!(value.ordinal, ordinal);
        assert_eq!(value.span, span);
        assert_eq!(value.body_span, body_span);
        assert_eq!(value.type_span, type_span);
        borrowed(value.raw.raw, bytes, body_span);
        borrowed(value.header.raw, bytes, type_span);
        assert_eq!(value.name, units(name));
        assert_eq!(value.classifier, Classifier::ClassParameter(ordinal));
        assert_eq!(value.header.type_parameter, Some(ordinal as i32));
        assert_eq!(value.header.nullable, None);
        assert_eq!(value.raw.flags, Some(2));
        assert!(value.declares_default_value());
        assert!(!value.has_annotations());
        assert!(!value.is_crossinline());
        assert!(!value.is_noinline());
        let declaration = &result.parameters[ordinal];
        assert_eq!(declaration.parameter.id, ordinal as i32);
        assert_eq!(declaration.body_span, declaration_span);
        borrowed(declaration.parameter.raw, bytes, declaration_span);
        assert_eq!(
            declaration.name,
            units(if ordinal == 0 { "A" } else { "B" })
        );
        assert_eq!(
            declaration.parameter.variance,
            Some(metadata::type_parameter::Variance::Out)
        );
    }
    assert_eq!(
        decode(&input, 3, Limits::default()).unwrap_err().kind,
        ErrorKind::NotFunction
    );
    let equals = decode(&input, 10, Limits::default()).unwrap();
    assert_eq!(equals.function.span, 198..220);
    assert_eq!(equals.function.body_span, 200..220);
    assert_eq!(equals.function.name, units("equals"));
    borrowed(equals.function.header.raw, bytes, 200..220);
    assert_eq!(equals.function.parameters.len(), 1);
    let other = &equals.function.parameters[0];
    assert_eq!(other.ordinal, 0);
    assert_eq!(other.span, 206..216);
    assert_eq!(other.body_span, 208..216);
    assert_eq!(other.type_span, 212..216);
    borrowed(other.raw.raw, bytes, 208..216);
    borrowed(other.header.raw, bytes, 212..216);
    assert_eq!(other.raw.name, 21);
    assert_eq!(other.name, units("other"));
    assert_eq!(other.raw.flags, None);
    assert!(!other.declares_default_value());
    assert_eq!(other.header.class_name, Some(22));
    assert_eq!(other.header.nullable, Some(true));
    assert!(header.d2[22].is_empty());
    let Classifier::Class(id) = &other.classifier else {
        panic!("nominal Any")
    };
    assert_eq!(id.package_name(), units("kotlin"));
    assert_eq!(id.relative_class_name(), units("Any"));
    assert!(!id.local);
    for declaration in &equals.parameters {
        borrowed(
            declaration.parameter.raw,
            bytes,
            declaration.body_span.clone(),
        );
    }
}

#[test]
fn signed_reordered_ids_and_equal_names_keep_original_distinct_declarations() {
    let mut roles = value(4, &leaf(-7));
    roles.extend(value(4, &leaf(12345)));
    roles.extend(value(4, &leaf(-7))); // Equal names/types are separate value declarations.
    let mut bytes = function(&roles);
    bytes.extend(declaration(12345, 1));
    bytes.extend(declaration(-7, 1)); // Class declarations may follow the selected Function.
    let input = input(class(&bytes), NAMES);
    let result = decode(&input, 1, Limits::default()).unwrap();
    assert_eq!(result.parameters[0].name, result.parameters[1].name);
    assert_eq!(result.parameters[0].parameter.id, 12345);
    assert_eq!(result.parameters[1].parameter.id, -7);
    for (ordinal, expected) in [1, 0, 1].into_iter().enumerate() {
        let value = &result.function.parameters[ordinal];
        assert_eq!(value.ordinal, ordinal);
        assert_eq!(value.classifier, Classifier::ClassParameter(expected));
    }
    assert_ne!(
        result.function.parameters[0].raw.raw.as_ptr(),
        result.function.parameters[2].raw.raw.as_ptr()
    );
    let mut duplicate = declaration(0, 1);
    duplicate.extend(declaration(0, 2));
    duplicate.extend(function(&ordinary_roles()));
    assert_eq!(
        decode(&input_for(&duplicate), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Context(class_context::ErrorKind::DuplicateParameterId)
    );
    assert_eq!(type_kind(&leaf(99)), ErrorKind::UnknownParameterId);
    assert_eq!(type_kind(&scalar(9, 1)), ErrorKind::UnknownParameterName);
    assert!(matches!(
        kind(&message(4, &[])),
        ErrorKind::Context(class_context::ErrorKind::TypeParameter(
            metadata::type_parameter::ErrorKind::MissingId
        ))
    ));
    assert_eq!(kind(&scalar(4, 0)), ErrorKind::UnsupportedFunction);
}
fn input_for(fields: &[u8]) -> metadata::Outcome<'static> {
    input(class(fields), NAMES)
}

#[test]
fn inline_precedence_presence_and_retained_raw_flag_accessors() {
    for nullable in [None, Some(false), Some(true)] {
        for flags in [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(4),
            Some(8),
            Some(-1),
            Some(16),
        ] {
            let mut ty = leaf(0);
            if let Some(marking) = nullable {
                ty.extend(scalar(3, u64::from(marking)));
            }
            let mut body = value_body(4, &ty);
            body.extend(scalar(5, u64::from(u32::MAX))); // Inline wins; no lookup.
            if let Some(flags) = flags {
                body.extend(scalar(1, u64::from(flags as u32)));
            }
            let input = input_for(&fields(&message(6, &body)));
            let result = decode(&input, 3, Limits::default()).unwrap();
            let value = &result.function.parameters[0];
            assert_eq!(value.header.nullable, nullable);
            assert_eq!(value.raw.flags, flags);
            assert_eq!(value.raw.type_id, Some(-1));
            let bits = flags.unwrap_or(0);
            assert_eq!(value.has_annotations(), bits & 1 != 0);
            assert_eq!(value.declares_default_value(), bits & 2 != 0);
            assert_eq!(value.is_crossinline(), bits & 4 != 0);
            assert_eq!(value.is_noinline(), bits & 8 != 0);
        }
    }
    let mut duplicate = value_body(4, &leaf(0));
    duplicate.extend(message(3, &leaf(1)));
    assert_eq!(
        kind(&message(6, &duplicate)),
        ErrorKind::DuplicateInlineRole
    );
    assert_eq!(kind(&message(6, &[0x10, 4])), ErrorKind::MissingRole);
    assert_eq!(
        kind(&message(6, &[0x10, 4, 0x28, 0])),
        ErrorKind::UnsupportedTableRole
    );
    // Int32 width and last-recognized scalar semantics are unchanged.
    let mut ty = leaf(0);
    ty.extend(scalar(1, 1));
    ty.extend(scalar(1, 1u64 << 32));
    ty.extend(scalar(7, 1u64 << 32));
    let input = input_for(&fields(&value(4, &ty)));
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.function.parameters[0].header.flags, Some(0));
    assert_eq!(
        result.function.parameters[0].classifier,
        Classifier::ClassParameter(0)
    );
}

#[test]
fn nominal_resolution_uses_real_operations_predefined_local_and_lossless_utf16() {
    for (record, encoded, expected) in [
        (
            vec![0x18, 1],
            units("a/b/Outer$Inner"),
            units("Outer.Inner"),
        ),
        (
            vec![0x18, 2],
            units("La/b/Outer$Inner;"),
            units("Outer.Inner"),
        ),
    ] {
        let mut table = message(1, &[8, 6]); // First six fallback names.
        table.extend(message(1, &record));
        table.extend(scalar(5, 6)); // Local membership refers to actual nominal index.
        let mut names = NAMES.to_vec();
        names[6] = &encoded;
        let input = input(
            class_with_table(&fields(&value(4, &[0x30, 6])), &table),
            &names,
        );
        let result = decode(&input, 3, Limits::default()).unwrap();
        let Classifier::Class(id) = &result.function.parameters[0].classifier else {
            panic!("nominal")
        };
        assert_eq!(id.package_name(), units("a.b"));
        assert_eq!(id.relative_class_name(), expected);
        assert!(id.local);
    }
    let mut table = message(1, &[8, 6]);
    table.extend(message(1, &[0x10, 0])); // Actual predefined index0 = kotlin/Any.
    let predefined = input(
        class_with_table(&fields(&value(4, &[0x30, 6])), &table),
        NAMES,
    );
    let result = decode(&predefined, 3, Limits::default()).unwrap();
    let Classifier::Class(id) = &result.function.parameters[0].classifier else {
        panic!("predefined nominal")
    };
    assert_eq!(id.package_name(), units("kotlin"));
    assert_eq!(id.relative_class_name(), units("Any"));
    let value_name = [0xd800, 0, 0xdc00];
    let function_name = [0xdc00, 0xd800];
    let mut nominal = units("a/b/`Quoted/Name`.Inner");
    nominal.extend([0xd800, 0]);
    let mut names = NAMES.to_vec();
    names[3] = &function_name;
    names[4] = &value_name;
    names[6] = &nominal;
    let input = input(class(&fields(&value(4, &[0x30, 6]))), &names);
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.function.name, function_name);
    assert_eq!(result.function.parameters[0].name, value_name);
    let Classifier::Class(id) = &result.function.parameters[0].classifier else {
        panic!("lossless nominal")
    };
    let mut expected = units("Quoted/Name.Inner");
    expected.extend([0xd800, 0]);
    assert_eq!(id.package_name(), units("a.b"));
    assert_eq!(id.relative_class_name(), expected);
    assert!(matches!(type_kind(&[0x30, 99]), ErrorKind::Name(_)));
}

#[test]
fn significant_shapes_wrong_wires_and_varargs_refuse_without_dropping_values() {
    for extra in [
        scalar(6, 6),
        scalar(9, 1),
        scalar(12, 0),
        scalar(4, 0),
        scalar(8, 0),
        scalar(11, 0),
        scalar(14, 0),
        message(2, &[]),
        message(5, &[]),
        message(10, &[]),
        message(13, &[]),
        message(1, &[]),
        message(3, &[]),
        message(6, &[]),
        message(7, &[]),
        scalar(1, 1),
    ] {
        let mut ty = leaf(0);
        ty.extend(extra);
        assert_eq!(type_kind(&ty), ErrorKind::UnsupportedType);
    }
    let mut conflicting = vec![0x30, 6];
    conflicting.extend(leaf(0));
    assert_eq!(type_kind(&conflicting), ErrorKind::UnsupportedType);
    assert_eq!(type_kind(&[]), ErrorKind::UnsupportedType);
    // Classifier6 nominal leaves have the same forbidden wrappers and significant wires.
    for number in 1..=14 {
        if matches!(number, 1 | 3 | 6) {
            continue;
        }
        let mut ty = vec![0x30, 6];
        ty.extend(message(number, &[]));
        assert_eq!(type_kind(&ty), ErrorKind::UnsupportedType);
    }
    for number in [4, 6] {
        for extra in [message(number, &[]), scalar(number, 0)] {
            let mut body = value_body(4, &leaf(0));
            body.extend(extra);
            let mut roles = value(4, &leaf(0)); // Earlier success must not escape.
            roles.extend(message(6, &body));
            assert_eq!(kind(&roles), ErrorKind::UnsupportedVararg);
        }
    }
    for number in [1, 2, 3, 5] {
        let mut body = value_body(4, &leaf(0));
        body.extend(if number == 3 {
            scalar(number, 0)
        } else {
            message(number, &[])
        });
        assert_eq!(kind(&message(6, &body)), ErrorKind::UnsupportedParameter);
    }
    assert!(matches!(kind(&message(6, &[])), ErrorKind::Parameter(_)));
    for number in [1, 2, 6, 9] {
        let mut roles = ordinary_roles();
        roles.extend(if number == 6 {
            scalar(number, 0)
        } else {
            message(number, &[])
        });
        assert_eq!(kind(&roles), ErrorKind::UnsupportedFunction);
    }
}

#[test]
fn complete_framing_ordinal_selection_and_opaque_unselected_roles() {
    let mut bytes = fields(&ordinary_roles());
    bytes.extend(message(9, &[0x80]));
    bytes.extend(message(10, &[0x80]));
    bytes.extend(message(30, &[0x80])); // Opaque Class table, not a table fallback.
    assert!(decode(&input_for(&bytes), 3, Limits::default()).is_ok());
    bytes.push(0x80);
    assert!(decode(&input_for(&bytes), 3, Limits::default()).is_err());
    assert_eq!(
        decode(&ordinary(), usize::MAX, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::MissingOrdinal
    );
    assert_eq!(
        decode(&ordinary(), 0, Limits::default()).unwrap_err().kind,
        ErrorKind::NotFunction
    );
    let mut bytes = declaration(0, 1);
    bytes.extend(declaration(1, 2));
    bytes.extend(scalar(9, 0));
    assert_eq!(
        decode(&input_for(&bytes), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::NotFunction
    );
    let mut roles = ordinary_roles();
    for number in [3, 5, 10, 11, 13, 30, 32, 100] {
        roles.extend(message(number, &[0x80]));
    }
    let mut body = value_body(4, &leaf(0));
    body.extend(message(100, &[0x80]));
    roles.extend(message(6, &body));
    let input = input_for(&fields(&roles));
    assert_eq!(
        decode(&input, 3, Limits::default())
            .unwrap()
            .function
            .parameters
            .len(),
        3
    );
    let input = input_for(&fields(&[]));
    assert!(
        decode(&input, 3, Limits::default())
            .unwrap()
            .function
            .parameters
            .is_empty()
    );
    let mut roles = ordinary_roles();
    roles.push(0x80);
    assert!(matches!(kind(&roles), ErrorKind::Function(_)));
    let mut roles = ordinary_roles();
    roles.extend(value(4, &[0x38]));
    assert!(matches!(
        kind(&roles),
        ErrorKind::Wire(wire::ErrorKind::Truncated)
    ));
    assert!(matches!(kind(&value(99, &leaf(0))), ErrorKind::Name(_)));
    let mut fields = declaration(0, 1);
    fields.extend(declaration(1, 2));
    fields.extend(message(9, &[0x10, 99]));
    assert!(matches!(
        decode(&input_for(&fields), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Name(_)
    ));
}

#[test]
fn aggregate_limits_are_exact_monotonic_and_not_reset_per_value() {
    let input = ordinary();
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        unreachable!()
    };
    let admission = bytes.len() + header.d2.iter().map(|s| s.len() * 2).sum::<usize>();
    type Set = fn(&mut Limits, usize);
    let dimensions: [(Set, usize); 9] = [
        (|l, n| l.bytes = n, admission),
        (|l, n| l.records = n, 4096),
        (|l, n| l.nodes = n, 128),
        (|l, n| l.work = n, 100_000),
        (|l, n| l.owned_bytes = n, 100_000),
        (|l, n| l.queries = n, 128),
        (|l, n| l.output_units = n, 1024),
        (|l, n| l.entries = n, 128),
        (|l, n| l.range_slots = n, 128),
    ];
    let mut exact = Limits {
        depth: 2,
        ..Default::default()
    };
    for (set, bound) in dimensions {
        let accepted = |n| {
            let mut limits = Limits::default();
            set(&mut limits, n);
            decode(&input, 3, limits).is_ok()
        };
        assert!(!accepted(0));
        assert!(accepted(bound));
        let (mut low, mut high) = (0, bound);
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if accepted(middle) {
                high = middle;
            } else {
                low = middle;
            }
        }
        assert!(!accepted(high - 1));
        assert!(accepted(high));
        set(&mut exact, high);
    }
    assert_eq!(
        decode(&input, 3, exact).unwrap(),
        decode(&input, 3, Limits::default()).unwrap()
    );
    let mut larger_roles = ordinary_roles();
    larger_roles.extend(value(4, &leaf(0)));
    let larger = input_for(&fields(&larger_roles));
    assert!(decode(&larger, 3, Limits::default()).is_ok());
    for (set, value) in [
        ((|l: &mut Limits, n| l.records = n) as Set, exact.records),
        ((|l: &mut Limits, n| l.nodes = n) as Set, exact.nodes),
        ((|l: &mut Limits, n| l.work = n) as Set, exact.work),
        (
            (|l: &mut Limits, n| l.owned_bytes = n) as Set,
            exact.owned_bytes,
        ),
        ((|l: &mut Limits, n| l.queries = n) as Set, exact.queries),
        (
            (|l: &mut Limits, n| l.output_units = n) as Set,
            exact.output_units,
        ),
    ] {
        let mut limits = Limits::default();
        set(&mut limits, value);
        assert!(decode(&larger, 3, limits).is_err());
    }
    assert!(
        decode(
            &input,
            3,
            Limits {
                depth: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    let output_units = NAMES[..6].iter().map(|s| s.len()).sum::<usize>();
    assert!(
        decode(
            &input,
            3,
            Limits {
                output_units,
                ..Default::default()
            }
        )
        .is_ok()
    );
    assert!(
        decode(
            &input,
            3,
            Limits {
                output_units: output_units - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    let huge = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        nodes: usize::MAX,
        depth: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
        entries: usize::MAX,
        range_slots: usize::MAX,
        queries: usize::MAX,
        output_units: usize::MAX,
    };
    assert_eq!(
        decode(&input, 3, huge).unwrap(),
        decode(&input, 3, Limits::default()).unwrap()
    );
    let mut malformed = vec![0x0a];
    malformed.extend(varint(u64::MAX)); // Overflow-sized Class length-delimited field.
    assert!(decode(&input_for(&malformed), 3, huge).is_err());
}

#[test]
fn unsupported_containers_and_hostile_truncations_never_panic_or_expose_partial_results() {
    for (kind, version) in [(2, vec![2, 3, 0]), (1, vec![2, 2, 0])] {
        let mut input = ordinary();
        let metadata::Outcome::Decoded { header, .. } = &mut input else {
            unreachable!()
        };
        header.k = kind;
        header.mv = version;
        assert_eq!(
            decode(&input, 3, Limits::default()).unwrap_err().kind,
            ErrorKind::Context(class_context::ErrorKind::UnsupportedContainer)
        );
    }
    let metadata::Outcome::Decoded { bytes, .. } = ordinary() else {
        unreachable!()
    };
    for end in 0..bytes.len() {
        assert!(decode(&input(bytes[..end].to_vec(), NAMES), 3, Limits::default()).is_err());
    }
    for ty in [
        vec![0x80],
        vec![0x38, 0x80],
        vec![0x0b],
        vec![0x30, 0x80],
        vec![0x12, 0xff],
    ] {
        assert!(decode(&input_for(&fields(&value(4, &ty))), 3, Limits::default()).is_err());
    }
}

// Authored cases below are NOT evidence of genuine compiler-emitted ID leaves.
fn own(id: i32, name: u8) -> Vec<u8> {
    let mut body = scalar(1, u64::from(id as u32));
    body.extend(scalar(2, u64::from(name)));
    message(4, &body)
}

#[test]
fn genuine_result_companion_name_binding_preserves_original_sibling_declarations() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Result$Companion.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { bytes, .. } = &input else {
        panic!("decoded Class")
    };
    for (ordinal, function_span, body_span, declaration_span, value_span, type_span, name) in [
        (4, 49..86, 51..86, 65..69, 71..77, 75..77, "success"),
        (5, 86..123, 88..123, 102..106, 108..114, 112..114, "failure"),
    ] {
        let result = decode(&input, ordinal, Limits::default()).unwrap();
        assert_eq!(result.owner.class_id.package_name(), units("kotlin"));
        assert_eq!(result.owner.class_id.relative_class_name(), units("Result.Companion"));
        assert!(!result.owner.class_id.local);
        assert_eq!(result.owner.body_span, 29..126);
        borrowed(result.owner.header.raw, bytes, 29..126);
        assert!(result.parameters.is_empty());
        let f = &result.function;
        assert_eq!(f.ordinal, ordinal);
        assert_eq!(f.span, function_span);
        assert_eq!(f.body_span, body_span);
        borrowed(f.header.raw, bytes, body_span);
        assert_eq!(f.name, units(name));
        assert_eq!(f.type_parameters.len(), 1);
        let local = &f.type_parameters[0];
        assert_eq!(local.body_span, declaration_span);
        borrowed(local.parameter.raw, bytes, declaration_span);
        assert_eq!(local.parameter.raw, [8, 1, 16, 6]);
        assert_eq!(local.parameter.id, 1);
        assert_eq!(local.parameter.name, 6);
        assert_eq!(local.name, units("T"));
        assert_eq!(local.parameter.reified, None);
        assert_eq!(local.parameter.variance, None);
        assert!(local.parameter.upper_bound.is_empty());
        assert!(local.parameter.upper_bound_id.is_empty());
        assert_eq!(f.parameters.len(), 1);
        let v = &f.parameters[0];
        assert_eq!(v.ordinal, 0);
        assert_eq!(v.body_span, value_span);
        assert_eq!(v.type_span, type_span);
        borrowed(v.raw.raw, bytes, value_span);
        borrowed(v.header.raw, bytes, type_span);
        assert_eq!(v.header.nullable, None);
        assert_eq!(v.header.flags, None);
        assert_eq!(v.raw.flags, None);
        assert_eq!(v.header.type_parameter, None);
        if ordinal == 4 {
            assert_eq!(v.name, units("value"));
            assert_eq!(v.header.raw, [0x48, 6]);
            assert_eq!(v.header.type_parameter_name, Some(6));
            assert_eq!(v.classifier, Classifier::FunctionParameter(0));
        } else {
            assert_eq!(v.name, units("exception"));
            assert_eq!(v.header.raw, [0x30, 11]);
            assert_eq!(v.header.type_parameter_name, None);
            let Classifier::Class(id) = &v.classifier else {
                panic!("nominal Throwable")
            };
            assert_eq!(id.package_name(), units("kotlin"));
            assert_eq!(id.relative_class_name(), units("Throwable"));
        }
    }
}

#[test]
fn authored_own_signed_ids_shadow_class_and_late_declarations_keep_original_ordinals() {
    let mut roles = value(4, &leaf(-7));
    roles.extend(value(4, &leaf(12345)));
    roles.extend(value(4, &leaf(1))); // Miss own scope, then exact Class ID.
    roles.extend(own(12345, 1));
    roles.extend(own(-7, 1)); // Equal own names do not invalidate ID lookup.
    let mut fields = declaration(-7, 2);
    fields.extend(declaration(1, 2));
    fields.extend(function(&roles));
    let input = input_for(&fields);
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.parameters[0].parameter.id, -7);
    assert_eq!(result.function.type_parameters[0].parameter.id, 12345);
    assert_eq!(result.function.type_parameters[1].parameter.id, -7);
    for (v, expected) in result.function.parameters.iter().zip([
        Classifier::FunctionParameter(1),
        Classifier::FunctionParameter(0),
        Classifier::ClassParameter(1),
    ]) {
        assert_eq!(v.classifier, expected);
    }
    let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
    for p in &result.function.type_parameters {
        borrowed(p.parameter.raw, bytes, p.body_span.clone());
    }
    let mut roles = own(7, 1);
    roles.extend(value(4, &leaf(7)));
    roles.extend(own(7, 2)); // Late duplicate fails, not a partial value result.
    assert_eq!(
        kind(&roles),
        ErrorKind::Context(class_context::ErrorKind::DuplicateParameterId)
    );
    let mut siblings = function(&own(7, 1));
    siblings.extend(function(&value(4, &leaf(7))));
    assert_eq!(
        decode(&input_for(&siblings), 2, Limits::default()).unwrap_err().kind,
        ErrorKind::UnknownParameterId
    );
    let mut siblings = function(&own(7, 1));
    siblings.extend(function(&value(4, &scalar(9, 1))));
    assert_eq!(
        decode(&input_for(&siblings), 2, Limits::default()).unwrap_err().kind,
        ErrorKind::UnknownParameterName
    );
}

#[test]
fn authored_name_binding_is_unique_immediate_own_resolved_content_only() {
    let mut roles = own(7, 1);
    roles.extend(value(4, &scalar(9, 2)));
    assert_eq!(kind(&roles), ErrorKind::UnknownParameterName); // Class B is not searched.
    roles.extend(own(8, 2)); // Late declaration now supplies B.
    let result_input = input_for(&fields(&roles));
    let result = decode(&result_input, 3, Limits::default()).unwrap();
    assert_eq!(result.function.parameters[0].classifier, Classifier::FunctionParameter(1));
    let mut roles = own(7, 1);
    roles.extend(own(8, 1));
    roles.extend(value(4, &scalar(9, 1)));
    assert_eq!(kind(&roles), ErrorKind::AmbiguousParameterName);
    let mut roles = own(7, 1);
    roles.extend(value(4, &scalar(9, 99)));
    assert!(matches!(kind(&roles), ErrorKind::Name(_)));
    // Distinct raw indices, transformed by the real resolver into identical content.
    for (record, encoded, resolved) in [
        (vec![0x18, 1], units("a/b/T$Inner"), units("a/b/T.Inner")),
        (vec![0x18, 2], units("La/b/T$Inner;"), units("a/b/T.Inner")),
        (vec![0x10, 0], vec![], units("kotlin/Any")),
    ] {
        let mut names = NAMES.to_vec();
        names[1] = &resolved;
        names[6] = &encoded;
        let mut table = message(1, &[8, 6]);
        table.extend(message(1, &record));
        let mut roles = own(123, 1);
        roles.extend(value(4, &scalar(9, 6)));
        let original = input(class_with_table(&function(&roles), &table), &names);
        let result = decode(&original, 1, Limits::default()).unwrap();
        assert_eq!(result.function.type_parameters[0].name, resolved);
        assert_eq!(result.function.parameters[0].classifier, Classifier::FunctionParameter(0));
        roles.extend(own(124, 6)); // Different index, equal resolved name is ambiguous.
        let input = input(class_with_table(&function(&roles), &table), &names);
        assert_eq!(decode(&input, 1, Limits::default()).unwrap_err().kind, ErrorKind::AmbiguousParameterName);
    }
    let surrogate = [0xd800, 0, 0xdc00];
    let different = [0xfffd, 0, 0xfffd];
    let mut names = NAMES.to_vec();
    names[1] = &surrogate;
    names[2] = &different;
    names[6] = &surrogate;
    let mut roles = own(7, 1);
    roles.extend(own(8, 2));
    roles.extend(value(4, &scalar(9, 6)));
    let input = input(class(&function(&roles)), &names);
    let result = decode(&input, 1, Limits::default()).unwrap();
    assert_eq!(result.function.type_parameters[0].name, surrogate);
    assert_eq!(result.function.parameters[0].classifier, Classifier::FunctionParameter(0));
}

#[test]
fn authored_own_options_opaque_bounds_and_nullable_presence_are_retained() {
    for nullable in [None, Some(false), Some(true)] {
        let mut declaration = scalar(1, u64::from(u32::MAX));
        declaration.extend([16, 1, 24, 1, 32, 0]);
        declaration.extend(message(5, &[0x80])); // Bound remains opaque, not decoded Type.
        declaration.extend(scalar(6, u64::from(u32::MAX)));
        declaration.extend(message(6, &[7, 8]));
        let mut ty = scalar(9, 1);
        if let Some(marking) = nullable {
            ty.extend(scalar(3, u64::from(marking)));
        }
        let mut roles = value(4, &ty);
        roles.extend(message(4, &declaration));
        let input = input_for(&fields(&roles));
        let result = decode(&input, 3, Limits::default()).unwrap();
        let p = &result.function.type_parameters[0];
        assert_eq!(p.parameter.id, -1);
        assert_eq!(p.parameter.reified, Some(true));
        assert_eq!(p.parameter.variance, Some(metadata::type_parameter::Variance::In));
        assert_eq!(p.parameter.upper_bound_id, [-1, 7, 8]);
        assert_eq!(p.parameter.upper_bound.len(), 1);
        let bound = &p.parameter.upper_bound[0];
        borrowed(bound.raw, p.parameter.raw, bound.span.clone());
        assert_eq!(bound.raw, [0x80]);
        assert_eq!(result.function.parameters[0].header.nullable, nullable);
        assert_eq!(result.function.parameters[0].classifier, Classifier::FunctionParameter(0));
    }
}

#[test]
fn authored_late_own_failures_are_atomic_rebased_once_and_name_shapes_stay_bare() {
    for body in [
        vec![8, 7], // Missing required name.
        vec![16, 1], // Missing required ID.
        vec![8, 7, 16, 1, 50, 1, 0x80], // Truncated packed bound ID.
        vec![8, 7, 16, 1, 10, 0], // Significant ID wrong wire, despite earlier valid ID.
        vec![8, 7, 16, 1, 18, 0], // Significant name wrong wire.
    ] {
        let mut roles = value(4, &leaf(0));
        roles.extend(message(4, &body));
        let input = input_for(&fields(&roles));
        let error = decode(&input, 3, Limits::default()).unwrap_err();
        let metadata::Outcome::Decoded { bytes, .. } = &input else { unreachable!() };
        let start = bytes.len() - body.len();
        assert!(error.offset >= start && error.offset <= bytes.len());
        assert!(matches!(error.kind, ErrorKind::Context(_)));
    }
    for extra in [
        scalar(6, 6), scalar(7, 7), scalar(12, 0), scalar(4, 0),
        scalar(8, 0), scalar(11, 0), scalar(14, 0), scalar(1, 1),
        message(2, &[]), message(5, &[]), message(10, &[]), message(13, &[]),
        message(1, &[]), message(3, &[]), message(6, &[]), message(7, &[]),
        message(9, &[]),
    ] {
        let mut ty = scalar(9, 1);
        ty.extend(extra);
        let mut roles = own(7, 1);
        roles.extend(value(4, &ty));
        assert_eq!(kind(&roles), ErrorKind::UnsupportedType);
    }
    // Last-recognized NAME scalar, full signed-width conversion, inline wins table ID.
    let mut ty = scalar(9, 2);
    ty.extend(scalar(9, (1u64 << 32) + 1));
    let mut v = value_body(4, &ty);
    v.extend(scalar(5, u64::from(u32::MAX)));
    let mut roles = own(-1, 1);
    roles.extend(message(6, &v));
    let input = input_for(&fields(&roles));
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.function.parameters[0].header.type_parameter_name, Some(1));
    assert_eq!(result.function.parameters[0].classifier, Classifier::FunctionParameter(0));
}

fn minimum_limit(input: &metadata::Outcome<'_>, set: fn(&mut Limits, usize)) -> usize {
    let accepted = |n| {
        let mut limits = Limits::default();
        set(&mut limits, n);
        decode(input, 1, limits).is_ok()
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
    assert!(!accepted(high - 1));
    assert!(accepted(high));
    high
}

#[test]
fn authored_own_bounds_values_and_temporary_name_queries_share_exact_budget() {
    let mut declaration = vec![8, 7, 16, 1];
    declaration.extend(message(5, &[0x80]));
    declaration.extend(message(6, &[0, 7]));
    let mut roles = value(4, &scalar(9, 1));
    roles.extend(message(4, &declaration));
    roles.extend(own(8, 2));
    roles.extend(value(5, &leaf(8)));
    let original = input_for(&function(&roles));
    type Set = fn(&mut Limits, usize);
    let dimensions: [Set; 9] = [
        |l, n| l.bytes = n, |l, n| l.records = n, |l, n| l.nodes = n,
        |l, n| l.work = n, |l, n| l.owned_bytes = n, |l, n| l.queries = n,
        |l, n| l.output_units = n, |l, n| l.entries = n, |l, n| l.range_slots = n,
    ];
    let mut exact = Limits { depth: 2, ..Default::default() };
    for set in dimensions {
        set(&mut exact, minimum_limit(&original, set));
    }
    assert_eq!(
        decode(&original, 1, exact).unwrap(),
        decode(&original, 1, Limits::default()).unwrap()
    );
    assert!(decode(&original, 1, Limits { depth: 1, ..Default::default() }).is_err());
    assert_eq!(
        decode(&original, 1, Limits { depth: usize::MAX, ..Default::default() }).unwrap(),
        decode(&original, 1, exact).unwrap()
    );
    let mut extra_value = roles.clone();
    extra_value.extend(value(4, &scalar(9, 1))); // Temporary query not refunded.
    let mut extra_local = roles.clone();
    extra_local.extend(own(9, 6));
    let mut extra_bound = declaration.clone();
    extra_bound.extend(message(5, &[0x80]));
    extra_bound.extend(message(6, &[0, 7]));
    let mut bound_roles = value(4, &scalar(9, 1));
    bound_roles.extend(message(4, &extra_bound));
    bound_roles.extend(own(8, 2));
    bound_roles.extend(value(5, &leaf(8)));
    for larger_roles in [extra_value, extra_local, bound_roles] {
        let larger = input_for(&function(&larger_roles));
        assert!(decode(&larger, 1, Limits::default()).is_ok());
        for (set, n) in [
            ((|l: &mut Limits, n| l.records = n) as Set, exact.records),
            ((|l: &mut Limits, n| l.work = n) as Set, exact.work),
            ((|l: &mut Limits, n| l.owned_bytes = n) as Set, exact.owned_bytes),
        ] {
            let mut limits = Limits::default();
            set(&mut limits, n);
            assert!(decode(&larger, 1, limits).is_err());
        }
    }
    // Owner string+local membership, Function, two locals, two values, temporary NAME.
    assert_eq!(exact.queries, 8);
    let produced = NAMES[0].len() + NAMES[3].len() + NAMES[1].len() * 2
        + NAMES[2].len() + NAMES[4].len() + NAMES[5].len();
    assert_eq!(exact.output_units, produced);
}

#[test]
fn authored_long_equal_prefix_name_comparisons_consume_work_per_inspected_unit() {
    let target = vec![65u16; 256];
    let mut early = target.clone();
    early[0] = 66;
    let mut late = target.clone();
    late[255] = 66;
    let mut roles = own(7, 1);
    roles.extend(own(8, 2));
    roles.extend(value(4, &scalar(9, 6)));
    let bytes = class(&function(&roles));
    let mut names = NAMES.to_vec();
    names[1] = &early;
    names[2] = &target;
    names[6] = &target;
    let early_input = input(bytes.clone(), &names);
    let early_work = minimum_limit(&early_input, |l, n| l.work = n);
    names[1] = &late;
    let late_input = input(bytes, &names);
    let late_work = minimum_limit(&late_input, |l, n| l.work = n);
    assert_eq!(late_work - early_work, 255);
    assert!(decode(&late_input, 1, Limits { work: early_work, ..Default::default() }).is_err());
    assert_eq!(
        decode(&late_input, 1, Limits::default()).unwrap().function.parameters[0].classifier,
        Classifier::FunctionParameter(1)
    );
}
