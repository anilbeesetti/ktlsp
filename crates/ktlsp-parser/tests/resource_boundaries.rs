//! #107 resource evidence: reconstructed production replies, never captured stdout.
use ktlsp_parser::{
    Element, FileKind, MAX_DIAGNOSTICS, MAX_ELEMENTS, MAX_OUTPUT_BYTES, MAX_SOURCE_BYTES, Parsed,
    Parser, Reply, SourceSnapshot,
};
use std::sync::atomic::AtomicBool;

const PRODUCTION_REPLY_BOUND: usize = 64 + 16_384 * 192 + 256 * 1600;

fn element(e: &Element, kind: &str, span: (usize, usize), parent: Option<usize>, end: usize) {
    assert_eq!((e.kind.as_str(), e.start, e.end), (kind, span.0, span.1));
    assert_eq!((e.parent, e.subtree_end), (parent, end));
    assert_eq!(
        e.token,
        !matches!(
            kind,
            "KT_FILE"
                | "PACKAGE_DIRECTIVE"
                | "IMPORT_LIST"
                | "PROPERTY"
                | "INTEGER_CONSTANT"
                | "ERROR_ELEMENT"
        )
    );
}

fn reconstructed(
    parser: &Parser,
    name: &str,
    source: &str,
    counts: (usize, usize, usize),
    check: impl FnOnce(&Parsed),
) {
    assert!(!source.contains('\r'), "LF normalization must be identity");
    let snapshot = SourceSnapshot::new(name, 1, FileKind::Source, source).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    assert_eq!(result.snapshot(), &snapshot);
    assert_eq!(tree.source(), source);
    assert_eq!(
        (
            tree.elements().len(),
            tree.elements().iter().filter(|e| e.token).count(),
            tree.diagnostics().len()
        ),
        counts
    );
    element(
        &tree.elements()[0],
        "KT_FILE",
        (0, source.len()),
        None,
        counts.0,
    );
    element(&tree.elements()[1], "PACKAGE_DIRECTIVE", (0, 0), Some(0), 2);
    element(&tree.elements()[2], "IMPORT_LIST", (0, 0), Some(0), 3);
    let mut cursor = 0;
    for token in tree.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
    assert!(tree.elements().iter().all(|e| e.kind.len() <= 64));
    assert!(tree.diagnostics().iter().all(|d| d.message.len() <= 256));
    check(tree);
    // LF keeps validate's only wire-field mutations (range remapping) equal to their inputs.
    let bytes = serde_json::to_vec(&Reply::Tree {
        elements: tree.elements().to_vec(),
        diagnostics: tree.diagnostics().to_vec(),
    })
    .unwrap();
    assert!(bytes.len() <= PRODUCTION_REPLY_BOUND);
    assert!(bytes.len() < MAX_OUTPUT_BYTES);
    eprintln!(
        "{name}: reconstructed production-reply serialization bytes={} elements={} tokens={} diagnostics={}",
        bytes.len(),
        counts.0,
        counts.1,
        counts.2
    );
}

fn properties(tree: &Parsed, name: &str, repetitions: usize) {
    let line = format!("val {name}=1\n");
    let name_end = 4 + name.len();
    for i in 0..repetitions {
        let base = 3 + 8 * i;
        let start = i * line.len();
        for (offset, kind, from, to, parent, end) in [
            (0, "PROPERTY", 0, line.len() - 1, 0, base + 7),
            (1, "VAL_KEYWORD", 0, 3, base, base + 2),
            (2, "WHITE_SPACE", 3, 4, base, base + 3),
            (3, "IDENTIFIER", 4, name_end, base, base + 4),
            (4, "EQ", name_end, name_end + 1, base, base + 5),
            (
                5,
                "INTEGER_CONSTANT",
                name_end + 1,
                name_end + 2,
                base,
                base + 7,
            ),
            (
                6,
                "INTEGER_LITERAL",
                name_end + 1,
                name_end + 2,
                base + 5,
                base + 7,
            ),
            (7, "WHITE_SPACE", line.len() - 1, line.len(), 0, base + 8),
        ] {
            element(
                &tree.elements()[base + offset],
                kind,
                (start + from, start + to),
                Some(parent),
                end,
            );
        }
        let identifier = &tree.elements()[base + 3];
        assert_eq!(&tree.source()[identifier.start..identifier.end], name);
    }
}

#[test]
fn lf_production_replies_have_bounded_reconstructed_serialization() {
    assert_eq!(
        (
            MAX_SOURCE_BYTES,
            MAX_ELEMENTS,
            MAX_DIAGNOSTICS,
            MAX_OUTPUT_BYTES
        ),
        (65_536, 16_384, 256, 4_194_304)
    );
    assert_eq!(PRODUCTION_REPLY_BOUND, 3_555_392);
    const { assert!(PRODUCTION_REPLY_BOUND < MAX_OUTPUT_BYTES) };
    for raw in 0..=ktrs_syntax::SyntaxKind::DUMMY_HOLDER as u16 {
        let name = format!("{:?}", ktrs_syntax::SyntaxKind::from_raw(raw));
        assert!(name.len() <= 64);
        assert!(name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'));
    }
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let whitespace = " ".repeat(MAX_SOURCE_BYTES);
    reconstructed(
        &parser,
        "exact-input-whitespace",
        &whitespace,
        (4, 1, 0),
        |tree| {
            element(
                &tree.elements()[3],
                "WHITE_SPACE",
                (0, MAX_SOURCE_BYTES),
                Some(0),
                4,
            );
        },
    );
    let source = "val x=1\n".repeat(2000);
    reconstructed(
        &parser,
        "16003-elements",
        &source,
        (16_003, 12_000, 0),
        |tree| properties(tree, "x", 2000),
    );
    reconstructed(
        &parser,
        "unicode-lf",
        "val `café🦀`=1\n",
        (11, 6, 0),
        |tree| properties(tree, "`café🦀`", 1),
    );
    let source = "?\n".repeat(256);
    reconstructed(
        &parser,
        "256-diagnostics",
        &source,
        (771, 512, 256),
        |tree| {
            for (i, diagnostic) in tree.diagnostics().iter().enumerate() {
                assert_eq!(
                    (
                        diagnostic.start,
                        diagnostic.end,
                        diagnostic.message.as_str()
                    ),
                    (2 * i, 2 * i + 1, "Expecting a top level declaration")
                );
                let base = 3 + 3 * i;
                element(
                    &tree.elements()[base],
                    "ERROR_ELEMENT",
                    (2 * i, 2 * i + 1),
                    Some(0),
                    base + 2,
                );
                element(
                    &tree.elements()[base + 1],
                    "QUEST",
                    (2 * i, 2 * i + 1),
                    Some(base),
                    base + 2,
                );
                element(
                    &tree.elements()[base + 2],
                    "WHITE_SPACE",
                    (2 * i + 1, 2 * i + 2),
                    Some(0),
                    base + 3,
                );
            }
        },
    );
    reconstructed(
        &parser,
        "healthy-followup",
        "val healthy=1\n",
        (11, 6, 0),
        |tree| properties(tree, "healthy", 1),
    );
}
