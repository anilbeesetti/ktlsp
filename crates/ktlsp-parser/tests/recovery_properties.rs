//! Finite property-header contract frozen before implementation; runtime evidence is separate.
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
                        || matches!(e.kind.as_str(), "TYPE_PARAMETER_LIST" | "TYPE_PARAMETER"))
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

fn names(tree: &Parsed, parameters: bool) -> Value {
    let elements = tree.elements();
    Value::Array(
        elements
            .iter()
            .filter(|e| {
                e.token
                    && e.kind == "IDENTIFIER"
                    && e.parent.is_some_and(|p| {
                        if parameters {
                            elements[p].kind == "TYPE_PARAMETER"
                        } else {
                            declaration(&elements[p].kind)
                        }
                    })
            })
            .map(|e| {
                let parent = &elements[e.parent.unwrap()];
                let spelling = &tree.source()[e.start..e.end];
                if parameters {
                    json!([
                        e.start,
                        e.end,
                        spelling,
                        parent.kind,
                        parent.start,
                        parent.end
                    ])
                } else {
                    json!([parent.start, e.start, e.end, spelling])
                }
            })
            .collect(),
    )
}

fn node(tree: &Parsed, kind: &str, start: usize) -> usize {
    let nodes: Vec<_> = tree
        .elements()
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.kind == kind && e.start == start)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(nodes.len(), 1, "{kind} at {start}");
    nodes[0]
}

fn check(tree: &Parsed, case: &Value) {
    let id = case["id"].as_str().unwrap_or("healthy-followup");
    let source = case["source"].as_str().unwrap();
    assert_eq!(tree.source(), source, "{id}");
    let root = &tree.elements()[0];
    assert_eq!(
        (root.kind.as_str(), root.start, root.end, root.parent),
        ("KT_FILE", 0, source.len(), None),
        "{id}"
    );
    let mut cursor = 0;
    let mut spelling = String::new();
    for token in tree.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor, "{id}");
        assert!(
            source.is_char_boundary(token.start) && source.is_char_boundary(token.end),
            "{id}"
        );
        spelling.push_str(&source[token.start..token.end]);
        cursor = token.end;
    }
    assert_eq!(cursor, source.len(), "{id}");
    assert_eq!(spelling, source, "{id}");
    if let Some(expected) = case.get("projection") {
        assert_eq!(projection(tree), *expected, "{id}");
    }
    if let Some(expected) = case.get("declaration_names") {
        assert_eq!(names(tree, false), *expected, "{id}");
    }
    if let Some(expected) = case.get("parameter_names") {
        assert_eq!(names(tree, true), *expected, "{id}");
    }
    if case["diagnostics"].is_array() {
        let diagnostics: Vec<_> = tree
            .diagnostics()
            .iter()
            .map(|d| json!([d.start, d.end, d.message]))
            .collect();
        assert_eq!(json!(diagnostics), case["diagnostics"], "{id}");
    } else {
        assert!(!tree.diagnostics().is_empty(), "{id}");
    }
    if let Some(positions) = case["positions"].as_object() {
        for (offset, expected) in positions {
            let offset = offset.parse::<usize>().unwrap();
            if expected.is_string() {
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
    }
    if let Some(invariant) = case["invariant"].as_str() {
        let offset = |key: &str| case[key].as_u64().unwrap() as usize;
        match invariant {
            "class-end-greater-than-boundary" | "object-end-greater-than-boundary" => {
                let kind = if invariant.starts_with("object") {
                    "OBJECT_DECLARATION"
                } else {
                    "CLASS"
                };
                assert!(
                    tree.elements()[node(tree, kind, 0)].end > offset("boundary"),
                    "{id}"
                );
            }
            "whole-class-no-top-level-property" => {
                assert_eq!(
                    tree.elements()[node(tree, "CLASS", 0)].end,
                    offset("class_end"),
                    "{id}"
                );
                assert!(
                    !tree.elements().iter().any(|e| !e.token
                        && e.kind == "PROPERTY"
                        && e.parent
                            .is_some_and(|p| tree.elements()[p].kind == "KT_FILE")),
                    "{id}"
                );
            }
            "whole-class-then-ordinary-property" => {
                assert_eq!(
                    tree.elements()[node(tree, "CLASS", 0)].end,
                    offset("class_end"),
                    "{id}"
                );
                let property_id = node(tree, "PROPERTY", offset("property_start"));
                let property = &tree.elements()[property_id];
                assert_eq!(property.end, offset("property_end"), "{id}");
                assert_eq!(
                    tree.elements()[property.parent.unwrap()].kind,
                    "KT_FILE",
                    "{id}"
                );
                let property_names: Vec<_> = tree
                    .elements()
                    .iter()
                    .filter(|e| e.token && e.kind == "IDENTIFIER" && e.parent == Some(property_id))
                    .map(|e| (e.start, e.end, &source[e.start..e.end]))
                    .collect();
                assert_eq!(
                    property_names,
                    [(offset("name_start"), offset("name_end"), "later")],
                    "{id}"
                );
            }
            other => panic!("unknown frozen invariant: {other}"),
        }
    }
}

#[test]
fn ordinary_property_headers_recover_through_production_snapshot_worker() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let positives = fixtures["positive"].as_array().unwrap();
    let controls = fixtures["controls"].as_array().unwrap();
    let normal = fixtures["normal"].as_array().unwrap();
    assert_eq!((positives.len(), controls.len(), normal.len()), (15, 18, 3));
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for case in positives.iter().chain(controls).chain(normal) {
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
            .unwrap();
        assert_eq!(accepted.snapshot(), &snapshot, "{id}");
        check(accepted.tree_for(&snapshot).unwrap(), case);
        let original_elements =
            serde_json::to_value(accepted.tree_for(&snapshot).unwrap().elements()).unwrap();
        let original_diagnostics =
            serde_json::to_value(accepted.tree_for(&snapshot).unwrap().diagnostics()).unwrap();
        for mismatch in [
            SourceSnapshot::new(id, 2, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(format!("{id}-other"), 1, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Source, format!("{source} ")).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Script, source).unwrap(),
        ] {
            assert_eq!(
                accepted.tree_for(&mismatch).unwrap_err(),
                ParserError::StaleSnapshot,
                "{id}"
            );
        }
        let healthy = SourceSnapshot::new(
            id,
            2,
            FileKind::Source,
            fixtures["healthy_followup"]["source"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            accepted.tree_for(&healthy).unwrap_err(),
            ParserError::StaleSnapshot,
            "{id}"
        );
        let followup = parser
            .parse_snapshot(&healthy, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(followup.snapshot(), &healthy, "{id}");
        check(
            followup.tree_for(&healthy).unwrap(),
            &fixtures["healthy_followup"],
        );
        assert_eq!(accepted.snapshot(), &snapshot, "{id}");
        assert_eq!(snapshot.source(), source, "{id}");
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

// Exact frozen contract SHA256: 2305fb9abb52d67356ad67527f3c3d5c622725a1d07753bf099c82fc0004bc8f
const FIXTURES: &str = r####"{
  "version": 1,
  "base": "50bef1c6d907c673839dd06666023314cad5c1fa",
  "base_tree": "6f7154fe6115970fa92414f5ad9ff54eab16706d",
  "status": "CONTRACT ONLY; SOURCE HOLD; no native execution",
  "positive": [
    {
      "id": "seed-lf",
      "source": "class Box<T\nval later = 1",
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
          "PROPERTY",
          12,
          25,
          "KT_FILE",
          null
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
          16,
          21,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "25": [
          1,
          13
        ]
      },
      "bytes": 25,
      "source_sha256": "4a2dfeb01c1abb0a8f3a889cc12ae501a2f6fd31ccd13e260e99c179b5babf14"
    },
    {
      "id": "seed-crlf",
      "source": "class Box<T\r\nval later = 1",
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
          "PROPERTY",
          13,
          26,
          "KT_FILE",
          null
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
          17,
          22,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "17": [
          1,
          4
        ],
        "22": [
          1,
          9
        ],
        "26": [
          1,
          13
        ]
      },
      "bytes": 26,
      "source_sha256": "148b628c8ad1fea56c33585f1c34ca4e8981890247d0082e431a11ad417339d4"
    },
    {
      "id": "seed-cr",
      "source": "class Box<T\rval later = 1",
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
          "PROPERTY",
          12,
          25,
          "KT_FILE",
          null
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
          16,
          21,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "25": [
          1,
          13
        ]
      },
      "bytes": 25,
      "source_sha256": "2be6d8b494e238379600b5281d8fea87c1d7efd6eaf894aca8d76365ef89995f"
    },
    {
      "id": "interface",
      "source": "interface Box<T\nval later = 1",
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
          "PROPERTY",
          16,
          29,
          "KT_FILE",
          null
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
          20,
          25,
          "later"
        ]
      ],
      "parameter_names": [
        [
          14,
          15,
          "T",
          "TYPE_PARAMETER",
          14,
          15
        ]
      ],
      "diagnostics": [
        [
          15,
          15,
          "Missing '>'"
        ]
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
        "20": [
          1,
          4
        ],
        "25": [
          1,
          9
        ],
        "29": [
          1,
          13
        ]
      },
      "bytes": 29,
      "source_sha256": "1e99d3b1469343fa647a7294fdfbb3794450d0f4af42519882ad565c67f8e45c"
    },
    {
      "id": "empty-list",
      "source": "class Box<\nval later = 1",
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
          "PROPERTY",
          11,
          24,
          "KT_FILE",
          null
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
          15,
          20,
          "later"
        ]
      ],
      "parameter_names": [],
      "diagnostics": [
        [
          10,
          10,
          "Type parameter declaration expected"
        ]
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
        "15": [
          1,
          4
        ],
        "20": [
          1,
          9
        ],
        "24": [
          1,
          13
        ]
      },
      "bytes": 24,
      "source_sha256": "6203571717ce82bac72eff1eb90b3ccfe90b0ac5a77cbfdf3b70fd97620032f9"
    },
    {
      "id": "trailing-comma",
      "source": "class Box<T,\nval later = 1",
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
          "PROPERTY",
          13,
          26,
          "KT_FILE",
          null
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
          17,
          22,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          12,
          12,
          "Type parameter declaration expected"
        ]
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
        "17": [
          1,
          4
        ],
        "22": [
          1,
          9
        ],
        "26": [
          1,
          13
        ]
      },
      "bytes": 26,
      "source_sha256": "09d16c5d29b88fb24966fa1bf90f5802bc36c90272672a88ee82fa04920b5449"
    },
    {
      "id": "same-line",
      "source": "class Box<T val later = 1",
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
          "PROPERTY",
          12,
          25,
          "KT_FILE",
          null
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
          16,
          21,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "16": [
          0,
          16
        ],
        "21": [
          0,
          21
        ],
        "25": [
          0,
          25
        ]
      },
      "bytes": 25,
      "source_sha256": "e48d92f0e4d749adf1f46d3284be20847a1c0d87b83a7d48e6c4486436038f1b"
    },
    {
      "id": "internal-comment",
      "source": "class Box<T\nval /*x*/ later = 1",
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
          "PROPERTY",
          12,
          31,
          "KT_FILE",
          null
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
          22,
          27,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "22": [
          1,
          10
        ],
        "27": [
          1,
          15
        ],
        "31": [
          1,
          19
        ]
      },
      "bytes": 31,
      "source_sha256": "d538e11dc8fb6b12521ba4f7414f7fb5154e21debb2636d88ae292d4c9c339d3"
    },
    {
      "id": "soft-name",
      "source": "class Box<T\nval get = 1",
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
          "PROPERTY",
          12,
          23,
          "KT_FILE",
          null
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
          16,
          19,
          "get"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "16": [
          1,
          4
        ],
        "19": [
          1,
          7
        ],
        "23": [
          1,
          11
        ]
      },
      "bytes": 23,
      "source_sha256": "f816539f4e30e1dc54630303949ede94ac71079febf53c48c397d42faaa13efd"
    },
    {
      "id": "unicode-backtick",
      "source": "class Box<T\nval `later 🦀é` = 1",
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
          "PROPERTY",
          12,
          35,
          "KT_FILE",
          null
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
          16,
          31,
          "`later 🦀é`"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "16": [
          1,
          4
        ],
        "24": "INVALID_UTF8_INTERIOR",
        "25": "INVALID_UTF8_INTERIOR",
        "26": "INVALID_UTF8_INTERIOR",
        "29": "INVALID_UTF8_INTERIOR",
        "31": [
          1,
          16
        ],
        "35": [
          1,
          20
        ]
      },
      "bytes": 35,
      "source_sha256": "a17f158b078197c1853d0d14e3f126f46dd654ae6e8f38be98eeeb7c0882977c"
    },
    {
      "id": "member",
      "source": "class Host {\nclass Box<T\nval later = 1\n}",
      "projection": [
        [
          "CLASS",
          0,
          40,
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
            40
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
          "PROPERTY",
          25,
          38,
          "CLASS_BODY",
          [
            0,
            40
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
          29,
          34,
          "later"
        ]
      ],
      "parameter_names": [
        [
          23,
          24,
          "T",
          "TYPE_PARAMETER",
          23,
          24
        ]
      ],
      "diagnostics": [
        [
          24,
          24,
          "Missing '>'"
        ]
      ],
      "positions": {
        "0": [
          0,
          0
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
        "29": [
          2,
          4
        ],
        "34": [
          2,
          9
        ],
        "38": [
          2,
          13
        ],
        "40": [
          3,
          1
        ]
      },
      "bytes": 40,
      "source_sha256": "fd7c8dc748130887b8004bc49dd2288475f70640e02934bc38aa31efc95858a6"
    },
    {
      "id": "local",
      "source": "fun outer() {\nclass Box<T\nval later = 1\n}",
      "projection": [
        [
          "FUN",
          0,
          41,
          "KT_FILE",
          null
        ],
        [
          "CLASS",
          14,
          25,
          "BLOCK",
          [
            0,
            41
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
          "PROPERTY",
          26,
          39,
          "BLOCK",
          [
            0,
            41
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
          30,
          35,
          "later"
        ]
      ],
      "parameter_names": [
        [
          24,
          25,
          "T",
          "TYPE_PARAMETER",
          24,
          25
        ]
      ],
      "diagnostics": [
        [
          25,
          25,
          "Missing '>'"
        ]
      ],
      "positions": {
        "0": [
          0,
          0
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
        "30": [
          2,
          4
        ],
        "35": [
          2,
          9
        ],
        "39": [
          2,
          13
        ],
        "41": [
          3,
          1
        ]
      },
      "bytes": 41,
      "source_sha256": "cbac5852c12559ea2af6bb9e0c24a648c6add19c6687cf35d7aadaadd970b9fd"
    },
    {
      "id": "header-only-eq",
      "source": "class Box<T\nval later =",
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
          "PROPERTY",
          12,
          23,
          "KT_FILE",
          null
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
          16,
          21,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ],
        [
          23,
          23,
          "Expecting an expression"
        ]
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
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "23": [
          1,
          11
        ]
      },
      "bytes": 23,
      "source_sha256": "c05a15b35b4b94d4bc9b8fb6271534716d163b5af8a03648145c8806df40b1f8"
    },
    {
      "id": "raw256",
      "source": "class Box<T\nval /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*y*/later = 1",
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
          "PROPERTY",
          12,
          780,
          "KT_FILE",
          null
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
          776,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "771": [
          1,
          759
        ],
        "776": [
          1,
          764
        ],
        "780": [
          1,
          768
        ]
      },
      "probe_accounting": {
        "eq_raw_ordinal": 256,
        "copied_raw_tokens": 256
      },
      "bytes": 780,
      "source_sha256": "3cb4ed6eccc0eceeb262c1f98cd9995adadff0c59d62907806a0330357d71e69"
    },
    {
      "id": "byte4096",
      "source": "class Box<T\nval /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ later = 1",
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
          "PROPERTY",
          12,
          4110,
          "KT_FILE",
          null
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
          4101,
          4106,
          "later"
        ]
      ],
      "parameter_names": [
        [
          10,
          11,
          "T",
          "TYPE_PARAMETER",
          10,
          11
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
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
        "4101": [
          1,
          4089
        ],
        "4106": [
          1,
          4094
        ],
        "4110": [
          1,
          4098
        ]
      },
      "probe_accounting": {
        "eq_end_from_probe_start": 4096
      },
      "bytes": 4110,
      "source_sha256": "80521765386684ab42c7c9de9439124c556bb790fa9475b759eeb9a60ac656db"
    }
  ],
  "controls": [
    {
      "id": "only-val",
      "source": "class Box<T\nval",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 15,
      "source_sha256": "327b6647e44e5af3ddeadbdb2ab7cc67687a985e61caff037b1c676f0950eded"
    },
    {
      "id": "no-eq",
      "source": "class Box<T\nval later",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 21,
      "source_sha256": "3e897b80a8e64928e9a652269249cf3a193e2888094982be64323944832dfb09"
    },
    {
      "id": "no-name",
      "source": "class Box<T\nval = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 19,
      "source_sha256": "bfeae093629d54b4113bfe7db6ebc7698132d81a5a8a96b88922d1ecfd9b85b6"
    },
    {
      "id": "typed",
      "source": "class Box<T\nval later: Int = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 30,
      "source_sha256": "f01425d6add7606c3eca2efed0b473ebdcd4361beb15184b06258b47c9335821"
    },
    {
      "id": "destructured",
      "source": "class Box<T\nval (later) = item",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 30,
      "source_sha256": "8c8a60b4b4f2cd377a7d01ba82445341a8f9dcd87395b7618bc689eb4d36f2ff"
    },
    {
      "id": "var",
      "source": "class Box<T\nvar later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 25,
      "source_sha256": "0b683eee63e02c0abc415e818e82d9ab502896b947b1bd1bf744ccdd9836a245"
    },
    {
      "id": "annotation",
      "source": "class Box<T\n@A val later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 28,
      "source_sha256": "9cd5c20b643801d0586cd79b8118bb01801d981ac52f93a8fb0ab20c01d10f26"
    },
    {
      "id": "modifier",
      "source": "class Box<T\npublic val later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 32,
      "source_sha256": "f4ec00703ac6b80b4e5c59baacc062d8c2eb3dece516acffb9b7916dd2200c23"
    },
    {
      "id": "receiver",
      "source": "class Box<T\nval T.later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 27,
      "source_sha256": "fd140823fb66083c7ffb76e44e840747c8f874d21925ccffe0e9574d988b8d08"
    },
    {
      "id": "generic",
      "source": "class Box<T\nval <U> later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 29,
      "source_sha256": "cce7512a1ccc70727f067f984992efbed955e8ba87c94016a6a98f75bfbc370f"
    },
    {
      "id": "delegate",
      "source": "class Box<T\nval later by lazy { 1 }",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 35,
      "source_sha256": "7ff428f5ae5528ea25abe81ed08d8ed6c82dfb545e3602ff7072f525a4ce7fe0"
    },
    {
      "id": "raw257",
      "source": "class Box<T\nval /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 781,
      "source_sha256": "23f37709b6b605f389fe2280c9eec8cad9a9c6414e49aa5d6b741b17a0ac8dd0"
    },
    {
      "id": "byte4097",
      "source": "class Box<T\nval /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 11,
      "diagnostics": "nonempty",
      "bytes": 4111,
      "source_sha256": "85b6bb84488715a1f02b4cf87650c28d863cbf304bacb8e92ce054cfab048129"
    },
    {
      "id": "incomplete-bound",
      "source": "class Box<T : List<Int\nval later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 22,
      "diagnostics": "nonempty",
      "bytes": 36,
      "source_sha256": "c4a68a4f9b0a5916709a1ff0978713aa29274bfe2dd93c40edd16dac457ca750"
    },
    {
      "id": "unfinished-annotation",
      "source": "class Box<T : @A(\nval later = 1",
      "invariant": "class-end-greater-than-boundary",
      "class_start": 0,
      "boundary": 17,
      "diagnostics": "nonempty",
      "bytes": 31,
      "source_sha256": "9dbb981044142ba61fa0209f92685efe19f8ba1cce9682fd5cb3f97f7ec08b7d"
    },
    {
      "id": "nonclassifier-object",
      "source": "object Box<T\nval later = 1",
      "invariant": "object-end-greater-than-boundary",
      "boundary": 12,
      "diagnostics": "nonempty",
      "bytes": 26,
      "source_sha256": "970762e75908d4641b0949dc16e4e27e9488bd2e4caebcfc3740b7936e3a0626"
    },
    {
      "id": "nested-valid-annotation",
      "source": "class Box<@A({ val later = 1 }) T>",
      "invariant": "whole-class-no-top-level-property",
      "class_start": 0,
      "class_end": 34,
      "diagnostics": [],
      "bytes": 34,
      "source_sha256": "ab18bee8cbb6e1530e0a17ec9a5cdddae60374f61b526b872fe30fb7d1bef849"
    },
    {
      "id": "quoted-lookalike",
      "source": "class Box<@A(\"val later = 1\") T>",
      "invariant": "whole-class-no-top-level-property",
      "class_start": 0,
      "class_end": 32,
      "diagnostics": [],
      "bytes": 32,
      "source_sha256": "761322c6fb4298ec540d866307d3bf7120938dfa61b09ec2ca53f42150cd98c1"
    }
  ],
  "normal": [
    {
      "id": "normal-property",
      "source": "val later = 1",
      "projection": [
        [
          "PROPERTY",
          0,
          13,
          "KT_FILE",
          null
        ]
      ],
      "declaration_names": [
        [
          0,
          4,
          9,
          "later"
        ]
      ],
      "diagnostics": [],
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
        "13": [
          0,
          13
        ]
      },
      "bytes": 13,
      "source_sha256": "e04c49d5534cf8e267e05db01701e3c66db22b346f730b4374f46c26f2a7422b"
    },
    {
      "id": "valid-generic",
      "source": "class Box<T>\nval later = 1",
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
          "PROPERTY",
          13,
          26,
          "KT_FILE",
          null
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
          17,
          22,
          "later"
        ]
      ],
      "diagnostics": [],
      "positions": {
        "0": [
          0,
          0
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
        "17": [
          1,
          4
        ],
        "22": [
          1,
          9
        ],
        "26": [
          1,
          13
        ]
      },
      "bytes": 26,
      "source_sha256": "1d7a6fc7eacc717d098dd07f498af5b9bcf5d3f6e46c047f26ae29c9a46b4d20"
    },
    {
      "id": "valid-bound",
      "source": "class Box<T : List<Int>>\nval later = 1",
      "invariant": "whole-class-then-ordinary-property",
      "class_end": 24,
      "property_start": 25,
      "property_end": 38,
      "name_start": 29,
      "name_end": 34,
      "diagnostics": [],
      "bytes": 38,
      "source_sha256": "1f99290f994633b8e19545c2162036d6abaf5778bcb171b02a975222541c11e6"
    }
  ],
  "healthy_followup": {
    "source": "val healthy = 2",
    "projection": [
      [
        "PROPERTY",
        0,
        15,
        "KT_FILE",
        null
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
    "diagnostics": [],
    "positions": {
      "0": [
        0,
        0
      ],
      "4": [
        0,
        4
      ],
      "11": [
        0,
        11
      ],
      "15": [
        0,
        15
      ]
    }
  },
  "selected_kinds": [
    "CLASS",
    "OBJECT_DECLARATION",
    "FUN",
    "PROPERTY",
    "TYPEALIAS",
    "TYPE_PARAMETER_LIST",
    "TYPE_PARAMETER"
  ],
  "every_case": [
    "production Parser::parse_snapshot exact document/revision1/FileKind::Source/source identity; tree_for exact original",
    "all original token spans partition source on UTF8 boundaries and reconstruct identical bytes; root KT_FILE spans complete source",
    "selected projections/names/parameter names/diagnostics and supplied original UTF16 positions are exact literals where supplied; decline controls assert only stated invariants",
    "capture accepted snapshot/source/elements/diagnostics before healthy followup; tree_for rejects revision2 and changed document/source/kind at revision1",
    "healthy revision2 uses SAME Parser, has exact PROPERTY/name spans and zero diagnostics; each request starts separate worker",
    "original accepted snapshot/source/elements/diagnostics unchanged after healthy followup"
  ],
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
    "copied_tokens_per_classifier_list": 4096
  },
  "compatibility": "All existing functions/nullable/qualified/116-case families, C14/C15/C16/resource tests and historical payloads byte-identical. Replace ONLY val later = 1 contradictory exclusion in declarations.rs. Existing aggregate/reset and const EOF assertions preserved; no new B11 end-to-end runtime claim.",
  "scope": [
    "first significant VAL_KEYWORD only, scratch expect(VAL_KEYWORD) and expect(IDENTIFIER) then at(EQ); no modifiers/annotations/property parser callback",
    "same existing classifier-only admission and completed-bound proof, scratch allocation/debit, effective EOF and recursive-probe suppression",
    "ordinary soft-keyword/backtick IDENTIFIER as existing expect remapping; no new newline prerequisite"
  ],
  "excluded": [
    "typed/destructured/var/receiver/generic/annotated/modifier/delegate property extensions",
    "shared parameter contexts/helpers/API/Cargo/deps/lock/worker/protocol/workflows/root docs",
    "fresh oracle/full-C13/semantics/full-parity/B3/B4/B11/#96 claims"
  ]
}
"####;
