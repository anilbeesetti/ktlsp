//! C16 expectations are derived from source grammar before controlled Linux execution.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use std::sync::atomic::AtomicBool;

type Extent<'a> = (&'a str, usize, usize, &'a str, Option<(usize, usize)>);

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS"
    )
}

fn selected(kind: &str) -> bool {
    declaration(kind)
        || matches!(
            kind,
            "BLOCK"
                | "CALL_EXPRESSION"
                | "VALUE_ARGUMENT_LIST"
                | "VALUE_ARGUMENT"
                | "PRIMARY_CONSTRUCTOR"
                | "FUNCTION_TYPE_RECEIVER"
                | "MODIFIER_LIST"
                | "CONSTRUCTOR_CALLEE"
                | "VALUE_PARAMETER_LIST"
                | "VALUE_PARAMETER"
                | "TYPE_REFERENCE"
                | "USER_TYPE"
                | "FUNCTION_TYPE"
                | "NULLABLE_TYPE"
                | "INTERSECTION_TYPE"
                | "DYNAMIC_TYPE"
                | "TYPE_ARGUMENT_LIST"
                | "TYPE_PROJECTION"
                | "TYPE_PARAMETER_LIST"
                | "TYPE_PARAMETER"
                | "ANNOTATION"
                | "ANNOTATION_ENTRY"
        )
}

fn projection(parsed: &Parsed) -> Vec<Extent<'_>> {
    let elements = parsed.elements();
    elements
        .iter()
        .filter(|e| !e.token && selected(&e.kind))
        .map(|e| {
            let mut parent = e.parent;
            let owner = loop {
                match parent {
                    Some(id) if declaration(&elements[id].kind) => {
                        break Some((elements[id].start, elements[id].end));
                    }
                    Some(id) => parent = elements[id].parent,
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

fn names(parsed: &Parsed) -> Vec<(usize, usize, usize, &str)> {
    let elements = parsed.elements();
    elements
        .iter()
        .filter(|e| {
            e.token
                && e.kind == "IDENTIFIER"
                && e.parent.is_some_and(|p| selected(&elements[p].kind))
        })
        .map(|e| {
            (
                elements[e.parent.unwrap()].start,
                e.start,
                e.end,
                &parsed.source()[e.start..e.end],
            )
        })
        .collect()
}

fn check(
    source: &str,
    expected: &[Extent<'_>],
    expected_names: &[(usize, usize, usize, &str)],
    diagnostics: &[(usize, usize, &str)],
) {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new("c16", 1, FileKind::Source, source).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    let tree = result.tree_for(&snapshot).unwrap();
    assert_eq!(tree.source(), source);
    assert_eq!(projection(tree), expected, "{source:?}");
    assert_eq!(names(tree), expected_names, "{source:?}");
    assert_eq!(
        tree.diagnostics()
            .iter()
            .map(|d| (d.start, d.end, d.message.as_str()))
            .collect::<Vec<_>>(),
        diagnostics,
        "{source:?}"
    );
    let mut cursor = 0;
    for token in tree.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
    if source.starts_with("fun broken(@A(") && source.ends_with("fun healthy() = 2") {
        let start = source.len() - 17;
        for (offset, line, character) in [
            (14, 0, 14),
            (start, 1, 0),
            (start + 4, 1, 4),
            (start + 11, 1, 11),
            (source.len(), 1, 17),
        ] {
            assert_eq!(
                tree.position(offset).unwrap(),
                ktlsp_text::Position { line, character }
            );
        }
        if source.as_bytes()[14] == b'\r' && source.as_bytes()[15] == b'\n' {
            assert!(tree.position(15).is_err());
        }
    }
    let healthy = SourceSnapshot::new("c16", 2, FileKind::Source, "fun healthy() = 2").unwrap();
    assert_eq!(
        result.tree_for(&healthy).unwrap_err(),
        ParserError::StaleSnapshot
    );
    let followup = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(followup.snapshot(), &healthy);
    let followup_tree = followup.tree_for(&healthy).unwrap();
    assert_eq!(followup_tree.source(), healthy.source());
    assert_eq!(
        projection(followup_tree),
        [
            ("FUN", 0, 17, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 11, 13, "FUN", Some((0, 17)))
        ]
    );
    assert_eq!(names(followup_tree), [(0, 4, 11, "healthy")]);
    assert!(followup_tree.diagnostics().is_empty());
    assert_eq!(snapshot.source(), source);
    assert_eq!(result.tree_for(&snapshot).unwrap().source(), source);
}

#[test]
fn unclosed_parameter_annotation_restores_complete_sibling_in_original_newlines() {
    let _functional = functional::serial();
    for newline in ["\n", "\r\n", "\r"] {
        let start = 14 + newline.len();
        let end = start + 17;
        let source = format!("fun broken(@A({newline}fun healthy() = 2");
        let broken = Some((0, 14));
        check(
            &source,
            &[
                ("FUN", 0, 14, "KT_FILE", None),
                ("VALUE_PARAMETER_LIST", 10, 14, "FUN", broken),
                ("VALUE_PARAMETER", 11, 14, "VALUE_PARAMETER_LIST", broken),
                ("MODIFIER_LIST", 11, 14, "VALUE_PARAMETER", broken),
                ("ANNOTATION_ENTRY", 11, 14, "MODIFIER_LIST", broken),
                ("CONSTRUCTOR_CALLEE", 12, 13, "ANNOTATION_ENTRY", broken),
                ("TYPE_REFERENCE", 12, 13, "CONSTRUCTOR_CALLEE", broken),
                ("USER_TYPE", 12, 13, "TYPE_REFERENCE", broken),
                ("VALUE_ARGUMENT_LIST", 13, 14, "ANNOTATION_ENTRY", broken),
                ("FUN", start, end, "KT_FILE", None),
                (
                    "VALUE_PARAMETER_LIST",
                    start + 11,
                    start + 13,
                    "FUN",
                    Some((start, end)),
                ),
            ],
            &[
                (0, 4, 10, "broken"),
                (start, start + 4, start + 11, "healthy"),
            ],
            &[
                (14, 14, "Expecting ')'"),
                (14, 14, "Parameter name expected"),
            ],
        );
    }
}

#[test]
fn complete_annotation_argument_preserves_anonymous_function_parameter() {
    let _functional = functional::serial();
    let outer = Some((0, 35));
    let anonymous = Some((9, 23));
    check(
        "fun f(@A(fun(x: Int) {}) x: Int) {}",
        &[
            ("FUN", 0, 35, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 32, "FUN", outer),
            ("VALUE_PARAMETER", 6, 31, "VALUE_PARAMETER_LIST", outer),
            ("MODIFIER_LIST", 6, 24, "VALUE_PARAMETER", outer),
            ("ANNOTATION_ENTRY", 6, 24, "MODIFIER_LIST", outer),
            ("CONSTRUCTOR_CALLEE", 7, 8, "ANNOTATION_ENTRY", outer),
            ("TYPE_REFERENCE", 7, 8, "CONSTRUCTOR_CALLEE", outer),
            ("USER_TYPE", 7, 8, "TYPE_REFERENCE", outer),
            ("VALUE_ARGUMENT_LIST", 8, 24, "ANNOTATION_ENTRY", outer),
            ("VALUE_ARGUMENT", 9, 23, "VALUE_ARGUMENT_LIST", outer),
            ("FUN", 9, 23, "VALUE_ARGUMENT", outer),
            ("VALUE_PARAMETER_LIST", 12, 20, "FUN", anonymous),
            ("VALUE_PARAMETER", 13, 19, "VALUE_PARAMETER_LIST", anonymous),
            ("TYPE_REFERENCE", 16, 19, "VALUE_PARAMETER", anonymous),
            ("USER_TYPE", 16, 19, "TYPE_REFERENCE", anonymous),
            ("BLOCK", 21, 23, "FUN", anonymous),
            ("TYPE_REFERENCE", 28, 31, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 28, 31, "TYPE_REFERENCE", outer),
            ("BLOCK", 33, 35, "FUN", outer),
        ],
        &[(0, 4, 5, "f"), (13, 13, 14, "x"), (6, 25, 26, "x")],
        &[],
    );
}

#[test]
fn ordinary_call_argument_preserves_anonymous_function_local_property() {
    let _functional = functional::serial();
    let outer = Some((0, 42));
    let anonymous = Some((18, 41));
    check(
        "fun f() = consume(fun() { val local = 1 })",
        &[
            ("FUN", 0, 42, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 7, "FUN", outer),
            ("CALL_EXPRESSION", 10, 42, "FUN", outer),
            ("VALUE_ARGUMENT_LIST", 17, 42, "CALL_EXPRESSION", outer),
            ("VALUE_ARGUMENT", 18, 41, "VALUE_ARGUMENT_LIST", outer),
            ("FUN", 18, 41, "VALUE_ARGUMENT", outer),
            ("VALUE_PARAMETER_LIST", 21, 23, "FUN", anonymous),
            ("BLOCK", 24, 41, "FUN", anonymous),
            ("PROPERTY", 26, 39, "BLOCK", anonymous),
        ],
        &[(0, 4, 5, "f"), (26, 30, 35, "local")],
        &[],
    );
}

#[test]
fn complete_annotation_argument_preserves_anonymous_function_local_function() {
    let _functional = functional::serial();
    let outer = Some((0, 46));
    let anonymous = Some((9, 34));
    let local = Some((17, 32));
    check(
        "fun f(@A(fun() { fun local() = 1 }) x: Int) {}",
        &[
            ("FUN", 0, 46, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 43, "FUN", outer),
            ("VALUE_PARAMETER", 6, 42, "VALUE_PARAMETER_LIST", outer),
            ("MODIFIER_LIST", 6, 35, "VALUE_PARAMETER", outer),
            ("ANNOTATION_ENTRY", 6, 35, "MODIFIER_LIST", outer),
            ("CONSTRUCTOR_CALLEE", 7, 8, "ANNOTATION_ENTRY", outer),
            ("TYPE_REFERENCE", 7, 8, "CONSTRUCTOR_CALLEE", outer),
            ("USER_TYPE", 7, 8, "TYPE_REFERENCE", outer),
            ("VALUE_ARGUMENT_LIST", 8, 35, "ANNOTATION_ENTRY", outer),
            ("VALUE_ARGUMENT", 9, 34, "VALUE_ARGUMENT_LIST", outer),
            ("FUN", 9, 34, "VALUE_ARGUMENT", outer),
            ("VALUE_PARAMETER_LIST", 12, 14, "FUN", anonymous),
            ("BLOCK", 15, 34, "FUN", anonymous),
            ("FUN", 17, 32, "BLOCK", anonymous),
            ("VALUE_PARAMETER_LIST", 26, 28, "FUN", local),
            ("TYPE_REFERENCE", 39, 42, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 39, 42, "TYPE_REFERENCE", outer),
            ("BLOCK", 44, 46, "FUN", outer),
        ],
        &[(0, 4, 5, "f"), (17, 21, 26, "local"), (6, 36, 37, "x")],
        &[],
    );
}
#[test]
fn reparsed_anonymous_body_keeps_damaged_local_annotation_nested() {
    let _functional = functional::serial();
    let outer = Some((0, 51));
    let anonymous = Some((10, 51));
    let local = Some((18, 49));
    let healthy = Some((32, 49));
    check(
        "fun f() = fun() { fun local(@A(\nfun healthy() = 2 }",
        &[
            ("FUN", 0, 51, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 7, "FUN", outer),
            ("FUN", 10, 51, "FUN", outer),
            ("VALUE_PARAMETER_LIST", 13, 15, "FUN", anonymous),
            ("BLOCK", 16, 51, "FUN", anonymous),
            ("FUN", 18, 49, "BLOCK", anonymous),
            ("VALUE_PARAMETER_LIST", 27, 49, "FUN", local),
            ("VALUE_PARAMETER", 28, 49, "VALUE_PARAMETER_LIST", local),
            ("MODIFIER_LIST", 28, 49, "VALUE_PARAMETER", local),
            ("ANNOTATION_ENTRY", 28, 49, "MODIFIER_LIST", local),
            ("CONSTRUCTOR_CALLEE", 29, 30, "ANNOTATION_ENTRY", local),
            ("TYPE_REFERENCE", 29, 30, "CONSTRUCTOR_CALLEE", local),
            ("USER_TYPE", 29, 30, "TYPE_REFERENCE", local),
            ("VALUE_ARGUMENT_LIST", 30, 49, "ANNOTATION_ENTRY", local),
            ("VALUE_ARGUMENT", 32, 49, "VALUE_ARGUMENT_LIST", local),
            ("FUN", 32, 49, "VALUE_ARGUMENT", local),
            ("VALUE_PARAMETER_LIST", 43, 45, "FUN", healthy),
        ],
        &[
            (0, 4, 5, "f"),
            (18, 22, 27, "local"),
            (32, 36, 43, "healthy"),
        ],
        &[(49, 49, "Expecting ')'")],
    );
}
