use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, class_context,
        resolved_class_function_return::{ErrorKind, Limits, decode},
        type_argument::Projection,
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
    &[112, 107, 103, 47, 66, 111, 120], // same ClassId through a DIFFERENT resolver index
    &[112, 107, 103, 47, 79, 116, 104, 101, 114],
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
fn parameter(id: i32, name: u8) -> Vec<u8> {
    let mut body = scalar(1, u64::from(id as u32));
    body.extend([0x10, name]);
    message(5, &body)
}
fn leaf(id: i32) -> Vec<u8> {
    scalar(7, u64::from(id as u32))
}
fn argument(ty: &[u8]) -> Vec<u8> {
    message(2, ty)
}
fn nominal(arguments: &[Vec<u8>], index: u8) -> Vec<u8> {
    let mut bytes = vec![0x30, index];
    for arg in arguments {
        bytes.extend(message(2, arg));
    }
    bytes
}
fn ordinary_return() -> Vec<u8> {
    nominal(&[argument(&leaf(0)), argument(&leaf(1))], 0)
}
fn function(roles: &[u8]) -> Vec<u8> {
    let mut body = vec![0x10, 3];
    body.extend(roles);
    message(9, &body)
}
fn class(fields: &[u8]) -> Vec<u8> {
    let mut bytes = vec![4, 0x0a, 2, 8, 6, 0x18, 0]; // Six actual fallback name records.
    bytes.extend(fields);
    bytes
}
fn fields(roles: &[u8]) -> Vec<u8> {
    let mut bytes = parameter(0, 1);
    bytes.extend(parameter(1, 2));
    bytes.extend(function(roles));
    bytes
}
fn ordinary() -> metadata::Outcome<'static> {
    input(class(&fields(&message(3, &ordinary_return()))), NAMES)
}
fn kind(roles: &[u8]) -> ErrorKind {
    decode(&input(class(&fields(roles)), NAMES), 3, Limits::default())
        .unwrap_err()
        .kind
}
fn return_kind(ty: &[u8]) -> ErrorKind {
    kind(&message(3, ty))
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn genuine_pair_copy_ordinal_nine_binds_original_a_b_borrowed_declarations() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { bytes, .. } = &input else {
        panic!("decoded Class")
    };
    let result = decode(&input, 9, Limits::default()).unwrap();
    assert_eq!(result.owner.body_span, 47..285);
    assert_eq!(result.owner.header.raw.as_ptr(), bytes[47..285].as_ptr());
    assert_eq!(result.owner.class_id.package_name(), units("kotlin"));
    assert_eq!(result.owner.class_id.relative_class_name(), units("Pair"));
    assert!(!result.owner.class_id.local);
    assert_eq!(result.parameters.len(), 2);
    let selected = &result.function;
    assert_eq!(selected.ordinal, 9);
    assert_eq!(selected.span, 149..198);
    assert_eq!(selected.body_span, 151..198);
    assert_eq!(selected.header.raw.as_ptr(), bytes[151..198].as_ptr());
    assert_eq!(selected.header.name, 17);
    assert_eq!(selected.header.flags, Some(65734));
    assert_eq!(selected.name, units("copy"));
    assert_eq!(selected.return_type.body_span, 155..169);
    assert_eq!(
        selected.return_type.header.raw.as_ptr(),
        bytes[155..169].as_ptr()
    );
    assert_eq!(selected.return_type.header.class_name, Some(0));
    assert_eq!(selected.return_type.header.nullable, None);
    assert_eq!(selected.return_type.arguments.len(), 2);
    for (ordinal, (name, declaration_span, argument_span, leaf_span)) in [
        ("A", 54..60, 157..161, 159..161),
        ("B", 62..68, 163..167, 165..167),
    ]
    .into_iter()
    .enumerate()
    {
        let argument = &selected.return_type.arguments[ordinal];
        assert_eq!(argument.body_span, argument_span);
        assert_eq!(argument.raw.raw.as_ptr(), bytes[argument_span].as_ptr());
        assert_eq!(argument.raw.projection, None); // Effective invariant, NOT declaration OUT.
        assert_eq!(argument.type_span, leaf_span);
        assert_eq!(argument.header.raw.as_ptr(), bytes[leaf_span].as_ptr());
        assert_eq!(argument.header.nullable, None);
        assert_eq!(argument.header.type_parameter, Some(ordinal as i32));
        assert_eq!(argument.parameter_ordinal, ordinal);
        let declaration = &result.parameters[argument.parameter_ordinal];
        assert!(std::ptr::eq(declaration, &result.parameters[ordinal]));
        assert_eq!(declaration.name, units(name));
        assert_eq!(declaration.body_span, declaration_span);
        assert_eq!(
            declaration.parameter.raw.as_ptr(),
            bytes[declaration_span].as_ptr()
        );
        assert_eq!(
            declaration.parameter.variance,
            Some(metadata::type_parameter::Variance::Out)
        );
    }
    // Function-list ordinal3 is NOT the all-wire-field copy selection.
    assert_eq!(
        decode(&input, 3, Limits::default()).unwrap_err().kind,
        ErrorKind::NotFunction
    );
    for ordinal in [6, 7, 8, 10, 11] {
        assert!(decode(&input, ordinal, Limits::default()).is_err());
    }
}

#[test]
fn signed_reordered_ids_bind_ordinals_not_names_or_positions() {
    let ty = nominal(&[argument(&leaf(-7)), argument(&leaf(12345))], 0);
    let mut bytes = function(&message(3, &ty)); // Declarations may follow the function.
    bytes.extend(parameter(12345, 1));
    bytes.extend(parameter(-7, 1));
    let input = input(class(&bytes), NAMES);
    let result = decode(&input, 1, Limits::default()).unwrap();
    assert_eq!(result.parameters[0].name, result.parameters[1].name);
    assert_eq!(
        result.function.return_type.arguments[0].parameter_ordinal,
        1
    );
    assert_eq!(
        result.function.return_type.arguments[1].parameter_ordinal,
        0
    );
    assert_eq!(result.parameters[0].parameter.id, 12345);
    assert_eq!(result.parameters[1].parameter.id, -7);
    let mut duplicate = parameter(0, 1);
    duplicate.extend(parameter(0, 2));
    duplicate.extend(function(&message(3, &ordinary_return())));
    assert_eq!(
        decode(&input_for(&duplicate), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Context(class_context::ErrorKind::DuplicateParameterId)
    );
    assert_eq!(
        return_kind(&nominal(&[argument(&leaf(99)), argument(&leaf(1))], 0)),
        ErrorKind::UnknownParameterId
    );
}
fn input_for(fields: &[u8]) -> metadata::Outcome<'static> {
    input(class(fields), NAMES)
}

#[test]
fn selected_ordinal_framing_and_opaque_other_members_are_atomic() {
    let mut bytes = fields(&message(3, &ordinary_return()));
    // Invalid Function/Property interiors are intentionally not interpreted by this query.
    bytes.extend(message(9, &[0x80]));
    bytes.extend(message(10, &[0x80]));
    let input = input_for(&bytes);
    assert!(decode(&input, 3, Limits::default()).is_ok());
    assert_eq!(
        decode(&input, 0, Limits::default()).unwrap_err().kind,
        ErrorKind::NotFunction
    );
    assert_eq!(
        decode(&input, usize::MAX, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::MissingOrdinal
    );
    bytes.push(0x80); // Malformed LATE Class framing cannot be hidden behind selected success.
    assert!(matches!(
        decode(&input_for(&bytes), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Context(class_context::ErrorKind::Wire(_))
    ));
    let mut wrong = parameter(0, 1);
    wrong.extend(parameter(1, 2));
    wrong.extend(scalar(9, 0));
    assert_eq!(
        decode(&input_for(&wrong), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::NotFunction
    );
    assert!(matches!(kind(&[0x1a, 0x80]), ErrorKind::Function(_)));
    let mut roles = message(3, &ordinary_return());
    for number in [5, 6, 10, 11, 30, 32, 100] {
        roles.extend(message(number, &[0x80]));
    }
    assert!(decode(&input_for(&fields(&roles)), 3, Limits::default()).is_ok());
    for number in [1, 2, 3, 7, 9] {
        let mut roles = message(3, &ordinary_return());
        roles.extend(message(number, &[]));
        let expected = if number == 3 {
            ErrorKind::DuplicateInlineRole
        } else {
            ErrorKind::UnsupportedFunction
        };
        assert_eq!(kind(&roles), expected);
    }
}

#[test]
fn empty_own_scope_and_id_only_classifier_cannot_bind_class_by_text() {
    for own in [vec![], vec![8, 0, 16, 1], vec![0x80]] {
        let mut roles = message(3, &ordinary_return());
        roles.extend(message(4, &own));
        assert_eq!(kind(&roles), ErrorKind::UnsupportedTypeParameters);
    }
    for leaf_type in [
        vec![0x48, 1],
        vec![0x38, 0, 0x48, 1],
        vec![0x38, 0, 0x30, 0],
    ] {
        assert_eq!(
            return_kind(&nominal(&[argument(&leaf_type), argument(&leaf(1))], 0)),
            ErrorKind::UnsupportedType
        );
    }
}

#[test]
fn inline_roles_win_over_ids_but_duplicate_messages_never_merge() {
    let mut first = argument(&leaf(0));
    first.extend(scalar(3, u64::MAX));
    let ty = nominal(&[first, argument(&leaf(1))], 0);
    let mut roles = scalar(7, u64::MAX);
    roles.extend(message(3, &ty));
    roles.extend(scalar(7, 0));
    let input = input_for(&fields(&roles));
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(
        result.function.return_type.arguments[0].raw.type_id,
        Some(-1)
    );
    let mut duplicate = message(3, &ordinary_return());
    duplicate.extend(message(3, &[]));
    assert_eq!(kind(&duplicate), ErrorKind::DuplicateInlineRole);
    assert_eq!(kind(&[]), ErrorKind::MissingRole);
    assert_eq!(kind(&scalar(7, 0)), ErrorKind::UnsupportedTableRole);
    let mut duplicate = argument(&leaf(0));
    duplicate.extend(argument(&[]));
    assert_eq!(
        return_kind(&nominal(&[duplicate, argument(&leaf(1))], 0)),
        ErrorKind::DuplicateInlineRole
    );
    for (arg, expected) in [
        (vec![], ErrorKind::MissingRole),
        (scalar(3, 0), ErrorKind::UnsupportedTableRole),
    ] {
        assert_eq!(
            return_kind(&nominal(&[arg, argument(&leaf(1))], 0)),
            expected
        );
    }
}

#[test]
fn projections_markings_and_effective_scalar_narrowing_are_retained() {
    let mut first_leaf = leaf(0);
    first_leaf.extend(scalar(3, 1));
    first_leaf.extend(scalar(3, 0));
    first_leaf.extend(scalar(1, 1));
    first_leaf.extend(scalar(1, 1u64 << 32)); // Effective int32 zero.
    let mut first = scalar(1, 1);
    first.extend(scalar(1, (1u64 << 32) + 2));
    first.extend(argument(&first_leaf));
    let mut second_leaf = leaf(99);
    second_leaf.extend(scalar(7, (1u64 << 32) + 1));
    second_leaf.extend(scalar(3, 2));
    let mut ty = nominal(&[first, argument(&second_leaf)], 0);
    ty.extend(scalar(3, 0));
    ty.extend(scalar(3, 1));
    ty.extend(scalar(1, 1u64 << 32));
    let input = input_for(&fields(&message(3, &ty)));
    let result = decode(&input, 3, Limits::default()).unwrap();
    assert_eq!(result.function.return_type.header.nullable, Some(true));
    let args = &result.function.return_type.arguments;
    assert_eq!(args[0].raw.projection, Some(Projection::Inv));
    assert_eq!(args[0].header.nullable, Some(false));
    assert_eq!(args[0].header.flags, Some(0));
    assert_eq!(args[1].header.nullable, Some(true));
    assert_eq!(args[1].header.type_parameter, Some(1));
    for projection in [0, 1, 3, 4, u64::MAX] {
        let mut arg = scalar(1, projection);
        arg.extend(argument(&leaf(0)));
        assert_eq!(
            return_kind(&nominal(&[arg, argument(&leaf(1))], 0)),
            ErrorKind::UnsupportedProjection
        );
    }
    let mut unknown_then_inv = scalar(1, 4);
    unknown_then_inv.extend(scalar(1, 2));
    unknown_then_inv.extend(argument(&leaf(0)));
    assert_eq!(
        return_kind(&nominal(&[unknown_then_inv, argument(&leaf(1))], 0)),
        ErrorKind::UnsupportedProjection
    );
}

#[test]
fn self_target_compares_real_classids_and_argument_count_not_raw_index() {
    let ty = nominal(&[argument(&leaf(0)), argument(&leaf(1))], 4);
    assert!(decode(&input_for(&fields(&message(3, &ty))), 3, Limits::default()).is_ok());
    assert_eq!(
        return_kind(&nominal(&[argument(&leaf(0)), argument(&leaf(1))], 5)),
        ErrorKind::WrongReturnClass
    );
    for args in [
        vec![],
        vec![argument(&leaf(0))],
        vec![argument(&leaf(0)), argument(&leaf(1)), argument(&leaf(0))],
    ] {
        assert_eq!(return_kind(&nominal(&args, 0)), ErrorKind::ArgumentCount);
    }
    // Same text but different local membership is not the same semantic ClassId.
    let mut bytes = vec![6, 0x0a, 2, 8, 6, 0x28, 4, 0x18, 0];
    bytes.extend(fields(&message(3, &ty)));
    assert_eq!(
        decode(&input(bytes, NAMES), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::WrongReturnClass
    );
    let mut narrow = ordinary_return();
    narrow.extend(scalar(6, 1u64 << 32));
    assert!(
        decode(
            &input_for(&fields(&message(3, &narrow))),
            3,
            Limits::default()
        )
        .is_ok()
    );
}

#[test]
fn significant_type_forms_wrong_wires_and_nonzero_flags_refuse() {
    for extra in [
        scalar(7, 0),
        scalar(9, 1),
        scalar(12, 0),
        scalar(4, 0),
        scalar(8, 0),
        scalar(11, 0),
        scalar(14, 0),
        message(5, &[]),
        message(10, &[]),
        message(13, &[]),
        message(1, &[]),
        message(3, &[]),
        message(6, &[]),
        scalar(2, 0),
        scalar(1, 1),
    ] {
        let mut ty = ordinary_return();
        ty.extend(extra);
        assert_eq!(return_kind(&ty), ErrorKind::UnsupportedType);
    }
    for extra in [
        scalar(6, 0),
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
        message(7, &[]),
        message(3, &[]),
        scalar(1, 1),
    ] {
        let mut ty = leaf(0);
        ty.extend(extra);
        assert_eq!(
            return_kind(&nominal(&[argument(&ty), argument(&leaf(1))], 0)),
            ErrorKind::UnsupportedType
        );
    }
    for (wrong, expected) in [
        (message(1, &[]), ErrorKind::UnsupportedProjection),
        (scalar(2, 0), ErrorKind::UnsupportedType),
        (message(3, &[]), ErrorKind::UnsupportedType),
    ] {
        let mut arg = argument(&leaf(0));
        arg.extend(wrong);
        assert_eq!(
            return_kind(&nominal(&[arg, argument(&leaf(1))], 0)),
            expected
        );
    }
    let mut ty = ordinary_return();
    ty.extend(message(100, &[0x80]));
    assert!(decode(&input_for(&fields(&message(3, &ty))), 3, Limits::default()).is_ok());
    let missing = nominal(&[argument(&[]), argument(&leaf(1))], 0);
    assert_eq!(return_kind(&missing), ErrorKind::UnsupportedType);
}

#[test]
fn every_composite_resource_has_an_exact_monotonic_boundary() {
    let input = ordinary();
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        unreachable!()
    };
    let admission = bytes.len() + header.d2.iter().map(|s| s.len() * 2).sum::<usize>();
    assert!(
        decode(
            &input,
            3,
            Limits {
                bytes: admission,
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
                bytes: admission - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    type Set = fn(&mut Limits, usize);
    let mut exact = Limits {
        bytes: admission,
        depth: 2,
        ..Default::default()
    };
    for (set, high) in [
        ((|l: &mut Limits, n| l.records = n) as Set, 4096),
        ((|l: &mut Limits, n| l.nodes = n) as Set, 128),
        ((|l: &mut Limits, n| l.work = n) as Set, 100_000),
        ((|l: &mut Limits, n| l.owned_bytes = n) as Set, 100_000),
        ((|l: &mut Limits, n| l.queries = n) as Set, 128),
        ((|l: &mut Limits, n| l.output_units = n) as Set, 1024),
        ((|l: &mut Limits, n| l.entries = n) as Set, 128),
        ((|l: &mut Limits, n| l.range_slots = n) as Set, 128),
    ] {
        let accepted = |n| {
            let mut limits = Limits::default();
            set(&mut limits, n);
            decode(&input, 3, limits).is_ok()
        };
        assert!(!accepted(0));
        assert!(accepted(high));
        let (mut low, mut high) = (0, high);
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if accepted(middle) {
                high = middle
            } else {
                low = middle
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
    let mut larger = parameter(0, 1);
    larger.extend(parameter(1, 2));
    larger.extend(parameter(2, 1));
    larger.extend(function(&message(
        3,
        &nominal(
            &[argument(&leaf(0)), argument(&leaf(1)), argument(&leaf(2))],
            0,
        ),
    )));
    let larger = input_for(&larger);
    assert!(decode(&larger, 4, Limits::default()).is_ok());
    for (set, n) in [
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
        set(&mut limits, n);
        assert!(decode(&larger, 4, limits).is_err());
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
    assert!(
        decode(
            &input,
            3,
            Limits {
                depth: 2,
                ..Default::default()
            }
        )
        .is_ok()
    );
    // Common same-index self return pays only owner ClassId + A + B + function names.
    let expected_units = NAMES[0].len() + NAMES[1].len() + NAMES[2].len() + NAMES[3].len();
    assert!(
        decode(
            &input,
            3,
            Limits {
                output_units: expected_units,
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
                output_units: expected_units - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    let different = input_for(&fields(&message(
        3,
        &nominal(&[argument(&leaf(0)), argument(&leaf(1))], 4),
    )));
    assert!(
        decode(
            &different,
            3,
            Limits {
                output_units: expected_units,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        decode(
            &different,
            3,
            Limits {
                output_units: expected_units + NAMES[4].len(),
                ..Default::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn unsupported_container_and_name_resolution_fail_without_a_partial_result() {
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
    let mut body = vec![0x10, 99];
    body.extend(message(3, &ordinary_return()));
    let mut fields = parameter(0, 1);
    fields.extend(parameter(1, 2));
    fields.extend(message(9, &body));
    assert!(matches!(
        decode(&input_for(&fields), 3, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Name(_)
    ));
    // An invalid return class index must use the resolver, never raw-index fallback.
    assert!(matches!(
        return_kind(&nominal(&[argument(&leaf(0)), argument(&leaf(1))], 99)),
        ErrorKind::Name(_)
    ));
    let metadata::Outcome::Decoded { bytes, .. } = ordinary() else {
        unreachable!()
    };
    for end in 0..bytes.len() {
        assert!(decode(&input(bytes[..end].to_vec(), NAMES), 3, Limits::default()).is_err());
    }
    assert!(matches!(
        return_kind(&[0x30, 0, 0x12, 0x80]),
        ErrorKind::Wire(wire::ErrorKind::Truncated)
    ));
}
