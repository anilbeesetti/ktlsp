//! Two unchanged production fixtures; native runtime execution is separately admitted.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SourceSnapshot, declaration_name_index::build_name_index,
    lowering::lower_declarations, structural_regions::build_regions,
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
    assert!(std::ptr::eq(index.directory(), &directory));
    let actual = index
        .groups()
        .map(|g| {
            let region = directory
                .regions()
                .iter()
                .position(|r| std::ptr::eq(r, g.region()))
                .unwrap();
            (region, g.raw_name(), g.declaration_ordinals())
        })
        .collect::<Vec<_>>();
    if nested {
        assert_eq!(
            actual,
            [
                (0, "Host", &[0][..]),
                (1, "same", &[1, 2][..]),
                (2, "Local", &[3][..]),
                (2, "inner", &[4][..]),
                (2, "`local 🦀e\u{301}`", &[5][..]),
                (0, "same", &[6][..]),
            ]
        );
        assert_eq!(directory.regions().len(), 4);
        for (region, ordinals) in [(0, &[6][..]), (1, &[1, 2][..])] {
            let group = index
                .find(&directory.regions()[region], "same")
                .unwrap()
                .unwrap();
            assert_eq!(group.declaration_ordinals(), ordinals);
        }
        for region in [2, 3] {
            assert!(
                index
                    .find(&directory.regions()[region], "same")
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            index
                .find(&directory.regions()[3], "inner")
                .unwrap()
                .is_none()
        );
        assert!(
            index
                .find(&directory.regions()[2], "local 🦀e\u{301}")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            (
                lower.declarations()[1].kind(),
                lower.declarations()[2].kind()
            ),
            ("PROPERTY", "FUN")
        );
        assert!(lower.errors().is_empty());
    } else {
        assert_eq!(
            actual,
            [
                (0, "broken", &[0][..]),
                (0, "later", &[1][..]),
                (0, "healthy", &[2][..])
            ]
        );
        assert_eq!(lower.errors().len(), 1);
        let e = &lower.errors()[0];
        assert_eq!(
            (e.range(), e.owner(), e.message()),
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
        assert_eq!(
            index
                .find(&directory.regions()[0], "broken")
                .unwrap()
                .unwrap()
                .declaration_ordinals(),
            [0]
        );
    }
    let rows = facts["declarations"].as_array().unwrap();
    assert_eq!(
        index
            .groups()
            .map(|g| g.declaration_ordinals().len())
            .sum::<usize>(),
        rows.len()
    );
    for group in index.groups() {
        let first = &lower.declarations()[group.declaration_ordinals()[0]];
        assert!(std::ptr::eq(group.raw_name(), first.raw_name()));
        for &ordinal in group.declaration_ordinals() {
            assert_eq!(
                group.raw_name(),
                rows[ordinal]["raw_name"].as_str().unwrap()
            );
            let name = lower.declarations()[ordinal].name_range();
            assert_eq!(&source[name], group.raw_name());
        }
    }
}

#[test]
fn production_raw_groups_preserve_duplicates_regions_and_empty_block() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/declarations.json"),
        true,
    );
}
#[test]
fn production_incomplete_name_and_zero_width_error_remain_data() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/error-node.json"),
        false,
    );
}
