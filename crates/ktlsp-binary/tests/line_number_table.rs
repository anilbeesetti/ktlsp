use ktlsp_binary::code::{self, CodeBody, ExceptionHandler};
use ktlsp_binary::line_number_table::{Limits, LineNumber, LineNumberTable, decode};
use ktlsp_binary::{
    Attribute, Constant, Error, MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, parse_class,
};

// Frozen before source: fixtures/line_number_table/authored.json and EXPECTATIONS.md.
const V01: &str = "0000";
const V02: &str = "00010000000d";
const V03: &str = "00050003ffff0000000000030001000300010001ffff";
const V04: &str = "0001fffeffff";
const V05: &str = "000100000000";
const UNIT: &[u8] = include_bytes!("../fixtures/t017c2/Unit.class");

fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn cp() -> Vec<Constant> {
    vec![
        Constant::Unusable,
        Constant::Utf8("LineNumberTable".encode_utf16().collect()),
        Constant::Utf8(vec![88]),
        Constant::Integer(0),
        Constant::Unusable,
        Constant::Utf8("LineNumberTable".encode_utf16().collect()),
        Constant::Utf8(vec![0, 0xd800]),
        Constant::Utf8("LineNumberTabl".encode_utf16().collect()),
    ]
}
fn body<'a>(bytecode: &'a [u8], attributes: Vec<Attribute<'a>>) -> CodeBody<'a> {
    CodeBody {
        raw: &[],
        max_stack: u16::MAX,
        max_locals: u16::MAX,
        bytecode,
        exceptions: vec![],
        attributes,
    }
}
fn single<'a>(bytecode: &'a [u8], name: u16, raw: &'a [u8]) -> CodeBody<'a> {
    body(bytecode, vec![Attribute { name, bytes: raw }])
}
fn unlimited() -> Limits {
    Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    }
}
fn storage(tables: usize, entries: usize) -> usize {
    tables * size_of::<LineNumberTable<'_>>() + entries * size_of::<LineNumber>()
}
fn tuples(t: &LineNumberTable<'_>) -> Vec<(u16, u16)> {
    t.entries
        .iter()
        .map(|e| (e.start_pc, e.line_number))
        .collect()
}
fn cuts(b: &CodeBody<'_>, constants: &[Constant], exact: Limits) {
    assert!(decode(b, constants, exact).is_ok());
    assert!(decode(b, constants, unlimited()).is_ok());
    for (used, label) in [
        (exact.bytes, "line number bytes"),
        (exact.records, "line number records"),
        (exact.work, "line number work"),
        (exact.owned_bytes, "line number owned bytes"),
    ] {
        if used == 0 {
            continue;
        }
        let mut lower = exact;
        match label {
            "line number bytes" => lower.bytes -= 1,
            "line number records" => lower.records -= 1,
            "line number work" => lower.work -= 1,
            _ => lower.owned_bytes -= 1,
        }
        assert_eq!(decode(b, constants, lower), Err(Error::Limit(label)));
    }
}
fn prefixes(bytecode: &[u8], name: u16, raw: &[u8], constants: &[Constant]) {
    for end in 0..raw.len() {
        assert_eq!(
            decode(&single(bytecode, name, &raw[..end]), constants, unlimited()),
            Err(Error::Truncated),
            "prefix {end} of {raw:?}"
        );
    }
    let mut tail = raw.to_vec();
    tail.push(0);
    assert_eq!(
        decode(&single(bytecode, name, &tail), constants, unlimited()),
        Err(Error::Malformed("line number trailing bytes"))
    );
}

#[test]
fn frozen_literals_order_borrows_prefixes_and_exact_cuts() {
    let constants = cp();
    for (raw, name, code_len, entries, records, work) in [
        (V01, 1, 1, vec![], 1, 19),
        (V02, 1, 5, vec![(0, 13)], 2, 24),
        (
            V03,
            1,
            4,
            vec![(3, 65535), (0, 0), (3, 1), (3, 1), (1, 65535)],
            6,
            44,
        ),
        (V04, 1, 65535, vec![(65534, 65535)], 2, 24),
        (V05, 5, 1, vec![(0, 0)], 2, 24),
    ] {
        let raw = hex(raw);
        let bytecode = vec![0xff; code_len];
        let b = single(&bytecode, name, &raw);
        let before = format!("{b:?}{constants:?}");
        let tables = decode(&b, &constants, unlimited()).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, name);
        assert_eq!(tables[0].raw, raw);
        assert_eq!(tables[0].raw.as_ptr(), raw.as_ptr());
        assert_eq!(tuples(&tables[0]), entries);
        assert_eq!(format!("{b:?}{constants:?}"), before);
        cuts(
            &b,
            &constants,
            Limits {
                bytes: raw.len(),
                records,
                work,
                owned_bytes: storage(1, entries.len()),
            },
        );
        prefixes(&bytecode, name, &raw, &constants);
    }
}

#[test]
fn multiple_tables_unknown_interleave_and_true_aliases_share_one_budget() {
    let constants = cp();
    let one = hex(V02);
    let empty = hex(V01);
    let many = hex(V03);
    let ignored = [0xaa, 0xbb];
    let b = body(
        &[0xff; 5],
        vec![
            Attribute {
                name: 1,
                bytes: &one,
            },
            Attribute {
                name: 2,
                bytes: &ignored,
            },
            Attribute {
                name: 5,
                bytes: &empty,
            },
            Attribute {
                name: 1,
                bytes: &many,
            },
        ],
    );
    let before = format!("{b:?}{constants:?}");
    let tables = decode(&b, &constants, unlimited()).unwrap();
    assert_eq!(tables.iter().map(|t| t.name).collect::<Vec<_>>(), [1, 5, 1]);
    assert_eq!(
        tables.iter().map(tuples).collect::<Vec<_>>(),
        [
            vec![(0, 13)],
            vec![],
            vec![(3, 65535), (0, 0), (3, 1), (3, 1), (1, 65535)]
        ]
    );
    for (t, raw) in tables
        .iter()
        .zip([one.as_slice(), empty.as_slice(), many.as_slice()])
    {
        assert_eq!(t.raw, raw);
        assert_eq!(t.raw.as_ptr(), raw.as_ptr());
    }
    cuts(
        &b,
        &constants,
        Limits {
            bytes: 32,
            records: 10,
            work: 92,
            owned_bytes: storage(3, 6),
        },
    );
    assert_eq!(format!("{b:?}{constants:?}"), before);
    let aliases = body(
        &[0xff; 5],
        vec![
            Attribute {
                name: 1,
                bytes: &one,
            },
            Attribute {
                name: 5,
                bytes: &one,
            },
        ],
    );
    let tables = decode(&aliases, &constants, unlimited()).unwrap();
    assert_eq!(tables.len(), 2);
    for (t, name) in tables.iter().zip([1, 5]) {
        assert_eq!(t.name, name);
        assert_eq!(t.raw.as_ptr(), one.as_ptr());
        assert_eq!(tuples(t), [(0, 13)]);
    }
    cuts(
        &aliases,
        &constants,
        Limits {
            bytes: 12,
            records: 4,
            work: 48,
            owned_bytes: storage(2, 2),
        },
    );
    for (name, raw, bytes, work) in [(6, &ignored[..], 2, 6), (7, &[][..], 0, 16)] {
        let b = single(&[0xff], name, raw);
        assert_eq!(decode(&b, &constants, unlimited()), Ok(vec![]));
        cuts(
            &b,
            &constants,
            Limits {
                bytes,
                records: 1,
                work,
                owned_bytes: 0,
            },
        );
    }
    cuts(
        &body(&[0xff], vec![]),
        &constants,
        Limits {
            bytes: 0,
            records: 0,
            work: 0,
            owned_bytes: 0,
        },
    );
}

#[test]
fn all_fourteen_frozen_malformed_payloads() {
    let constants = cp();
    for (raw, len, error) in [
        ("", 1, Error::Truncated),
        ("00", 1, Error::Truncated),
        ("0001", 1, Error::Truncated),
        ("000100", 1, Error::Truncated),
        ("00010001", 1, Error::Truncated),
        ("0001000000", 1, Error::Truncated),
        ("00010001000d", 1, Error::Malformed("line number start pc")),
        (
            "0001ffff0000",
            65535,
            Error::Malformed("line number start pc"),
        ),
        ("000000", 1, Error::Malformed("line number trailing bytes")),
        (
            "00010000000d00",
            1,
            Error::Malformed("line number trailing bytes"),
        ),
        (
            "00000000000d",
            1,
            Error::Malformed("line number trailing bytes"),
        ),
        ("00020000000d", 1, Error::Truncated),
        ("ffff", 1, Error::Limit("line number records")),
        ("00020001000d", 1, Error::Malformed("line number start pc")),
    ] {
        let raw = hex(raw);
        let code = vec![0xff; len];
        let b = single(&code, 1, &raw);
        let before = format!("{b:?}{constants:?}");
        assert_eq!(decode(&b, &constants, unlimited()), Err(error));
        assert_eq!(format!("{b:?}{constants:?}"), before);
    }
    let mut invalid_first = hex(V03);
    invalid_first[2..4].copy_from_slice(&4u16.to_be_bytes());
    invalid_first.push(0);
    assert_eq!(
        decode(
            &single(&[0xff; 4], 1, &invalid_first),
            &constants,
            unlimited()
        ),
        Err(Error::Malformed("line number start pc"))
    );
}

#[test]
fn genuine_unit_actual_class_code_lnt_pipeline_full_tuples_and_pointer_identity() {
    assert_eq!(UNIT.len(), 781);
    let class = parse_class(UNIT).unwrap();
    let before = format!("{class:?}");
    let bodies = code::decode(&class, code::Limits::default()).unwrap();
    assert_eq!(class.methods.len(), 3);
    assert_eq!(bodies.len(), 3);
    assert_eq!(
        class.utf16(38).unwrap(),
        "LineNumberTable".encode_utf16().collect::<Vec<_>>()
    );
    for (
        i,
        flags,
        name,
        descriptor,
        name_text,
        desc_text,
        start,
        end,
        stack,
        locals,
        cstart,
        cend,
        code_hex,
        raw_hex,
        nested,
        line,
    ) in [
        (
            0,
            2,
            5,
            6,
            "<init>",
            "()V",
            549,
            596,
            1,
            1,
            557,
            562,
            "2ab70008b1",
            "00010001000000052ab70008b10000000200260000000600010000000d00270000000c0001000000050009000a0000",
            vec![
                (38, 572, 578, "00010000000d"),
                (39, 584, 596, "0001000000050009000a0000"),
            ],
            Some(13),
        ),
        (
            1,
            1,
            11,
            12,
            "toString",
            "()Ljava/lang/String;",
            610,
            655,
            1,
            1,
            618,
            621,
            "120fb0",
            "0001000100000003120fb00000000200260000000600010000000e00270000000c0001000000030009000a0000",
            vec![
                (38, 631, 637, "00010000000e"),
                (39, 643, 655, "0001000000030009000a0000"),
            ],
            Some(14),
        ),
        (
            2,
            8,
            16,
            6,
            "<clinit>",
            "()V",
            681,
            704,
            2,
            0,
            689,
            700,
            "bb000259b70011b30014b1",
            "000200000000000bbb000259b70011b30014b100000000",
            vec![],
            None,
        ),
    ] {
        let m = &class.methods[i];
        assert_eq!(
            (m.access_flags, m.name, m.descriptor),
            (flags, name, descriptor)
        );
        assert_eq!(
            class.utf16(name).unwrap(),
            name_text.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            class.utf16(descriptor).unwrap(),
            desc_text.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(m.attributes.len(), if i == 1 { 2 } else { 1 });
        assert_eq!(m.attributes[0].name, 37);
        assert_eq!(m.attributes[0].bytes, &UNIT[start..end]);
        assert_eq!(m.attributes[0].bytes.as_ptr(), UNIT[start..].as_ptr());
        let b = bodies[i].as_ref().unwrap();
        assert_eq!((b.max_stack, b.max_locals), (stack, locals));
        assert_eq!(b.raw, hex(raw_hex));
        assert_eq!(b.raw, &UNIT[start..end]);
        assert_eq!(b.raw.as_ptr(), UNIT[start..].as_ptr());
        assert_eq!(b.bytecode, hex(code_hex));
        assert_eq!(b.bytecode, &UNIT[cstart..cend]);
        assert_eq!(b.bytecode.as_ptr(), UNIT[cstart..].as_ptr());
        assert!(b.exceptions.is_empty());
        assert_eq!(b.attributes.len(), nested.len());
        for (attr, (index, a, z, text)) in b.attributes.iter().zip(nested) {
            assert_eq!(attr.name, index);
            assert_eq!(attr.bytes, hex(text));
            assert_eq!(attr.bytes, &UNIT[a..z]);
            assert_eq!(attr.bytes.as_ptr(), UNIT[a..].as_ptr());
        }
        let body_before = format!("{b:?}");
        let tables = decode(b, &class.constants, unlimited()).unwrap();
        match line {
            Some(line) => {
                let (a, z) = if i == 0 { (572, 578) } else { (631, 637) };
                assert_eq!(tables.len(), 1);
                assert_eq!(tables[0].name, 38);
                assert_eq!(tables[0].raw, &UNIT[a..z]);
                assert_eq!(tables[0].raw.as_ptr(), UNIT[a..].as_ptr());
                assert_eq!(tuples(&tables[0]), [(0, line)]);
                cuts(
                    b,
                    &class.constants,
                    Limits {
                        bytes: 18,
                        records: 3,
                        work: 56,
                        owned_bytes: storage(1, 1),
                    },
                );
                prefixes(b.bytecode, 38, &UNIT[a..z], &class.constants);
            }
            None => {
                assert!(tables.is_empty());
                cuts(
                    b,
                    &class.constants,
                    Limits {
                        bytes: 0,
                        records: 0,
                        work: 0,
                        owned_bytes: 0,
                    },
                );
            }
        }
        assert_eq!(format!("{b:?}"), body_before);
    }
    assert_eq!(class.methods[1].attributes[1].name, 36);
    assert_eq!(class.methods[1].attributes[1].bytes, hex("0001000d0000"));
    assert_eq!(
        class.methods[1].attributes[1].bytes.as_ptr(),
        UNIT[661..].as_ptr()
    );
    assert_eq!(format!("{class:?}"), before);
}

#[test]
fn forged_models_prechecks_consumed_references_and_unvalidated_state() {
    let raw = hex(V02);
    let mut b = single(&[0xff], 1, &raw);
    for constants in [
        vec![],
        (0..=MAX_RECORDS).map(|_| Constant::Unusable).collect(),
    ] {
        assert_eq!(
            decode(&b, &constants, unlimited()),
            Err(Error::Malformed("line number constant pool size"))
        );
    }
    let constants = cp();
    for len in [0, 65536] {
        let code = vec![0; len];
        assert_eq!(
            decode(&body(&code, vec![]), &constants, unlimited()),
            Err(Error::Malformed("line number code length"))
        );
    }
    for len in [1, 65535] {
        let code = vec![0xff; len];
        cuts(
            &body(&code, vec![]),
            &constants,
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
        );
    }
    let bad_code = body(&[], vec![]);
    assert_eq!(
        decode(
            &bad_code,
            &[],
            Limits {
                records: 0,
                ..unlimited()
            }
        ),
        Err(Error::Malformed("line number constant pool size"))
    );
    let mut forged = cp();
    forged[0] = Constant::Utf8("LineNumberTable".encode_utf16().collect());
    forged.push(Constant::Class(0));
    b.raw = &[0xde, 0xad];
    b.exceptions.push(ExceptionHandler {
        start_pc: 65535,
        end_pc: 0,
        handler_pc: 65535,
        catch_type: 65535,
    });
    assert_eq!(
        tuples(&decode(&b, &forged, unlimited()).unwrap()[0]),
        [(0, 13)]
    );
    for (name, error) in [
        (0, "constant index"),
        (65535, "constant index"),
        (4, "constant index"),
        (3, "UTF8 index"),
    ] {
        assert_eq!(
            decode(&single(&[0xff], name, &raw), &forged, unlimited()),
            Err(Error::Malformed(error))
        );
    }
    for name in [
        vec![],
        vec![0],
        vec![0xd800],
        vec![0, 0xd800],
        "linenumbertable".encode_utf16().collect(),
        "LineNumberTable\0".encode_utf16().collect(),
    ] {
        let mut constants = cp();
        constants[1] = Constant::Utf8(name);
        assert_eq!(
            decode(&single(&[0xff], 1, &[0xff]), &constants, unlimited()),
            Ok(vec![])
        );
    }
    assert_eq!(
        decode(
            &body(&[0xff], vec![]),
            &[Constant::Utf8(vec![0xd800])],
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
                owned_bytes: 0
            }
        ),
        Ok(vec![])
    );
}

#[test]
fn first_attribute_failure_payload_name_and_entry_work_precedence() {
    let constants = cp();
    let raw = hex(V02);
    for (attrs, error) in [
        (
            vec![
                Attribute {
                    name: 1,
                    bytes: &[],
                },
                Attribute {
                    name: 3,
                    bytes: &raw,
                },
            ],
            Error::Truncated,
        ),
        (
            vec![
                Attribute {
                    name: 3,
                    bytes: &raw,
                },
                Attribute {
                    name: 1,
                    bytes: &[],
                },
            ],
            Error::Malformed("UTF8 index"),
        ),
        (
            vec![
                Attribute {
                    name: 1,
                    bytes: &raw,
                },
                Attribute {
                    name: 3,
                    bytes: &[],
                },
            ],
            Error::Malformed("UTF8 index"),
        ),
    ] {
        let b = body(&[0xff], attrs);
        assert_eq!(decode(&b, &constants, unlimited()), Err(error));
    }
    let b = single(&[0xff], 1, &raw);
    for work in [0, 6, 7, 8, 23] {
        assert_eq!(
            decode(
                &b,
                &constants,
                Limits {
                    work,
                    ..unlimited()
                }
            ),
            Err(Error::Limit("line number work"))
        );
    }
    assert!(
        decode(
            &b,
            &constants,
            Limits {
                work: 24,
                ..unlimited()
            }
        )
        .is_ok()
    );
    let b = single(&[0xff], 3, &raw);
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                records: 0,
                work: 0,
                bytes: 0,
                owned_bytes: 0
            }
        ),
        Err(Error::Limit("line number records"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                bytes: 5,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number bytes"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                work: 7,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number work"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                work: 8,
                ..unlimited()
            }
        ),
        Err(Error::Malformed("UTF8 index"))
    );
    // Full unknown name work is charged before a mismatching first code unit.
    let mut constants = cp();
    constants[1] = Constant::Utf8(vec![88; 100]);
    let b = single(&[0xff], 1, &[]);
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                work: 101,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number work"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                work: 102,
                ..unlimited()
            }
        ),
        Ok(vec![])
    );
}

#[test]
fn selected_table_and_count_storage_precede_missing_bytes() {
    let constants = cp();
    let s = storage(1, 0);
    let b = single(&[0xff], 1, &[]);
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                owned_bytes: s - 1,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number owned bytes"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                owned_bytes: s,
                ..unlimited()
            }
        ),
        Err(Error::Truncated)
    );
    let b = single(&[0xff], 1, &[0, 1]);
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                records: 1,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number records"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                records: 2,
                owned_bytes: storage(1, 1) - 1,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number owned bytes"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                records: 2,
                owned_bytes: storage(1, 1),
                work: 19,
                ..unlimited()
            }
        ),
        Err(Error::Limit("line number work"))
    );
    assert_eq!(
        decode(
            &b,
            &constants,
            Limits {
                records: 2,
                owned_bytes: storage(1, 1),
                work: 20,
                ..unlimited()
            }
        ),
        Err(Error::Truncated)
    );
}

#[test]
fn public_hard_record_bytes_and_name_work_caps_cannot_be_raised() {
    let constants = cp();
    let mut raw = vec![0; 2 + 4 * 65534];
    raw[..2].copy_from_slice(&65534u16.to_be_bytes());
    let b = single(&[0xff], 1, &raw);
    let tables = decode(&b, &constants, unlimited()).unwrap();
    assert_eq!(tables[0].entries.len(), 65534);
    assert!(
        tables[0]
            .entries
            .iter()
            .all(|e| e.start_pc == 0 && e.line_number == 0)
    );
    cuts(
        &b,
        &constants,
        Limits {
            bytes: 262138,
            records: 65535,
            work: 327689,
            owned_bytes: storage(1, 65534),
        },
    );
    raw[..2].copy_from_slice(&65535u16.to_be_bytes());
    assert_eq!(
        decode(&single(&[0xff], 1, &raw), &constants, unlimited()),
        Err(Error::Limit("line number records"))
    );
    let raw = vec![0; MAX_BYTES + 1];
    let b = single(&[0xff], 2, &raw[..MAX_BYTES]);
    cuts(
        &b,
        &constants,
        Limits {
            bytes: MAX_BYTES,
            records: 1,
            work: MAX_BYTES + 3,
            owned_bytes: 0,
        },
    );
    assert_eq!(
        decode(&single(&[0xff], 2, &raw), &constants, unlimited()),
        Err(Error::Limit("line number bytes"))
    );
    let mut constants = cp();
    constants[2] = Constant::Utf8(vec![88; 65534]);
    let b = body(
        &[0xff],
        (0..1024)
            .map(|_| Attribute {
                name: 2,
                bytes: &[],
            })
            .collect(),
    );
    cuts(
        &b,
        &constants,
        Limits {
            bytes: 0,
            records: 1024,
            work: MAX_VALIDATION_UNITS,
            owned_bytes: 0,
        },
    );
    constants[2] = Constant::Utf8(vec![88; 65535]);
    assert_eq!(
        decode(&b, &constants, unlimited()),
        Err(Error::Limit("line number work"))
    );
}

#[test]
fn hard_record_count_of_empty_alias_tables_and_amortized_output_growth() {
    let constants = cp();
    let b = body(
        &[0xff],
        (0..MAX_RECORDS)
            .map(|_| Attribute {
                name: 1,
                bytes: &[0, 0],
            })
            .collect(),
    );
    let tables = decode(&b, &constants, unlimited()).unwrap();
    assert_eq!(tables.len(), 65535);
    assert!(
        tables
            .iter()
            .all(|t| t.name == 1 && t.raw == [0, 0] && t.entries.is_empty())
    );
    cuts(
        &b,
        &constants,
        Limits {
            bytes: 131070,
            records: 65535,
            work: 1245165,
            owned_bytes: storage(65535, 0),
        },
    );
    let mut b = b;
    b.attributes.push(Attribute {
        name: 1,
        bytes: &[0, 0],
    });
    assert_eq!(
        decode(&b, &constants, unlimited()),
        Err(Error::Limit("line number records"))
    );
}

#[test]
fn exact_frozen_1041_hostile_payloads_in_six_limit_settings_never_panic() {
    let constants = cp();
    let mut payloads = Vec::new();
    for i in 0..=255u8 {
        payloads.push(vec![i]);
    }
    for i in 0..=255u8 {
        payloads.push(vec![i, i ^ 0xa5]);
    }
    for seed in [hex(V03), hex("00010000000d"), hex("00010000000e")] {
        for at in 0..seed.len() {
            for bit in 0..8 {
                let mut mutated = seed.clone();
                mutated[at] ^= 1 << bit;
                payloads.push(mutated);
            }
        }
    }
    let mut state = 0x4c4e5431u32;
    for len in 0..=256 {
        payloads.push(
            (0..len)
                .map(|_| {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    (state >> 24) as u8
                })
                .collect(),
        );
    }
    assert_eq!(payloads.len(), 1041);
    assert_eq!(payloads.iter().map(Vec::len).sum::<usize>(), 38112);
    assert_eq!(state, 0x066de8b1);
    let mut calls = 0;
    for raw in payloads {
        let b = single(&[0xff; 4], 1, &raw);
        for limits in [
            Limits::default(),
            Limits {
                bytes: 0,
                ..Limits::default()
            },
            Limits {
                records: 0,
                ..Limits::default()
            },
            Limits {
                work: 0,
                ..Limits::default()
            },
            Limits {
                owned_bytes: 0,
                ..Limits::default()
            },
            Limits {
                bytes: raw.len().min(8),
                records: 4,
                work: 32,
                owned_bytes: storage(1, 2),
            },
        ] {
            let _ = decode(&b, &constants, limits);
            calls += 1;
        }
    }
    assert_eq!(calls, 6246);
}
