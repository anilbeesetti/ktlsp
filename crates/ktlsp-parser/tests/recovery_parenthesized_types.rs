//! C15 expectations are derived from source grammar before controlled Linux execution.
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
            "PRIMARY_CONSTRUCTOR"
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
    let snapshot = SourceSnapshot::new("c15", 1, FileKind::Source, source).unwrap();
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
    if source.starts_with("fun broken(x: (@A") && source.ends_with("fun healthy() = 2") {
        let start = source.len() - 17;
        for (offset, line, character) in [
            (17, 0, 17),
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
        if source.as_bytes()[17] == b'\r' && source.as_bytes()[18] == b'\n' {
            assert!(tree.position(18).is_err());
        }
    }
    let healthy = SourceSnapshot::new("c15", 2, FileKind::Source, "fun healthy() = 2").unwrap();
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
fn malformed_parenthesized_type_restores_complete_sibling_in_original_newlines() {
    for newline in ["\n", "\r\n", "\r"] {
        let start = 17 + newline.len();
        let end = start + 17;
        let source = format!("fun broken(x: (@A{newline}fun healthy() = 2");
        let broken = Some((0, 17));
        check(
            &source,
            &[
                ("FUN", 0, 17, "KT_FILE", None),
                ("VALUE_PARAMETER_LIST", 10, 17, "FUN", broken),
                ("VALUE_PARAMETER", 11, 17, "VALUE_PARAMETER_LIST", broken),
                ("TYPE_REFERENCE", 14, 17, "VALUE_PARAMETER", broken),
                ("MODIFIER_LIST", 15, 17, "TYPE_REFERENCE", broken),
                ("ANNOTATION_ENTRY", 15, 17, "MODIFIER_LIST", broken),
                ("CONSTRUCTOR_CALLEE", 16, 17, "ANNOTATION_ENTRY", broken),
                ("TYPE_REFERENCE", 16, 17, "CONSTRUCTOR_CALLEE", broken),
                ("USER_TYPE", 16, 17, "TYPE_REFERENCE", broken),
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
                (11, 11, 12, "x"),
                (start, start + 4, start + 11, "healthy"),
            ],
            &[(17, 17, "Type expected")],
        );
    }
}

#[test]
fn annotated_function_type_preserves_its_parameters() {
    let outer = Some((0, 37));
    check(
        "fun f(x: (@A Int, String) -> Unit) {}",
        &[
            ("FUN", 0, 37, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 34, "FUN", outer),
            ("VALUE_PARAMETER", 6, 33, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 33, "VALUE_PARAMETER", outer),
            ("FUNCTION_TYPE", 9, 33, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER_LIST", 9, 25, "FUNCTION_TYPE", outer),
            ("VALUE_PARAMETER", 10, 16, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 10, 16, "VALUE_PARAMETER", outer),
            ("MODIFIER_LIST", 10, 12, "TYPE_REFERENCE", outer),
            ("ANNOTATION_ENTRY", 10, 12, "MODIFIER_LIST", outer),
            ("CONSTRUCTOR_CALLEE", 11, 12, "ANNOTATION_ENTRY", outer),
            ("TYPE_REFERENCE", 11, 12, "CONSTRUCTOR_CALLEE", outer),
            ("USER_TYPE", 11, 12, "TYPE_REFERENCE", outer),
            ("USER_TYPE", 13, 16, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER", 18, 24, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 18, 24, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 18, 24, "TYPE_REFERENCE", outer),
            ("TYPE_REFERENCE", 29, 33, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 29, 33, "TYPE_REFERENCE", outer),
        ],
        &[(0, 4, 5, "f"), (6, 6, 7, "x")],
        &[],
    );
}

#[test]
fn nested_nullable_function_type_stays_inside_parameter() {
    let outer = Some((0, 31));
    check(
        "fun f(x: ((Int) -> String)?) {}",
        &[
            ("FUN", 0, 31, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 28, "FUN", outer),
            ("VALUE_PARAMETER", 6, 27, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 27, "VALUE_PARAMETER", outer),
            ("NULLABLE_TYPE", 9, 27, "TYPE_REFERENCE", outer),
            ("FUNCTION_TYPE", 10, 25, "NULLABLE_TYPE", outer),
            ("VALUE_PARAMETER_LIST", 10, 15, "FUNCTION_TYPE", outer),
            ("VALUE_PARAMETER", 11, 14, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 11, 14, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 11, 14, "TYPE_REFERENCE", outer),
            ("TYPE_REFERENCE", 19, 25, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 19, 25, "TYPE_REFERENCE", outer),
        ],
        &[(0, 4, 5, "f"), (6, 6, 7, "x")],
        &[],
    );
}

#[test]
fn function_type_receiver_stays_inside_parameter() {
    let outer = Some((0, 33));
    check(
        "fun f(x: Int.(String) -> Unit) {}",
        &[
            ("FUN", 0, 33, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 30, "FUN", outer),
            ("VALUE_PARAMETER", 6, 29, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 29, "VALUE_PARAMETER", outer),
            ("FUNCTION_TYPE", 9, 29, "TYPE_REFERENCE", outer),
            ("FUNCTION_TYPE_RECEIVER", 9, 12, "FUNCTION_TYPE", outer),
            ("TYPE_REFERENCE", 9, 12, "FUNCTION_TYPE_RECEIVER", outer),
            ("USER_TYPE", 9, 12, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER_LIST", 13, 21, "FUNCTION_TYPE", outer),
            ("VALUE_PARAMETER", 14, 20, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 14, 20, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 14, 20, "TYPE_REFERENCE", outer),
            ("TYPE_REFERENCE", 25, 29, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 25, 29, "TYPE_REFERENCE", outer),
        ],
        &[(0, 4, 5, "f"), (6, 6, 7, "x")],
        &[],
    );
}

#[test]
fn constructor_annotated_function_type_keeps_constructor_ownership() {
    let outer = Some((0, 47));
    check(
        "class C(val callback: (@A Int, String) -> Unit)",
        &[
            ("CLASS", 0, 47, "KT_FILE", None),
            ("PRIMARY_CONSTRUCTOR", 7, 47, "CLASS", outer),
            ("VALUE_PARAMETER_LIST", 7, 47, "PRIMARY_CONSTRUCTOR", outer),
            ("VALUE_PARAMETER", 8, 46, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 22, 46, "VALUE_PARAMETER", outer),
            ("FUNCTION_TYPE", 22, 46, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER_LIST", 22, 38, "FUNCTION_TYPE", outer),
            ("VALUE_PARAMETER", 23, 29, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 23, 29, "VALUE_PARAMETER", outer),
            ("MODIFIER_LIST", 23, 25, "TYPE_REFERENCE", outer),
            ("ANNOTATION_ENTRY", 23, 25, "MODIFIER_LIST", outer),
            ("CONSTRUCTOR_CALLEE", 24, 25, "ANNOTATION_ENTRY", outer),
            ("TYPE_REFERENCE", 24, 25, "CONSTRUCTOR_CALLEE", outer),
            ("USER_TYPE", 24, 25, "TYPE_REFERENCE", outer),
            ("USER_TYPE", 26, 29, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER", 31, 37, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 31, 37, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 31, 37, "TYPE_REFERENCE", outer),
            ("TYPE_REFERENCE", 42, 46, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 42, 46, "TYPE_REFERENCE", outer),
        ],
        &[(0, 6, 7, "C"), (8, 12, 20, "callback")],
        &[],
    );
}
