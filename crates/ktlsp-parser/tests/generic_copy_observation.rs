#![cfg(unix)]
//! Frozen pre-runtime B11 contract; machinery controls are not production-copy evidence.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, GenericCopyEvidence, GenericCopyRecord, Parsed, Parser, ParserError, SnapshotResult,
    SourceSnapshot,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn positions(tree: &Parsed, start: usize, end: usize) -> Value {
    let start = tree.position(start).unwrap();
    let end = tree.position(end).unwrap();
    json!([{"line":start.line,"character":start.character},{"line":end.line,"character":end.character}])
}
fn check(result: &SnapshotResult, snapshot: &SourceSnapshot, case: &Value) {
    assert_eq!(result.snapshot(), snapshot);
    let tree = result.tree_for(snapshot).unwrap();
    assert_eq!(tree.source(), case["source_utf8"].as_str().unwrap());
    let elements = tree.elements();
    let selected: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            !e.token
                && matches!(
                    e.kind.as_str(),
                    "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS"
                )
        })
        .collect();
    let declarations: Vec<_> = selected.iter().map(|(id,e)| {
        let mut parent = e.parent;
        let owner = loop {
            match parent {
                Some(p) => {
                    if let Some(index) = selected.iter().position(|(id,_)| *id == p) { break Some(index); }
                    parent = elements[p].parent;
                }
                None => break None,
            }
        };
        let names: Vec<_> = elements.iter().filter(|n| n.token && n.kind == "IDENTIFIER" && n.parent == Some(*id)).collect();
        assert!(names.len() <= 1);
        let name = names.first().map(|n| json!({"text":&tree.source()[n.start..n.end],"bytes":[n.start,n.end],"utf16":positions(tree,n.start,n.end)}));
        json!({"kind":e.kind,"bytes":[e.start,e.end],"text":&tree.source()[e.start..e.end],
            "immediate_parent_kind":elements[e.parent.unwrap()].kind,"nearest_selected_parent_index":owner,
            "direct_name":name,"utf16":positions(tree,e.start,e.end)})
    }).collect();
    assert_eq!(json!(declarations), case["declarations"], "{}", case["id"]);
    let lists: Vec<_> = elements.iter().filter(|e| !e.token && e.kind == "TYPE_PARAMETER_LIST").map(|e| {
        json!({"kind":e.kind,"bytes":[e.start,e.end],"parent_selected_index":selected.iter().position(|(id,_)| Some(*id)==e.parent).unwrap()})
    }).collect();
    assert_eq!(
        json!(lists),
        if case["type_parameter_list"].is_null() {
            json!([])
        } else {
            json!([case["type_parameter_list"]])
        }
    );
    let diagnostics: Vec<_> = tree.diagnostics().iter().map(|d| json!({"bytes":[d.start,d.end],"message":d.message,"utf16":positions(tree,d.start,d.end)})).collect();
    assert_eq!(json!(diagnostics), case["diagnostics"]);
    let mut end = 0;
    for token in elements.iter().filter(|e| e.token) {
        assert_eq!(token.start, end);
        assert!(
            tree.source().is_char_boundary(token.start)
                && tree.source().is_char_boundary(token.end)
        );
        end = token.end;
    }
    assert_eq!(end, snapshot.source().len());
    let eof = tree.position(end).unwrap();
    assert_eq!(
        json!({"line":eof.line,"character":eof.character}),
        case["eof_utf16"]
    );
    for stale in [
        SourceSnapshot::new(
            "other-document",
            snapshot.revision(),
            snapshot.kind(),
            snapshot.source(),
        )
        .unwrap(),
        SourceSnapshot::new(
            snapshot.document(),
            snapshot.revision() + 1,
            snapshot.kind(),
            snapshot.source(),
        )
        .unwrap(),
        SourceSnapshot::new(
            snapshot.document(),
            snapshot.revision(),
            FileKind::Script,
            snapshot.source(),
        )
        .unwrap(),
        SourceSnapshot::new(
            snapshot.document(),
            snapshot.revision(),
            snapshot.kind(),
            format!("{} ", snapshot.source()),
        )
        .unwrap(),
    ] {
        assert_eq!(
            result.tree_for(&stale).unwrap_err(),
            ParserError::StaleSnapshot
        );
    }
}
fn snapshot(case: &Value) -> SourceSnapshot {
    SourceSnapshot::new(
        case["snapshot"]["document"].as_str().unwrap(),
        case["snapshot"]["revision"].as_i64().unwrap(),
        FileKind::Source,
        case["source_utf8"].as_str().unwrap(),
    )
    .unwrap()
}
#[test]
fn frozen_production_copy_boundary_and_strict_ordinary_recovery() {
    let _functional = functional::serial();
    let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let healthy = &cases[2];
    for (case, observing) in [
        (&cases[1], true),
        (&cases[1], false),
        (&cases[0], true),
        (&cases[0], false),
    ] {
        let input = snapshot(case);
        let original = input.clone();
        let (result, evidence) = if observing {
            parser.parse_snapshot_observing_generic_copy(&input, &AtomicBool::new(false))
        } else {
            (
                parser.parse_snapshot(&input, &AtomicBool::new(false)),
                GenericCopyEvidence::Missing,
            )
        };
        eprintln!(
            "B11 first frozen request id={} observing={observing} result={:?} evidence={evidence:?}",
            case["id"],
            result.as_ref().map(|_| ())
        );
        let followup = snapshot(healthy);
        check(
            &parser
                .parse_snapshot(&followup, &AtomicBool::new(false))
                .unwrap(),
            &followup,
            healthy,
        );
        assert_eq!(input, original);
        let result = result.unwrap();
        check(&result, &input, case);
        if observing {
            let expected = if case["id"] == "active-positive-candidate" {
                GenericCopyRecord {
                    status: 1,
                    source_len: 68,
                    active_events: 1,
                    active_layers: 1,
                    production_bound: 57,
                    independent_min: 57,
                    window_start: 28,
                    actual_copy_end: 57,
                    next_start: 57,
                    next_end: 58,
                    next_fits_other_caps: 1,
                    reserved: 0,
                }
            } else {
                GenericCopyRecord {
                    source_len: 30,
                    ..GenericCopyRecord::default()
                }
            };
            assert_eq!(evidence, GenericCopyEvidence::Complete(expected));
        }
    }
}

// Exact data copied from the reviewed pre-runtime manifest; never regenerate from parser output.
const CASES: &str = r#"[
  {
    "id": "unclipped-control",
    "source_utf8": "class Box<T\nfun healthy() = 2\n",
    "snapshot": {
      "document": "b11/unclipped-control",
      "revision": 1,
      "kind": "Source",
      "source_equals_file": true
    },
    "declarations": [
      {
        "kind": "CLASS",
        "bytes": [
          0,
          11
        ],
        "text": "class Box<T",
        "immediate_parent_kind": "KT_FILE",
        "nearest_selected_parent_index": null,
        "direct_name": {
          "text": "Box",
          "bytes": [
            6,
            9
          ],
          "utf16": [
            {
              "line": 0,
              "character": 6
            },
            {
              "line": 0,
              "character": 9
            }
          ]
        },
        "utf16": [
          {
            "line": 0,
            "character": 0
          },
          {
            "line": 0,
            "character": 11
          }
        ]
      },
      {
        "kind": "FUN",
        "bytes": [
          12,
          29
        ],
        "text": "fun healthy() = 2",
        "immediate_parent_kind": "KT_FILE",
        "nearest_selected_parent_index": null,
        "direct_name": {
          "text": "healthy",
          "bytes": [
            16,
            23
          ],
          "utf16": [
            {
              "line": 1,
              "character": 4
            },
            {
              "line": 1,
              "character": 11
            }
          ]
        },
        "utf16": [
          {
            "line": 1,
            "character": 0
          },
          {
            "line": 1,
            "character": 17
          }
        ]
      }
    ],
    "diagnostics": [
      {
        "bytes": [
          11,
          11
        ],
        "message": "Missing '>'",
        "utf16": [
          {
            "line": 0,
            "character": 11
          },
          {
            "line": 0,
            "character": 11
          }
        ]
      }
    ],
    "eof_utf16": {
      "line": 2,
      "character": 0
    },
    "type_parameter_list": {
      "kind": "TYPE_PARAMETER_LIST",
      "bytes": [
        9,
        11
      ],
      "parent_selected_index": 0
    }
  },
  {
    "id": "active-positive-candidate",
    "source_utf8": "fun @A(object { class Box<T\nfun healthy() = 2\n}) Receiver.ext() = 0\n",
    "snapshot": {
      "document": "b11/active-positive-candidate",
      "revision": 1,
      "kind": "Source",
      "source_equals_file": true
    },
    "declarations": [
      {
        "kind": "FUN",
        "bytes": [
          0,
          67
        ],
        "text": "fun @A(object { class Box<T\nfun healthy() = 2\n}) Receiver.ext() = 0",
        "immediate_parent_kind": "KT_FILE",
        "nearest_selected_parent_index": null,
        "direct_name": {
          "text": "ext",
          "bytes": [
            58,
            61
          ],
          "utf16": [
            {
              "line": 2,
              "character": 12
            },
            {
              "line": 2,
              "character": 15
            }
          ]
        },
        "utf16": [
          {
            "line": 0,
            "character": 0
          },
          {
            "line": 2,
            "character": 21
          }
        ]
      },
      {
        "kind": "OBJECT_DECLARATION",
        "bytes": [
          7,
          47
        ],
        "text": "object { class Box<T\nfun healthy() = 2\n}",
        "immediate_parent_kind": "OBJECT_LITERAL",
        "nearest_selected_parent_index": 0,
        "direct_name": null,
        "utf16": [
          {
            "line": 0,
            "character": 7
          },
          {
            "line": 2,
            "character": 1
          }
        ]
      },
      {
        "kind": "CLASS",
        "bytes": [
          16,
          27
        ],
        "text": "class Box<T",
        "immediate_parent_kind": "CLASS_BODY",
        "nearest_selected_parent_index": 1,
        "direct_name": {
          "text": "Box",
          "bytes": [
            22,
            25
          ],
          "utf16": [
            {
              "line": 0,
              "character": 22
            },
            {
              "line": 0,
              "character": 25
            }
          ]
        },
        "utf16": [
          {
            "line": 0,
            "character": 16
          },
          {
            "line": 0,
            "character": 27
          }
        ]
      },
      {
        "kind": "FUN",
        "bytes": [
          28,
          45
        ],
        "text": "fun healthy() = 2",
        "immediate_parent_kind": "CLASS_BODY",
        "nearest_selected_parent_index": 1,
        "direct_name": {
          "text": "healthy",
          "bytes": [
            32,
            39
          ],
          "utf16": [
            {
              "line": 1,
              "character": 4
            },
            {
              "line": 1,
              "character": 11
            }
          ]
        },
        "utf16": [
          {
            "line": 1,
            "character": 0
          },
          {
            "line": 1,
            "character": 17
          }
        ]
      }
    ],
    "diagnostics": [
      {
        "bytes": [
          27,
          27
        ],
        "message": "Missing '>'",
        "utf16": [
          {
            "line": 0,
            "character": 27
          },
          {
            "line": 0,
            "character": 27
          }
        ]
      }
    ],
    "eof_utf16": {
      "line": 3,
      "character": 0
    },
    "type_parameter_list": {
      "kind": "TYPE_PARAMETER_LIST",
      "bytes": [
        25,
        27
      ],
      "parent_selected_index": 2
    }
  },
  {
    "id": "healthy-followup",
    "source_utf8": "fun healthy() = 2",
    "snapshot": {
      "document": "b11/healthy-followup",
      "revision": 1,
      "kind": "Source",
      "source_equals_file": true
    },
    "declarations": [
      {
        "kind": "FUN",
        "bytes": [
          0,
          17
        ],
        "text": "fun healthy() = 2",
        "immediate_parent_kind": "KT_FILE",
        "nearest_selected_parent_index": null,
        "direct_name": {
          "text": "healthy",
          "bytes": [
            4,
            11
          ],
          "utf16": [
            {
              "line": 0,
              "character": 4
            },
            {
              "line": 0,
              "character": 11
            }
          ]
        },
        "utf16": [
          {
            "line": 0,
            "character": 0
          },
          {
            "line": 0,
            "character": 17
          }
        ]
      }
    ],
    "diagnostics": [],
    "eof_utf16": {
      "line": 0,
      "character": 17
    },
    "type_parameter_list": null
  }
]"#;
