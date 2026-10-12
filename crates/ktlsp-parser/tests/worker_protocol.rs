//! Rust-only fault executable and assert-based public-boundary suite.
use ktlsp_parser::{
    Diagnostic, Element, FileKind, MAX_OUTPUT_BYTES, Parser, ParserError, Reply, SourceSnapshot,
};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

fn main() {
    if std::env::args().any(|arg| arg == "--source") {
        fault_worker();
        return;
    }
    let faulty = Parser::new(std::env::current_exe().unwrap()).unwrap();
    let healthy = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for (source, expected) in [
        ("unknown-kind", ParserError::InvalidTree),
        ("wrong-root", ParserError::InvalidTree),
        ("node-as-token", ParserError::InvalidTree),
        ("wrong-span", ParserError::InvalidTree),
        ("nonempty-leaf", ParserError::InvalidTree),
        ("missing-diagnostic", ParserError::InvalidTree),
        ("wrong-diagnostic", ParserError::InvalidTree),
        ("empty-message", ParserError::InvalidTree),
        ("utf8-😀", ParserError::InvalidTree),
        ("truncated", ParserError::InvalidTree),
        ("unknown-reply", ParserError::InvalidTree),
        ("extra-field", ParserError::InvalidTree),
        ("extra-reply-field", ParserError::InvalidTree),
        ("oversize", ParserError::OutputLimit),
        ("crash", ParserError::WorkerFailed),
        ("exit-failure", ParserError::WorkerFailed),
        ("hang", ParserError::Timeout),
        ("depth129-token", ParserError::NestingLimit),
        ("depth129-empty", ParserError::NestingLimit),
    ] {
        let snapshot = SourceSnapshot::new("fault-test", 1, FileKind::Source, source).unwrap();
        let error = faulty
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_err();
        if source == "crash" {
            // Abort completion can cross the deadline checked before exit status.
            assert!(matches!(
                error,
                ParserError::WorkerFailed | ParserError::Timeout
            ));
        } else {
            assert_eq!(error, expected, "{source}");
        }
        assert_eq!(snapshot.source(), source);
        let next =
            SourceSnapshot::new("healthy", 2, FileKind::Source, "val healthy = 1\r\n").unwrap();
        let result = healthy
            .parse_snapshot(&next, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("healthy worker after {source}: {error:?}"));
        assert_eq!(result.tree_for(&next).unwrap().source(), next.source());
    }
    let snapshot = SourceSnapshot::new("cancel", 1, FileKind::Source, "hang").unwrap();
    let cancelled = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(30));
            cancelled.store(true, Ordering::Relaxed);
        });
        assert_eq!(
            faulty.parse_snapshot(&snapshot, &cancelled).unwrap_err(),
            ParserError::Cancelled
        );
    });
    let accepted = faulty
        .parse_snapshot(
            &SourceSnapshot::new(
                "healthy-fault-executable",
                2,
                FileKind::Source,
                "identifier",
            )
            .unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(accepted.snapshot().source(), "identifier");
    for source in ["depth128-token", "depth128-empty"] {
        let snapshot = SourceSnapshot::new("depth-boundary", 3, FileKind::Source, source).unwrap();
        assert!(
            faulty
                .parse_snapshot(&snapshot, &AtomicBool::new(false))
                .unwrap()
                .tree_for(&snapshot)
                .is_ok()
        );
    }
    println!(
        "worker_protocol: 19 fault cases, exact depth boundaries, midflight cancellation, and healthy followups passed"
    );
}

fn fault_worker() {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).unwrap();
    match source.as_str() {
        "hang" => loop {
            std::thread::sleep(Duration::from_secs(1));
        },
        "crash" => std::process::abort(),
        "exit-failure" => std::process::exit(1),
        "oversize" => {
            std::io::stdout()
                .write_all(&vec![b'x'; MAX_OUTPUT_BYTES + 1])
                .unwrap();
            return;
        }
        "truncated" => {
            print!("{{\"Tree\":");
            return;
        }
        "unknown-reply" => {
            print!("\"UnknownReply\"");
            return;
        }
        _ => {}
    }
    let len = source.len();
    let mut elements = vec![
        Element {
            kind: "KT_FILE".into(),
            start: 0,
            end: len,
            parent: None,
            subtree_end: 3,
            token: false,
        },
        Element {
            kind: "REFERENCE_EXPRESSION".into(),
            start: 0,
            end: len,
            parent: Some(0),
            subtree_end: 3,
            token: false,
        },
        Element {
            kind: "IDENTIFIER".into(),
            start: 0,
            end: len,
            parent: Some(1),
            subtree_end: 3,
            token: true,
        },
    ];
    if source.starts_with("depth") {
        let depth = if source.starts_with("depth128") {
            128
        } else {
            129
        };
        let empty = source.ends_with("empty");
        let count = depth + usize::from(empty);
        elements.clear();
        elements.push(Element {
            kind: "KT_FILE".into(),
            start: 0,
            end: len,
            parent: None,
            subtree_end: count,
            token: false,
        });
        if empty {
            elements.push(Element {
                kind: "IDENTIFIER".into(),
                start: 0,
                end: len,
                parent: Some(0),
                subtree_end: 2,
                token: true,
            });
        }
        for i in elements.len()..count {
            elements.push(Element {
                kind: if !empty && i + 1 == count {
                    "IDENTIFIER"
                } else {
                    "REFERENCE_EXPRESSION"
                }
                .into(),
                start: if empty { len } else { 0 },
                end: len,
                parent: Some(if empty && i == 2 { 0 } else { i - 1 }),
                subtree_end: count,
                token: !empty && i + 1 == count,
            });
        }
    }
    let mut diagnostics = vec![];
    match source.as_str() {
        "unknown-kind" => elements[1].kind = "INVENTED".into(),
        "wrong-root" => elements[0].kind = "SCRIPT".into(),
        "node-as-token" => {
            elements[2].kind = "CLASS".into();
        }
        "wrong-span" => elements[1].start = 1,
        "nonempty-leaf" => {
            elements[1].subtree_end = 2;
            elements[2].parent = Some(0);
        }
        "missing-diagnostic" | "wrong-diagnostic" | "empty-message" => {
            elements[0].subtree_end = 4;
            elements.push(Element {
                kind: "ERROR_ELEMENT".into(),
                start: len,
                end: len,
                parent: Some(0),
                subtree_end: 4,
                token: false,
            });
            if source != "missing-diagnostic" {
                diagnostics.push(Diagnostic {
                    start: if source == "wrong-diagnostic" { 0 } else { len },
                    end: len,
                    message: if source == "empty-message" {
                        ""
                    } else {
                        "Expected declaration"
                    }
                    .into(),
                });
            }
        }
        "utf8-😀" => elements[2].end -= 1,
        _ => {}
    }
    let mut value = serde_json::to_value(Reply::Tree {
        elements,
        diagnostics,
    })
    .unwrap();
    if source == "extra-field" {
        value["Tree"]["elements"][0]["untrusted"] = true.into();
    }
    if source == "extra-reply-field" {
        value["Tree"]["untrusted"] = true.into();
    }
    serde_json::to_writer(std::io::stdout(), &value).unwrap();
}
