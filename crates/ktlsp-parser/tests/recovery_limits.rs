use ktlsp_parser::{FileKind, Parser, ParserError, SourceSnapshot};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[test]
fn annotation_probes_remain_bounded_through_production_snapshot_worker() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for depth in [8, 12, 14, 16, 20, 30] {
        let mut parameter = "x: Int".to_string();
        for _ in 0..depth {
            parameter = format!("@A(fun({parameter}) {{}}) x: Int");
        }
        let source = format!("fun f({parameter}) {{}}");
        let snapshot = SourceSnapshot::new(
            "annotation-probes",
            depth,
            FileKind::Source,
            source.as_str(),
        )
        .unwrap();
        let start = Instant::now();
        let result = parser.parse_snapshot(&snapshot, &AtomicBool::new(false));
        assert!(start.elapsed() < Duration::from_secs(3));
        if depth <= 16 {
            let accepted = result.unwrap();
            let parsed = accepted.tree_for(&snapshot).unwrap();
            assert_eq!(parsed.source(), source);
            assert!(parsed.diagnostics().is_empty());
        } else {
            assert_eq!(result.unwrap_err(), ParserError::NestingLimit);
        }
        assert_eq!(snapshot.source(), source);
        let healthy = SourceSnapshot::new(
            "healthy-after-annotation-probe",
            depth + 1,
            FileKind::Source,
            "fun healthy() = 1",
        )
        .unwrap();
        let accepted = parser
            .parse_snapshot(&healthy, &AtomicBool::new(false))
            .unwrap();
        assert!(
            accepted
                .tree_for(&healthy)
                .unwrap()
                .diagnostics()
                .is_empty()
        );
    }
}

#[test]
fn repeated_recovery_keeps_every_original_declaration_until_diagnostic_gate() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for count in [2, 20, 100] {
        let mut source = String::new();
        let mut ranges = Vec::new();
        for i in 0..count {
            let name = format!("broken{i}");
            let declaration = format!("fun {name}(");
            ranges.push((source.len(), source.len() + declaration.len(), name));
            source.push_str(&declaration);
            source.push('\n');
        }
        let healthy_start = source.len();
        source.push_str("fun healthy() = 1");
        let snapshot = SourceSnapshot::new(
            "repeated-parameter-recovery",
            count,
            FileKind::Source,
            source.as_str(),
        )
        .unwrap();
        let result = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let parsed = result.tree_for(&snapshot).unwrap();
        let functions: Vec<_> = parsed
            .elements()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.token && e.kind == "FUN")
            .collect();
        assert_eq!(functions.len(), count as usize + 1);
        assert_eq!(parsed.diagnostics().len(), count as usize);
        for ((id, function), (start, end, name)) in functions.iter().zip(ranges.iter()) {
            assert_eq!(
                (function.start, function.end, function.parent),
                (*start, *end, Some(0))
            );
            assert!(parsed.elements().iter().any(|e| e.token
                && e.parent == Some(*id)
                && e.kind == "IDENTIFIER"
                && (e.start, e.end) == (*start + 4, *start + 4 + name.len())));
        }
        let healthy = functions.last().unwrap().1;
        assert_eq!(
            (healthy.start, healthy.end, healthy.parent),
            (healthy_start, source.len(), Some(0))
        );
        for (diagnostic, (_, end, _)) in parsed.diagnostics().iter().zip(ranges.iter()) {
            assert_eq!(
                (
                    diagnostic.start,
                    diagnostic.end,
                    diagnostic.message.as_str()
                ),
                (*end, *end, "Expecting ')'")
            );
        }
        assert_eq!(
            parsed
                .elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
    }
    let source = "fun broken(\nval later = 1\n)\n".repeat(257);
    let snapshot = SourceSnapshot::new(
        "recovery-diagnostic-limit",
        257,
        FileKind::Source,
        source.as_str(),
    )
    .unwrap();
    assert_eq!(
        parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_err(),
        ParserError::OutputLimit
    );
    let healthy = SourceSnapshot::new(
        "healthy-after-recovery-limit",
        258,
        FileKind::Source,
        "val healthy = 1",
    )
    .unwrap();
    let result = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert!(result.tree_for(&healthy).unwrap().diagnostics().is_empty());
}
