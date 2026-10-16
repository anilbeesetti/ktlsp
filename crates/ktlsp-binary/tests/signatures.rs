use ktlsp_binary::signatures::*;
use ktlsp_binary::{Attribute, ClassFile, Constant, Error, Member, parse_class};

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn parse(s: &str, c: Context) -> Result<(), Error> {
    parse_signature(&u(s), c).map(drop)
}
fn simple<'a>(
    package: Vec<&'a [u16]>,
    name: &'a [u16],
    arguments: Vec<Argument<'a>>,
) -> ClassType<'a> {
    ClassType {
        package,
        segments: vec![Segment { name, arguments }],
    }
}
fn blank() -> ClassFile<'static> {
    ClassFile {
        minor: 0,
        major: 67,
        constants: vec![
            Constant::Unusable,
            Constant::Utf8(u("Signature")),
            Constant::Utf8(u("TT;")),
        ],
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes: vec![],
    }
}
fn member(attrs: Vec<Attribute<'static>>) -> Member<'static> {
    Member {
        access_flags: 0,
        name: 0,
        descriptor: 0,
        attributes: attrs,
    }
}
fn attr() -> Attribute<'static> {
    Attribute {
        name: 1,
        bytes: &[0, 2],
    }
}

#[test]
fn complete_owner_variance_array_ast() {
    let text = u("Lpkg/Outer<TT;>.Inner<+Ljava/lang/Number;-[I*>;");
    assert_eq!(
        parse_signature(&text, Context::Field).unwrap(),
        Signature::Field(Type::Class(ClassType {
            package: vec![&u("pkg")],
            segments: vec![
                Segment {
                    name: &u("Outer"),
                    arguments: vec![Argument::Exact(Type::Variable(&u("T")))]
                },
                Segment {
                    name: &u("Inner"),
                    arguments: vec![
                        Argument::Extends(Type::Class(simple(
                            vec![&u("java"), &u("lang")],
                            &u("Number"),
                            vec![]
                        ))),
                        Argument::Super(Type::Array {
                            dimensions: 1,
                            component: Box::new(Type::Base(73))
                        }),
                        Argument::Any,
                    ]
                },
            ],
        }))
    );
}
#[test]
fn complete_class_bounds_and_recursive_arguments_ast() {
    let text = u(
        "<T:Ljava/lang/Object;:Ljava/lang/Comparable<TT;>;U::Ljava/io/Serializable;>Ljava/lang/Object;Lpkg/I<TU;>;",
    );
    assert_eq!(
        parse_signature(&text, Context::Class).unwrap(),
        Signature::Class(ClassSignature {
            parameters: vec![
                Parameter {
                    name: &u("T"),
                    class_bound: Some(Type::Class(simple(
                        vec![&u("java"), &u("lang")],
                        &u("Object"),
                        vec![]
                    ))),
                    interface_bounds: vec![Type::Class(simple(
                        vec![&u("java"), &u("lang")],
                        &u("Comparable"),
                        vec![Argument::Exact(Type::Variable(&u("T")))]
                    ))]
                },
                Parameter {
                    name: &u("U"),
                    class_bound: None,
                    interface_bounds: vec![Type::Class(simple(
                        vec![&u("java"), &u("io")],
                        &u("Serializable"),
                        vec![]
                    ))]
                },
            ],
            superclass: simple(vec![&u("java"), &u("lang")], &u("Object"), vec![]),
            interfaces: vec![simple(
                vec![&u("pkg")],
                &u("I"),
                vec![Argument::Exact(Type::Variable(&u("U")))]
            )],
        })
    );
}
#[test]
fn complete_method_ast_and_void_constructor() {
    let text = u("<E:Ljava/lang/Exception;>(BCDFIJSZ[[TT;)TT;^TE;^Ljava/io/IOException;");
    let mut arguments: Vec<_> = "BCDFIJSZ".encode_utf16().map(Type::Base).collect();
    let t = u("T");
    arguments.push(Type::Array {
        dimensions: 2,
        component: Box::new(Type::Variable(&t)),
    });
    assert_eq!(
        parse_signature(&text, Context::Method).unwrap(),
        Signature::Method(MethodSignature {
            parameters: vec![Parameter {
                name: &u("E"),
                class_bound: Some(Type::Class(simple(
                    vec![&u("java"), &u("lang")],
                    &u("Exception"),
                    vec![]
                ))),
                interface_bounds: vec![]
            }],
            arguments,
            result: Some(Type::Variable(&t)),
            throws: vec![
                Type::Variable(&u("E")),
                Type::Class(simple(
                    vec![&u("java"), &u("io")],
                    &u("IOException"),
                    vec![]
                ))
            ],
        })
    );
    let text = u("()V");
    assert_eq!(
        parse_signature(&text, Context::Method).unwrap(),
        Signature::Method(MethodSignature {
            parameters: vec![],
            arguments: vec![],
            result: None,
            throws: vec![]
        })
    );
}
#[test]
fn optional_bounds_and_lossless_identifier_rules() {
    for s in [
        "<A:Long:>Ljava/lang/Object;",
        "<A:T:>Ljava/lang/Object;",
        "<A:L:TT;>Ljava/lang/Object;",
        "<A:>Ljava/lang/Object;",
    ] {
        parse(s, Context::Class).unwrap();
    }
    let text = u("<A:Long:>Ljava/lang/Object;");
    let Signature::Class(c) = parse_signature(&text, Context::Class).unwrap() else {
        panic!()
    };
    assert_eq!(
        c.parameters,
        [
            Parameter {
                name: &u("A"),
                class_bound: None,
                interface_bounds: vec![]
            },
            Parameter {
                name: &u("Long"),
                class_bound: None,
                interface_bounds: vec![]
            }
        ]
    );
    for code in 0..=u16::MAX {
        let text = [84, code, 59];
        let accepted = parse_signature(&text, Context::Field).is_ok();
        assert_eq!(
            accepted,
            ![46, 59, 91, 47, 60, 62, 58].contains(&code),
            "code {code}"
        );
        if accepted {
            assert_eq!(
                parse_signature(&text, Context::Field).unwrap(),
                Signature::Field(Type::Variable(&text[1..2]))
            );
        }
    }
}
#[test]
fn grammar_rejects_malformed_and_wrong_context() {
    for s in [
        "",
        "I",
        "V",
        "T;",
        "TT",
        "L;",
        "L/foo;",
        "Lfoo/;",
        "Lfoo..Bar;",
        "Lfoo.;",
        "Lfoo<>;",
        "Lfoo<+>;",
        "Lfoo<I>;",
        "Lfoo<TT;>/Bar;",
        "[V",
        "[",
        "TT;;",
        "Lfoo;garbage",
        "()V",
        "<T:>TT;",
    ] {
        assert!(parse(s, Context::Field).is_err(), "{s}");
    }
    for s in [
        "",
        "TT;",
        "[I",
        "<>Ljava/lang/Object;",
        "<T>Ljava/lang/Object;",
        "<T::>Ljava/lang/Object;",
        "<T:I>Ljava/lang/Object;",
        "<T:Ljava/lang/Object;>",
        "Ljava/lang/Object;TT;",
        "Ljava/lang/Object;()V",
    ] {
        assert!(parse(s, Context::Class).is_err(), "{s}");
    }
    for s in [
        "",
        "()",
        "(V)V",
        "()VV",
        "()V^[I",
        "()V^I",
        "()V^",
        "(TT;)TT;x",
        "<> ()V",
        "()Lfoo<>;",
    ] {
        assert!(parse(s, Context::Method).is_err(), "{s}");
    }
}
#[test]
fn every_unit_truncation_and_generated_malformed_tails() {
    for (s, c) in [
        ("Lpkg/Outer<TT;>.Inner<+[I-[[TT;*>;", Context::Field),
        ("<T:Ljava/lang/Object;>([TT;)TT;", Context::Method),
        ("<T:Ljava/lang/Object;>Ljava/lang/Object;", Context::Class),
    ] {
        let text = u(s);
        parse_signature(&text, c).unwrap();
        for end in 0..text.len() {
            assert!(
                parse_signature(&text[..end], c).is_err(),
                "prefix {end} of {s}"
            );
        }
        for bad in [0, 59, 62, 91, 0xd800, 0xffff] {
            let mut tail = text.clone();
            tail.push(bad);
            assert!(parse_signature(&tail, c).is_err());
        }
    }
    for n in 0..300 {
        for (head, tail) in [
            ("Lx<", ">;"),
            ("[", ""),
            ("<X:", ">Ljava/lang/Object;"),
            ("()V^", ""),
        ] {
            let text = u(&format!("{}?{}", head.repeat(n), tail.repeat(n)));
            for c in [Context::Class, Context::Field, Context::Method] {
                assert!(parse_signature(&text, c).is_err());
            }
        }
    }
}
#[test]
fn context_attributes_duplicates_lengths_indices_and_versions() {
    let mut c = blank();
    c.constants.push(Constant::Utf8(u("Ljava/lang/Object;")));
    c.constants.push(Constant::Utf8(u("()V")));
    c.constants.push(Constant::Integer(0));
    c.attributes.push(Attribute {
        name: 1,
        bytes: &[0, 3],
    });
    c.fields.push(member(vec![attr()]));
    c.fields.push(member(vec![]));
    c.methods.push(member(vec![Attribute {
        name: 1,
        bytes: &[0, 4],
    }]));
    let got = decode_signatures(&c).unwrap();
    assert!(matches!(got.class, Some(Signature::Class(_))));
    assert!(matches!(got.fields[0], Some(Signature::Field(_))));
    assert_eq!(got.fields[1], None);
    assert!(matches!(got.methods[0], Some(Signature::Method(_))));
    assert_eq!(c.fields[0].attributes[0].bytes, [0, 2]);
    for attrs in [
        &mut c.attributes,
        &mut c.fields[0].attributes,
        &mut c.methods[0].attributes,
    ] {
        let saved_name = attrs[0].name;
        let saved_bytes = attrs[0].bytes;
        attrs.push(Attribute {
            name: saved_name,
            bytes: saved_bytes,
        });
    }
    assert!(decode_signatures(&c).is_err());
    c.attributes.pop();
    assert!(decode_signatures(&c).is_err());
    c.fields[0].attributes.pop();
    assert!(decode_signatures(&c).is_err());
    c.methods[0].attributes.pop();
    decode_signatures(&c).unwrap();
    for bytes in [
        &[][..],
        &[0][..],
        &[0, 3, 0][..],
        &[0, 0][..],
        &[0, 5][..],
        &[255, 255][..],
        &[0, 4][..],
    ] {
        c.attributes[0].bytes = bytes;
        assert!(decode_signatures(&c).is_err(), "{bytes:?}");
    }
    c.attributes[0].bytes = &[0, 3];
    for name in [0, 5, u16::MAX] {
        c.attributes[0].name = name;
        assert!(decode_signatures(&c).is_err());
    }
    c.constants[0] = Constant::Utf8(u("Signature"));
    c.attributes[0].name = 0;
    assert!(decode_signatures(&c).is_err());
    c.attributes[0].name = 1;
    c.constants[0] = Constant::Utf8(u("Ljava/lang/Object;"));
    c.attributes[0].bytes = &[0, 0];
    assert!(decode_signatures(&c).is_err());
    c.attributes[0].bytes = &[0, 3];
    for (major, minor) in [(48, 0), (70, 0), (67, 65535), (44, 0)] {
        c.major = major;
        c.minor = minor;
        assert!(decode_signatures(&c).is_err());
    }
}
#[test]
fn exact_standalone_and_whole_operation_limits() {
    let text = u("TT;");
    let exact = Limits {
        bytes: 6,
        nodes: 3,
        work: 3,
        records: 0,
        depth: 1,
    };
    parse_signature_with_limits(&text, Context::Field, exact).unwrap();
    for l in [
        Limits { bytes: 5, ..exact },
        Limits { nodes: 2, ..exact },
        Limits { work: 2, ..exact },
        Limits { depth: 0, ..exact },
    ] {
        assert!(matches!(
            parse_signature_with_limits(&text, Context::Field, l),
            Err(Error::Limit(_))
        ));
    }
    let mut c = blank();
    c.fields = vec![member(vec![attr()]), member(vec![attr()])];
    let exact = Limits {
        bytes: 16,
        nodes: 6,
        work: 24,
        records: 5,
        depth: 1,
    };
    decode_signatures_with_limits(&c, exact).unwrap();
    for l in [
        Limits { bytes: 15, ..exact },
        Limits { nodes: 5, ..exact },
        Limits { work: 23, ..exact },
        Limits {
            records: 4,
            ..exact
        },
        Limits { depth: 0, ..exact },
    ] {
        assert!(matches!(
            decode_signatures_with_limits(&c, l),
            Err(Error::Limit(_))
        ));
    }
    c.fields[0].attributes[0].name = 2; // Ignored attributes still consume budget.
    c.fields[1].attributes.clear();
    decode_signatures_with_limits(
        &c,
        Limits {
            nodes: 0,
            bytes: 2,
            work: 3,
            records: 4,
            depth: 0,
        },
    )
    .unwrap();
    assert!(
        decode_signatures_with_limits(
            &c,
            Limits {
                work: 2,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        decode_signatures_with_limits(
            &c,
            Limits {
                bytes: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    c.fields = (0..MAX_RECORDS).map(|_| member(vec![])).collect();
    assert_eq!(
        decode_signatures(&c).unwrap_err(),
        Error::Limit("signature records")
    );
    c.fields.pop();
    decode_signatures(&c).unwrap();
    assert_eq!(
        parse_signature(&vec![84; MAX_BYTES / 2 + 1], Context::Field).unwrap_err(),
        Error::Limit("signature bytes")
    );
}
#[test]
fn depth_arrays_flat_width_and_partial_ast_drop() {
    for depth in [1, MAX_DEPTH - 1, MAX_DEPTH, MAX_DEPTH + 1, 10000] {
        let text = u(&format!(
            "{}TT;{}",
            "Lx<".repeat(depth - 1),
            ">;".repeat(depth - 1)
        ));
        assert_eq!(
            parse_signature(&text, Context::Field).is_ok(),
            depth <= MAX_DEPTH
        );
        let mut broken = text.clone();
        broken.pop();
        assert!(parse_signature(&broken, Context::Field).is_err());
    }
    for n in [1, 255, 256] {
        assert_eq!(
            parse(&format!("{}I", "[".repeat(n)), Context::Field).is_ok(),
            n <= 255
        );
    }
    let text = u(&format!("L{};", "x.".repeat(MAX_NODES)));
    assert_eq!(
        parse_signature(&text, Context::Field).unwrap_err(),
        Error::Limit("signature nodes")
    );
    let text = u(&format!("L{}x;", "x.".repeat((MAX_NODES - 4) / 2)));
    parse_signature(&text, Context::Field).unwrap();
}
#[test]
fn deterministic_mutations_random_utf16_and_forged_models_never_panic() {
    let seeds = [
        u("<T:Ljava/lang/Object;>Ljava/lang/Object;"),
        u("Lx<TT;>.Y<+[I*>;"),
        u("<E:>(TT;)V^TE;"),
    ];
    for text in seeds {
        for at in 0..text.len() {
            for code in [0, 40, 41, 58, 59, 60, 62, 76, 84, 91, 0xd800, 0xffff] {
                let mut probe = text.clone();
                probe[at] = code;
                for c in [Context::Class, Context::Field, Context::Method] {
                    let _ = parse_signature(&probe, c);
                }
            }
        }
    }
    let mut seed = 17u32;
    for n in 0..4096 {
        let text: Vec<_> = (0..n % 128)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 16) as u16
            })
            .collect();
        for c in [Context::Class, Context::Field, Context::Method] {
            let _ = parse_signature(&text, c);
        }
        let mut c = blank();
        c.constants.push(Constant::Class((seed >> 16) as u16));
        c.fields.push(member(vec![Attribute {
            name: (seed >> 16) as u16,
            bytes: &[255, 255],
        }]));
        let _ = decode_signatures(&c);
    }
}
#[test]
fn precompiled_apache_pair_independent_expected_ast() {
    let bytes = include_bytes!("../fixtures/t017b/Pair.class");
    let c = parse_class(bytes).unwrap();
    assert_eq!(c.major, 52);
    let got = decode_signatures(&c).unwrap();
    let object = || {
        Type::Class(simple(
            vec![&[106, 97, 118, 97], &[108, 97, 110, 103]],
            &[79, 98, 106, 101, 99, 116],
            vec![],
        ))
    };
    let params = || {
        vec![
            Parameter {
                name: &[76],
                class_bound: Some(object()),
                interface_bounds: vec![],
            },
            Parameter {
                name: &[82],
                class_bound: Some(object()),
                interface_bounds: vec![],
            },
        ]
    };
    assert_eq!(
        got.class,
        Some(Signature::Class(ClassSignature {
            parameters: params(),
            superclass: simple(vec![&u("java"), &u("lang")], &u("Object"), vec![]),
            interfaces: vec![
                simple(
                    vec![&u("java"), &u("util")],
                    &u("Map$Entry"),
                    vec![
                        Argument::Exact(Type::Variable(&[76])),
                        Argument::Exact(Type::Variable(&[82]))
                    ]
                ),
                simple(
                    vec![&u("java"), &u("lang")],
                    &u("Comparable"),
                    vec![Argument::Exact(Type::Class(simple(
                        vec![
                            &u("org"),
                            &u("apache"),
                            &u("commons"),
                            &u("lang3"),
                            &u("tuple")
                        ],
                        &u("Pair"),
                        vec![
                            Argument::Exact(Type::Variable(&[76])),
                            Argument::Exact(Type::Variable(&[82]))
                        ]
                    )))]
                ),
                simple(vec![&u("java"), &u("io")], &u("Serializable"), vec![]),
            ],
        }))
    );
    assert_eq!(got.fields.iter().filter(|s| s.is_some()).count(), 1);
    assert_eq!(
        got.fields[c
            .fields
            .iter()
            .position(|m| c.utf16(m.name).unwrap() == u("EMPTY_ARRAY"))
            .unwrap()],
        Some(Signature::Field(Type::Array {
            dimensions: 1,
            component: Box::new(Type::Class(simple(
                vec![
                    &u("org"),
                    &u("apache"),
                    &u("commons"),
                    &u("lang3"),
                    &u("tuple")
                ],
                &u("Pair"),
                vec![Argument::Any, Argument::Any]
            )))
        }))
    );
    assert_eq!(got.methods.iter().filter(|s| s.is_some()).count(), 11);
    for (m, signature) in c.methods.iter().zip(&got.methods) {
        if c.utf16(m.name).unwrap() == u("getLeft") {
            assert_eq!(
                *signature,
                Some(Signature::Method(MethodSignature {
                    parameters: vec![],
                    arguments: vec![],
                    result: Some(Type::Variable(&[76])),
                    throws: vec![]
                }))
            );
        }
        if c.utf16(m.name).unwrap() == u("accept") {
            assert_eq!(
                *signature,
                Some(Signature::Method(MethodSignature {
                    parameters: vec![Parameter {
                        name: &[69],
                        class_bound: Some(Type::Class(simple(
                            vec![&u("java"), &u("lang")],
                            &u("Throwable"),
                            vec![]
                        ))),
                        interface_bounds: vec![]
                    }],
                    arguments: vec![Type::Class(simple(
                        vec![
                            &u("org"),
                            &u("apache"),
                            &u("commons"),
                            &u("lang3"),
                            &u("function")
                        ],
                        &u("FailableBiConsumer"),
                        vec![
                            Argument::Exact(Type::Variable(&[76])),
                            Argument::Exact(Type::Variable(&[82])),
                            Argument::Exact(Type::Variable(&[69]))
                        ]
                    ))],
                    result: None,
                    throws: vec![Type::Variable(&[69])],
                }))
            );
        }
    }
    for end in 0..bytes.len() {
        assert!(parse_class(&bytes[..end]).is_err());
    }
}

#[test]
fn bounded_depth_subprocess_guard() {
    const CHILD: &str = "KTLSP_T017B_CRASH_CHILD";
    if std::env::var_os(CHILD).is_some() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                for n in [MAX_DEPTH, MAX_DEPTH + 1, 10000] {
                    let text = u(&format!("{}TT;{}", "Lx<".repeat(n - 1), ">;".repeat(n - 1)));
                    assert_eq!(
                        parse_signature(&text, Context::Field).is_ok(),
                        n <= MAX_DEPTH
                    );
                    let mut broken = text;
                    broken.pop();
                    let _ = parse_signature(&broken, Context::Field);
                }
                for prefix in ["Lx<", "Lx<+", "Lx<-"] {
                    let nested = format!(
                        "{}TT;{}",
                        prefix.repeat(MAX_DEPTH - 1),
                        ">;".repeat(MAX_DEPTH - 1)
                    );
                    for (text, context) in [
                        (nested.clone(), Context::Field),
                        (format!("<T:{nested}>Lx;"), Context::Class),
                        (format!("({nested}){nested}"), Context::Method),
                    ] {
                        let text = u(&text);
                        parse_signature_with_limits(
                            &text,
                            context,
                            Limits {
                                depth: usize::MAX,
                                ..Limits::default()
                            },
                        )
                        .unwrap();
                        let mut broken = text;
                        broken.pop();
                        let _ = parse_signature(&broken, context);
                    }
                }
                let nested = u(&format!(
                    "{}TT;{}",
                    "[Lx<".repeat((MAX_DEPTH - 1) / 2),
                    ">;".repeat((MAX_DEPTH - 1) / 2)
                ));
                parse_signature(&nested, Context::Field).unwrap();
            })
            .unwrap()
            .join()
            .unwrap();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "bounded_depth_subprocess_guard", "--nocapture"])
        .env(CHILD, "1")
        .env("PATH", "")
        .env_remove("JAVA_HOME")
        .env_remove("JDK_HOME")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn optional_suffixes_have_valid_complete_prefixes() {
    for (s, context, complete) in [
        ("()V^TE;^Lx;", Context::Method, vec![3, 7, 11]),
        ("Lx;Ly;Lz;", Context::Class, vec![3, 6, 9]),
    ] {
        let text = u(s);
        for end in 0..=text.len() {
            assert_eq!(
                parse_signature(&text[..end], context).is_ok(),
                complete.contains(&end),
                "{s} prefix {end}"
            );
        }
    }
}

#[test]
fn modern_nonempty_signatures_preserve_owner_contexts() {
    for major in [68, 69] {
        let mut c = blank();
        c.major = major;
        c.constants.push(Constant::Utf8(u("Ljava/lang/Object;")));
        c.constants.push(Constant::Utf8(u("()V")));
        c.attributes.push(Attribute {
            name: 1,
            bytes: &[0, 3],
        });
        c.fields.push(member(vec![attr()]));
        c.methods.push(member(vec![Attribute {
            name: 1,
            bytes: &[0, 4],
        }]));
        let got = decode_signatures(&c).unwrap();
        assert!(matches!(got.class, Some(Signature::Class(_))));
        assert!(matches!(got.fields[0], Some(Signature::Field(_))));
        assert!(matches!(got.methods[0], Some(Signature::Method(_))));
    }
}
