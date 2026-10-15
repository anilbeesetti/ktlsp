#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, Parser, ParserError, SourceSnapshot};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[test]
fn annotation_probes_remain_bounded_through_production_snapshot_worker() {
    let _functional = functional::serial();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for depth in [8, 12, 14, 16, 20, 30] {
        let mut parameter = "x: Int".to_string();
        let mut anonymous_ranges = Vec::new();
        for _ in 0..depth {
            for (start, end) in &mut anonymous_ranges {
                *start += 7;
                *end += 7;
            }
            anonymous_ranges.insert(0, (3, 3 + 8 + parameter.len()));
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
            let elements = parsed.elements();
            let declarations: Vec<_> = elements
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    !e.token
                        && matches!(
                            e.kind.as_str(),
                            "PACKAGE_DIRECTIVE"
                                | "CLASS"
                                | "OBJECT_DECLARATION"
                                | "FUN"
                                | "PROPERTY"
                                | "TYPEALIAS"
                                | "IMPORT_DIRECTIVE"
                                | "IMPORT_ALIAS"
                        )
                        && !(e.kind == "PACKAGE_DIRECTIVE" && e.start == e.end)
                })
                .collect();
            assert_eq!(declarations.len(), depth as usize + 1);
            for (index, &(id, function)) in declarations.iter().enumerate() {
                let (start, end, parent_kind) = if index == 0 {
                    (0, source.len(), "KT_FILE")
                } else {
                    let (start, end) = anonymous_ranges[index - 1];
                    (6 + start, 6 + end, "VALUE_ARGUMENT")
                };
                assert_eq!(
                    (function.kind.as_str(), function.start, function.end),
                    ("FUN", start, end)
                );
                assert_eq!(elements[function.parent.unwrap()].kind, parent_kind);
                let mut parent = function.parent;
                let owner = loop {
                    match parent {
                        Some(p) if declarations.iter().any(|&(id, _)| id == p) => break Some(p),
                        Some(p) => parent = elements[p].parent,
                        None => break None,
                    }
                };
                assert_eq!(owner, index.checked_sub(1).map(|i| declarations[i].0));
                let names: Vec<_> = elements
                    .iter()
                    .filter(|e| e.token && e.kind == "IDENTIFIER" && e.parent == Some(id))
                    .collect();
                if index == 0 {
                    assert_eq!(names.len(), 1);
                    assert_eq!(
                        (
                            names[0].start,
                            names[0].end,
                            &source[names[0].start..names[0].end]
                        ),
                        (4, 5, "f")
                    );
                } else {
                    assert!(names.is_empty());
                }
            }
            let mut cursor = 0;
            for token in elements.iter().filter(|e| e.token) {
                assert_eq!(token.start, cursor);
                assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
                cursor = token.end;
            }
            assert_eq!(cursor, source.len());
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
    let _functional = functional::serial();
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
        let &(healthy_id, healthy) = functions.last().unwrap();
        assert_eq!(
            (healthy.start, healthy.end, healthy.parent),
            (healthy_start, source.len(), Some(0))
        );
        let names: Vec<_> = parsed
            .elements()
            .iter()
            .filter(|e| e.token && e.kind == "IDENTIFIER" && e.parent == Some(healthy_id))
            .collect();
        assert_eq!(names.len(), 1);
        assert_eq!(
            (
                names[0].start,
                names[0].end,
                &source[names[0].start..names[0].end]
            ),
            (healthy_start + 4, healthy_start + 11, "healthy")
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
    let _functional = functional::serial();
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
fn large_leading_generic_trivia_keeps_copied_prefix_accounting() {
    let _functional = functional::serial();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let trivia = "/*x*/ ".repeat(2200);
    // Blank lines keep ordinary declaration comment binders outside the fixture's ranges.
    let source = format!("class Box<\n\n{trivia}\n\nfun healthy() = 2");
    let fun_start = 10 + trivia.len() + 4;
    let snapshot = SourceSnapshot::new(
        "leading-generic-trivia",
        1,
        FileKind::Source,
        source.as_str(),
    )
    .unwrap();
    let accepted = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = accepted.tree_for(&snapshot).unwrap();
    assert_eq!(trivia.len(), 13200);
    assert_eq!(
        tree.elements()
            .iter()
            .filter(|e| e.token && e.start >= 10 && e.end <= fun_start)
            .count(),
        4401
    );
    let class_id = tree
        .elements()
        .iter()
        .position(|e| !e.token && e.kind == "CLASS")
        .unwrap();
    for (kind, start, end, parent) in [
        ("CLASS", 0, 10, Some(0)),
        ("TYPE_PARAMETER_LIST", 9, 10, Some(class_id)),
    ] {
        let element = tree
            .elements()
            .iter()
            .find(|e| !e.token && e.kind == kind)
            .unwrap();
        assert_eq!(
            (element.start, element.end, element.parent),
            (start, end, parent)
        );
    }
    let (fun_id, fun) = tree
        .elements()
        .iter()
        .enumerate()
        .find(|(_, e)| !e.token && e.kind == "FUN")
        .unwrap();
    assert_eq!(
        (fun.start, fun.end, fun.parent),
        (fun_start, source.len(), Some(0))
    );
    assert!(tree.elements().iter().any(|e| e.token
        && e.kind == "IDENTIFIER"
        && e.parent == Some(fun_id)
        && (e.start, e.end) == (fun_start + 4, fun_start + 11)));
    assert_eq!(tree.diagnostics().len(), 1);
    let error = &tree.diagnostics()[0];
    assert_eq!(
        (error.start, error.end, error.message.as_str()),
        (10, 10, "Type parameter declaration expected")
    );
    assert_eq!(
        tree.elements()
            .iter()
            .filter(|e| e.token)
            .map(|e| &source[e.start..e.end])
            .collect::<String>(),
        source
    );
    assert_eq!(accepted.snapshot(), &snapshot);
    assert_eq!(snapshot.source(), source);
    let healthy = SourceSnapshot::new(
        "healthy-after-leading-generic-trivia",
        2,
        FileKind::Source,
        "fun healthy() = 1",
    )
    .unwrap();
    assert!(
        parser
            .parse_snapshot(&healthy, &AtomicBool::new(false))
            .unwrap()
            .tree_for(&healthy)
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[test]
fn repeated_generic_recovery_and_failed_probes_keep_resource_gates() {
    let _functional = functional::serial();
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
    let _functional = functional::serial();
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
