use ktlsp_binary::{
    annotations::{Annotation, Visibility},
    metadata::{
        self,
        resolved_alias_signature::{ErrorKind, Limits, decode},
        wire,
    },
};

static RAW: Annotation<'static> = Annotation {
    descriptor: &[],
    elements: Vec::new(),
};
fn input<'a>(bytes: Vec<u8>, d2: &[&'a [u16]]) -> metadata::Outcome<'a> {
    metadata::Outcome::Decoded {
        bytes,
        header: metadata::Header {
            raw: &RAW,
            visibility: &Visibility::RuntimeVisible,
            present: 0,
            k: 2,
            mv: vec![2, 3, 0],
            bv: vec![],
            d1: vec![],
            d2: d2.to_vec(),
            xs: &[],
            pn: &[],
            xi: 0,
        },
    }
}
fn message(field: u8, body: &[u8]) -> Vec<u8> {
    assert!(body.len() < 128);
    let mut out = vec![field * 8 + 2, body.len() as u8];
    out.extend_from_slice(body);
    out
}
fn alias(underlying: &[u8], expanded: &[u8]) -> Vec<u8> {
    let mut out = vec![0x10, 0];
    out.extend(message(4, underlying));
    out.extend(message(6, expanded));
    out
}
fn package(body: &[u8]) -> Vec<u8> {
    let mut out = vec![4, 0x0a, 0, 0x0a, 0]; // Two fallback records select d2[0/1].
    out.extend(message(5, body));
    out
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn genuine_predefined_any_is_not_empty_d2() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_package_member_names/KTypeParameterBaseKt.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    let metadata::Outcome::Decoded { header, .. } = &input else {
        panic!("decoded Package")
    };
    assert!(header.d2[1].is_empty());
    let signatures = decode(&input, Limits::default()).unwrap();
    assert_eq!(signatures.len(), 1);
    let s = &signatures[0];
    assert_eq!(s.name, units("TypeParameterContainer"));
    assert_eq!(
        (s.ordinal, s.span.clone(), s.body_span.clone()),
        (0, 9..23, 11..23)
    );
    for t in [&s.underlying, &s.expanded] {
        assert_eq!(t.class_id.package_name(), units("kotlin"));
        assert_eq!(t.class_id.relative_class_name(), units("Any"));
        assert!(!t.class_id.local && !t.nullable);
        assert_eq!(t.nullable_presence, None);
    }
}

#[test]
fn nominal_flags_use_int32_width_and_last_scalar_precedence() {
    let declaration = units("Alias");
    let classifier = units("pkg/Value");
    for underlying in [
        &[0x08, 0x80, 0x80, 0x80, 0x80, 0x10, 0x30, 1][..],
        &[0x08, 1, 0x08, 0x80, 0x80, 0x80, 0x80, 0x10, 0x30, 1][..],
    ] {
        let input = input(
            package(&alias(underlying, &[0x30, 1])),
            &[&declaration, &classifier],
        );
        let signatures = decode(&input, Limits::default()).unwrap();
        assert_eq!(signatures[0].underlying.class_id.package_name(), units("pkg"));
        assert_eq!(signatures[0].underlying.class_id.relative_class_name(), units("Value"));
        assert_eq!(signatures[0].underlying, signatures[0].expanded);
    }
}

#[test]
fn class_id_quoted_slash_nested_local_and_lossless_units() {
    let mut name = units("a/b/`Outer/Quoted`.Inner");
    name.extend([0xd800, 0, 0xdc00]);
    let declaration = [0xd800, 0];
    let mut bytes = vec![6, 0x0a, 0, 0x0a, 0, 0x28, 1]; // Two records; local_name = 1.
    bytes.extend(message(5, &alias(&[0x30, 1, 0x18, 0], &[0x30, 1, 0x18, 1])));
    let input = input(bytes, &[&declaration, &name]);
    let result = decode(&input, Limits::default()).unwrap();
    assert_eq!(result[0].name, declaration);
    let mut relative = units("Outer/Quoted.Inner");
    relative.extend([0xd800, 0, 0xdc00]);
    for t in [&result[0].underlying, &result[0].expanded] {
        assert_eq!(t.class_id.package_name(), units("a.b"));
        assert_eq!(t.class_id.relative_class_name(), relative);
        assert!(t.class_id.local);
    }
    assert_eq!(result[0].underlying.nullable_presence, Some(false));
    assert!(!result[0].underlying.nullable);
    assert_eq!(result[0].expanded.nullable_presence, Some(true));
    assert!(result[0].expanded.nullable);
    for (text, package_name, relative) in [
        ("Plain.Nested", "", "Plain.Nested"),
        ("`a/b`", "", "a/b"),
        ("/Root", "", "Root"),
    ] {
        let text = units(text);
        let input = input_for_class(&text);
        let result = decode(&input, Limits::default()).unwrap();
        assert_eq!(
            result[0].underlying.class_id.package_name(),
            units(package_name)
        );
        assert_eq!(
            result[0].underlying.class_id.relative_class_name(),
            units(relative)
        );
    }
    let root = units("Root");
    let slash_root = units("/Root");
    let plain = decode(&input_for_class(&root), Limits::default()).unwrap();
    let prefixed = decode(&input_for_class(&slash_root), Limits::default()).unwrap();
    assert_eq!(
        plain[0].underlying.class_id,
        prefixed[0].underlying.class_id
    );
}
fn input_for_class(name: &[u16]) -> metadata::Outcome<'_> {
    input(package(&alias(&[0x30, 1], &[0x30, 1])), &[&[65], name])
}

#[test]
fn inline_priority_duplicates_missing_and_generic_refusals() {
    let names: &[&[u16]] = &[&[65], &[67]];
    let mut body = alias(&[0x30, 1], &[0x30, 1]);
    body.extend([0x28, 127, 0x38, 127]);
    assert!(decode(&input(package(&body), names), Limits::default()).is_ok());
    for field in [4, 6] {
        let mut duplicate = body.clone();
        duplicate.extend(message(field, &[0x30, 1]));
        assert_eq!(
            decode(&input(package(&duplicate), names), Limits::default())
                .unwrap_err()
                .kind,
            ErrorKind::DuplicateInlineRole
        );
    }
    for (body, kind) in [
        (vec![0x10, 0], ErrorKind::MissingRole),
        (
            vec![0x10, 0, 0x28, 0, 0x38, 0],
            ErrorKind::UnsupportedTableRole,
        ),
        (vec![0x22, 0, 0x32, 0], ErrorKind::MissingName),
        (vec![0x10, 0, 0x1a, 0], ErrorKind::UnsupportedTypeParameters),
        (alias(&[], &[0x30, 1]), ErrorKind::MissingClass),
    ] {
        assert_eq!(
            decode(&input(package(&body), names), Limits::default())
                .unwrap_err()
                .kind,
            kind
        );
    }
    // Every known non-nominal semantic form refuses, even alongside a class classifier.
    for extra in [
        vec![8, 1],
        vec![0x12, 0],
        vec![0x20, 0],
        vec![0x2a, 0],
        vec![0x38, 0],
        vec![0x40, 0],
        vec![0x48, 0],
        vec![0x52, 0],
        vec![0x58, 0],
        vec![0x60, 0],
        vec![0x6a, 0],
        vec![0x70, 0],
    ] {
        let mut ty = vec![0x30, 1];
        ty.extend(extra);
        assert_eq!(
            decode(
                &input(package(&alias(&ty, &[0x30, 1])), names),
                Limits::default()
            )
            .unwrap_err()
            .kind,
            ErrorKind::UnsupportedType
        );
    }
    let malformed = package(&alias(&[0x30], &[0x30, 1]));
    assert!(matches!(
        decode(&input(malformed, names), Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Wire(_)
    ));
}

#[test]
fn complete_scan_no_partial_results_and_constructed_container_validation() {
    let names: &[&[u16]] = &[&[65], &[67]];
    let mut bytes = package(&alias(&[0x30, 1], &[0x30, 1]));
    bytes.extend([0x80]);
    assert!(matches!(
        decode(&input(bytes, names), Limits::default()),
        Err(metadata::resolved_alias_signature::Error {
            kind: ErrorKind::Wire(_),
            ..
        })
    ));
    let mut bytes = package(&alias(&[0x30, 1], &[0x30, 1]));
    bytes.extend(message(5, &[0x10, 0]));
    assert_eq!(
        decode(&input(bytes, names), Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::MissingRole
    );
    for (k, mv) in [(1, vec![2, 3, 0]), (2, vec![2, 2, 0]), (5, vec![2, 3, 0])] {
        let mut value = input(package(&alias(&[0x30, 1], &[0x30, 1])), names);
        let metadata::Outcome::Decoded { header, .. } = &mut value else {
            unreachable!()
        };
        header.k = k;
        header.mv = mv;
        assert_eq!(decode(&value, Limits::default()).is_ok(), k == 5);
    }
    let bad_index = input(package(&alias(&[0x30, 2], &[0x30, 1])), names);
    assert!(matches!(
        decode(&bad_index, Limits::default()).unwrap_err().kind,
        ErrorKind::Name(_)
    ));
}

#[test]
fn exact_aggregate_allowances_never_reset_between_roles_or_declarations() {
    let class = ktlsp_binary::parse_class(include_bytes!(
        "fixtures/metadata_package_member_names/KTypeParameterBaseKt.class.bin"
    ))
    .unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    // Locate each exact admission boundary without pinning allocator layouts or error wording.
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
    assert_eq!(exact.queries, 5); // declaration, two classifier strings, two local queries.
    assert_eq!(exact.output_units, 22 + 2 * 10);
    let metadata::Outcome::Decoded { header, bytes } = &input else {
        unreachable!()
    };
    let mut doubled = bytes.clone();
    doubled.extend_from_slice(&bytes[9..23]);
    let doubled = self::input(doubled, &header.d2);
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
        assert!(decode(&doubled, limits).is_err());
    }
    let raised = Limits {
        depth: usize::MAX,
        ..Limits::default()
    };
    assert_eq!(decode(&input, raised).unwrap(), expected);
    let tiny = Limits {
        bytes: 0,
        ..Limits::default()
    };
    assert_eq!(
        decode(&input, tiny).unwrap_err().kind,
        ErrorKind::Wire(wire::ErrorKind::Limit("bytes"))
    );
}
