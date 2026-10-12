use ktlsp_parser::{FileKind, Parser};
use std::{sync::atomic::AtomicBool, time::Instant};
fn parse(source: &str) -> ktlsp_parser::Parsed {
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse(source, FileKind::Source, &AtomicBool::new(false))
        .unwrap()
}
#[test]
fn speculative_nested_annotation_resources() {
    for depth in [8, 12, 14, 16, 20, 30, 40, 50] {
        let mut parameter = "x: Int".to_string();
        for _ in 0..depth {
            parameter = format!("@A(fun({parameter}) {{}}) x: Int");
        }
        let source = format!("fun f({parameter}) {{}}");
        let start = Instant::now();
        let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
            .unwrap()
            .parse(&source, FileKind::Source, &AtomicBool::new(false));
        println!(
            "depth={depth} bytes={} result={:?} elapsed={:?}",
            source.len(),
            result.as_ref().map(|x| x.diagnostics().len()),
            start.elapsed()
        );
        if depth <= 16 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(ktlsp_parser::ParserError::NestingLimit)
            ));
        }
        if let Ok(parsed) = result {
            assert_eq!(parsed.source(), source);
            assert!(parsed.diagnostics().is_empty());
        }
        parse("fun healthy() = 1");
    }
}
#[test]
fn original_source_recovery() {
    for newline in ["\n", "\r\n", "\r"] {
        for (prefix, suffix, parent_kind) in [
            ("", "", "KT_FILE"),
            ("class C { ", "}", "CLASS"),
            ("fun outer() { ", "}", "FUN"),
        ] {
            let source = format!(
                "{prefix}fun broken({newline}val `later 🦀` = 1{newline}fun healthy() = 2{newline}{suffix}"
            );
            let parsed = parse(&source);
            for (kind, text, name) in [
                ("FUN", "fun broken(", "broken"),
                ("PROPERTY", "val `later 🦀` = 1", "`later 🦀`"),
                ("FUN", "fun healthy() = 2", "healthy"),
            ] {
                let start = source.find(text).unwrap();
                let (id, e) = parsed
                    .elements()
                    .iter()
                    .enumerate()
                    .find(|(_, e)| {
                        e.kind == kind && e.start == start && e.end == start + text.len()
                    })
                    .unwrap();
                assert!(parsed.elements().iter().any(|token| token.token
                    && token.kind == "IDENTIFIER"
                    && token.parent == Some(id)
                    && token.start == start + 4
                    && token.end == start + 4 + name.len()));
                let mut p = e.parent.unwrap();
                while !["KT_FILE", "CLASS", "FUN"].contains(&parsed.elements()[p].kind.as_str()) {
                    p = parsed.elements()[p].parent.unwrap();
                }
                assert_eq!(parsed.elements()[p].kind, parent_kind);
                assert_eq!(
                    (parsed.elements()[p].start, parsed.elements()[p].end),
                    (0, source.len())
                );
            }
            assert_eq!(parsed.diagnostics().len(), 1);
            assert_eq!(parsed.diagnostics()[0].message, "Expecting ')'");
            assert_eq!(
                (parsed.diagnostics()[0].start, parsed.diagnostics()[0].end),
                (prefix.len() + 11, prefix.len() + 11)
            );
            let text: String = parsed
                .elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect();
            assert_eq!(text, source);
        }
    }
}
