//! Production snapshot acceptance against unchanged, independently authored PR174 facts.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parser, SourceSnapshot, lowering::lower_declarations};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

fn acceptance(
    source: &str,
    expected: &str,
    declarations: usize,
    errors: usize,
    semantic_spellings: &[&str],
) {
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
    assert_eq!(semantic_spellings.len(), declarations);
    for ((record, fact), &spelling) in lowered
        .declarations()
        .iter()
        .zip(facts)
        .zip(semantic_spellings)
    {
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
        assert_eq!(record.semantic_spelling(), Some(spelling));
        let start = record.name_range().start + usize::from(record.raw_name().starts_with('`'));
        assert_eq!(
            record.semantic_spelling().unwrap().as_ptr(),
            parsed.source()[start..].as_ptr()
        );
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
        &[
            "Host",
            "same",
            "same",
            "Local",
            "inner",
            "local 🦀e\u{301}",
            "same",
        ],
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
        &["broken", "later", "healthy"],
    );
}

#[test]
fn production_reference_initializer_preserves_declarations_and_regions() {
    let _serial = functional::serial();
    let source = include_str!("../../../docs/specs/reference-binding-acceptance/same-block.kt");
    assert_eq!(source, "fun f() { val x = 1; val y = x }\n");
    let snapshot =
        SourceSnapshot::new("reference-binding/same-block", 1, FileKind::Source, source).unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let parsed = result.tree_for(&snapshot).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.elements().len(), 37);
    assert_eq!(parsed.elements().iter().filter(|e| e.token).count(), 27);
    let actual = parsed
        .elements()
        .iter()
        .map(|e| {
            (
                e.kind.as_str(),
                e.token,
                e.start,
                e.end,
                e.parent,
                e.subtree_end,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            ("KT_FILE", false, 0, 33, None, 37),
            ("PACKAGE_DIRECTIVE", false, 0, 0, Some(0), 2),
            ("IMPORT_LIST", false, 0, 0, Some(0), 3),
            ("FUN", false, 0, 32, Some(0), 36),
            ("FUN_KEYWORD", true, 0, 3, Some(3), 5),
            ("WHITE_SPACE", true, 3, 4, Some(3), 6),
            ("IDENTIFIER", true, 4, 5, Some(3), 7),
            ("VALUE_PARAMETER_LIST", false, 5, 7, Some(3), 10),
            ("LPAR", true, 5, 6, Some(7), 9),
            ("RPAR", true, 6, 7, Some(7), 10),
            ("WHITE_SPACE", true, 7, 8, Some(3), 11),
            ("BLOCK", false, 8, 32, Some(3), 36),
            ("LBRACE", true, 8, 9, Some(11), 13),
            ("WHITE_SPACE", true, 9, 10, Some(11), 14),
            ("PROPERTY", false, 10, 19, Some(11), 23),
            ("VAL_KEYWORD", true, 10, 13, Some(14), 16),
            ("WHITE_SPACE", true, 13, 14, Some(14), 17),
            ("IDENTIFIER", true, 14, 15, Some(14), 18),
            ("WHITE_SPACE", true, 15, 16, Some(14), 19),
            ("EQ", true, 16, 17, Some(14), 20),
            ("WHITE_SPACE", true, 17, 18, Some(14), 21),
            ("INTEGER_CONSTANT", false, 18, 19, Some(14), 23),
            ("INTEGER_LITERAL", true, 18, 19, Some(21), 23),
            ("SEMICOLON", true, 19, 20, Some(11), 24),
            ("WHITE_SPACE", true, 20, 21, Some(11), 25),
            ("PROPERTY", false, 21, 30, Some(11), 34),
            ("VAL_KEYWORD", true, 21, 24, Some(25), 27),
            ("WHITE_SPACE", true, 24, 25, Some(25), 28),
            ("IDENTIFIER", true, 25, 26, Some(25), 29),
            ("WHITE_SPACE", true, 26, 27, Some(25), 30),
            ("EQ", true, 27, 28, Some(25), 31),
            ("WHITE_SPACE", true, 28, 29, Some(25), 32),
            ("REFERENCE_EXPRESSION", false, 29, 30, Some(25), 34),
            ("IDENTIFIER", true, 29, 30, Some(32), 34),
            ("WHITE_SPACE", true, 30, 31, Some(11), 35),
            ("RBRACE", true, 31, 32, Some(11), 36),
            ("WHITE_SPACE", true, 32, 33, Some(0), 37),
        ]
    );
    let lower = lower_declarations(&result, &snapshot).unwrap();
    assert!(std::ptr::eq(lower.snapshot(), result.snapshot()));
    assert!(lower.errors().is_empty());
    assert_eq!(
        lower
            .declarations()
            .iter()
            .map(|d| (
                d.cst_index(),
                d.kind(),
                d.raw_name(),
                d.range(),
                d.name_range(),
                d.cst_parent_index(),
                d.owner(),
                d.incomplete()
            ))
            .collect::<Vec<_>>(),
        [
            (3, "FUN", "f", 0..32, 4..5, Some(0), None, false),
            (
                14,
                "PROPERTY",
                "x",
                10..19,
                14..15,
                Some(11),
                Some(0),
                false
            ),
            (
                25,
                "PROPERTY",
                "y",
                21..30,
                25..26,
                Some(11),
                Some(0),
                false
            ),
        ]
    );
    let directory =
        ktlsp_parser::structural_regions::build_regions(&lower, &result, &snapshot).unwrap();
    assert!(std::ptr::eq(directory.lowered(), &lower));
    assert!(std::ptr::eq(directory.snapshot(), result.snapshot()));
    assert_eq!(
        directory
            .regions()
            .iter()
            .map(|r| (
                r.cst_index(),
                r.kind(),
                r.range(),
                r.subtree_end(),
                r.parent(),
                r.introducer()
            ))
            .collect::<Vec<_>>(),
        [
            (0, "KT_FILE", 0..33, 37, None, None),
            (11, "BLOCK", 8..32, 36, Some(0), Some(0)),
        ]
    );
    assert_eq!(
        directory
            .memberships()
            .iter()
            .map(|m| m.region())
            .collect::<Vec<_>>(),
        [0, 1, 1]
    );
    let site = ktlsp_parser::lowering::reference_site(&lower, &result, &snapshot, 32).unwrap();
    assert!(std::ptr::eq(site.origin(), &lower));
    assert_eq!(site.expression_index(), 32);
    assert_eq!(site.token_index(), 33);
    assert_eq!(site.range(), 29..30);
    assert_eq!(site.raw_spelling(), "x");
    assert_eq!(site.semantic_spelling(), Some("x"));
    assert_eq!(
        site.raw_spelling().as_ptr(),
        parsed.source()[29..30].as_ptr()
    );
    assert!(std::ptr::eq(
        site.containing_property(),
        &lower.declarations()[2]
    ));
    assert_eq!(site.body_index(), 11);
    assert!(std::ptr::eq(site.function(), &lower.declarations()[0]));
    let binding =
        ktlsp_parser::local_value_binding::bind_local_value(&directory, &site, &snapshot).unwrap();
    assert!(std::ptr::eq(binding.site(), &site));
    assert!(std::ptr::eq(binding.body(), &directory.regions()[1]));
    assert!(std::ptr::eq(binding.target(), &lower.declarations()[1]));
    assert_eq!(binding.target().cst_index(), 14);
    assert_eq!(binding.target().range(), 10..19);
    assert_eq!(binding.target().name_range(), 14..15);
}
