//! Prerequisite CST acceptance only; this does not implement declaration lowering.
use ktlsp_parser::{Element, FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use std::{io::Write, sync::atomic::AtomicBool};

// Frozen before execution; these capture ceilings are not parser/lowering resource guarantees.
const MAX_ELEMENTS: usize = 512;
const MAX_DIAGNOSTICS: usize = 16;
const MAX_KIND_BYTES: usize = 64;
const MAX_MESSAGE_BYTES: usize = 4096;
const MAX_CAPTURE_BYTES: usize = 1_048_576;

type Declaration<'a> = (
    &'a str,
    usize,
    usize,
    &'a str,
    usize,
    usize,
    &'a str,
    Option<usize>,
    bool,
);

fn main() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    // Inputs remain the unchanged PR174 files; frozen blobs/hashes are checked at source sealing.
    let fixtures = [
        (
            "declarations",
            include_str!("../../../docs/specs/lowering-acceptance/declarations.kt"),
            146,
        ),
        (
            "error-node",
            include_str!("../../../docs/specs/lowering-acceptance/error-node.kt"),
            44,
        ),
    ];
    let mut results = Vec::new();
    for (name, source, length) in fixtures {
        assert_eq!(source.len(), length);
        assert!(source.ends_with('\n') && !source.contains('\r'));
        let snapshot = SourceSnapshot::new(
            format!("lowering-acceptance/{name}"),
            1,
            FileKind::Source,
            source,
        )
        .unwrap();
        // The harness itself owns sequential admission: one request per fixture, without retries.
        let result = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("{name}: retained parse failure: {error:?}"));
        assert_eq!(result.snapshot(), &snapshot);
        let tree = result.tree_for(&snapshot).unwrap();
        assert_eq!(tree.source(), snapshot.source());
        capture(name, tree);
        for stale in [
            SourceSnapshot::new("changed-key", 1, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(snapshot.document(), 2, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(snapshot.document(), 1, FileKind::Script, source).unwrap(),
            SourceSnapshot::new(snapshot.document(), 1, FileKind::Source, "").unwrap(),
        ] {
            assert_eq!(
                result.tree_for(&stale).unwrap_err(),
                ParserError::StaleSnapshot
            );
        }
        results.push((snapshot, result));
    }
    let declarations = results[0].1.tree_for(&results[0].0).unwrap();
    check(
        declarations,
        &[
            ("CLASS", 0, 129, "Host", 6, 10, "KT_FILE", None, false),
            (
                "PROPERTY",
                17,
                29,
                "same",
                21,
                25,
                "CLASS_BODY",
                Some(0),
                false,
            ),
            ("FUN", 34, 127, "same", 38, 42, "CLASS_BODY", Some(0), false),
            ("CLASS", 55, 66, "Local", 61, 66, "BLOCK", Some(2), false),
            ("FUN", 75, 89, "inner", 79, 84, "BLOCK", Some(2), false),
            (
                "PROPERTY",
                98,
                121,
                "`local 🦀e\u{301}`",
                102,
                117,
                "BLOCK",
                Some(2),
                false,
            ),
            ("FUN", 131, 145, "same", 135, 139, "KT_FILE", None, false),
        ],
        &[],
    );
    let error_node = results[1].1.tree_for(&results[1].0).unwrap();
    check(
        error_node,
        &[
            ("FUN", 0, 11, "broken", 4, 10, "KT_FILE", None, true),
            ("PROPERTY", 12, 25, "later", 16, 21, "KT_FILE", None, false),
            ("FUN", 26, 43, "healthy", 30, 37, "KT_FILE", None, false),
        ],
        &[(11, 11, "Expecting ')'", Some(0))],
    );
    println!(
        "lowering_fixture_acceptance: PASS; 2 sequential production snapshots, 10 literal declarations, 1 literal error; separate from ordinary tests and worker_protocol's 22 cases"
    );
}

struct BoundedOutput<W> {
    output: W,
    bytes: usize,
}
impl<W: Write> Write for BoundedOutput<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .bytes
            .checked_add(bytes.len())
            .filter(|&n| n <= MAX_CAPTURE_BYTES)
            .ok_or_else(|| std::io::Error::other("retained capture overflow"))?;
        self.output.write_all(bytes)?;
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

fn capture(name: &str, tree: &Parsed) {
    assert!(
        tree.elements().len() <= MAX_ELEMENTS,
        "retained element capture overflow"
    );
    assert!(
        tree.diagnostics().len() <= MAX_DIAGNOSTICS,
        "retained diagnostic capture overflow"
    );
    let mut output = BoundedOutput {
        output: std::io::stdout().lock(),
        bytes: 0,
    };
    writeln!(
        output,
        "CST_BEGIN {name} source_bytes={} elements={} diagnostics={}",
        tree.source().len(),
        tree.elements().len(),
        tree.diagnostics().len()
    )
    .unwrap();
    for (index, e) in tree.elements().iter().enumerate() {
        assert!(
            e.kind.len() <= MAX_KIND_BYTES,
            "retained kind capture overflow"
        );
        serde_json::to_writer(
            &mut output,
            &serde_json::json!({
                "element": index, "kind": e.kind, "token": e.token,
                "start": e.start, "end": e.end, "parent": e.parent, "subtree_end": e.subtree_end,
            }),
        )
        .unwrap();
        writeln!(output).unwrap();
    }
    for (index, d) in tree.diagnostics().iter().enumerate() {
        assert!(
            d.message.len() <= MAX_MESSAGE_BYTES,
            "retained message capture overflow"
        );
        serde_json::to_writer(
            &mut output,
            &serde_json::json!({
                "diagnostic": index, "start": d.start, "end": d.end, "message": d.message,
            }),
        )
        .unwrap();
        writeln!(output).unwrap();
    }
    writeln!(
        output,
        "CST_END {name} elements={} diagnostics={}",
        tree.elements().len(),
        tree.diagnostics().len()
    )
    .unwrap();
    output.flush().unwrap();
}

fn selected(e: &Element) -> bool {
    !e.token && matches!(e.kind.as_str(), "CLASS" | "FUN" | "PROPERTY")
}

fn owner(elements: &[Element], selected_indices: &[usize], index: usize) -> Option<usize> {
    let mut parent = elements[index].parent;
    while let Some(index) = parent {
        if let Some(ordinal) = selected_indices.iter().position(|&i| i == index) {
            return Some(ordinal);
        }
        parent = elements[index].parent;
    }
    None
}

fn check(
    tree: &Parsed,
    expected: &[Declaration<'_>],
    errors: &[(usize, usize, &str, Option<usize>)],
) {
    let elements = tree.elements();
    assert!(!elements.is_empty());
    assert_eq!(
        (
            elements[0].kind.as_str(),
            elements[0].start,
            elements[0].end,
            elements[0].parent,
            elements[0].subtree_end,
            elements[0].token
        ),
        (
            "KT_FILE",
            0,
            tree.source().len(),
            None,
            elements.len(),
            false
        )
    );
    let selected_indices: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| selected(e))
        .map(|(i, _)| i)
        .collect();
    let error_indices: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.kind == "ERROR_ELEMENT")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(selected_indices.len(), expected.len());
    assert_eq!(error_indices.len(), errors.len());
    assert_eq!(tree.diagnostics().len(), errors.len());
    for (i, e) in elements.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let parent = &elements[e.parent.unwrap()];
        if e.token {
            // Production spelling corrects the separately recorded TMP WHITESPACE typo.
            let allowed = matches!(
                e.kind.as_str(),
                "WHITE_SPACE" | "EOL_COMMENT" | "BLOCK_COMMENT"
            ) || match parent.kind.as_str() {
                "KT_FILE" => e.kind == "SEMICOLON",
                "CLASS" => matches!(e.kind.as_str(), "CLASS_KEYWORD" | "IDENTIFIER"),
                "FUN" => matches!(
                    e.kind.as_str(),
                    "FUN_KEYWORD" | "IDENTIFIER" | "EQ" | "SEMICOLON"
                ),
                "PROPERTY" => matches!(
                    e.kind.as_str(),
                    "VAL_KEYWORD" | "IDENTIFIER" | "EQ" | "SEMICOLON"
                ),
                "CLASS_BODY" | "BLOCK" => {
                    matches!(e.kind.as_str(), "LBRACE" | "RBRACE" | "SEMICOLON")
                }
                "VALUE_PARAMETER_LIST" => matches!(e.kind.as_str(), "LPAR" | "RPAR"),
                "INTEGER_CONSTANT" => e.kind == "INTEGER_LITERAL",
                "ERROR_ELEMENT" => true,
                _ => false,
            };
            assert!(allowed, "unsupported token {i}: {e:?}");
            continue;
        }
        let allowed = match e.kind.as_str() {
            "PACKAGE_DIRECTIVE" | "IMPORT_LIST" => {
                parent.kind == "KT_FILE" && e.start == e.end && e.subtree_end == i + 1
            }
            "CLASS" | "FUN" | "PROPERTY" => {
                matches!(parent.kind.as_str(), "KT_FILE" | "CLASS_BODY" | "BLOCK")
            }
            "CLASS_BODY" => parent.kind == "CLASS",
            "BLOCK" | "VALUE_PARAMETER_LIST" => parent.kind == "FUN",
            "INTEGER_CONSTANT" => matches!(parent.kind.as_str(), "FUN" | "PROPERTY"),
            "ERROR_ELEMENT" => matches!(
                parent.kind.as_str(),
                "KT_FILE"
                    | "CLASS"
                    | "FUN"
                    | "PROPERTY"
                    | "CLASS_BODY"
                    | "BLOCK"
                    | "VALUE_PARAMETER_LIST"
                    | "INTEGER_CONSTANT"
                    | "ERROR_ELEMENT"
            ),
            _ => false,
        };
        assert!(allowed, "unsupported node {i}: {e:?}");
        if e.kind == "INTEGER_CONSTANT" {
            assert_eq!(
                elements
                    .iter()
                    .filter(|c| c.parent == Some(i) && c.token && c.kind == "INTEGER_LITERAL")
                    .count(),
                1
            );
        }
    }
    for kind in ["PACKAGE_DIRECTIVE", "IMPORT_LIST"] {
        assert_eq!(
            elements
                .iter()
                .filter(|e| !e.token && e.kind == kind)
                .count(),
            1
        );
    }
    for (ordinal, (&index, expected)) in selected_indices.iter().zip(expected).enumerate() {
        let e = &elements[index];
        let children: Vec<_> = elements
            .iter()
            .enumerate()
            .filter(|(_, c)| c.parent == Some(index))
            .collect();
        let names: Vec<_> = children
            .iter()
            .filter(|(_, c)| c.token && c.kind == "IDENTIFIER")
            .collect();
        assert_eq!(names.len(), 1, "direct name count at {index}");
        let (name_index, name) = **names.first().unwrap();
        let keyword = match e.kind.as_str() {
            "CLASS" => "CLASS_KEYWORD",
            "FUN" => "FUN_KEYWORD",
            "PROPERTY" => "VAL_KEYWORD",
            _ => unreachable!(),
        };
        let keywords: Vec<_> = children
            .iter()
            .filter(|(_, c)| c.token && c.kind == keyword)
            .collect();
        assert_eq!(keywords.len(), 1);
        assert!(keywords[0].0 < name_index);
        assert!(
            children
                .iter()
                .all(|(i, c)| c.token || c.kind == "ERROR_ELEMENT" || name_index < *i)
        );
        let incomplete = error_indices
            .iter()
            .any(|&i| owner(elements, &selected_indices, i) == Some(ordinal));
        assert_eq!(
            (
                e.kind.as_str(),
                e.start,
                e.end,
                &tree.source()[name.start..name.end],
                name.start,
                name.end,
                elements[e.parent.unwrap()].kind.as_str(),
                owner(elements, &selected_indices, index),
                incomplete
            ),
            *expected
        );
    }
    for ((&index, diagnostic), expected) in error_indices.iter().zip(tree.diagnostics()).zip(errors)
    {
        let e = &elements[index];
        assert_eq!((e.start, e.end), (diagnostic.start, diagnostic.end));
        assert_eq!(
            (
                e.start,
                e.end,
                diagnostic.message.as_str(),
                owner(elements, &selected_indices, index)
            ),
            *expected
        );
    }
    let tokens: String = elements
        .iter()
        .filter(|e| e.token)
        .map(|e| &tree.source()[e.start..e.end])
        .collect();
    assert_eq!(tokens, tree.source());
}
