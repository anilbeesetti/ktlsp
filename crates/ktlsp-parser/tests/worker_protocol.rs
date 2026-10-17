//! Rust-only fault executable and assert-based public-boundary suite.
#[path = "support/functional.rs"]
mod functional;

#[cfg(not(unix))]
use ktlsp_parser::PipedTimeoutSite;
use ktlsp_parser::{
    Diagnostic, Element, FileKind, MAX_ALLOCATION_BYTES, MAX_OUTPUT_BYTES, Parser, ParserError,
    Reply, SnapshotResult, SourceSnapshot,
};
use std::{
    alloc::{GlobalAlloc, Layout},
    io::{Read, Write},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[path = "../src/worker_allocator.rs"]
mod worker_allocator;

fn main() {
    if std::env::args().any(|arg| arg == "--source") {
        fault_worker();
        return;
    }
    let _functional = functional::serial();
    let faulty = Parser::new(std::env::current_exe().unwrap()).unwrap();
    let healthy = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    #[cfg(unix)]
    let timeout = ParserError::Timeout;
    #[cfg(not(unix))]
    let timeout = ParserError::PipedTimeout(PipedTimeoutSite::SupervisorPoll);
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
        ("allocation_alloc", ParserError::AllocationLimit),
        ("allocation_zeroed", ParserError::AllocationLimit),
        ("allocation_realloc", ParserError::AllocationLimit),
        ("hang", timeout),
        ("depth129-token", ParserError::NestingLimit),
        ("depth129-empty", ParserError::NestingLimit),
    ] {
        let allocation = source.starts_with("allocation_");
        if allocation {
            check_allocation_exit(source);
        }
        let snapshot = SourceSnapshot::new("fault-test", 1, FileKind::Source, source).unwrap();
        let error = faulty
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_err();
        if source == "crash" {
            // Abort completion can cross the deadline checked before exit status.
            assert!(error == ParserError::WorkerFailed || error == timeout);
        } else {
            assert_eq!(error, expected, "{source}");
        }
        assert_eq!(snapshot.source(), source);
        let next = SourceSnapshot::new(
            "healthy",
            2,
            FileKind::Source,
            if allocation {
                "val healthy=1\n"
            } else {
                "val healthy = 1\r\n"
            },
        )
        .unwrap();
        let result = healthy
            .parse_snapshot(&next, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("healthy worker after {source}: {error:?}"));
        assert_eq!(result.tree_for(&next).unwrap().source(), next.source());
        if allocation {
            check_healthy_snapshot(&result, &next);
        }
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
        "worker_protocol: 22 fault cases, exact depth boundaries, midflight cancellation, and healthy followups passed"
    );
}

fn fault_worker() {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).unwrap();
    if source.starts_with("allocation_") {
        allocation_fault(&source);
    }
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

fn allocation_fault(source: &str) -> ! {
    use worker_allocator::{LimitedAllocator, worker_exit_code};
    assert_eq!(worker_exit_code(true), 0);
    assert_eq!(worker_exit_code(false), 1);
    let small = Layout::from_size_align(1, 1).unwrap();
    let refused = Layout::from_size_align(MAX_ALLOCATION_BYTES + 1, 1).unwrap();
    // Direct trait calls exercise each callback without optimizable vec-only evidence.
    unsafe {
        match source {
            "allocation_alloc" => assert!(LimitedAllocator.alloc(refused).is_null()),
            "allocation_zeroed" => assert!(LimitedAllocator.alloc_zeroed(refused).is_null()),
            "allocation_realloc" => {
                let original = LimitedAllocator.alloc(small);
                assert!(!original.is_null());
                original.write(0xA5);
                assert!(
                    LimitedAllocator
                        .realloc(original, small, refused.size())
                        .is_null()
                );
                assert_eq!(original.read(), 0xA5);
                LimitedAllocator.dealloc(original, small);
            }
            _ => panic!("unknown allocation fault"),
        }
        assert_eq!(worker_exit_code(true), 86);
        assert_eq!(worker_exit_code(false), 86);
        if source == "allocation_alloc" {
            let later = LimitedAllocator.alloc(small);
            assert!(!later.is_null());
            LimitedAllocator.dealloc(later, small);
            assert_eq!(worker_exit_code(true), 86);
        }
    }
    if source == "allocation_alloc" {
        std::io::stdout().write_all(br#"{"Tree":{"elements":[{"kind":"KT_FILE","start":0,"end":16,"parent":null,"subtree_end":3,"token":false},{"kind":"REFERENCE_EXPRESSION","start":0,"end":16,"parent":0,"subtree_end":3,"token":false},{"kind":"IDENTIFIER","start":0,"end":16,"parent":1,"subtree_end":3,"token":true}],"diagnostics":[]}}"#).unwrap();
        std::io::stdout().flush().unwrap();
    }
    std::process::exit(worker_exit_code(source != "allocation_zeroed"));
}

fn check_allocation_exit(source: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--source")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_clear()
        .spawn()
        .unwrap();
    let input = child.stdin.take().unwrap().write_all(source.as_bytes());
    if let Err(error) = input {
        let _ = child.kill();
        let cleanup = child.wait();
        panic!("{source}: input error {error}; same-child cleanup {cleanup:?}");
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(2));
            }
            outcome => {
                let _ = child.kill();
                let cleanup = child.wait();
                panic!(
                    "{source}: first status outcome {outcome:?}; same-child cleanup {cleanup:?}"
                );
            }
        }
    };
    if status.code() != Some(86) {
        let _ = child.kill();
        let cleanup = child.wait();
        panic!("{source}: first status {status:?}; same-child cleanup {cleanup:?}");
    }
    assert!(!status.success());
    assert_eq!(child.wait().unwrap(), status);
    assert_eq!(child.try_wait().unwrap(), Some(status));
    println!("{source}: raw code86 and same-owned-Child wait/try_wait confirmed");
}

fn check_healthy_snapshot(result: &SnapshotResult, snapshot: &SourceSnapshot) {
    assert_eq!(result.snapshot(), snapshot);
    let tree = result.tree_for(snapshot).unwrap();
    assert_eq!(tree.source(), "val healthy=1\n");
    assert!(tree.diagnostics().is_empty());
    let actual: Vec<_> = tree
        .elements()
        .iter()
        .map(|e| {
            (
                e.kind.as_str(),
                e.start,
                e.end,
                e.parent,
                e.subtree_end,
                e.token,
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
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
        ]
    );
    let tokens: String = tree
        .elements()
        .iter()
        .filter(|e| e.token)
        .map(|e| &tree.source()[e.start..e.end])
        .collect();
    assert_eq!(tokens, snapshot.source());
    assert_eq!(&tree.source()[4..11], "healthy");
}
