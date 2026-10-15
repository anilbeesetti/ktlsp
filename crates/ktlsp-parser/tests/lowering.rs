//! Production snapshot acceptance against unchanged, independently authored PR174 facts.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parser, SourceSnapshot, lowering::lower_declarations};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

fn acceptance(source: &str, expected: &str, declarations: usize, errors: usize) {
    let expected: Value = serde_json::from_str(expected).unwrap();
    let identity = &expected["source"];
    assert_eq!(source.len(), identity["bytes"].as_u64().unwrap() as usize);
    assert_eq!(identity["file_kind"], "Source");
    let snapshot = SourceSnapshot::new(
        identity["document"].as_str().unwrap(),
        identity["revision"].as_i64().unwrap(),
        FileKind::Source,
        source,
    )
    .unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .expect("the exact fixture must pass the production snapshot validator");
    let parsed = result.tree_for(&snapshot).unwrap();
    let lowered = lower_declarations(&result, &snapshot).unwrap();
    assert_eq!(lowered.snapshot(), &snapshot);
    assert_eq!(lowered.declarations().len(), declarations);
    assert_eq!(lowered.errors().len(), errors);
    let facts = expected["declarations"].as_array().unwrap();
    assert_eq!(facts.len(), declarations);
    let owner = |value: &Value| {
        value.as_str().map(|label| {
            facts
                .iter()
                .position(|fact| fact["label"] == label)
                .unwrap()
        })
    };
    for (record, fact) in lowered.declarations().iter().zip(facts) {
        assert_eq!(record.kind(), fact["kind"].as_str().unwrap());
        assert_eq!(record.raw_name(), fact["raw_name"].as_str().unwrap());
        assert_eq!(
            serde_json::json!([record.range().start, record.range().end]),
            fact["range"]
        );
        assert_eq!(
            serde_json::json!([record.name_range().start, record.name_range().end]),
            fact["name_range"]
        );
        assert_eq!(
            record.cst_parent_kind(),
            fact["cst_parent_kind"].as_str().unwrap()
        );
        assert_eq!(record.owner(), owner(&fact["owner"]));
        assert_eq!(record.incomplete(), fact["incomplete"].as_bool().unwrap());
        let node = &parsed.elements()[record.cst_index()];
        assert_eq!(record.cst_parent_index(), node.parent);
        assert_eq!(record.range(), node.start..node.end);
        assert_eq!(&source[record.name_range()], record.raw_name());
    }
    let facts = expected["errors"].as_array().unwrap();
    assert_eq!(facts.len(), errors);
    for (record, fact) in lowered.errors().iter().zip(facts) {
        assert_eq!(record.message(), fact["message"].as_str().unwrap());
        assert_eq!(
            serde_json::json!([record.range().start, record.range().end]),
            fact["range"]
        );
        assert_eq!(record.owner(), owner(&fact["owner"]));
        let node = &parsed.elements()[record.cst_index()];
        assert_eq!(node.kind, "ERROR_ELEMENT");
        assert_eq!(record.range(), node.start..node.end);
    }
}

#[test]
fn production_declarations_preserve_all_seven_literal_records() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/declarations.json"),
        7,
        0,
    );
}

#[test]
fn production_error_preserves_three_declarations_and_one_ancestral_error() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/error-node.json"),
        3,
        1,
    );
}
