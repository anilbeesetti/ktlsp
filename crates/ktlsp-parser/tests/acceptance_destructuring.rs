//! One source-derived local destructuring fixture; runtime evidence is separate.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use ktlsp_text::{Position, TextError};
use std::sync::atomic::AtomicBool;

const SOURCE: &str =
    "fun split(value: Pair<Int, Int>) { /*😀*/\n    val (first, second) = value\n}";
const DOCUMENT: &str = "destructuring-acceptance";

// Parent/owner indices refer to these expected rows, never raw CST element IDs.
type Node<'a> = (&'a str, usize, usize, Option<usize>, Option<usize>);
type Token<'a> = (&'a str, &'a str, usize, usize, usize);
type Name<'a> = (&'a str, usize, usize, usize);

const NODES: &[Node<'_>] = &[
    ("KT_FILE", 0, 77, None, None),
    ("FUN", 0, 77, Some(0), None),
    ("VALUE_PARAMETER_LIST", 9, 32, Some(1), Some(1)),
    ("VALUE_PARAMETER", 10, 31, Some(2), Some(1)),
    ("TYPE_REFERENCE", 17, 31, Some(3), Some(1)),
    ("USER_TYPE", 17, 31, Some(4), Some(1)),
    ("REFERENCE_EXPRESSION", 17, 21, Some(5), Some(1)),
    ("TYPE_ARGUMENT_LIST", 21, 31, Some(5), Some(1)),
    ("TYPE_PROJECTION", 22, 25, Some(7), Some(1)),
    ("TYPE_REFERENCE", 22, 25, Some(8), Some(1)),
    ("USER_TYPE", 22, 25, Some(9), Some(1)),
    ("REFERENCE_EXPRESSION", 22, 25, Some(10), Some(1)),
    ("TYPE_PROJECTION", 27, 30, Some(7), Some(1)),
    ("TYPE_REFERENCE", 27, 30, Some(12), Some(1)),
    ("USER_TYPE", 27, 30, Some(13), Some(1)),
    ("REFERENCE_EXPRESSION", 27, 30, Some(14), Some(1)),
    ("BLOCK", 33, 77, Some(1), Some(1)),
    ("DESTRUCTURING_DECLARATION", 48, 75, Some(16), Some(1)),
    (
        "DESTRUCTURING_DECLARATION_ENTRY",
        53,
        58,
        Some(17),
        Some(17),
    ),
    (
        "DESTRUCTURING_DECLARATION_ENTRY",
        60,
        66,
        Some(17),
        Some(17),
    ),
    ("REFERENCE_EXPRESSION", 70, 75, Some(17), Some(17)),
];
const TOKENS: &[Token<'_>] = &[
    ("FUN_KEYWORD", "fun", 0, 3, 1),
    ("WHITE_SPACE", " ", 3, 4, 1),
    ("IDENTIFIER", "split", 4, 9, 1),
    ("LPAR", "(", 9, 10, 2),
    ("IDENTIFIER", "value", 10, 15, 3),
    ("COLON", ":", 15, 16, 3),
    ("WHITE_SPACE", " ", 16, 17, 3),
    ("IDENTIFIER", "Pair", 17, 21, 6),
    ("LT", "<", 21, 22, 7),
    ("IDENTIFIER", "Int", 22, 25, 11),
    ("COMMA", ",", 25, 26, 7),
    ("WHITE_SPACE", " ", 26, 27, 7),
    ("IDENTIFIER", "Int", 27, 30, 15),
    ("GT", ">", 30, 31, 7),
    ("RPAR", ")", 31, 32, 2),
    ("WHITE_SPACE", " ", 32, 33, 1),
    ("LBRACE", "{", 33, 34, 16),
    ("WHITE_SPACE", " ", 34, 35, 16),
    ("BLOCK_COMMENT", "/*😀*/", 35, 43, 16),
    ("WHITE_SPACE", "\n    ", 43, 48, 16),
    ("VAL_KEYWORD", "val", 48, 51, 17),
    ("WHITE_SPACE", " ", 51, 52, 17),
    ("LPAR", "(", 52, 53, 17),
    ("IDENTIFIER", "first", 53, 58, 18),
    ("COMMA", ",", 58, 59, 17),
    ("WHITE_SPACE", " ", 59, 60, 17),
    ("IDENTIFIER", "second", 60, 66, 19),
    ("RPAR", ")", 66, 67, 17),
    ("WHITE_SPACE", " ", 67, 68, 17),
    ("EQ", "=", 68, 69, 17),
    ("WHITE_SPACE", " ", 69, 70, 17),
    ("IDENTIFIER", "value", 70, 75, 20),
    ("WHITE_SPACE", "\n", 75, 76, 16),
    ("RBRACE", "}", 76, 77, 16),
];
const NAMES: &[Name<'_>] = &[
    ("split", 4, 9, 1),
    ("value", 10, 15, 3),
    ("Pair", 17, 21, 6),
    ("Int", 22, 25, 11),
    ("Int", 27, 30, 15),
    ("first", 53, 58, 18),
    ("second", 60, 66, 19),
    ("value", 70, 75, 20),
];
const POSITIONS: &[(usize, u32, u32)] = &[
    (0, 0, 0),
    (1, 0, 1),
    (2, 0, 2),
    (3, 0, 3),
    (4, 0, 4),
    (5, 0, 5),
    (6, 0, 6),
    (7, 0, 7),
    (8, 0, 8),
    (9, 0, 9),
    (10, 0, 10),
    (11, 0, 11),
    (12, 0, 12),
    (13, 0, 13),
    (14, 0, 14),
    (15, 0, 15),
    (16, 0, 16),
    (17, 0, 17),
    (18, 0, 18),
    (19, 0, 19),
    (20, 0, 20),
    (21, 0, 21),
    (22, 0, 22),
    (23, 0, 23),
    (24, 0, 24),
    (25, 0, 25),
    (26, 0, 26),
    (27, 0, 27),
    (28, 0, 28),
    (29, 0, 29),
    (30, 0, 30),
    (31, 0, 31),
    (32, 0, 32),
    (33, 0, 33),
    (34, 0, 34),
    (35, 0, 35),
    (36, 0, 36),
    (37, 0, 37),
    (41, 0, 39),
    (42, 0, 40),
    (43, 0, 41),
    (44, 1, 0),
    (45, 1, 1),
    (46, 1, 2),
    (47, 1, 3),
    (48, 1, 4),
    (49, 1, 5),
    (50, 1, 6),
    (51, 1, 7),
    (52, 1, 8),
    (53, 1, 9),
    (54, 1, 10),
    (55, 1, 11),
    (56, 1, 12),
    (57, 1, 13),
    (58, 1, 14),
    (59, 1, 15),
    (60, 1, 16),
    (61, 1, 17),
    (62, 1, 18),
    (63, 1, 19),
    (64, 1, 20),
    (65, 1, 21),
    (66, 1, 22),
    (67, 1, 23),
    (68, 1, 24),
    (69, 1, 25),
    (70, 1, 26),
    (71, 1, 27),
    (72, 1, 28),
    (73, 1, 29),
    (74, 1, 30),
    (75, 1, 31),
    (76, 2, 0),
    (77, 2, 1),
];

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "FUN"
            | "CLASS"
            | "OBJECT_DECLARATION"
            | "PROPERTY"
            | "TYPEALIAS"
            | "ENUM_ENTRY"
            | "DESTRUCTURING_DECLARATION"
            | "DESTRUCTURING_DECLARATION_ENTRY"
    )
}

fn check_source(tree: &Parsed, source: &str) {
    assert_eq!(tree.source(), source);
    assert!(tree.diagnostics().is_empty());
    assert!(!tree.elements().iter().any(|e| e.kind == "ERROR_ELEMENT"));
    let mut cursor = 0;
    let mut spelling = String::new();
    for token in tree.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor);
        assert!(token.start < token.end);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        spelling.push_str(&source[token.start..token.end]);
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
    assert_eq!(spelling, source);
}

fn check_nodes(tree: &Parsed, expected: &[Node<'_>]) -> Vec<usize> {
    let elements = tree.elements();
    let ids: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.start < e.end)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), expected.len());
    assert_eq!(ids[0], 0);
    for (&id, &(kind, start, end, parent, owner)) in ids.iter().zip(expected) {
        let e = &elements[id];
        assert_eq!((e.kind.as_str(), e.start, e.end), (kind, start, end));
        assert_eq!(
            elements
                .iter()
                .filter(|e| !e.token && e.kind == kind && e.start == start && e.end == end)
                .count(),
            1
        );
        assert_eq!(e.parent, parent.map(|index| ids[index]));
        let mut ancestor = e.parent;
        while let Some(p) = ancestor {
            if declaration(&elements[p].kind) {
                break;
            }
            ancestor = elements[p].parent;
        }
        assert_eq!(ancestor, owner.map(|index| ids[index]));
    }
    ids
}

fn check_names(tree: &Parsed, ids: &[usize], expected: &[Name<'_>]) {
    let actual: Vec<_> = tree
        .elements()
        .iter()
        .filter(|e| e.token && e.kind == "IDENTIFIER")
        .map(|e| {
            (
                &tree.source()[e.start..e.end],
                e.start,
                e.end,
                e.parent.unwrap(),
            )
        })
        .collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|&(name, start, end, parent)| (name, start, end, ids[parent]))
        .collect();
    assert_eq!(actual, expected);
}

fn check_destructuring(tree: &Parsed) {
    check_source(tree, SOURCE);
    let ids = check_nodes(tree, NODES);
    check_names(tree, &ids, NAMES);
    let actual: Vec<_> = tree
        .elements()
        .iter()
        .filter(|e| e.token)
        .map(|e| {
            (
                e.kind.as_str(),
                &tree.source()[e.start..e.end],
                e.start,
                e.end,
                e.parent.unwrap(),
            )
        })
        .collect();
    let expected: Vec<_> = TOKENS
        .iter()
        .map(|&(kind, spelling, start, end, parent)| (kind, spelling, start, end, ids[parent]))
        .collect();
    assert_eq!(actual, expected);
    for &(offset, line, character) in POSITIONS {
        assert_eq!(tree.position(offset).unwrap(), Position { line, character });
    }
    for offset in [38, 39, 40, 78] {
        assert_eq!(
            tree.position(offset).unwrap_err(),
            TextError::InvalidOffset(offset)
        );
    }
}

fn check_snapshot(snapshot: &SourceSnapshot, revision: i64, source: &str) {
    assert_eq!(snapshot.document(), DOCUMENT);
    assert_eq!(snapshot.revision(), revision);
    assert_eq!(snapshot.kind(), FileKind::Source);
    assert_eq!(snapshot.source(), source);
}

#[test]
fn local_destructuring_preserves_entries_parents_positions_and_snapshot() {
    let _functional = functional::serial();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new(DOCUMENT, 1, FileKind::Source, SOURCE).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    check_snapshot(result.snapshot(), 1, SOURCE);
    check_destructuring(result.tree_for(&snapshot).unwrap());

    for equal in [
        snapshot.clone(),
        SourceSnapshot::new(DOCUMENT.to_owned(), 1, FileKind::Source, SOURCE.to_owned()).unwrap(),
    ] {
        assert_eq!(equal, snapshot);
        check_destructuring(result.tree_for(&equal).unwrap());
    }
    for stale in [
        SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, SOURCE).unwrap(),
        SourceSnapshot::new(
            "destructuring-acceptance-other",
            1,
            FileKind::Source,
            SOURCE,
        )
        .unwrap(),
        SourceSnapshot::new(DOCUMENT, 1, FileKind::Script, SOURCE).unwrap(),
        SourceSnapshot::new(
            DOCUMENT,
            1,
            FileKind::Source,
            SOURCE.replacen("first", "third", 1),
        )
        .unwrap(),
    ] {
        assert_eq!(
            result.tree_for(&stale).unwrap_err(),
            ParserError::StaleSnapshot
        );
    }
    assert_eq!(result.snapshot(), &snapshot);
    check_snapshot(&snapshot, 1, SOURCE);
    check_destructuring(result.tree_for(&snapshot).unwrap());

    let healthy = SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, "fun healthy() = 2").unwrap();
    assert_eq!(
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    let followup = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(followup.snapshot(), &healthy);
    check_snapshot(followup.snapshot(), 2, "fun healthy() = 2");
    let tree = followup.tree_for(&healthy).unwrap();
    check_source(tree, "fun healthy() = 2");
    let ids = check_nodes(
        tree,
        &[
            ("KT_FILE", 0, 17, None, None),
            ("FUN", 0, 17, Some(0), None),
            ("VALUE_PARAMETER_LIST", 11, 13, Some(1), Some(1)),
            ("INTEGER_CONSTANT", 16, 17, Some(1), Some(1)),
        ],
    );
    check_names(tree, &ids, &[("healthy", 4, 11, 1)]);
    for offset in 0..=17 {
        assert_eq!(
            tree.position(offset).unwrap(),
            Position {
                line: 0,
                character: offset as u32
            }
        );
    }
    assert_eq!(
        followup.tree_for(&snapshot).unwrap_err(),
        ParserError::StaleSnapshot
    );
    assert_eq!(
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    assert_eq!(result.snapshot(), &snapshot);
    assert_eq!(followup.snapshot(), &healthy);
    check_snapshot(&snapshot, 1, SOURCE);
    check_snapshot(&healthy, 2, "fun healthy() = 2");
    check_destructuring(result.tree_for(&snapshot).unwrap());
}
