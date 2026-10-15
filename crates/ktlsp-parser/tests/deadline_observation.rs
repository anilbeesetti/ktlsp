#![cfg(unix)]
//! One finite production observation; machinery controls do not prove B3.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    DeadlineReceipt, Element, FileKind, Parsed, Parser, ParserError, SourceSnapshot,
};
use std::{cell::Cell, io::Write, sync::atomic::AtomicBool};

const SOURCE: &str = "val x = (a<(a<(a<(a<(a<(a<(a<(a<(a<(a<(a<(a<";
const HEALTHY: &str = "val healthy=1\n";

fn partition(tree: &Parsed, source: &str) {
    assert_eq!(tree.source(), source);
    let mut end = 0;
    for token in tree.elements().iter().filter(|e| e.token) {
        assert_eq!(token.start, end);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        end = token.end;
    }
    assert_eq!(end, source.len());
}

fn healthy_elements(elements: &[Element], diagnostics: usize) {
    assert_eq!(
        (
            elements.len(),
            elements.iter().filter(|e| e.token).count(),
            diagnostics
        ),
        (11, 6, 0)
    );
    for (element, (kind, start, end, parent, subtree_end, token)) in elements.iter().zip([
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
    ]) {
        assert_eq!(
            (
                element.kind.as_str(),
                element.start,
                element.end,
                element.parent,
                element.subtree_end,
                element.token
            ),
            (kind, start, end, parent, subtree_end, token)
        );
    }
}

#[test]
fn frozen_production_request_reports_first_outcome_and_normal_healthy_followup() {
    let _functional = functional::serial();
    assert_eq!((SOURCE.len(), HEALTHY.len()), (44, 14));
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot =
        SourceSnapshot::new("b3-original-deadline-v1", 1, FileKind::Source, SOURCE).unwrap();
    let original = snapshot.clone();
    let (result, evidence) =
        parser.parse_snapshot_observing_deadline(&snapshot, &AtomicBool::new(false));
    let allowed = matches!(
        &result,
        Ok(_)
            | Err(ParserError::WorkerFailed | ParserError::Timeout | ParserError::AllocationLimit)
    );
    let positive = matches!(&result, Err(ParserError::Timeout))
        && evidence.deadline_selected
        && evidence.decision_send == Some(Ok(16))
        && evidence.receipt == DeadlineReceipt::CheckpointAfterDecision;
    let disposition = if !allowed {
        "UNEXPECTED/B3-HOLD"
    } else if positive {
        "POSITIVE_FINITE_DEADLINE_COMPONENT"
    } else {
        "INCONCLUSIVE/B3-HOLD"
    };
    // Write directly so a successful original CI run retains the first observation.
    writeln!(std::io::stderr(), "B3-original-deadline-v1: document={} revision={} kind={:?} bytes={} outcome={:?} evidence={evidence:?} disposition={disposition} worker={}",
        snapshot.document(), snapshot.revision(), snapshot.kind(), snapshot.source().len(), result.as_ref().map(|_| ()), env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    assert!(allowed, "unexpected first result: {result:?}");
    assert_eq!(snapshot, original);
    if let Ok(result) = result {
        assert_eq!(result.snapshot(), &snapshot);
        partition(result.tree_for(&snapshot).unwrap(), SOURCE);
    }
    let healthy = SourceSnapshot::new(
        "b3-original-deadline-v1-healthy",
        2,
        FileKind::Source,
        HEALTHY,
    )
    .unwrap();
    let result = parser
        .parse_snapshot(&healthy, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.snapshot(), &healthy);
    let tree = result.tree_for(&healthy).unwrap();
    partition(tree, HEALTHY);
    healthy_elements(tree.elements(), tree.diagnostics().len());
    let stale = SourceSnapshot::new(healthy.document(), 3, FileKind::Source, HEALTHY).unwrap();
    assert_eq!(
        result.tree_for(&stale).unwrap_err(),
        ParserError::StaleSnapshot
    );
    assert_eq!(snapshot, original);
    writeln!(
        std::io::stderr(),
        "B3-original-deadline-v1: NORMAL healthy followup PASS elements=11 tokens=6 diagnostics=0"
    )
    .unwrap();
}

#[test]
fn observed_precancellation_never_selects_or_sends_a_deadline() {
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let snapshot = SourceSnapshot::new("b3-cancel-control", 1, FileKind::Source, HEALTHY).unwrap();
    let (result, evidence) =
        parser.parse_snapshot_observing_deadline(&snapshot, &AtomicBool::new(true));
    assert_eq!(result.unwrap_err(), ParserError::Cancelled);
    assert_eq!(evidence, ktlsp_parser::DeadlineEvidence::default());
}

thread_local! {
    static OUTER: Cell<usize> = const { Cell::new(0) };
    static INNER: Cell<usize> = const { Cell::new(0) };
    static NEST: Cell<bool> = const { Cell::new(false) };
}
fn inner() {
    INNER.with(|count| count.set(count.get() + 1));
}
fn outer() {
    OUTER.with(|count| count.set(count.get() + 1));
    if NEST.with(|nest| nest.replace(false)) {
        std::thread::spawn(|| {
            ktrs_parser::parse_file(HEALTHY, ktrs_parser::FileKind::Source);
            assert_eq!((OUTER.with(Cell::get), INNER.with(Cell::get)), (0, 0));
        })
        .join()
        .unwrap();
        ktrs_parser::parse_file_observing_eof(HEALTHY, ktrs_parser::FileKind::Source, inner);
    }
}
fn panic_checkpoint() {
    panic!("test observer unwind");
}

#[test]
fn vendor_observer_restores_nested_unwind_and_thread_local_state() {
    OUTER.with(|count| count.set(0));
    INNER.with(|count| count.set(0));
    NEST.with(|nest| nest.set(true));
    let parsed =
        ktrs_parser::parse_file_observing_eof(HEALTHY, ktrs_parser::FileKind::Source, outer);
    assert_eq!(parsed.tree.text(), HEALTHY);
    assert!(OUTER.with(Cell::get) > 1 && INNER.with(Cell::get) > 0);
    let before = (OUTER.with(Cell::get), INNER.with(Cell::get));
    ktrs_parser::parse_file(HEALTHY, ktrs_parser::FileKind::Source);
    std::thread::spawn(|| {
        ktrs_parser::parse_file(HEALTHY, ktrs_parser::FileKind::Source);
        assert_eq!((OUTER.with(Cell::get), INNER.with(Cell::get)), (0, 0));
    })
    .join()
    .unwrap();
    assert_eq!((OUTER.with(Cell::get), INNER.with(Cell::get)), before);
    assert!(
        std::panic::catch_unwind(|| ktrs_parser::parse_file_observing_eof(
            HEALTHY,
            ktrs_parser::FileKind::Source,
            panic_checkpoint
        ))
        .is_err()
    );
    assert_eq!(
        ktrs_parser::parse_file(HEALTHY, ktrs_parser::FileKind::Source)
            .tree
            .text(),
        HEALTHY
    );
    assert_eq!((OUTER.with(Cell::get), INNER.with(Cell::get)), before);
}

#[cfg(target_os = "linux")]
#[test]
fn file_and_pipe_observer_fallback_preserve_shared_flags_and_healthy_reply() {
    use std::{
        fs::{File, OpenOptions},
        io::{Read, pipe},
        os::fd::{AsFd, AsRawFd, BorrowedFd},
        process::{Command, Stdio},
    };
    fn flags(fd: BorrowedFd<'_>) -> u32 {
        let mut text = String::new();
        File::open(format!("/proc/self/fdinfo/{}", fd.as_raw_fd()))
            .unwrap()
            .take(4097)
            .read_to_string(&mut text)
            .unwrap();
        assert!(text.len() <= 4096);
        u32::from_str_radix(
            text.lines()
                .find_map(|line| line.strip_prefix("flags:"))
                .unwrap()
                .trim(),
            8,
        )
        .unwrap()
    }
    fn fallback(stderr: Stdio, retained: BorrowedFd<'_>) {
        let before = flags(retained);
        assert_eq!(before & 0o4000, 0, "control must start blocking");
        struct Cleanup(std::process::Child);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let deadline = std::time::Instant::now() + ktlsp_parser::TIMEOUT;
        let child = Command::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
            .args(["--source", "--observe-deadline-eof-v1"])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .unwrap();
        let mut child = Cleanup(child);
        child
            .0
            .stdin
            .take()
            .unwrap()
            .write_all(HEALTHY.as_bytes())
            .unwrap();
        let status = loop {
            assert!(
                std::time::Instant::now() < deadline,
                "fallback control exceeded original 500ms policy"
            );
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert!(status.success());
        let mut output = Vec::new();
        child
            .0
            .stdout
            .take()
            .unwrap()
            .take(ktlsp_parser::MAX_OUTPUT_BYTES as u64 + 1)
            .read_to_end(&mut output)
            .unwrap();
        assert!(output.len() <= ktlsp_parser::MAX_OUTPUT_BYTES);
        assert_eq!(flags(retained), before);
        match serde_json::from_slice::<ktlsp_parser::Reply>(&output).unwrap() {
            ktlsp_parser::Reply::Tree {
                elements,
                diagnostics,
            } => healthy_elements(&elements, diagnostics.len()),
            _ => panic!("fallback did not parse the healthy source"),
        }
    }
    let path = std::env::temp_dir().join(format!("ktlsp-b3-stderr-{}", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    fallback(Stdio::from(file.try_clone().unwrap()), file.as_fd());
    let (reader, writer) = pipe().unwrap();
    fallback(Stdio::from(writer.try_clone().unwrap()), writer.as_fd());
    drop(reader);
}
