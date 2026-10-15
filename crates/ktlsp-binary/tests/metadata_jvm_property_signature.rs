use ktlsp_binary::{
    MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS,
    metadata::{
        self,
        jvm_property_signature::{
            Error, ErrorKind, Kind, Limits, MemberSignatureBody, PropertySignatureExtension,
            SignatureBody, decode,
        },
        name_resolver, payload, property_header, wire,
    },
};
use std::mem::size_of;

const B: usize = size_of::<SignatureBody<'_>>();
const M: usize = size_of::<MemberSignatureBody<'_>>();
type Pair = (Option<i32>, Option<i32>);
type Signature = Option<[Option<Pair>; 5]>;
type Span = (usize, usize);
type Member = (Kind, usize, Span, Span);
struct Case {
    id: &'static str,
    raw: &'static str,
    signature: Signature,
    outer: &'static [Span],
    members: &'static [Member],
    records: usize,
    work: usize,
    owned: usize,
}
impl Case {
    fn limits(&self) -> Limits {
        Limits {
            bytes: hex(self.raw).len(),
            records: self.records,
            work: self.work,
            owned_bytes: self.owned,
        }
    }
}
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn signature(out: &PropertySignatureExtension<'_>) -> Signature {
    out.signature.as_ref().map(|s| {
        [
            s.field.as_ref().map(|s| (s.name, s.desc)),
            s.synthetic_method.as_ref().map(|s| (s.name, s.desc)),
            s.getter.as_ref().map(|s| (s.name, s.desc)),
            s.setter.as_ref().map(|s| (s.name, s.desc)),
            s.delegate_method.as_ref().map(|s| (s.name, s.desc)),
        ]
    })
}
fn check(
    out: &PropertySignatureExtension<'_>,
    raw: &[u8],
    sig: Signature,
    outer: &[Span],
    members: &[Member],
) {
    assert_eq!(out.raw, raw);
    assert_eq!(out.raw.as_ptr(), raw.as_ptr());
    assert_eq!(signature(out), sig);
    assert_eq!(out.bodies.len(), outer.len());
    for (actual, &(start, end)) in out.bodies.iter().zip(outer) {
        assert_eq!(actual.span, start..end);
        assert_eq!(actual.raw, &raw[start..end]);
        assert_eq!(actual.raw.as_ptr(), raw[start..end].as_ptr());
    }
    assert_eq!(out.member_bodies.len(), members.len());
    for (actual, &(kind, ordinal, field, (start, end))) in out.member_bodies.iter().zip(members) {
        assert_eq!((actual.kind, actual.outer_ordinal), (kind, ordinal));
        assert_eq!(actual.body.span, start..end);
        assert_eq!(actual.body.raw, &raw[start..end]);
        assert_eq!(actual.body.raw.as_ptr(), raw[start..end].as_ptr());
        assert!(
            outer[ordinal].0 <= field.0
                && field.0 <= start
                && end == field.1
                && end <= outer[ordinal].1
        );
    }
}
fn case(id: &str) -> &'static Case {
    CASES.iter().find(|c| c.id == id).unwrap()
}
fn error(raw: &[u8], limits: Limits, kind: ErrorKind, offset: usize) {
    assert_eq!(decode(raw, limits), Err(Error { kind, offset }));
}

#[test]
fn literal_presence_merges_borrowing_and_exact_limits() {
    assert_eq!(CASES.len(), 47);
    for c in CASES {
        let raw = hex(c.raw);
        check(
            &decode(&raw, c.limits()).unwrap(),
            &raw,
            c.signature,
            c.outer,
            c.members,
        );
        check(
            &decode(
                &raw,
                Limits {
                    bytes: usize::MAX,
                    records: usize::MAX,
                    work: usize::MAX,
                    owned_bytes: usize::MAX,
                },
            )
            .unwrap(),
            &raw,
            c.signature,
            c.outer,
            c.members,
        );
    }
    check(
        &decode(
            &[],
            Limits {
                bytes: 0,
                records: 0,
                work: 0,
                owned_bytes: 0,
            },
        )
        .unwrap(),
        &[],
        None,
        &[],
        &[],
    );
}

#[test]
fn every_prefix_confined_to_original_parent() {
    for c in CASES {
        let raw = hex(c.raw);
        let boundaries: &[usize] = match c.id {
            "V05" => &[7, 14, 19],
            "V22" => &[2],
            "V23" => &[3],
            "V24" | "V31" => &[7],
            "V27" => &[3, 6],
            "V28" => &[5, 8],
            "V33" => &[23],
            "V34" => &[9, 16],
            "P_first" | "P_second" => &[2, 6, 10],
            id if id.starts_with("C_") => &[7, 14],
            _ => &[],
        };
        for cut in 0..=raw.len() {
            if cut == 0 || cut == raw.len() || boundaries.contains(&cut) {
                let mut sig = c.signature;
                if cut == 0 {
                    sig = None;
                } else if cut != raw.len() {
                    match c.id {
                        "V05" if cut == 7 => sig.as_mut().unwrap()[2] = Some((Some(7), None)),
                        "V22" | "V23" | "P_first" | "P_second" => sig = None,
                        "V28" => sig.as_mut().unwrap()[0] = None,
                        "V33" => {
                            for slot in sig.as_mut().unwrap() {
                                slot.as_mut().unwrap().1 = None;
                            }
                        }
                        "V34" => {
                            sig.as_mut().unwrap()[2] =
                                Some((Some(if cut == 9 { 7 } else { 9 }), Some(8)))
                        }
                        id if id.starts_with("C_") && cut == 7 => {
                            for slot in sig.as_mut().unwrap().iter_mut().flatten() {
                                slot.1 = None;
                            }
                        }
                        _ => {}
                    }
                }
                let outer: Vec<_> = c.outer.iter().copied().filter(|s| s.1 <= cut).collect();
                let members: Vec<_> = c
                    .members
                    .iter()
                    .copied()
                    .filter(|s| s.1 < outer.len())
                    .collect();
                check(
                    &decode(&raw[..cut], Default::default()).unwrap(),
                    &raw[..cut],
                    sig,
                    &outer,
                    &members,
                );
            } else {
                error(&raw[..cut], Default::default(), ErrorKind::Truncated, cut);
            }
        }
    }
}

#[test]
fn malformed_selected_children_and_offsets() {
    assert_eq!(MALFORMED.len(), 24);
    for &(raw, kind, offset) in MALFORMED {
        error(&hex(raw), Default::default(), kind, offset);
    }
}

static CASES: &[Case] = &[
    Case {
        id: "V00",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V00.hex"),
        signature: None,
        outer: &[],
        members: &[],
        records: 0,
        work: 0,
        owned: 0,
    },
    Case {
        id: "V01",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V01.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(3, 3)],
        members: &[],
        records: 1,
        work: 5 + B,
        owned: B,
    },
    Case {
        id: "V02",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V02.hex"),
        signature: Some([Some((None, None)), None, None, None, None]),
        outer: &[(3, 5)],
        members: &[(Kind::Field, 0, (3, 5), (5, 5))],
        records: 2,
        work: 11 + B + M,
        owned: B + M,
    },
    Case {
        id: "V03",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V03.hex"),
        signature: Some([
            Some((Some(1), Some(2))),
            Some((Some(3), Some(4))),
            Some((Some(5), Some(6))),
            Some((Some(7), Some(8))),
            Some((Some(9), Some(10))),
        ]),
        outer: &[(3, 33)],
        members: &[
            (Kind::Field, 0, (3, 9), (5, 9)),
            (Kind::SyntheticMethod, 0, (9, 15), (11, 15)),
            (Kind::Getter, 0, (15, 21), (17, 21)),
            (Kind::Setter, 0, (21, 27), (23, 27)),
            (Kind::DelegateMethod, 0, (27, 33), (29, 33)),
        ],
        records: 16,
        work: 115 + B + 8 * M,
        owned: B + 8 * M,
    },
    Case {
        id: "V04",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V04.hex"),
        signature: Some([None, None, Some((Some(7), Some(8))), None, None]),
        outer: &[(3, 11)],
        members: &[
            (Kind::Getter, 0, (3, 7), (5, 7)),
            (Kind::Getter, 0, (7, 11), (9, 11)),
        ],
        records: 5,
        work: 33 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "V05",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V05.hex"),
        signature: Some([None, None, Some((Some(7), Some(8))), None, None]),
        outer: &[(3, 7), (10, 14), (17, 19), (22, 22)],
        members: &[
            (Kind::Getter, 0, (3, 7), (5, 7)),
            (Kind::Getter, 1, (10, 14), (12, 14)),
            (Kind::Getter, 2, (17, 19), (19, 19)),
        ],
        records: 9,
        work: 54 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "V06",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V06.hex"),
        signature: Some([Some((Some(1), Some(2))), None, None, None, None]),
        outer: &[(3, 11)],
        members: &[
            (Kind::Field, 0, (3, 7), (5, 7)),
            (Kind::Field, 0, (7, 11), (9, 11)),
        ],
        records: 5,
        work: 33 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "V07",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V07.hex"),
        signature: Some([Some((Some(2), Some(3))), None, None, None, None]),
        outer: &[(3, 11)],
        members: &[(Kind::Field, 0, (3, 11), (5, 11))],
        records: 5,
        work: 35 + B + M,
        owned: B + M,
    },
    Case {
        id: "V08",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V08.hex"),
        signature: Some([None, None, Some((Some(-1), None)), None, None]),
        outer: &[(3, 11)],
        members: &[(Kind::Getter, 0, (3, 11), (5, 11))],
        records: 3,
        work: 31 + B + M,
        owned: B + M,
    },
    Case {
        id: "V09",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V09.hex"),
        signature: Some([Some((None, Some(-2147483648))), None, None, None, None]),
        outer: &[(3, 11)],
        members: &[(Kind::Field, 0, (3, 11), (5, 11))],
        records: 3,
        work: 31 + B + M,
        owned: B + M,
    },
    Case {
        id: "V10",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V10.hex"),
        signature: Some([None, None, None, Some((Some(0), None)), None]),
        outer: &[(3, 11)],
        members: &[(Kind::Setter, 0, (3, 11), (5, 11))],
        records: 3,
        work: 31 + B + M,
        owned: B + M,
    },
    Case {
        id: "V11",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V11.hex"),
        signature: Some([None, None, None, None, Some((Some(-1), None))]),
        outer: &[(3, 16)],
        members: &[(Kind::DelegateMethod, 0, (3, 16), (5, 16))],
        records: 3,
        work: 46 + B + M,
        owned: B + M,
    },
    Case {
        id: "V12",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V12.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(5, 10)],
        members: &[(Kind::Getter, 0, (5, 10), (7, 10))],
        records: 3,
        work: 24 + B + M,
        owned: B + M,
    },
    Case {
        id: "V13",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V13.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(3, 12)],
        members: &[(Kind::Getter, 0, (3, 12), (9, 12))],
        records: 3,
        work: 30 + B + M,
        owned: B + M,
    },
    Case {
        id: "V14",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V14.hex"),
        signature: None,
        outer: &[],
        members: &[],
        records: 1,
        work: 5,
        owned: 0,
    },
    Case {
        id: "V15",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V15.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(3, 5)],
        members: &[],
        records: 2,
        work: 11 + B,
        owned: B,
    },
    Case {
        id: "V16",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V16.hex"),
        signature: Some([None, None, Some((None, None)), None, None]),
        outer: &[(3, 8)],
        members: &[(Kind::Getter, 0, (3, 8), (5, 8))],
        records: 3,
        work: 22 + B + M,
        owned: B + M,
    },
    Case {
        id: "V17",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V17.hex"),
        signature: Some([None, None, Some((None, None)), None, None]),
        outer: &[(3, 10)],
        members: &[(Kind::Getter, 0, (3, 10), (5, 10))],
        records: 3,
        work: 28 + B + M,
        owned: B + M,
    },
    Case {
        id: "V18",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V18.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(3, 11)],
        members: &[
            (Kind::Getter, 0, (3, 7), (5, 7)),
            (Kind::Getter, 0, (7, 11), (9, 11)),
        ],
        records: 5,
        work: 33 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "V19",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V19.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(3, 6)],
        members: &[],
        records: 2,
        work: 13 + B,
        owned: B,
    },
    Case {
        id: "V20",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V20.hex"),
        signature: None,
        outer: &[],
        members: &[],
        records: 1,
        work: 6,
        owned: 0,
    },
    Case {
        id: "V21",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V21.hex"),
        signature: Some([None, None, Some((None, None)), None, None]),
        outer: &[(3, 7)],
        members: &[(Kind::Getter, 0, (3, 7), (5, 7))],
        records: 3,
        work: 19 + B + M,
        owned: B + M,
    },
    Case {
        id: "V22",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V22.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(5, 5)],
        members: &[],
        records: 2,
        work: 9 + B,
        owned: B,
    },
    Case {
        id: "V23",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V23.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(6, 6)],
        members: &[],
        records: 2,
        work: 10 + B,
        owned: B,
    },
    Case {
        id: "V24",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V24.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(3, 7)],
        members: &[(Kind::Getter, 0, (3, 7), (5, 7))],
        records: 4,
        work: 24 + B + M,
        owned: B + M,
    },
    Case {
        id: "V25",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V25.hex"),
        signature: Some([None, None, Some((Some(0), Some(0))), None, None]),
        outer: &[(3, 9)],
        members: &[(Kind::Getter, 0, (3, 9), (5, 9))],
        records: 4,
        work: 27 + B + M,
        owned: B + M,
    },
    Case {
        id: "V26",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V26.hex"),
        signature: Some([
            Some((None, None)),
            Some((None, None)),
            Some((None, None)),
            Some((None, None)),
            Some((None, None)),
        ]),
        outer: &[(3, 13)],
        members: &[
            (Kind::Field, 0, (3, 5), (5, 5)),
            (Kind::SyntheticMethod, 0, (5, 7), (7, 7)),
            (Kind::Getter, 0, (7, 9), (9, 9)),
            (Kind::Setter, 0, (9, 11), (11, 11)),
            (Kind::DelegateMethod, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 35 + B + 8 * M,
        owned: B + 8 * M,
    },
    Case {
        id: "V27",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V27.hex"),
        signature: Some([None, None, None, None, None]),
        outer: &[(3, 3), (6, 6), (9, 9)],
        members: &[],
        records: 3,
        work: 15 + 4 * B,
        owned: 4 * B,
    },
    Case {
        id: "V28",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V28.hex"),
        signature: Some([Some((None, None)), None, Some((None, None)), None, None]),
        outer: &[(3, 5), (8, 8), (11, 13)],
        members: &[
            (Kind::Getter, 0, (3, 5), (5, 5)),
            (Kind::Field, 2, (11, 13), (13, 13)),
        ],
        records: 5,
        work: 27 + 4 * B + 2 * M,
        owned: 4 * B + 2 * M,
    },
    Case {
        id: "V29",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V29.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(7, 11)],
        members: &[(Kind::Getter, 0, (7, 11), (9, 11))],
        records: 3,
        work: 23 + B + M,
        owned: B + M,
    },
    Case {
        id: "V30",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V30.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(6, 10)],
        members: &[(Kind::Getter, 0, (6, 10), (8, 10))],
        records: 3,
        work: 22 + B + M,
        owned: B + M,
    },
    Case {
        id: "V31",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V31.hex"),
        signature: Some([None, None, Some((Some(7), None)), None, None]),
        outer: &[(3, 7), (10, 12)],
        members: &[(Kind::Getter, 0, (3, 7), (5, 7))],
        records: 5,
        work: 30 + 2 * B + M,
        owned: 2 * B + M,
    },
    Case {
        id: "V32",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V32.hex"),
        signature: Some([None, None, Some((Some(7), Some(8))), None, None]),
        outer: &[(3, 13)],
        members: &[
            (Kind::Getter, 0, (3, 9), (5, 9)),
            (Kind::Getter, 0, (9, 13), (11, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "V33",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V33.hex"),
        signature: Some([
            Some((Some(1), Some(2))),
            Some((Some(3), Some(4))),
            Some((Some(5), Some(6))),
            Some((Some(7), Some(8))),
            Some((Some(9), Some(10))),
        ]),
        outer: &[(3, 23), (26, 46)],
        members: &[
            (Kind::Field, 0, (3, 7), (5, 7)),
            (Kind::SyntheticMethod, 0, (7, 11), (9, 11)),
            (Kind::Getter, 0, (11, 15), (13, 15)),
            (Kind::Setter, 0, (15, 19), (17, 19)),
            (Kind::DelegateMethod, 0, (19, 23), (21, 23)),
            (Kind::Field, 1, (26, 30), (28, 30)),
            (Kind::SyntheticMethod, 1, (30, 34), (32, 34)),
            (Kind::Getter, 1, (34, 38), (36, 38)),
            (Kind::Setter, 1, (38, 42), (40, 42)),
            (Kind::DelegateMethod, 1, (42, 46), (44, 46)),
        ],
        records: 22,
        work: 150 + 2 * B + 16 * M,
        owned: 2 * B + 16 * M,
    },
    Case {
        id: "V34",
        raw: include_str!("fixtures/metadata_jvm_property_signature/V34.hex"),
        signature: Some([None, None, Some((Some(9), Some(10))), None, None]),
        outer: &[(3, 9), (12, 16), (19, 23)],
        members: &[
            (Kind::Getter, 0, (3, 9), (5, 9)),
            (Kind::Getter, 1, (12, 16), (14, 16)),
            (Kind::Getter, 2, (19, 23), (21, 23)),
        ],
        records: 10,
        work: 65 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "C_field",
        raw: include_str!("fixtures/metadata_jvm_property_signature/C_field.hex"),
        signature: Some([Some((Some(7), Some(8))), None, None, None, None]),
        outer: &[(3, 7), (10, 14), (17, 19)],
        members: &[
            (Kind::Field, 0, (3, 7), (5, 7)),
            (Kind::Field, 1, (10, 14), (12, 14)),
            (Kind::Field, 2, (17, 19), (19, 19)),
        ],
        records: 8,
        work: 49 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "D_field",
        raw: include_str!("fixtures/metadata_jvm_property_signature/D_field.hex"),
        signature: Some([Some((Some(2), Some(3))), None, None, None, None]),
        outer: &[(3, 13)],
        members: &[
            (Kind::Field, 0, (3, 11), (5, 11)),
            (Kind::Field, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "C_synthetic",
        raw: include_str!("fixtures/metadata_jvm_property_signature/C_synthetic.hex"),
        signature: Some([None, Some((Some(7), Some(8))), None, None, None]),
        outer: &[(3, 7), (10, 14), (17, 19)],
        members: &[
            (Kind::SyntheticMethod, 0, (3, 7), (5, 7)),
            (Kind::SyntheticMethod, 1, (10, 14), (12, 14)),
            (Kind::SyntheticMethod, 2, (17, 19), (19, 19)),
        ],
        records: 8,
        work: 49 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "D_synthetic",
        raw: include_str!("fixtures/metadata_jvm_property_signature/D_synthetic.hex"),
        signature: Some([None, Some((Some(2), Some(3))), None, None, None]),
        outer: &[(3, 13)],
        members: &[
            (Kind::SyntheticMethod, 0, (3, 11), (5, 11)),
            (Kind::SyntheticMethod, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "C_getter",
        raw: include_str!("fixtures/metadata_jvm_property_signature/C_getter.hex"),
        signature: Some([None, None, Some((Some(7), Some(8))), None, None]),
        outer: &[(3, 7), (10, 14), (17, 19)],
        members: &[
            (Kind::Getter, 0, (3, 7), (5, 7)),
            (Kind::Getter, 1, (10, 14), (12, 14)),
            (Kind::Getter, 2, (17, 19), (19, 19)),
        ],
        records: 8,
        work: 49 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "D_getter",
        raw: include_str!("fixtures/metadata_jvm_property_signature/D_getter.hex"),
        signature: Some([None, None, Some((Some(2), Some(3))), None, None]),
        outer: &[(3, 13)],
        members: &[
            (Kind::Getter, 0, (3, 11), (5, 11)),
            (Kind::Getter, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "C_setter",
        raw: include_str!("fixtures/metadata_jvm_property_signature/C_setter.hex"),
        signature: Some([None, None, None, Some((Some(7), Some(8))), None]),
        outer: &[(3, 7), (10, 14), (17, 19)],
        members: &[
            (Kind::Setter, 0, (3, 7), (5, 7)),
            (Kind::Setter, 1, (10, 14), (12, 14)),
            (Kind::Setter, 2, (17, 19), (19, 19)),
        ],
        records: 8,
        work: 49 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "D_setter",
        raw: include_str!("fixtures/metadata_jvm_property_signature/D_setter.hex"),
        signature: Some([None, None, None, Some((Some(2), Some(3))), None]),
        outer: &[(3, 13)],
        members: &[
            (Kind::Setter, 0, (3, 11), (5, 11)),
            (Kind::Setter, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "C_delegate",
        raw: include_str!("fixtures/metadata_jvm_property_signature/C_delegate.hex"),
        signature: Some([None, None, None, None, Some((Some(7), Some(8)))]),
        outer: &[(3, 7), (10, 14), (17, 19)],
        members: &[
            (Kind::DelegateMethod, 0, (3, 7), (5, 7)),
            (Kind::DelegateMethod, 1, (10, 14), (12, 14)),
            (Kind::DelegateMethod, 2, (17, 19), (19, 19)),
        ],
        records: 8,
        work: 49 + 4 * B + 4 * M,
        owned: 4 * B + 4 * M,
    },
    Case {
        id: "D_delegate",
        raw: include_str!("fixtures/metadata_jvm_property_signature/D_delegate.hex"),
        signature: Some([None, None, None, None, Some((Some(2), Some(3)))]),
        outer: &[(3, 13)],
        members: &[
            (Kind::DelegateMethod, 0, (3, 11), (5, 11)),
            (Kind::DelegateMethod, 0, (11, 13), (13, 13)),
        ],
        records: 6,
        work: 41 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "P_first",
        raw: include_str!("fixtures/metadata_jvm_property_signature/P_first.hex"),
        signature: Some([
            Some((None, Some(11))),
            None,
            Some((Some(9), Some(10))),
            None,
            None,
        ]),
        outer: &[(13, 23)],
        members: &[
            (Kind::Field, 0, (13, 17), (15, 17)),
            (Kind::Getter, 0, (17, 23), (19, 23)),
        ],
        records: 9,
        work: 57 + B + 2 * M,
        owned: B + 2 * M,
    },
    Case {
        id: "P_second",
        raw: include_str!("fixtures/metadata_jvm_property_signature/P_second.hex"),
        signature: Some([
            Some((None, Some(11))),
            None,
            Some((Some(12), Some(10))),
            None,
            None,
        ]),
        outer: &[(13, 23)],
        members: &[
            (Kind::Field, 0, (13, 17), (15, 17)),
            (Kind::Getter, 0, (17, 23), (19, 23)),
        ],
        records: 9,
        work: 57 + B + 2 * M,
        owned: B + 2 * M,
    },
];

static MALFORMED: &[(&str, ErrorKind, usize)] = &[
    (
        include_str!("fixtures/metadata_jvm_property_signature/M01.hex"),
        ErrorKind::InvalidFieldNumber,
        0,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M02.hex"),
        ErrorKind::InvalidWireType,
        0,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M03.hex"),
        ErrorKind::UnsupportedGroup,
        0,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M04.hex"),
        ErrorKind::InvalidFieldNumber,
        3,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M05.hex"),
        ErrorKind::Truncated,
        4,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M06.hex"),
        ErrorKind::Truncated,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M07.hex"),
        ErrorKind::InvalidFieldNumber,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M08.hex"),
        ErrorKind::Truncated,
        6,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M09.hex"),
        ErrorKind::Truncated,
        7,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M10.hex"),
        ErrorKind::InvalidWireType,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M11.hex"),
        ErrorKind::UnsupportedGroup,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M12.hex"),
        ErrorKind::Truncated,
        6,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M13.hex"),
        ErrorKind::Truncated,
        7,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M14.hex"),
        ErrorKind::InvalidFieldNumber,
        3,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M15.hex"),
        ErrorKind::Truncated,
        11,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M16.hex"),
        ErrorKind::NegativeLength,
        2,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M17.hex"),
        ErrorKind::NegativeLength,
        4,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M18.hex"),
        ErrorKind::NegativeLength,
        6,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M19.hex"),
        ErrorKind::MalformedVarint,
        15,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M20.hex"),
        ErrorKind::MalformedVarint,
        12,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M21.hex"),
        ErrorKind::Truncated,
        11,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M22.hex"),
        ErrorKind::UnsupportedGroup,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M23.hex"),
        ErrorKind::InvalidWireType,
        5,
    ),
    (
        include_str!("fixtures/metadata_jvm_property_signature/M24.hex"),
        ErrorKind::Truncated,
        6,
    ),
];
#[test]
fn shared_budget_boundaries_and_error_precedence() {
    for (id, limits, label, offset) in [
        (
            "V25",
            Limits {
                bytes: 8,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            "V25",
            Limits {
                records: 0,
                ..Default::default()
            },
            "records",
            0,
        ),
        (
            "V25",
            Limits {
                records: 1,
                ..Default::default()
            },
            "records",
            3,
        ),
        (
            "V25",
            Limits {
                records: 2,
                ..Default::default()
            },
            "records",
            5,
        ),
        (
            "V25",
            Limits {
                records: 3,
                ..Default::default()
            },
            "records",
            7,
        ),
        (
            "V25",
            Limits {
                work: 0,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            "V25",
            Limits {
                work: 1,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            "V25",
            Limits {
                work: 3,
                ..Default::default()
            },
            "work",
            2,
        ),
        (
            "V25",
            Limits {
                work: 9,
                ..Default::default()
            },
            "work",
            3,
        ),
        (
            "V25",
            Limits {
                work: 10,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            "V25",
            Limits {
                work: 18,
                ..Default::default()
            },
            "work",
            3,
        ),
        (
            "V25",
            Limits {
                work: 26,
                ..Default::default()
            },
            "work",
            7,
        ),
        (
            "V25",
            Limits {
                work: 27 + M - 1,
                ..Default::default()
            },
            "work",
            3,
        ),
        (
            "V25",
            Limits {
                work: 27 + M + B - 1,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            "V25",
            Limits {
                owned_bytes: M - 1,
                ..Default::default()
            },
            "owned_bytes",
            3,
        ),
        (
            "V25",
            Limits {
                owned_bytes: M + B - 1,
                ..Default::default()
            },
            "owned_bytes",
            0,
        ),
        (
            "V25",
            Limits {
                owned_bytes: M - 1,
                work: 27,
                ..Default::default()
            },
            "owned_bytes",
            3,
        ),
        (
            "V02",
            Limits {
                owned_bytes: M + B - 1,
                ..Default::default()
            },
            "owned_bytes",
            0,
        ),
        (
            "V03",
            Limits {
                records: 15,
                ..Default::default()
            },
            "records",
            31,
        ),
        (
            "V03",
            Limits {
                owned_bytes: 8 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            27,
        ),
        (
            "V03",
            Limits {
                owned_bytes: 8 * M + B - 1,
                ..Default::default()
            },
            "owned_bytes",
            0,
        ),
        (
            "V03",
            Limits {
                work: 115 + 8 * M - 1,
                ..Default::default()
            },
            "work",
            27,
        ),
        (
            "V03",
            Limits {
                work: 115 + 8 * M + B - 1,
                ..Default::default()
            },
            "work",
            0,
        ),
        (
            "V05",
            Limits {
                records: 8,
                ..Default::default()
            },
            "records",
            19,
        ),
        (
            "V05",
            Limits {
                owned_bytes: 2 * B + 4 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            17,
        ),
        (
            "V05",
            Limits {
                owned_bytes: 4 * B + 4 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            14,
        ),
        (
            "V05",
            Limits {
                work: 54 + 4 * B + 4 * M - 1,
                ..Default::default()
            },
            "work",
            19,
        ),
        (
            "V27",
            Limits {
                records: 2,
                ..Default::default()
            },
            "records",
            6,
        ),
        (
            "V27",
            Limits {
                owned_bytes: 4 * B - 1,
                ..Default::default()
            },
            "owned_bytes",
            6,
        ),
        (
            "V27",
            Limits {
                work: 15 + 4 * B - 1,
                ..Default::default()
            },
            "work",
            6,
        ),
        (
            "V28",
            Limits {
                records: 4,
                ..Default::default()
            },
            "records",
            11,
        ),
        (
            "V28",
            Limits {
                owned_bytes: 4 * B + 2 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            8,
        ),
        (
            "V28",
            Limits {
                work: 27 + 4 * B + 2 * M - 1,
                ..Default::default()
            },
            "work",
            8,
        ),
        (
            "P_first",
            Limits {
                bytes: 22,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            "P_first",
            Limits {
                records: 8,
                ..Default::default()
            },
            "records",
            21,
        ),
        (
            "P_first",
            Limits {
                owned_bytes: 2 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            17,
        ),
        (
            "P_first",
            Limits {
                owned_bytes: 2 * M + B - 1,
                ..Default::default()
            },
            "owned_bytes",
            10,
        ),
        (
            "P_first",
            Limits {
                work: 57 + 2 * M + B - 1,
                ..Default::default()
            },
            "work",
            10,
        ),
        (
            "P_second",
            Limits {
                bytes: 22,
                ..Default::default()
            },
            "bytes",
            0,
        ),
        (
            "P_second",
            Limits {
                records: 8,
                ..Default::default()
            },
            "records",
            21,
        ),
        (
            "P_second",
            Limits {
                owned_bytes: 2 * M - 1,
                ..Default::default()
            },
            "owned_bytes",
            17,
        ),
        (
            "P_second",
            Limits {
                owned_bytes: 2 * M + B - 1,
                ..Default::default()
            },
            "owned_bytes",
            10,
        ),
        (
            "P_second",
            Limits {
                work: 57 + 2 * M + B - 1,
                ..Default::default()
            },
            "work",
            10,
        ),
    ] {
        error(&hex(case(id).raw), limits, ErrorKind::Limit(label), offset);
    }
    let m07 = hex("a2 06 03 1a 01 00");
    error(
        &m07,
        Limits {
            owned_bytes: 0,
            ..Default::default()
        },
        ErrorKind::InvalidFieldNumber,
        5,
    );
    error(
        &m07,
        Limits {
            records: 2,
            ..Default::default()
        },
        ErrorKind::Limit("records"),
        5,
    );
    error(
        &m07,
        Limits {
            bytes: 5,
            ..Default::default()
        },
        ErrorKind::Limit("bytes"),
        0,
    );
    let m08 = hex("a2 06 03 1a 01 08");
    error(
        &m08,
        Limits {
            work: 14,
            ..Default::default()
        },
        ErrorKind::Limit("work"),
        5,
    );
    error(
        &m08,
        Limits {
            work: 15,
            ..Default::default()
        },
        ErrorKind::Limit("work"),
        6,
    );
    error(
        &m08,
        Limits {
            work: 16,
            ..Default::default()
        },
        ErrorKind::Truncated,
        6,
    );
    error(
        &hex("a2 06 03 1a 03 08 10 00"),
        Limits {
            work: 12,
            ..Default::default()
        },
        ErrorKind::Limit("work"),
        5,
    );
    error(
        &hex("a2 06 04 1a 02 0a 02 10 00"),
        Limits {
            work: 18,
            ..Default::default()
        },
        ErrorKind::Limit("work"),
        7,
    );
}

#[test]
fn hard_limits_and_aggregate_records() {
    let default = Limits::default();
    assert_eq!(
        (
            default.bytes,
            default.records,
            default.work,
            default.owned_bytes
        ),
        (MAX_BYTES, MAX_RECORDS, MAX_VALIDATION_UNITS, MAX_BYTES)
    );
    let raised = Limits {
        bytes: usize::MAX,
        records: usize::MAX,
        work: usize::MAX,
        owned_bytes: usize::MAX,
    };
    error(
        &vec![0; MAX_BYTES + 1],
        raised,
        ErrorKind::Limit("bytes"),
        0,
    );
    let empty = [0xa2, 0x06, 0];
    let raw = empty.repeat(65_535);
    let out = decode(
        &raw,
        Limits {
            bytes: 196_605,
            records: 65_535,
            work: 327_675 + 65_536 * B,
            owned_bytes: 65_536 * B,
        },
    )
    .unwrap();
    assert_eq!(out.bodies.len(), 65_535);
    assert_eq!(signature(&out), Some([None; 5]));
    for (i, body) in out.bodies.iter().enumerate() {
        assert_eq!(body.span, 3 * i + 3..3 * i + 3);
    }
    error(
        &empty.repeat(65_536),
        raised,
        ErrorKind::Limit("records"),
        196_605,
    );
    let leaf = [0xa2, 0x06, 2, 0x1a, 0];
    let mut raw = leaf.repeat(32_767);
    raw.extend(empty);
    let out = decode(
        &raw,
        Limits {
            bytes: 163_838,
            records: 65_535,
            work: 360_442 + 32_768 * (B + M),
            owned_bytes: 32_768 * (B + M),
        },
    )
    .unwrap();
    assert_eq!(
        (out.bodies.len(), out.member_bodies.len()),
        (32_768, 32_767)
    );
    for (i, member) in out.member_bodies.iter().enumerate() {
        assert_eq!((member.kind, member.outer_ordinal), (Kind::Getter, i));
        assert_eq!(member.body.span, 5 * i + 5..5 * i + 5);
    }
    error(
        &leaf.repeat(32_768),
        raised,
        ErrorKind::Limit("records"),
        163_838,
    );
}

#[test]
fn genuine_pair_getters_resolve_to_encoded_members() {
    let class_bytes = include_bytes!("fixtures/metadata_constructor/Pair.class.bin");
    let class = ktlsp_binary::parse_class(class_bytes).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let metadata::Outcome::Decoded { header, bytes } =
        metadata::decode(&annotations, Default::default()).unwrap()
    else {
        panic!("supported metadata")
    };
    assert_eq!((header.k, header.mv.as_slice()), (1, [2, 3, 0].as_slice()));
    let typed = payload::decode(&bytes, Default::default()).unwrap();
    assert_eq!(typed.string_table_span, 1..47);
    assert_eq!(typed.payload_span, 47..285);
    let flat = wire::decode(&bytes, Default::default()).unwrap();
    let properties: Vec<_> = flat
        .payload
        .fields
        .iter()
        .filter(|f| f.number == 10 && matches!(f.value, wire::Value::Bytes(_)))
        .collect();
    assert_eq!(properties.len(), 2);
    let mut resolver =
        name_resolver::Resolver::new(&typed.string_table, &header.d2, Default::default()).unwrap();
    for (ordinal, (id, base, name_index, method_ordinal, method_cp, method_start, name)) in [
        ("P_first", 234, 9, 1, (23, 24), 2036, "getFirst"),
        ("P_second", 259, 12, 2, (26, 24), 2105, "getSecond"),
    ]
    .into_iter()
    .enumerate()
    {
        let field = properties[ordinal];
        assert_eq!(field.span, base - 2..base + 23);
        let wire::Value::Bytes(parent) = field.value else {
            unreachable!()
        };
        let c = case(id);
        assert_eq!(parent, hex(c.raw));
        assert_eq!(parent.as_ptr(), bytes[base..base + 23].as_ptr());
        let original = property_header::decode(parent, Default::default()).unwrap();
        assert_eq!(original.name, 5 + ordinal as i32);
        let out = decode(parent, c.limits()).unwrap();
        check(&out, parent, c.signature, c.outer, c.members);
        assert_eq!(
            out.bodies[0].raw.as_ptr(),
            bytes[base + 13..base + 23].as_ptr()
        );
        assert_eq!(
            out.member_bodies[0].body.raw.as_ptr(),
            bytes[base + 15..base + 17].as_ptr()
        );
        assert_eq!(
            out.member_bodies[1].body.raw.as_ptr(),
            bytes[base + 19..base + 23].as_ptr()
        );
        let sig = out.signature.as_ref().unwrap();
        let getter = sig.getter.as_ref().unwrap();
        assert_eq!((getter.name, getter.desc), (Some(name_index), Some(10)));
        let resolved_name = resolver.get_string(getter.name.unwrap()).unwrap();
        let resolved_desc = resolver.get_string(getter.desc.unwrap()).unwrap();
        assert_eq!(resolved_name, name.encode_utf16().collect::<Vec<_>>());
        assert_eq!(
            resolved_desc,
            "()Ljava/lang/Object;".encode_utf16().collect::<Vec<_>>()
        );
        let matched: Vec<_> = class
            .methods
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                class.utf16(m.name).unwrap() == resolved_name
                    && class.utf16(m.descriptor).unwrap() == resolved_desc
            })
            .collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].0, method_ordinal);
        assert_eq!((matched[0].1.name, matched[0].1.descriptor), method_cp);
        assert_eq!(
            &class_bytes[method_start + 2..method_start + 6],
            &[0, method_cp.0 as u8, 0, method_cp.1 as u8]
        );
        let field = sig.field.as_ref().unwrap();
        assert_eq!((field.name, field.desc), (None, Some(11)));
        let descriptor = resolver.get_string(field.desc.unwrap()).unwrap();
        assert_eq!(
            descriptor,
            "Ljava/lang/Object;".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            class
                .fields
                .iter()
                .filter(|f| class.utf16(f.descriptor).unwrap() == descriptor)
                .count(),
            2
        );
    }
}

#[test]
fn deterministic_bounded_hostile_bytes() {
    let limits = Limits {
        bytes: 32,
        records: 8,
        work: 256,
        owned_bytes: 1024,
    };
    let check = |raw: &[u8]| {
        let out = decode(raw, limits);
        assert_eq!(out, decode(raw, limits));
        match out {
            Ok(out) => {
                assert_eq!(out.raw.as_ptr(), raw.as_ptr());
                assert_eq!(out.signature.is_none(), out.bodies.is_empty());
                for body in &out.bodies {
                    assert!(body.span.start <= body.span.end && body.span.end <= raw.len());
                    assert_eq!(body.raw, &raw[body.span.clone()]);
                    assert_eq!(body.raw.as_ptr(), raw[body.span.clone()].as_ptr());
                }
                let mut previous = 0;
                for member in &out.member_bodies {
                    let outer = &out.bodies[member.outer_ordinal];
                    let span = &member.body.span;
                    assert!(
                        outer.span.start <= span.start
                            && span.start <= span.end
                            && span.end <= outer.span.end
                    );
                    assert!(previous <= span.start);
                    previous = span.end;
                    assert_eq!(member.body.raw, &raw[span.clone()]);
                    assert_eq!(member.body.raw.as_ptr(), raw[span.clone()].as_ptr());
                }
            }
            Err(e) => assert!(e.offset <= raw.len()),
        }
    };
    for a in 0..=255 {
        for b in 0..=255 {
            check(&[a, b]);
        }
        for n in 0..32 {
            check(&vec![a; n]);
        }
    }
}
