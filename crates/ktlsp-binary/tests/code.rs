use ktlsp_binary::code::{CodeBody, ExceptionHandler, Limits, decode};
use ktlsp_binary::{
    Attribute, ClassFile, Constant, Error, MAX_BYTES, MAX_RECORDS, Member, parse_class,
};

// Independently frozen literals: fixtures/code/authored.json, V01–V04.
const V01: &str = "00 01 00 01 00 00 00 01 b1 00 00 00 00";
const V02: &str = "00 02 00 03 00 00 00 04 00 00 00 b1 00 02 00 00 00 04 00 03 00 00 00 01 00 03 00 02 00 03 00 02 00 02 00 00 00 02 aa bb 00 02 00 00 00 00";
const V03: &str =
    "ff ff 00 00 00 00 00 01 ff 00 00 00 02 00 07 00 00 00 02 de ad 00 07 00 00 00 00";
const V04: &str = "00 00 00 00 00 00 00 01 00 00 00 00 01 00 01 00 00 00 01 00";
const UNIT: &[u8] = include_bytes!("../fixtures/t017c2/Unit.class");

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn class(raw: &[u8]) -> ClassFile<'_> {
    ClassFile {
        minor: 0,
        major: 52,
        constants: vec![
            Constant::Unusable,
            Constant::Utf8("Code".encode_utf16().collect()),
            Constant::Utf8(vec![88]),
            Constant::Class(0),
            Constant::Integer(0),
            Constant::Unusable,
            Constant::Utf8("Code".encode_utf16().collect()),
            Constant::Utf8(vec![0, 0xd800]),
            Constant::Utf8(vec![109]),
        ],
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![method(raw)],
        attributes: vec![],
    }
}
fn method(raw: &[u8]) -> Member<'_> {
    Member {
        access_flags: 0,
        name: 8,
        descriptor: 0,
        attributes: vec![Attribute {
            name: 1,
            bytes: raw,
        }],
    }
}
fn unlimited() -> Limits {
    Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    }
}
fn storage(methods: usize, exceptions: usize, attributes: usize) -> usize {
    methods * size_of::<Option<CodeBody<'_>>>()
        + exceptions * size_of::<ExceptionHandler>()
        + attributes * size_of::<Attribute<'_>>()
}
fn cuts(c: &ClassFile<'_>, exact: Limits) {
    assert!(decode(c, exact).is_ok());
    assert!(decode(c, unlimited()).is_ok());
    for (limits, label) in [
        (
            Limits {
                bytes: exact.bytes - 1,
                ..exact
            },
            "code bytes",
        ),
        (
            Limits {
                records: exact.records - 1,
                ..exact
            },
            "code records",
        ),
        (
            Limits {
                work: exact.work - 1,
                ..exact
            },
            "code work",
        ),
        (
            Limits {
                owned_bytes: exact.owned_bytes - 1,
                ..exact
            },
            "code owned bytes",
        ),
    ] {
        assert_eq!(decode(c, limits), Err(Error::Limit(label)), "{limits:?}");
    }
}
fn prefixes(raw: &[u8]) {
    for end in 0..raw.len() {
        assert_eq!(
            decode(&class(&raw[..end]), unlimited()),
            Err(Error::Truncated),
            "prefix {end}"
        );
    }
}

#[test]
fn authored_literals_borrow_order_and_exact_budget_cuts() {
    for (text, len, stack, locals, end, handlers, nested, records, work) in [
        (V01, 13, 1, 1, 9, vec![], vec![], 2, 22),
        (
            V02,
            46,
            2,
            3,
            12,
            vec![(0, 4, 3, 0), (1, 3, 2, 3)],
            vec![(2, 38, 40), (2, 46, 46)],
            6,
            64,
        ),
        (
            V03,
            27,
            65535,
            0,
            9,
            vec![],
            vec![(7, 19, 21), (7, 27, 27)],
            4,
            44,
        ),
        (V04, 20, 0, 0, 9, vec![], vec![(1, 19, 20)], 3, 35),
    ] {
        let raw = hex(text);
        assert_eq!(raw.len(), len);
        let c = class(&raw);
        let before = format!("{c:?}");
        let decoded = decode(&c, unlimited()).unwrap();
        let body = decoded[0].as_ref().unwrap();
        assert_eq!((body.max_stack, body.max_locals), (stack, locals));
        assert_eq!(body.raw, raw);
        assert_eq!(body.raw.as_ptr(), raw.as_ptr());
        assert_eq!(body.bytecode, &raw[8..end]);
        assert_eq!(body.bytecode.as_ptr(), raw[8..].as_ptr());
        assert_eq!(
            body.exceptions
                .iter()
                .map(|h| (h.start_pc, h.end_pc, h.handler_pc, h.catch_type))
                .collect::<Vec<_>>(),
            handlers
        );
        assert_eq!(body.attributes.len(), nested.len());
        for (attr, (name, start, end)) in body.attributes.iter().zip(&nested) {
            assert_eq!(attr.name, *name);
            assert_eq!(attr.bytes, &raw[*start..*end]);
            assert_eq!(attr.bytes.as_ptr(), raw[*start..].as_ptr());
        }
        assert_eq!(format!("{c:?}"), before);
        cuts(
            &c,
            Limits {
                bytes: len,
                records,
                work,
                owned_bytes: storage(1, handlers.len(), nested.len()),
            },
        );
        prefixes(&raw);
        let mut trailing = raw.clone();
        trailing.push(0);
        assert_eq!(
            decode(&class(&trailing), unlimited()),
            Err(Error::Malformed("code trailing bytes"))
        );
    }
}

#[test]
fn all_nineteen_frozen_malformed_patches() {
    for (id, base, offset, patch, expected) in [
        (
            "M01",
            V01,
            4,
            "00 00 00 00",
            Error::Malformed("code length"),
        ),
        (
            "M02",
            V01,
            4,
            "00 01 00 00",
            Error::Malformed("code length"),
        ),
        (
            "M03",
            V01,
            4,
            "ff ff ff ff",
            Error::Malformed("code length"),
        ),
        ("M04", V01, 4, "00 00 00 02", Error::Truncated),
        (
            "M05",
            V02,
            14,
            "00 04",
            Error::Malformed("code exception range"),
        ),
        (
            "M06",
            V02,
            16,
            "00 00",
            Error::Malformed("code exception range"),
        ),
        (
            "M07",
            V02,
            16,
            "00 05",
            Error::Malformed("code exception range"),
        ),
        (
            "M08",
            V02,
            18,
            "00 04",
            Error::Malformed("code exception range"),
        ),
        ("M09", V02, 20, "00 04", Error::Malformed("class index")),
        ("M10", V02, 20, "00 05", Error::Malformed("constant index")),
        ("M11", V02, 20, "ff ff", Error::Malformed("constant index")),
        ("M12", V02, 32, "00 00", Error::Malformed("constant index")),
        ("M13", V02, 32, "00 03", Error::Malformed("UTF8 index")),
        ("M14", V02, 32, "00 05", Error::Malformed("constant index")),
        ("M15", V02, 32, "ff ff", Error::Malformed("constant index")),
        ("M16", V02, 34, "ff ff ff ff", Error::Truncated),
        ("M17", V02, 30, "00 03", Error::Truncated),
        (
            "M18",
            V02,
            12,
            "00 03",
            Error::Malformed("code exception range"),
        ),
        ("M19", V01, 9, "ff ff", Error::Limit("code records")),
    ] {
        let mut raw = hex(base);
        let patch = hex(patch);
        raw[offset..offset + patch.len()].copy_from_slice(&patch);
        assert_eq!(decode(&class(&raw), unlimited()), Err(expected), "{id}");
    }
}

#[test]
fn genuine_unit_ordered_full_tuples_borrows_and_aggregate_121_11_270() {
    assert_eq!(UNIT.len(), 781);
    let c = parse_class(UNIT).unwrap();
    assert_eq!((c.major, c.minor), (52, 0));
    let before = format!("{c:?}");
    let decoded = decode(&c, unlimited()).unwrap();
    assert_eq!(decoded.len(), 3);
    for (i, name, flags, start, end, stack, locals, bytes, nested) in [
        (
            0,
            "<init>",
            2,
            549,
            596,
            1,
            1,
            "2a b7 00 08 b1",
            vec![
                (38, 23, 29, "00 01 00 00 00 0d"),
                (39, 35, 47, "00 01 00 00 00 05 00 09 00 0a 00 00"),
            ],
        ),
        (
            1,
            "toString",
            1,
            610,
            655,
            1,
            1,
            "12 0f b0",
            vec![
                (38, 21, 27, "00 01 00 00 00 0e"),
                (39, 33, 45, "00 01 00 00 00 03 00 09 00 0a 00 00"),
            ],
        ),
        (
            2,
            "<clinit>",
            8,
            681,
            704,
            2,
            0,
            "bb 00 02 59 b7 00 11 b3 00 14 b1",
            vec![],
        ),
    ] {
        assert_eq!(
            c.utf16(c.methods[i].name).unwrap(),
            name.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(c.methods[i].access_flags, flags);
        let body = decoded[i].as_ref().unwrap();
        assert_eq!((body.max_stack, body.max_locals), (stack, locals));
        assert_eq!(body.raw, &UNIT[start..end]);
        assert_eq!(body.raw.as_ptr(), UNIT[start..].as_ptr());
        assert_eq!(body.bytecode, hex(bytes));
        assert_eq!(body.bytecode.as_ptr(), UNIT[start + 8..].as_ptr());
        assert!(body.exceptions.is_empty());
        assert_eq!(body.attributes.len(), nested.len());
        for (attr, (index, a, b, expected)) in body.attributes.iter().zip(nested) {
            assert_eq!(attr.name, index);
            assert_eq!(attr.bytes, hex(expected));
            assert_eq!(attr.bytes, &body.raw[a..b]);
            assert_eq!(attr.bytes.as_ptr(), body.raw[a..].as_ptr());
        }
        for prefix in 0..body.raw.len() {
            let mut forged = parse_class(UNIT).unwrap();
            forged.methods[i].attributes[0].bytes = &body.raw[..prefix];
            assert_eq!(
                decode(&forged, unlimited()),
                Err(Error::Truncated),
                "method{i}, prefix{prefix}"
            );
        }
        let mut extra = body.raw.to_vec();
        extra.push(0);
        let mut forged = parse_class(UNIT).unwrap();
        forged.methods[i].attributes[0].bytes = &extra;
        assert_eq!(
            decode(&forged, unlimited()),
            Err(Error::Malformed("code trailing bytes"))
        );
    }
    assert_eq!(
        c.methods[1].attributes[1],
        Attribute {
            name: 36,
            bytes: &[0, 1, 0, 13, 0, 0]
        }
    );
    cuts(
        &c,
        Limits {
            bytes: 121,
            records: 11,
            work: 270,
            owned_bytes: storage(3, 0, 4),
        },
    );
    assert_eq!(format!("{c:?}"), before);
}

#[test]
fn method_disposition_clinit_exception_alignment_and_multiplicity() {
    let raw = hex(V01);
    for flags in [0, 8, 0x0100, 0x0400, 0x0500, 0xffff] {
        for name in ["m", "<init>", "<clinit>", "<clinit>suffix"] {
            let mut c = class(&raw);
            c.constants[8] = Constant::Utf8(name.encode_utf16().collect());
            c.methods[0].access_flags = flags;
            let required = name == "<clinit>" || flags & 0x0500 == 0;
            if required {
                assert!(decode(&c, unlimited()).unwrap()[0].is_some());
            } else {
                assert_eq!(
                    decode(&c, unlimited()),
                    Err(Error::Malformed("unexpected Code attribute"))
                );
                c.methods[0].attributes[0].bytes = &[];
                assert_eq!(
                    decode(&c, unlimited()),
                    Err(Error::Malformed("unexpected Code attribute"))
                );
            }
            c.methods[0].attributes.clear();
            if required {
                assert_eq!(
                    decode(&c, unlimited()),
                    Err(Error::Malformed("missing Code attribute"))
                );
            } else {
                assert_eq!(decode(&c, unlimited()).unwrap(), vec![None]);
            }
        }
    }
    let two = hex(V02);
    let three = hex(V03);
    let mut c = class(&raw);
    c.methods = vec![
        method(&raw),
        method(&two),
        method(&three),
        method(&raw),
        method(&raw),
        method(&raw),
    ];
    for (i, flags) in [(3, 0x0100), (4, 0x0400)] {
        c.methods[i].access_flags = flags;
        c.methods[i].attributes.clear();
    }
    let out = decode(&c, unlimited()).unwrap();
    assert_eq!(
        out.iter()
            .map(|x| x.as_ref().map(|b| b.raw.len()))
            .collect::<Vec<_>>(),
        vec![Some(13), Some(46), Some(27), None, None, Some(13)]
    );
    for name in [1, 6] {
        let mut c = class(&[]);
        c.methods[0]
            .attributes
            .push(Attribute { name, bytes: &raw });
        assert_eq!(
            decode(&c, unlimited()),
            Err(Error::Malformed("duplicate Code attribute"))
        );
    }
    let mut c = class(&[]);
    c.methods[0].attributes.push(Attribute {
        name: 0,
        bytes: &[],
    });
    assert_eq!(
        decode(&c, unlimited()),
        Err(Error::Malformed("constant index"))
    );
    c.methods[0].attributes[1].name = 2;
    c.methods[0].attributes[1].bytes = &[1];
    assert_eq!(
        decode(
            &c,
            Limits {
                bytes: 0,
                ..unlimited()
            }
        ),
        Err(Error::Limit("code bytes"))
    );
}

#[test]
fn version_cp_shape_and_consumed_reference_checks_on_forged_models() {
    let raw = hex(V01);
    for (major, minor, ok) in [
        (45, 0, true),
        (45, 1, true),
        (45, 2, true),
        (45, 3, true),
        (46, 0, true),
        (67, 0, true),
        (44, 0, false),
        (45, 4, false),
        (46, 1, false),
        (67, 65535, false),
        (68, 0, false),
    ] {
        let mut c = class(&raw);
        c.major = major;
        c.minor = minor;
        if ok {
            assert!(decode(&c, unlimited()).is_ok());
        } else {
            assert_eq!(
                decode(&c, unlimited()),
                Err(Error::UnsupportedVersion { major, minor })
            );
        }
    }
    let zero = Limits {
        bytes: 0,
        records: 0,
        work: 0,
        owned_bytes: 0,
    };
    let mut c = class(&[]);
    c.methods.clear();
    assert_eq!(decode(&c, zero).unwrap(), vec![]);
    c.major = 68;
    c.constants.clear();
    assert_eq!(
        decode(&c, zero),
        Err(Error::UnsupportedVersion {
            major: 68,
            minor: 0
        })
    );
    c.major = 52;
    assert_eq!(
        decode(&c, zero),
        Err(Error::Malformed("code constant pool size"))
    );
    c.constants = (0..65536).map(|_| Constant::Unusable).collect();
    assert_eq!(
        decode(&c, zero),
        Err(Error::Malformed("code constant pool size"))
    );
    for (index, expected) in [
        (0u16, "constant index"),
        (3, "UTF8 index"),
        (4, "UTF8 index"),
        (5, "constant index"),
        (65535, "constant index"),
    ] {
        for location in 0..3 {
            let mut nested = hex(V02);
            nested[32..34].copy_from_slice(&index.to_be_bytes());
            let mut c = class(if location == 2 { &nested } else { &raw });
            c.constants[0] = Constant::Utf8("Code".encode_utf16().collect());
            if location == 0 {
                c.methods[0].name = index;
            }
            if location == 1 {
                c.methods[0].attributes[0].name = index;
            }
            assert_eq!(
                decode(&c, unlimited()),
                Err(Error::Malformed(expected)),
                "location{location}, index{index}"
            );
        }
    }
    for index in [1u16, 2, 4, 5, 65535] {
        let mut nested = hex(V02);
        nested[20..22].copy_from_slice(&index.to_be_bytes());
        let expected = if [5, 65535].contains(&index) {
            "constant index"
        } else {
            "class index"
        };
        assert_eq!(
            decode(&class(&nested), unlimited()),
            Err(Error::Malformed(expected))
        );
        nested[16..18].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(
            decode(&class(&nested), unlimited()),
            Err(Error::Malformed("code exception range"))
        );
    }
    let mut c = class(&raw);
    c.methods[0].name = 0;
    assert_eq!(decode(&c, zero), Err(Error::Limit("code records")));
    assert_eq!(
        decode(&c, unlimited()),
        Err(Error::Malformed("constant index"))
    );
    let nested = hex("00 00 00 00 00 00 00 01 b1 00 00 00 01 00 00");
    assert_eq!(
        decode(&class(&nested), unlimited()),
        Err(Error::Malformed("constant index"))
    );
}

#[test]
fn aggregate_cross_method_aliases_and_ignored_payloads() {
    for (text, bytes, records, work, handlers, attrs) in
        [(V01, 26, 4, 44, 0, 0), (V02, 92, 12, 128, 4, 4)]
    {
        let raw = hex(text);
        let mut c = class(&raw);
        c.methods.push(method(&raw));
        cuts(
            &c,
            Limits {
                bytes,
                records,
                work,
                owned_bytes: storage(2, handlers, attrs),
            },
        );
    }
    let raw = hex(V01);
    let mut c = class(&raw);
    c.methods[0].attributes.push(Attribute {
        name: 2,
        bytes: &[0xaa, 0xbb, 0xcc],
    });
    cuts(
        &c,
        Limits {
            bytes: 16,
            records: 3,
            work: 28,
            owned_bytes: storage(1, 0, 0),
        },
    );
    c.methods[0].attributes.remove(0);
    c.methods[0].access_flags = 0x0100;
    cuts(
        &c,
        Limits {
            bytes: 3,
            records: 2,
            work: 9,
            owned_bytes: storage(1, 0, 0),
        },
    );
    assert_eq!(decode(&c, unlimited()).unwrap(), vec![None]);
    c.methods[0].access_flags = 0;
    assert_eq!(
        decode(&c, unlimited()),
        Err(Error::Malformed("missing Code attribute"))
    );
    for name in [2, 7] {
        let mut c = class(&raw);
        c.methods[0].attributes.extend([
            Attribute { name, bytes: &[] },
            Attribute { name, bytes: &[] },
        ]);
        assert!(decode(&c, unlimited()).is_ok());
    }
    let mut c = class(&raw);
    c.constants[6] = Constant::Utf8(vec![88]);
    c.methods[0].attributes.extend([
        Attribute {
            name: 2,
            bytes: &[],
        },
        Attribute {
            name: 6,
            bytes: &[],
        },
    ]);
    assert!(decode(&c, unlimited()).is_ok());
}

#[test]
fn count_reservations_and_text_work_precede_future_reads() {
    let raw = hex(V01);
    let valid = class(&raw);
    for (limits, label) in [
        (
            Limits {
                bytes: 0,
                ..unlimited()
            },
            "code bytes",
        ),
        (
            Limits {
                records: 0,
                ..unlimited()
            },
            "code records",
        ),
        (
            Limits {
                work: 0,
                ..unlimited()
            },
            "code work",
        ),
        (
            Limits {
                owned_bytes: 0,
                ..unlimited()
            },
            "code owned bytes",
        ),
    ] {
        assert_eq!(decode(&valid, limits), Err(Error::Limit(label)));
    }
    let c = class(&[]);
    for (limits, expected) in [
        (
            Limits {
                work: 8,
                ..unlimited()
            },
            Error::Limit("code work"),
        ),
        (
            Limits {
                work: 9,
                ..unlimited()
            },
            Error::Truncated,
        ),
        (
            Limits {
                bytes: 0,
                ..unlimited()
            },
            Error::Truncated,
        ),
        (
            Limits {
                records: 1,
                ..unlimited()
            },
            Error::Limit("code records"),
        ),
        (
            Limits {
                owned_bytes: storage(1, 0, 0) - 1,
                ..unlimited()
            },
            Error::Limit("code owned bytes"),
        ),
    ] {
        assert_eq!(decode(&c, limits), Err(expected));
    }
    let mut raw = hex(V01);
    raw[9..11].copy_from_slice(&1u16.to_be_bytes());
    let c = class(&raw);
    for (limits, expected) in [
        (
            Limits {
                records: 2,
                ..unlimited()
            },
            Error::Limit("code records"),
        ),
        (
            Limits {
                records: 3,
                owned_bytes: storage(1, 1, 0) - 1,
                ..unlimited()
            },
            Error::Limit("code owned bytes"),
        ),
        (unlimited(), Error::Truncated),
    ] {
        assert_eq!(decode(&c, limits), Err(expected));
    }
    raw = hex(V01);
    raw[11..13].copy_from_slice(&1u16.to_be_bytes());
    let c = class(&raw);
    assert_eq!(
        decode(
            &c,
            Limits {
                records: 2,
                ..unlimited()
            }
        ),
        Err(Error::Limit("code records"))
    );
    assert_eq!(
        decode(
            &c,
            Limits {
                owned_bytes: storage(1, 0, 1) - 1,
                ..unlimited()
            }
        ),
        Err(Error::Limit("code owned bytes"))
    );
    assert_eq!(decode(&c, unlimited()), Err(Error::Truncated));
    let mut c = class(&[]);
    c.constants[1] = Constant::Utf8(vec![88; 65535]);
    assert_eq!(
        decode(
            &c,
            Limits {
                work: 65539,
                ..unlimited()
            }
        ),
        Err(Error::Limit("code work"))
    );
    assert_eq!(
        decode(
            &c,
            Limits {
                work: 65540,
                ..unlimited()
            }
        ),
        Err(Error::Malformed("missing Code attribute"))
    );
}

fn handlers(count: u16, code_len: u32) -> Vec<u8> {
    let mut raw = vec![0, 0, 0, 0];
    raw.extend(code_len.to_be_bytes());
    raw.resize(8 + code_len as usize, 0);
    raw.extend(count.to_be_bytes());
    for _ in 0..count {
        raw.extend([0, 0, 0, 1, 0, 0, 0, 0]);
    }
    raw.extend([0, 0]);
    raw
}

#[test]
fn code_length_and_exception_range_boundaries_remain_data_only() {
    let raw = handlers(0, 65535);
    assert_eq!(raw.len(), 65547);
    cuts(
        &class(&raw),
        Limits {
            bytes: 65547,
            records: 2,
            work: 65556,
            owned_bytes: storage(1, 0, 0),
        },
    );
    let mut raw = handlers(1, 65535);
    let h = 8 + 65535 + 2;
    raw[h..h + 8].copy_from_slice(&[0, 0, 255, 255, 255, 254, 0, 0]);
    let c = class(&raw);
    let out = decode(&c, unlimited()).unwrap();
    assert_eq!(
        out[0].as_ref().unwrap().exceptions[0],
        ExceptionHandler {
            start_pc: 0,
            end_pc: 65535,
            handler_pc: 65534,
            catch_type: 0
        }
    );
    let raw = handlers(1, 1);
    assert!(decode(&class(&raw), unlimited()).is_ok());
    let mut raw = hex(V02);
    raw[14..22].copy_from_slice(&[0, 1, 0, 3, 0, 2, 0, 3]);
    raw[22..30].copy_from_slice(&[0, 0, 0, 4, 0, 3, 0, 0]);
    let c = class(&raw);
    let out = decode(&c, unlimited()).unwrap();
    assert_eq!(
        out[0]
            .as_ref()
            .unwrap()
            .exceptions
            .iter()
            .map(|h| h.catch_type)
            .collect::<Vec<_>>(),
        vec![3, 0]
    );
    raw[22..30].copy_from_slice(&[0, 1, 0, 3, 0, 2, 0, 3]);
    let c = class(&raw);
    let out = decode(&c, unlimited()).unwrap();
    assert_eq!(
        out[0].as_ref().unwrap().exceptions[0],
        out[0].as_ref().unwrap().exceptions[1]
    );
    let mut c = class(&raw);
    c.constants[3] = Constant::Class(65535);
    assert!(decode(&c, unlimited()).is_ok());
    let mut raw = hex(V02);
    raw[20..22].fill(0);
    raw[28..30].fill(0);
    let mut c = class(&raw);
    c.constants[0] = Constant::Integer(9);
    assert!(decode(&c, unlimited()).is_ok());
}

#[test]
fn opaque_stack_map_payloads_and_finite_mutations_never_panic() {
    let raw = hex(V03);
    let mut c = class(&raw);
    c.constants[7] = Constant::Utf8("StackMapTable".encode_utf16().collect());
    assert!(decode(&c, unlimited()).is_ok());
    for text in [V01, V02, V03, V04] {
        let raw = hex(text);
        for index in 0..raw.len() {
            for mask in [1, 128, 255] {
                let mut changed = raw.clone();
                changed[index] ^= mask;
                let _ = decode(&class(&changed), unlimited());
            }
        }
    }
    let raw = hex(V01);
    let mut c = class(&raw);
    c.methods = (0..MAX_RECORDS + 1).map(|_| method(&raw)).collect();
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code records")));
    c.methods = vec![method(&raw)];
    c.methods[0].attributes = (0..MAX_RECORDS)
        .map(|_| Attribute {
            name: 2,
            bytes: &[],
        })
        .collect();
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code records")));
}

#[test]
fn hard_record_byte_work_caps_cannot_be_raised_by_caller() {
    let raw = handlers(65533, 1);
    assert_eq!(raw.len(), 524277);
    cuts(
        &class(&raw),
        Limits {
            bytes: 524277,
            records: 65535,
            work: 589819,
            owned_bytes: storage(1, 65533, 0),
        },
    );
    assert_eq!(
        decode(&class(&handlers(65534, 1)), unlimited()),
        Err(Error::Limit("code records"))
    );
    let half = handlers(32766, 1);
    let mut c = class(&half);
    c.methods.push(method(&half));
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code records")));

    let raw = hex(V01);
    let large = vec![0; 16777204];
    let mut c = class(&raw);
    c.methods[0].attributes.push(Attribute {
        name: 2,
        bytes: &large[..16777203],
    });
    cuts(
        &c,
        Limits {
            bytes: MAX_BYTES,
            records: 3,
            work: 16777228,
            owned_bytes: storage(1, 0, 0),
        },
    );
    c.methods[0].attributes[1].bytes = &large;
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code bytes")));
    c.methods[0].attributes[1].bytes = &large[..8388595];
    let mut second = method(&raw);
    second.attributes.push(Attribute {
        name: 2,
        bytes: &large[..8388595],
    });
    c.methods.push(second);
    assert!(decode(&c, unlimited()).is_ok());
    c.methods[1].attributes[1].bytes = &large[..8388596];
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code bytes")));

    let mut c = class(&raw);
    c.constants[2] = Constant::Utf8(vec![88; 65535]);
    c.constants.push(Constant::Utf8(vec![89; 64489]));
    c.methods[0].attributes.extend((0..1023).map(|_| Attribute {
        name: 2,
        bytes: &[],
    }));
    c.methods[0].attributes.push(Attribute {
        name: 9,
        bytes: &[],
    });
    cuts(
        &c,
        Limits {
            bytes: 13,
            records: 1026,
            work: 67108864,
            owned_bytes: storage(1, 0, 0),
        },
    );
    c.constants[9] = Constant::Utf8(vec![89; 64490]);
    assert_eq!(decode(&c, unlimited()), Err(Error::Limit("code work")));
}
