//! Actual production snapshots and unchanged PR174 literals; no synthetic public input.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SourceSnapshot,
    lowering::lower_declarations,
    structural_regions::{Membership, build_regions},
};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

fn acceptance(source: &str, expected: &str, declarations: usize, errors: usize) {
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
    let directory = build_regions(&lower, &result, &snapshot.clone()).unwrap();
    assert_eq!(directory.snapshot(), &snapshot);
    assert_eq!(lower.declarations().len(), declarations);
    assert_eq!(lower.errors().len(), errors);
    let rows = facts["declarations"].as_array().unwrap();
    for (d, f) in lower.declarations().iter().zip(rows) {
        assert_eq!(d.raw_name(), f["raw_name"].as_str().unwrap());
        assert_eq!(d.kind(), f["kind"].as_str().unwrap());
        assert_eq!(
            [d.range().start, d.range().end],
            [
                f["range"][0].as_u64().unwrap() as usize,
                f["range"][1].as_u64().unwrap() as usize
            ]
        );
    }
    let actual = directory
        .regions()
        .iter()
        .map(|r| {
            (
                r.cst_index(),
                r.kind(),
                r.range(),
                r.subtree_end(),
                r.parent(),
                r.introducer(),
            )
        })
        .collect::<Vec<_>>();
    let memberships = directory
        .memberships()
        .iter()
        .map(Membership::region)
        .collect::<Vec<_>>();
    if declarations == 7 {
        assert_eq!(
            actual,
            [
                (0, "KT_FILE", 0..146, 76, None, None),
                (8, "CLASS_BODY", 11..129, 62, Some(0), Some(0)),
                (29, "BLOCK", 45..127, 60, Some(1), Some(2)),
                (45, "BLOCK", 87..89, 48, Some(2), Some(4))
            ]
        );
        assert_eq!(memberships, [0, 1, 1, 2, 2, 2, 0]);
        assert_eq!(
            lower
                .declarations()
                .iter()
                .map(|d| d.cst_index())
                .collect::<Vec<_>>(),
            [3, 11, 21, 32, 37, 49, 63]
        );
        assert!(!memberships.contains(&3));
    } else {
        assert_eq!(actual, [(0, "KT_FILE", 0..44, 34, None, None)]);
        assert_eq!(memberships, [0, 0, 0]);
        assert_eq!(
            lower
                .declarations()
                .iter()
                .map(|d| d.cst_index())
                .collect::<Vec<_>>(),
            [3, 11, 21]
        );
        let e = &directory.lowered().errors()[0];
        assert_eq!(
            (e.cst_index(), e.range(), e.owner(), e.message()),
            (9, 11..11, Some(0), "Expecting ')'")
        );
        assert!(lower.declarations()[0].incomplete());
    }
}

#[test]
fn production_four_regions_preserve_seven_memberships_and_empty_block() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/declarations.json"),
        7,
        0,
    );
}
#[test]
fn production_file_memberships_preserve_zero_width_error_owner() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
        include_str!("../../../docs/specs/lowering-acceptance/error-node.json"),
        3,
        1,
    );
}
