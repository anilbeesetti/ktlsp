//! C14 expectations are derived from source grammar before controlled Linux execution.
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

fn projection(parsed: &Parsed) -> Vec<Extent<'_>> {
    let elements = parsed.elements();
    elements
        .iter()
        .filter(|e| {
            !e.token
                && (declaration(&e.kind)
                    || matches!(
                        e.kind.as_str(),
                        "VALUE_PARAMETER_LIST"
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
                    ))
        })
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
                && e.parent.is_some_and(|p| declaration(&elements[p].kind))
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
    let snapshot = SourceSnapshot::new("c14", 1, FileKind::Source, source).unwrap();
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
    if source.starts_with("fun broken(x: Int =") && source.ends_with("fun healthy() = 2") {
        let start = source.len() - 17;
        for (offset, line, character) in [
            (19, 0, 19),
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
        if source.as_bytes()[19] == b'\r' && source.as_bytes()[20] == b'\n' {
            assert!(tree.position(20).is_err());
        }
    }
    let healthy = SourceSnapshot::new("c14", 2, FileKind::Source, "fun healthy() = 2").unwrap();
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
fn missing_default_restores_complete_named_sibling_in_original_newlines() {
    let _functional = functional::serial();
    for newline in ["\n", "\r\n", "\r"] {
        let start = 19 + newline.len();
        let end = start + 17;
        let source = format!("fun broken(x: Int ={newline}fun healthy() = 2");
        check(
            &source,
            &[
                ("FUN", 0, 19, "KT_FILE", None),
                ("VALUE_PARAMETER_LIST", 10, 19, "FUN", Some((0, 19))),
                (
                    "VALUE_PARAMETER",
                    11,
                    19,
                    "VALUE_PARAMETER_LIST",
                    Some((0, 19)),
                ),
                ("TYPE_REFERENCE", 14, 17, "VALUE_PARAMETER", Some((0, 19))),
                ("USER_TYPE", 14, 17, "TYPE_REFERENCE", Some((0, 19))),
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
            &[(19, 19, "Expecting an expression")],
        );
    }
}

#[test]
fn anonymous_function_and_object_defaults_keep_all_nested_owners() {
    let _functional = functional::serial();
    let source = "fun f(x: () -> Int = fun(): Int { val local = 1; return local }, y: Any = object { val member = 2 }) {}";
    let outer = Some((0, 103));
    check(
        source,
        &[
            ("FUN", 0, 103, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 100, "FUN", outer),
            ("VALUE_PARAMETER", 6, 63, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 18, "VALUE_PARAMETER", outer),
            ("FUNCTION_TYPE", 9, 18, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER_LIST", 9, 11, "FUNCTION_TYPE", outer),
            ("TYPE_REFERENCE", 15, 18, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 15, 18, "TYPE_REFERENCE", outer),
            ("FUN", 21, 63, "VALUE_PARAMETER", outer),
            ("VALUE_PARAMETER_LIST", 24, 26, "FUN", Some((21, 63))),
            ("TYPE_REFERENCE", 28, 31, "FUN", Some((21, 63))),
            ("USER_TYPE", 28, 31, "TYPE_REFERENCE", Some((21, 63))),
            ("PROPERTY", 34, 47, "BLOCK", Some((21, 63))),
            ("VALUE_PARAMETER", 65, 99, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 68, 71, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 68, 71, "TYPE_REFERENCE", outer),
            ("OBJECT_DECLARATION", 74, 99, "OBJECT_LITERAL", outer),
            ("PROPERTY", 83, 97, "CLASS_BODY", Some((74, 99))),
        ],
        &[
            (0, 4, 5, "f"),
            (34, 38, 43, "local"),
            (83, 87, 93, "member"),
        ],
        &[],
    );
}

#[test]
fn lambda_local_function_stays_inside_default_expression() {
    let _functional = functional::serial();
    let source = "fun f(x: Int = run {\nfun local() = 1\nlocal()\n}) {}";
    let outer = Some((0, 50));
    check(
        source,
        &[
            ("FUN", 0, 50, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 47, "FUN", outer),
            ("VALUE_PARAMETER", 6, 46, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 12, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 9, 12, "TYPE_REFERENCE", outer),
            ("FUN", 21, 36, "BLOCK", outer),
            ("VALUE_PARAMETER_LIST", 30, 32, "FUN", Some((21, 36))),
        ],
        &[(0, 4, 5, "f"), (21, 25, 30, "local")],
        &[],
    );
}

#[test]
fn nested_calls_keep_their_parameter_extent() {
    let _functional = functional::serial();
    let outer = Some((0, 40));
    check(
        "fun f(x: Int = nested(call(1, 2), 3)) {}",
        &[
            ("FUN", 0, 40, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 37, "FUN", outer),
            ("VALUE_PARAMETER", 6, 36, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 12, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 9, 12, "TYPE_REFERENCE", outer),
        ],
        &[(0, 4, 5, "f")],
        &[],
    );
}

#[test]
fn same_line_anonymous_function_remains_a_default() {
    let _functional = functional::serial();
    let outer = Some((0, 34));
    check(
        "fun f(x: () -> Int = fun() = 1) {}",
        &[
            ("FUN", 0, 34, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 31, "FUN", outer),
            ("VALUE_PARAMETER", 6, 30, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 18, "VALUE_PARAMETER", outer),
            ("FUNCTION_TYPE", 9, 18, "TYPE_REFERENCE", outer),
            ("VALUE_PARAMETER_LIST", 9, 11, "FUNCTION_TYPE", outer),
            ("TYPE_REFERENCE", 15, 18, "FUNCTION_TYPE", outer),
            ("USER_TYPE", 15, 18, "TYPE_REFERENCE", outer),
            ("FUN", 21, 30, "VALUE_PARAMETER", outer),
            ("VALUE_PARAMETER_LIST", 24, 26, "FUN", Some((21, 30))),
        ],
        &[(0, 4, 5, "f")],
        &[],
    );
}

#[test]
fn nonempty_named_header_retains_existing_expression_ownership() {
    let _functional = functional::serial();
    // shortcut: broader named headers need a separate recovery contract.
    let outer = Some((0, 40));
    check(
        "fun f(x: Int = fun named(y: Int) = 1) {}",
        &[
            ("FUN", 0, 40, "KT_FILE", None),
            ("VALUE_PARAMETER_LIST", 5, 37, "FUN", outer),
            ("VALUE_PARAMETER", 6, 36, "VALUE_PARAMETER_LIST", outer),
            ("TYPE_REFERENCE", 9, 12, "VALUE_PARAMETER", outer),
            ("USER_TYPE", 9, 12, "TYPE_REFERENCE", outer),
            ("FUN", 15, 36, "VALUE_PARAMETER", outer),
            ("VALUE_PARAMETER_LIST", 24, 32, "FUN", Some((15, 36))),
            (
                "VALUE_PARAMETER",
                25,
                31,
                "VALUE_PARAMETER_LIST",
                Some((15, 36)),
            ),
            ("TYPE_REFERENCE", 28, 31, "VALUE_PARAMETER", Some((15, 36))),
            ("USER_TYPE", 28, 31, "TYPE_REFERENCE", Some((15, 36))),
        ],
        &[(0, 4, 5, "f"), (15, 19, 24, "named")],
        &[],
    );
}

#[test]
fn scratch_raw_token_and_byte_limits_decline_without_losing_default_tokens() {
    let _functional = functional::serial();
    for (padding, recovered) in [
        (format!("/*{}*/", "x".repeat(4079)), true),
        (format!("/*{}*/", "x".repeat(4080)), false),
        ("/*x*/ ".repeat(125), true),
        ("/*x*/ ".repeat(126), false),
    ] {
        let source = format!("fun broken(x: Int =\nfun {padding}healthy() = 2");
        let end = source.len();
        let broken_end = if recovered { 19 } else { end };
        let outer = Some((0, broken_end));
        let name = 24 + padding.len();
        check(
            &source,
            &[
                ("FUN", 0, broken_end, "KT_FILE", None),
                ("VALUE_PARAMETER_LIST", 10, broken_end, "FUN", outer),
                (
                    "VALUE_PARAMETER",
                    11,
                    broken_end,
                    "VALUE_PARAMETER_LIST",
                    outer,
                ),
                ("TYPE_REFERENCE", 14, 17, "VALUE_PARAMETER", outer),
                ("USER_TYPE", 14, 17, "TYPE_REFERENCE", outer),
                (
                    "FUN",
                    20,
                    end,
                    if recovered {
                        "KT_FILE"
                    } else {
                        "VALUE_PARAMETER"
                    },
                    if recovered { None } else { outer },
                ),
                (
                    "VALUE_PARAMETER_LIST",
                    name + 7,
                    name + 9,
                    "FUN",
                    Some((20, end)),
                ),
            ],
            &[(0, 4, 10, "broken"), (20, name, name + 7, "healthy")],
            &if recovered {
                vec![(19, 19, "Expecting an expression")]
            } else {
                vec![
                    (end, end, "Expecting comma or ')'"),
                    (end, end, "Expecting ')'"),
                ]
            },
        );
    }
}
