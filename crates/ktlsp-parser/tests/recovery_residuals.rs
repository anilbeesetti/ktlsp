//! C17 known-defect fixtures; passing these assertions does not establish recovery.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parsed, Parser, SourceSnapshot};
use std::sync::atomic::AtomicBool;

// Kind, original extent, immediate parent, nearest declaration extent, direct names.
type Declaration<'a> = (
    &'a str,
    usize,
    usize,
    &'a str,
    Option<(usize, usize)>,
    Vec<(&'a str, usize, usize)>,
);

fn declarations(parsed: &Parsed) -> Vec<Declaration<'_>> {
    let elements = parsed.elements();
    let is_declaration = |kind: &str| {
        matches!(
            kind,
            "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS"
        )
    };
    elements
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && is_declaration(&e.kind))
        .map(|(id, e)| {
            let mut ancestor = e.parent;
            let owner = loop {
                match ancestor {
                    Some(p) if is_declaration(&elements[p].kind) => {
                        break Some((elements[p].start, elements[p].end));
                    }
                    Some(p) => ancestor = elements[p].parent,
                    None => break None,
                }
            };
            let names = elements
                .iter()
                .filter(|n| n.token && n.kind == "IDENTIFIER" && n.parent == Some(id))
                .map(|n| (&parsed.source()[n.start..n.end], n.start, n.end))
                .collect();
            (
                e.kind.as_str(),
                e.start,
                e.end,
                elements[e.parent.unwrap()].kind.as_str(),
                owner,
                names,
            )
        })
        .collect()
}

fn check(source: &str, expected: Vec<Declaration<'_>>, diagnostics: &[(usize, usize, &str)]) {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new("c17", 17, FileKind::Source, source).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    let parsed = result.tree_for(&snapshot).unwrap();
    assert_eq!(parsed.source(), source);
    assert_eq!(declarations(parsed), expected, "{source:?}");
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .map(|d| (d.start, d.end, d.message.as_str()))
            .collect::<Vec<_>>(),
        diagnostics,
        "{source:?}"
    );
    let mut cursor = 0;
    for token in parsed.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
    assert_eq!(snapshot.source(), source);

    let healthy = SourceSnapshot::new("c17", 18, FileKind::Source, "fun healthy() = 2").unwrap();
    let followup = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(followup.snapshot(), &healthy);
    let tree = followup.tree_for(&healthy).unwrap();
    assert_eq!(tree.source(), healthy.source());
    assert_eq!(
        declarations(tree),
        vec![("FUN", 0, 17, "KT_FILE", None, vec![("healthy", 4, 11)])]
    );
    assert!(tree.diagnostics().is_empty());
    assert_eq!(result.tree_for(&snapshot).unwrap().source(), source);
}

#[test]
fn missing_default_restores_later_function_as_sibling() {
    let _functional = functional::serial();
    let source = "fun broken(x: Int =\nfun healthy() = 2";
    check(
        source,
        vec![
            ("FUN", 0, 19, "KT_FILE", None, vec![("broken", 4, 10)]),
            ("FUN", 20, 37, "KT_FILE", None, vec![("healthy", 24, 31)]),
        ],
        &[(19, 19, "Expecting an expression")],
    );
}

#[test]
fn malformed_parenthesized_type_restores_later_function_as_sibling() {
    let _functional = functional::serial();
    let source = "fun broken(x: (@A\nfun healthy() = 2";
    check(
        source,
        vec![
            ("FUN", 0, 17, "KT_FILE", None, vec![("broken", 4, 10)]),
            ("FUN", 18, 35, "KT_FILE", None, vec![("healthy", 22, 29)]),
        ],
        &[(17, 17, "Type expected")],
    );
}

#[test]
fn malformed_parameter_annotation_restores_later_function_as_sibling() {
    let _functional = functional::serial();
    let source = "fun broken(@A(\nfun healthy() = 2";
    check(
        source,
        vec![
            ("FUN", 0, 14, "KT_FILE", None, vec![("broken", 4, 10)]),
            ("FUN", 15, 32, "KT_FILE", None, vec![("healthy", 19, 26)]),
        ],
        &[
            (14, 14, "Expecting ')'"),
            (14, 14, "Parameter name expected"),
        ],
    );
}
