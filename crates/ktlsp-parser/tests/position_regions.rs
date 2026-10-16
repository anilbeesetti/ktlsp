//! Literal queries over the unchanged accepted production fixtures.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SourceSnapshot,
    lowering::lower_declarations,
    position_regions::{PositionRegionError, containing_regions},
    structural_regions::build_regions,
};
use std::{ops::Range, sync::atomic::AtomicBool};

fn acceptance(
    source: &str,
    document: &str,
    ranges: &[Range<usize>],
    queries: &[(usize, &[usize])],
    errors: &[(usize, PositionRegionError)],
) {
    let snapshot = SourceSnapshot::new(document, 1, FileKind::Source, source).unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let lowered = lower_declarations(&result, &snapshot).unwrap();
    let directory = build_regions(&lowered, &result, &snapshot).unwrap();
    let other = build_regions(&lowered, &result, &snapshot).unwrap();
    assert_eq!(
        directory
            .regions()
            .iter()
            .map(|r| r.range())
            .collect::<Vec<_>>(),
        ranges
    );
    for &(offset, expected) in queries {
        // A temporary reference binding must not shorten the returned borrow.
        let mut actual = {
            let borrowed = &directory;
            containing_regions(borrowed, offset).unwrap()
        };
        for &ordinal in expected {
            let region = actual.next().unwrap();
            assert!(std::ptr::eq(region, &directory.regions()[ordinal]));
            assert!(!std::ptr::eq(region, &other.regions()[ordinal]));
        }
        assert!(actual.next().is_none(), "offset {offset}");
        assert!(actual.next().is_none());
        let mut actual_other = containing_regions(&other, offset).unwrap();
        for &ordinal in expected {
            assert!(std::ptr::eq(
                actual_other.next().unwrap(),
                &other.regions()[ordinal]
            ));
        }
        assert!(actual_other.next().is_none());
    }
    for &(offset, expected) in errors {
        assert_eq!(containing_regions(&directory, offset).err(), Some(expected));
    }
}

#[test]
fn production_nested_braces_ends_unicode_and_eof() {
    let _serial = functional::serial();
    use PositionRegionError::{NotCharBoundary, OffsetOutOfBounds};
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
        "lowering-acceptance/declarations",
        &[0..146, 11..129, 45..127, 87..89],
        &[
            (0, &[0]),
            (10, &[0]),
            (11, &[0, 1]),
            (12, &[0, 1]),
            (44, &[0, 1]),
            (45, &[0, 1, 2]),
            (46, &[0, 1, 2]),
            (86, &[0, 1, 2]),
            (87, &[0, 1, 2, 3]),
            (88, &[0, 1, 2, 3]),
            (89, &[0, 1, 2]),
            (98, &[0, 1, 2]),
            (109, &[0, 1, 2]),
            (113, &[0, 1, 2]),
            (114, &[0, 1, 2]),
            (116, &[0, 1, 2]),
            (121, &[0, 1, 2]),
            (126, &[0, 1, 2]),
            (127, &[0, 1]),
            (128, &[0, 1]),
            (129, &[0]),
            (130, &[0]),
            (131, &[0]),
            (145, &[0]),
            (146, &[]),
        ],
        &[
            (110, NotCharBoundary),
            (111, NotCharBoundary),
            (112, NotCharBoundary),
            (115, NotCharBoundary),
            (147, OffsetOutOfBounds),
            (usize::MAX, OffsetOutOfBounds),
        ],
    );
}

#[test]
fn production_incomplete_declaration_error_position_and_eof() {
    let _serial = functional::serial();
    acceptance(
        include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
        "lowering-acceptance/error-node",
        std::slice::from_ref(&(0..44)),
        &[
            (0, &[0]),
            (10, &[0]),
            (11, &[0]),
            (12, &[0]),
            (25, &[0]),
            (26, &[0]),
            (43, &[0]),
            (44, &[]),
        ],
        &[(45, PositionRegionError::OffsetOutOfBounds)],
    );
}
