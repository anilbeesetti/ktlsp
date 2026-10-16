//! Unchanged production fixtures; runtime execution is separately admitted.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SourceSnapshot, ancestor_candidates::ancestor_candidates,
    declaration_name_index::build_name_index, lowering::lower_declarations,
    structural_regions::build_regions,
};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

fn acceptance(source: &str, expected: &str, nested: bool) {
    let facts: Value = serde_json::from_str(expected).unwrap();
    let identity = &facts["source"];
    assert_eq!(source.len(), identity["bytes"].as_u64().unwrap() as usize);
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
        .unwrap();
    let lower = lower_declarations(&result, &snapshot).unwrap();
    let directory = build_regions(&lower, &result, &snapshot).unwrap();
    let index = build_name_index(&directory).unwrap();
    let check = |start: usize, query: &str, expected: &[(usize, &[usize])]| {
        let found = ancestor_candidates(&index, &directory.regions()[start], query).unwrap();
        assert!(std::ptr::eq(found.directory(), &directory));
        let actual = found
            .groups()
            .map(|g| {
                let region = directory
                    .regions()
                    .iter()
                    .position(|r| std::ptr::eq(r, g.region()))
                    .unwrap();
                let original = index.find(g.region(), query).unwrap().unwrap();
                assert!(std::ptr::eq(g.raw_name(), original.raw_name()));
                assert!(std::ptr::eq(
                    g.declaration_ordinals(),
                    original.declaration_ordinals()
                ));
                for &i in g.declaration_ordinals() {
                    assert_eq!(
                        g.raw_name(),
                        facts["declarations"][i]["raw_name"].as_str().unwrap()
                    );
                    assert_eq!(&source[lower.declarations()[i].name_range()], g.raw_name());
                }
                (region, g.declaration_ordinals())
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    };
    if nested {
        assert_eq!(
            directory
                .regions()
                .iter()
                .map(|r| r.parent())
                .collect::<Vec<_>>(),
            [None, Some(0), Some(1), Some(2)]
        );
        for start in [1, 2, 3] {
            check(start, "same", &[(1, &[1, 2]), (0, &[6])]);
        }
        check(0, "same", &[(0, &[6])]);
        check(3, "inner", &[(2, &[4])]);
        check(3, "Local", &[(2, &[3])]);
        check(1, "inner", &[]);
        check(1, "Local", &[]);
        check(3, "Host", &[(0, &[0])]);
        check(3, "`local 🦀e\u{301}`", &[(2, &[5])]);
        for query in [
            "local 🦀e\u{301}",
            "`local 🦀é`",
            "Same",
            "",
            "\0",
            "absent",
        ] {
            check(3, query, &[]);
        }
        assert_eq!(
            (
                lower.declarations()[1].kind(),
                lower.declarations()[2].kind()
            ),
            ("PROPERTY", "FUN")
        );
        assert!(lower.errors().is_empty());
    } else {
        for (name, ordinal) in [("broken", 0), ("later", 1), ("healthy", 2)] {
            check(0, name, &[(0, &[ordinal])]);
        }
        assert_eq!(lower.errors().len(), 1);
        let error = &lower.errors()[0];
        assert_eq!(
            (error.range(), error.owner(), error.message()),
            (11..11, Some(0), "Expecting ')'")
        );
        assert_eq!(
            lower
                .declarations()
                .iter()
                .map(|d| d.incomplete())
                .collect::<Vec<_>>(),
            [true, false, false]
        );
    }
}

#[test]
fn production_complete_ancestors_keep_outer_hits_duplicates_and_raw_names() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/declarations.json"),
        true,
    );
}
#[test]
fn production_incomplete_declaration_and_error_remain_unchanged_data() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/error-node.json"),
        false,
    );
}
