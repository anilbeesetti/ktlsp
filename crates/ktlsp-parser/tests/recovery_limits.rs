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

#[test]
fn generic_prefix_probe_thresholds_decline_without_losing_source() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for (prefix, recovered) in [
        ("@A ".repeat(84), true),
        ("@A ".repeat(85), false),
        (format!("@A/*{}*/", "x".repeat(4084)), true),
        (format!("@A/*{}*/", "x".repeat(4085)), false),
    ] {
        let source = format!("class Box<T\n{prefix}fun f() = 2");
        let snapshot =
            SourceSnapshot::new("generic-prefix-limit", 1, FileKind::Source, source.as_str())
                .unwrap();
        let accepted = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let tree = accepted.tree_for(&snapshot).unwrap();
        let class = tree
            .elements()
            .iter()
            .find(|e| !e.token && e.kind == "CLASS")
            .unwrap();
        assert_eq!(class.end == 11, recovered, "prefix bytes {}", prefix.len());
        if recovered {
            let fun = tree
                .elements()
                .iter()
                .enumerate()
                .find(|(_, e)| !e.token && e.kind == "FUN")
                .unwrap();
            assert_eq!(
                (fun.1.start, fun.1.end, fun.1.parent),
                (12, source.len(), Some(0))
            );
            let name = source.len() - "f() = 2".len();
            assert!(tree.elements().iter().any(|e| e.token
                && e.parent == Some(fun.0)
                && e.kind == "IDENTIFIER"
                && (e.start, e.end) == (name, name + 1)));
            assert_eq!(tree.diagnostics().len(), 1);
            assert_eq!(
                (
                    tree.diagnostics()[0].start,
                    tree.diagnostics()[0].end,
                    tree.diagnostics()[0].message.as_str()
                ),
                (11, 11, "Missing '>'")
            );
        }
        assert_eq!(
            tree.elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
        assert_eq!(snapshot.source(), source);
    }
    let healthy = SourceSnapshot::new(
        "healthy-after-generic-prefix-limit",
        2,
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

#[test]
fn repeated_generic_recovery_and_failed_probes_keep_resource_gates() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for count in [2, 20, 100] {
        let mut source = String::new();
        let mut expected = Vec::new();
        for i in 0..count {
            let class_start = source.len();
            let class = format!("class Box{i}<T");
            source.push_str(&class);
            source.push('\n');
            let fun_start = source.len();
            let fun = format!("@A public fun f{i}() = 1");
            source.push_str(&fun);
            source.push('\n');
            expected.push((
                class_start,
                class_start + class.len(),
                fun_start,
                fun_start + fun.len(),
            ));
        }
        let snapshot = SourceSnapshot::new(
            "repeated-generic-recovery",
            count,
            FileKind::Source,
            source.as_str(),
        )
        .unwrap();
        let accepted = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let tree = accepted.tree_for(&snapshot).unwrap();
        let declarations: Vec<_> = tree
            .elements()
            .iter()
            .filter(|e| !e.token && matches!(e.kind.as_str(), "CLASS" | "FUN"))
            .collect();
        assert_eq!(declarations.len(), count as usize * 2);
        assert_eq!(tree.diagnostics().len(), count as usize);
        for (pair, &(class_start, class_end, fun_start, fun_end)) in
            declarations.as_chunks::<2>().0.iter().zip(&expected)
        {
            assert_eq!(
                (&*pair[0].kind, pair[0].start, pair[0].end, pair[0].parent),
                ("CLASS", class_start, class_end, Some(0))
            );
            assert_eq!(
                (&*pair[1].kind, pair[1].start, pair[1].end, pair[1].parent),
                ("FUN", fun_start, fun_end, Some(0))
            );
        }
        assert_eq!(
            tree.elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
    }
    let parameters = (0..64)
        .map(|i| format!("@A T{i},"))
        .collect::<Vec<_>>()
        .join("\n");
    let source =
        format!("class Box<{parameters}\nfun declined() = 1\nclass Fresh<T\nfun healthy() = 2");
    let snapshot = SourceSnapshot::new(
        "aggregate-generic-probe-limit",
        101,
        FileKind::Source,
        source.as_str(),
    )
    .unwrap();
    let accepted = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = accepted.tree_for(&snapshot).unwrap();
    let first = tree
        .elements()
        .iter()
        .find(|e| !e.token && e.kind == "CLASS")
        .unwrap();
    assert!(first.end > source.find("fun declined").unwrap());
    let healthy = tree
        .elements()
        .iter()
        .find(|e| !e.token && e.kind == "FUN" && &source[e.start..e.end] == "fun healthy() = 2")
        .unwrap();
    assert_eq!(healthy.parent, Some(0));
    assert_eq!(
        tree.elements()
            .iter()
            .filter(|e| e.token)
            .map(|e| &source[e.start..e.end])
            .collect::<String>(),
        source
    );
}

#[test]
fn generic_scratch_annotations_disable_recursive_recovery_probes() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for depth in [8, 12] {
        let mut parameter = "x: Int".to_string();
        for _ in 0..depth {
            parameter = format!("@A(fun({parameter}) {{}}) x: Int");
        }
        let later = format!("@A(fun({parameter}) {{}}) fun healthy() = 2");
        let source = format!("class Box<T\n{later}");
        let snapshot = SourceSnapshot::new(
            "generic-scratch-annotations",
            depth,
            FileKind::Source,
            source.as_str(),
        )
        .unwrap();
        let accepted = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let tree = accepted.tree_for(&snapshot).unwrap();
        assert_eq!(tree.diagnostics().len(), 1);
        let function = tree
            .elements()
            .iter()
            .find(|e| !e.token && e.kind == "FUN" && e.start == 12)
            .unwrap();
        assert_eq!((function.end, function.parent), (source.len(), Some(0)));
        assert_eq!(
            tree.elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
    }
}
