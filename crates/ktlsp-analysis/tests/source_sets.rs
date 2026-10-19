use ktlsp_analysis::{Analyzer, Error, Severity};
use ktlsp_parser::{FileKind, ParserError, SourceSnapshot};
use std::{path::Path, sync::atomic::AtomicBool};

fn snapshot(name: &str, version: i64, source: &str) -> SourceSnapshot {
    SourceSnapshot::new(name, version, FileKind::Source, source).unwrap()
}
fn analyzer() -> Analyzer {
    Analyzer::new(Path::new(env!("CARGO_BIN_EXE_ktlsp-analysis-worker"))).unwrap()
}

#[test]
fn cross_file_generic_inference_and_real_diagnostics_follow_edits() {
    let analyzer = analyzer();
    let sources = [
        snapshot(
            "file:///Box.kt",
            1,
            "package shared\r\nclass Box<T>(val value: T)\r\nfun make() = Box(42)\r\n",
        ),
        snapshot(
            "file:///Use.kt",
            1,
            "// 😀\r\npackage app\r\nimport shared.make\r\nfun read(): Int {\r\n var total = make().value + 1\r\n if (total > 1) { total = total + 1 }\r\n return total\r\n}\r\n",
        ),
    ];
    let (analysis, closed) =
        analyzer.analyze_snapshots_with_cleanup(&sources, &AtomicBool::new(false));
    assert!(closed);
    let analysis = analysis.unwrap();
    let files = analysis.files_for(&sources).unwrap();
    assert!(
        files.iter().all(|file| file
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error)),
        "{files:?}"
    );
    assert!(
        files[1].expressions.iter().any(|expression| {
            &sources[1].source()[expression.start..expression.end] == "make().value"
                && expression.ty == "Int"
        }),
        "{:?}",
        files[1].expressions
    );

    let mut changed = sources.clone();
    changed[1] = snapshot(
        "file:///Use.kt",
        2,
        "// 😀\r\npackage app\r\nimport shared.make\r\nfun read(): Int { var total = make().value + 1; return \"bad😀\" }\r\n",
    );
    assert!(matches!(
        analysis.files_for(&changed),
        Err(Error::Parser(ParserError::StaleSnapshot))
    ));
    let (broken, closed) =
        analyzer.analyze_snapshots_with_cleanup(&changed, &AtomicBool::new(false));
    assert!(closed);
    let broken = broken.unwrap();
    let files = broken.files_for(&changed).unwrap();
    assert!(
        files[1].diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Error
                && diagnostic.message.contains("Int")
                && diagnostic.message.contains("String")
                && changed[1].source()[diagnostic.start..diagnostic.end].contains("bad😀")
        }),
        "{:?}",
        files[1].diagnostics
    );
    assert!(files.iter().all(|file| file.expressions.is_empty()));

    changed[1] = snapshot(
        "file:///Use.kt",
        3,
        "// 😀\r\npackage app\r\nimport shared.make\r\nfun read(): Int = make().value\r\n",
    );
    let (fixed, closed) =
        analyzer.analyze_snapshots_with_cleanup(&changed, &AtomicBool::new(false));
    assert!(closed);
    let fixed = fixed.unwrap();
    let files = fixed.files_for(&changed).unwrap();
    assert!(
        files.iter().all(|file| file
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error)),
        "{files:?}"
    );
    assert!(files[1].expressions.iter().any(|expression| {
        &changed[1].source()[expression.start..expression.end] == "make().value"
            && expression.ty == "Int"
    }));
}

#[test]
fn excessive_template_recursion_refuses_whole_request_then_healthy_analysis_works() {
    let analyzer = analyzer();
    let mut expression = "1".to_owned();
    for _ in 0..140 {
        expression = format!("\"${{{expression}}}\"");
    }
    let source = snapshot("file:///deep.kt", 1, &format!("fun deep() = {expression}"));
    let (result, closed) = analyzer.analyze_snapshot_with_cleanup(&source, &AtomicBool::new(false));
    assert!(closed);
    assert!(matches!(result, Err(Error::Resource)), "{result:?}");
    let healthy = snapshot("file:///healthy.kt", 1, "fun value(): Int = 42");
    let (result, closed) =
        analyzer.analyze_snapshot_with_cleanup(&healthy, &AtomicBool::new(false));
    assert!(closed);
    let result = result.unwrap();
    assert!(
        result.files_for(std::slice::from_ref(&healthy)).unwrap()[0]
            .expressions
            .iter()
            .any(|expression| {
                &healthy.source()[expression.start..expression.end] == "42"
                    && expression.ty == "Int"
            })
    );
}

#[test]
fn native_resource_scope_is_cumulative_and_does_not_swallow_unrelated_panics() {
    use krusty_frontend::{LimitExceeded, Limits, diag::DiagSink, with_limits};
    let mut diagnostics = DiagSink::new();
    let result = with_limits(Limits { work: 1, depth: 32 }, || {
        with_limits(
            Limits {
                work: usize::MAX,
                depth: usize::MAX,
            },
            || krusty_frontend::lexer::lex("val x = 1", &mut diagnostics),
        )
    });
    assert!(matches!(result, Err(LimitExceeded::Work)));
    let panic = std::panic::catch_unwind(|| {
        with_limits(
            Limits {
                work: 10,
                depth: 10,
            },
            || panic!("unrelated failure"),
        )
    });
    assert_eq!(
        panic.unwrap_err().downcast_ref::<&str>(),
        Some(&"unrelated failure")
    );
    let healthy = with_limits(
        Limits {
            work: 100,
            depth: 10,
        },
        || krusty_frontend::lexer::lex("42", &mut diagnostics),
    )
    .unwrap();
    assert_eq!(healthy[0].kind, krusty_frontend::token::TokenKind::IntLit);
    assert_eq!(healthy[0].text("42"), "42");
}
