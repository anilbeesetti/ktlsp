use ktlsp_binary::{
    Error,
    descriptors::{
        Component, FieldDescriptor, MAX_UNITS, MethodDescriptor, Primitive, SlotContext,
    },
    parse_class,
};

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn method(text: &[u16], receiver: bool) -> Result<MethodDescriptor<'_>, Error> {
    MethodDescriptor::parse(
        text,
        if receiver {
            SlotContext::WithReceiver
        } else {
            SlotContext::WithoutReceiver
        },
    )
}

#[test]
fn primitive_array_and_lossless_borrowed_identity() {
    for (tag, kind, slots) in [
        ("B", Primitive::Byte, 1),
        ("C", Primitive::Char, 1),
        ("D", Primitive::Double, 2),
        ("F", Primitive::Float, 1),
        ("I", Primitive::Int, 1),
        ("J", Primitive::Long, 2),
        ("S", Primitive::Short, 1),
        ("Z", Primitive::Boolean, 1),
    ] {
        let raw = units(tag);
        let field = FieldDescriptor::parse(&raw).unwrap();
        assert_eq!(field.component(), Component::Primitive(kind));
        assert_eq!((field.dimensions(), field.slots()), (0, slots));
        assert!(std::ptr::eq(field.raw(), raw.as_slice()));
        let raw = units(&format!("[[{tag}"));
        let field = FieldDescriptor::parse(&raw).unwrap();
        assert_eq!((field.dimensions(), field.slots()), (2, 1));
        assert_eq!(field.component(), Component::Primitive(kind));
    }
    // Isolated surrogate, encoded NUL and a surrogate pair must remain untouched.
    let raw = [91, 91, 76, 112, 47, 0xd800, 0, 0xd83d, 0xde00, 59];
    let field = FieldDescriptor::parse(&raw).unwrap();
    assert_eq!((field.dimensions(), field.slots()), (2, 1));
    assert!(std::ptr::eq(field.raw(), raw.as_slice()));
    let Component::Object(name) = field.component() else {
        panic!("object")
    };
    assert!(std::ptr::eq(name, &raw[3..9]));
}

#[test]
fn parameters_return_void_and_iterator_identity() {
    let raw = units("(ID[[JLjava/lang/Thread;)Ljava/lang/Object;");
    let m = method(&raw, true).unwrap();
    assert!(std::ptr::eq(m.raw(), raw.as_slice()));
    assert_eq!(m.parameter_slots(), 6);
    let mut params = m.parameters();
    let mut cloned = params.clone();
    for (begin, end, slots) in [(1, 2, 1), (2, 3, 2), (3, 6, 1), (6, 24, 1)] {
        let field = params.next().unwrap();
        assert!(std::ptr::eq(field.raw(), &raw[begin..end]));
        assert_eq!(field.slots(), slots);
        assert_eq!(cloned.next(), Some(field));
    }
    assert_eq!(params.next(), None);
    assert_eq!(params.next(), None);
    assert_eq!(m.parameters().count(), 4);
    let result = m.result().unwrap();
    assert!(std::ptr::eq(result.raw(), &raw[25..]));
    assert_eq!(
        result.component(),
        Component::Object(&raw[26..raw.len() - 1])
    );
    for (text, result) in [("()V", None), ("()J", Some(Primitive::Long))] {
        let raw = units(text);
        let m = method(&raw, false).unwrap();
        assert_eq!((m.parameters().count(), m.parameter_slots()), (0, 0));
        assert_eq!(
            m.result().map(|r| r.component()),
            result.map(Component::Primitive)
        );
    }
    assert_eq!(method(&units("()V"), true).unwrap().parameter_slots(), 1);
}

#[test]
fn malformed_complete_input_and_void_positions_fail_explicitly() {
    for text in [
        "",
        "V",
        "[V",
        "[",
        "L;",
        "Lfoo",
        "L/foo;",
        "Lfoo/;",
        "Lfoo//bar;",
        "Lfoo.bar;",
        "Lfoo[bar;",
        "I;",
        "II",
        "Ljava/lang/Object;;",
        "()V",
        "TName;",
        "Qx;",
    ] {
        assert!(FieldDescriptor::parse(&units(text)).is_err(), "{text}");
    }
    for text in [
        "",
        "I",
        "(",
        "()",
        "(I",
        "(I)",
        "(V)V",
        "([V)V",
        "()VV",
        "()VI",
        "()IV",
        "(I)Vx",
        "()L;",
        "(Lx)V",
        "(Lx;)V;",
        "((I)V",
        "()TName;",
        "()Lfoo//bar;",
    ] {
        assert!(method(&units(text), false).is_err(), "{text}");
    }
    // Every strict prefix is invalid; no partially validated view may escape.
    let raw = units("([[ILjava/lang/Object;)Ljava/lang/Thread;");
    for end in 0..raw.len() {
        assert!(method(&raw[..end], false).is_err(), "prefix {end}");
    }
    assert!(method(&raw, false).is_ok());
}

#[test]
fn array_rank_slot_context_and_input_boundaries() {
    let mut raw = vec![91; 255];
    raw.push(74);
    assert_eq!(FieldDescriptor::parse(&raw).unwrap().dimensions(), 255);
    raw.insert(0, 91);
    assert_eq!(
        FieldDescriptor::parse(&raw),
        Err(Error::Malformed("array depth"))
    );
    for (count, receiver, valid) in [
        (254, true, true),
        (255, true, false),
        (255, false, true),
        (256, false, false),
    ] {
        let raw = units(&format!("({})V", "I".repeat(count)));
        assert_eq!(method(&raw, receiver).is_ok(), valid);
    }
    let raw = units(&format!("({}I)V", "D".repeat(127)));
    assert_eq!(method(&raw, false).unwrap().parameter_slots(), 255);
    assert_eq!(method(&raw, true), Err(Error::Malformed("parameter slots")));
    let raw = units(&format!("({})V", "[J".repeat(255)));
    assert_eq!(method(&raw, false).unwrap().parameters().count(), 255);
    assert!(method(&raw, true).is_err());
    let mut raw = vec![120; MAX_UNITS];
    raw[0] = 76;
    raw[MAX_UNITS - 1] = 59;
    assert_eq!(FieldDescriptor::parse(&raw).unwrap().raw().len(), MAX_UNITS);
    raw.insert(1, 120);
    assert_eq!(
        FieldDescriptor::parse(&raw),
        Err(Error::Limit("descriptor units"))
    );
    assert_eq!(method(&raw, false), Err(Error::Limit("descriptor units")));
    let raw = units(&format!("()L{};", "x".repeat(MAX_UNITS - 4)));
    assert_eq!(method(&raw, false).unwrap().raw().len(), MAX_UNITS);
}

#[test]
fn genuine_retained_classfiles_expose_structural_descriptors() {
    let fixtures: [&[u8]; 3] = [
        include_bytes!("../fixtures/t017c1/AppendableJoiner$Builder.class.bin"),
        include_bytes!("../fixtures/t017c1/CharRange$CharacterIterator.class.bin"),
        include_bytes!("../fixtures/t017c1/AnnotationUtils$1.class.bin"),
    ];
    for bytes in fixtures {
        let class = parse_class(bytes).unwrap();
        for field in &class.fields {
            let raw = class.utf16(field.descriptor).unwrap();
            let view = FieldDescriptor::parse(raw).unwrap();
            assert!(std::ptr::eq(view.raw(), raw));
        }
        for member in &class.methods {
            let raw = class.utf16(member.descriptor).unwrap();
            let receiver = member.access_flags & 8 == 0
                && class.utf16(member.name).unwrap() != units("<clinit>");
            let view = method(raw, receiver).unwrap();
            assert!(std::ptr::eq(view.raw(), raw));
            assert_eq!(
                view.parameters()
                    .map(|p| usize::from(p.slots()))
                    .sum::<usize>()
                    + usize::from(receiver),
                usize::from(view.parameter_slots())
            );
        }
    }
    let class = parse_class(fixtures[0]).unwrap();
    let field = FieldDescriptor::parse(class.utf16(class.fields[0].descriptor).unwrap()).unwrap();
    assert_eq!(
        field.component(),
        Component::Object(&units("java/lang/CharSequence"))
    );
    let setter = MethodDescriptor::parse(
        class.utf16(class.methods[2].descriptor).unwrap(),
        SlotContext::WithReceiver,
    )
    .unwrap();
    assert_eq!(setter.parameter_slots(), 2);
    assert_eq!(
        setter.parameters().next().unwrap().component(),
        field.component()
    );
    assert_eq!(
        setter.result().unwrap().component(),
        Component::Object(&units("org/apache/commons/lang3/AppendableJoiner$Builder"))
    );
    // The class reader rejects the same invalid field grammar, without changing its caller API.
    let mut malformed = fixtures[0].to_vec();
    let needle = b"Ljava/lang/CharSequence;";
    let at = malformed
        .windows(needle.len())
        .position(|s| s == needle)
        .unwrap();
    malformed[at] = b'V';
    assert!(FieldDescriptor::parse(&units("Vjava/lang/CharSequence;")).is_err());
    assert!(parse_class(&malformed).is_err());
}
