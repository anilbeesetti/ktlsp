//! One source-derived enum-entry acceptance fixture; runtime evidence is separate.
use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use ktlsp_text::Position;
use std::sync::atomic::AtomicBool;

const SOURCE: &str = "enum class Mode {\n    READY,\n    DONE\n}";
const DOCUMENT: &str = "enum-entry-acceptance";

type Extent<'a> = (&'a str, usize, usize, &'a str, Option<(usize, usize)>);
type Name<'a> = (&'a str, usize, usize, &'a str, usize, usize);
type Token<'a> = (&'a str, usize, usize, &'a str, usize, usize);

const ENUM_EXTENTS: &[Extent<'_>] = &[
    ("CLASS", 0, 39, "KT_FILE", None),
    ("MODIFIER_LIST", 0, 4, "CLASS", Some((0, 39))),
    ("CLASS_BODY", 16, 39, "CLASS", Some((0, 39))),
    ("ENUM_ENTRY", 22, 28, "CLASS_BODY", Some((0, 39))),
    ("ENUM_ENTRY", 33, 37, "CLASS_BODY", Some((0, 39))),
];
const ENUM_NAMES: &[Name<'_>] = &[
    ("CLASS", 0, 39, "Mode", 11, 15),
    ("ENUM_ENTRY", 22, 28, "READY", 22, 27),
    ("ENUM_ENTRY", 33, 37, "DONE", 33, 37),
];
const ENUM_TOKENS: &[Token<'_>] = &[
    ("ENUM_KEYWORD", 0, 4, "MODIFIER_LIST", 0, 4),
    ("WHITE_SPACE", 4, 5, "CLASS", 0, 39),
    ("CLASS_KEYWORD", 5, 10, "CLASS", 0, 39),
    ("WHITE_SPACE", 10, 11, "CLASS", 0, 39),
    ("IDENTIFIER", 11, 15, "CLASS", 0, 39),
    ("WHITE_SPACE", 15, 16, "CLASS", 0, 39),
    ("LBRACE", 16, 17, "CLASS_BODY", 16, 39),
    ("WHITE_SPACE", 17, 22, "CLASS_BODY", 16, 39),
    ("IDENTIFIER", 22, 27, "ENUM_ENTRY", 22, 28),
    ("COMMA", 27, 28, "ENUM_ENTRY", 22, 28),
    ("WHITE_SPACE", 28, 33, "CLASS_BODY", 16, 39),
    ("IDENTIFIER", 33, 37, "ENUM_ENTRY", 33, 37),
    ("WHITE_SPACE", 37, 38, "CLASS_BODY", 16, 39),
    ("RBRACE", 38, 39, "CLASS_BODY", 16, 39),
];

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS" | "ENUM_ENTRY"
    )
}

fn selected(kind: &str) -> bool {
    declaration(kind)
        || matches!(
            kind,
            "MODIFIER_LIST"
                | "CLASS_BODY"
                | "PRIMARY_CONSTRUCTOR"
                | "SECONDARY_CONSTRUCTOR"
                | "VALUE_PARAMETER_LIST"
                | "VALUE_PARAMETER"
                | "TYPE_REFERENCE"
                | "USER_TYPE"
                | "FUNCTION_TYPE"
                | "NULLABLE_TYPE"
                | "TYPE_PARAMETER_LIST"
                | "TYPE_PARAMETER"
                | "TYPE_ARGUMENT_LIST"
                | "TYPE_PROJECTION"
        )
}

fn projection(tree: &Parsed) -> Vec<Extent<'_>> {
    let elements = tree.elements();
    elements
        .iter()
        .filter(|e| !e.token && selected(&e.kind))
        .map(|e| {
            let mut ancestor = e.parent;
            let owner = loop {
                match ancestor {
                    Some(p) if declaration(&elements[p].kind) => {
                        break Some((elements[p].start, elements[p].end));
                    }
                    Some(p) => ancestor = elements[p].parent,
                    None => break None,
                }
            };
            (
                e.kind.as_str(),
                e.start,
                e.end,
                elements[e.parent.unwrap()].kind.as_str(),
                owner,
            )
        })
        .collect()
}

fn names(tree: &Parsed) -> Vec<Name<'_>> {
    let elements = tree.elements();
    elements
        .iter()
        .filter(|e| e.token && e.kind == "IDENTIFIER")
        .map(|e| {
            let parent = &elements[e.parent.unwrap()];
            (
                parent.kind.as_str(),
                parent.start,
                parent.end,
                &tree.source()[e.start..e.end],
                e.start,
                e.end,
            )
        })
        .collect()
}

fn node(tree: &Parsed, kind: &str, start: usize, end: usize) -> usize {
    let ids: Vec<_> = tree
        .elements()
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.kind == kind && (e.start, e.end) == (start, end))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), 1);
    ids[0]
}

fn check_source(tree: &Parsed, source: &str) {
    assert_eq!(tree.source(), source);
    let root = &tree.elements()[0];
    assert_eq!(
        (root.kind.as_str(), root.start, root.end, root.parent),
        ("KT_FILE", 0, source.len(), None)
    );
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

fn check_enum(tree: &Parsed) {
    check_source(tree, SOURCE);
    assert_eq!(projection(tree), ENUM_EXTENTS);
    assert_eq!(names(tree), ENUM_NAMES);
    let elements = tree.elements();
    let class = node(tree, "CLASS", 0, 39);
    let body = node(tree, "CLASS_BODY", 16, 39);
    assert_eq!(elements[class].parent, Some(0));
    assert_eq!(elements[body].parent, Some(class));
    assert_eq!(
        elements[node(tree, "MODIFIER_LIST", 0, 4)].parent,
        Some(class)
    );
    for (start, end) in [(22, 28), (33, 37)] {
        assert_eq!(
            elements[node(tree, "ENUM_ENTRY", start, end)].parent,
            Some(body)
        );
    }
    let tokens: Vec<Token<'_>> = elements
        .iter()
        .filter(|e| e.token)
        .map(|e| {
            let parent = &elements[e.parent.unwrap()];
            (
                e.kind.as_str(),
                e.start,
                e.end,
                parent.kind.as_str(),
                parent.start,
                parent.end,
            )
        })
        .collect();
    assert_eq!(tokens, ENUM_TOKENS);
    for (start, end, line) in [(0, 17, 0), (18, 28, 1), (29, 37, 2), (38, 39, 3)] {
        for offset in start..=end {
            assert_eq!(
                tree.position(offset).unwrap(),
                Position {
                    line,
                    character: (offset - start) as u32,
                }
            );
        }
    }
}

#[test]
fn bare_enum_entries_preserve_kinds_names_parents_and_snapshot() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new(DOCUMENT, 1, FileKind::Source, SOURCE).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    check_enum(result.tree_for(&snapshot).unwrap());

    for stale in [
        SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, SOURCE).unwrap(),
        SourceSnapshot::new("enum-entry-other", 1, FileKind::Source, SOURCE).unwrap(),
        SourceSnapshot::new(DOCUMENT, 1, FileKind::Script, SOURCE).unwrap(),
        SourceSnapshot::new(DOCUMENT, 1, FileKind::Source, format!("{SOURCE}\n")).unwrap(),
    ] {
        assert_eq!(
            result.tree_for(&stale).unwrap_err(),
            ParserError::StaleSnapshot
        );
    }
    assert_eq!(result.snapshot(), &snapshot);
    assert_eq!(snapshot.source(), SOURCE);
    check_enum(result.tree_for(&snapshot).unwrap());

    let healthy = SourceSnapshot::new(DOCUMENT, 2, FileKind::Source, "fun healthy() = 2").unwrap();
    assert_eq!(
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    let followup = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(followup.snapshot(), &healthy);
    let tree = followup.tree_for(&healthy).unwrap();
    check_source(tree, healthy.source());
    assert_eq!(
        projection(tree),
        [
            ("FUN", 0, 17, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 11, 13, "FUN", Some((0, 17))),
        ]
    );
    assert_eq!(names(tree), [("FUN", 0, 17, "healthy", 4, 11)]);
    let function = node(tree, "FUN", 0, 17);
    assert_eq!(tree.elements()[function].parent, Some(0));
    assert_eq!(
        tree.elements()[node(tree, "VALUE_PARAMETER_LIST", 11, 13)].parent,
        Some(function)
    );
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
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    assert_eq!(result.snapshot(), &snapshot);
    assert_eq!(snapshot.source(), SOURCE);
    check_enum(result.tree_for(&snapshot).unwrap());
}
