//! One source-derived constructor-delegation fixture; runtime evidence is separate.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use ktlsp_text::{Position, TextError};
use std::sync::atomic::AtomicBool;

const SOURCE: &str = "class Box() { constructor(value: String) : this() {} }";
const HEALTHY: &str = "fun healthy() = 2";
const DOCUMENT: &str = "constructor-delegation-acceptance";

// Parent/owner indices refer to expected rows, never raw CST element IDs.
type Node<'a> = (&'a str, usize, usize, Option<usize>, Option<usize>);
type Token<'a> = (&'a str, &'a str, usize, usize, usize);
type Name<'a> = (&'a str, usize, usize, usize);

const NODES: &[Node<'_>] = &[
    ("KT_FILE", 0, 54, None, None),
    ("CLASS", 0, 54, Some(0), None),
    ("PRIMARY_CONSTRUCTOR", 9, 11, Some(1), Some(1)),
    ("VALUE_PARAMETER_LIST", 9, 11, Some(2), Some(2)),
    ("CLASS_BODY", 12, 54, Some(1), Some(1)),
    ("SECONDARY_CONSTRUCTOR", 14, 52, Some(4), Some(1)),
    ("VALUE_PARAMETER_LIST", 25, 40, Some(5), Some(5)),
    ("VALUE_PARAMETER", 26, 39, Some(6), Some(5)),
    ("TYPE_REFERENCE", 33, 39, Some(7), Some(5)),
    ("USER_TYPE", 33, 39, Some(8), Some(5)),
    ("REFERENCE_EXPRESSION", 33, 39, Some(9), Some(5)),
    ("CONSTRUCTOR_DELEGATION_CALL", 43, 49, Some(5), Some(5)),
    (
        "CONSTRUCTOR_DELEGATION_REFERENCE",
        43,
        47,
        Some(11),
        Some(5),
    ),
    ("VALUE_ARGUMENT_LIST", 47, 49, Some(11), Some(5)),
    ("BLOCK", 50, 52, Some(5), Some(5)),
];

const TOKENS: &[Token<'_>] = &[
    ("CLASS_KEYWORD", "class", 0, 5, 1),
    ("WHITE_SPACE", " ", 5, 6, 1),
    ("IDENTIFIER", "Box", 6, 9, 1),
    ("LPAR", "(", 9, 10, 3),
    ("RPAR", ")", 10, 11, 3),
    ("WHITE_SPACE", " ", 11, 12, 1),
    ("LBRACE", "{", 12, 13, 4),
    ("WHITE_SPACE", " ", 13, 14, 4),
    ("CONSTRUCTOR_KEYWORD", "constructor", 14, 25, 5),
    ("LPAR", "(", 25, 26, 6),
    ("IDENTIFIER", "value", 26, 31, 7),
    ("COLON", ":", 31, 32, 7),
    ("WHITE_SPACE", " ", 32, 33, 7),
    ("IDENTIFIER", "String", 33, 39, 10),
    ("RPAR", ")", 39, 40, 6),
    ("WHITE_SPACE", " ", 40, 41, 5),
    ("COLON", ":", 41, 42, 5),
    ("WHITE_SPACE", " ", 42, 43, 5),
    ("THIS_KEYWORD", "this", 43, 47, 12),
    ("LPAR", "(", 47, 48, 13),
    ("RPAR", ")", 48, 49, 13),
    ("WHITE_SPACE", " ", 49, 50, 5),
    ("LBRACE", "{", 50, 51, 14),
    ("RBRACE", "}", 51, 52, 14),
    ("WHITE_SPACE", " ", 52, 53, 4),
    ("RBRACE", "}", 53, 54, 4),
];

const NAMES: &[Name<'_>] = &[
    ("Box", 6, 9, 1),
    ("value", 26, 31, 7),
    ("String", 33, 39, 10),
];

const HEALTHY_NODES: &[Node<'_>] = &[
    ("KT_FILE", 0, 17, None, None),
    ("FUN", 0, 17, Some(0), None),
    ("VALUE_PARAMETER_LIST", 11, 13, Some(1), Some(1)),
    ("INTEGER_CONSTANT", 16, 17, Some(1), Some(1)),
];

const HEALTHY_TOKENS: &[Token<'_>] = &[
    ("FUN_KEYWORD", "fun", 0, 3, 1),
    ("WHITE_SPACE", " ", 3, 4, 1),
    ("IDENTIFIER", "healthy", 4, 11, 1),
    ("LPAR", "(", 11, 12, 2),
    ("RPAR", ")", 12, 13, 2),
    ("WHITE_SPACE", " ", 13, 14, 1),
    ("EQ", "=", 14, 15, 1),
    ("WHITE_SPACE", " ", 15, 16, 1),
    ("INTEGER_LITERAL", "2", 16, 17, 3),
];

const HEALTHY_NAMES: &[Name<'_>] = &[("healthy", 4, 11, 1)];

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "CLASS"
            | "OBJECT_DECLARATION"
            | "FUN"
            | "PROPERTY"
            | "TYPEALIAS"
            | "PRIMARY_CONSTRUCTOR"
            | "SECONDARY_CONSTRUCTOR"
    )
}

fn check_tree(
    tree: &Parsed,
    source: &str,
    nodes: &[Node<'_>],
    tokens: &[Token<'_>],
    names: &[Name<'_>],
) {
    assert_eq!(tree.source(), source);
    assert!(tree.diagnostics().is_empty());
    let elements = tree.elements();
    assert!(!elements.iter().any(|e| e.kind == "ERROR_ELEMENT"));
    let ids: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.start < e.end)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), nodes.len());
    assert_eq!(ids[0], 0);
    for (&id, &(kind, start, end, parent, owner)) in ids.iter().zip(nodes) {
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
    let actual_names: Vec<_> = elements
        .iter()
        .filter(|e| e.token && e.kind == "IDENTIFIER")
        .map(|e| (&source[e.start..e.end], e.start, e.end, e.parent.unwrap()))
        .collect();
    let expected_names: Vec<_> = names
        .iter()
        .map(|&(name, start, end, parent)| (name, start, end, ids[parent]))
        .collect();
    assert_eq!(actual_names, expected_names);

    let mut cursor = 0;
    let mut spelling = String::new();
    let actual_tokens: Vec<_> = elements
        .iter()
        .filter(|e| e.token)
        .map(|e| {
            assert_eq!(e.start, cursor);
            assert!(e.start < e.end);
            assert!(source.is_char_boundary(e.start) && source.is_char_boundary(e.end));
            spelling.push_str(&source[e.start..e.end]);
            cursor = e.end;
            (
                e.kind.as_str(),
                &source[e.start..e.end],
                e.start,
                e.end,
                e.parent.unwrap(),
            )
        })
        .collect();
    let expected_tokens: Vec<_> = tokens
        .iter()
        .map(|&(kind, spelling, start, end, parent)| (kind, spelling, start, end, ids[parent]))
        .collect();
    assert_eq!(actual_tokens, expected_tokens);
    assert_eq!(cursor, source.len());
    assert_eq!(spelling, source);
    assert!(source.is_ascii());
    for offset in 0..=source.len() {
        assert_eq!(
            tree.position(offset).unwrap(),
            Position {
                line: 0,
                character: offset as u32,
            }
        );
    }
    assert_eq!(
        tree.position(source.len() + 1).unwrap_err(),
        TextError::InvalidOffset(source.len() + 1)
    );
}

fn check_snapshot(snapshot: &SourceSnapshot, revision: i64, source: &str) {
    assert_eq!(snapshot.document(), DOCUMENT);
    assert_eq!(snapshot.revision(), revision);
    assert_eq!(snapshot.kind(), FileKind::Source);
    assert_eq!(snapshot.source(), source);
}

#[test]
fn constructors_preserve_delegation_parents_positions_and_snapshot() {
    let _functional = functional::serial();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new(DOCUMENT, 1, FileKind::Source, SOURCE).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    check_snapshot(result.snapshot(), 1, SOURCE);
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        SOURCE,
        NODES,
        TOKENS,
        NAMES,
    );

    for equal in [
        snapshot.clone(),
        SourceSnapshot::new(DOCUMENT.to_owned(), 1, FileKind::Source, SOURCE.to_owned()).unwrap(),
    ] {
        assert_eq!(equal, snapshot);
        check_tree(
            result.tree_for(&equal).unwrap(),
            SOURCE,
            NODES,
            TOKENS,
            NAMES,
        );
    }
    for stale in [
        SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, SOURCE).unwrap(),
        SourceSnapshot::new(
            "constructor-delegation-acceptance-other",
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
            SOURCE.replacen("Box", "Bag", 1),
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
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        SOURCE,
        NODES,
        TOKENS,
        NAMES,
    );

    let healthy = SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, HEALTHY).unwrap();
    assert_eq!(
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    let followup = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(followup.snapshot(), &healthy);
    check_snapshot(followup.snapshot(), 2, HEALTHY);
    check_tree(
        followup.tree_for(&healthy).unwrap(),
        HEALTHY,
        HEALTHY_NODES,
        HEALTHY_TOKENS,
        HEALTHY_NAMES,
    );
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
    check_snapshot(&healthy, 2, HEALTHY);
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        SOURCE,
        NODES,
        TOKENS,
        NAMES,
    );
    check_tree(
        followup.tree_for(&healthy).unwrap(),
        HEALTHY,
        HEALTHY_NODES,
        HEALTHY_TOKENS,
        HEALTHY_NAMES,
    );
}
