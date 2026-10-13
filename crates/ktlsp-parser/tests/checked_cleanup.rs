use ktlsp_parser::{FileKind, Parser, SourceSnapshot};
use std::sync::atomic::AtomicBool;

#[test]
fn checked_cleanup_preserves_healthy_snapshots() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let source = "val healthy=1\n";
    let expected = [
        ("KT_FILE", 0, 14, None, 11, false),
        ("PACKAGE_DIRECTIVE", 0, 0, Some(0), 2, false),
        ("IMPORT_LIST", 0, 0, Some(0), 3, false),
        ("PROPERTY", 0, 13, Some(0), 10, false),
        ("VAL_KEYWORD", 0, 3, Some(3), 5, true),
        ("WHITE_SPACE", 3, 4, Some(3), 6, true),
        ("IDENTIFIER", 4, 11, Some(3), 7, true),
        ("EQ", 11, 12, Some(3), 8, true),
        ("INTEGER_CONSTANT", 12, 13, Some(3), 10, false),
        ("INTEGER_LITERAL", 12, 13, Some(8), 10, true),
        ("WHITE_SPACE", 13, 14, Some(0), 11, true),
    ];
    for revision in [1, 2] {
        let snapshot = SourceSnapshot::new(
            "reap-production-healthy",
            revision,
            FileKind::Source,
            source,
        )
        .unwrap();
        let result = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(result.snapshot(), &snapshot);
        let tree = result.tree_for(&snapshot).unwrap();
        assert_eq!(tree.source(), source);
        assert!(tree.diagnostics().is_empty());
        assert_eq!(tree.elements().len(), expected.len());
        for (element, expected) in tree.elements().iter().zip(expected) {
            assert_eq!(
                (
                    element.kind.as_str(),
                    element.start,
                    element.end,
                    element.parent,
                    element.subtree_end,
                    element.token,
                ),
                expected
            );
        }
        let mut cursor = 0;
        let mut tokens = 0;
        for token in tree.elements().iter().filter(|element| element.token) {
            assert_eq!(token.start, cursor);
            assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
            cursor = token.end;
            tokens += 1;
        }
        assert_eq!((cursor, tokens), (source.len(), 6));
    }
}
