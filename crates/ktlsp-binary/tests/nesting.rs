use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, nesting, parse_class,
};

fn text(s: &str) -> Constant {
    Constant::Utf8(s.encode_utf16().collect())
}
fn model(attributes: Vec<Attribute<'_>>) -> ClassFile<'_> {
    ClassFile {
        minor: 0,
        major: 67,
        constants: vec![
            Constant::Unusable,
            text("Current"),
            Constant::Class(1),
            text("Owner"),
            Constant::Class(3),
            text("InnerClasses"),
            text("EnclosingMethod"),
            text("NestHost"),
            text("NestMembers"),
            text("Named"),
            text("factory"),
            text("()V"),
            Constant::NameAndType {
                name: 10,
                descriptor: 11,
            },
            text("Other"),
            Constant::Class(13),
            Constant::Long(0),
            Constant::Unusable,
        ],
        access_flags: 0x00a1,
        this_class: 2,
        super_class: 4,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes,
    }
}
fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_be_bytes()).collect()
}
fn attr(name: u16, bytes: &[u8]) -> Attribute<'_> {
    Attribute { name, bytes }
}

#[test]
fn explicit_member_static_local_anonymous_and_initializer_metadata() {
    let bytes = words(&[2, 2, 4, 9, 0x800a, 14, 4, 9, 0x0004]);
    let c = model(vec![attr(5, &bytes)]);
    let n = nesting::decode(&c).unwrap();
    let entries = n.inner_classes.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].inner.index, 2);
    assert_eq!(entries[0].outer.unwrap().index, 4);
    assert_eq!(entries[0].name_index, Some(9));
    assert_eq!(entries[0].access_flags, 0x800a); // Preserve reserved bits and private/static.
    assert_eq!(entries[1].access_flags, 4);
    assert_eq!(c.access_flags, 0x00a1);
    for name in [0, 9] {
        let inner = words(&[1, 2, 0, name, 0]);
        for method in [0, 12] {
            let enclosing = words(&[4, method]);
            let c = model(vec![attr(5, &inner), attr(6, &enclosing)]);
            let n = nesting::decode(&c).unwrap();
            let entry = &n.inner_classes.as_ref().unwrap()[0];
            assert_eq!(entry.outer, None);
            assert_eq!(entry.name_index, (name != 0).then_some(name));
            let e = n.enclosing.unwrap();
            assert_eq!(e.class.name, "Owner".encode_utf16().collect::<Vec<_>>());
            assert_eq!(
                e.method.as_ref().map(|m| m.index),
                (method != 0).then_some(method)
            );
            if let Some(m) = e.method {
                assert_eq!(m.descriptor, [40, 41, 86]);
            }
        }
    }
    // A dollar in a binary name conveys no relationship.
    let mut c = model(vec![]);
    c.constants[1] = text("Has$Dollar");
    assert_eq!(nesting::decode(&c).unwrap(), nesting::Nesting::default());
    let c = model(vec![attr(5, &[0, 0]), attr(8, &[0, 0])]);
    assert_eq!(nesting::decode(&c).unwrap().inner_classes, Some(vec![]));
    assert_eq!(nesting::decode(&c).unwrap().nest_members, Some(vec![]));
}

#[test]
fn nestmates_are_declarations_not_lexical_nesting_or_access_control() {
    let c = model(vec![attr(7, &[0, 4])]);
    let n = nesting::decode(&c).unwrap();
    assert_eq!(n.nest_host.unwrap().index, 4);
    assert!(n.inner_classes.is_none());
    let bytes = words(&[4, 14, 2, 14, 4]);
    let c = model(vec![attr(8, &bytes)]);
    let n = nesting::decode(&c).unwrap();
    assert_eq!(
        n.nest_members
            .unwrap()
            .iter()
            .map(|c| c.index)
            .collect::<Vec<_>>(),
        [14, 2, 14, 4]
    );
    let c = model(vec![attr(7, &[0, 2])]); // Self host is also retained.
    assert_eq!(nesting::decode(&c).unwrap().nest_host.unwrap().index, 2);
    let c = model(vec![attr(7, &[0, 4]), attr(8, &[0, 0])]);
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Malformed("conflicting nest attributes")
    );
}

#[test]
fn lengths_duplicates_contexts_and_versions() {
    let payloads = [
        (5, words(&[1, 2, 4, 9, 1])),
        (6, words(&[4, 12])),
        (7, words(&[4])),
        (8, words(&[1, 14])),
    ];
    for (name, bytes) in &payloads {
        assert!(nesting::decode(&model(vec![attr(*name, bytes)])).is_ok());
        let mut module = model(vec![attr(*name, bytes)]);
        module.access_flags = 0x8000;
        module.constants[1] = text("module-info");
        module.super_class = 0;
        assert_eq!(nesting::decode(&module).is_ok(), *name == 5);
        for end in 0..bytes.len() {
            assert!(
                nesting::decode(&model(vec![attr(*name, &bytes[..end])])).is_err(),
                "{name} prefix {end}"
            );
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(nesting::decode(&model(vec![attr(*name, &extra)])).is_err());
        assert_eq!(
            nesting::decode(&model(vec![attr(*name, bytes), attr(*name, bytes)])).unwrap_err(),
            Error::Malformed("duplicate nesting attribute")
        );
        for method in [false, true] {
            let mut c = model(vec![]);
            let member = ktlsp_binary::Member {
                access_flags: 0x8002,
                name: 9,
                descriptor: 11,
                attributes: vec![attr(*name, bytes)],
            };
            if method {
                c.methods.push(member);
            } else {
                c.fields.push(member);
            }
            assert_eq!(
                nesting::decode(&c).unwrap_err(),
                Error::Malformed("nesting attribute context")
            );
            assert_eq!(
                if method {
                    c.methods[0].access_flags
                } else {
                    c.fields[0].access_flags
                },
                0x8002
            );
        }
        for (major, minor) in [
            (45, 0),
            (45, 2),
            (45, 3),
            (48, 0),
            (49, 0),
            (54, 0),
            (55, 0),
            (67, 0),
        ] {
            let mut c = model(vec![attr(*name, bytes)]);
            c.major = major;
            c.minor = minor;
            let allowed = match name {
                5 => (major, minor) >= (45, 3),
                6 => major >= 49,
                _ => major >= 55,
            };
            assert_eq!(
                nesting::decode(&c).is_ok(),
                allowed,
                "{name} {major}.{minor}"
            );
        }
    }
    for (major, minor) in [(44, 0), (45, 4), (67, 1), (70, 0), (65535, 65535)] {
        let mut c = model(vec![]);
        c.major = major;
        c.minor = minor;
        assert_eq!(
            nesting::decode(&c).unwrap_err(),
            Error::UnsupportedVersion { major, minor }
        );
    }
    let bytes = words(&[1, 2, 4, 0, 0]);
    for major in [50, 51] {
        let mut c = model(vec![attr(5, &bytes)]);
        c.major = major;
        assert_eq!(nesting::decode(&c).is_ok(), major == 50);
    }
    assert!(nesting::decode(&model(vec![attr(5, &[255, 255])])).is_err());
    assert!(nesting::decode(&model(vec![attr(8, &[255, 255])])).is_err());
}

#[test]
fn typed_zero_wide_hole_and_recursive_references_in_public_models() {
    for invalid in [0, 1, 9, 12, 15, 16, 17, 65535] {
        let inner = words(&[1, invalid, 0, 9, 0]);
        let outer = words(&[1, 2, invalid, 9, 0]);
        let enclosing = words(&[invalid, 0]);
        let host = words(&[invalid]);
        let members = words(&[1, invalid]);
        for (name, bytes) in [(5, &inner), (6, &enclosing), (7, &host), (8, &members)] {
            assert!(
                nesting::decode(&model(vec![attr(name, bytes)])).is_err(),
                "{name}: {invalid}"
            );
        }
        assert_eq!(
            nesting::decode(&model(vec![attr(5, &outer)])).is_ok(),
            invalid == 0
        );
        let mut c = model(vec![]);
        c.this_class = invalid;
        assert!(nesting::decode(&c).is_err());
    }
    for invalid in [1, 2, 4, 9, 15, 16, 17, 65535] {
        let bytes = words(&[4, invalid]);
        assert!(nesting::decode(&model(vec![attr(6, &bytes)])).is_err());
    }
    for invalid in [2, 4, 12, 15, 16, 17, 65535] {
        let bytes = words(&[1, 2, 0, invalid, 0]);
        assert!(nesting::decode(&model(vec![attr(5, &bytes)])).is_err());
        assert!(nesting::decode(&model(vec![attr(invalid, &[])])).is_err());
    }
    // Even a caller putting a typed constant at CP slot zero cannot make it valid.
    let mut c = model(vec![attr(7, &[0, 0])]);
    c.constants[0] = Constant::Class(1);
    assert!(nesting::decode(&c).is_err());
    c.attributes = vec![attr(0, &[])];
    c.constants[0] = text("NestMembers");
    assert!(nesting::decode(&c).is_err());
    for target in [0, 2, 15, 16, 65535] {
        let mut c = model(vec![attr(7, &[0, 4])]);
        c.constants[4] = Constant::Class(target);
        assert!(nesting::decode(&c).is_err());
    }
    for (name, desc, valid) in [
        ("<init>", "()V", true),
        ("<init>", "()I", false),
        ("<clinit>", "()V", false),
        ("bad/name", "()V", false),
        ("f", "I", false),
        ("f", "(V)V", false),
        ("f", "()Vx", false),
        ("f", "([[I)Ljava/lang/Object;", true),
    ] {
        let mut c = model(vec![attr(6, &[0, 4, 0, 12])]);
        c.constants[10] = text(name);
        c.constants[11] = text(desc);
        assert_eq!(nesting::decode(&c).is_ok(), valid, "{name} {desc}");
    }
    for bad in ["", "[I", "a//b", "a.b", "a;b"] {
        let mut c = model(vec![attr(7, &[0, 4])]);
        c.constants[3] = text(bad);
        assert!(nesting::decode(&c).is_err());
    }
    for slots in [254, 255] {
        for constructor in [false, true] {
            let mut c = model(vec![attr(6, &[0, 4, 0, 12])]);
            c.constants[10] = text(if constructor { "<init>" } else { "factory" });
            c.constants[11] = text(&format!("({})V", "I".repeat(slots)));
            assert_eq!(nesting::decode(&c).is_ok(), !constructor || slots == 254);
        }
    }
}

#[test]
fn alias_identities_inconsistencies_cycles_and_lossless_surrogates() {
    for entries in [
        vec![1, 2, 2, 9, 0],
        vec![2, 2, 4, 9, 0, 2, 14, 9, 0],
        vec![2, 2, 4, 9, 0, 4, 2, 9, 0],
    ] {
        let bytes = words(&entries);
        assert!(nesting::decode(&model(vec![attr(5, &bytes)])).is_err());
    }
    // Different CP indices with the same binary name must not hide duplicates/cycles.
    let bytes = words(&[2, 2, 4, 9, 0, 14, 4, 9, 0]);
    let mut c = model(vec![attr(5, &bytes)]);
    c.constants[13] = text("Current");
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Malformed("duplicate inner class")
    );
    let bytes = words(&[1, 2, 14, 9, 0]);
    c.attributes = vec![attr(5, &bytes)];
    assert!(nesting::decode(&c).is_err());
    let bytes = words(&[1, 2, 4, 9, 0]);
    let c = model(vec![attr(5, &bytes), attr(6, &[0, 4, 0, 0])]);
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Malformed("member class has enclosing method")
    );
    let bytes = words(&[1, 4, 2, 9, 0]);
    let c = model(vec![attr(5, &bytes), attr(6, &[0, 4, 0, 0])]);
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Malformed("cyclic nesting")
    );
    let bytes = words(&[1, 2, 0, 9, 0]);
    let mut c = model(vec![attr(5, &bytes)]);
    c.constants[1] = Constant::Utf8(vec![0xd800]);
    c.constants[9] = Constant::Utf8(vec![0xdc00]);
    assert_eq!(
        nesting::decode(&c).unwrap().inner_classes.unwrap()[0]
            .inner
            .name,
        [0xd800]
    );
    assert_eq!(c.utf16(9).unwrap(), [0xdc00]);
}

#[test]
fn constructed_typed_wide_second_slots_remain_unusable() {
    for double in [false, true] {
        let mut valid = model(vec![attr(7, &[0, 18])]);
        valid.constants[15] = if double {
            Constant::Double(0)
        } else {
            Constant::Long(0)
        };
        valid.constants.push(text("AfterWide"));
        valid.constants.push(Constant::Class(17));
        assert_eq!(
            nesting::decode(&valid).unwrap().nest_host.unwrap().index,
            18
        );
        for (attribute, payload, target) in [
            (7, words(&[13]), Constant::Class(3)),
            (13, words(&[4]), text("NestHost")),
            (7, words(&[4]), text("ForgedClassName")),
            (
                6,
                words(&[4, 13]),
                Constant::NameAndType {
                    name: 10,
                    descriptor: 11,
                },
            ),
            (6, words(&[4, 17]), text("factory")),
            (6, words(&[4, 17]), text("()V")),
        ] {
            let mut c = model(vec![attr(attribute, &payload)]);
            c.constants[12] = if double {
                Constant::Double(0)
            } else {
                Constant::Long(0)
            };
            let is_descriptor = target == text("()V");
            let is_name = target == text("factory");
            c.constants[13] = target;
            if attribute == 7 && payload == [0, 4] {
                c.constants[4] = Constant::Class(13);
            }
            if is_name || is_descriptor {
                c.constants.push(Constant::NameAndType {
                    name: if is_name { 13 } else { 10 },
                    descriptor: if is_descriptor { 13 } else { 11 },
                });
            }
            assert_eq!(
                nesting::decode(&c).unwrap_err(),
                Error::Malformed("nesting wide second slot")
            );
        }
    }
}

#[test]
fn aggregate_limits_repeated_references_and_long_chains() {
    let mut c = model(vec![]);
    c.interfaces = vec![4; MAX_RECORDS - c.constants.len()];
    assert!(nesting::decode(&c).is_ok());
    c.attributes.push(attr(8, &[0, 0]));
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Limit("nesting records")
    );
    let bytes = vec![0; MAX_BYTES];
    let c = model(vec![attr(9, &bytes)]);
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Limit("nesting bytes")
    );
    let mut c = model(vec![]);
    c.constants[9] = Constant::Utf8(vec![65; MAX_BYTES / 2]);
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Limit("nesting bytes")
    );
    let mut c = model(vec![]);
    c.constants[9] = Constant::Utf8(vec![65; 65535]);
    c.fields.push(ktlsp_binary::Member {
        access_flags: 0,
        name: 9,
        descriptor: 11,
        attributes: (0..1100).map(|_| attr(9, &[])).collect(),
    });
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Limit("nesting work")
    );
    let mut c = model(vec![]);
    let mut records = vec![3000];
    for i in 0..3000_u16 {
        let n = c.constants.len() as u16;
        c.constants
            .push(text(&format!("{}{:04}", "Prefix".repeat(30), i)));
        c.constants.push(Constant::Class(n));
        records.extend([n + 1, if i == 2999 { 0 } else { n + 3 }, 9, 0]);
    }
    let bytes = words(&records);
    c.attributes = vec![attr(5, &bytes)];
    assert_eq!(
        nesting::decode(&c).unwrap().inner_classes.unwrap().len(),
        3000
    );
    // Closing the chain must terminate with a controlled error, without recursion.
    records[2 + 2999 * 4] = 18;
    let cyclic = words(&records);
    c.attributes = vec![attr(5, &cyclic)];
    assert_eq!(
        nesting::decode(&c).unwrap_err(),
        Error::Malformed("cyclic nesting")
    );
}

#[test]
fn deterministic_attribute_mutation_and_truncation_corpus() {
    let inputs = [
        (5, words(&[2, 2, 4, 9, 10, 14, 0, 0, 0])),
        (6, words(&[4, 12])),
        (7, words(&[4])),
        (8, words(&[2, 2, 14])),
    ];
    for (name, input) in inputs {
        for i in 0..input.len() {
            for value in 0..=255 {
                let mut bytes = input.clone();
                bytes[i] = value;
                let _ = nesting::decode(&model(vec![attr(name, &bytes)]));
            }
        }
    }
    let mut seed = 0x8313_u32;
    for len in 0..256 {
        let bytes: Vec<_> = (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as u8
            })
            .collect();
        for name in 5..=8 {
            let _ = nesting::decode(&model(vec![attr(name, &bytes)]));
        }
    }
}

#[test]
fn precompiled_apache_fixtures_and_every_byte_truncation() {
    let fixtures = [
        (
            include_bytes!("../fixtures/t017c1/AppendableJoiner$Builder.class.bin") as &[u8],
            25,
            Some("org/apache/commons/lang3/AppendableJoiner"),
            false,
        ),
        (
            include_bytes!("../fixtures/t017c1/CharRange$CharacterIterator.class.bin"),
            26,
            Some("org/apache/commons/lang3/CharRange"),
            false,
        ),
        (
            include_bytes!("../fixtures/t017c1/AnnotationUtils$1.class.bin"),
            0,
            None,
            true,
        ),
    ];
    for (bytes, flags, outer, initializer) in fixtures {
        let c = parse_class(bytes).unwrap();
        let n = nesting::decode(&c).unwrap();
        let Constant::Class(name) = c.constants[c.this_class as usize] else {
            panic!()
        };
        let entry = n
            .inner_classes
            .as_ref()
            .unwrap()
            .iter()
            .find(|e| e.inner.name == c.utf16(name).unwrap())
            .unwrap();
        assert_eq!(entry.access_flags, flags);
        assert_eq!(
            entry
                .outer
                .map(|o| String::from_utf16(o.name).unwrap())
                .as_deref(),
            outer
        );
        if initializer {
            let e = n.enclosing.unwrap();
            // Static initializer enclosure is encoded with method_index zero.
            assert!(e.method.is_none());
            assert_eq!(
                String::from_utf16(e.class.name).unwrap(),
                "org/apache/commons/lang3/AnnotationUtils"
            );
            assert!(entry.name_index.is_none());
        } else {
            assert!(n.enclosing.is_none());
        }
        for end in 0..bytes.len() {
            assert!(parse_class(&bytes[..end]).is_err());
        }
    }
}

#[test]
fn modern_nonempty_nesting_keeps_encoded_relationships() {
    for major in [68, 69] {
        let inner = words(&[1, 2, 4, 9, 0x800a]);
        let mut c = model(vec![attr(5, &inner)]);
        c.major = major;
        let decoded = nesting::decode(&c).unwrap();
        let row = &decoded.inner_classes.as_ref().unwrap()[0];
        assert_eq!(
            (
                row.inner.index,
                row.outer.unwrap().index,
                row.name_index,
                row.access_flags
            ),
            (2, 4, Some(9), 0x800a)
        );
    }
}
