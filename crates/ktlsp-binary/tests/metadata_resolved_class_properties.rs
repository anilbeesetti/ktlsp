use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self,
        resolved_class_properties::{ErrorKind, Limits, decode},
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
    &[102, 105, 114, 115, 116],
    &[115, 101, 99, 111, 110, 100],
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
fn property(name: u8, ty: &[u8]) -> Vec<u8> {
    let mut body = vec![0x10, name];
    body.extend(message(3, ty));
    message(10, &body)
}
fn id_type(id: i32) -> Vec<u8> {
    scalar(7, u64::from(id as u32))
}
fn class(fields: &[u8]) -> Vec<u8> {
    let mut bytes = vec![4, 0x0a, 2, 8, 5, 0x18, 0]; // Five real fallback records.
    bytes.extend(fields);
    bytes
}
fn ordinary_fields() -> Vec<u8> {
    let mut fields = parameter(0, 1);
    fields.extend(parameter(1, 2));
    fields.extend(property(3, &id_type(0)));
    fields.extend(property(4, &id_type(1)));
    fields
}
fn kind(fields: &[u8]) -> ErrorKind {
    decode(&input(class(fields), NAMES), Limits::default())
        .unwrap_err()
        .kind
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn genuine_pair_binds_original_declarations_through_native_public_api() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        panic!("supported Class metadata")
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let result = decode(&input, Limits::default()).unwrap();
    assert_eq!(result.owner.body_span, 47..285);
    assert_eq!(result.owner.header.raw.as_ptr(), bytes[47..285].as_ptr());
    assert_eq!(result.owner.class_id.package_name(), units("kotlin"));
    assert_eq!(result.owner.class_id.relative_class_name(), units("Pair"));
    assert!(!result.owner.class_id.local);
    assert_eq!(result.parameters.len(), 2);
    assert_eq!(result.properties.len(), 2);
    for (ordinal, (parameter, name, span)) in [
        (&result.parameters[0], "A", 54..60),
        (&result.parameters[1], "B", 62..68),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(parameter.name, units(name));
        assert_eq!(parameter.parameter.id, ordinal as i32);
        assert_eq!(parameter.body_span, span);
        assert_eq!(parameter.parameter.raw.as_ptr(), bytes[span].as_ptr());
    }
    let raw = wire::decode(bytes, Default::default()).unwrap();
    let selected: Vec<_> = raw
        .payload
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.number == 10)
        .collect();
    for (ordinal, (property, name, span, type_span)) in [
        (&result.properties[0], "first", 232..257, 238..240),
        (&result.properties[1], "second", 257..282, 263..265),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(property.name, units(name));
        assert_eq!(property.ordinal, selected[ordinal].0);
        assert_eq!(property.span, span);
        assert_eq!(property.body_span, span.start + 2..span.end);
        assert_eq!(
            property.header.raw.as_ptr(),
            bytes[property.body_span.clone()].as_ptr()
        );
        let reference = &property.return_type;
        assert_eq!(reference.body_span, type_span);
        assert_eq!(reference.header.raw.as_ptr(), bytes[type_span].as_ptr());
        assert_eq!(reference.header.nullable, None);
        let declaration = &result.parameters[reference.parameter_ordinal];
        assert!(std::ptr::eq(declaration, &result.parameters[ordinal]));
        assert_eq!(
            declaration.parameter.raw.as_ptr(),
            bytes[declaration.body_span.clone()].as_ptr()
        );
    }
}

#[test]
fn signed_noncontiguous_reordered_ids_and_equal_names_preserve_identity() {
    let mut fields = property(3, &id_type(-7)); // Properties precede declarations.
    fields.extend(property(4, &id_type(12345)));
    fields.extend(parameter(12345, 1));
    fields.extend(parameter(-7, 1)); // Equal names are not equal declarations.
    let input = input(class(&fields), NAMES);
    let result = decode(&input, Limits::default()).unwrap();
    assert_eq!(result.parameters[0].name, result.parameters[1].name);
    assert_eq!(result.parameters[0].parameter.id, 12345);
    assert_eq!(result.parameters[1].parameter.id, -7);
    assert_eq!(result.properties[0].return_type.parameter_ordinal, 1);
    assert_eq!(result.properties[1].return_type.parameter_ordinal, 0);
    assert_ne!(
        result.parameters[0].body_span,
        result.parameters[1].body_span
    );
    // Both ten-byte signed and five-byte uint32 encodings decode the same int32 ID.
    let mut fields = parameter(-7, 1);
    fields.extend(property(3, &scalar(7, (-7i64) as u64)));
    assert_eq!(
        decode(&input_for_fields(&fields), Limits::default())
            .unwrap()
            .properties[0]
            .return_type
            .parameter_ordinal,
        0
    );
}
fn input_for_fields(fields: &[u8]) -> metadata::Outcome<'static> {
    input(class(fields), NAMES)
}

#[test]
fn lexical_id_scope_refuses_duplicates_unknown_ids_and_parent_name_lookup() {
    let mut duplicate = parameter(9, 1);
    duplicate.extend(parameter(9, 2));
    duplicate.extend(property(3, &id_type(9)));
    assert_eq!(kind(&duplicate), ErrorKind::DuplicateParameterId);
    let mut unknown = parameter(9, 1);
    unknown.extend(property(3, &id_type(0)));
    assert_eq!(kind(&unknown), ErrorKind::UnknownParameterId);
    let mut name = parameter(0, 1);
    name.extend(property(3, &[0x48, 1])); // Own property scope is empty, Class A is irrelevant.
    assert_eq!(kind(&name), ErrorKind::UnsupportedType);
    let mut own = parameter(0, 1);
    let mut body = vec![0x10, 3];
    body.extend(message(4, &[8, 0, 0x10, 1]));
    body.extend(message(3, &id_type(0)));
    own.extend(message(10, &body));
    assert_eq!(kind(&own), ErrorKind::UnsupportedTypeParameters);
}

#[test]
fn owner_and_names_share_lossless_resolver_and_neutral_class_id_semantics() {
    let mut owner = units("a/b/`Outer/Quoted`.Inner");
    owner.extend([0xd800, 0, 0xdc00]);
    let declaration = [0xd800, 0];
    let property_name = [0xdc00, 0xd800];
    let names: &[&[u16]] = &[&owner, &declaration, NAMES[2], &property_name, NAMES[4]];
    // Same five fallback records, with owner index0 declared local.
    let mut bytes = vec![6, 0x0a, 2, 8, 5, 0x28, 0, 0x18, 0];
    bytes.extend(parameter(7, 1));
    bytes.extend(property(3, &id_type(7)));
    let input = input(bytes, names);
    let result = decode(&input, Limits::default()).unwrap();
    let class_id: &metadata::class_id::ClassId = &result.owner.class_id;
    assert_eq!(class_id.package_name(), units("a.b"));
    let mut relative = units("Outer/Quoted.Inner");
    relative.extend([0xd800, 0, 0xdc00]);
    assert_eq!(class_id.relative_class_name(), relative);
    assert!(class_id.local);
    assert_eq!(result.parameters[0].name, declaration);
    assert_eq!(result.properties[0].name, property_name);
    assert_eq!(result.properties[0].return_type.parameter_ordinal, 0);
}

#[test]
fn inline_precedence_duplicate_merge_refusal_and_atomic_failure() {
    let mut fields = parameter(0, 1);
    let mut body = vec![0x10, 3, 0x48, 127]; // Invalid table index is unselected.
    body.extend(message(3, &id_type(0)));
    fields.extend(message(10, &body));
    assert!(decode(&input_for_fields(&fields), Limits::default()).is_ok());
    body.extend(message(3, &[0x18, 1])); // Merging would change nullability; never last/first win.
    let mut duplicate = parameter(0, 1);
    duplicate.extend(message(10, &body));
    assert_eq!(kind(&duplicate), ErrorKind::DuplicateInlineRole);
    for (body, expected) in [
        (vec![0x10, 3], ErrorKind::MissingRole),
        (vec![0x10, 3, 0x48, 0], ErrorKind::UnsupportedTableRole),
        (
            message(3, &id_type(0)),
            ErrorKind::Property(metadata::property_header::ErrorKind::MissingName),
        ),
    ] {
        let mut fields = ordinary_fields();
        fields.extend(message(10, &body));
        assert_eq!(kind(&fields), expected); // Two prior valid properties cannot escape as success.
    }
    let mut malformed = class(&ordinary_fields());
    malformed.push(0x80);
    assert!(decode(&input(malformed, NAMES), Limits::default()).is_err());
}

#[test]
fn all_nonbare_type_forms_and_significant_wrong_wires_refuse() {
    for extra in [
        vec![8, 1],
        vec![0x12, 0],
        vec![0x20, 0],
        vec![0x2a, 0],
        vec![0x30, 0],
        vec![0x40, 0],
        vec![0x48, 1],
        vec![0x52, 0],
        vec![0x58, 0],
        vec![0x60, 0],
        vec![0x6a, 0],
        vec![0x70, 0],
    ] {
        let mut ty = id_type(0);
        ty.extend(extra);
        let mut fields = parameter(0, 1);
        fields.extend(property(3, &ty));
        assert_eq!(kind(&fields), ErrorKind::UnsupportedType);
    }
    for number in 1..=14 {
        let mut ty = id_type(0);
        // Length-delimited for scalar fields, scalar for known message fields.
        ty.extend(if matches!(number, 2 | 5 | 10 | 13) {
            scalar(number, 0)
        } else {
            message(number, &[])
        });
        let mut fields = parameter(0, 1);
        fields.extend(property(3, &ty));
        assert_eq!(kind(&fields), ErrorKind::UnsupportedType);
    }
    let mut fields = parameter(0, 1);
    fields.extend(property(3, &[]));
    assert_eq!(kind(&fields), ErrorKind::MissingParameterId);
    for extra in [
        message(5, &[]),
        scalar(10, 0),
        message(12, &[]),
        scalar(13, 0),
        message(17, &[]),
        message(1, &[]),
        message(2, &[]),
        scalar(3, 0),
        message(7, &[]),
        message(8, &[]),
        message(9, &[]),
        message(11, &[]),
    ] {
        let mut body = vec![0x10, 3];
        body.extend(message(3, &id_type(0)));
        body.extend(extra);
        let mut fields = parameter(0, 1);
        fields.extend(message(10, &body));
        assert_eq!(kind(&fields), ErrorKind::UnsupportedProperty);
    }
    for number in [1, 3, 5, 10] {
        let mut fields = ordinary_fields();
        fields.extend(if matches!(number, 5 | 10) {
            scalar(number, 0)
        } else {
            message(number, &[])
        });
        assert_eq!(kind(&fields), ErrorKind::UnsupportedClass);
    }
    for number in [1, 2] {
        let mut body = vec![8, 0, 0x10, 1];
        body.extend(message(number, &[]));
        let mut fields = message(5, &body);
        fields.extend(property(3, &id_type(0)));
        assert_eq!(kind(&fields), ErrorKind::UnsupportedParameter);
    }
}

#[test]
fn scalar_width_last_values_nullable_marking_and_opaque_provenance() {
    for nullable in [None, Some(false), Some(true)] {
        let mut ty = scalar(1, 1);
        ty.extend(scalar(1, 1u64 << 32)); // Effective int32 flags zero wins.
        ty.extend(id_type(99));
        ty.extend(scalar(7, (1u64 << 32) + 7)); // Exact signed int32 parameter ID 7.
        if let Some(marking) = nullable {
            ty.extend([0x18, u8::from(marking)]);
        }
        ty.extend(message(150, &[0xff])); // Unknown Type field stays bounded opaque.
        let mut declaration = vec![8, 99, 8, 7, 0x10, 2, 0x10, 1];
        declaration.extend(message(5, &[0xff])); // Bound body intentionally uninterpreted.
        let mut fields = message(5, &declaration);
        fields.extend(property(3, &ty));
        fields.extend(message(30, &[0xff])); // Unselected Class table stays opaque.
        let input = input_for_fields(&fields);
        let result = decode(&input, Limits::default()).unwrap();
        assert_eq!(result.properties[0].return_type.header.flags, Some(0));
        assert_eq!(result.properties[0].return_type.header.nullable, nullable);
        assert_eq!(result.properties[0].return_type.parameter_ordinal, 0);
        assert_eq!(result.parameters[0].name, units("A"));
        assert_eq!(result.parameters[0].parameter.upper_bound[0].raw, [0xff]);
    }
    let mut fields = parameter(0, 1);
    fields.extend(property(3, &[8, 0, 8, 1, 0x38, 0]));
    assert_eq!(kind(&fields), ErrorKind::UnsupportedType);
}

#[test]
fn required_fields_name_failures_and_container_selection() {
    for (fields, expected) in [
        (
            message(5, &[]),
            ErrorKind::TypeParameter(metadata::type_parameter::ErrorKind::MissingId),
        ),
        (
            message(5, &[8, 0]),
            ErrorKind::TypeParameter(metadata::type_parameter::ErrorKind::MissingName),
        ),
    ] {
        assert_eq!(kind(&fields), expected);
    }
    let missing_class = input(vec![4, 0x0a, 2, 8, 5], NAMES);
    assert_eq!(
        decode(&missing_class, Limits::default()).unwrap_err().kind,
        ErrorKind::Class(metadata::class_header::ErrorKind::MissingFqName)
    );
    for index in [0, 1, 2] {
        let mut fields = parameter(0, if index == 1 { 5 } else { 1 });
        fields.extend(property(if index == 2 { 5 } else { 3 }, &id_type(0)));
        let mut bytes = class(&fields);
        if index == 0 {
            bytes[6] = 5;
        }
        assert!(matches!(
            decode(&input(bytes, NAMES), Limits::default())
                .unwrap_err()
                .kind,
            ErrorKind::Name(_)
        ));
    }
    for (k, mv) in [(2, vec![2, 3, 0]), (1, vec![2, 2, 0])] {
        let mut value = input_for_fields(&ordinary_fields());
        let metadata::Outcome::Decoded { header, .. } = &mut value else {
            unreachable!()
        };
        header.k = k;
        header.mv = mv;
        assert_eq!(
            decode(&value, Limits::default()).unwrap_err().kind,
            ErrorKind::UnsupportedContainer
        );
    }
}

#[test]
fn exact_shared_boundaries_cover_all_members_and_cannot_reset() {
    let fields = ordinary_fields();
    let input = input_for_fields(&fields);
    let axes: [fn(&mut Limits, usize); 10] = [
        |l, n| l.bytes = n,
        |l, n| l.records = n,
        |l, n| l.nodes = n,
        |l, n| l.depth = n,
        |l, n| l.work = n,
        |l, n| l.owned_bytes = n,
        |l, n| l.entries = n,
        |l, n| l.range_slots = n,
        |l, n| l.queries = n,
        |l, n| l.output_units = n,
    ];
    let mut exact = Limits::default();
    for axis in axes {
        let (mut low, mut high) = (0, 16 * 1024 * 1024);
        while low < high {
            let mid = low + (high - low) / 2;
            let mut limits = Limits::default();
            axis(&mut limits, mid);
            if decode(&input, limits).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        assert!(low > 0);
        axis(&mut exact, low);
        let mut below = Limits::default();
        axis(&mut below, low - 1);
        assert!(decode(&input, below).is_err());
    }
    let expected = decode(&input, Limits::default()).unwrap();
    assert_eq!(decode(&input, exact).unwrap(), expected);
    assert_eq!(exact.depth, 2);
    assert_eq!(exact.queries, 6); // owner string/local, two declaration names, two properties.
    assert_eq!(exact.output_units, 7 + 2 + 5 + 6);
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        unreachable!()
    };
    assert_eq!(
        exact.bytes,
        bytes.len() + header.d2.iter().map(|s| s.len() * 2).sum::<usize>()
    );
    let mut more = fields;
    more.extend(property(3, &id_type(0)));
    let more = input_for_fields(&more);
    for (axis, n) in [
        (axes[1], exact.records),
        (axes[2], exact.nodes),
        (axes[4], exact.work),
        (axes[5], exact.owned_bytes),
        (axes[8], exact.queries),
        (axes[9], exact.output_units),
    ] {
        let mut limits = Limits::default();
        axis(&mut limits, n);
        assert!(decode(&more, limits).is_err());
    }
    assert_eq!(
        decode(
            &input,
            Limits {
                depth: usize::MAX,
                ..exact
            }
        )
        .unwrap(),
        expected
    );
}

#[test]
fn malformed_selected_leaves_fail_without_panicking_or_partial_success() {
    for byte in 0..=255u8 {
        let mut fields = ordinary_fields();
        fields.extend(property(3, &[byte]));
        assert!(decode(&input_for_fields(&fields), Limits::default()).is_err());
    }
}
