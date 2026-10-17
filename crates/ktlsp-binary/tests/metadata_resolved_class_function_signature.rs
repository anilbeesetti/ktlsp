use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self, class_context,
        resolved_class_function_parameters::{self as parameters, Classifier},
        resolved_class_function_return as self_return,
        resolved_class_function_signature::{ErrorKind, Limits, decode},
        type_argument::Projection,
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
fn scalar(number: u32, mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut key = u64::from(number) * 8;
    while key >= 128 {
        out.push(key as u8 | 128);
        key >>= 7;
    }
    out.push(key as u8);
    while value >= 128 {
        out.push(value as u8 | 128);
        value >>= 7;
    }
    out.push(value as u8);
    out
}
fn message(number: u32, body: &[u8]) -> Vec<u8> {
    let mut out = scalar(number, body.len() as u64);
    out[0] |= 2;
    out.extend(body);
    out
}
fn declaration(number: u32, id: i32, name: u8) -> Vec<u8> {
    let mut body = scalar(1, u64::from(id as u32));
    body.extend(scalar(2, u64::from(name)));
    message(number, &body)
}
fn leaf(id: i32) -> Vec<u8> {
    scalar(7, u64::from(id as u32))
}
fn value(ty: &[u8]) -> Vec<u8> {
    let mut body = vec![16, 4];
    body.extend(message(3, ty));
    message(6, &body)
}
fn function(roles: &[u8]) -> Vec<u8> {
    let mut body = vec![16, 3];
    body.extend(roles);
    message(9, &body)
}
fn class(fields: &[u8], table: &[u8]) -> Vec<u8> {
    assert!(table.len() < 128);
    let mut out = vec![table.len() as u8];
    out.extend(table);
    out.extend([24, 0]);
    out.extend(fields);
    out
}
fn authored(roles: &[u8]) -> metadata::Outcome<'static> {
    input(class(&function(roles), &[10, 2, 8, 7]), NAMES)
}
fn nominal(target: u8, arguments: &[Vec<u8>]) -> Vec<u8> {
    let mut out = scalar(6, u64::from(target));
    for argument in arguments {
        out.extend(message(2, argument));
    }
    out
}
fn return_role(arguments: &[Vec<u8>]) -> Vec<u8> {
    message(3, &nominal(6, arguments))
}
fn ordinary_roles() -> Vec<u8> {
    let mut roles = value(&scalar(9, 1));
    roles.extend(declaration(4, -7, 1)); // Deliberately after the value.
    roles.extend(return_role(&[message(2, &scalar(9, 1))]));
    roles
}
fn kind(roles: &[u8]) -> ErrorKind {
    decode(&authored(roles), 1, Limits::default())
        .unwrap_err()
        .kind
}
fn return_kind(ty: &[u8]) -> ErrorKind {
    let mut roles = declaration(4, -7, 1);
    roles.extend(message(3, ty));
    kind(&roles)
}
fn argument_kind(arg: &[u8]) -> ErrorKind {
    return_kind(&nominal(6, &[arg.to_vec()]))
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn borrowed(raw: &[u8], bytes: &[u8], span: std::ops::Range<usize>) {
    assert_eq!(raw, &bytes[span.clone()]);
    assert_eq!(raw.as_ptr(), bytes[span].as_ptr());
}

#[test]
fn genuine_result_companion_return_name_binds_its_original_selected_function() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Result$Companion.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let original = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { bytes, header } = &original else {
        panic!("decoded Class")
    };
    // Independent public resolver control authenticates encoded target index5;
    // neither the owner name nor a prospective schema name is the target oracle.
    let framed = metadata::payload::decode(bytes, Default::default()).unwrap();
    let mut resolver = metadata::name_resolver::Resolver::new(
        &framed.string_table,
        &header.d2,
        Default::default(),
    )
    .unwrap();
    let actual_target = resolver.get_string(5).unwrap();
    let target_local = resolver.is_local_class_name(5).unwrap();
    let mut declaration_pointers = Vec::new();
    for (ordinal, return_span, argument_span, leaf_span, own_span, value_span, value_type) in [
        (4, 55..63, 57..61, 59..61, 65..69, 71..77, 75..77),
        (5, 92..100, 94..98, 96..98, 102..106, 108..114, 112..114),
    ] {
        let selected = decode(&original, ordinal, Limits::default()).unwrap();
        assert_eq!(
            selected.parameters,
            parameters::decode(&original, ordinal, Limits::default()).unwrap()
        );
        assert_eq!(
            selected.parameters.owner.class_id.relative_class_name(),
            units("Result.Companion")
        );
        assert!(selected.parameters.parameters.is_empty());
        let f = &selected.parameters.function;
        assert_eq!(f.ordinal, ordinal);
        borrowed(f.header.raw, bytes, f.body_span.clone());
        assert_eq!(f.type_parameters.len(), 1);
        let own = &f.type_parameters[0];
        assert_eq!(own.body_span, own_span);
        borrowed(own.parameter.raw, bytes, own_span);
        assert_eq!(own.parameter.id, 1);
        assert_eq!(own.name, units("T"));
        declaration_pointers.push(own.parameter.raw.as_ptr());
        let value = &f.parameters[0];
        borrowed(value.raw.raw, bytes, value_span);
        borrowed(value.header.raw, bytes, value_type);
        if ordinal == 4 {
            assert_eq!(value.classifier, Classifier::FunctionParameter(0));
        } else {
            let Classifier::Class(id) = &value.classifier else {
                panic!("Throwable value")
            };
            assert_eq!(id.relative_class_name(), units("Throwable"));
        }
        let ret = &selected.return_type;
        assert_eq!(ret.body_span, return_span);
        assert_eq!(ret.span, return_span.start - 2..return_span.end);
        borrowed(ret.header.raw, bytes, return_span);
        assert_eq!(ret.header.class_name, Some(5));
        assert_eq!(ret.class_id.local, target_local);
        let mut resolved_identity = ret.class_id.package_name().to_vec();
        if !resolved_identity.is_empty() {
            resolved_identity.push(47);
        }
        resolved_identity.extend(ret.class_id.relative_class_name());
        assert_eq!(resolved_identity, actual_target);
        assert_ne!(ret.class_id, selected.parameters.owner.class_id);
        assert_eq!(ret.arguments.len(), 1); // NOT owning Class declaration count (zero).
        let arg = &ret.arguments[0];
        assert_eq!(arg.body_span, argument_span);
        assert_eq!(arg.type_span, leaf_span);
        borrowed(arg.raw.raw, bytes, argument_span);
        borrowed(arg.header.raw, bytes, leaf_span);
        assert_eq!(arg.header.raw, [0x48, 6]);
        assert_eq!(arg.header.type_parameter_name, Some(6));
        assert_eq!(arg.classifier, Classifier::FunctionParameter(0));
        assert_eq!(arg.raw.projection, None);
        assert_eq!(arg.header.nullable, None);
    }
    assert_ne!(declaration_pointers[0], declaration_pointers[1]);
    assert_eq!(
        decode(&original, 3, Limits::default()).unwrap_err().kind,
        ErrorKind::Parameters(parameters::ErrorKind::NotFunction)
    );
    assert_eq!(
        decode(&original, usize::MAX, Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Parameters(parameters::ErrorKind::MissingOrdinal)
    );
}

#[test]
fn genuine_pair_copy_keeps_values_and_explicit_self_class_target() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_constructor/Pair.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let original = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { bytes, .. } = &original else {
        unreachable!()
    };
    let result = decode(&original, 9, Limits::default()).unwrap();
    assert_eq!(
        result.parameters,
        parameters::decode(&original, 9, Limits::default()).unwrap()
    );
    assert_eq!(
        result.return_type.class_id,
        result.parameters.owner.class_id
    );
    assert_eq!(result.return_type.class_id.package_name(), units("kotlin"));
    assert_eq!(
        result.return_type.class_id.relative_class_name(),
        units("Pair")
    );
    assert_eq!(result.return_type.span, 153..169);
    borrowed(result.return_type.header.raw, bytes, 155..169);
    let old = self_return::decode(&original, 9, Limits::default()).unwrap();
    for (ordinal, (arg_span, leaf_span, declaration_span)) in
        [(157..161, 159..161, 54..60), (163..167, 165..167, 62..68)]
            .into_iter()
            .enumerate()
    {
        let arg = &result.return_type.arguments[ordinal];
        borrowed(arg.raw.raw, bytes, arg_span.clone());
        borrowed(arg.header.raw, bytes, leaf_span.clone());
        assert_eq!(arg.body_span, arg_span);
        assert_eq!(arg.type_span, leaf_span);
        assert_eq!(arg.classifier, Classifier::ClassParameter(ordinal));
        assert_eq!(arg.raw.projection, None);
        let declaration = &result.parameters.parameters[ordinal];
        borrowed(declaration.parameter.raw, bytes, declaration_span);
        assert_eq!(declaration.parameter.id, ordinal as i32);
        assert_eq!(
            result.parameters.function.parameters[ordinal].classifier,
            Classifier::ClassParameter(ordinal)
        );
        assert_eq!(
            old.function.return_type.arguments[ordinal].parameter_ordinal,
            ordinal
        );
    }
}

#[test]
fn authored_signed_shadow_class_fallback_and_late_original_scopes() {
    let mut roles = value(&scalar(7, u64::MAX)); // signed -1, NOT an ordinal
    roles.extend(return_role(&[
        message(2, &leaf(-1)),
        message(2, &leaf(12345)),
        message(2, &leaf(77)),
    ]));
    roles.extend(declaration(4, 12345, 1));
    roles.extend(declaration(4, -1, 1)); // Equal names are harmless for ID lookup.
    let mut fields = function(&roles);
    fields.extend(declaration(5, -1, 2)); // Own shadows this parent.
    fields.extend(declaration(5, 77, 2)); // Exact Class fallback.
    let original = input(class(&fields, &[10, 2, 8, 7]), NAMES);
    let selected = decode(&original, 1, Limits::default()).unwrap();
    assert_eq!(
        selected.parameters.function.parameters[0].classifier,
        Classifier::FunctionParameter(1)
    );
    for (arg, expected) in selected.return_type.arguments.iter().zip([
        Classifier::FunctionParameter(1),
        Classifier::FunctionParameter(0),
        Classifier::ClassParameter(1),
    ]) {
        assert_eq!(arg.classifier, expected);
    }
    assert_eq!(selected.parameters.parameters[0].parameter.id, -1);
    assert_eq!(
        selected.parameters.function.type_parameters[1].parameter.id,
        -1
    );
    let metadata::Outcome::Decoded { bytes, .. } = &original else {
        unreachable!()
    };
    for p in selected
        .parameters
        .parameters
        .iter()
        .chain(&selected.parameters.function.type_parameters)
    {
        borrowed(p.parameter.raw, bytes, p.body_span.clone());
    }
}

#[test]
fn authored_name_is_immediate_own_unique_resolver_content_and_exact_utf16() {
    let mut roles = declaration(4, 7, 1);
    roles.extend(return_role(&[message(2, &scalar(9, 2))]));
    let mut fields = declaration(5, 8, 2);
    fields.extend(function(&roles));
    let original = input(class(&fields, &[10, 2, 8, 7]), NAMES);
    assert_eq!(
        decode(&original, 2, Limits::default()).unwrap_err().kind,
        ErrorKind::Parameters(parameters::ErrorKind::UnknownParameterName)
    );
    roles.extend(declaration(4, 9, 2));
    let original = authored(&roles);
    assert_eq!(
        decode(&original, 1, Limits::default())
            .unwrap()
            .return_type
            .arguments[0]
            .classifier,
        Classifier::FunctionParameter(1)
    );
    // Distinct raw indices resolve through real record operations to equal content.
    for (record, encoded, resolved) in [
        (vec![24, 1], units("a/b/T$Inner"), units("a/b/T.Inner")),
        (vec![24, 2], units("La/b/T$Inner;"), units("a/b/T.Inner")),
        (vec![16, 0], vec![], units("kotlin/Any")),
    ] {
        let mut names = NAMES.to_vec();
        names[1] = &resolved;
        names[6] = &encoded;
        let mut table = message(1, &[8, 6]);
        table.extend(message(1, &record));
        let mut roles = declaration(4, 7, 1);
        roles.extend(message(3, &nominal(0, &[message(2, &scalar(9, 6))])));
        let original = input(class(&function(&roles), &table), &names);
        assert_eq!(
            decode(&original, 1, Limits::default())
                .unwrap()
                .return_type
                .arguments[0]
                .classifier,
            Classifier::FunctionParameter(0)
        );
        roles.extend(declaration(4, 8, 6));
        let original = input(class(&function(&roles), &table), &names);
        assert_eq!(
            decode(&original, 1, Limits::default()).unwrap_err().kind,
            ErrorKind::Parameters(parameters::ErrorKind::AmbiguousParameterName)
        );
    }
    for (query, declaration_name, expected) in [
        (vec![0xd800, 0, 0xdc00], vec![0xd800, 0, 0xdc00], true),
        (vec![0xfffd, 0, 0xfffd], vec![0xd800, 0, 0xdc00], false),
        (units("e\u{301}"), units("\u{e9}"), false),
    ] {
        let mut names = NAMES.to_vec();
        names[1] = &declaration_name;
        names[6] = &query;
        let mut roles = declaration(4, 7, 1);
        roles.extend(message(3, &nominal(0, &[message(2, &scalar(9, 6))])));
        let original = input(class(&function(&roles), &[10, 2, 8, 7]), &names);
        assert_eq!(decode(&original, 1, Limits::default()).is_ok(), expected);
    }
    assert_eq!(
        argument_kind(&message(2, &leaf(88))),
        ErrorKind::Parameters(parameters::ErrorKind::UnknownParameterId)
    );
    assert!(matches!(
        argument_kind(&message(2, &scalar(9, 99))),
        ErrorKind::Parameters(parameters::ErrorKind::Name(_))
    ));
}

#[test]
fn authored_target_is_independent_including_local_membership_and_unknown_arity() {
    let mut names = NAMES.to_vec();
    names[6] = names[0]; // Same resolved text, different local membership.
    let mut table = vec![10, 2, 8, 7];
    table.extend(scalar(5, 6));
    for arguments in [
        vec![],
        vec![message(2, &leaf(-7))],
        vec![message(2, &leaf(-7)); 3],
    ] {
        let mut roles = declaration(4, -7, 1);
        roles.extend(return_role(&arguments));
        let original = input(class(&function(&roles), &table), &names);
        let selected = decode(&original, 1, Limits::default()).unwrap();
        assert_eq!(
            selected.return_type.class_id.package_name(),
            selected.parameters.owner.class_id.package_name()
        );
        assert_eq!(
            selected.return_type.class_id.relative_class_name(),
            selected.parameters.owner.class_id.relative_class_name()
        );
        assert!(!selected.parameters.owner.class_id.local);
        assert!(selected.return_type.class_id.local);
        assert_ne!(
            selected.return_type.class_id,
            selected.parameters.owner.class_id
        );
        assert_eq!(selected.return_type.arguments.len(), arguments.len());
    }
    assert!(matches!(return_kind(&nominal(99, &[])), ErrorKind::Name(_)));
}

#[test]
fn authored_markings_projection_presence_inline_precedence_and_scalar_narrowing() {
    for nullable in [None, Some(false), Some(true)] {
        for projection in [None, Some(2)] {
            let mut ty = leaf(-7);
            if let Some(n) = nullable {
                ty.extend(scalar(3, u64::from(n)));
            }
            ty.extend(scalar(1, 1u64 << 32)); // Effective zero flags.
            let mut arg = message(2, &ty);
            arg.extend(scalar(3, u64::MAX)); // Inline beats raw table ID.
            if let Some(p) = projection {
                arg.extend(scalar(1, p));
            }
            let mut ret = nominal(6, &[arg]);
            if let Some(n) = nullable {
                ret.extend(scalar(3, u64::from(n)));
            }
            let mut roles = declaration(4, -7, 1);
            roles.extend(scalar(7, u64::MAX)); // Inline return wins too.
            roles.extend(message(3, &ret));
            let original = authored(&roles);
            let result = decode(&original, 1, Limits::default()).unwrap();
            assert_eq!(result.return_type.header.nullable, nullable);
            let argument = &result.return_type.arguments[0];
            assert_eq!(argument.header.nullable, nullable);
            assert_eq!(argument.raw.projection, projection.map(|_| Projection::Inv));
            assert_eq!(argument.raw.type_id, Some(-1));
            assert_eq!(argument.classifier, Classifier::FunctionParameter(0));
        }
    }
}

#[test]
fn authored_atomic_framing_scopes_values_and_opaque_unselected_bodies() {
    let mut roles = ordinary_roles();
    roles.extend(declaration(4, -7, 2)); // Duplicate LATE own ID.
    assert_eq!(
        kind(&roles),
        ErrorKind::Parameters(parameters::ErrorKind::Context(
            class_context::ErrorKind::DuplicateParameterId
        ))
    );
    let mut fields = function(&ordinary_roles());
    fields.extend(declaration(5, 9, 1));
    fields.extend(declaration(5, 9, 2)); // Duplicate late parent.
    let original = input(class(&fields, &[10, 2, 8, 7]), NAMES);
    assert_eq!(
        decode(&original, 1, Limits::default()).unwrap_err().kind,
        ErrorKind::Parameters(parameters::ErrorKind::Context(
            class_context::ErrorKind::DuplicateParameterId
        ))
    );
    let mut fields = function(&ordinary_roles());
    fields.extend(message(9, &[0x80]));
    fields.extend(message(10, &[0x80])); // Other Function/property bodies opaque.
    let original = input(class(&fields, &[10, 2, 8, 7]), NAMES);
    assert!(decode(&original, 1, Limits::default()).is_ok());
    fields.push(0x80);
    let original = input(class(&fields, &[10, 2, 8, 7]), NAMES);
    assert!(decode(&original, 1, Limits::default()).is_err());
    let mut roles = ordinary_roles();
    roles.push(0x80); // Selected Function malformed tail.
    assert!(decode(&authored(&roles), 1, Limits::default()).is_err());
    let mut roles = ordinary_roles();
    roles.extend(value(&leaf(999))); // Selected value cannot disappear.
    assert_eq!(
        kind(&roles),
        ErrorKind::Parameters(parameters::ErrorKind::UnknownParameterId)
    );
    let mut roles = ordinary_roles();
    let mut vararg = vec![16, 4];
    vararg.extend(message(3, &leaf(-7)));
    vararg.extend(message(4, &leaf(-7)));
    roles.extend(message(6, &vararg));
    assert_eq!(
        kind(&roles),
        ErrorKind::Parameters(parameters::ErrorKind::UnsupportedVararg)
    );
    let mut roles = ordinary_roles();
    for number in [5, 10, 11, 30, 32, 100] {
        roles.extend(message(number, &[0x80]));
    }
    assert!(decode(&authored(&roles), 1, Limits::default()).is_ok());
    let mut siblings = function(&declaration(4, -7, 1));
    siblings.extend(function(&return_role(&[message(2, &leaf(-7))])));
    let original = input(class(&siblings, &[10, 2, 8, 7]), NAMES);
    assert_eq!(
        decode(&original, 2, Limits::default()).unwrap_err().kind,
        ErrorKind::Parameters(parameters::ErrorKind::UnknownParameterId)
    );
}

#[test]
fn authored_all_return_and_argument_role_refusals_preserve_parameter_only_opacity() {
    assert_eq!(kind(&[]), ErrorKind::MissingRole);
    assert_eq!(kind(&scalar(7, 0)), ErrorKind::UnsupportedTableRole);
    let mut duplicate = ordinary_roles();
    duplicate.extend(return_role(&[]));
    assert_eq!(kind(&duplicate), ErrorKind::DuplicateInlineRole);
    let mut duplicate_arg = message(2, &leaf(-7));
    duplicate_arg.extend(message(2, &[]));
    assert_eq!(
        argument_kind(&duplicate_arg),
        ErrorKind::DuplicateInlineRole
    );
    assert_eq!(argument_kind(&[]), ErrorKind::MissingRole);
    assert_eq!(
        argument_kind(&scalar(3, 0)),
        ErrorKind::UnsupportedTableRole
    );
    for number in [3, 7] {
        let mut roles = ordinary_roles();
        roles.extend(if number == 3 {
            scalar(3, 0)
        } else {
            message(7, &[])
        });
        assert_eq!(kind(&roles), ErrorKind::UnsupportedFunction);
    }
    for projection in [0, 1, 3, 4, u64::MAX] {
        let mut arg = scalar(1, projection);
        arg.extend(message(2, &leaf(-7)));
        assert_eq!(argument_kind(&arg), ErrorKind::UnsupportedProjection);
        arg.extend(scalar(1, 2)); // Unknown projection occurrence never hidden by INV.
        if projection >= 4 {
            assert_eq!(argument_kind(&arg), ErrorKind::UnsupportedProjection);
        }
    }
    for (number, wrong) in [
        (1, message(1, &[])),
        (2, scalar(2, 0)),
        (3, message(3, &[])),
    ] {
        let mut arg = message(2, &leaf(-7));
        arg.extend(wrong);
        assert_eq!(
            argument_kind(&arg),
            if number == 1 {
                ErrorKind::UnsupportedProjection
            } else {
                ErrorKind::UnsupportedType
            }
        );
    }
    for number in 1..=14 {
        if [1, 2, 3, 6].contains(&number) {
            continue;
        }
        let mut ty = nominal(6, &[]);
        ty.extend(scalar(number, 0));
        assert_eq!(return_kind(&ty), ErrorKind::UnsupportedType);
    }
    for number in 1..=14 {
        if [1, 3, 7, 9].contains(&number) {
            continue;
        }
        let mut ty = leaf(-7);
        ty.extend(if number == 2 {
            message(2, &[])
        } else {
            scalar(number, 0)
        });
        assert!(matches!(
            argument_kind(&message(2, &ty)),
            ErrorKind::Parameters(parameters::ErrorKind::UnsupportedType)
                | ErrorKind::UnsupportedType
        ));
    }
    for number in [1, 3, 6] {
        let mut ty = nominal(6, &[]);
        ty.extend(message(number, &[]));
        assert_eq!(return_kind(&ty), ErrorKind::UnsupportedType);
    }
    for number in [1, 3, 7, 9] {
        let mut ty = leaf(-7);
        ty.extend(message(number, &[]));
        assert_eq!(
            argument_kind(&message(2, &ty)),
            ErrorKind::Parameters(parameters::ErrorKind::UnsupportedType)
        );
    }
    for ty in [leaf(-7), scalar(9, 1), vec![]] {
        assert_eq!(return_kind(&ty), ErrorKind::UnsupportedType);
    }
    let mut flagged = nominal(6, &[]);
    flagged.extend(scalar(1, 1));
    assert_eq!(return_kind(&flagged), ErrorKind::UnsupportedType);
    let mut flagged_leaf = leaf(-7);
    flagged_leaf.extend(scalar(1, 1));
    assert_eq!(
        argument_kind(&message(2, &flagged_leaf)),
        ErrorKind::Parameters(parameters::ErrorKind::UnsupportedType)
    );
    let mut conflicting = leaf(-7);
    conflicting.extend(scalar(9, 1));
    assert_eq!(
        argument_kind(&message(2, &conflicting)),
        ErrorKind::Parameters(parameters::ErrorKind::UnsupportedType)
    );
    for return_role in [message(3, &[0x80]), scalar(7, 0), message(3, &leaf(-7))] {
        let mut roles = declaration(4, -7, 1);
        roles.extend(value(&leaf(-7)));
        roles.extend(return_role);
        let original = authored(&roles);
        assert!(parameters::decode(&original, 1, Limits::default()).is_ok());
        assert!(decode(&original, 1, Limits::default()).is_err());
        assert_eq!(
            self_return::decode(&original, 1, Limits::default())
                .unwrap_err()
                .kind,
            self_return::ErrorKind::UnsupportedTypeParameters
        );
    }
}

type Set = fn(&mut Limits, usize);
fn minimum(original: &metadata::Outcome<'_>, set: Set, composed: bool) -> usize {
    let accepted = |n| {
        let mut limits = Limits::default();
        set(&mut limits, n);
        if composed {
            decode(original, 1, limits).is_ok()
        } else {
            parameters::decode(original, 1, limits).is_ok()
        }
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
    assert!(accepted(high));
    assert!(!accepted(high - 1));
    high
}

#[test]
fn authored_values_and_return_share_all_exact_monotonic_allowances() {
    let original = authored(&ordinary_roles());
    let dimensions: [Set; 9] = [
        |l, n| l.bytes = n,
        |l, n| l.records = n,
        |l, n| l.nodes = n,
        |l, n| l.work = n,
        |l, n| l.owned_bytes = n,
        |l, n| l.queries = n,
        |l, n| l.output_units = n,
        |l, n| l.entries = n,
        |l, n| l.range_slots = n,
    ];
    let mut exact = Limits {
        depth: 2,
        ..Default::default()
    };
    for set in dimensions {
        set(&mut exact, minimum(&original, set, true));
    }
    assert_eq!(
        decode(&original, 1, exact).unwrap(),
        decode(&original, 1, Limits::default()).unwrap()
    );
    let raised = Limits {
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
        decode(&original, 1, raised).unwrap(),
        decode(&original, 1, exact).unwrap()
    );
    for set in dimensions {
        let n = minimum(&original, set, true);
        let mut below = exact;
        set(&mut below, n - 1);
        assert!(decode(&original, 1, below).is_err());
    }
    assert!(
        decode(
            &original,
            1,
            Limits {
                depth: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        decode(
            &original,
            1,
            Limits {
                depth: usize::MAX,
                ..Default::default()
            }
        )
        .unwrap(),
        decode(&original, 1, exact).unwrap()
    );
    // These dimensions grow specifically because the return is interpreted, not
    // just because both independent child calls happened to fit separate budgets.
    for set in [
        dimensions[1],
        dimensions[2],
        dimensions[3],
        dimensions[4],
        dimensions[5],
        dimensions[6],
    ] {
        let parameter_only = minimum(&original, set, false);
        assert!(minimum(&original, set, true) > parameter_only);
        let mut limits = Limits::default();
        set(&mut limits, parameter_only);
        assert!(parameters::decode(&original, 1, limits).is_ok());
        assert!(decode(&original, 1, limits).is_err());
    }
    let mut extra_value = ordinary_roles();
    extra_value.extend(value(&scalar(9, 1)));
    let mut extra_return = value(&scalar(9, 1));
    extra_return.extend(declaration(4, -7, 1));
    extra_return.extend(return_role(&[
        message(2, &scalar(9, 1)),
        message(2, &scalar(9, 1)),
    ]));
    for roles in [extra_value, extra_return] {
        let larger = authored(&roles);
        assert!(decode(&larger, 1, Limits::default()).is_ok());
        for set in [
            dimensions[1],
            dimensions[2],
            dimensions[3],
            dimensions[4],
            dimensions[5],
            dimensions[6],
        ] {
            let mut limits = Limits::default();
            set(&mut limits, minimum(&original, set, true));
            assert!(decode(&larger, 1, limits).is_err());
        }
    }
    // Owner string+local, own declaration, Function, value NAME+name,
    // independently resolved return string+local, return leaf NAME: nine queries.
    assert_eq!(exact.queries, 9);
}

#[test]
fn authored_return_name_comparisons_charge_each_inspected_utf16_unit() {
    let target = vec![65u16; 256];
    let mut early = target.clone();
    early[0] = 66;
    let mut late = target.clone();
    late[255] = 66;
    let mut roles = declaration(4, 7, 1);
    roles.extend(declaration(4, 8, 2));
    roles.extend(message(3, &nominal(0, &[message(2, &scalar(9, 6))])));
    let mut names = NAMES.to_vec();
    names[1] = &early;
    names[2] = &target;
    names[6] = &target;
    let early_input = input(class(&function(&roles), &[10, 2, 8, 7]), &names);
    let early_work = minimum(&early_input, |l, n| l.work = n, true);
    names[1] = &late;
    let late_input = input(class(&function(&roles), &[10, 2, 8, 7]), &names);
    let late_work = minimum(&late_input, |l, n| l.work = n, true);
    assert_eq!(late_work - early_work, 255);
    assert!(
        decode(
            &late_input,
            1,
            Limits {
                work: early_work,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        decode(&late_input, 1, Limits::default())
            .unwrap()
            .return_type
            .arguments[0]
            .classifier,
        Classifier::FunctionParameter(1)
    );
}

#[test]
fn authored_original_container_admission_stays_class_only() {
    assert!(decode(&metadata::Outcome::Absent, 1, Limits::default()).is_err());
    for (k, mv) in [(2, vec![2, 3, 0]), (5, vec![2, 3, 0]), (1, vec![2, 2, 0])] {
        let mut original = authored(&ordinary_roles());
        let metadata::Outcome::Decoded { header, .. } = &mut original else {
            unreachable!()
        };
        header.k = k;
        header.mv = mv;
        assert_eq!(
            decode(&original, 1, Limits::default()).unwrap_err().kind,
            ErrorKind::Parameters(parameters::ErrorKind::Context(
                class_context::ErrorKind::UnsupportedContainer
            ))
        );
    }
}
