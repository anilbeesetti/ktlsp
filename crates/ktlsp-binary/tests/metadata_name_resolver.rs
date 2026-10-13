use ktlsp_binary::metadata::{
    name_resolver::{Error, ErrorKind, Limits, Resolver},
    string_table::{self, Operation, Record, StringTableTypes},
    wire::{Field, Value},
};

fn hex(text: &str) -> Vec<u8> {
    let (pairs, remainder) = text.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
fn error(kind: ErrorKind, index: Option<i32>, record: Option<usize>) -> Error {
    Error {
        kind,
        index,
        record,
    }
}
fn record() -> Record<'static> {
    Record {
        raw: &[],
        span: 0..0,
        range: None,
        predefined_index: None,
        string: None,
        operation: None,
        substring_index: vec![],
        replace_char: vec![],
        unknown: vec![],
    }
}
fn table(records: Vec<Record<'static>>) -> StringTableTypes<'static> {
    StringTableTypes {
        raw: &[],
        records,
        local_name: vec![],
        unknown: vec![],
    }
}
fn lookup(raw: &str, d2: &[&[u16]], index: i32) -> Result<Vec<u16>, Error> {
    let bytes = hex(raw);
    let table = string_table::decode(&bytes, Default::default()).unwrap();
    Resolver::new(&table, d2, Limits::default())
        .unwrap()
        .get_string(index)
}

#[test]
fn frozen_ascii_byte_fixtures() {
    // F01–F12, F15–F26, F28–F29. Unicode/local rows are checked separately.
    let cases = [
        ("", vec![], 0, Err(ErrorKind::InvalidIndex)),
        ("0a00", vec!["ab"], 0, Ok("ab")),
        ("0a023200", vec!["fallback"], 0, Ok("")),
        ("0a04100e3200", vec![], 0, Ok("")),
        ("0a02100e", vec![], 0, Ok("kotlin/String")),
        (
            "0a02102b",
            vec![],
            0,
            Ok("kotlin/collections/MutableListIterator"),
        ),
        ("0a02102c", vec!["fallback"], 0, Ok("fallback")),
        ("0a00", vec![], 0, Err(ErrorKind::MissingString)),
        ("0a040802100e", vec![], 0, Ok("kotlin/String")),
        ("0a040802100e", vec![], 1, Ok("kotlin/String")),
        ("0a040802100e", vec![], 2, Err(ErrorKind::InvalidIndex)),
        ("0a020802", vec!["first", "second"], 1, Ok("second")),
        ("0a0208000a00", vec!["picked"], 0, Ok("picked")),
        ("0a0608ffffffff0f0a00", vec!["picked"], 0, Ok("picked")),
        ("0a0422020201", vec!["abc"], 0, Ok("abc")),
        ("0a0422020004", vec!["abc"], 0, Ok("abc")),
        ("0a03220101", vec!["abc"], 0, Ok("abc")),
        ("0a052203010263", vec!["abc"], 0, Ok("b")),
        ("0a042a02242e", vec!["a$b$"], 0, Ok("a.b.")),
        ("0a021801", vec!["p/A$B"], 0, Ok("p/A.B")),
        ("0a021802", vec!["xy"], 0, Ok("")),
        ("0a021802", vec!["x"], 0, Ok("x")),
        ("0a021802", vec![""], 0, Ok("")),
        ("0a0a1802220201062a027824", vec!["zLx$Y;z"], 0, Ok("..Y")),
        ("0a0418011863", vec!["A$B"], 0, Ok("A.B")),
        ("0a021863", vec!["A$B"], 0, Ok("A$B")),
        (
            "0a033201ff",
            vec!["ignored"],
            0,
            Err(ErrorKind::UnsupportedUtf8),
        ),
        ("0a0508003201ff0a00", vec!["ok"], 0, Ok("ok")),
    ];
    for (raw, strings, index, expected) in cases {
        let owned: Vec<_> = strings.iter().map(|s| units(s)).collect();
        let d2: Vec<_> = owned.iter().map(Vec::as_slice).collect();
        assert_eq!(
            lookup(raw, &d2, index).map_err(|e| e.kind),
            expected.map(units),
            "{raw} index{index}"
        );
    }
}

#[test]
fn frozen_utf16_and_local_byte_fixtures() {
    let astral = [0x61, 0xd83d, 0xde00, 0x62];
    assert_eq!(
        lookup("0a0422020103", &[&astral], 0).unwrap(),
        [0xd83d, 0xde00]
    );
    assert_eq!(lookup("0a0422020102", &[&astral], 0).unwrap(), [0xd83d]);
    assert_eq!(
        lookup("0a063204f09f9880", &[], 0).unwrap(),
        [0xd83d, 0xde00]
    );
    let raw_units = [0, 0xd800, 0xdc00, 0xffff];
    assert_eq!(lookup("0a00", &[&raw_units], 0).unwrap(), raw_units);
    assert_eq!(
        lookup("0a021802", &[&[0xd83d, 0xde00, 0x61]], 0).unwrap(),
        [0xde00]
    );
    let bytes = hex("0a002801280128ffffffff0f");
    let table = string_table::decode(&bytes, Default::default()).unwrap();
    let name = units("name");
    let d2 = [name.as_slice()];
    let mut resolver = Resolver::new(&table, &d2, Limits::default()).unwrap();
    assert!(resolver.is_local_class_name(1).unwrap());
    assert!(resolver.is_local_class_name(-1).unwrap());
    assert!(!resolver.is_local_class_name(2).unwrap());
    assert_eq!(resolver.get_string(0).unwrap(), name);
}

#[test]
fn all_predefined_values_and_presence() {
    let expected = include_str!("../fixtures/metadata_name_resolver/EXPECTATIONS.md")
        .split("Indices0..43, one present predefined_index per typed-record case, d2 empty:")
        .nth(1)
        .unwrap();
    for (i, line) in expected.lines().filter(|s| !s.is_empty()).enumerate() {
        let (index, text) = line.split_once(' ').unwrap();
        assert_eq!(index.parse::<usize>().unwrap(), i);
        let mut r = record();
        r.predefined_index = Some(i as i32);
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            units(text)
        );
    }
    for predefined in [None, Some(-1), Some(44)] {
        let mut r = record();
        r.predefined_index = predefined;
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[&[0xd800]], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            [0xd800]
        );
    }
}

#[test]
fn typed_transformations_and_opaque_public_fields() {
    let mut r = record();
    r.replace_char = vec![-1, 65536, 55];
    r.raw = b"not wire";
    r.span = std::ops::Range {
        start: usize::MAX,
        end: 0,
    };
    r.unknown.push(Field {
        number: 0,
        span: 0..usize::MAX,
        raw: b"opaque",
        value: Value::Bytes(b"\xff"),
    });
    let t = table(vec![r]);
    assert_eq!(
        Resolver::new(&t, &[&[0xffff, 0xffff, 0]], Limits::default())
            .unwrap()
            .get_string(0)
            .unwrap(),
        [0, 0, 0]
    );
    for substring in [vec![-1, 1], vec![0], vec![2, 1], vec![0, 4]] {
        let mut r = record();
        r.substring_index = substring;
        r.replace_char = vec![97];
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[&[97, 98, 99]], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            [97, 98, 99]
        );
    }
    for substring in [vec![0, 0], vec![3, 3], vec![1, 1, i32::MAX]] {
        let mut r = record();
        r.substring_index = substring;
        let t = table(vec![r]);
        assert!(
            Resolver::new(&t, &[&[97, 98, 99]], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap()
                .is_empty()
        );
    }
    for (replace, expected) in [(vec![36, 120], 120), (vec![120, 36], 46)] {
        let input = if replace[0] == 36 { 36 } else { 120 };
        let mut r = record();
        r.operation = Some(Operation::InternalToClassId);
        r.replace_char = replace;
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[&[input]], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            [expected]
        );
    }
    for explicit in [&b"\xed\xa0\x80"[..], &b"\xc0\x80"[..], &b"\xff"[..]] {
        let mut r = record();
        r.string = Some(explicit);
        let t = table(vec![record(), r]);
        let d2 = [&[0xd800][..]];
        let mut resolver = Resolver::new(&t, &d2, Limits::default()).unwrap();
        assert_eq!(resolver.get_string(0).unwrap(), [0xd800]);
        assert_eq!(
            resolver.get_string(1),
            Err(error(ErrorKind::UnsupportedUtf8, Some(1), Some(1)))
        );
    }
    for explicit in [&b""[..], &b"\0"[..]] {
        let mut r = record();
        r.string = Some(explicit);
        r.predefined_index = Some(-1);
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            explicit.iter().copied().map(u16::from).collect::<Vec<_>>()
        );
    }
    for operation in [None, Some(Operation::None)] {
        let mut r = record();
        r.operation = operation;
        let t = table(vec![r]);
        assert_eq!(
            Resolver::new(&t, &[&[36]], Limits::default())
                .unwrap()
                .get_string(0)
                .unwrap(),
            [36]
        );
    }
    let mut r = record();
    r.range = Some(i32::MIN);
    let t = table(vec![r]);
    assert_eq!(
        Resolver::new(&t, &[&[1], &[2]], Limits::default())
            .unwrap()
            .get_string(0),
        Err(error(ErrorKind::InvalidIndex, Some(0), None))
    );
}

#[test]
fn constructor_limits_and_error_order() {
    let empty = table(vec![]);
    let zero = Limits {
        entries: 0,
        input_bytes: 0,
        range_slots: 0,
        queries: 0,
        work: 0,
        output_units: 0,
    };
    let mut resolver = Resolver::new(&empty, &[], zero).unwrap();
    assert_eq!(
        resolver.get_string(0),
        Err(error(ErrorKind::Limit("queries"), Some(0), None))
    );
    let mut resolver = Resolver::new(
        &empty,
        &[],
        Limits {
            queries: 1,
            work: 1,
            ..zero
        },
    )
    .unwrap();
    assert_eq!(
        resolver.get_string(0),
        Err(error(ErrorKind::InvalidIndex, Some(0), None))
    );
    let t = table(vec![record()]);
    let d2 = [&[97, 98][..]];
    let exact = Limits {
        entries: 2,
        input_bytes: 4,
        range_slots: 1,
        work: 6,
        ..Limits::default()
    };
    Resolver::new(&t, &d2, exact).unwrap();
    for (limits, label, rec) in [
        (
            Limits {
                entries: 1,
                ..exact
            },
            "entries",
            None,
        ),
        (
            Limits {
                input_bytes: 3,
                ..exact
            },
            "input_bytes",
            None,
        ),
        (Limits { work: 5, ..exact }, "work", None),
        (
            Limits {
                range_slots: 0,
                ..exact
            },
            "range_slots",
            Some(0),
        ),
    ] {
        assert_eq!(
            Resolver::new(&t, &d2, limits).unwrap_err(),
            error(ErrorKind::Limit(label), None, rec)
        );
    }
    let mut huge = record();
    huge.range = Some(i32::MAX);
    assert_eq!(
        Resolver::new(
            &table(vec![huge]),
            &[],
            Limits {
                range_slots: usize::MAX,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        error(ErrorKind::Limit("range_slots"), None, Some(0))
    );
    let t = table((0..65536).map(|_| record()).collect());
    assert_eq!(
        Resolver::new(
            &t,
            &[],
            Limits {
                entries: usize::MAX,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::Limit("entries")
    );
    let mut huge = record();
    huge.substring_index = vec![0; 65535];
    assert_eq!(
        Resolver::new(&table(vec![huge]), &[], Limits::default())
            .unwrap_err()
            .kind,
        ErrorKind::Limit("entries")
    );
    let mut r = record();
    r.range = Some(0);
    r.string = Some(b"abcd");
    r.replace_char = vec![1, 2, 3];
    let mut t = table(vec![r]);
    t.local_name = vec![9, 9];
    assert_eq!(
        Resolver::new(
            &t,
            &[],
            Limits {
                entries: 5,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::Limit("entries")
    );
    assert_eq!(
        Resolver::new(
            &t,
            &[],
            Limits {
                input_bytes: 3,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .kind,
        ErrorKind::Limit("input_bytes")
    );
}

#[test]
fn exact_lifetime_lookup_budgets_and_failed_debits() {
    let t = table(vec![record()]);
    let d2 = [&[97, 98][..]];
    let exact = Limits {
        work: 14,
        queries: 1,
        output_units: 2,
        ..Limits::default()
    };
    assert_eq!(
        Resolver::new(&t, &d2, exact)
            .unwrap()
            .get_string(0)
            .unwrap(),
        [97, 98]
    );
    for limits in [
        Limits { work: 13, ..exact },
        Limits {
            output_units: 1,
            ..exact
        },
    ] {
        let label = if limits.work == 13 {
            "work"
        } else {
            "output_units"
        };
        assert_eq!(
            Resolver::new(&t, &d2, limits).unwrap().get_string(0),
            Err(error(ErrorKind::Limit(label), Some(0), Some(0)))
        );
    }
    let mut resolver = Resolver::new(
        &t,
        &d2,
        Limits {
            work: 17,
            queries: 3,
            output_units: 2,
            ..exact
        },
    )
    .unwrap();
    assert_eq!(
        resolver.get_string(-1),
        Err(error(ErrorKind::InvalidIndex, Some(-1), None))
    );
    assert_eq!(resolver.get_string(0).unwrap(), [97, 98]);
    assert_eq!(
        resolver.get_string(1),
        Err(error(ErrorKind::InvalidIndex, Some(1), None))
    );
    assert_eq!(
        resolver.get_string(-1),
        Err(error(ErrorKind::Limit("queries"), Some(-1), None))
    );
    let mut resolver = Resolver::new(
        &t,
        &d2,
        Limits {
            work: 17,
            queries: 2,
            ..exact
        },
    )
    .unwrap();
    resolver.get_string(-1).unwrap_err();
    resolver.get_string(0).unwrap();
    assert_eq!(
        resolver.get_string(1),
        Err(error(ErrorKind::Limit("queries"), Some(1), None))
    );
    for work in [2, 3] {
        let expected = if work == 2 {
            error(ErrorKind::Limit("work"), Some(0), None)
        } else {
            error(ErrorKind::MissingString, Some(0), Some(0))
        };
        assert_eq!(
            Resolver::new(
                &t,
                &[],
                Limits {
                    work,
                    queries: 1,
                    ..exact
                }
            )
            .unwrap()
            .get_string(0),
            Err(expected)
        );
    }
    let mut r = record();
    r.string = Some(b"\xff");
    let bad = table(vec![r]);
    for (work, expected) in [
        (4, ErrorKind::Limit("work")),
        (5, ErrorKind::UnsupportedUtf8),
    ] {
        assert_eq!(
            Resolver::new(
                &bad,
                &[],
                Limits {
                    work,
                    queries: 1,
                    output_units: 0,
                    ..exact
                }
            )
            .unwrap()
            .get_string(0),
            Err(error(expected, Some(0), Some(0)))
        );
    }
    let mut resolver = Resolver::new(
        &bad,
        &[],
        Limits {
            work: 8,
            queries: 2,
            ..exact
        },
    )
    .unwrap();
    for _ in 0..2 {
        assert_eq!(
            resolver.get_string(0),
            Err(error(ErrorKind::UnsupportedUtf8, Some(0), Some(0)))
        );
    }
    assert_eq!(
        resolver.is_local_class_name(0),
        Err(error(ErrorKind::Limit("queries"), Some(0), None))
    );
    let mut r = record();
    r.substring_index = vec![0, 0];
    assert!(
        Resolver::new(
            &table(vec![r]),
            &d2,
            Limits {
                work: 12,
                output_units: 0,
                ..exact
            }
        )
        .unwrap()
        .get_string(0)
        .unwrap()
        .is_empty()
    );
    let two = table(vec![record(), record()]);
    let d2 = [&[97, 98][..], &[99][..]];
    let mut resolver = Resolver::new(
        &two,
        &d2,
        Limits {
            work: 20,
            queries: 2,
            output_units: 1,
            ..exact
        },
    )
    .unwrap();
    assert_eq!(
        resolver.get_string(0),
        Err(error(ErrorKind::Limit("output_units"), Some(0), Some(0)))
    );
    assert_eq!(resolver.get_string(1).unwrap(), [99]);
    let mut t = table(vec![record()]);
    t.local_name = vec![1, 1, -1];
    let d2 = [&[110, 97, 109, 101][..]];
    let mut resolver = Resolver::new(
        &t,
        &d2,
        Limits {
            work: 23,
            queries: 3,
            ..exact
        },
    )
    .unwrap();
    assert!(resolver.is_local_class_name(1).unwrap());
    assert!(resolver.is_local_class_name(-1).unwrap());
    assert!(!resolver.is_local_class_name(2).unwrap());
}

#[test]
fn bounded_deterministic_public_record_probes() {
    for range in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        for operation in [
            None,
            Some(Operation::None),
            Some(Operation::InternalToClassId),
            Some(Operation::DescToClassId),
        ] {
            for pair in [
                [-1, 0],
                [0, 0],
                [0, 1],
                [1, 2],
                [2, 1],
                [i32::MAX, i32::MAX],
            ] {
                let mut r = record();
                r.range = Some(range);
                r.operation = operation;
                r.substring_index = pair.to_vec();
                r.replace_char = pair.to_vec();
                let t = table(vec![r]);
                let d2 = [&[0xd800, 36][..]];
                let result = Resolver::new(
                    &t,
                    &d2,
                    Limits {
                        range_slots: 2,
                        queries: 8,
                        work: 128,
                        output_units: 8,
                        ..Limits::default()
                    },
                );
                if let Ok(mut resolver) = result {
                    for index in [-1, 0, 1, 2] {
                        if let Ok(name) = resolver.get_string(index) {
                            assert!(name.len() <= 2);
                        }
                        resolver.is_local_class_name(index).unwrap();
                    }
                } else {
                    assert_eq!(range, i32::MAX);
                }
            }
        }
    }
}

#[test]
fn valid_explicit_utf8_pass_budgets_and_surrogate_transforms() {
    let mut r = record();
    r.string = Some("a😀b".as_bytes());
    r.substring_index = vec![1, 2];
    let mut t = table(vec![r]);
    // Constructor3+6; lookup1+1+6+6+4+2 = total29, selecting one UTF-16 unit.
    for (work, expected) in [
        (29, Ok(vec![0xd83d])),
        (28, Err(error(ErrorKind::Limit("work"), Some(0), Some(0)))),
    ] {
        let limits = Limits {
            work,
            output_units: 1,
            queries: 1,
            ..Limits::default()
        };
        assert_eq!(
            Resolver::new(&t, &[], limits).unwrap().get_string(0),
            expected
        );
    }
    t.records[0].replace_char = vec![0xd83d, 0xdc01];
    assert_eq!(
        Resolver::new(&t, &[], Limits::default())
            .unwrap()
            .get_string(0)
            .unwrap(),
        [0xdc01]
    );
}

#[test]
fn output_debit_survives_a_later_work_failure() {
    let t = table(vec![record(), record()]);
    let d2 = [&[97, 98, 99, 100][..], &[99][..]];
    let limits = Limits {
        work: 27,
        queries: 2,
        output_units: 4,
        ..Limits::default()
    };
    let mut resolver = Resolver::new(&t, &d2, limits).unwrap();
    // Constructor14; first lookup reaches work20, debits output4, then work8 fails.
    assert_eq!(
        resolver.get_string(0),
        Err(error(ErrorKind::Limit("work"), Some(0), Some(0)))
    );
    // Second lookup reaches work24. Refunding output would incorrectly let it succeed.
    assert_eq!(
        resolver.get_string(1),
        Err(error(ErrorKind::Limit("output_units"), Some(1), Some(1)))
    );
}
