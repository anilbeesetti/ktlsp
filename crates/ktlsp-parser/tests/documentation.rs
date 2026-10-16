//! Original-worker acceptance, never a synthetic producer/runtime substitute.
#[path = "support/functional.rs"]
mod functional;
use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    documentation::{self, Budget},
    lowering::lower_declarations,
};
use std::sync::atomic::AtomicBool;

fn parse(source: &str) -> SnapshotResult {
    let snapshot =
        SourceSnapshot::new("file:///documentation.kt", 1, FileKind::Source, source).unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    // TEMP397: retain until ROOT and independent OWNER read original hosted CST.
    eprintln!(
        "TEMP397_PRODUCER {}",
        serde_json::json!({
            "document":snapshot.document(), "version":snapshot.revision(),
            "source":tree.source(), "elements":tree.elements(),
            "diagnostics":tree.diagnostics()
        })
    );
    result
}

fn accepted(source: &str, name: &str, expected: &str, doc_spelling: &str) {
    let result = parse(source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
    let mut cursor = 0;
    for leaf in tree.elements().iter().filter(|leaf| leaf.token) {
        assert_eq!(leaf.start, cursor);
        assert!(source.is_char_boundary(leaf.start) && source.is_char_boundary(leaf.end));
        assert!(leaf.start < leaf.end);
        cursor = leaf.end;
    }
    assert_eq!(cursor, source.len());
    let mut budget = Budget::new(&|| false);
    let offset = source.rfind(name).unwrap();
    let token = tree
        .elements()
        .iter()
        .position(|e| {
            e.token && e.kind == "IDENTIFIER" && e.start == offset && e.end == offset + name.len()
        })
        .unwrap();
    assert!(documentation::candidate(tree, token, &mut budget).unwrap());
    let lowered = documentation::lower(&result, current, &mut budget).unwrap();
    let attached = documentation::select(&lowered, &result, current, offset, &mut budget).unwrap();
    assert_eq!(attached.prose(), Some(expected));
    let declaration = attached.declaration();
    assert!(std::ptr::eq(
        declaration,
        lowered
            .declarations()
            .iter()
            .find(|d| d.name_range().contains(&offset))
            .unwrap()
    ));
    assert_eq!(declaration.raw_name(), name);
    assert_eq!(declaration.name_range(), offset..offset + name.len());
    assert!(!declaration.incomplete());
    let doc = attached.documentation_range().unwrap();
    assert_eq!(&source[doc.clone()], doc_spelling);
    let doc_index = tree
        .elements()
        .iter()
        .position(|e| e.kind == "DOC_COMMENT" && e.start == doc.start && e.end == doc.end)
        .unwrap();
    assert_eq!(
        tree.elements()[doc_index].parent,
        Some(declaration.cst_index())
    );
    assert!(
        tree.elements()[doc_index + 1..tree.elements()[doc_index].subtree_end]
            .iter()
            .any(|e| !e.token && e.kind == "KDOC_SECTION" && e.parent == Some(doc_index))
    );
    assert_eq!(attached.source_range(), doc.end..declaration.range().end);
    assert_eq!(
        declaration.range(),
        tree.elements()[declaration.cst_index()].start
            ..tree.elements()[declaration.cst_index()].end
    );
    assert!(!source[attached.source_range()].contains(doc_spelling));
    assert!(
        lower_declarations(&result, current).is_err(),
        "old global grammar must remain strict"
    );
}

#[test]
fn original_prose_keeps_all_nondecoration_bytes_and_line_endings() {
    let _serial = functional::serial();
    accepted(
        "/** hello */\nfun named() {}",
        "named",
        " hello ",
        "/** hello */",
    );
    for newline in ["\n", "\r\n", "\r"] {
        let doc = format!(
            "/**{newline} * 🦀e\u{301}\tfirst{newline} *{newline} *  paragraph{newline} */"
        );
        let source = format!("{doc}{newline}fun named() {{}}");
        let prose =
            format!("{newline}  🦀e\u{301}\tfirst{newline} {newline}   paragraph{newline} ");
        accepted(&source, "named", &prose, &doc);
    }
    accepted(
        "/** kept */\n\n// ordinary\n/* middle */\npublic final fun named() {}",
        "named",
        " kept ",
        "/** kept */",
    );
    accepted(
        "/** quoted */\nfun `🦀e\u{301}`() {}",
        "`🦀e\u{301}`",
        " quoted ",
        "/** quoted */",
    );
    accepted(
        "/** literal <b>&amp; javascript:alert(1) */\nfun named() {}",
        "named",
        " literal <b>&amp; javascript:alert(1) ",
        "/** literal <b>&amp; javascript:alert(1) */",
    );
}

#[test]
fn original_producer_not_proximity_decides_consecutive_and_nested_ownership() {
    let _serial = functional::serial();
    accepted(
        "/** earlier */\n/** chosen */\nfun named() {}",
        "named",
        " chosen ",
        "/** chosen */",
    );
    accepted(
        "class Host {\n/** member */\nfun named() {}\n}",
        "named",
        " member ",
        "/** member */",
    );
    accepted(
        "fun host() {\n/** local */\nval named = 1\n}",
        "named",
        " local ",
        "/** local */",
    );
    accepted(
        "/** outer */\nfun named() { /** local */ val x = 1 }",
        "named",
        " outer ",
        "/** outer */",
    );
    accepted(
        "fun host(/** parameter */ named: Int) {}",
        "named",
        " parameter ",
        "/** parameter */",
    );
    accepted(
        "/** property */\npublic val named = 1",
        "named",
        " property ",
        "/** property */",
    );
    let source = "/** outer */\nclass Host { fun named() {} }";
    let result = parse(source);
    let mut budget = Budget::new(&|| false);
    let lower = documentation::lower(&result, result.snapshot(), &mut budget).unwrap();
    let attached = documentation::select(
        &lower,
        &result,
        result.snapshot(),
        source.find("named").unwrap(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(attached.prose(), None);
    assert_eq!(attached.source_range(), attached.declaration().range());
    assert_eq!(attached.declaration().owner(), Some(0));
}

#[test]
fn docless_ordinary_trailing_and_value_role_boundaries_stay_strict() {
    let _serial = functional::serial();
    for source in [
        "/**/\nfun named() {}",
        "/* ordinary */\nfun named() {}",
        "fun named() {} // trailing",
    ] {
        let result = parse(source);
        let old = lower_declarations(&result, result.snapshot()).unwrap();
        let mut budget = Budget::new(&|| false);
        let lower = documentation::lower(&result, result.snapshot(), &mut budget).unwrap();
        let attached = documentation::select(
            &lower,
            &result,
            result.snapshot(),
            source.find("named").unwrap(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(attached.prose(), None);
        assert_eq!(
            attached.declaration().range(),
            old.declarations()[0].range()
        );
        assert_eq!(attached.source_range(), old.declarations()[0].range());
    }
    for source in [
        "public fun named() {}",
        "fun host() { /** target */ val x = 1; val y = x }",
    ] {
        let result = parse(source);
        let mut budget = Budget::new(&|| false);
        let lower = documentation::lower(&result, result.snapshot(), &mut budget);
        if source.starts_with("public") {
            assert!(lower.is_err());
        } else {
            let lower = lower.unwrap();
            assert!(
                documentation::select(
                    &lower,
                    &result,
                    result.snapshot(),
                    source.rfind('x').unwrap(),
                    &mut budget
                )
                .is_err()
            );
        }
    }
}

#[test]
fn many_original_leading_comments_remain_eligible_for_legacy_docless_hover() {
    let _serial = functional::serial();
    let source = format!("{}fun named() {{}}", "//x\n".repeat(3000));
    let result = parse(&source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    let old = lower_declarations(&result, current).unwrap();
    let declaration = old
        .declarations()
        .iter()
        .find(|d| d.raw_name() == "named")
        .unwrap();
    assert_eq!(declaration.kind(), "FUN");
    assert_eq!(declaration.range(), 0..source.len());
    assert_eq!(declaration.name_range(), 12004..12009);
    let name_token = tree
        .elements()
        .iter()
        .position(|e| {
            e.token && e.kind == "IDENTIFIER" && (e.start..e.end) == declaration.name_range()
        })
        .unwrap();
    let mut budget = Budget::new(&|| false);
    assert!(!documentation::candidate(tree, name_token, &mut budget).unwrap());
}

#[test]
fn semantic_and_malformed_prefixes_refuse_whole_operation() {
    let _serial = functional::serial();
    for source in [
        "/** @return result */\nfun named() {}",
        "/** [named] */\nfun named() {}",
        "/** [external](command:go) */\nfun named() {}",
        "/** ![image](file:C:/path) */\nfun named() {}",
        "/** [reference]: javascript:alert */\nfun named() {}",
        "/** ~~~\n~~~ */\nfun named() {}",
        "/** */\nfun named() {}",
        "/** valid */\npublic private fun named() {}",
        "/** valid */\nfinal open fun named() {}",
        "/** `code` */\nfun named() {}",
        "/** ```\n``` */\nfun named() {}",
        "/**\n *     indented code\n */\nfun named() {}",
        "/** prose */\n@Ann fun named() {}",
        "@Ann(\"/** nested */\") fun named() {}",
        "/** unterminated\nfun named() {}",
        "/** outer /* nested */ tail */\nfun named() {}",
    ] {
        let result = parse(source);
        let mut budget = Budget::new(&|| false);
        let refused = match documentation::lower(&result, result.snapshot(), &mut budget) {
            Err(_) => true,
            Ok(lower) => documentation::select(
                &lower,
                &result,
                result.snapshot(),
                source.find("named").unwrap(),
                &mut budget,
            )
            .is_err(),
        };
        assert!(
            refused,
            "must not return raw/empty/enclosing fallback: {source}"
        );
    }
}

#[test]
fn original_snapshot_lowering_and_name_authority_cannot_be_transplanted() {
    let _serial = functional::serial();
    let source = "/** prose */\nfun named() {}";
    let result = parse(source);
    let foreign = parse(source);
    let mut first = Budget::new(&|| false);
    let lower = documentation::lower(&result, result.snapshot(), &mut first).unwrap();
    let mut second = Budget::new(&|| false);
    let other = documentation::lower(&result, result.snapshot(), &mut second).unwrap();
    assert!(
        documentation::select(
            &other,
            &result,
            result.snapshot(),
            source.find("named").unwrap(),
            &mut first
        )
        .is_err()
    );
    assert!(
        documentation::select(
            &lower,
            &foreign,
            foreign.snapshot(),
            source.find("named").unwrap(),
            &mut second
        )
        .is_err()
    );
    for offset in [
        0,
        source.find("prose").unwrap(),
        source.find("fun").unwrap(),
        source.find("named").unwrap() + 5,
    ] {
        let mut budget = Budget::new(&|| false);
        let lower = documentation::lower(&result, result.snapshot(), &mut budget).unwrap();
        assert!(
            documentation::select(&lower, &result, result.snapshot(), offset, &mut budget).is_err()
        );
    }
    let stale =
        SourceSnapshot::new("file:///documentation.kt", 2, FileKind::Source, source).unwrap();
    let mut budget = Budget::new(&|| false);
    assert!(documentation::lower(&result, &stale, &mut budget).is_err());
}
