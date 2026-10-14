//! Finite two-component nullable receiver expectations frozen before source or runtime.
use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS"
    )
}

fn projection(tree: &Parsed) -> Value {
    let elements = tree.elements();
    Value::Array(
        elements
            .iter()
            .filter(|e| {
                !e.token
                    && (declaration(&e.kind)
                        || matches!(
                            e.kind.as_str(),
                            "TYPE_PARAMETER_LIST"
                                | "TYPE_PARAMETER"
                                | "TYPE_REFERENCE"
                                | "NULLABLE_TYPE"
                                | "USER_TYPE"
                                | "DYNAMIC_TYPE"
                                | "REFERENCE_EXPRESSION"
                                | "VALUE_PARAMETER_LIST"
                                | "VALUE_PARAMETER"
                                | "TYPE_ARGUMENT_LIST"
                                | "TYPE_PROJECTION"
                                | "ANNOTATION_ENTRY"
                                | "PROPERTY_ACCESSOR"
                        ))
            })
            .map(|e| {
                let mut parent = e.parent;
                let owner = loop {
                    match parent {
                        Some(id) if declaration(&elements[id].kind) => {
                            break Some((elements[id].start, elements[id].end));
                        }
                        Some(id) => parent = elements[id].parent,
                        None => break None,
                    }
                };
                json!([
                    e.kind,
                    e.start,
                    e.end,
                    elements[e.parent.unwrap()].kind,
                    owner
                ])
            })
            .collect(),
    )
}

fn names(tree: &Parsed) -> Value {
    let elements = tree.elements();
    Value::Array(
        elements
            .iter()
            .filter(|e| {
                e.token
                    && e.kind == "IDENTIFIER"
                    && e.parent.is_some_and(|p| declaration(&elements[p].kind))
            })
            .map(|e| {
                json!([
                    elements[e.parent.unwrap()].start,
                    e.start,
                    e.end,
                    &tree.source()[e.start..e.end]
                ])
            })
            .collect(),
    )
}

fn diagnostics(tree: &Parsed) -> Value {
    Value::Array(
        tree.diagnostics()
            .iter()
            .map(|d| json!([d.start, d.end, d.message]))
            .collect(),
    )
}

fn positive(tree: &Parsed, case: &Value) {
    let id = case["id"].as_str().unwrap();
    assert_eq!(projection(tree), case["projection"], "{id}");
    assert_eq!(names(tree), case["declaration_names"], "{id}");
    assert_eq!(diagnostics(tree), case["diagnostics"], "{id}");
    for expected in case["receiver_tokens"].as_array().unwrap() {
        let start = expected[0].as_u64().unwrap() as usize;
        let end = expected[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && (e.start, e.end) == (start, end)
                && e.kind == expected[2].as_str().unwrap()
                && &tree.source()[start..end] == expected[3].as_str().unwrap()
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == expected[4].as_str().unwrap())),
            "{id}"
        );
    }
    for expected in case["raw_header_tokens"].as_array().unwrap() {
        let start = expected[0].as_u64().unwrap() as usize;
        let end = expected[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && (e.start, e.end) == (start, end)
                && e.kind == expected[2].as_str().unwrap()
                && &tree.source()[start..end] == expected[3].as_str().unwrap()),
            "{id}"
        );
    }
    if let Some(range) = case["type_parameter_name"].as_array() {
        let start = range[0].as_u64().unwrap() as usize;
        let end = range[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && e.kind == "IDENTIFIER"
                && (e.start, e.end) == (start, end)
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == "TYPE_PARAMETER")),
            "{id}"
        );
    }
    for (offset, expected) in case["positions"].as_object().unwrap() {
        let offset = offset.parse::<usize>().unwrap();
        if expected.as_str() == Some("INVALID_CRLF_MIDPOINT") {
            assert!(tree.position(offset).is_err(), "{id}: {offset}");
        } else {
            let actual = tree.position(offset).unwrap();
            assert_eq!(
                json!([actual.line, actual.character]),
                *expected,
                "{id}: {offset}"
            );
        }
    }
    if !case["body_owner"].is_null() {
        node(tree, &case["body_owner"], id);
    }
    for offset in case["invalid_utf8_offsets"].as_array().unwrap() {
        assert!(
            tree.position(offset.as_u64().unwrap() as usize).is_err(),
            "{id}"
        );
    }
}

fn node(tree: &Parsed, expected: &Value, id: &str) {
    let elements = tree.elements();
    let matches: Vec<_> = elements
        .iter()
        .filter(|e| {
            !e.token
                && e.kind == expected[0].as_str().unwrap()
                && e.start == expected[1].as_u64().unwrap() as usize
                && e.end == expected[2].as_u64().unwrap() as usize
        })
        .collect();
    assert_eq!(matches.len(), 1, "{id}: {expected}");
    let parent = matches[0].parent.unwrap();
    assert_eq!(elements[parent].kind, expected[3].as_str().unwrap(), "{id}");
    let mut owner = Some(parent);
    while let Some(index) = owner {
        if declaration(&elements[index].kind) {
            break;
        }
        owner = elements[index].parent;
    }
    assert_eq!(
        owner.map(|index| json!([elements[index].start, elements[index].end])),
        (!expected[4].is_null()).then(|| expected[4].clone()),
        "{id}: {expected}"
    );
}

fn top_level_function(tree: &Parsed, start: usize) -> bool {
    tree.elements().iter().any(|e| {
        !e.token
            && e.kind == "FUN"
            && (e.start, e.end) == (start, tree.source().len())
            && e.parent
                .is_some_and(|p| tree.elements()[p].kind == "KT_FILE")
    })
}

fn declined(tree: &Parsed, id: &str) {
    let class = tree
        .elements()
        .iter()
        .find(|e| !e.token && e.kind == "CLASS")
        .unwrap();
    assert_eq!(class.start, 0, "{id}");
    assert!(class.end > 11, "{id}");
    assert!(!tree.diagnostics().is_empty(), "{id}");
}

fn compatibility(tree: &Parsed, case: &Value) {
    let id = case["id"].as_str().unwrap();
    match case["invariant"].as_str().unwrap() {
        "class-end-greater-than-11" => declined(tree, id),
        "no-complete-KT_FILE-FUN-at-candidate" => {
            assert!(
                !top_level_function(tree, case["candidate_start"].as_u64().unwrap() as usize),
                "{id}"
            );
            assert!(!tree.diagnostics().is_empty(), "{id}");
        }
        "class[0,11); KT_FILE FUN[candidate,source.len); downstream parameter errors" => {
            for expected in [
                json!(["CLASS", 0, 11, "KT_FILE", null]),
                json!(["TYPE_PARAMETER_LIST", 9, 11, "CLASS", [0, 11]]),
                json!(["TYPE_PARAMETER", 10, 11, "TYPE_PARAMETER_LIST", [0, 11]]),
            ] {
                node(tree, &expected, id);
            }
            assert!(
                top_level_function(tree, case["candidate_start"].as_u64().unwrap() as usize),
                "{id}"
            );
            assert_eq!(
                tree.diagnostics()
                    .first()
                    .map(|d| (d.start, d.end, d.message.as_str())),
                Some((11, 11, "Missing '>'")),
                "{id}"
            );
            assert!(
                tree.diagnostics()
                    .iter()
                    .skip(1)
                    .any(|d| d.message != "Missing '>'"),
                "{id}"
            );
        }
        "PROPERTY[0,32) under KT_FILE; anonymous FUN[8,32) nearest PROPERTY[0,32); no direct declaration name on FUN; zero diagnostics"
        | "only KT_FILE FUN[0,source.len); PROPERTY is BLOCK-owned, anonymous FUN is PROPERTY-owned; no promoted sibling; zero diagnostics" =>
        {
            assert!(tree.diagnostics().is_empty(), "{id}");
            for expected in case["ownership"].as_array().unwrap() {
                node(tree, expected, id);
            }
            let start = case["anonymous_start"].as_u64().map_or(8, |s| s as usize);
            let anonymous = tree
                .elements()
                .iter()
                .position(|e| !e.token && e.kind == "FUN" && e.start == start)
                .unwrap();
            assert!(
                !tree
                    .elements()
                    .iter()
                    .any(|e| e.token && e.kind == "IDENTIFIER" && e.parent == Some(anonymous)),
                "{id}"
            );
            let expected_top_level = usize::from(case.get("anonymous_start").is_some());
            assert_eq!(
                tree.elements()
                    .iter()
                    .filter(|e| !e.token
                        && e.kind == "FUN"
                        && e.parent
                            .is_some_and(|p| tree.elements()[p].kind == "KT_FILE"))
                    .count(),
                expected_top_level,
                "{id}"
            );
        }
        "same complete-bound eligibility; CLASS[0,class_end) under KT_FILE; complete FUN[candidate_start,source.len) under KT_FILE" =>
        {
            node(
                tree,
                &json!(["CLASS", 0, case["class_end"], "KT_FILE", null]),
                id,
            );
            assert!(
                top_level_function(tree, case["candidate_start"].as_u64().unwrap() as usize),
                "{id}"
            );
            assert_eq!(diagnostics(tree), case["diagnostics"], "{id}");
        }
        "OBJECT_DECLARATION[0,end) with end>12; no direct KT_FILE FUN starting13" => {
            let object = tree
                .elements()
                .iter()
                .find(|e| {
                    !e.token
                        && e.kind == "OBJECT_DECLARATION"
                        && e.start == 0
                        && e.parent
                            .is_some_and(|p| tree.elements()[p].kind == "KT_FILE")
                })
                .unwrap();
            assert!(object.end > 12, "{id}");
            assert!(
                !tree.elements().iter().any(|e| !e.token
                    && e.kind == "FUN"
                    && e.start == 13
                    && e.parent
                        .is_some_and(|p| tree.elements()[p].kind == "KT_FILE")),
                "{id}"
            );
            assert!(!tree.diagnostics().is_empty(), "{id}");
        }
        "first CLASS extends beyond declined_start; fresh CLASS recovers complete KT_FILE healthy FUN at healthy_start; diagnostics nonempty" =>
        {
            let class = tree
                .elements()
                .iter()
                .find(|e| !e.token && e.kind == "CLASS")
                .unwrap();
            assert_eq!(class.start, 0, "{id}");
            assert!(
                class.end > case["declined_start"].as_u64().unwrap() as usize,
                "{id}"
            );
            assert!(
                top_level_function(tree, case["healthy_start"].as_u64().unwrap() as usize),
                "{id}"
            );
            assert!(!tree.diagnostics().is_empty(), "{id}");
        }
        other => panic!("unknown frozen control: {id}: {other}"),
    }
}

#[test]
fn frozen_two_component_nullable_receiver_contract_through_production_snapshot_worker() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let positives = fixtures["positive"].as_array().unwrap();
    let controls = fixtures["controls"].as_array().unwrap();
    let cuts = fixtures["copied_window_cuts"].as_array().unwrap();
    assert_eq!((positives.len(), controls.len(), cuts.len()), (49, 61, 42));
    assert_eq!(
        fixtures["effective_eof_token_fit"]
            .as_array()
            .unwrap()
            .len(),
        40
    );
    assert_eq!(
        positives
            .iter()
            .map(|c| c["positions"].as_object().unwrap().len())
            .sum::<usize>(),
        1416
    );
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for case in positives.iter().chain(controls).chain(cuts) {
        let id = case["id"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        assert_eq!(
            source.len(),
            case["bytes"].as_u64().unwrap() as usize,
            "{id}"
        );
        let snapshot = SourceSnapshot::new(id, 1, FileKind::Source, source).unwrap();
        let accepted = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert_eq!(accepted.snapshot(), &snapshot, "{id}");
        let tree = accepted.tree_for(&snapshot).unwrap();
        assert_eq!(tree.source(), source, "{id}");
        let mut cursor = 0;
        let mut reconstructed = String::new();
        for e in tree.elements().iter().filter(|e| e.token) {
            assert_eq!(e.start, cursor, "{id}");
            assert!(
                source.is_char_boundary(e.start) && source.is_char_boundary(e.end),
                "{id}"
            );
            reconstructed.push_str(&source[e.start..e.end]);
            cursor = e.end;
        }
        assert_eq!(cursor, source.len(), "{id}");
        assert_eq!(reconstructed, source, "{id}");
        if case.get("projection").is_some() {
            positive(tree, case);
        } else if case.get("target").is_some() {
            if let Some(positive_id) = case["positive_id"].as_str() {
                let expected = positives
                    .iter()
                    .find(|c| c["id"].as_str() == Some(positive_id))
                    .unwrap();
                assert_eq!(source, expected["source"].as_str().unwrap(), "{id}");
                positive(tree, expected);
            } else {
                declined(tree, id);
            }
        } else {
            compatibility(tree, case);
        }
        let original_elements = serde_json::to_value(tree.elements()).unwrap();
        let original_diagnostics = serde_json::to_value(tree.diagnostics()).unwrap();
        for mismatch in [
            SourceSnapshot::new(format!("{id}-other"), 1, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Source, format!("{source} ")).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Script, source).unwrap(),
            SourceSnapshot::new(id, 2, FileKind::Source, source).unwrap(),
        ] {
            assert_eq!(
                accepted.tree_for(&mismatch).unwrap_err(),
                ParserError::StaleSnapshot,
                "{id}"
            );
        }
        let healthy = SourceSnapshot::new(id, 2, FileKind::Source, "fun healthy() = 2").unwrap();
        assert_eq!(
            accepted.tree_for(&healthy).unwrap_err(),
            ParserError::StaleSnapshot,
            "{id}"
        );
        let followup = parser
            .parse_snapshot(&healthy, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(followup.snapshot(), &healthy, "{id}");
        let next = followup.tree_for(&healthy).unwrap();
        assert_eq!(next.source(), healthy.source(), "{id}");
        assert_eq!(
            projection(next),
            fixtures["healthy_followup"]["projection"],
            "{id}"
        );
        assert_eq!(
            names(next),
            fixtures["healthy_followup"]["declaration_names"],
            "{id}"
        );
        assert_eq!(
            diagnostics(next),
            fixtures["healthy_followup"]["diagnostics"],
            "{id}"
        );
        assert_eq!(accepted.snapshot(), &snapshot, "{id}");
        let original = accepted.tree_for(&snapshot).unwrap();
        assert_eq!(original.source(), source, "{id}");
        assert_eq!(
            serde_json::to_value(original.elements()).unwrap(),
            original_elements,
            "{id}"
        );
        assert_eq!(
            serde_json::to_value(original.diagnostics()).unwrap(),
            original_diagnostics,
            "{id}"
        );
    }
}

// Byte-identical reviewed expectations.json SHA256 6f0c74c77dbd250f86039df96fc2810ac83b422381782b0838051bd2584351f2.
const FIXTURES: &str = r####"{
  "stage": "UNEXECUTED CONTRACT ONLY",
  "positive": [
    {
      "id": "two-nullable-class-lf-top",
      "source": "class Box<T\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          41,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            41
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          35,
          37,
          "FUN",
          [
            12,
            41
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          35,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          35,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          35,
          36,
          "LPAR",
          "(",
          9
        ],
        [
          36,
          37,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "35": [
          1,
          23
        ],
        "36": [
          1,
          24
        ],
        "37": [
          1,
          25
        ],
        "41": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 41,
      "source_sha256": "3f15755be73b9e8c660004bc877ccf0c0140be22e5e96cb99d5838b00c4d8bcf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-lf-member",
      "source": "class Host {\nclass Box<T\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          56,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          24,
          "CLASS_BODY",
          [
            0,
            56
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          22,
          24,
          "CLASS",
          [
            13,
            24
          ]
        ],
        [
          "TYPE_PARAMETER",
          23,
          24,
          "TYPE_PARAMETER_LIST",
          [
            13,
            24
          ]
        ],
        [
          "FUN",
          25,
          54,
          "CLASS_BODY",
          [
            0,
            56
          ]
        ],
        [
          "TYPE_REFERENCE",
          29,
          40,
          "FUN",
          [
            25,
            54
          ]
        ],
        [
          "NULLABLE_TYPE",
          29,
          40,
          "TYPE_REFERENCE",
          [
            25,
            54
          ]
        ],
        [
          "USER_TYPE",
          29,
          39,
          "NULLABLE_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "USER_TYPE",
          29,
          32,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          29,
          32,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          33,
          39,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          48,
          50,
          "FUN",
          [
            25,
            54
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          19,
          22,
          "Box"
        ],
        [
          25,
          41,
          48,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          24,
          24,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        23,
        24
      ],
      "receiver_tokens": [
        [
          29,
          32,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          33,
          39,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          39,
          40,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          40,
          41,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          25,
          28,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          28,
          29,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          29,
          32,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          32,
          33,
          "DOT",
          ".",
          4
        ],
        [
          33,
          39,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          39,
          40,
          "QUEST",
          "?",
          6
        ],
        [
          40,
          41,
          "DOT",
          ".",
          7
        ],
        [
          41,
          48,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          48,
          49,
          "LPAR",
          "(",
          9
        ],
        [
          49,
          50,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "19": [
          1,
          6
        ],
        "22": [
          1,
          9
        ],
        "23": [
          1,
          10
        ],
        "24": [
          1,
          11
        ],
        "25": [
          2,
          0
        ],
        "28": [
          2,
          3
        ],
        "29": [
          2,
          4
        ],
        "32": [
          2,
          7
        ],
        "33": [
          2,
          8
        ],
        "39": [
          2,
          14
        ],
        "40": [
          2,
          15
        ],
        "41": [
          2,
          16
        ],
        "48": [
          2,
          23
        ],
        "49": [
          2,
          24
        ],
        "50": [
          2,
          25
        ],
        "54": [
          2,
          29
        ],
        "56": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 56,
      "source_sha256": "2af4bf04f87b352ef4ce6ecf3ee81f61312ed0c4fd7908024428cf25d35db540",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-lf-local",
      "source": "fun outer() {\nclass Box<T\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          57,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            57
          ]
        ],
        [
          "CLASS",
          14,
          25,
          "BLOCK",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          23,
          25,
          "CLASS",
          [
            14,
            25
          ]
        ],
        [
          "TYPE_PARAMETER",
          24,
          25,
          "TYPE_PARAMETER_LIST",
          [
            14,
            25
          ]
        ],
        [
          "FUN",
          26,
          55,
          "BLOCK",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_REFERENCE",
          30,
          41,
          "FUN",
          [
            26,
            55
          ]
        ],
        [
          "NULLABLE_TYPE",
          30,
          41,
          "TYPE_REFERENCE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          40,
          "NULLABLE_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          40,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          49,
          51,
          "FUN",
          [
            26,
            55
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          20,
          23,
          "Box"
        ],
        [
          26,
          42,
          49,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          25,
          25,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        24,
        25
      ],
      "receiver_tokens": [
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          33,
          34,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          41,
          42,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          26,
          29,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          29,
          30,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          33,
          34,
          "DOT",
          ".",
          4
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          6
        ],
        [
          41,
          42,
          "DOT",
          ".",
          7
        ],
        [
          42,
          49,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          49,
          50,
          "LPAR",
          "(",
          9
        ],
        [
          50,
          51,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "20": [
          1,
          6
        ],
        "23": [
          1,
          9
        ],
        "24": [
          1,
          10
        ],
        "25": [
          1,
          11
        ],
        "26": [
          2,
          0
        ],
        "29": [
          2,
          3
        ],
        "30": [
          2,
          4
        ],
        "33": [
          2,
          7
        ],
        "34": [
          2,
          8
        ],
        "40": [
          2,
          14
        ],
        "41": [
          2,
          15
        ],
        "42": [
          2,
          16
        ],
        "49": [
          2,
          23
        ],
        "50": [
          2,
          24
        ],
        "51": [
          2,
          25
        ],
        "55": [
          2,
          29
        ],
        "57": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 57,
      "source_sha256": "4fc1fd99b2377e818b9e5dafce82df0aa9379102dfc499b4b922f846114e092a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-class-lf",
      "source": "class Box<T>\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          13,
          42,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            42
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            42
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "42": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 42,
      "source_sha256": "d330a754ab3f19fb20250662d239412841c5d03a743a0dd19c51dd81b3da49ce",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-lf-top",
      "source": "interface Box<T\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          15,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          15,
          "CLASS",
          [
            0,
            15
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            15
          ]
        ],
        [
          "FUN",
          16,
          45,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          20,
          31,
          "FUN",
          [
            16,
            45
          ]
        ],
        [
          "NULLABLE_TYPE",
          20,
          31,
          "TYPE_REFERENCE",
          [
            16,
            45
          ]
        ],
        [
          "USER_TYPE",
          20,
          30,
          "NULLABLE_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "USER_TYPE",
          20,
          23,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          23,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          24,
          30,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          39,
          41,
          "FUN",
          [
            16,
            45
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          16,
          32,
          39,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          15,
          15,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          20,
          23,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          23,
          24,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          24,
          30,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          30,
          31,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          31,
          32,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          16,
          19,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          19,
          20,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          20,
          23,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          23,
          24,
          "DOT",
          ".",
          4
        ],
        [
          24,
          30,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          30,
          31,
          "QUEST",
          "?",
          6
        ],
        [
          31,
          32,
          "DOT",
          ".",
          7
        ],
        [
          32,
          39,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          39,
          40,
          "LPAR",
          "(",
          9
        ],
        [
          40,
          41,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": [
          1,
          0
        ],
        "19": [
          1,
          3
        ],
        "20": [
          1,
          4
        ],
        "23": [
          1,
          7
        ],
        "24": [
          1,
          8
        ],
        "30": [
          1,
          14
        ],
        "31": [
          1,
          15
        ],
        "32": [
          1,
          16
        ],
        "39": [
          1,
          23
        ],
        "40": [
          1,
          24
        ],
        "41": [
          1,
          25
        ],
        "45": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 45,
      "source_sha256": "a3196238ad9b3dcf1a3ade7c8f43721370c4e574a2c8ff1c1ca3241e344616d2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-lf-member",
      "source": "class Host {\ninterface Box<T\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          60,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          28,
          "CLASS_BODY",
          [
            0,
            60
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          26,
          28,
          "CLASS",
          [
            13,
            28
          ]
        ],
        [
          "TYPE_PARAMETER",
          27,
          28,
          "TYPE_PARAMETER_LIST",
          [
            13,
            28
          ]
        ],
        [
          "FUN",
          29,
          58,
          "CLASS_BODY",
          [
            0,
            60
          ]
        ],
        [
          "TYPE_REFERENCE",
          33,
          44,
          "FUN",
          [
            29,
            58
          ]
        ],
        [
          "NULLABLE_TYPE",
          33,
          44,
          "TYPE_REFERENCE",
          [
            29,
            58
          ]
        ],
        [
          "USER_TYPE",
          33,
          43,
          "NULLABLE_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "USER_TYPE",
          33,
          36,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          33,
          36,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          37,
          43,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          52,
          54,
          "FUN",
          [
            29,
            58
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          23,
          26,
          "Box"
        ],
        [
          29,
          45,
          52,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          28,
          28,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        27,
        28
      ],
      "receiver_tokens": [
        [
          33,
          36,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          36,
          37,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          37,
          43,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          43,
          44,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          44,
          45,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          29,
          32,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          32,
          33,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          33,
          36,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          36,
          37,
          "DOT",
          ".",
          4
        ],
        [
          37,
          43,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          43,
          44,
          "QUEST",
          "?",
          6
        ],
        [
          44,
          45,
          "DOT",
          ".",
          7
        ],
        [
          45,
          52,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          52,
          53,
          "LPAR",
          "(",
          9
        ],
        [
          53,
          54,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "23": [
          1,
          10
        ],
        "26": [
          1,
          13
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          2,
          0
        ],
        "32": [
          2,
          3
        ],
        "33": [
          2,
          4
        ],
        "36": [
          2,
          7
        ],
        "37": [
          2,
          8
        ],
        "43": [
          2,
          14
        ],
        "44": [
          2,
          15
        ],
        "45": [
          2,
          16
        ],
        "52": [
          2,
          23
        ],
        "53": [
          2,
          24
        ],
        "54": [
          2,
          25
        ],
        "58": [
          2,
          29
        ],
        "60": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 60,
      "source_sha256": "795e479d0f6e3f1c301a94582ab533766eb1179444484a374eda43b3d7514601",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-lf-local",
      "source": "fun outer() {\ninterface Box<T\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          61,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            61
          ]
        ],
        [
          "CLASS",
          14,
          29,
          "BLOCK",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          27,
          29,
          "CLASS",
          [
            14,
            29
          ]
        ],
        [
          "TYPE_PARAMETER",
          28,
          29,
          "TYPE_PARAMETER_LIST",
          [
            14,
            29
          ]
        ],
        [
          "FUN",
          30,
          59,
          "BLOCK",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_REFERENCE",
          34,
          45,
          "FUN",
          [
            30,
            59
          ]
        ],
        [
          "NULLABLE_TYPE",
          34,
          45,
          "TYPE_REFERENCE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          44,
          "NULLABLE_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          38,
          44,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          53,
          55,
          "FUN",
          [
            30,
            59
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          24,
          27,
          "Box"
        ],
        [
          30,
          46,
          53,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          29,
          29,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        28,
        29
      ],
      "receiver_tokens": [
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          37,
          38,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          45,
          46,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          30,
          33,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          33,
          34,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          37,
          38,
          "DOT",
          ".",
          4
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          6
        ],
        [
          45,
          46,
          "DOT",
          ".",
          7
        ],
        [
          46,
          53,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          53,
          54,
          "LPAR",
          "(",
          9
        ],
        [
          54,
          55,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "24": [
          1,
          10
        ],
        "27": [
          1,
          13
        ],
        "28": [
          1,
          14
        ],
        "29": [
          1,
          15
        ],
        "30": [
          2,
          0
        ],
        "33": [
          2,
          3
        ],
        "34": [
          2,
          4
        ],
        "37": [
          2,
          7
        ],
        "38": [
          2,
          8
        ],
        "44": [
          2,
          14
        ],
        "45": [
          2,
          15
        ],
        "46": [
          2,
          16
        ],
        "53": [
          2,
          23
        ],
        "54": [
          2,
          24
        ],
        "55": [
          2,
          25
        ],
        "59": [
          2,
          29
        ],
        "61": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 61,
      "source_sha256": "ea6c10a5db8430c09fe158201e7d5f8458a9f1d00b901913c6575a727d713d36",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-interface-lf",
      "source": "interface Box<T>\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          16,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          16,
          "CLASS",
          [
            0,
            16
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            16
          ]
        ],
        [
          "FUN",
          17,
          46,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          21,
          32,
          "FUN",
          [
            17,
            46
          ]
        ],
        [
          "NULLABLE_TYPE",
          21,
          32,
          "TYPE_REFERENCE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          31,
          "NULLABLE_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          25,
          31,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          40,
          42,
          "FUN",
          [
            17,
            46
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          17,
          33,
          40,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          24,
          25,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          17,
          20,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          20,
          21,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          24,
          25,
          "DOT",
          ".",
          4
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          6
        ],
        [
          32,
          33,
          "DOT",
          ".",
          7
        ],
        [
          33,
          40,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          40,
          41,
          "LPAR",
          "(",
          9
        ],
        [
          41,
          42,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": [
          0,
          16
        ],
        "17": [
          1,
          0
        ],
        "20": [
          1,
          3
        ],
        "21": [
          1,
          4
        ],
        "24": [
          1,
          7
        ],
        "25": [
          1,
          8
        ],
        "31": [
          1,
          14
        ],
        "32": [
          1,
          15
        ],
        "33": [
          1,
          16
        ],
        "40": [
          1,
          23
        ],
        "41": [
          1,
          24
        ],
        "42": [
          1,
          25
        ],
        "46": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 46,
      "source_sha256": "359802afd9f9f85e88f1c07f70bb6dc3a1f3f43714398a52e0967750e8e9d2ce",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-spaced-lf",
      "source": "class Box<T\nfun pkg.String ? .healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          28,
          "FUN",
          [
            12,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          28,
          "TYPE_REFERENCE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          37,
          39,
          "FUN",
          [
            12,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          30,
          37,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          29,
          30,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "WHITE_SPACE",
          " ",
          6
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          7
        ],
        [
          28,
          29,
          "WHITE_SPACE",
          " ",
          8
        ],
        [
          29,
          30,
          "DOT",
          ".",
          9
        ],
        [
          30,
          37,
          "IDENTIFIER",
          "healthy",
          10
        ],
        [
          37,
          38,
          "LPAR",
          "(",
          11
        ],
        [
          38,
          39,
          "RPAR",
          ")",
          12
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "29": [
          1,
          17
        ],
        "30": [
          1,
          18
        ],
        "37": [
          1,
          25
        ],
        "38": [
          1,
          26
        ],
        "39": [
          1,
          27
        ],
        "43": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 26
      },
      "body_owner": null,
      "bytes": 43,
      "source_sha256": "a18f19944ffca405a82ae2a564b20f8f66a583c3c1d7ba5894b0a26765a6f104",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comment-lf",
      "source": "class Box<T\nfun pkg.String?/*x*/.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          46,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            46
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            46
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          40,
          42,
          "FUN",
          [
            12,
            46
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          33,
          40,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          32,
          "BLOCK_COMMENT",
          "/*x*/",
          7
        ],
        [
          32,
          33,
          "DOT",
          ".",
          8
        ],
        [
          33,
          40,
          "IDENTIFIER",
          "healthy",
          9
        ],
        [
          40,
          41,
          "LPAR",
          "(",
          10
        ],
        [
          41,
          42,
          "RPAR",
          ")",
          11
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "32": [
          1,
          20
        ],
        "33": [
          1,
          21
        ],
        "40": [
          1,
          28
        ],
        "41": [
          1,
          29
        ],
        "42": [
          1,
          30
        ],
        "46": [
          1,
          34
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 10,
        "lpar_end_from_probe_start": 29
      },
      "body_owner": null,
      "bytes": 46,
      "source_sha256": "d305e1d5569603f8d738bcfcd2ffe39f3801d828b6af94546bf929c00d402cac",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-empty-lf",
      "source": "class Box<\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          11,
          40,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          15,
          26,
          "FUN",
          [
            11,
            40
          ]
        ],
        [
          "NULLABLE_TYPE",
          15,
          26,
          "TYPE_REFERENCE",
          [
            11,
            40
          ]
        ],
        [
          "USER_TYPE",
          15,
          25,
          "NULLABLE_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "USER_TYPE",
          15,
          18,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          15,
          18,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          19,
          25,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          34,
          36,
          "FUN",
          [
            11,
            40
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          11,
          27,
          34,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          15,
          18,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          18,
          19,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          19,
          25,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          25,
          26,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          26,
          27,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          11,
          14,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          14,
          15,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          15,
          18,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          18,
          19,
          "DOT",
          ".",
          4
        ],
        [
          19,
          25,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          25,
          26,
          "QUEST",
          "?",
          6
        ],
        [
          26,
          27,
          "DOT",
          ".",
          7
        ],
        [
          27,
          34,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          34,
          35,
          "LPAR",
          "(",
          9
        ],
        [
          35,
          36,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          1,
          0
        ],
        "14": [
          1,
          3
        ],
        "15": [
          1,
          4
        ],
        "18": [
          1,
          7
        ],
        "19": [
          1,
          8
        ],
        "25": [
          1,
          14
        ],
        "26": [
          1,
          15
        ],
        "27": [
          1,
          16
        ],
        "34": [
          1,
          23
        ],
        "35": [
          1,
          24
        ],
        "36": [
          1,
          25
        ],
        "40": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 40,
      "source_sha256": "445a0553edfa3c4bcc8ecf1bed80b990b0ace5ff4cab5a67e5e93f948e689e76",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comma-lf",
      "source": "class Box<T,\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          13,
          42,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            42
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            42
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          12,
          12,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "42": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 42,
      "source_sha256": "fa57869d6b0e2b3d37114a47a5b4574e3467211948595e2e6cb8250bd4d3fd15",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-unicode-lf",
      "source": "class Box<T\nfun pkg.String?.`healthy 🦀é`() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          51,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            51
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            51
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          45,
          47,
          "FUN",
          [
            12,
            51
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          45,
          "`healthy 🦀é`"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          45,
          "IDENTIFIER",
          "`healthy 🦀é`",
          8
        ],
        [
          45,
          46,
          "LPAR",
          "(",
          9
        ],
        [
          46,
          47,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "45": [
          1,
          30
        ],
        "46": [
          1,
          31
        ],
        "47": [
          1,
          32
        ],
        "51": [
          1,
          36
        ]
      },
      "invalid_utf8_offsets": [
        38,
        39,
        40,
        43
      ],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 34
      },
      "body_owner": null,
      "bytes": 51,
      "source_sha256": "e7d2b27c36e7ed0a28ae57ddd6adb9af1146e81445d6d7a0b11209362c8cc21e",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-body-lf",
      "source": "class Box<T\nfun pkg.String?.healthy() { 2 }",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          35,
          37,
          "FUN",
          [
            12,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          35,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          35,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          35,
          36,
          "LPAR",
          "(",
          9
        ],
        [
          36,
          37,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "35": [
          1,
          23
        ],
        "36": [
          1,
          24
        ],
        "37": [
          1,
          25
        ],
        "43": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": [
        "BLOCK",
        38,
        43,
        "FUN",
        [
          12,
          43
        ]
      ],
      "bytes": 43,
      "source_sha256": "0167d266a8242418adf5721e1a5c7f3d953b9c40c1354756f31f77eec4246991",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-leading-trivia-lf",
      "source": "class Box<\n\n/*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ \nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          12,
          13242,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          13217,
          13228,
          "FUN",
          [
            12,
            13242
          ]
        ],
        [
          "NULLABLE_TYPE",
          13217,
          13228,
          "TYPE_REFERENCE",
          [
            12,
            13242
          ]
        ],
        [
          "USER_TYPE",
          13217,
          13227,
          "NULLABLE_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "USER_TYPE",
          13217,
          13220,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13217,
          13220,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13221,
          13227,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          13236,
          13238,
          "FUN",
          [
            12,
            13242
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          13229,
          13236,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          13217,
          13220,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          13220,
          13221,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          13221,
          13227,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          13227,
          13228,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          13228,
          13229,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13213,
          13216,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          13216,
          13217,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          13217,
          13220,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          13220,
          13221,
          "DOT",
          ".",
          4
        ],
        [
          13221,
          13227,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          13227,
          13228,
          "QUEST",
          "?",
          6
        ],
        [
          13228,
          13229,
          "DOT",
          ".",
          7
        ],
        [
          13229,
          13236,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          13236,
          13237,
          "LPAR",
          "(",
          9
        ],
        [
          13237,
          13238,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "12": [
          2,
          0
        ],
        "13213": [
          3,
          0
        ],
        "13216": [
          3,
          3
        ],
        "13217": [
          3,
          4
        ],
        "13220": [
          3,
          7
        ],
        "13221": [
          3,
          8
        ],
        "13227": [
          3,
          14
        ],
        "13228": [
          3,
          15
        ],
        "13229": [
          3,
          16
        ],
        "13236": [
          3,
          23
        ],
        "13237": [
          3,
          24
        ],
        "13238": [
          3,
          25
        ],
        "13242": [
          3,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 13242,
      "source_sha256": "c96b1b1e4de3c2f75cca60424983fec6f1f12f52de886ca871311d697704fee2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-crlf-top",
      "source": "class Box<T\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          13,
          42,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            42
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            42
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": "INVALID_CRLF_MIDPOINT",
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "42": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 42,
      "source_sha256": "3fd26b9f09c4d58971f0a96b0bcd50df88fe3673a32f010e04ce8b3eb6822963",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-crlf-member",
      "source": "class Host {\nclass Box<T\r\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          57,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          24,
          "CLASS_BODY",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          22,
          24,
          "CLASS",
          [
            13,
            24
          ]
        ],
        [
          "TYPE_PARAMETER",
          23,
          24,
          "TYPE_PARAMETER_LIST",
          [
            13,
            24
          ]
        ],
        [
          "FUN",
          26,
          55,
          "CLASS_BODY",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_REFERENCE",
          30,
          41,
          "FUN",
          [
            26,
            55
          ]
        ],
        [
          "NULLABLE_TYPE",
          30,
          41,
          "TYPE_REFERENCE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          40,
          "NULLABLE_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          40,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          49,
          51,
          "FUN",
          [
            26,
            55
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          19,
          22,
          "Box"
        ],
        [
          26,
          42,
          49,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          24,
          24,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        23,
        24
      ],
      "receiver_tokens": [
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          33,
          34,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          41,
          42,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          26,
          29,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          29,
          30,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          33,
          34,
          "DOT",
          ".",
          4
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          6
        ],
        [
          41,
          42,
          "DOT",
          ".",
          7
        ],
        [
          42,
          49,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          49,
          50,
          "LPAR",
          "(",
          9
        ],
        [
          50,
          51,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "19": [
          1,
          6
        ],
        "22": [
          1,
          9
        ],
        "23": [
          1,
          10
        ],
        "24": [
          1,
          11
        ],
        "25": "INVALID_CRLF_MIDPOINT",
        "26": [
          2,
          0
        ],
        "29": [
          2,
          3
        ],
        "30": [
          2,
          4
        ],
        "33": [
          2,
          7
        ],
        "34": [
          2,
          8
        ],
        "40": [
          2,
          14
        ],
        "41": [
          2,
          15
        ],
        "42": [
          2,
          16
        ],
        "49": [
          2,
          23
        ],
        "50": [
          2,
          24
        ],
        "51": [
          2,
          25
        ],
        "55": [
          2,
          29
        ],
        "57": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 57,
      "source_sha256": "dd87928c0f49342dcf792c4f66fb0ff191112736e67fedb902ee5e1d2b51a738",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-crlf-local",
      "source": "fun outer() {\nclass Box<T\r\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          58,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            58
          ]
        ],
        [
          "CLASS",
          14,
          25,
          "BLOCK",
          [
            0,
            58
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          23,
          25,
          "CLASS",
          [
            14,
            25
          ]
        ],
        [
          "TYPE_PARAMETER",
          24,
          25,
          "TYPE_PARAMETER_LIST",
          [
            14,
            25
          ]
        ],
        [
          "FUN",
          27,
          56,
          "BLOCK",
          [
            0,
            58
          ]
        ],
        [
          "TYPE_REFERENCE",
          31,
          42,
          "FUN",
          [
            27,
            56
          ]
        ],
        [
          "NULLABLE_TYPE",
          31,
          42,
          "TYPE_REFERENCE",
          [
            27,
            56
          ]
        ],
        [
          "USER_TYPE",
          31,
          41,
          "NULLABLE_TYPE",
          [
            27,
            56
          ]
        ],
        [
          "USER_TYPE",
          31,
          34,
          "USER_TYPE",
          [
            27,
            56
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          31,
          34,
          "USER_TYPE",
          [
            27,
            56
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          35,
          41,
          "USER_TYPE",
          [
            27,
            56
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          50,
          52,
          "FUN",
          [
            27,
            56
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          20,
          23,
          "Box"
        ],
        [
          27,
          43,
          50,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          25,
          25,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        24,
        25
      ],
      "receiver_tokens": [
        [
          31,
          34,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          34,
          35,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          35,
          41,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          41,
          42,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          42,
          43,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          27,
          30,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          30,
          31,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          31,
          34,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          34,
          35,
          "DOT",
          ".",
          4
        ],
        [
          35,
          41,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          41,
          42,
          "QUEST",
          "?",
          6
        ],
        [
          42,
          43,
          "DOT",
          ".",
          7
        ],
        [
          43,
          50,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          50,
          51,
          "LPAR",
          "(",
          9
        ],
        [
          51,
          52,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "20": [
          1,
          6
        ],
        "23": [
          1,
          9
        ],
        "24": [
          1,
          10
        ],
        "25": [
          1,
          11
        ],
        "26": "INVALID_CRLF_MIDPOINT",
        "27": [
          2,
          0
        ],
        "30": [
          2,
          3
        ],
        "31": [
          2,
          4
        ],
        "34": [
          2,
          7
        ],
        "35": [
          2,
          8
        ],
        "41": [
          2,
          14
        ],
        "42": [
          2,
          15
        ],
        "43": [
          2,
          16
        ],
        "50": [
          2,
          23
        ],
        "51": [
          2,
          24
        ],
        "52": [
          2,
          25
        ],
        "56": [
          2,
          29
        ],
        "58": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 58,
      "source_sha256": "c4e7d8fa816a46fe2d910fb4910cc15384822221eb4db47c616b5d6fec716a44",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-class-crlf",
      "source": "class Box<T>\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          14,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          18,
          29,
          "FUN",
          [
            14,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          18,
          29,
          "TYPE_REFERENCE",
          [
            14,
            43
          ]
        ],
        [
          "USER_TYPE",
          18,
          28,
          "NULLABLE_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "USER_TYPE",
          18,
          21,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          18,
          21,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          22,
          28,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          37,
          39,
          "FUN",
          [
            14,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          14,
          30,
          37,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          18,
          21,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          21,
          22,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          22,
          28,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          29,
          30,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          14,
          17,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          17,
          18,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          18,
          21,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          21,
          22,
          "DOT",
          ".",
          4
        ],
        [
          22,
          28,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          6
        ],
        [
          29,
          30,
          "DOT",
          ".",
          7
        ],
        [
          30,
          37,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          37,
          38,
          "LPAR",
          "(",
          9
        ],
        [
          38,
          39,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": "INVALID_CRLF_MIDPOINT",
        "14": [
          1,
          0
        ],
        "17": [
          1,
          3
        ],
        "18": [
          1,
          4
        ],
        "21": [
          1,
          7
        ],
        "22": [
          1,
          8
        ],
        "28": [
          1,
          14
        ],
        "29": [
          1,
          15
        ],
        "30": [
          1,
          16
        ],
        "37": [
          1,
          23
        ],
        "38": [
          1,
          24
        ],
        "39": [
          1,
          25
        ],
        "43": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 43,
      "source_sha256": "28782b29f1c66dd4abd4701aa192d1e1d806c16ea7e9727f92ef2d2ee7417b4b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-crlf-top",
      "source": "interface Box<T\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          15,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          15,
          "CLASS",
          [
            0,
            15
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            15
          ]
        ],
        [
          "FUN",
          17,
          46,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          21,
          32,
          "FUN",
          [
            17,
            46
          ]
        ],
        [
          "NULLABLE_TYPE",
          21,
          32,
          "TYPE_REFERENCE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          31,
          "NULLABLE_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          25,
          31,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          40,
          42,
          "FUN",
          [
            17,
            46
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          17,
          33,
          40,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          15,
          15,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          24,
          25,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          17,
          20,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          20,
          21,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          24,
          25,
          "DOT",
          ".",
          4
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          6
        ],
        [
          32,
          33,
          "DOT",
          ".",
          7
        ],
        [
          33,
          40,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          40,
          41,
          "LPAR",
          "(",
          9
        ],
        [
          41,
          42,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": "INVALID_CRLF_MIDPOINT",
        "17": [
          1,
          0
        ],
        "20": [
          1,
          3
        ],
        "21": [
          1,
          4
        ],
        "24": [
          1,
          7
        ],
        "25": [
          1,
          8
        ],
        "31": [
          1,
          14
        ],
        "32": [
          1,
          15
        ],
        "33": [
          1,
          16
        ],
        "40": [
          1,
          23
        ],
        "41": [
          1,
          24
        ],
        "42": [
          1,
          25
        ],
        "46": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 46,
      "source_sha256": "b0bea383cd529211388300950104b2d682bb8ace52aeeec2f4b98e6be1e3666d",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-crlf-member",
      "source": "class Host {\ninterface Box<T\r\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          61,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          28,
          "CLASS_BODY",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          26,
          28,
          "CLASS",
          [
            13,
            28
          ]
        ],
        [
          "TYPE_PARAMETER",
          27,
          28,
          "TYPE_PARAMETER_LIST",
          [
            13,
            28
          ]
        ],
        [
          "FUN",
          30,
          59,
          "CLASS_BODY",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_REFERENCE",
          34,
          45,
          "FUN",
          [
            30,
            59
          ]
        ],
        [
          "NULLABLE_TYPE",
          34,
          45,
          "TYPE_REFERENCE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          44,
          "NULLABLE_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          38,
          44,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          53,
          55,
          "FUN",
          [
            30,
            59
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          23,
          26,
          "Box"
        ],
        [
          30,
          46,
          53,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          28,
          28,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        27,
        28
      ],
      "receiver_tokens": [
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          37,
          38,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          45,
          46,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          30,
          33,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          33,
          34,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          37,
          38,
          "DOT",
          ".",
          4
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          6
        ],
        [
          45,
          46,
          "DOT",
          ".",
          7
        ],
        [
          46,
          53,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          53,
          54,
          "LPAR",
          "(",
          9
        ],
        [
          54,
          55,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "23": [
          1,
          10
        ],
        "26": [
          1,
          13
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": "INVALID_CRLF_MIDPOINT",
        "30": [
          2,
          0
        ],
        "33": [
          2,
          3
        ],
        "34": [
          2,
          4
        ],
        "37": [
          2,
          7
        ],
        "38": [
          2,
          8
        ],
        "44": [
          2,
          14
        ],
        "45": [
          2,
          15
        ],
        "46": [
          2,
          16
        ],
        "53": [
          2,
          23
        ],
        "54": [
          2,
          24
        ],
        "55": [
          2,
          25
        ],
        "59": [
          2,
          29
        ],
        "61": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 61,
      "source_sha256": "37edbdb79a022b22d0bdb30309b112d577caf4ac6b4e6a0538f79c6d2002d1fb",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-crlf-local",
      "source": "fun outer() {\ninterface Box<T\r\nfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          62,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            62
          ]
        ],
        [
          "CLASS",
          14,
          29,
          "BLOCK",
          [
            0,
            62
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          27,
          29,
          "CLASS",
          [
            14,
            29
          ]
        ],
        [
          "TYPE_PARAMETER",
          28,
          29,
          "TYPE_PARAMETER_LIST",
          [
            14,
            29
          ]
        ],
        [
          "FUN",
          31,
          60,
          "BLOCK",
          [
            0,
            62
          ]
        ],
        [
          "TYPE_REFERENCE",
          35,
          46,
          "FUN",
          [
            31,
            60
          ]
        ],
        [
          "NULLABLE_TYPE",
          35,
          46,
          "TYPE_REFERENCE",
          [
            31,
            60
          ]
        ],
        [
          "USER_TYPE",
          35,
          45,
          "NULLABLE_TYPE",
          [
            31,
            60
          ]
        ],
        [
          "USER_TYPE",
          35,
          38,
          "USER_TYPE",
          [
            31,
            60
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          35,
          38,
          "USER_TYPE",
          [
            31,
            60
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          39,
          45,
          "USER_TYPE",
          [
            31,
            60
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          54,
          56,
          "FUN",
          [
            31,
            60
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          24,
          27,
          "Box"
        ],
        [
          31,
          47,
          54,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          29,
          29,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        28,
        29
      ],
      "receiver_tokens": [
        [
          35,
          38,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          38,
          39,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          39,
          45,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          45,
          46,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          46,
          47,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          31,
          34,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          34,
          35,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          35,
          38,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          38,
          39,
          "DOT",
          ".",
          4
        ],
        [
          39,
          45,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          45,
          46,
          "QUEST",
          "?",
          6
        ],
        [
          46,
          47,
          "DOT",
          ".",
          7
        ],
        [
          47,
          54,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          54,
          55,
          "LPAR",
          "(",
          9
        ],
        [
          55,
          56,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "24": [
          1,
          10
        ],
        "27": [
          1,
          13
        ],
        "28": [
          1,
          14
        ],
        "29": [
          1,
          15
        ],
        "30": "INVALID_CRLF_MIDPOINT",
        "31": [
          2,
          0
        ],
        "34": [
          2,
          3
        ],
        "35": [
          2,
          4
        ],
        "38": [
          2,
          7
        ],
        "39": [
          2,
          8
        ],
        "45": [
          2,
          14
        ],
        "46": [
          2,
          15
        ],
        "47": [
          2,
          16
        ],
        "54": [
          2,
          23
        ],
        "55": [
          2,
          24
        ],
        "56": [
          2,
          25
        ],
        "60": [
          2,
          29
        ],
        "62": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 62,
      "source_sha256": "e8c8572cc6be63991785d45068229adf5eb7bcce92bbf621016f4ff0d3192b42",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-interface-crlf",
      "source": "interface Box<T>\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          16,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          16,
          "CLASS",
          [
            0,
            16
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            16
          ]
        ],
        [
          "FUN",
          18,
          47,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          22,
          33,
          "FUN",
          [
            18,
            47
          ]
        ],
        [
          "NULLABLE_TYPE",
          22,
          33,
          "TYPE_REFERENCE",
          [
            18,
            47
          ]
        ],
        [
          "USER_TYPE",
          22,
          32,
          "NULLABLE_TYPE",
          [
            18,
            47
          ]
        ],
        [
          "USER_TYPE",
          22,
          25,
          "USER_TYPE",
          [
            18,
            47
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          22,
          25,
          "USER_TYPE",
          [
            18,
            47
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          26,
          32,
          "USER_TYPE",
          [
            18,
            47
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          41,
          43,
          "FUN",
          [
            18,
            47
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          18,
          34,
          41,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          22,
          25,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          25,
          26,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          26,
          32,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          32,
          33,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          33,
          34,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          18,
          21,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          21,
          22,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          22,
          25,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          25,
          26,
          "DOT",
          ".",
          4
        ],
        [
          26,
          32,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          32,
          33,
          "QUEST",
          "?",
          6
        ],
        [
          33,
          34,
          "DOT",
          ".",
          7
        ],
        [
          34,
          41,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          41,
          42,
          "LPAR",
          "(",
          9
        ],
        [
          42,
          43,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": [
          0,
          16
        ],
        "17": "INVALID_CRLF_MIDPOINT",
        "18": [
          1,
          0
        ],
        "21": [
          1,
          3
        ],
        "22": [
          1,
          4
        ],
        "25": [
          1,
          7
        ],
        "26": [
          1,
          8
        ],
        "32": [
          1,
          14
        ],
        "33": [
          1,
          15
        ],
        "34": [
          1,
          16
        ],
        "41": [
          1,
          23
        ],
        "42": [
          1,
          24
        ],
        "43": [
          1,
          25
        ],
        "47": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 47,
      "source_sha256": "523c5071ad66db490b96968e92650158f81314f7c25aeb021d7939a35c81a476",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-spaced-crlf",
      "source": "class Box<T\r\nfun pkg.String ? .healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          13,
          44,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          29,
          "FUN",
          [
            13,
            44
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          29,
          "TYPE_REFERENCE",
          [
            13,
            44
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          38,
          40,
          "FUN",
          [
            13,
            44
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          31,
          38,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          30,
          31,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "WHITE_SPACE",
          " ",
          6
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          7
        ],
        [
          29,
          30,
          "WHITE_SPACE",
          " ",
          8
        ],
        [
          30,
          31,
          "DOT",
          ".",
          9
        ],
        [
          31,
          38,
          "IDENTIFIER",
          "healthy",
          10
        ],
        [
          38,
          39,
          "LPAR",
          "(",
          11
        ],
        [
          39,
          40,
          "RPAR",
          ")",
          12
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": "INVALID_CRLF_MIDPOINT",
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "30": [
          1,
          17
        ],
        "31": [
          1,
          18
        ],
        "38": [
          1,
          25
        ],
        "39": [
          1,
          26
        ],
        "40": [
          1,
          27
        ],
        "44": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 26
      },
      "body_owner": null,
      "bytes": 44,
      "source_sha256": "cbe895d4d9d517b6ff9fe02e27233b9f55b03a8d3a7874f5ccd0b0d4e3d331ff",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comment-crlf",
      "source": "class Box<T\r\nfun pkg.String?/*x*/.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          13,
          47,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            47
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            47
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            47
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            47
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            47
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            47
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          41,
          43,
          "FUN",
          [
            13,
            47
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          34,
          41,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          33,
          34,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          33,
          "BLOCK_COMMENT",
          "/*x*/",
          7
        ],
        [
          33,
          34,
          "DOT",
          ".",
          8
        ],
        [
          34,
          41,
          "IDENTIFIER",
          "healthy",
          9
        ],
        [
          41,
          42,
          "LPAR",
          "(",
          10
        ],
        [
          42,
          43,
          "RPAR",
          ")",
          11
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": "INVALID_CRLF_MIDPOINT",
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "33": [
          1,
          20
        ],
        "34": [
          1,
          21
        ],
        "41": [
          1,
          28
        ],
        "42": [
          1,
          29
        ],
        "43": [
          1,
          30
        ],
        "47": [
          1,
          34
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 10,
        "lpar_end_from_probe_start": 29
      },
      "body_owner": null,
      "bytes": 47,
      "source_sha256": "acc988779f3dc9b3bb9267d47198b074c89b72a4fe3ebcf16bcb3d94cd5dacdf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-empty-crlf",
      "source": "class Box<\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          12,
          41,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            41
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          35,
          37,
          "FUN",
          [
            12,
            41
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          35,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          35,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          35,
          36,
          "LPAR",
          "(",
          9
        ],
        [
          36,
          37,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": "INVALID_CRLF_MIDPOINT",
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "35": [
          1,
          23
        ],
        "36": [
          1,
          24
        ],
        "37": [
          1,
          25
        ],
        "41": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 41,
      "source_sha256": "9a29de399a43f48d36fd05de22634d7628a46a1c771e88ef1eb7e1b3830c7503",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comma-crlf",
      "source": "class Box<T,\r\nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          14,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          18,
          29,
          "FUN",
          [
            14,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          18,
          29,
          "TYPE_REFERENCE",
          [
            14,
            43
          ]
        ],
        [
          "USER_TYPE",
          18,
          28,
          "NULLABLE_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "USER_TYPE",
          18,
          21,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          18,
          21,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          22,
          28,
          "USER_TYPE",
          [
            14,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          37,
          39,
          "FUN",
          [
            14,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          14,
          30,
          37,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          12,
          12,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          18,
          21,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          21,
          22,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          22,
          28,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          29,
          30,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          14,
          17,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          17,
          18,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          18,
          21,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          21,
          22,
          "DOT",
          ".",
          4
        ],
        [
          22,
          28,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          28,
          29,
          "QUEST",
          "?",
          6
        ],
        [
          29,
          30,
          "DOT",
          ".",
          7
        ],
        [
          30,
          37,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          37,
          38,
          "LPAR",
          "(",
          9
        ],
        [
          38,
          39,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": "INVALID_CRLF_MIDPOINT",
        "14": [
          1,
          0
        ],
        "17": [
          1,
          3
        ],
        "18": [
          1,
          4
        ],
        "21": [
          1,
          7
        ],
        "22": [
          1,
          8
        ],
        "28": [
          1,
          14
        ],
        "29": [
          1,
          15
        ],
        "30": [
          1,
          16
        ],
        "37": [
          1,
          23
        ],
        "38": [
          1,
          24
        ],
        "39": [
          1,
          25
        ],
        "43": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 43,
      "source_sha256": "3d7085999370f842cf7716ce564b1986af0bba69e633a103c37ce1a77787aa78",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-unicode-crlf",
      "source": "class Box<T\r\nfun pkg.String?.`healthy 🦀é`() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          13,
          52,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            52
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            52
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            52
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            52
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            52
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            52
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          46,
          48,
          "FUN",
          [
            13,
            52
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          46,
          "`healthy 🦀é`"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          46,
          "IDENTIFIER",
          "`healthy 🦀é`",
          8
        ],
        [
          46,
          47,
          "LPAR",
          "(",
          9
        ],
        [
          47,
          48,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": "INVALID_CRLF_MIDPOINT",
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "46": [
          1,
          30
        ],
        "47": [
          1,
          31
        ],
        "48": [
          1,
          32
        ],
        "52": [
          1,
          36
        ]
      },
      "invalid_utf8_offsets": [
        39,
        40,
        41,
        44
      ],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 34
      },
      "body_owner": null,
      "bytes": 52,
      "source_sha256": "4723202773adf6d1597e987352d3259181d1e8870046ec8fc1b01f3c9dac2530",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-body-crlf",
      "source": "class Box<T\r\nfun pkg.String?.healthy() { 2 }",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          13,
          44,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            44
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            44
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            44
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            44
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": "INVALID_CRLF_MIDPOINT",
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "44": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": [
        "BLOCK",
        39,
        44,
        "FUN",
        [
          13,
          44
        ]
      ],
      "bytes": 44,
      "source_sha256": "78cd751dba177496ebfa0d15a92257a140f7ccaeba86ac30fc662e109652d331",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-leading-trivia-crlf",
      "source": "class Box<\r\n\n/*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ \nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          13,
          13243,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          13218,
          13229,
          "FUN",
          [
            13,
            13243
          ]
        ],
        [
          "NULLABLE_TYPE",
          13218,
          13229,
          "TYPE_REFERENCE",
          [
            13,
            13243
          ]
        ],
        [
          "USER_TYPE",
          13218,
          13228,
          "NULLABLE_TYPE",
          [
            13,
            13243
          ]
        ],
        [
          "USER_TYPE",
          13218,
          13221,
          "USER_TYPE",
          [
            13,
            13243
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13218,
          13221,
          "USER_TYPE",
          [
            13,
            13243
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13222,
          13228,
          "USER_TYPE",
          [
            13,
            13243
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          13237,
          13239,
          "FUN",
          [
            13,
            13243
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          13230,
          13237,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          13218,
          13221,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          13221,
          13222,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          13222,
          13228,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          13228,
          13229,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          13229,
          13230,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13214,
          13217,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          13217,
          13218,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          13218,
          13221,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          13221,
          13222,
          "DOT",
          ".",
          4
        ],
        [
          13222,
          13228,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          13228,
          13229,
          "QUEST",
          "?",
          6
        ],
        [
          13229,
          13230,
          "DOT",
          ".",
          7
        ],
        [
          13230,
          13237,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          13237,
          13238,
          "LPAR",
          "(",
          9
        ],
        [
          13238,
          13239,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": "INVALID_CRLF_MIDPOINT",
        "12": [
          1,
          0
        ],
        "13": [
          2,
          0
        ],
        "13214": [
          3,
          0
        ],
        "13217": [
          3,
          3
        ],
        "13218": [
          3,
          4
        ],
        "13221": [
          3,
          7
        ],
        "13222": [
          3,
          8
        ],
        "13228": [
          3,
          14
        ],
        "13229": [
          3,
          15
        ],
        "13230": [
          3,
          16
        ],
        "13237": [
          3,
          23
        ],
        "13238": [
          3,
          24
        ],
        "13239": [
          3,
          25
        ],
        "13243": [
          3,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 13243,
      "source_sha256": "47e36ac16f2640a2b667512831d10394f722862be51aea8cd39acc01c697258b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-cr-top",
      "source": "class Box<T\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          41,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            41
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            41
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          35,
          37,
          "FUN",
          [
            12,
            41
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          35,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          35,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          35,
          36,
          "LPAR",
          "(",
          9
        ],
        [
          36,
          37,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "35": [
          1,
          23
        ],
        "36": [
          1,
          24
        ],
        "37": [
          1,
          25
        ],
        "41": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 41,
      "source_sha256": "cf695c456d104d5d413e2f8f71a0ddb77ada42a21a7d7d6dc2979967ac8716ab",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-cr-member",
      "source": "class Host {\nclass Box<T\rfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          56,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          24,
          "CLASS_BODY",
          [
            0,
            56
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          22,
          24,
          "CLASS",
          [
            13,
            24
          ]
        ],
        [
          "TYPE_PARAMETER",
          23,
          24,
          "TYPE_PARAMETER_LIST",
          [
            13,
            24
          ]
        ],
        [
          "FUN",
          25,
          54,
          "CLASS_BODY",
          [
            0,
            56
          ]
        ],
        [
          "TYPE_REFERENCE",
          29,
          40,
          "FUN",
          [
            25,
            54
          ]
        ],
        [
          "NULLABLE_TYPE",
          29,
          40,
          "TYPE_REFERENCE",
          [
            25,
            54
          ]
        ],
        [
          "USER_TYPE",
          29,
          39,
          "NULLABLE_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "USER_TYPE",
          29,
          32,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          29,
          32,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          33,
          39,
          "USER_TYPE",
          [
            25,
            54
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          48,
          50,
          "FUN",
          [
            25,
            54
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          19,
          22,
          "Box"
        ],
        [
          25,
          41,
          48,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          24,
          24,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        23,
        24
      ],
      "receiver_tokens": [
        [
          29,
          32,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          33,
          39,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          39,
          40,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          40,
          41,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          25,
          28,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          28,
          29,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          29,
          32,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          32,
          33,
          "DOT",
          ".",
          4
        ],
        [
          33,
          39,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          39,
          40,
          "QUEST",
          "?",
          6
        ],
        [
          40,
          41,
          "DOT",
          ".",
          7
        ],
        [
          41,
          48,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          48,
          49,
          "LPAR",
          "(",
          9
        ],
        [
          49,
          50,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "19": [
          1,
          6
        ],
        "22": [
          1,
          9
        ],
        "23": [
          1,
          10
        ],
        "24": [
          1,
          11
        ],
        "25": [
          2,
          0
        ],
        "28": [
          2,
          3
        ],
        "29": [
          2,
          4
        ],
        "32": [
          2,
          7
        ],
        "33": [
          2,
          8
        ],
        "39": [
          2,
          14
        ],
        "40": [
          2,
          15
        ],
        "41": [
          2,
          16
        ],
        "48": [
          2,
          23
        ],
        "49": [
          2,
          24
        ],
        "50": [
          2,
          25
        ],
        "54": [
          2,
          29
        ],
        "56": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 56,
      "source_sha256": "d163cf87cbe218da604c953384397bb74375bf517014bca1e8f1cf4331a1b70d",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-class-cr-local",
      "source": "fun outer() {\nclass Box<T\rfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          57,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            57
          ]
        ],
        [
          "CLASS",
          14,
          25,
          "BLOCK",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          23,
          25,
          "CLASS",
          [
            14,
            25
          ]
        ],
        [
          "TYPE_PARAMETER",
          24,
          25,
          "TYPE_PARAMETER_LIST",
          [
            14,
            25
          ]
        ],
        [
          "FUN",
          26,
          55,
          "BLOCK",
          [
            0,
            57
          ]
        ],
        [
          "TYPE_REFERENCE",
          30,
          41,
          "FUN",
          [
            26,
            55
          ]
        ],
        [
          "NULLABLE_TYPE",
          30,
          41,
          "TYPE_REFERENCE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          40,
          "NULLABLE_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "USER_TYPE",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          30,
          33,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          40,
          "USER_TYPE",
          [
            26,
            55
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          49,
          51,
          "FUN",
          [
            26,
            55
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          20,
          23,
          "Box"
        ],
        [
          26,
          42,
          49,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          25,
          25,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        24,
        25
      ],
      "receiver_tokens": [
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          33,
          34,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          41,
          42,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          26,
          29,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          29,
          30,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          30,
          33,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          33,
          34,
          "DOT",
          ".",
          4
        ],
        [
          34,
          40,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          40,
          41,
          "QUEST",
          "?",
          6
        ],
        [
          41,
          42,
          "DOT",
          ".",
          7
        ],
        [
          42,
          49,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          49,
          50,
          "LPAR",
          "(",
          9
        ],
        [
          50,
          51,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "20": [
          1,
          6
        ],
        "23": [
          1,
          9
        ],
        "24": [
          1,
          10
        ],
        "25": [
          1,
          11
        ],
        "26": [
          2,
          0
        ],
        "29": [
          2,
          3
        ],
        "30": [
          2,
          4
        ],
        "33": [
          2,
          7
        ],
        "34": [
          2,
          8
        ],
        "40": [
          2,
          14
        ],
        "41": [
          2,
          15
        ],
        "42": [
          2,
          16
        ],
        "49": [
          2,
          23
        ],
        "50": [
          2,
          24
        ],
        "51": [
          2,
          25
        ],
        "55": [
          2,
          29
        ],
        "57": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 57,
      "source_sha256": "f4a89cf12220cbbeff8e1ca84f72edd9f74f513b5b853dca843b9c9a7630b459",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-class-cr",
      "source": "class Box<T>\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          13,
          42,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            42
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            42
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "42": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 42,
      "source_sha256": "ba1f839daf1a05e6b3c3ef346447878a11abe76aaa632bce91d90bdd45841fe2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-cr-top",
      "source": "interface Box<T\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          15,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          15,
          "CLASS",
          [
            0,
            15
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            15
          ]
        ],
        [
          "FUN",
          16,
          45,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          20,
          31,
          "FUN",
          [
            16,
            45
          ]
        ],
        [
          "NULLABLE_TYPE",
          20,
          31,
          "TYPE_REFERENCE",
          [
            16,
            45
          ]
        ],
        [
          "USER_TYPE",
          20,
          30,
          "NULLABLE_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "USER_TYPE",
          20,
          23,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          23,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          24,
          30,
          "USER_TYPE",
          [
            16,
            45
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          39,
          41,
          "FUN",
          [
            16,
            45
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          16,
          32,
          39,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          15,
          15,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          20,
          23,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          23,
          24,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          24,
          30,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          30,
          31,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          31,
          32,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          16,
          19,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          19,
          20,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          20,
          23,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          23,
          24,
          "DOT",
          ".",
          4
        ],
        [
          24,
          30,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          30,
          31,
          "QUEST",
          "?",
          6
        ],
        [
          31,
          32,
          "DOT",
          ".",
          7
        ],
        [
          32,
          39,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          39,
          40,
          "LPAR",
          "(",
          9
        ],
        [
          40,
          41,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": [
          1,
          0
        ],
        "19": [
          1,
          3
        ],
        "20": [
          1,
          4
        ],
        "23": [
          1,
          7
        ],
        "24": [
          1,
          8
        ],
        "30": [
          1,
          14
        ],
        "31": [
          1,
          15
        ],
        "32": [
          1,
          16
        ],
        "39": [
          1,
          23
        ],
        "40": [
          1,
          24
        ],
        "41": [
          1,
          25
        ],
        "45": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 45,
      "source_sha256": "1266a4a4b5addc4b23ecf1cb46306853ffc89241ca7faf1fef908a9c4f79a3e7",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-cr-member",
      "source": "class Host {\ninterface Box<T\rfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "CLASS",
          0,
          60,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          13,
          28,
          "CLASS_BODY",
          [
            0,
            60
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          26,
          28,
          "CLASS",
          [
            13,
            28
          ]
        ],
        [
          "TYPE_PARAMETER",
          27,
          28,
          "TYPE_PARAMETER_LIST",
          [
            13,
            28
          ]
        ],
        [
          "FUN",
          29,
          58,
          "CLASS_BODY",
          [
            0,
            60
          ]
        ],
        [
          "TYPE_REFERENCE",
          33,
          44,
          "FUN",
          [
            29,
            58
          ]
        ],
        [
          "NULLABLE_TYPE",
          33,
          44,
          "TYPE_REFERENCE",
          [
            29,
            58
          ]
        ],
        [
          "USER_TYPE",
          33,
          43,
          "NULLABLE_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "USER_TYPE",
          33,
          36,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          33,
          36,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          37,
          43,
          "USER_TYPE",
          [
            29,
            58
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          52,
          54,
          "FUN",
          [
            29,
            58
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          10,
          "Host"
        ],
        [
          13,
          23,
          26,
          "Box"
        ],
        [
          29,
          45,
          52,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          28,
          28,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        27,
        28
      ],
      "receiver_tokens": [
        [
          33,
          36,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          36,
          37,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          37,
          43,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          43,
          44,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          44,
          45,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          29,
          32,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          32,
          33,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          33,
          36,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          36,
          37,
          "DOT",
          ".",
          4
        ],
        [
          37,
          43,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          43,
          44,
          "QUEST",
          "?",
          6
        ],
        [
          44,
          45,
          "DOT",
          ".",
          7
        ],
        [
          45,
          52,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          52,
          53,
          "LPAR",
          "(",
          9
        ],
        [
          53,
          54,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "10": [
          0,
          10
        ],
        "13": [
          1,
          0
        ],
        "23": [
          1,
          10
        ],
        "26": [
          1,
          13
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          2,
          0
        ],
        "32": [
          2,
          3
        ],
        "33": [
          2,
          4
        ],
        "36": [
          2,
          7
        ],
        "37": [
          2,
          8
        ],
        "43": [
          2,
          14
        ],
        "44": [
          2,
          15
        ],
        "45": [
          2,
          16
        ],
        "52": [
          2,
          23
        ],
        "53": [
          2,
          24
        ],
        "54": [
          2,
          25
        ],
        "58": [
          2,
          29
        ],
        "60": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 60,
      "source_sha256": "f77eb4391f91f34d951a7892d38c73fe3a913fdf04c27efeffeec2ed9d6cd67c",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-interface-cr-local",
      "source": "fun outer() {\ninterface Box<T\rfun pkg.String?.healthy() = 2\n}",
      "projection": [
        [
          "FUN",
          0,
          61,
          "KT_FILE",
          null
        ],
        [
          "VALUE_PARAMETER_LIST",
          9,
          11,
          "FUN",
          [
            0,
            61
          ]
        ],
        [
          "CLASS",
          14,
          29,
          "BLOCK",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_PARAMETER_LIST",
          27,
          29,
          "CLASS",
          [
            14,
            29
          ]
        ],
        [
          "TYPE_PARAMETER",
          28,
          29,
          "TYPE_PARAMETER_LIST",
          [
            14,
            29
          ]
        ],
        [
          "FUN",
          30,
          59,
          "BLOCK",
          [
            0,
            61
          ]
        ],
        [
          "TYPE_REFERENCE",
          34,
          45,
          "FUN",
          [
            30,
            59
          ]
        ],
        [
          "NULLABLE_TYPE",
          34,
          45,
          "TYPE_REFERENCE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          44,
          "NULLABLE_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "USER_TYPE",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          37,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          38,
          44,
          "USER_TYPE",
          [
            30,
            59
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          53,
          55,
          "FUN",
          [
            30,
            59
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "outer"
        ],
        [
          14,
          24,
          27,
          "Box"
        ],
        [
          30,
          46,
          53,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          29,
          29,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        28,
        29
      ],
      "receiver_tokens": [
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          37,
          38,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          45,
          46,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          30,
          33,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          33,
          34,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          34,
          37,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          37,
          38,
          "DOT",
          ".",
          4
        ],
        [
          38,
          44,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          44,
          45,
          "QUEST",
          "?",
          6
        ],
        [
          45,
          46,
          "DOT",
          ".",
          7
        ],
        [
          46,
          53,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          53,
          54,
          "LPAR",
          "(",
          9
        ],
        [
          54,
          55,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "4": [
          0,
          4
        ],
        "9": [
          0,
          9
        ],
        "11": [
          0,
          11
        ],
        "14": [
          1,
          0
        ],
        "24": [
          1,
          10
        ],
        "27": [
          1,
          13
        ],
        "28": [
          1,
          14
        ],
        "29": [
          1,
          15
        ],
        "30": [
          2,
          0
        ],
        "33": [
          2,
          3
        ],
        "34": [
          2,
          4
        ],
        "37": [
          2,
          7
        ],
        "38": [
          2,
          8
        ],
        "44": [
          2,
          14
        ],
        "45": [
          2,
          15
        ],
        "46": [
          2,
          16
        ],
        "53": [
          2,
          23
        ],
        "54": [
          2,
          24
        ],
        "55": [
          2,
          25
        ],
        "59": [
          2,
          29
        ],
        "61": [
          3,
          1
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 61,
      "source_sha256": "3adad69117141769bd0ee1c08ab2a945868d11f9cdee739655e9ea8dcf718575",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-intact-interface-cr",
      "source": "interface Box<T>\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          16,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          13,
          16,
          "CLASS",
          [
            0,
            16
          ]
        ],
        [
          "TYPE_PARAMETER",
          14,
          15,
          "TYPE_PARAMETER_LIST",
          [
            0,
            16
          ]
        ],
        [
          "FUN",
          17,
          46,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          21,
          32,
          "FUN",
          [
            17,
            46
          ]
        ],
        [
          "NULLABLE_TYPE",
          21,
          32,
          "TYPE_REFERENCE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          31,
          "NULLABLE_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "USER_TYPE",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          24,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          25,
          31,
          "USER_TYPE",
          [
            17,
            46
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          40,
          42,
          "FUN",
          [
            17,
            46
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          10,
          13,
          "Box"
        ],
        [
          17,
          33,
          40,
          "healthy"
        ]
      ],
      "diagnostics": [],
      "type_parameter_name": [
        14,
        15
      ],
      "receiver_tokens": [
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          24,
          25,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          17,
          20,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          20,
          21,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          21,
          24,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          24,
          25,
          "DOT",
          ".",
          4
        ],
        [
          25,
          31,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          31,
          32,
          "QUEST",
          "?",
          6
        ],
        [
          32,
          33,
          "DOT",
          ".",
          7
        ],
        [
          33,
          40,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          40,
          41,
          "LPAR",
          "(",
          9
        ],
        [
          41,
          42,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "10": [
          0,
          10
        ],
        "13": [
          0,
          13
        ],
        "14": [
          0,
          14
        ],
        "15": [
          0,
          15
        ],
        "16": [
          0,
          16
        ],
        "17": [
          1,
          0
        ],
        "20": [
          1,
          3
        ],
        "21": [
          1,
          4
        ],
        "24": [
          1,
          7
        ],
        "25": [
          1,
          8
        ],
        "31": [
          1,
          14
        ],
        "32": [
          1,
          15
        ],
        "33": [
          1,
          16
        ],
        "40": [
          1,
          23
        ],
        "41": [
          1,
          24
        ],
        "42": [
          1,
          25
        ],
        "46": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 46,
      "source_sha256": "616217f939677647c9e1c38784f25979cb24756cdc33ad9c7b073c9eddfb5c2f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-spaced-cr",
      "source": "class Box<T\rfun pkg.String ? .healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          28,
          "FUN",
          [
            12,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          28,
          "TYPE_REFERENCE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          37,
          39,
          "FUN",
          [
            12,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          30,
          37,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          29,
          30,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "WHITE_SPACE",
          " ",
          6
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          7
        ],
        [
          28,
          29,
          "WHITE_SPACE",
          " ",
          8
        ],
        [
          29,
          30,
          "DOT",
          ".",
          9
        ],
        [
          30,
          37,
          "IDENTIFIER",
          "healthy",
          10
        ],
        [
          37,
          38,
          "LPAR",
          "(",
          11
        ],
        [
          38,
          39,
          "RPAR",
          ")",
          12
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "29": [
          1,
          17
        ],
        "30": [
          1,
          18
        ],
        "37": [
          1,
          25
        ],
        "38": [
          1,
          26
        ],
        "39": [
          1,
          27
        ],
        "43": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 26
      },
      "body_owner": null,
      "bytes": 43,
      "source_sha256": "bba2c84fc38aa4ea1a5c393d3e9b630302d0f0628f18b6ebd6f1d272e8bd57d0",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comment-cr",
      "source": "class Box<T\rfun pkg.String?/*x*/.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          46,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            46
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            46
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            46
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          40,
          42,
          "FUN",
          [
            12,
            46
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          33,
          40,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          32,
          33,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          32,
          "BLOCK_COMMENT",
          "/*x*/",
          7
        ],
        [
          32,
          33,
          "DOT",
          ".",
          8
        ],
        [
          33,
          40,
          "IDENTIFIER",
          "healthy",
          9
        ],
        [
          40,
          41,
          "LPAR",
          "(",
          10
        ],
        [
          41,
          42,
          "RPAR",
          ")",
          11
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "QUEST",
        "DOT",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "32": [
          1,
          20
        ],
        "33": [
          1,
          21
        ],
        "40": [
          1,
          28
        ],
        "41": [
          1,
          29
        ],
        "42": [
          1,
          30
        ],
        "46": [
          1,
          34
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 10,
        "lpar_end_from_probe_start": 29
      },
      "body_owner": null,
      "bytes": 46,
      "source_sha256": "939a2ce869b20668c860615a94050abc5944fb7eddb4adfd84dbed392887373d",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-empty-cr",
      "source": "class Box<\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          11,
          40,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          15,
          26,
          "FUN",
          [
            11,
            40
          ]
        ],
        [
          "NULLABLE_TYPE",
          15,
          26,
          "TYPE_REFERENCE",
          [
            11,
            40
          ]
        ],
        [
          "USER_TYPE",
          15,
          25,
          "NULLABLE_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "USER_TYPE",
          15,
          18,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          15,
          18,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          19,
          25,
          "USER_TYPE",
          [
            11,
            40
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          34,
          36,
          "FUN",
          [
            11,
            40
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          11,
          27,
          34,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          15,
          18,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          18,
          19,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          19,
          25,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          25,
          26,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          26,
          27,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          11,
          14,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          14,
          15,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          15,
          18,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          18,
          19,
          "DOT",
          ".",
          4
        ],
        [
          19,
          25,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          25,
          26,
          "QUEST",
          "?",
          6
        ],
        [
          26,
          27,
          "DOT",
          ".",
          7
        ],
        [
          27,
          34,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          34,
          35,
          "LPAR",
          "(",
          9
        ],
        [
          35,
          36,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          1,
          0
        ],
        "14": [
          1,
          3
        ],
        "15": [
          1,
          4
        ],
        "18": [
          1,
          7
        ],
        "19": [
          1,
          8
        ],
        "25": [
          1,
          14
        ],
        "26": [
          1,
          15
        ],
        "27": [
          1,
          16
        ],
        "34": [
          1,
          23
        ],
        "35": [
          1,
          24
        ],
        "36": [
          1,
          25
        ],
        "40": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 40,
      "source_sha256": "85287670c5a41874019fd719d4663e4698d8b81a980a07c3647597720251c40e",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-comma-cr",
      "source": "class Box<T,\rfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          12,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          12,
          "CLASS",
          [
            0,
            12
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            12
          ]
        ],
        [
          "FUN",
          13,
          42,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          17,
          28,
          "FUN",
          [
            13,
            42
          ]
        ],
        [
          "NULLABLE_TYPE",
          17,
          28,
          "TYPE_REFERENCE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          27,
          "NULLABLE_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "USER_TYPE",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          17,
          20,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          21,
          27,
          "USER_TYPE",
          [
            13,
            42
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          36,
          38,
          "FUN",
          [
            13,
            42
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          13,
          29,
          36,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          12,
          12,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          20,
          21,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          28,
          29,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13,
          16,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          16,
          17,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          17,
          20,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          20,
          21,
          "DOT",
          ".",
          4
        ],
        [
          21,
          27,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          27,
          28,
          "QUEST",
          "?",
          6
        ],
        [
          28,
          29,
          "DOT",
          ".",
          7
        ],
        [
          29,
          36,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          36,
          37,
          "LPAR",
          "(",
          9
        ],
        [
          37,
          38,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          0,
          12
        ],
        "13": [
          1,
          0
        ],
        "16": [
          1,
          3
        ],
        "17": [
          1,
          4
        ],
        "20": [
          1,
          7
        ],
        "21": [
          1,
          8
        ],
        "27": [
          1,
          14
        ],
        "28": [
          1,
          15
        ],
        "29": [
          1,
          16
        ],
        "36": [
          1,
          23
        ],
        "37": [
          1,
          24
        ],
        "38": [
          1,
          25
        ],
        "42": [
          1,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 42,
      "source_sha256": "17a3b6be4d34ae0580e358a93e3e0648856fccb774329aae6f88f09bbe85d984",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-unicode-cr",
      "source": "class Box<T\rfun pkg.String?.`healthy 🦀é`() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          51,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            51
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            51
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            51
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          45,
          47,
          "FUN",
          [
            12,
            51
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          45,
          "`healthy 🦀é`"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          45,
          "IDENTIFIER",
          "`healthy 🦀é`",
          8
        ],
        [
          45,
          46,
          "LPAR",
          "(",
          9
        ],
        [
          46,
          47,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "45": [
          1,
          30
        ],
        "46": [
          1,
          31
        ],
        "47": [
          1,
          32
        ],
        "51": [
          1,
          36
        ]
      },
      "invalid_utf8_offsets": [
        38,
        39,
        40,
        43
      ],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 34
      },
      "body_owner": null,
      "bytes": 51,
      "source_sha256": "0276831d525e9594a917e9e9a295628b99b0df067c659ebed90d1aaeb312717a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-body-cr",
      "source": "class Box<T\rfun pkg.String?.healthy() { 2 }",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          43,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          16,
          27,
          "FUN",
          [
            12,
            43
          ]
        ],
        [
          "NULLABLE_TYPE",
          16,
          27,
          "TYPE_REFERENCE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          26,
          "NULLABLE_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "USER_TYPE",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          16,
          19,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          20,
          26,
          "USER_TYPE",
          [
            12,
            43
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          35,
          37,
          "FUN",
          [
            12,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          28,
          35,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          19,
          20,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          27,
          28,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          19,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          19,
          20,
          "DOT",
          ".",
          4
        ],
        [
          20,
          26,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          26,
          27,
          "QUEST",
          "?",
          6
        ],
        [
          27,
          28,
          "DOT",
          ".",
          7
        ],
        [
          28,
          35,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          35,
          36,
          "LPAR",
          "(",
          9
        ],
        [
          36,
          37,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "20": [
          1,
          8
        ],
        "26": [
          1,
          14
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "35": [
          1,
          23
        ],
        "36": [
          1,
          24
        ],
        "37": [
          1,
          25
        ],
        "43": [
          1,
          31
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": [
        "BLOCK",
        38,
        43,
        "FUN",
        [
          12,
          43
        ]
      ],
      "bytes": 43,
      "source_sha256": "6faed8827a4676f2452cb87384d0292cf59bc638a47bee322feed72a293857e6",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-leading-trivia-cr",
      "source": "class Box<\r\n/*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ \nfun pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          10,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          10,
          "CLASS",
          [
            0,
            10
          ]
        ],
        [
          "FUN",
          12,
          13242,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          13217,
          13228,
          "FUN",
          [
            12,
            13242
          ]
        ],
        [
          "NULLABLE_TYPE",
          13217,
          13228,
          "TYPE_REFERENCE",
          [
            12,
            13242
          ]
        ],
        [
          "USER_TYPE",
          13217,
          13227,
          "NULLABLE_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "USER_TYPE",
          13217,
          13220,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13217,
          13220,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          13221,
          13227,
          "USER_TYPE",
          [
            12,
            13242
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          13236,
          13238,
          "FUN",
          [
            12,
            13242
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          13229,
          13236,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
      ],
      "type_parameter_name": null,
      "receiver_tokens": [
        [
          13217,
          13220,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          13220,
          13221,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          13221,
          13227,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          13227,
          13228,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          13228,
          13229,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          13213,
          13216,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          13216,
          13217,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          13217,
          13220,
          "IDENTIFIER",
          "pkg",
          3
        ],
        [
          13220,
          13221,
          "DOT",
          ".",
          4
        ],
        [
          13221,
          13227,
          "IDENTIFIER",
          "String",
          5
        ],
        [
          13227,
          13228,
          "QUEST",
          "?",
          6
        ],
        [
          13228,
          13229,
          "DOT",
          ".",
          7
        ],
        [
          13229,
          13236,
          "IDENTIFIER",
          "healthy",
          8
        ],
        [
          13236,
          13237,
          "LPAR",
          "(",
          9
        ],
        [
          13237,
          13238,
          "RPAR",
          ")",
          10
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": "INVALID_CRLF_MIDPOINT",
        "12": [
          1,
          0
        ],
        "13213": [
          2,
          0
        ],
        "13216": [
          2,
          3
        ],
        "13217": [
          2,
          4
        ],
        "13220": [
          2,
          7
        ],
        "13221": [
          2,
          8
        ],
        "13227": [
          2,
          14
        ],
        "13228": [
          2,
          15
        ],
        "13229": [
          2,
          16
        ],
        "13236": [
          2,
          23
        ],
        "13237": [
          2,
          24
        ],
        "13238": [
          2,
          25
        ],
        "13242": [
          2,
          29
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 9,
        "lpar_end_from_probe_start": 24
      },
      "body_owner": null,
      "bytes": 13242,
      "source_sha256": "3fba66a27f39b0ff7737b622a0c9a2d9bac0149f0b35e33c349d32f5b3dcadd0",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-raw255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          779,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          754,
          765,
          "FUN",
          [
            12,
            779
          ]
        ],
        [
          "NULLABLE_TYPE",
          754,
          765,
          "TYPE_REFERENCE",
          [
            12,
            779
          ]
        ],
        [
          "USER_TYPE",
          754,
          764,
          "NULLABLE_TYPE",
          [
            12,
            779
          ]
        ],
        [
          "USER_TYPE",
          754,
          757,
          "USER_TYPE",
          [
            12,
            779
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          754,
          757,
          "USER_TYPE",
          [
            12,
            779
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          758,
          764,
          "USER_TYPE",
          [
            12,
            779
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          773,
          775,
          "FUN",
          [
            12,
            779
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          766,
          773,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          754,
          757,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          757,
          758,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          758,
          764,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          764,
          765,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          765,
          766,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          21,
          "BLOCK_COMMENT",
          "/*x*/",
          3
        ],
        [
          21,
          22,
          "WHITE_SPACE",
          " ",
          4
        ],
        [
          22,
          27,
          "BLOCK_COMMENT",
          "/*x*/",
          5
        ],
        [
          27,
          28,
          "WHITE_SPACE",
          " ",
          6
        ],
        [
          28,
          33,
          "BLOCK_COMMENT",
          "/*x*/",
          7
        ],
        [
          33,
          34,
          "WHITE_SPACE",
          " ",
          8
        ],
        [
          34,
          39,
          "BLOCK_COMMENT",
          "/*x*/",
          9
        ],
        [
          39,
          40,
          "WHITE_SPACE",
          " ",
          10
        ],
        [
          40,
          45,
          "BLOCK_COMMENT",
          "/*x*/",
          11
        ],
        [
          45,
          46,
          "WHITE_SPACE",
          " ",
          12
        ],
        [
          46,
          51,
          "BLOCK_COMMENT",
          "/*x*/",
          13
        ],
        [
          51,
          52,
          "WHITE_SPACE",
          " ",
          14
        ],
        [
          52,
          57,
          "BLOCK_COMMENT",
          "/*x*/",
          15
        ],
        [
          57,
          58,
          "WHITE_SPACE",
          " ",
          16
        ],
        [
          58,
          63,
          "BLOCK_COMMENT",
          "/*x*/",
          17
        ],
        [
          63,
          64,
          "WHITE_SPACE",
          " ",
          18
        ],
        [
          64,
          69,
          "BLOCK_COMMENT",
          "/*x*/",
          19
        ],
        [
          69,
          70,
          "WHITE_SPACE",
          " ",
          20
        ],
        [
          70,
          75,
          "BLOCK_COMMENT",
          "/*x*/",
          21
        ],
        [
          75,
          76,
          "WHITE_SPACE",
          " ",
          22
        ],
        [
          76,
          81,
          "BLOCK_COMMENT",
          "/*x*/",
          23
        ],
        [
          81,
          82,
          "WHITE_SPACE",
          " ",
          24
        ],
        [
          82,
          87,
          "BLOCK_COMMENT",
          "/*x*/",
          25
        ],
        [
          87,
          88,
          "WHITE_SPACE",
          " ",
          26
        ],
        [
          88,
          93,
          "BLOCK_COMMENT",
          "/*x*/",
          27
        ],
        [
          93,
          94,
          "WHITE_SPACE",
          " ",
          28
        ],
        [
          94,
          99,
          "BLOCK_COMMENT",
          "/*x*/",
          29
        ],
        [
          99,
          100,
          "WHITE_SPACE",
          " ",
          30
        ],
        [
          100,
          105,
          "BLOCK_COMMENT",
          "/*x*/",
          31
        ],
        [
          105,
          106,
          "WHITE_SPACE",
          " ",
          32
        ],
        [
          106,
          111,
          "BLOCK_COMMENT",
          "/*x*/",
          33
        ],
        [
          111,
          112,
          "WHITE_SPACE",
          " ",
          34
        ],
        [
          112,
          117,
          "BLOCK_COMMENT",
          "/*x*/",
          35
        ],
        [
          117,
          118,
          "WHITE_SPACE",
          " ",
          36
        ],
        [
          118,
          123,
          "BLOCK_COMMENT",
          "/*x*/",
          37
        ],
        [
          123,
          124,
          "WHITE_SPACE",
          " ",
          38
        ],
        [
          124,
          129,
          "BLOCK_COMMENT",
          "/*x*/",
          39
        ],
        [
          129,
          130,
          "WHITE_SPACE",
          " ",
          40
        ],
        [
          130,
          135,
          "BLOCK_COMMENT",
          "/*x*/",
          41
        ],
        [
          135,
          136,
          "WHITE_SPACE",
          " ",
          42
        ],
        [
          136,
          141,
          "BLOCK_COMMENT",
          "/*x*/",
          43
        ],
        [
          141,
          142,
          "WHITE_SPACE",
          " ",
          44
        ],
        [
          142,
          147,
          "BLOCK_COMMENT",
          "/*x*/",
          45
        ],
        [
          147,
          148,
          "WHITE_SPACE",
          " ",
          46
        ],
        [
          148,
          153,
          "BLOCK_COMMENT",
          "/*x*/",
          47
        ],
        [
          153,
          154,
          "WHITE_SPACE",
          " ",
          48
        ],
        [
          154,
          159,
          "BLOCK_COMMENT",
          "/*x*/",
          49
        ],
        [
          159,
          160,
          "WHITE_SPACE",
          " ",
          50
        ],
        [
          160,
          165,
          "BLOCK_COMMENT",
          "/*x*/",
          51
        ],
        [
          165,
          166,
          "WHITE_SPACE",
          " ",
          52
        ],
        [
          166,
          171,
          "BLOCK_COMMENT",
          "/*x*/",
          53
        ],
        [
          171,
          172,
          "WHITE_SPACE",
          " ",
          54
        ],
        [
          172,
          177,
          "BLOCK_COMMENT",
          "/*x*/",
          55
        ],
        [
          177,
          178,
          "WHITE_SPACE",
          " ",
          56
        ],
        [
          178,
          183,
          "BLOCK_COMMENT",
          "/*x*/",
          57
        ],
        [
          183,
          184,
          "WHITE_SPACE",
          " ",
          58
        ],
        [
          184,
          189,
          "BLOCK_COMMENT",
          "/*x*/",
          59
        ],
        [
          189,
          190,
          "WHITE_SPACE",
          " ",
          60
        ],
        [
          190,
          195,
          "BLOCK_COMMENT",
          "/*x*/",
          61
        ],
        [
          195,
          196,
          "WHITE_SPACE",
          " ",
          62
        ],
        [
          196,
          201,
          "BLOCK_COMMENT",
          "/*x*/",
          63
        ],
        [
          201,
          202,
          "WHITE_SPACE",
          " ",
          64
        ],
        [
          202,
          207,
          "BLOCK_COMMENT",
          "/*x*/",
          65
        ],
        [
          207,
          208,
          "WHITE_SPACE",
          " ",
          66
        ],
        [
          208,
          213,
          "BLOCK_COMMENT",
          "/*x*/",
          67
        ],
        [
          213,
          214,
          "WHITE_SPACE",
          " ",
          68
        ],
        [
          214,
          219,
          "BLOCK_COMMENT",
          "/*x*/",
          69
        ],
        [
          219,
          220,
          "WHITE_SPACE",
          " ",
          70
        ],
        [
          220,
          225,
          "BLOCK_COMMENT",
          "/*x*/",
          71
        ],
        [
          225,
          226,
          "WHITE_SPACE",
          " ",
          72
        ],
        [
          226,
          231,
          "BLOCK_COMMENT",
          "/*x*/",
          73
        ],
        [
          231,
          232,
          "WHITE_SPACE",
          " ",
          74
        ],
        [
          232,
          237,
          "BLOCK_COMMENT",
          "/*x*/",
          75
        ],
        [
          237,
          238,
          "WHITE_SPACE",
          " ",
          76
        ],
        [
          238,
          243,
          "BLOCK_COMMENT",
          "/*x*/",
          77
        ],
        [
          243,
          244,
          "WHITE_SPACE",
          " ",
          78
        ],
        [
          244,
          249,
          "BLOCK_COMMENT",
          "/*x*/",
          79
        ],
        [
          249,
          250,
          "WHITE_SPACE",
          " ",
          80
        ],
        [
          250,
          255,
          "BLOCK_COMMENT",
          "/*x*/",
          81
        ],
        [
          255,
          256,
          "WHITE_SPACE",
          " ",
          82
        ],
        [
          256,
          261,
          "BLOCK_COMMENT",
          "/*x*/",
          83
        ],
        [
          261,
          262,
          "WHITE_SPACE",
          " ",
          84
        ],
        [
          262,
          267,
          "BLOCK_COMMENT",
          "/*x*/",
          85
        ],
        [
          267,
          268,
          "WHITE_SPACE",
          " ",
          86
        ],
        [
          268,
          273,
          "BLOCK_COMMENT",
          "/*x*/",
          87
        ],
        [
          273,
          274,
          "WHITE_SPACE",
          " ",
          88
        ],
        [
          274,
          279,
          "BLOCK_COMMENT",
          "/*x*/",
          89
        ],
        [
          279,
          280,
          "WHITE_SPACE",
          " ",
          90
        ],
        [
          280,
          285,
          "BLOCK_COMMENT",
          "/*x*/",
          91
        ],
        [
          285,
          286,
          "WHITE_SPACE",
          " ",
          92
        ],
        [
          286,
          291,
          "BLOCK_COMMENT",
          "/*x*/",
          93
        ],
        [
          291,
          292,
          "WHITE_SPACE",
          " ",
          94
        ],
        [
          292,
          297,
          "BLOCK_COMMENT",
          "/*x*/",
          95
        ],
        [
          297,
          298,
          "WHITE_SPACE",
          " ",
          96
        ],
        [
          298,
          303,
          "BLOCK_COMMENT",
          "/*x*/",
          97
        ],
        [
          303,
          304,
          "WHITE_SPACE",
          " ",
          98
        ],
        [
          304,
          309,
          "BLOCK_COMMENT",
          "/*x*/",
          99
        ],
        [
          309,
          310,
          "WHITE_SPACE",
          " ",
          100
        ],
        [
          310,
          315,
          "BLOCK_COMMENT",
          "/*x*/",
          101
        ],
        [
          315,
          316,
          "WHITE_SPACE",
          " ",
          102
        ],
        [
          316,
          321,
          "BLOCK_COMMENT",
          "/*x*/",
          103
        ],
        [
          321,
          322,
          "WHITE_SPACE",
          " ",
          104
        ],
        [
          322,
          327,
          "BLOCK_COMMENT",
          "/*x*/",
          105
        ],
        [
          327,
          328,
          "WHITE_SPACE",
          " ",
          106
        ],
        [
          328,
          333,
          "BLOCK_COMMENT",
          "/*x*/",
          107
        ],
        [
          333,
          334,
          "WHITE_SPACE",
          " ",
          108
        ],
        [
          334,
          339,
          "BLOCK_COMMENT",
          "/*x*/",
          109
        ],
        [
          339,
          340,
          "WHITE_SPACE",
          " ",
          110
        ],
        [
          340,
          345,
          "BLOCK_COMMENT",
          "/*x*/",
          111
        ],
        [
          345,
          346,
          "WHITE_SPACE",
          " ",
          112
        ],
        [
          346,
          351,
          "BLOCK_COMMENT",
          "/*x*/",
          113
        ],
        [
          351,
          352,
          "WHITE_SPACE",
          " ",
          114
        ],
        [
          352,
          357,
          "BLOCK_COMMENT",
          "/*x*/",
          115
        ],
        [
          357,
          358,
          "WHITE_SPACE",
          " ",
          116
        ],
        [
          358,
          363,
          "BLOCK_COMMENT",
          "/*x*/",
          117
        ],
        [
          363,
          364,
          "WHITE_SPACE",
          " ",
          118
        ],
        [
          364,
          369,
          "BLOCK_COMMENT",
          "/*x*/",
          119
        ],
        [
          369,
          370,
          "WHITE_SPACE",
          " ",
          120
        ],
        [
          370,
          375,
          "BLOCK_COMMENT",
          "/*x*/",
          121
        ],
        [
          375,
          376,
          "WHITE_SPACE",
          " ",
          122
        ],
        [
          376,
          381,
          "BLOCK_COMMENT",
          "/*x*/",
          123
        ],
        [
          381,
          382,
          "WHITE_SPACE",
          " ",
          124
        ],
        [
          382,
          387,
          "BLOCK_COMMENT",
          "/*x*/",
          125
        ],
        [
          387,
          388,
          "WHITE_SPACE",
          " ",
          126
        ],
        [
          388,
          393,
          "BLOCK_COMMENT",
          "/*x*/",
          127
        ],
        [
          393,
          394,
          "WHITE_SPACE",
          " ",
          128
        ],
        [
          394,
          399,
          "BLOCK_COMMENT",
          "/*x*/",
          129
        ],
        [
          399,
          400,
          "WHITE_SPACE",
          " ",
          130
        ],
        [
          400,
          405,
          "BLOCK_COMMENT",
          "/*x*/",
          131
        ],
        [
          405,
          406,
          "WHITE_SPACE",
          " ",
          132
        ],
        [
          406,
          411,
          "BLOCK_COMMENT",
          "/*x*/",
          133
        ],
        [
          411,
          412,
          "WHITE_SPACE",
          " ",
          134
        ],
        [
          412,
          417,
          "BLOCK_COMMENT",
          "/*x*/",
          135
        ],
        [
          417,
          418,
          "WHITE_SPACE",
          " ",
          136
        ],
        [
          418,
          423,
          "BLOCK_COMMENT",
          "/*x*/",
          137
        ],
        [
          423,
          424,
          "WHITE_SPACE",
          " ",
          138
        ],
        [
          424,
          429,
          "BLOCK_COMMENT",
          "/*x*/",
          139
        ],
        [
          429,
          430,
          "WHITE_SPACE",
          " ",
          140
        ],
        [
          430,
          435,
          "BLOCK_COMMENT",
          "/*x*/",
          141
        ],
        [
          435,
          436,
          "WHITE_SPACE",
          " ",
          142
        ],
        [
          436,
          441,
          "BLOCK_COMMENT",
          "/*x*/",
          143
        ],
        [
          441,
          442,
          "WHITE_SPACE",
          " ",
          144
        ],
        [
          442,
          447,
          "BLOCK_COMMENT",
          "/*x*/",
          145
        ],
        [
          447,
          448,
          "WHITE_SPACE",
          " ",
          146
        ],
        [
          448,
          453,
          "BLOCK_COMMENT",
          "/*x*/",
          147
        ],
        [
          453,
          454,
          "WHITE_SPACE",
          " ",
          148
        ],
        [
          454,
          459,
          "BLOCK_COMMENT",
          "/*x*/",
          149
        ],
        [
          459,
          460,
          "WHITE_SPACE",
          " ",
          150
        ],
        [
          460,
          465,
          "BLOCK_COMMENT",
          "/*x*/",
          151
        ],
        [
          465,
          466,
          "WHITE_SPACE",
          " ",
          152
        ],
        [
          466,
          471,
          "BLOCK_COMMENT",
          "/*x*/",
          153
        ],
        [
          471,
          472,
          "WHITE_SPACE",
          " ",
          154
        ],
        [
          472,
          477,
          "BLOCK_COMMENT",
          "/*x*/",
          155
        ],
        [
          477,
          478,
          "WHITE_SPACE",
          " ",
          156
        ],
        [
          478,
          483,
          "BLOCK_COMMENT",
          "/*x*/",
          157
        ],
        [
          483,
          484,
          "WHITE_SPACE",
          " ",
          158
        ],
        [
          484,
          489,
          "BLOCK_COMMENT",
          "/*x*/",
          159
        ],
        [
          489,
          490,
          "WHITE_SPACE",
          " ",
          160
        ],
        [
          490,
          495,
          "BLOCK_COMMENT",
          "/*x*/",
          161
        ],
        [
          495,
          496,
          "WHITE_SPACE",
          " ",
          162
        ],
        [
          496,
          501,
          "BLOCK_COMMENT",
          "/*x*/",
          163
        ],
        [
          501,
          502,
          "WHITE_SPACE",
          " ",
          164
        ],
        [
          502,
          507,
          "BLOCK_COMMENT",
          "/*x*/",
          165
        ],
        [
          507,
          508,
          "WHITE_SPACE",
          " ",
          166
        ],
        [
          508,
          513,
          "BLOCK_COMMENT",
          "/*x*/",
          167
        ],
        [
          513,
          514,
          "WHITE_SPACE",
          " ",
          168
        ],
        [
          514,
          519,
          "BLOCK_COMMENT",
          "/*x*/",
          169
        ],
        [
          519,
          520,
          "WHITE_SPACE",
          " ",
          170
        ],
        [
          520,
          525,
          "BLOCK_COMMENT",
          "/*x*/",
          171
        ],
        [
          525,
          526,
          "WHITE_SPACE",
          " ",
          172
        ],
        [
          526,
          531,
          "BLOCK_COMMENT",
          "/*x*/",
          173
        ],
        [
          531,
          532,
          "WHITE_SPACE",
          " ",
          174
        ],
        [
          532,
          537,
          "BLOCK_COMMENT",
          "/*x*/",
          175
        ],
        [
          537,
          538,
          "WHITE_SPACE",
          " ",
          176
        ],
        [
          538,
          543,
          "BLOCK_COMMENT",
          "/*x*/",
          177
        ],
        [
          543,
          544,
          "WHITE_SPACE",
          " ",
          178
        ],
        [
          544,
          549,
          "BLOCK_COMMENT",
          "/*x*/",
          179
        ],
        [
          549,
          550,
          "WHITE_SPACE",
          " ",
          180
        ],
        [
          550,
          555,
          "BLOCK_COMMENT",
          "/*x*/",
          181
        ],
        [
          555,
          556,
          "WHITE_SPACE",
          " ",
          182
        ],
        [
          556,
          561,
          "BLOCK_COMMENT",
          "/*x*/",
          183
        ],
        [
          561,
          562,
          "WHITE_SPACE",
          " ",
          184
        ],
        [
          562,
          567,
          "BLOCK_COMMENT",
          "/*x*/",
          185
        ],
        [
          567,
          568,
          "WHITE_SPACE",
          " ",
          186
        ],
        [
          568,
          573,
          "BLOCK_COMMENT",
          "/*x*/",
          187
        ],
        [
          573,
          574,
          "WHITE_SPACE",
          " ",
          188
        ],
        [
          574,
          579,
          "BLOCK_COMMENT",
          "/*x*/",
          189
        ],
        [
          579,
          580,
          "WHITE_SPACE",
          " ",
          190
        ],
        [
          580,
          585,
          "BLOCK_COMMENT",
          "/*x*/",
          191
        ],
        [
          585,
          586,
          "WHITE_SPACE",
          " ",
          192
        ],
        [
          586,
          591,
          "BLOCK_COMMENT",
          "/*x*/",
          193
        ],
        [
          591,
          592,
          "WHITE_SPACE",
          " ",
          194
        ],
        [
          592,
          597,
          "BLOCK_COMMENT",
          "/*x*/",
          195
        ],
        [
          597,
          598,
          "WHITE_SPACE",
          " ",
          196
        ],
        [
          598,
          603,
          "BLOCK_COMMENT",
          "/*x*/",
          197
        ],
        [
          603,
          604,
          "WHITE_SPACE",
          " ",
          198
        ],
        [
          604,
          609,
          "BLOCK_COMMENT",
          "/*x*/",
          199
        ],
        [
          609,
          610,
          "WHITE_SPACE",
          " ",
          200
        ],
        [
          610,
          615,
          "BLOCK_COMMENT",
          "/*x*/",
          201
        ],
        [
          615,
          616,
          "WHITE_SPACE",
          " ",
          202
        ],
        [
          616,
          621,
          "BLOCK_COMMENT",
          "/*x*/",
          203
        ],
        [
          621,
          622,
          "WHITE_SPACE",
          " ",
          204
        ],
        [
          622,
          627,
          "BLOCK_COMMENT",
          "/*x*/",
          205
        ],
        [
          627,
          628,
          "WHITE_SPACE",
          " ",
          206
        ],
        [
          628,
          633,
          "BLOCK_COMMENT",
          "/*x*/",
          207
        ],
        [
          633,
          634,
          "WHITE_SPACE",
          " ",
          208
        ],
        [
          634,
          639,
          "BLOCK_COMMENT",
          "/*x*/",
          209
        ],
        [
          639,
          640,
          "WHITE_SPACE",
          " ",
          210
        ],
        [
          640,
          645,
          "BLOCK_COMMENT",
          "/*x*/",
          211
        ],
        [
          645,
          646,
          "WHITE_SPACE",
          " ",
          212
        ],
        [
          646,
          651,
          "BLOCK_COMMENT",
          "/*x*/",
          213
        ],
        [
          651,
          652,
          "WHITE_SPACE",
          " ",
          214
        ],
        [
          652,
          657,
          "BLOCK_COMMENT",
          "/*x*/",
          215
        ],
        [
          657,
          658,
          "WHITE_SPACE",
          " ",
          216
        ],
        [
          658,
          663,
          "BLOCK_COMMENT",
          "/*x*/",
          217
        ],
        [
          663,
          664,
          "WHITE_SPACE",
          " ",
          218
        ],
        [
          664,
          669,
          "BLOCK_COMMENT",
          "/*x*/",
          219
        ],
        [
          669,
          670,
          "WHITE_SPACE",
          " ",
          220
        ],
        [
          670,
          675,
          "BLOCK_COMMENT",
          "/*x*/",
          221
        ],
        [
          675,
          676,
          "WHITE_SPACE",
          " ",
          222
        ],
        [
          676,
          681,
          "BLOCK_COMMENT",
          "/*x*/",
          223
        ],
        [
          681,
          682,
          "WHITE_SPACE",
          " ",
          224
        ],
        [
          682,
          687,
          "BLOCK_COMMENT",
          "/*x*/",
          225
        ],
        [
          687,
          688,
          "WHITE_SPACE",
          " ",
          226
        ],
        [
          688,
          693,
          "BLOCK_COMMENT",
          "/*x*/",
          227
        ],
        [
          693,
          694,
          "WHITE_SPACE",
          " ",
          228
        ],
        [
          694,
          699,
          "BLOCK_COMMENT",
          "/*x*/",
          229
        ],
        [
          699,
          700,
          "WHITE_SPACE",
          " ",
          230
        ],
        [
          700,
          705,
          "BLOCK_COMMENT",
          "/*x*/",
          231
        ],
        [
          705,
          706,
          "WHITE_SPACE",
          " ",
          232
        ],
        [
          706,
          711,
          "BLOCK_COMMENT",
          "/*x*/",
          233
        ],
        [
          711,
          712,
          "WHITE_SPACE",
          " ",
          234
        ],
        [
          712,
          717,
          "BLOCK_COMMENT",
          "/*x*/",
          235
        ],
        [
          717,
          718,
          "WHITE_SPACE",
          " ",
          236
        ],
        [
          718,
          723,
          "BLOCK_COMMENT",
          "/*x*/",
          237
        ],
        [
          723,
          724,
          "WHITE_SPACE",
          " ",
          238
        ],
        [
          724,
          729,
          "BLOCK_COMMENT",
          "/*x*/",
          239
        ],
        [
          729,
          730,
          "WHITE_SPACE",
          " ",
          240
        ],
        [
          730,
          735,
          "BLOCK_COMMENT",
          "/*x*/",
          241
        ],
        [
          735,
          736,
          "WHITE_SPACE",
          " ",
          242
        ],
        [
          736,
          741,
          "BLOCK_COMMENT",
          "/*x*/",
          243
        ],
        [
          741,
          742,
          "WHITE_SPACE",
          " ",
          244
        ],
        [
          742,
          747,
          "BLOCK_COMMENT",
          "/*x*/",
          245
        ],
        [
          747,
          748,
          "WHITE_SPACE",
          " ",
          246
        ],
        [
          748,
          753,
          "BLOCK_COMMENT",
          "/*x*/",
          247
        ],
        [
          753,
          754,
          "WHITE_SPACE",
          " ",
          248
        ],
        [
          754,
          757,
          "IDENTIFIER",
          "pkg",
          249
        ],
        [
          757,
          758,
          "DOT",
          ".",
          250
        ],
        [
          758,
          764,
          "IDENTIFIER",
          "String",
          251
        ],
        [
          764,
          765,
          "QUEST",
          "?",
          252
        ],
        [
          765,
          766,
          "DOT",
          ".",
          253
        ],
        [
          766,
          773,
          "IDENTIFIER",
          "healthy",
          254
        ],
        [
          773,
          774,
          "LPAR",
          "(",
          255
        ],
        [
          774,
          775,
          "RPAR",
          ")",
          256
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "22": [
          1,
          10
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "33": [
          1,
          21
        ],
        "34": [
          1,
          22
        ],
        "39": [
          1,
          27
        ],
        "40": [
          1,
          28
        ],
        "45": [
          1,
          33
        ],
        "46": [
          1,
          34
        ],
        "51": [
          1,
          39
        ],
        "52": [
          1,
          40
        ],
        "57": [
          1,
          45
        ],
        "58": [
          1,
          46
        ],
        "63": [
          1,
          51
        ],
        "64": [
          1,
          52
        ],
        "69": [
          1,
          57
        ],
        "70": [
          1,
          58
        ],
        "75": [
          1,
          63
        ],
        "76": [
          1,
          64
        ],
        "81": [
          1,
          69
        ],
        "82": [
          1,
          70
        ],
        "87": [
          1,
          75
        ],
        "88": [
          1,
          76
        ],
        "93": [
          1,
          81
        ],
        "94": [
          1,
          82
        ],
        "99": [
          1,
          87
        ],
        "100": [
          1,
          88
        ],
        "105": [
          1,
          93
        ],
        "106": [
          1,
          94
        ],
        "111": [
          1,
          99
        ],
        "112": [
          1,
          100
        ],
        "117": [
          1,
          105
        ],
        "118": [
          1,
          106
        ],
        "123": [
          1,
          111
        ],
        "124": [
          1,
          112
        ],
        "129": [
          1,
          117
        ],
        "130": [
          1,
          118
        ],
        "135": [
          1,
          123
        ],
        "136": [
          1,
          124
        ],
        "141": [
          1,
          129
        ],
        "142": [
          1,
          130
        ],
        "147": [
          1,
          135
        ],
        "148": [
          1,
          136
        ],
        "153": [
          1,
          141
        ],
        "154": [
          1,
          142
        ],
        "159": [
          1,
          147
        ],
        "160": [
          1,
          148
        ],
        "165": [
          1,
          153
        ],
        "166": [
          1,
          154
        ],
        "171": [
          1,
          159
        ],
        "172": [
          1,
          160
        ],
        "177": [
          1,
          165
        ],
        "178": [
          1,
          166
        ],
        "183": [
          1,
          171
        ],
        "184": [
          1,
          172
        ],
        "189": [
          1,
          177
        ],
        "190": [
          1,
          178
        ],
        "195": [
          1,
          183
        ],
        "196": [
          1,
          184
        ],
        "201": [
          1,
          189
        ],
        "202": [
          1,
          190
        ],
        "207": [
          1,
          195
        ],
        "208": [
          1,
          196
        ],
        "213": [
          1,
          201
        ],
        "214": [
          1,
          202
        ],
        "219": [
          1,
          207
        ],
        "220": [
          1,
          208
        ],
        "225": [
          1,
          213
        ],
        "226": [
          1,
          214
        ],
        "231": [
          1,
          219
        ],
        "232": [
          1,
          220
        ],
        "237": [
          1,
          225
        ],
        "238": [
          1,
          226
        ],
        "243": [
          1,
          231
        ],
        "244": [
          1,
          232
        ],
        "249": [
          1,
          237
        ],
        "250": [
          1,
          238
        ],
        "255": [
          1,
          243
        ],
        "256": [
          1,
          244
        ],
        "261": [
          1,
          249
        ],
        "262": [
          1,
          250
        ],
        "267": [
          1,
          255
        ],
        "268": [
          1,
          256
        ],
        "273": [
          1,
          261
        ],
        "274": [
          1,
          262
        ],
        "279": [
          1,
          267
        ],
        "280": [
          1,
          268
        ],
        "285": [
          1,
          273
        ],
        "286": [
          1,
          274
        ],
        "291": [
          1,
          279
        ],
        "292": [
          1,
          280
        ],
        "297": [
          1,
          285
        ],
        "298": [
          1,
          286
        ],
        "303": [
          1,
          291
        ],
        "304": [
          1,
          292
        ],
        "309": [
          1,
          297
        ],
        "310": [
          1,
          298
        ],
        "315": [
          1,
          303
        ],
        "316": [
          1,
          304
        ],
        "321": [
          1,
          309
        ],
        "322": [
          1,
          310
        ],
        "327": [
          1,
          315
        ],
        "328": [
          1,
          316
        ],
        "333": [
          1,
          321
        ],
        "334": [
          1,
          322
        ],
        "339": [
          1,
          327
        ],
        "340": [
          1,
          328
        ],
        "345": [
          1,
          333
        ],
        "346": [
          1,
          334
        ],
        "351": [
          1,
          339
        ],
        "352": [
          1,
          340
        ],
        "357": [
          1,
          345
        ],
        "358": [
          1,
          346
        ],
        "363": [
          1,
          351
        ],
        "364": [
          1,
          352
        ],
        "369": [
          1,
          357
        ],
        "370": [
          1,
          358
        ],
        "375": [
          1,
          363
        ],
        "376": [
          1,
          364
        ],
        "381": [
          1,
          369
        ],
        "382": [
          1,
          370
        ],
        "387": [
          1,
          375
        ],
        "388": [
          1,
          376
        ],
        "393": [
          1,
          381
        ],
        "394": [
          1,
          382
        ],
        "399": [
          1,
          387
        ],
        "400": [
          1,
          388
        ],
        "405": [
          1,
          393
        ],
        "406": [
          1,
          394
        ],
        "411": [
          1,
          399
        ],
        "412": [
          1,
          400
        ],
        "417": [
          1,
          405
        ],
        "418": [
          1,
          406
        ],
        "423": [
          1,
          411
        ],
        "424": [
          1,
          412
        ],
        "429": [
          1,
          417
        ],
        "430": [
          1,
          418
        ],
        "435": [
          1,
          423
        ],
        "436": [
          1,
          424
        ],
        "441": [
          1,
          429
        ],
        "442": [
          1,
          430
        ],
        "447": [
          1,
          435
        ],
        "448": [
          1,
          436
        ],
        "453": [
          1,
          441
        ],
        "454": [
          1,
          442
        ],
        "459": [
          1,
          447
        ],
        "460": [
          1,
          448
        ],
        "465": [
          1,
          453
        ],
        "466": [
          1,
          454
        ],
        "471": [
          1,
          459
        ],
        "472": [
          1,
          460
        ],
        "477": [
          1,
          465
        ],
        "478": [
          1,
          466
        ],
        "483": [
          1,
          471
        ],
        "484": [
          1,
          472
        ],
        "489": [
          1,
          477
        ],
        "490": [
          1,
          478
        ],
        "495": [
          1,
          483
        ],
        "496": [
          1,
          484
        ],
        "501": [
          1,
          489
        ],
        "502": [
          1,
          490
        ],
        "507": [
          1,
          495
        ],
        "508": [
          1,
          496
        ],
        "513": [
          1,
          501
        ],
        "514": [
          1,
          502
        ],
        "519": [
          1,
          507
        ],
        "520": [
          1,
          508
        ],
        "525": [
          1,
          513
        ],
        "526": [
          1,
          514
        ],
        "531": [
          1,
          519
        ],
        "532": [
          1,
          520
        ],
        "537": [
          1,
          525
        ],
        "538": [
          1,
          526
        ],
        "543": [
          1,
          531
        ],
        "544": [
          1,
          532
        ],
        "549": [
          1,
          537
        ],
        "550": [
          1,
          538
        ],
        "555": [
          1,
          543
        ],
        "556": [
          1,
          544
        ],
        "561": [
          1,
          549
        ],
        "562": [
          1,
          550
        ],
        "567": [
          1,
          555
        ],
        "568": [
          1,
          556
        ],
        "573": [
          1,
          561
        ],
        "574": [
          1,
          562
        ],
        "579": [
          1,
          567
        ],
        "580": [
          1,
          568
        ],
        "585": [
          1,
          573
        ],
        "586": [
          1,
          574
        ],
        "591": [
          1,
          579
        ],
        "592": [
          1,
          580
        ],
        "597": [
          1,
          585
        ],
        "598": [
          1,
          586
        ],
        "603": [
          1,
          591
        ],
        "604": [
          1,
          592
        ],
        "609": [
          1,
          597
        ],
        "610": [
          1,
          598
        ],
        "615": [
          1,
          603
        ],
        "616": [
          1,
          604
        ],
        "621": [
          1,
          609
        ],
        "622": [
          1,
          610
        ],
        "627": [
          1,
          615
        ],
        "628": [
          1,
          616
        ],
        "633": [
          1,
          621
        ],
        "634": [
          1,
          622
        ],
        "639": [
          1,
          627
        ],
        "640": [
          1,
          628
        ],
        "645": [
          1,
          633
        ],
        "646": [
          1,
          634
        ],
        "651": [
          1,
          639
        ],
        "652": [
          1,
          640
        ],
        "657": [
          1,
          645
        ],
        "658": [
          1,
          646
        ],
        "663": [
          1,
          651
        ],
        "664": [
          1,
          652
        ],
        "669": [
          1,
          657
        ],
        "670": [
          1,
          658
        ],
        "675": [
          1,
          663
        ],
        "676": [
          1,
          664
        ],
        "681": [
          1,
          669
        ],
        "682": [
          1,
          670
        ],
        "687": [
          1,
          675
        ],
        "688": [
          1,
          676
        ],
        "693": [
          1,
          681
        ],
        "694": [
          1,
          682
        ],
        "699": [
          1,
          687
        ],
        "700": [
          1,
          688
        ],
        "705": [
          1,
          693
        ],
        "706": [
          1,
          694
        ],
        "711": [
          1,
          699
        ],
        "712": [
          1,
          700
        ],
        "717": [
          1,
          705
        ],
        "718": [
          1,
          706
        ],
        "723": [
          1,
          711
        ],
        "724": [
          1,
          712
        ],
        "729": [
          1,
          717
        ],
        "730": [
          1,
          718
        ],
        "735": [
          1,
          723
        ],
        "736": [
          1,
          724
        ],
        "741": [
          1,
          729
        ],
        "742": [
          1,
          730
        ],
        "747": [
          1,
          735
        ],
        "748": [
          1,
          736
        ],
        "753": [
          1,
          741
        ],
        "754": [
          1,
          742
        ],
        "757": [
          1,
          745
        ],
        "758": [
          1,
          746
        ],
        "764": [
          1,
          752
        ],
        "765": [
          1,
          753
        ],
        "766": [
          1,
          754
        ],
        "773": [
          1,
          761
        ],
        "774": [
          1,
          762
        ],
        "775": [
          1,
          763
        ],
        "779": [
          1,
          767
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 255,
        "lpar_end_from_probe_start": 762
      },
      "body_owner": null,
      "bytes": 779,
      "source_sha256": "9b74996c52be8bde6a1a2890105771b62032fe2214308f2dbaff4ac8ddf5abc1",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-raw256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          784,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          759,
          770,
          "FUN",
          [
            12,
            784
          ]
        ],
        [
          "NULLABLE_TYPE",
          759,
          770,
          "TYPE_REFERENCE",
          [
            12,
            784
          ]
        ],
        [
          "USER_TYPE",
          759,
          769,
          "NULLABLE_TYPE",
          [
            12,
            784
          ]
        ],
        [
          "USER_TYPE",
          759,
          762,
          "USER_TYPE",
          [
            12,
            784
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          759,
          762,
          "USER_TYPE",
          [
            12,
            784
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          763,
          769,
          "USER_TYPE",
          [
            12,
            784
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          778,
          780,
          "FUN",
          [
            12,
            784
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          771,
          778,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          759,
          762,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          762,
          763,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          763,
          769,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          769,
          770,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          770,
          771,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          21,
          "BLOCK_COMMENT",
          "/*x*/",
          3
        ],
        [
          21,
          22,
          "WHITE_SPACE",
          " ",
          4
        ],
        [
          22,
          27,
          "BLOCK_COMMENT",
          "/*x*/",
          5
        ],
        [
          27,
          28,
          "WHITE_SPACE",
          " ",
          6
        ],
        [
          28,
          33,
          "BLOCK_COMMENT",
          "/*x*/",
          7
        ],
        [
          33,
          34,
          "WHITE_SPACE",
          " ",
          8
        ],
        [
          34,
          39,
          "BLOCK_COMMENT",
          "/*x*/",
          9
        ],
        [
          39,
          40,
          "WHITE_SPACE",
          " ",
          10
        ],
        [
          40,
          45,
          "BLOCK_COMMENT",
          "/*x*/",
          11
        ],
        [
          45,
          46,
          "WHITE_SPACE",
          " ",
          12
        ],
        [
          46,
          51,
          "BLOCK_COMMENT",
          "/*x*/",
          13
        ],
        [
          51,
          52,
          "WHITE_SPACE",
          " ",
          14
        ],
        [
          52,
          57,
          "BLOCK_COMMENT",
          "/*x*/",
          15
        ],
        [
          57,
          58,
          "WHITE_SPACE",
          " ",
          16
        ],
        [
          58,
          63,
          "BLOCK_COMMENT",
          "/*x*/",
          17
        ],
        [
          63,
          64,
          "WHITE_SPACE",
          " ",
          18
        ],
        [
          64,
          69,
          "BLOCK_COMMENT",
          "/*x*/",
          19
        ],
        [
          69,
          70,
          "WHITE_SPACE",
          " ",
          20
        ],
        [
          70,
          75,
          "BLOCK_COMMENT",
          "/*x*/",
          21
        ],
        [
          75,
          76,
          "WHITE_SPACE",
          " ",
          22
        ],
        [
          76,
          81,
          "BLOCK_COMMENT",
          "/*x*/",
          23
        ],
        [
          81,
          82,
          "WHITE_SPACE",
          " ",
          24
        ],
        [
          82,
          87,
          "BLOCK_COMMENT",
          "/*x*/",
          25
        ],
        [
          87,
          88,
          "WHITE_SPACE",
          " ",
          26
        ],
        [
          88,
          93,
          "BLOCK_COMMENT",
          "/*x*/",
          27
        ],
        [
          93,
          94,
          "WHITE_SPACE",
          " ",
          28
        ],
        [
          94,
          99,
          "BLOCK_COMMENT",
          "/*x*/",
          29
        ],
        [
          99,
          100,
          "WHITE_SPACE",
          " ",
          30
        ],
        [
          100,
          105,
          "BLOCK_COMMENT",
          "/*x*/",
          31
        ],
        [
          105,
          106,
          "WHITE_SPACE",
          " ",
          32
        ],
        [
          106,
          111,
          "BLOCK_COMMENT",
          "/*x*/",
          33
        ],
        [
          111,
          112,
          "WHITE_SPACE",
          " ",
          34
        ],
        [
          112,
          117,
          "BLOCK_COMMENT",
          "/*x*/",
          35
        ],
        [
          117,
          118,
          "WHITE_SPACE",
          " ",
          36
        ],
        [
          118,
          123,
          "BLOCK_COMMENT",
          "/*x*/",
          37
        ],
        [
          123,
          124,
          "WHITE_SPACE",
          " ",
          38
        ],
        [
          124,
          129,
          "BLOCK_COMMENT",
          "/*x*/",
          39
        ],
        [
          129,
          130,
          "WHITE_SPACE",
          " ",
          40
        ],
        [
          130,
          135,
          "BLOCK_COMMENT",
          "/*x*/",
          41
        ],
        [
          135,
          136,
          "WHITE_SPACE",
          " ",
          42
        ],
        [
          136,
          141,
          "BLOCK_COMMENT",
          "/*x*/",
          43
        ],
        [
          141,
          142,
          "WHITE_SPACE",
          " ",
          44
        ],
        [
          142,
          147,
          "BLOCK_COMMENT",
          "/*x*/",
          45
        ],
        [
          147,
          148,
          "WHITE_SPACE",
          " ",
          46
        ],
        [
          148,
          153,
          "BLOCK_COMMENT",
          "/*x*/",
          47
        ],
        [
          153,
          154,
          "WHITE_SPACE",
          " ",
          48
        ],
        [
          154,
          159,
          "BLOCK_COMMENT",
          "/*x*/",
          49
        ],
        [
          159,
          160,
          "WHITE_SPACE",
          " ",
          50
        ],
        [
          160,
          165,
          "BLOCK_COMMENT",
          "/*x*/",
          51
        ],
        [
          165,
          166,
          "WHITE_SPACE",
          " ",
          52
        ],
        [
          166,
          171,
          "BLOCK_COMMENT",
          "/*x*/",
          53
        ],
        [
          171,
          172,
          "WHITE_SPACE",
          " ",
          54
        ],
        [
          172,
          177,
          "BLOCK_COMMENT",
          "/*x*/",
          55
        ],
        [
          177,
          178,
          "WHITE_SPACE",
          " ",
          56
        ],
        [
          178,
          183,
          "BLOCK_COMMENT",
          "/*x*/",
          57
        ],
        [
          183,
          184,
          "WHITE_SPACE",
          " ",
          58
        ],
        [
          184,
          189,
          "BLOCK_COMMENT",
          "/*x*/",
          59
        ],
        [
          189,
          190,
          "WHITE_SPACE",
          " ",
          60
        ],
        [
          190,
          195,
          "BLOCK_COMMENT",
          "/*x*/",
          61
        ],
        [
          195,
          196,
          "WHITE_SPACE",
          " ",
          62
        ],
        [
          196,
          201,
          "BLOCK_COMMENT",
          "/*x*/",
          63
        ],
        [
          201,
          202,
          "WHITE_SPACE",
          " ",
          64
        ],
        [
          202,
          207,
          "BLOCK_COMMENT",
          "/*x*/",
          65
        ],
        [
          207,
          208,
          "WHITE_SPACE",
          " ",
          66
        ],
        [
          208,
          213,
          "BLOCK_COMMENT",
          "/*x*/",
          67
        ],
        [
          213,
          214,
          "WHITE_SPACE",
          " ",
          68
        ],
        [
          214,
          219,
          "BLOCK_COMMENT",
          "/*x*/",
          69
        ],
        [
          219,
          220,
          "WHITE_SPACE",
          " ",
          70
        ],
        [
          220,
          225,
          "BLOCK_COMMENT",
          "/*x*/",
          71
        ],
        [
          225,
          226,
          "WHITE_SPACE",
          " ",
          72
        ],
        [
          226,
          231,
          "BLOCK_COMMENT",
          "/*x*/",
          73
        ],
        [
          231,
          232,
          "WHITE_SPACE",
          " ",
          74
        ],
        [
          232,
          237,
          "BLOCK_COMMENT",
          "/*x*/",
          75
        ],
        [
          237,
          238,
          "WHITE_SPACE",
          " ",
          76
        ],
        [
          238,
          243,
          "BLOCK_COMMENT",
          "/*x*/",
          77
        ],
        [
          243,
          244,
          "WHITE_SPACE",
          " ",
          78
        ],
        [
          244,
          249,
          "BLOCK_COMMENT",
          "/*x*/",
          79
        ],
        [
          249,
          250,
          "WHITE_SPACE",
          " ",
          80
        ],
        [
          250,
          255,
          "BLOCK_COMMENT",
          "/*x*/",
          81
        ],
        [
          255,
          256,
          "WHITE_SPACE",
          " ",
          82
        ],
        [
          256,
          261,
          "BLOCK_COMMENT",
          "/*x*/",
          83
        ],
        [
          261,
          262,
          "WHITE_SPACE",
          " ",
          84
        ],
        [
          262,
          267,
          "BLOCK_COMMENT",
          "/*x*/",
          85
        ],
        [
          267,
          268,
          "WHITE_SPACE",
          " ",
          86
        ],
        [
          268,
          273,
          "BLOCK_COMMENT",
          "/*x*/",
          87
        ],
        [
          273,
          274,
          "WHITE_SPACE",
          " ",
          88
        ],
        [
          274,
          279,
          "BLOCK_COMMENT",
          "/*x*/",
          89
        ],
        [
          279,
          280,
          "WHITE_SPACE",
          " ",
          90
        ],
        [
          280,
          285,
          "BLOCK_COMMENT",
          "/*x*/",
          91
        ],
        [
          285,
          286,
          "WHITE_SPACE",
          " ",
          92
        ],
        [
          286,
          291,
          "BLOCK_COMMENT",
          "/*x*/",
          93
        ],
        [
          291,
          292,
          "WHITE_SPACE",
          " ",
          94
        ],
        [
          292,
          297,
          "BLOCK_COMMENT",
          "/*x*/",
          95
        ],
        [
          297,
          298,
          "WHITE_SPACE",
          " ",
          96
        ],
        [
          298,
          303,
          "BLOCK_COMMENT",
          "/*x*/",
          97
        ],
        [
          303,
          304,
          "WHITE_SPACE",
          " ",
          98
        ],
        [
          304,
          309,
          "BLOCK_COMMENT",
          "/*x*/",
          99
        ],
        [
          309,
          310,
          "WHITE_SPACE",
          " ",
          100
        ],
        [
          310,
          315,
          "BLOCK_COMMENT",
          "/*x*/",
          101
        ],
        [
          315,
          316,
          "WHITE_SPACE",
          " ",
          102
        ],
        [
          316,
          321,
          "BLOCK_COMMENT",
          "/*x*/",
          103
        ],
        [
          321,
          322,
          "WHITE_SPACE",
          " ",
          104
        ],
        [
          322,
          327,
          "BLOCK_COMMENT",
          "/*x*/",
          105
        ],
        [
          327,
          328,
          "WHITE_SPACE",
          " ",
          106
        ],
        [
          328,
          333,
          "BLOCK_COMMENT",
          "/*x*/",
          107
        ],
        [
          333,
          334,
          "WHITE_SPACE",
          " ",
          108
        ],
        [
          334,
          339,
          "BLOCK_COMMENT",
          "/*x*/",
          109
        ],
        [
          339,
          340,
          "WHITE_SPACE",
          " ",
          110
        ],
        [
          340,
          345,
          "BLOCK_COMMENT",
          "/*x*/",
          111
        ],
        [
          345,
          346,
          "WHITE_SPACE",
          " ",
          112
        ],
        [
          346,
          351,
          "BLOCK_COMMENT",
          "/*x*/",
          113
        ],
        [
          351,
          352,
          "WHITE_SPACE",
          " ",
          114
        ],
        [
          352,
          357,
          "BLOCK_COMMENT",
          "/*x*/",
          115
        ],
        [
          357,
          358,
          "WHITE_SPACE",
          " ",
          116
        ],
        [
          358,
          363,
          "BLOCK_COMMENT",
          "/*x*/",
          117
        ],
        [
          363,
          364,
          "WHITE_SPACE",
          " ",
          118
        ],
        [
          364,
          369,
          "BLOCK_COMMENT",
          "/*x*/",
          119
        ],
        [
          369,
          370,
          "WHITE_SPACE",
          " ",
          120
        ],
        [
          370,
          375,
          "BLOCK_COMMENT",
          "/*x*/",
          121
        ],
        [
          375,
          376,
          "WHITE_SPACE",
          " ",
          122
        ],
        [
          376,
          381,
          "BLOCK_COMMENT",
          "/*x*/",
          123
        ],
        [
          381,
          382,
          "WHITE_SPACE",
          " ",
          124
        ],
        [
          382,
          387,
          "BLOCK_COMMENT",
          "/*x*/",
          125
        ],
        [
          387,
          388,
          "WHITE_SPACE",
          " ",
          126
        ],
        [
          388,
          393,
          "BLOCK_COMMENT",
          "/*x*/",
          127
        ],
        [
          393,
          394,
          "WHITE_SPACE",
          " ",
          128
        ],
        [
          394,
          399,
          "BLOCK_COMMENT",
          "/*x*/",
          129
        ],
        [
          399,
          400,
          "WHITE_SPACE",
          " ",
          130
        ],
        [
          400,
          405,
          "BLOCK_COMMENT",
          "/*x*/",
          131
        ],
        [
          405,
          406,
          "WHITE_SPACE",
          " ",
          132
        ],
        [
          406,
          411,
          "BLOCK_COMMENT",
          "/*x*/",
          133
        ],
        [
          411,
          412,
          "WHITE_SPACE",
          " ",
          134
        ],
        [
          412,
          417,
          "BLOCK_COMMENT",
          "/*x*/",
          135
        ],
        [
          417,
          418,
          "WHITE_SPACE",
          " ",
          136
        ],
        [
          418,
          423,
          "BLOCK_COMMENT",
          "/*x*/",
          137
        ],
        [
          423,
          424,
          "WHITE_SPACE",
          " ",
          138
        ],
        [
          424,
          429,
          "BLOCK_COMMENT",
          "/*x*/",
          139
        ],
        [
          429,
          430,
          "WHITE_SPACE",
          " ",
          140
        ],
        [
          430,
          435,
          "BLOCK_COMMENT",
          "/*x*/",
          141
        ],
        [
          435,
          436,
          "WHITE_SPACE",
          " ",
          142
        ],
        [
          436,
          441,
          "BLOCK_COMMENT",
          "/*x*/",
          143
        ],
        [
          441,
          442,
          "WHITE_SPACE",
          " ",
          144
        ],
        [
          442,
          447,
          "BLOCK_COMMENT",
          "/*x*/",
          145
        ],
        [
          447,
          448,
          "WHITE_SPACE",
          " ",
          146
        ],
        [
          448,
          453,
          "BLOCK_COMMENT",
          "/*x*/",
          147
        ],
        [
          453,
          454,
          "WHITE_SPACE",
          " ",
          148
        ],
        [
          454,
          459,
          "BLOCK_COMMENT",
          "/*x*/",
          149
        ],
        [
          459,
          460,
          "WHITE_SPACE",
          " ",
          150
        ],
        [
          460,
          465,
          "BLOCK_COMMENT",
          "/*x*/",
          151
        ],
        [
          465,
          466,
          "WHITE_SPACE",
          " ",
          152
        ],
        [
          466,
          471,
          "BLOCK_COMMENT",
          "/*x*/",
          153
        ],
        [
          471,
          472,
          "WHITE_SPACE",
          " ",
          154
        ],
        [
          472,
          477,
          "BLOCK_COMMENT",
          "/*x*/",
          155
        ],
        [
          477,
          478,
          "WHITE_SPACE",
          " ",
          156
        ],
        [
          478,
          483,
          "BLOCK_COMMENT",
          "/*x*/",
          157
        ],
        [
          483,
          484,
          "WHITE_SPACE",
          " ",
          158
        ],
        [
          484,
          489,
          "BLOCK_COMMENT",
          "/*x*/",
          159
        ],
        [
          489,
          490,
          "WHITE_SPACE",
          " ",
          160
        ],
        [
          490,
          495,
          "BLOCK_COMMENT",
          "/*x*/",
          161
        ],
        [
          495,
          496,
          "WHITE_SPACE",
          " ",
          162
        ],
        [
          496,
          501,
          "BLOCK_COMMENT",
          "/*x*/",
          163
        ],
        [
          501,
          502,
          "WHITE_SPACE",
          " ",
          164
        ],
        [
          502,
          507,
          "BLOCK_COMMENT",
          "/*x*/",
          165
        ],
        [
          507,
          508,
          "WHITE_SPACE",
          " ",
          166
        ],
        [
          508,
          513,
          "BLOCK_COMMENT",
          "/*x*/",
          167
        ],
        [
          513,
          514,
          "WHITE_SPACE",
          " ",
          168
        ],
        [
          514,
          519,
          "BLOCK_COMMENT",
          "/*x*/",
          169
        ],
        [
          519,
          520,
          "WHITE_SPACE",
          " ",
          170
        ],
        [
          520,
          525,
          "BLOCK_COMMENT",
          "/*x*/",
          171
        ],
        [
          525,
          526,
          "WHITE_SPACE",
          " ",
          172
        ],
        [
          526,
          531,
          "BLOCK_COMMENT",
          "/*x*/",
          173
        ],
        [
          531,
          532,
          "WHITE_SPACE",
          " ",
          174
        ],
        [
          532,
          537,
          "BLOCK_COMMENT",
          "/*x*/",
          175
        ],
        [
          537,
          538,
          "WHITE_SPACE",
          " ",
          176
        ],
        [
          538,
          543,
          "BLOCK_COMMENT",
          "/*x*/",
          177
        ],
        [
          543,
          544,
          "WHITE_SPACE",
          " ",
          178
        ],
        [
          544,
          549,
          "BLOCK_COMMENT",
          "/*x*/",
          179
        ],
        [
          549,
          550,
          "WHITE_SPACE",
          " ",
          180
        ],
        [
          550,
          555,
          "BLOCK_COMMENT",
          "/*x*/",
          181
        ],
        [
          555,
          556,
          "WHITE_SPACE",
          " ",
          182
        ],
        [
          556,
          561,
          "BLOCK_COMMENT",
          "/*x*/",
          183
        ],
        [
          561,
          562,
          "WHITE_SPACE",
          " ",
          184
        ],
        [
          562,
          567,
          "BLOCK_COMMENT",
          "/*x*/",
          185
        ],
        [
          567,
          568,
          "WHITE_SPACE",
          " ",
          186
        ],
        [
          568,
          573,
          "BLOCK_COMMENT",
          "/*x*/",
          187
        ],
        [
          573,
          574,
          "WHITE_SPACE",
          " ",
          188
        ],
        [
          574,
          579,
          "BLOCK_COMMENT",
          "/*x*/",
          189
        ],
        [
          579,
          580,
          "WHITE_SPACE",
          " ",
          190
        ],
        [
          580,
          585,
          "BLOCK_COMMENT",
          "/*x*/",
          191
        ],
        [
          585,
          586,
          "WHITE_SPACE",
          " ",
          192
        ],
        [
          586,
          591,
          "BLOCK_COMMENT",
          "/*x*/",
          193
        ],
        [
          591,
          592,
          "WHITE_SPACE",
          " ",
          194
        ],
        [
          592,
          597,
          "BLOCK_COMMENT",
          "/*x*/",
          195
        ],
        [
          597,
          598,
          "WHITE_SPACE",
          " ",
          196
        ],
        [
          598,
          603,
          "BLOCK_COMMENT",
          "/*x*/",
          197
        ],
        [
          603,
          604,
          "WHITE_SPACE",
          " ",
          198
        ],
        [
          604,
          609,
          "BLOCK_COMMENT",
          "/*x*/",
          199
        ],
        [
          609,
          610,
          "WHITE_SPACE",
          " ",
          200
        ],
        [
          610,
          615,
          "BLOCK_COMMENT",
          "/*x*/",
          201
        ],
        [
          615,
          616,
          "WHITE_SPACE",
          " ",
          202
        ],
        [
          616,
          621,
          "BLOCK_COMMENT",
          "/*x*/",
          203
        ],
        [
          621,
          622,
          "WHITE_SPACE",
          " ",
          204
        ],
        [
          622,
          627,
          "BLOCK_COMMENT",
          "/*x*/",
          205
        ],
        [
          627,
          628,
          "WHITE_SPACE",
          " ",
          206
        ],
        [
          628,
          633,
          "BLOCK_COMMENT",
          "/*x*/",
          207
        ],
        [
          633,
          634,
          "WHITE_SPACE",
          " ",
          208
        ],
        [
          634,
          639,
          "BLOCK_COMMENT",
          "/*x*/",
          209
        ],
        [
          639,
          640,
          "WHITE_SPACE",
          " ",
          210
        ],
        [
          640,
          645,
          "BLOCK_COMMENT",
          "/*x*/",
          211
        ],
        [
          645,
          646,
          "WHITE_SPACE",
          " ",
          212
        ],
        [
          646,
          651,
          "BLOCK_COMMENT",
          "/*x*/",
          213
        ],
        [
          651,
          652,
          "WHITE_SPACE",
          " ",
          214
        ],
        [
          652,
          657,
          "BLOCK_COMMENT",
          "/*x*/",
          215
        ],
        [
          657,
          658,
          "WHITE_SPACE",
          " ",
          216
        ],
        [
          658,
          663,
          "BLOCK_COMMENT",
          "/*x*/",
          217
        ],
        [
          663,
          664,
          "WHITE_SPACE",
          " ",
          218
        ],
        [
          664,
          669,
          "BLOCK_COMMENT",
          "/*x*/",
          219
        ],
        [
          669,
          670,
          "WHITE_SPACE",
          " ",
          220
        ],
        [
          670,
          675,
          "BLOCK_COMMENT",
          "/*x*/",
          221
        ],
        [
          675,
          676,
          "WHITE_SPACE",
          " ",
          222
        ],
        [
          676,
          681,
          "BLOCK_COMMENT",
          "/*x*/",
          223
        ],
        [
          681,
          682,
          "WHITE_SPACE",
          " ",
          224
        ],
        [
          682,
          687,
          "BLOCK_COMMENT",
          "/*x*/",
          225
        ],
        [
          687,
          688,
          "WHITE_SPACE",
          " ",
          226
        ],
        [
          688,
          693,
          "BLOCK_COMMENT",
          "/*x*/",
          227
        ],
        [
          693,
          694,
          "WHITE_SPACE",
          " ",
          228
        ],
        [
          694,
          699,
          "BLOCK_COMMENT",
          "/*x*/",
          229
        ],
        [
          699,
          700,
          "WHITE_SPACE",
          " ",
          230
        ],
        [
          700,
          705,
          "BLOCK_COMMENT",
          "/*x*/",
          231
        ],
        [
          705,
          706,
          "WHITE_SPACE",
          " ",
          232
        ],
        [
          706,
          711,
          "BLOCK_COMMENT",
          "/*x*/",
          233
        ],
        [
          711,
          712,
          "WHITE_SPACE",
          " ",
          234
        ],
        [
          712,
          717,
          "BLOCK_COMMENT",
          "/*x*/",
          235
        ],
        [
          717,
          718,
          "WHITE_SPACE",
          " ",
          236
        ],
        [
          718,
          723,
          "BLOCK_COMMENT",
          "/*x*/",
          237
        ],
        [
          723,
          724,
          "WHITE_SPACE",
          " ",
          238
        ],
        [
          724,
          729,
          "BLOCK_COMMENT",
          "/*x*/",
          239
        ],
        [
          729,
          730,
          "WHITE_SPACE",
          " ",
          240
        ],
        [
          730,
          735,
          "BLOCK_COMMENT",
          "/*x*/",
          241
        ],
        [
          735,
          736,
          "WHITE_SPACE",
          " ",
          242
        ],
        [
          736,
          741,
          "BLOCK_COMMENT",
          "/*x*/",
          243
        ],
        [
          741,
          742,
          "WHITE_SPACE",
          " ",
          244
        ],
        [
          742,
          747,
          "BLOCK_COMMENT",
          "/*x*/",
          245
        ],
        [
          747,
          748,
          "WHITE_SPACE",
          " ",
          246
        ],
        [
          748,
          753,
          "BLOCK_COMMENT",
          "/*x*/",
          247
        ],
        [
          753,
          754,
          "WHITE_SPACE",
          " ",
          248
        ],
        [
          754,
          759,
          "BLOCK_COMMENT",
          "/*y*/",
          249
        ],
        [
          759,
          762,
          "IDENTIFIER",
          "pkg",
          250
        ],
        [
          762,
          763,
          "DOT",
          ".",
          251
        ],
        [
          763,
          769,
          "IDENTIFIER",
          "String",
          252
        ],
        [
          769,
          770,
          "QUEST",
          "?",
          253
        ],
        [
          770,
          771,
          "DOT",
          ".",
          254
        ],
        [
          771,
          778,
          "IDENTIFIER",
          "healthy",
          255
        ],
        [
          778,
          779,
          "LPAR",
          "(",
          256
        ],
        [
          779,
          780,
          "RPAR",
          ")",
          257
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "22": [
          1,
          10
        ],
        "27": [
          1,
          15
        ],
        "28": [
          1,
          16
        ],
        "33": [
          1,
          21
        ],
        "34": [
          1,
          22
        ],
        "39": [
          1,
          27
        ],
        "40": [
          1,
          28
        ],
        "45": [
          1,
          33
        ],
        "46": [
          1,
          34
        ],
        "51": [
          1,
          39
        ],
        "52": [
          1,
          40
        ],
        "57": [
          1,
          45
        ],
        "58": [
          1,
          46
        ],
        "63": [
          1,
          51
        ],
        "64": [
          1,
          52
        ],
        "69": [
          1,
          57
        ],
        "70": [
          1,
          58
        ],
        "75": [
          1,
          63
        ],
        "76": [
          1,
          64
        ],
        "81": [
          1,
          69
        ],
        "82": [
          1,
          70
        ],
        "87": [
          1,
          75
        ],
        "88": [
          1,
          76
        ],
        "93": [
          1,
          81
        ],
        "94": [
          1,
          82
        ],
        "99": [
          1,
          87
        ],
        "100": [
          1,
          88
        ],
        "105": [
          1,
          93
        ],
        "106": [
          1,
          94
        ],
        "111": [
          1,
          99
        ],
        "112": [
          1,
          100
        ],
        "117": [
          1,
          105
        ],
        "118": [
          1,
          106
        ],
        "123": [
          1,
          111
        ],
        "124": [
          1,
          112
        ],
        "129": [
          1,
          117
        ],
        "130": [
          1,
          118
        ],
        "135": [
          1,
          123
        ],
        "136": [
          1,
          124
        ],
        "141": [
          1,
          129
        ],
        "142": [
          1,
          130
        ],
        "147": [
          1,
          135
        ],
        "148": [
          1,
          136
        ],
        "153": [
          1,
          141
        ],
        "154": [
          1,
          142
        ],
        "159": [
          1,
          147
        ],
        "160": [
          1,
          148
        ],
        "165": [
          1,
          153
        ],
        "166": [
          1,
          154
        ],
        "171": [
          1,
          159
        ],
        "172": [
          1,
          160
        ],
        "177": [
          1,
          165
        ],
        "178": [
          1,
          166
        ],
        "183": [
          1,
          171
        ],
        "184": [
          1,
          172
        ],
        "189": [
          1,
          177
        ],
        "190": [
          1,
          178
        ],
        "195": [
          1,
          183
        ],
        "196": [
          1,
          184
        ],
        "201": [
          1,
          189
        ],
        "202": [
          1,
          190
        ],
        "207": [
          1,
          195
        ],
        "208": [
          1,
          196
        ],
        "213": [
          1,
          201
        ],
        "214": [
          1,
          202
        ],
        "219": [
          1,
          207
        ],
        "220": [
          1,
          208
        ],
        "225": [
          1,
          213
        ],
        "226": [
          1,
          214
        ],
        "231": [
          1,
          219
        ],
        "232": [
          1,
          220
        ],
        "237": [
          1,
          225
        ],
        "238": [
          1,
          226
        ],
        "243": [
          1,
          231
        ],
        "244": [
          1,
          232
        ],
        "249": [
          1,
          237
        ],
        "250": [
          1,
          238
        ],
        "255": [
          1,
          243
        ],
        "256": [
          1,
          244
        ],
        "261": [
          1,
          249
        ],
        "262": [
          1,
          250
        ],
        "267": [
          1,
          255
        ],
        "268": [
          1,
          256
        ],
        "273": [
          1,
          261
        ],
        "274": [
          1,
          262
        ],
        "279": [
          1,
          267
        ],
        "280": [
          1,
          268
        ],
        "285": [
          1,
          273
        ],
        "286": [
          1,
          274
        ],
        "291": [
          1,
          279
        ],
        "292": [
          1,
          280
        ],
        "297": [
          1,
          285
        ],
        "298": [
          1,
          286
        ],
        "303": [
          1,
          291
        ],
        "304": [
          1,
          292
        ],
        "309": [
          1,
          297
        ],
        "310": [
          1,
          298
        ],
        "315": [
          1,
          303
        ],
        "316": [
          1,
          304
        ],
        "321": [
          1,
          309
        ],
        "322": [
          1,
          310
        ],
        "327": [
          1,
          315
        ],
        "328": [
          1,
          316
        ],
        "333": [
          1,
          321
        ],
        "334": [
          1,
          322
        ],
        "339": [
          1,
          327
        ],
        "340": [
          1,
          328
        ],
        "345": [
          1,
          333
        ],
        "346": [
          1,
          334
        ],
        "351": [
          1,
          339
        ],
        "352": [
          1,
          340
        ],
        "357": [
          1,
          345
        ],
        "358": [
          1,
          346
        ],
        "363": [
          1,
          351
        ],
        "364": [
          1,
          352
        ],
        "369": [
          1,
          357
        ],
        "370": [
          1,
          358
        ],
        "375": [
          1,
          363
        ],
        "376": [
          1,
          364
        ],
        "381": [
          1,
          369
        ],
        "382": [
          1,
          370
        ],
        "387": [
          1,
          375
        ],
        "388": [
          1,
          376
        ],
        "393": [
          1,
          381
        ],
        "394": [
          1,
          382
        ],
        "399": [
          1,
          387
        ],
        "400": [
          1,
          388
        ],
        "405": [
          1,
          393
        ],
        "406": [
          1,
          394
        ],
        "411": [
          1,
          399
        ],
        "412": [
          1,
          400
        ],
        "417": [
          1,
          405
        ],
        "418": [
          1,
          406
        ],
        "423": [
          1,
          411
        ],
        "424": [
          1,
          412
        ],
        "429": [
          1,
          417
        ],
        "430": [
          1,
          418
        ],
        "435": [
          1,
          423
        ],
        "436": [
          1,
          424
        ],
        "441": [
          1,
          429
        ],
        "442": [
          1,
          430
        ],
        "447": [
          1,
          435
        ],
        "448": [
          1,
          436
        ],
        "453": [
          1,
          441
        ],
        "454": [
          1,
          442
        ],
        "459": [
          1,
          447
        ],
        "460": [
          1,
          448
        ],
        "465": [
          1,
          453
        ],
        "466": [
          1,
          454
        ],
        "471": [
          1,
          459
        ],
        "472": [
          1,
          460
        ],
        "477": [
          1,
          465
        ],
        "478": [
          1,
          466
        ],
        "483": [
          1,
          471
        ],
        "484": [
          1,
          472
        ],
        "489": [
          1,
          477
        ],
        "490": [
          1,
          478
        ],
        "495": [
          1,
          483
        ],
        "496": [
          1,
          484
        ],
        "501": [
          1,
          489
        ],
        "502": [
          1,
          490
        ],
        "507": [
          1,
          495
        ],
        "508": [
          1,
          496
        ],
        "513": [
          1,
          501
        ],
        "514": [
          1,
          502
        ],
        "519": [
          1,
          507
        ],
        "520": [
          1,
          508
        ],
        "525": [
          1,
          513
        ],
        "526": [
          1,
          514
        ],
        "531": [
          1,
          519
        ],
        "532": [
          1,
          520
        ],
        "537": [
          1,
          525
        ],
        "538": [
          1,
          526
        ],
        "543": [
          1,
          531
        ],
        "544": [
          1,
          532
        ],
        "549": [
          1,
          537
        ],
        "550": [
          1,
          538
        ],
        "555": [
          1,
          543
        ],
        "556": [
          1,
          544
        ],
        "561": [
          1,
          549
        ],
        "562": [
          1,
          550
        ],
        "567": [
          1,
          555
        ],
        "568": [
          1,
          556
        ],
        "573": [
          1,
          561
        ],
        "574": [
          1,
          562
        ],
        "579": [
          1,
          567
        ],
        "580": [
          1,
          568
        ],
        "585": [
          1,
          573
        ],
        "586": [
          1,
          574
        ],
        "591": [
          1,
          579
        ],
        "592": [
          1,
          580
        ],
        "597": [
          1,
          585
        ],
        "598": [
          1,
          586
        ],
        "603": [
          1,
          591
        ],
        "604": [
          1,
          592
        ],
        "609": [
          1,
          597
        ],
        "610": [
          1,
          598
        ],
        "615": [
          1,
          603
        ],
        "616": [
          1,
          604
        ],
        "621": [
          1,
          609
        ],
        "622": [
          1,
          610
        ],
        "627": [
          1,
          615
        ],
        "628": [
          1,
          616
        ],
        "633": [
          1,
          621
        ],
        "634": [
          1,
          622
        ],
        "639": [
          1,
          627
        ],
        "640": [
          1,
          628
        ],
        "645": [
          1,
          633
        ],
        "646": [
          1,
          634
        ],
        "651": [
          1,
          639
        ],
        "652": [
          1,
          640
        ],
        "657": [
          1,
          645
        ],
        "658": [
          1,
          646
        ],
        "663": [
          1,
          651
        ],
        "664": [
          1,
          652
        ],
        "669": [
          1,
          657
        ],
        "670": [
          1,
          658
        ],
        "675": [
          1,
          663
        ],
        "676": [
          1,
          664
        ],
        "681": [
          1,
          669
        ],
        "682": [
          1,
          670
        ],
        "687": [
          1,
          675
        ],
        "688": [
          1,
          676
        ],
        "693": [
          1,
          681
        ],
        "694": [
          1,
          682
        ],
        "699": [
          1,
          687
        ],
        "700": [
          1,
          688
        ],
        "705": [
          1,
          693
        ],
        "706": [
          1,
          694
        ],
        "711": [
          1,
          699
        ],
        "712": [
          1,
          700
        ],
        "717": [
          1,
          705
        ],
        "718": [
          1,
          706
        ],
        "723": [
          1,
          711
        ],
        "724": [
          1,
          712
        ],
        "729": [
          1,
          717
        ],
        "730": [
          1,
          718
        ],
        "735": [
          1,
          723
        ],
        "736": [
          1,
          724
        ],
        "741": [
          1,
          729
        ],
        "742": [
          1,
          730
        ],
        "747": [
          1,
          735
        ],
        "748": [
          1,
          736
        ],
        "753": [
          1,
          741
        ],
        "754": [
          1,
          742
        ],
        "759": [
          1,
          747
        ],
        "762": [
          1,
          750
        ],
        "763": [
          1,
          751
        ],
        "769": [
          1,
          757
        ],
        "770": [
          1,
          758
        ],
        "771": [
          1,
          759
        ],
        "778": [
          1,
          766
        ],
        "779": [
          1,
          767
        ],
        "780": [
          1,
          768
        ],
        "784": [
          1,
          772
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 256,
        "lpar_end_from_probe_start": 767
      },
      "body_owner": null,
      "bytes": 784,
      "source_sha256": "9b521a5da5e203c1b47c139f733985be24a591ed8cfcf4f680d07af230e79faa",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-byte4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          4112,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          4087,
          4098,
          "FUN",
          [
            12,
            4112
          ]
        ],
        [
          "NULLABLE_TYPE",
          4087,
          4098,
          "TYPE_REFERENCE",
          [
            12,
            4112
          ]
        ],
        [
          "USER_TYPE",
          4087,
          4097,
          "NULLABLE_TYPE",
          [
            12,
            4112
          ]
        ],
        [
          "USER_TYPE",
          4087,
          4090,
          "USER_TYPE",
          [
            12,
            4112
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          4087,
          4090,
          "USER_TYPE",
          [
            12,
            4112
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          4091,
          4097,
          "USER_TYPE",
          [
            12,
            4112
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          4106,
          4108,
          "FUN",
          [
            12,
            4112
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          4099,
          4106,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          4087,
          4090,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          4090,
          4091,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          4091,
          4097,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          4097,
          4098,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          4098,
          4099,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          4086,
          "BLOCK_COMMENT",
          "/*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/",
          3
        ],
        [
          4086,
          4087,
          "WHITE_SPACE",
          " ",
          4
        ],
        [
          4087,
          4090,
          "IDENTIFIER",
          "pkg",
          5
        ],
        [
          4090,
          4091,
          "DOT",
          ".",
          6
        ],
        [
          4091,
          4097,
          "IDENTIFIER",
          "String",
          7
        ],
        [
          4097,
          4098,
          "QUEST",
          "?",
          8
        ],
        [
          4098,
          4099,
          "DOT",
          ".",
          9
        ],
        [
          4099,
          4106,
          "IDENTIFIER",
          "healthy",
          10
        ],
        [
          4106,
          4107,
          "LPAR",
          "(",
          11
        ],
        [
          4107,
          4108,
          "RPAR",
          ")",
          12
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "4086": [
          1,
          4074
        ],
        "4087": [
          1,
          4075
        ],
        "4090": [
          1,
          4078
        ],
        "4091": [
          1,
          4079
        ],
        "4097": [
          1,
          4085
        ],
        "4098": [
          1,
          4086
        ],
        "4099": [
          1,
          4087
        ],
        "4106": [
          1,
          4094
        ],
        "4107": [
          1,
          4095
        ],
        "4108": [
          1,
          4096
        ],
        "4112": [
          1,
          4100
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 4095
      },
      "body_owner": null,
      "bytes": 4112,
      "source_sha256": "f37c7d260c3408ab0b96c8a20054b2a7f3732671712da589cf8d15676fca43a0",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-byte4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "FUN",
          12,
          4113,
          "KT_FILE",
          null
        ],
        [
          "TYPE_REFERENCE",
          4088,
          4099,
          "FUN",
          [
            12,
            4113
          ]
        ],
        [
          "NULLABLE_TYPE",
          4088,
          4099,
          "TYPE_REFERENCE",
          [
            12,
            4113
          ]
        ],
        [
          "USER_TYPE",
          4088,
          4098,
          "NULLABLE_TYPE",
          [
            12,
            4113
          ]
        ],
        [
          "USER_TYPE",
          4088,
          4091,
          "USER_TYPE",
          [
            12,
            4113
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          4088,
          4091,
          "USER_TYPE",
          [
            12,
            4113
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          4092,
          4098,
          "USER_TYPE",
          [
            12,
            4113
          ]
        ],
        [
          "VALUE_PARAMETER_LIST",
          4107,
          4109,
          "FUN",
          [
            12,
            4113
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          4100,
          4107,
          "healthy"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "type_parameter_name": [
        10,
        11
      ],
      "receiver_tokens": [
        [
          4088,
          4091,
          "IDENTIFIER",
          "pkg",
          "REFERENCE_EXPRESSION"
        ],
        [
          4091,
          4092,
          "DOT",
          ".",
          "USER_TYPE"
        ],
        [
          4092,
          4098,
          "IDENTIFIER",
          "String",
          "REFERENCE_EXPRESSION"
        ],
        [
          4098,
          4099,
          "QUEST",
          "?",
          "NULLABLE_TYPE"
        ],
        [
          4099,
          4100,
          "DOT",
          ".",
          "FUN"
        ]
      ],
      "raw_header_tokens": [
        [
          12,
          15,
          "FUN_KEYWORD",
          "fun",
          1
        ],
        [
          15,
          16,
          "WHITE_SPACE",
          " ",
          2
        ],
        [
          16,
          4087,
          "BLOCK_COMMENT",
          "/*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/",
          3
        ],
        [
          4087,
          4088,
          "WHITE_SPACE",
          " ",
          4
        ],
        [
          4088,
          4091,
          "IDENTIFIER",
          "pkg",
          5
        ],
        [
          4091,
          4092,
          "DOT",
          ".",
          6
        ],
        [
          4092,
          4098,
          "IDENTIFIER",
          "String",
          7
        ],
        [
          4098,
          4099,
          "QUEST",
          "?",
          8
        ],
        [
          4099,
          4100,
          "DOT",
          ".",
          9
        ],
        [
          4100,
          4107,
          "IDENTIFIER",
          "healthy",
          10
        ],
        [
          4107,
          4108,
          "LPAR",
          "(",
          11
        ],
        [
          4108,
          4109,
          "RPAR",
          ")",
          12
        ]
      ],
      "scratch_receiver_tokens": [
        "IDENTIFIER",
        "DOT",
        "IDENTIFIER",
        "SAFE_ACCESS",
        "IDENTIFIER",
        "LPAR"
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "15": [
          1,
          3
        ],
        "16": [
          1,
          4
        ],
        "4087": [
          1,
          4075
        ],
        "4088": [
          1,
          4076
        ],
        "4091": [
          1,
          4079
        ],
        "4092": [
          1,
          4080
        ],
        "4098": [
          1,
          4086
        ],
        "4099": [
          1,
          4087
        ],
        "4100": [
          1,
          4088
        ],
        "4107": [
          1,
          4095
        ],
        "4108": [
          1,
          4096
        ],
        "4109": [
          1,
          4097
        ],
        "4113": [
          1,
          4101
        ]
      },
      "invalid_utf8_offsets": [],
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 4096
      },
      "body_owner": null,
      "bytes": 4113,
      "source_sha256": "394179c739b8b48b13ec20f9889b4c4bbbefb244d7c74f2c3858f5b6b87f52a8",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    }
  ],
  "controls": [
    {
      "id": "two-nullable-decline-missing-first",
      "source": "class Box<T\nfun .String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 38,
      "source_sha256": "b0cde3ea313740b942a3cbc470d5f5db1a6c7847b3f221a4f5a88195bf7c1045",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-missing-second",
      "source": "class Box<T\nfun pkg.?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 35,
      "source_sha256": "eba0726728262b2b9292396bd12491838bdd23e7f95d1ae9bd5c20cd7646de14",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-double-dot",
      "source": "class Box<T\nfun pkg..String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 42,
      "source_sha256": "40ff66471cdcb44b4dd4adf2fe03d364d071eef1904739a0a5286aa89fca8250",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-missing-name",
      "source": "class Box<T\nfun pkg.String?.() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 34,
      "source_sha256": "d9b17e4d1d4d43eeb63effef6e5e188efadda9b6959f8d5768b6991ba5828f8a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-missing-lpar",
      "source": "class Box<T\nfun pkg.String?.healthy = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 39,
      "source_sha256": "dfb0f78259a1082af2149ae297a9237954cba30e538727a251309add9a58c19c",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-long-qualified",
      "source": "class Box<T\nfun a.pkg.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 43,
      "source_sha256": "0fae122575e67b223e6202ab9d25bd6c391c9d369e7bb645dadd8067349d1b9d",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-long-spaced",
      "source": "class Box<T\nfun a.pkg.String? .healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 44,
      "source_sha256": "eb4a9c2e8723688faf8134626c70927f3e2477bbd43d2f4ca7dae4e422b8ba16",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-long-comment",
      "source": "class Box<T\nfun a.pkg.String?/*x*/.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 48,
      "source_sha256": "b5dff0f1c24080f4d61f3f3ca461eb61745f037b98a10355980f20be256097b1",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-nullable-first",
      "source": "class Box<T\nfun pkg?.String.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 41,
      "source_sha256": "f823b3c1a92324ade7f71de424d7d404c5791de665c5bc024401f9e17d700dc6",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-double-nullable",
      "source": "class Box<T\nfun pkg?.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 42,
      "source_sha256": "52a952724946ca1d5dd8c4cd2f0ddf6b9c631031a8935380e0286f0a2d0dcd0d",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-double-question",
      "source": "class Box<T\nfun pkg.String??.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 42,
      "source_sha256": "6de8092a0507f1d931adb5726cc1336cfaf96b66daccc4ddfc6bebeb3f609750",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-double-question-spaced",
      "source": "class Box<T\nfun pkg.String? ?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 43,
      "source_sha256": "1bdba7a2ba8e8863a018e5056673929114c5fd3d99d66671096232a1899593de",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-elvis",
      "source": "class Box<T\nfun pkg.String?:healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 41,
      "source_sha256": "74fed906a1b37865a9bb11d098c0a7470ea956c7ecac097da4130a54d5f54a82",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-elvis-spaced",
      "source": "class Box<T\nfun pkg.String? : .healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 44,
      "source_sha256": "ba04e3aaf99507536dfa5234e84d453d7e7eb17413c0d2062f4879b84dabb9f8",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-elvis-comment",
      "source": "class Box<T\nfun pkg.String?/*x*/:.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 47,
      "source_sha256": "0181eff1d1e6d8008082761ca47d190a4777510ac1c89992dcc9e7a6081d5093",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-missing-dot",
      "source": "class Box<T\nfun pkg.String?healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 40,
      "source_sha256": "5ca240e871186d589e7467cf6dcd0e3e9c50c8764b7c79e44f0bf8079ac0f5e4",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-generic-first",
      "source": "class Box<T\nfun pkg<Int>.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 46,
      "source_sha256": "11cf318b6ecd78337db69124761f0dbf05a02a5b8a2ffc255d5c24195cadbafe",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-generic-second",
      "source": "class Box<T\nfun pkg.String<Int>?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 46,
      "source_sha256": "2347028e3849896dfd9a15455ee4cb78dcbee9dd83ce4bb000d0a163467a7356",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-generic-function",
      "source": "class Box<T\nfun <U> pkg.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 45,
      "source_sha256": "1302c49182ce8da5d815748bd3b536861d2c3ae2421c1924a2a4fe488f2db5cc",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-parenthesized",
      "source": "class Box<T\nfun (pkg.String)?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 43,
      "source_sha256": "266fb7be1b3f9e735f57ac60afebd3674a8681efdf3f90f5ed7d77a4126a598f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-receiver-annotation",
      "source": "class Box<T\nfun @A pkg.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 44,
      "source_sha256": "10a83f34bc26fceaec81313d77fccc61ea676e35400f63f344211bdae9ea5084",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-anonymous",
      "source": "class Box<T\nfun pkg.String?.() { 2 }",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 36,
      "source_sha256": "e1483de642df2c2e8387b9671b505c2f2bb3592c02817f0ec1f2a9296a0a6e0a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-anonymous-no-dot",
      "source": "class Box<T\nfun pkg.String?() { 2 }",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 35,
      "source_sha256": "a62abdda2a52cacb20d2ccdb3476b01a6f441b0f58b63d139b73568a9595f202",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-decline-unclosed-comment",
      "source": "class Box<T\nfun pkg.String?/* .healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty; full declined CST/error array intentionally unspecified",
      "bytes": 44,
      "source_sha256": "d4139ce34201610021047a1406fc7cf7d9ce93e7d908a503985f9c6ca482f0e2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-fun",
      "source": "class Box<T\nfun",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 15,
      "source_sha256": "9641927c6e4a8f1cf32ba45977c2da3b3bcc463afc49e27b521bfbd6db7c8adf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-first",
      "source": "class Box<T\nfun pkg",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 19,
      "source_sha256": "eb533ef75d0261206c293c104401c9e9f611ac301b6c12ff861faa03ea1ef880",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-dot",
      "source": "class Box<T\nfun pkg.",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 20,
      "source_sha256": "e52adea1a7e37c8014b9bcb0150b67c2d250c38ebae10d5e4e191f406a1b120f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-second",
      "source": "class Box<T\nfun pkg.String",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 26,
      "source_sha256": "242e22150e0025b0dc7155af7198e4882bf2236838cf4f4b08ec334e0efe5b36",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-question",
      "source": "class Box<T\nfun pkg.String?",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 27,
      "source_sha256": "89ab47dd9c2f2231bf8701cc766b03a7c25d271471fae5e3e128060c662eac57",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-separator",
      "source": "class Box<T\nfun pkg.String?.",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 28,
      "source_sha256": "8c0262ed7692a9a1af2a46756d01fcdc7d4e027aa5e9dcf0f851d4bd2f471ff8",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-partial-name",
      "source": "class Box<T\nfun pkg.String?.healthy",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "diagnostics": "nonempty",
      "bytes": 35,
      "source_sha256": "d0592cbb1cc23ae80b60733f833b7c07bb9cf2a4949c87bb77adaafab366cc95",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-31-lf",
      "source": "class Box<T : List<Int\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 23,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 52,
      "source_sha256": "2abf637a8618216ac242c81c553190332aad2b14799d91fded368a0e163ee7e2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-32-lf",
      "source": "class Box<T : (Int\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "ad21df22f7d73c450de4a83925840885cf45d3c1a8eb8b734f93753e4858dc5b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-33-lf",
      "source": "class Box<T : Int.\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "2c0523fab3d82e59234beb02e2a08ab3404b702ab57486c0bd3e9db804a7ac15",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-34-lf",
      "source": "class Box<T : Int &\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 20,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 49,
      "source_sha256": "79789d0ecc458d6eb905c1b679dad80054a4d2106f0aeb1db7d09f91fdb4c66a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-35-lf",
      "source": "class Box<T : @A(1\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "cbd24e1e34ef5e3f82df3bbc92ee064ce054588642ba5245189ec99618dcddd4",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-header-only-lf",
      "source": "class Box<T\nfun pkg.String?.healthy(",
      "invariant": "class[0,11); KT_FILE FUN[candidate,source.len); downstream parameter errors",
      "candidate_start": 12,
      "class_end": 11,
      "recognizer": true,
      "diagnostics": "nonempty; Missing > followed by parameter error; whole downstream array unspecified",
      "bytes": 36,
      "source_sha256": "6044db36c9d8b8bff2dfc8a284cc38061ffa472ca52745751f226aef5be7d329",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-valid-anonymous-lf",
      "source": "val f = fun pkg.String?.() { 2 }\n",
      "invariant": "PROPERTY[0,32) under KT_FILE; anonymous FUN[8,32) nearest PROPERTY[0,32); no direct declaration name on FUN; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "ownership": [
        [
          "PROPERTY",
          0,
          32,
          "KT_FILE",
          null
        ],
        [
          "FUN",
          8,
          32,
          "PROPERTY",
          [
            0,
            32
          ]
        ]
      ],
      "bytes": 33,
      "source_sha256": "abc484e35f863c99564cbe5cd3d9119eaac4d3e2cffc0e65b6c7f0d13689c429",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-anonymous-lf",
      "source": "fun outer() {\nval f = fun pkg.String?.() { 2 }\n}",
      "invariant": "only KT_FILE FUN[0,source.len); PROPERTY is BLOCK-owned, anonymous FUN is PROPERTY-owned; no promoted sibling; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "anonymous_start": 22,
      "ownership": [
        [
          "FUN",
          0,
          48,
          "KT_FILE",
          null
        ],
        [
          "PROPERTY",
          14,
          46,
          "BLOCK",
          [
            0,
            48
          ]
        ],
        [
          "FUN",
          22,
          46,
          "PROPERTY",
          [
            14,
            46
          ]
        ]
      ],
      "bytes": 48,
      "source_sha256": "25afc6a10f4ddf2142fac90c318cacc71f0dcd109d05f1224d8eef402f5ab410",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-39-crlf",
      "source": "class Box<T : List<Int\r\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 24,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 53,
      "source_sha256": "c70d442dc9e1cb172111c05d328ca52377f89507ed62f04d271242e9e05e729c",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-40-crlf",
      "source": "class Box<T : (Int\r\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 20,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 49,
      "source_sha256": "6745bcf11a52ebd601a5472ba5dd1fdc8140edcbab733c59c9696f00664d0993",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-41-crlf",
      "source": "class Box<T : Int.\r\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 20,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 49,
      "source_sha256": "edcad6e8ee39b35879562571572fad5523874892246432b1d6daff2953672376",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-42-crlf",
      "source": "class Box<T : Int &\r\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 21,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 50,
      "source_sha256": "8839743c5e0e518acce3d9ad4d5ff8e1b9f68c2a6633f401e0b7a895c7637269",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-43-crlf",
      "source": "class Box<T : @A(1\r\nfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 20,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 49,
      "source_sha256": "4a5098f7bb9133af01646248085269c374fc4dd2e6b919cfbc59f0a1f00469cd",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-header-only-crlf",
      "source": "class Box<T\r\nfun pkg.String?.healthy(",
      "invariant": "class[0,11); KT_FILE FUN[candidate,source.len); downstream parameter errors",
      "candidate_start": 13,
      "class_end": 11,
      "recognizer": true,
      "diagnostics": "nonempty; Missing > followed by parameter error; whole downstream array unspecified",
      "bytes": 37,
      "source_sha256": "cabeee73603db140939e3e257afff0fc210f8a8061dfa8719818b97551f6ef0f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-valid-anonymous-crlf",
      "source": "val f = fun pkg.String?.() { 2 }\r\n",
      "invariant": "PROPERTY[0,32) under KT_FILE; anonymous FUN[8,32) nearest PROPERTY[0,32); no direct declaration name on FUN; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "ownership": [
        [
          "PROPERTY",
          0,
          32,
          "KT_FILE",
          null
        ],
        [
          "FUN",
          8,
          32,
          "PROPERTY",
          [
            0,
            32
          ]
        ]
      ],
      "bytes": 34,
      "source_sha256": "e00842b46fb8042b028f86016b7b8ad71cbedda3e35d9a9b70bada68fdd41bbf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-anonymous-crlf",
      "source": "fun outer() {\r\nval f = fun pkg.String?.() { 2 }\r\n}",
      "invariant": "only KT_FILE FUN[0,source.len); PROPERTY is BLOCK-owned, anonymous FUN is PROPERTY-owned; no promoted sibling; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "anonymous_start": 23,
      "ownership": [
        [
          "FUN",
          0,
          50,
          "KT_FILE",
          null
        ],
        [
          "PROPERTY",
          15,
          47,
          "BLOCK",
          [
            0,
            50
          ]
        ],
        [
          "FUN",
          23,
          47,
          "PROPERTY",
          [
            15,
            47
          ]
        ]
      ],
      "bytes": 50,
      "source_sha256": "b6a660bf4fe2e7b8e19bcb703b5f9465166865e0e8007d088b625fbd704c4061",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-47-cr",
      "source": "class Box<T : List<Int\rfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 23,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 52,
      "source_sha256": "ef2651827dc1a0675ec24d4484a88df81443ecf214b76cf35b23c4c7d95429ea",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-48-cr",
      "source": "class Box<T : (Int\rfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "ca2e593f9f45c59bc58a9afa196ff85ac1695a6d72af637d502eed10093fc269",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-49-cr",
      "source": "class Box<T : Int.\rfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "a1fbdff1e195bb67feb8684f9008057adc7341e6e234dde463e1938701aa3847",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-50-cr",
      "source": "class Box<T : Int &\rfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 20,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 49,
      "source_sha256": "dc7794f0d9e7fdc0facab8d900ab7adb5d58eebcce3fe88fbcdbc5a26cf8f7ac",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-incomplete-bound-51-cr",
      "source": "class Box<T : @A(1\rfun pkg.String?.healthy() = 2",
      "invariant": "no-complete-KT_FILE-FUN-at-candidate",
      "candidate_start": 19,
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 48,
      "source_sha256": "f9b157f27f4e90cb61da34f0ca0e020e696cb58199c6ab48df00bafe31c86732",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-header-only-cr",
      "source": "class Box<T\rfun pkg.String?.healthy(",
      "invariant": "class[0,11); KT_FILE FUN[candidate,source.len); downstream parameter errors",
      "candidate_start": 12,
      "class_end": 11,
      "recognizer": true,
      "diagnostics": "nonempty; Missing > followed by parameter error; whole downstream array unspecified",
      "bytes": 36,
      "source_sha256": "e6c6d25ba5ed251296a22e81291ebdf41cb5ec1e105a0edc40b7466520c72269",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-valid-anonymous-cr",
      "source": "val f = fun pkg.String?.() { 2 }\r",
      "invariant": "PROPERTY[0,32) under KT_FILE; anonymous FUN[8,32) nearest PROPERTY[0,32); no direct declaration name on FUN; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "ownership": [
        [
          "PROPERTY",
          0,
          32,
          "KT_FILE",
          null
        ],
        [
          "FUN",
          8,
          32,
          "PROPERTY",
          [
            0,
            32
          ]
        ]
      ],
      "bytes": 33,
      "source_sha256": "82f39101470e983bad2601d640177107a41d85054230914468b9d1073c346dfc",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-lazy-anonymous-cr",
      "source": "fun outer() {\rval f = fun pkg.String?.() { 2 }\r}",
      "invariant": "only KT_FILE FUN[0,source.len); PROPERTY is BLOCK-owned, anonymous FUN is PROPERTY-owned; no promoted sibling; zero diagnostics",
      "recognizer": false,
      "diagnostics": [],
      "anonymous_start": 22,
      "ownership": [
        [
          "FUN",
          0,
          48,
          "KT_FILE",
          null
        ],
        [
          "PROPERTY",
          14,
          46,
          "BLOCK",
          [
            0,
            48
          ]
        ],
        [
          "FUN",
          22,
          46,
          "PROPERTY",
          [
            14,
            46
          ]
        ]
      ],
      "bytes": 48,
      "source_sha256": "d6dca5e578d3ea48b8edc0705272d197a9fbc7c331005b4577305cf2835c9aa3",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-bound-complete-class",
      "source": "class Box<T : Int\nfun pkg.String?.healthy() = 2",
      "invariant": "same complete-bound eligibility; CLASS[0,class_end) under KT_FILE; complete FUN[candidate_start,source.len) under KT_FILE",
      "recognizer": true,
      "class_end": 17,
      "candidate_start": 18,
      "diagnostics": [
        [
          17,
          17,
          "Missing '>'"
        ]
      ],
      "bytes": 47,
      "source_sha256": "b59dcc5c65b0a782c1bf2dfe3d2e1549bfcb97e0a12035aae4bfce63d1f0f2f6",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-bound-complete-interface",
      "source": "interface Box<T : Int\nfun pkg.String?.healthy() = 2",
      "invariant": "same complete-bound eligibility; CLASS[0,class_end) under KT_FILE; complete FUN[candidate_start,source.len) under KT_FILE",
      "recognizer": true,
      "class_end": 21,
      "candidate_start": 22,
      "diagnostics": [
        [
          21,
          21,
          "Missing '>'"
        ]
      ],
      "bytes": 51,
      "source_sha256": "5e9d294f33275b7f267b80004d440e59eac2de49aa992199c4f423f8c1d55d80",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-object",
      "source": "object Box<T\nfun pkg.String?.healthy() = 2",
      "invariant": "OBJECT_DECLARATION[0,end) with end>12; no direct KT_FILE FUN starting13",
      "recognizer_eligibility": false,
      "diagnostics": "nonempty",
      "bytes": 42,
      "source_sha256": "f66d7642e4c02137fd75f85c7ef725e9e8e0a8b93a40401ba038674e00f4cbf6",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-raw257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "probe_accounting": {
        "lpar_raw_ordinal": 257,
        "lpar_end_from_probe_start": 768
      },
      "diagnostics": "nonempty",
      "bytes": 785,
      "source_sha256": "884383ee4af3f8ccb2cc68faaaedac1501076189478f8302be6608037758de4b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-byte4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "invariant": "class-end-greater-than-11",
      "recognizer": false,
      "probe_accounting": {
        "lpar_raw_ordinal": 11,
        "lpar_end_from_probe_start": 4097
      },
      "diagnostics": "nonempty",
      "bytes": 4114,
      "source_sha256": "0a482733d0fb1245cb8da63100a02820dc8fdb9d3994d7fc9bbccdb4ede81197",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    },
    {
      "id": "two-nullable-aggregate-reset",
      "source": "class Box<@A T0,\n@A T1,\n@A T2,\n@A T3,\n@A T4,\n@A T5,\n@A T6,\n@A T7,\n@A T8,\n@A T9,\n@A T10,\n@A T11,\n@A T12,\n@A T13,\n@A T14,\n@A T15,\n@A T16,\n@A T17,\n@A T18,\n@A T19,\n@A T20,\n@A T21,\n@A T22,\n@A T23,\n@A T24,\n@A T25,\n@A T26,\n@A T27,\n@A T28,\n@A T29,\n@A T30,\n@A T31,\n@A T32,\n@A T33,\n@A T34,\n@A T35,\n@A T36,\n@A T37,\n@A T38,\n@A T39,\n@A T40,\n@A T41,\n@A T42,\n@A T43,\n@A T44,\n@A T45,\n@A T46,\n@A T47,\n@A T48,\n@A T49,\n@A T50,\n@A T51,\n@A T52,\n@A T53,\n@A T54,\n@A T55,\n@A T56,\n@A T57,\n@A T58,\n@A T59,\n@A T60,\n@A T61,\n@A T62,\n@A T63,\nfun pkg.String?.declined() = 1\nclass Fresh<T\nfun pkg.String?.healthy() = 2",
      "invariant": "first CLASS extends beyond declined_start; fresh CLASS recovers complete KT_FILE healthy FUN at healthy_start; diagnostics nonempty",
      "recognizer": false,
      "declined_start": 512,
      "healthy_start": 557,
      "probe_accounting": {
        "first_eligible_windows": 16,
        "each_copied_raw_tokens": 256,
        "total_debit": 4096,
        "fresh_list_budget": 4096
      },
      "diagnostics": "nonempty",
      "bytes": 586,
      "source_sha256": "e1e14656bfabf187ad4ad67d56664f9866415c58fedb689a969f6e7915a676c4",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output"
    }
  ],
  "copied_window_cuts": [
    {
      "id": "raw-cut-first-identifier-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "first-identifier",
      "last_copied_token": [
        775,
        776,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 261,
      "recognizer": false,
      "bytes": 797,
      "source_sha256": "99fc5db6cc48a57f4fe8cab79d9e2348d13204018a678295bce690b890222e4f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-first-identifier-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "first-identifier",
      "last_copied_token": [
        777,
        780,
        "IDENTIFIER",
        "pkg",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 262,
      "recognizer": false,
      "bytes": 802,
      "source_sha256": "5f6a2e9c9669f44833d3ff5b070867707ef02d41151d892a795df13247e524de",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-first-identifier-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "first-identifier",
      "last_copied_token": [
        777,
        778,
        "WHITE_SPACE",
        " ",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 263,
      "recognizer": false,
      "bytes": 803,
      "source_sha256": "bfbf5d8ddf43115de85d0c219d38ba4c7535b1211c0c8e99a9475f92d5addfad",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-receiver-dot-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "receiver-dot",
      "last_copied_token": [
        775,
        781,
        "IDENTIFIER",
        "String",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 260,
      "recognizer": false,
      "bytes": 796,
      "source_sha256": "3904e43c2899a85c0cde0a61daaa5ea649e891d2be0732c4c6abbc4c6ccc78e7",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-receiver-dot-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "receiver-dot",
      "last_copied_token": [
        775,
        776,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 261,
      "recognizer": false,
      "bytes": 797,
      "source_sha256": "99fc5db6cc48a57f4fe8cab79d9e2348d13204018a678295bce690b890222e4f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-receiver-dot-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "receiver-dot",
      "last_copied_token": [
        777,
        780,
        "IDENTIFIER",
        "pkg",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 262,
      "recognizer": false,
      "bytes": 802,
      "source_sha256": "5f6a2e9c9669f44833d3ff5b070867707ef02d41151d892a795df13247e524de",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-second-identifier-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "second-identifier",
      "last_copied_token": [
        776,
        777,
        "QUEST",
        "?",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 259,
      "recognizer": false,
      "bytes": 791,
      "source_sha256": "fbf30b26180137767904140c4f2a2faec05cd361cf522d2770a164185d31c623",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-second-identifier-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "second-identifier",
      "last_copied_token": [
        775,
        781,
        "IDENTIFIER",
        "String",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 260,
      "recognizer": false,
      "bytes": 796,
      "source_sha256": "3904e43c2899a85c0cde0a61daaa5ea649e891d2be0732c4c6abbc4c6ccc78e7",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-second-identifier-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "second-identifier",
      "last_copied_token": [
        775,
        776,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 261,
      "recognizer": false,
      "bytes": 797,
      "source_sha256": "99fc5db6cc48a57f4fe8cab79d9e2348d13204018a678295bce690b890222e4f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-question-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "question",
      "last_copied_token": [
        776,
        777,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 258,
      "recognizer": false,
      "bytes": 790,
      "source_sha256": "4c4ef615abfe076949cff9317f6d3e5cd96fe4e0887f51c02d5205d9162ff4c2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-question-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "question",
      "last_copied_token": [
        776,
        777,
        "QUEST",
        "?",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 259,
      "recognizer": false,
      "bytes": 791,
      "source_sha256": "fbf30b26180137767904140c4f2a2faec05cd361cf522d2770a164185d31c623",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-question-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "question",
      "last_copied_token": [
        775,
        781,
        "IDENTIFIER",
        "String",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 260,
      "recognizer": false,
      "bytes": 796,
      "source_sha256": "3904e43c2899a85c0cde0a61daaa5ea649e891d2be0732c4c6abbc4c6ccc78e7",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-separator-dot-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "separator-dot",
      "last_copied_token": [
        772,
        779,
        "IDENTIFIER",
        "healthy",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 257,
      "recognizer": false,
      "bytes": 785,
      "source_sha256": "884383ee4af3f8ccb2cc68faaaedac1501076189478f8302be6608037758de4b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-separator-dot-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "separator-dot",
      "last_copied_token": [
        776,
        777,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 258,
      "recognizer": false,
      "bytes": 790,
      "source_sha256": "4c4ef615abfe076949cff9317f6d3e5cd96fe4e0887f51c02d5205d9162ff4c2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-separator-dot-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "separator-dot",
      "last_copied_token": [
        776,
        777,
        "QUEST",
        "?",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 259,
      "recognizer": false,
      "bytes": 791,
      "source_sha256": "fbf30b26180137767904140c4f2a2faec05cd361cf522d2770a164185d31c623",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-function-name-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "function-name",
      "last_copied_token": [
        778,
        779,
        "LPAR",
        "(",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 256,
      "recognizer": true,
      "bytes": 784,
      "source_sha256": "9b521a5da5e203c1b47c139f733985be24a591ed8cfcf4f680d07af230e79faa",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-raw256"
    },
    {
      "id": "raw-cut-function-name-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "function-name",
      "last_copied_token": [
        772,
        779,
        "IDENTIFIER",
        "healthy",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 257,
      "recognizer": false,
      "bytes": 785,
      "source_sha256": "884383ee4af3f8ccb2cc68faaaedac1501076189478f8302be6608037758de4b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-function-name-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "function-name",
      "last_copied_token": [
        776,
        777,
        "DOT",
        ".",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 258,
      "recognizer": false,
      "bytes": 790,
      "source_sha256": "4c4ef615abfe076949cff9317f6d3e5cd96fe4e0887f51c02d5205d9162ff4c2",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "raw-cut-opening-parenthesis-255",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 255,
      "target": "opening-parenthesis",
      "last_copied_token": [
        774,
        775,
        "RPAR",
        ")",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 255,
      "recognizer": true,
      "bytes": 779,
      "source_sha256": "9b74996c52be8bde6a1a2890105771b62032fe2214308f2dbaff4ac8ddf5abc1",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-raw255"
    },
    {
      "id": "raw-cut-opening-parenthesis-256",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/pkg.String?.healthy() = 2",
      "target_raw_ordinal": 256,
      "target": "opening-parenthesis",
      "last_copied_token": [
        778,
        779,
        "LPAR",
        "(",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 256,
      "recognizer": true,
      "bytes": 784,
      "source_sha256": "9b521a5da5e203c1b47c139f733985be24a591ed8cfcf4f680d07af230e79faa",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-raw256"
    },
    {
      "id": "raw-cut-opening-parenthesis-257",
      "source": "class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ pkg.String?.healthy() = 2",
      "target_raw_ordinal": 257,
      "target": "opening-parenthesis",
      "last_copied_token": [
        772,
        779,
        "IDENTIFIER",
        "healthy",
        256
      ],
      "copied_raw_tokens": 256,
      "lpar_raw_ordinal": 257,
      "recognizer": false,
      "bytes": 785,
      "source_sha256": "884383ee4af3f8ccb2cc68faaaedac1501076189478f8302be6608037758de4b",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-first-identifier-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "first-identifier",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4104,
        4107,
        "IDENTIFIER",
        "pkg",
        5
      ],
      "copied_raw_tokens": 6,
      "last_copied_token": [
        4107,
        4108,
        "DOT",
        ".",
        6
      ],
      "lpar_end_from_probe_start": 4112,
      "recognizer": false,
      "bytes": 4129,
      "source_sha256": "047be51d18a99f45ac35559d47eaadd3771f2c96365d5492599649d21a126dae",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-first-identifier-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "first-identifier",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4105,
        4108,
        "IDENTIFIER",
        "pkg",
        5
      ],
      "copied_raw_tokens": 5,
      "last_copied_token": [
        4105,
        4108,
        "IDENTIFIER",
        "pkg",
        5
      ],
      "lpar_end_from_probe_start": 4113,
      "recognizer": false,
      "bytes": 4130,
      "source_sha256": "70d1914f553fd7281af4608e6c4ebefdc976576ad5bcdd11f03dccf4db034adf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-first-identifier-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "first-identifier",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4106,
        4109,
        "IDENTIFIER",
        "pkg",
        5
      ],
      "copied_raw_tokens": 4,
      "last_copied_token": [
        4105,
        4106,
        "WHITE_SPACE",
        " ",
        4
      ],
      "lpar_end_from_probe_start": 4114,
      "recognizer": false,
      "bytes": 4131,
      "source_sha256": "de843d75a43e7454f4ecd2592f7938c08cbbe4e26ea4f4b35fbc0faf76096c2a",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-receiver-dot-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "receiver-dot",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4106,
        4107,
        "DOT",
        ".",
        6
      ],
      "copied_raw_tokens": 6,
      "last_copied_token": [
        4106,
        4107,
        "DOT",
        ".",
        6
      ],
      "lpar_end_from_probe_start": 4111,
      "recognizer": false,
      "bytes": 4128,
      "source_sha256": "18723a4f125e8958b25147232d4e653e0ba187ef329d498f99b4f6c84e47ca4e",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-receiver-dot-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "receiver-dot",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4107,
        4108,
        "DOT",
        ".",
        6
      ],
      "copied_raw_tokens": 6,
      "last_copied_token": [
        4107,
        4108,
        "DOT",
        ".",
        6
      ],
      "lpar_end_from_probe_start": 4112,
      "recognizer": false,
      "bytes": 4129,
      "source_sha256": "047be51d18a99f45ac35559d47eaadd3771f2c96365d5492599649d21a126dae",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-receiver-dot-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "receiver-dot",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4108,
        4109,
        "DOT",
        ".",
        6
      ],
      "copied_raw_tokens": 5,
      "last_copied_token": [
        4105,
        4108,
        "IDENTIFIER",
        "pkg",
        5
      ],
      "lpar_end_from_probe_start": 4113,
      "recognizer": false,
      "bytes": 4130,
      "source_sha256": "70d1914f553fd7281af4608e6c4ebefdc976576ad5bcdd11f03dccf4db034adf",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-second-identifier-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "second-identifier",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4101,
        4107,
        "IDENTIFIER",
        "String",
        7
      ],
      "copied_raw_tokens": 8,
      "last_copied_token": [
        4107,
        4108,
        "QUEST",
        "?",
        8
      ],
      "lpar_end_from_probe_start": 4105,
      "recognizer": false,
      "bytes": 4122,
      "source_sha256": "860f1ab11dbca643a912c1214a21bf52af549db52372e2bd395bed80dad04811",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-second-identifier-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "second-identifier",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4102,
        4108,
        "IDENTIFIER",
        "String",
        7
      ],
      "copied_raw_tokens": 7,
      "last_copied_token": [
        4102,
        4108,
        "IDENTIFIER",
        "String",
        7
      ],
      "lpar_end_from_probe_start": 4106,
      "recognizer": false,
      "bytes": 4123,
      "source_sha256": "544f119076f2672c67cb634d32b0589c73e6977797c456f0f83391ff55e31166",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-second-identifier-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "second-identifier",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4103,
        4109,
        "IDENTIFIER",
        "String",
        7
      ],
      "copied_raw_tokens": 6,
      "last_copied_token": [
        4102,
        4103,
        "DOT",
        ".",
        6
      ],
      "lpar_end_from_probe_start": 4107,
      "recognizer": false,
      "bytes": 4124,
      "source_sha256": "13dcfbd8b69e9e2ce96f904dc30639d1d1f6476a54351e3ab338264d53e9add4",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-question-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "question",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4106,
        4107,
        "QUEST",
        "?",
        8
      ],
      "copied_raw_tokens": 9,
      "last_copied_token": [
        4107,
        4108,
        "DOT",
        ".",
        9
      ],
      "lpar_end_from_probe_start": 4104,
      "recognizer": false,
      "bytes": 4121,
      "source_sha256": "348222d3c3c7d4320480456edab5523a7b99fed236b5aa44dad457685d7b8136",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-question-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "question",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4107,
        4108,
        "QUEST",
        "?",
        8
      ],
      "copied_raw_tokens": 8,
      "last_copied_token": [
        4107,
        4108,
        "QUEST",
        "?",
        8
      ],
      "lpar_end_from_probe_start": 4105,
      "recognizer": false,
      "bytes": 4122,
      "source_sha256": "860f1ab11dbca643a912c1214a21bf52af549db52372e2bd395bed80dad04811",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-question-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "question",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4108,
        4109,
        "QUEST",
        "?",
        8
      ],
      "copied_raw_tokens": 7,
      "last_copied_token": [
        4102,
        4108,
        "IDENTIFIER",
        "String",
        7
      ],
      "lpar_end_from_probe_start": 4106,
      "recognizer": false,
      "bytes": 4123,
      "source_sha256": "544f119076f2672c67cb634d32b0589c73e6977797c456f0f83391ff55e31166",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-separator-dot-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "separator-dot",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4106,
        4107,
        "DOT",
        ".",
        9
      ],
      "copied_raw_tokens": 9,
      "last_copied_token": [
        4106,
        4107,
        "DOT",
        ".",
        9
      ],
      "lpar_end_from_probe_start": 4103,
      "recognizer": false,
      "bytes": 4120,
      "source_sha256": "882e2e0bdf69fad0f9b16bcf5975008f3699f4f2f4a3b5b40f666afd226e0b14",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-separator-dot-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "separator-dot",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4107,
        4108,
        "DOT",
        ".",
        9
      ],
      "copied_raw_tokens": 9,
      "last_copied_token": [
        4107,
        4108,
        "DOT",
        ".",
        9
      ],
      "lpar_end_from_probe_start": 4104,
      "recognizer": false,
      "bytes": 4121,
      "source_sha256": "348222d3c3c7d4320480456edab5523a7b99fed236b5aa44dad457685d7b8136",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-separator-dot-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "separator-dot",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4108,
        4109,
        "DOT",
        ".",
        9
      ],
      "copied_raw_tokens": 8,
      "last_copied_token": [
        4107,
        4108,
        "QUEST",
        "?",
        8
      ],
      "lpar_end_from_probe_start": 4105,
      "recognizer": false,
      "bytes": 4122,
      "source_sha256": "860f1ab11dbca643a912c1214a21bf52af549db52372e2bd395bed80dad04811",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-function-name-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "function-name",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4100,
        4107,
        "IDENTIFIER",
        "healthy",
        10
      ],
      "copied_raw_tokens": 11,
      "last_copied_token": [
        4107,
        4108,
        "LPAR",
        "(",
        11
      ],
      "lpar_end_from_probe_start": 4096,
      "recognizer": true,
      "bytes": 4113,
      "source_sha256": "394179c739b8b48b13ec20f9889b4c4bbbefb244d7c74f2c3858f5b6b87f52a8",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-byte4096"
    },
    {
      "id": "byte-cut-function-name-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "function-name",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4101,
        4108,
        "IDENTIFIER",
        "healthy",
        10
      ],
      "copied_raw_tokens": 10,
      "last_copied_token": [
        4101,
        4108,
        "IDENTIFIER",
        "healthy",
        10
      ],
      "lpar_end_from_probe_start": 4097,
      "recognizer": false,
      "bytes": 4114,
      "source_sha256": "0a482733d0fb1245cb8da63100a02820dc8fdb9d3994d7fc9bbccdb4ede81197",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-function-name-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "function-name",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4102,
        4109,
        "IDENTIFIER",
        "healthy",
        10
      ],
      "copied_raw_tokens": 9,
      "last_copied_token": [
        4101,
        4102,
        "DOT",
        ".",
        9
      ],
      "lpar_end_from_probe_start": 4098,
      "recognizer": false,
      "bytes": 4115,
      "source_sha256": "f08a4b293b7e137897b55051494c2f0985512cdd46a8013b992f97bd543e624f",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    },
    {
      "id": "byte-cut-opening-parenthesis-4095",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "opening-parenthesis",
      "target_end_from_probe_start": 4095,
      "target_token": [
        4106,
        4107,
        "LPAR",
        "(",
        11
      ],
      "copied_raw_tokens": 12,
      "last_copied_token": [
        4107,
        4108,
        "RPAR",
        ")",
        12
      ],
      "lpar_end_from_probe_start": 4095,
      "recognizer": true,
      "bytes": 4112,
      "source_sha256": "f37c7d260c3408ab0b96c8a20054b2a7f3732671712da589cf8d15676fca43a0",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-byte4095"
    },
    {
      "id": "byte-cut-opening-parenthesis-4096",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "opening-parenthesis",
      "target_end_from_probe_start": 4096,
      "target_token": [
        4107,
        4108,
        "LPAR",
        "(",
        11
      ],
      "copied_raw_tokens": 11,
      "last_copied_token": [
        4107,
        4108,
        "LPAR",
        "(",
        11
      ],
      "lpar_end_from_probe_start": 4096,
      "recognizer": true,
      "bytes": 4113,
      "source_sha256": "394179c739b8b48b13ec20f9889b4c4bbbefb244d7c74f2c3858f5b6b87f52a8",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "exact positive projection by frozen reference",
      "positive_id": "two-nullable-byte4096"
    },
    {
      "id": "byte-cut-opening-parenthesis-4097",
      "source": "class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ pkg.String?.healthy() = 2",
      "target": "opening-parenthesis",
      "target_end_from_probe_start": 4097,
      "target_token": [
        4108,
        4109,
        "LPAR",
        "(",
        11
      ],
      "copied_raw_tokens": 10,
      "last_copied_token": [
        4101,
        4108,
        "IDENTIFIER",
        "healthy",
        10
      ],
      "lpar_end_from_probe_start": 4097,
      "recognizer": false,
      "bytes": 4114,
      "source_sha256": "0a482733d0fb1245cb8da63100a02820dc8fdb9d3994d7fc9bbccdb4ede81197",
      "expectation_basis": "Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output",
      "production_invariant": "class-end-greater-than-11; diagnostics nonempty"
    }
  ],
  "effective_eof_token_fit": [
    {
      "token": [
        12,
        15,
        "FUN_KEYWORD",
        "fun"
      ],
      "effective_eof": 11,
      "token_fits": false,
      "maximum_slice_end": 11,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        12,
        15,
        "FUN_KEYWORD",
        "fun"
      ],
      "effective_eof": 12,
      "token_fits": false,
      "maximum_slice_end": 12,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        12,
        15,
        "FUN_KEYWORD",
        "fun"
      ],
      "effective_eof": 13,
      "token_fits": false,
      "maximum_slice_end": 13,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        12,
        15,
        "FUN_KEYWORD",
        "fun"
      ],
      "effective_eof": 15,
      "token_fits": true,
      "maximum_slice_end": 15,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        12,
        15,
        "FUN_KEYWORD",
        "fun"
      ],
      "effective_eof": 16,
      "token_fits": true,
      "maximum_slice_end": 16,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        16,
        19,
        "IDENTIFIER",
        "pkg"
      ],
      "effective_eof": 15,
      "token_fits": false,
      "maximum_slice_end": 15,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        16,
        19,
        "IDENTIFIER",
        "pkg"
      ],
      "effective_eof": 16,
      "token_fits": false,
      "maximum_slice_end": 16,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        16,
        19,
        "IDENTIFIER",
        "pkg"
      ],
      "effective_eof": 17,
      "token_fits": false,
      "maximum_slice_end": 17,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        16,
        19,
        "IDENTIFIER",
        "pkg"
      ],
      "effective_eof": 19,
      "token_fits": true,
      "maximum_slice_end": 19,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        16,
        19,
        "IDENTIFIER",
        "pkg"
      ],
      "effective_eof": 20,
      "token_fits": true,
      "maximum_slice_end": 20,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        19,
        20,
        "DOT",
        "."
      ],
      "effective_eof": 18,
      "token_fits": false,
      "maximum_slice_end": 18,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        19,
        20,
        "DOT",
        "."
      ],
      "effective_eof": 19,
      "token_fits": false,
      "maximum_slice_end": 19,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        19,
        20,
        "DOT",
        "."
      ],
      "effective_eof": 20,
      "token_fits": true,
      "maximum_slice_end": 20,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        19,
        20,
        "DOT",
        "."
      ],
      "effective_eof": 21,
      "token_fits": true,
      "maximum_slice_end": 21,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        20,
        26,
        "IDENTIFIER",
        "String"
      ],
      "effective_eof": 19,
      "token_fits": false,
      "maximum_slice_end": 19,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        20,
        26,
        "IDENTIFIER",
        "String"
      ],
      "effective_eof": 20,
      "token_fits": false,
      "maximum_slice_end": 20,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        20,
        26,
        "IDENTIFIER",
        "String"
      ],
      "effective_eof": 23,
      "token_fits": false,
      "maximum_slice_end": 23,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        20,
        26,
        "IDENTIFIER",
        "String"
      ],
      "effective_eof": 26,
      "token_fits": true,
      "maximum_slice_end": 26,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        20,
        26,
        "IDENTIFIER",
        "String"
      ],
      "effective_eof": 27,
      "token_fits": true,
      "maximum_slice_end": 27,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        26,
        27,
        "QUEST",
        "?"
      ],
      "effective_eof": 25,
      "token_fits": false,
      "maximum_slice_end": 25,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        26,
        27,
        "QUEST",
        "?"
      ],
      "effective_eof": 26,
      "token_fits": false,
      "maximum_slice_end": 26,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        26,
        27,
        "QUEST",
        "?"
      ],
      "effective_eof": 27,
      "token_fits": true,
      "maximum_slice_end": 27,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        26,
        27,
        "QUEST",
        "?"
      ],
      "effective_eof": 28,
      "token_fits": true,
      "maximum_slice_end": 28,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        27,
        28,
        "DOT",
        "."
      ],
      "effective_eof": 26,
      "token_fits": false,
      "maximum_slice_end": 26,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        27,
        28,
        "DOT",
        "."
      ],
      "effective_eof": 27,
      "token_fits": false,
      "maximum_slice_end": 27,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        27,
        28,
        "DOT",
        "."
      ],
      "effective_eof": 28,
      "token_fits": true,
      "maximum_slice_end": 28,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        27,
        28,
        "DOT",
        "."
      ],
      "effective_eof": 29,
      "token_fits": true,
      "maximum_slice_end": 29,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        28,
        35,
        "IDENTIFIER",
        "healthy"
      ],
      "effective_eof": 27,
      "token_fits": false,
      "maximum_slice_end": 27,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        28,
        35,
        "IDENTIFIER",
        "healthy"
      ],
      "effective_eof": 28,
      "token_fits": false,
      "maximum_slice_end": 28,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        28,
        35,
        "IDENTIFIER",
        "healthy"
      ],
      "effective_eof": 31,
      "token_fits": false,
      "maximum_slice_end": 31,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        28,
        35,
        "IDENTIFIER",
        "healthy"
      ],
      "effective_eof": 35,
      "token_fits": true,
      "maximum_slice_end": 35,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        28,
        35,
        "IDENTIFIER",
        "healthy"
      ],
      "effective_eof": 36,
      "token_fits": true,
      "maximum_slice_end": 36,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        35,
        36,
        "LPAR",
        "("
      ],
      "effective_eof": 34,
      "token_fits": false,
      "maximum_slice_end": 34,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        35,
        36,
        "LPAR",
        "("
      ],
      "effective_eof": 35,
      "token_fits": false,
      "maximum_slice_end": 35,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        35,
        36,
        "LPAR",
        "("
      ],
      "effective_eof": 36,
      "token_fits": true,
      "maximum_slice_end": 36,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        35,
        36,
        "LPAR",
        "("
      ],
      "effective_eof": 37,
      "token_fits": true,
      "maximum_slice_end": 37,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        36,
        37,
        "RPAR",
        ")"
      ],
      "effective_eof": 35,
      "token_fits": false,
      "maximum_slice_end": 35,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        36,
        37,
        "RPAR",
        ")"
      ],
      "effective_eof": 36,
      "token_fits": false,
      "maximum_slice_end": 36,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        36,
        37,
        "RPAR",
        ")"
      ],
      "effective_eof": 37,
      "token_fits": true,
      "maximum_slice_end": 37,
      "basis": "complete raw token end <= active EOF; original source bytes"
    },
    {
      "token": [
        36,
        37,
        "RPAR",
        ")"
      ],
      "effective_eof": 38,
      "token_fits": true,
      "maximum_slice_end": 38,
      "basis": "complete raw token end <= active EOF; original source bytes"
    }
  ],
  "healthy_followup": {
    "source": "fun healthy() = 2",
    "projection": [
      [
        "FUN",
        0,
        17,
        "KT_FILE",
        null
      ],
      [
        "VALUE_PARAMETER_LIST",
        11,
        13,
        "FUN",
        [
          0,
          17
        ]
      ]
    ],
    "declaration_names": [
      [
        0,
        4,
        11,
        "healthy"
      ]
    ],
    "diagnostics": []
  },
  "limits": {
    "source_bytes": 65536,
    "deadline_ms": 500,
    "cumulative_requested_allocation_bytes": 67108864,
    "output_bytes": 4194304,
    "elements": 16384,
    "diagnostics": 256,
    "nesting": 128,
    "probe_raw_tokens": 256,
    "probe_source_bytes": 4096,
    "copied_raw_tokens_per_classifier_list": 4096
  }
}
"####;
