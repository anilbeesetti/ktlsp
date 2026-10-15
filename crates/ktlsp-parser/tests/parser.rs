#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{FileKind, MAX_NESTING, MAX_SOURCE_BYTES, Parser, ParserError};
use std::{
    io::Write,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
fn parser() -> Parser {
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap()
}
fn check(source: &str, kind: FileKind) -> ktlsp_parser::Parsed {
    let parsed = parser()
        .parse(source, kind, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(parsed.source(), source);
    let tokens: String = parsed
        .elements()
        .iter()
        .filter(|e| e.token)
        .map(|e| &source[e.start..e.end])
        .collect();
    assert_eq!(tokens, source);
    parsed
}
#[test]
fn original_source_partition_crlf_unicode_and_script() {
    let _functional = functional::serial();
    let parsed = check("// 😀\r\nval café = \"😀\"\r\n\r\n", FileKind::Source);
    let e = parsed
        .elements()
        .iter()
        .find(|e| e.token && &parsed.source()[e.start..e.end] == "val")
        .unwrap();
    assert_eq!(
        parsed.position(e.start).unwrap(),
        ktlsp_text::Position {
            line: 1,
            character: 0
        }
    );
    assert!(parsed.position(4).is_err());
    assert!(parsed.position(8).is_err()); // interior of CRLF after astral comment
    check("println(\"x\")\r", FileKind::Script);
    check("", FileKind::Source);
}
#[test]
fn malformed_eof_and_generated_unicode_are_lossless_or_explicitly_rejected() {
    let _functional = functional::serial();
    for source in [
        "fun f(",
        "val x = \"😀",
        "/* unterminated",
        "class 😀 {",
        "val `δ` = '\\'",
        "val x = ${",
        "\0😀\r\n",
    ] {
        match parser().parse(source, FileKind::Source, &AtomicBool::new(false)) {
            Ok(parsed) => {
                assert_eq!(parsed.source(), source);
            }
            Err(error) => assert!(matches!(
                error,
                ParserError::Coverage
                    | ParserError::WorkerFailed
                    | ParserError::Timeout
                    | ParserError::AllocationLimit
            )),
        }
    }
    let alphabet = [
        "a", "(", "<", ")", "{", "}", "\"", "😀", "\r\n", "/*", "'", "\0",
    ];
    let mut seed = 7u64;
    for _ in 0..64 {
        let mut source = String::new();
        for _ in 0..24 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push_str(alphabet[(seed as usize >> 16) % alphabet.len()]);
        }
        let result = parser().parse(&source, FileKind::Source, &AtomicBool::new(false));
        if let Ok(parsed) = result {
            assert_eq!(parsed.source(), source);
        }
    }
    check("fun healthy() = 1", FileKind::Source);
}
#[test]
fn token_loss_is_rejected_and_next_parse_works() {
    let _functional = functional::serial();
    for kind in [FileKind::Source, FileKind::Script] {
        assert_eq!(
            parser()
                .parse("{fun<)]<T:@( {})", kind, &AtomicBool::new(false))
                .unwrap_err(),
            ParserError::Coverage
        );
        check("val x = 1", kind);
    }
}
#[test]
fn backtracking_regressions_are_bounded() {
    let _functional = functional::serial();
    for depth in [8, 10, 12, 32] {
        let source = format!("val x = {}", "(a<".repeat(depth));
        let start = Instant::now();
        let result = parser().parse(&source, FileKind::Source, &AtomicBool::new(false));
        assert!(start.elapsed() < Duration::from_secs(3));
        writeln!(
            std::io::stderr(),
            "allocation-observation: depth={depth} outcome={:?}",
            result.as_ref().map(|_| ())
        )
        .unwrap();
        match result {
            Ok(parsed) => assert_eq!(parsed.source(), source),
            Err(error) => assert!(matches!(
                error,
                ParserError::WorkerFailed | ParserError::Timeout | ParserError::AllocationLimit
            )),
        }
        check("val healthy = 1", FileKind::Source);
    }
}
#[test]
fn preflight_limits_and_cancel_do_not_launch_work() {
    let _functional = functional::serial();
    assert_eq!(
        parser()
            .parse(
                &"x".repeat(MAX_SOURCE_BYTES + 1),
                FileKind::Source,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        ParserError::InputLimit
    );
    assert_eq!(
        parser()
            .parse(
                &"(".repeat(MAX_NESTING + 1),
                FileKind::Source,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        ParserError::NestingLimit
    );
    assert_eq!(
        parser()
            .parse("val x=1", FileKind::Source, &AtomicBool::new(true))
            .unwrap_err(),
        ParserError::Cancelled
    );
    assert!(Parser::new("relative-worker").is_err());
    assert_eq!(
        Parser::new("/definitely/missing/worker")
            .unwrap()
            .parse("", FileKind::Source, &AtomicBool::new(false))
            .unwrap_err(),
        ParserError::Io
    );
}

#[test]
fn actual_worker_output_count_limits_reject_without_partial_tree() {
    let _functional = functional::serial();
    let source = "val x = 0\n".repeat(2000);
    assert!(source.len() < MAX_SOURCE_BYTES);
    assert_eq!(
        parser()
            .parse(&source, FileKind::Source, &AtomicBool::new(false))
            .unwrap_err(),
        ParserError::OutputLimit
    );
    check("val healthy = 1", FileKind::Source);
}

#[test]
fn exact_input_boundary_and_actual_diagnostic_limit() {
    let _functional = functional::serial();
    check(&" ".repeat(MAX_SOURCE_BYTES), FileKind::Source);
    let source = "?\n".repeat(257);
    assert_eq!(
        parser()
            .parse(&source, FileKind::Source, &AtomicBool::new(false))
            .unwrap_err(),
        ParserError::OutputLimit
    );
    check("val healthy = 1", FileKind::Source);
}
