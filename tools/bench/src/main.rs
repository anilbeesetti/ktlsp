mod process;

use ktlsp_syntax::{Lexed, SyntaxError, lex};
use ktlsp_text::{Document, Position, Range, TextChange, position_to_offset};
use process::Process;
use serde_json::{Value, json};
use std::{
    env, fs,
    hint::black_box,
    io,
    path::Path,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

// Same independently authored malformed EOF cases as the checkpoint's lexer tests.
const MALFORMED: [&str; 10] = [
    "/*",
    "/* /*",
    "\"",
    "\"\\",
    "\"\\uZZZZ\"",
    "\"${f(\"x",
    "\"\"\"raw",
    "\"line\r\nval next = 1",
    "\"${ /*",
    "\"${ //tail",
];
const CHECKPOINT: &str = "0d54973ce66f59362829360261fde6e46b4fc420";
const WARMUP: usize = 10;
const BUDGET: Duration = Duration::from_secs(30);
type Result<T> = std::result::Result<T, String>;

fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn corpus(name: &str) -> String {
    match name {
        "small" => {
            "package demo\r\nclass 名 {\r\n fun hello() = \"😀 café ${1}\"\r\n}\r\n".repeat(4)
        }
        "large" => "val 名 = \"😀 café\" // comment\r\n".repeat(32768),
        "short_tokens" => "@ ".repeat(131072),
        "malformed" => "\"${f(\"x".into(),
        _ => unreachable!(),
    }
}
fn partition(source: &str, result: &Lexed) -> Result<()> {
    let mut end = 0;
    for token in &result.tokens {
        ensure(
            token.start == end
                && token.end > token.start
                && source.is_char_boundary(token.start)
                && source.is_char_boundary(token.end),
            "non-lossless token partition",
        )?;
        end = token.end;
    }
    ensure(end == source.len(), "incomplete partition")?;
    for diagnostic in &result.diagnostics {
        ensure(
            diagnostic.start <= diagnostic.end
                && source.is_char_boundary(diagnostic.start)
                && source.is_char_boundary(diagnostic.end),
            "invalid diagnostic span",
        )?;
    }
    Ok(())
}
fn core_checks() -> Result<()> {
    for name in ["small", "large", "short_tokens", "malformed"] {
        let source = corpus(name);
        let result = lex(&source).map_err(|e| e.to_string())?;
        partition(&source, &result)?;
        ensure(
            result.diagnostics.is_empty() == (name != "malformed"),
            "unexpected lexical diagnostics",
        )?;
    }
    for source in MALFORMED {
        let result = lex(source).map_err(|e| e.to_string())?;
        partition(source, &result)?;
        ensure(
            !result.diagnostics.is_empty(),
            "missing malformed diagnostic",
        )?;
    }
    for (source, expected) in [
        (
            " ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES + 1),
            SyntaxError::SourceTooLarge,
        ),
        (
            "@ ".repeat(ktlsp_syntax::MAX_TOKENS / 2 + 1),
            SyntaxError::TooManyTokens,
        ),
        (
            "\0".repeat(ktlsp_syntax::MAX_DIAGNOSTICS + 1),
            SyntaxError::TooManyDiagnostics,
        ),
        (
            "/*".repeat(ktlsp_syntax::MAX_NESTING + 1),
            SyntaxError::NestingTooDeep,
        ),
    ] {
        ensure(
            lex(&source) == Err(expected),
            "wrong controlled lexer failure",
        )?;
    }
    let mut doc = Document::new(0, "a😀\r\nb".into()).map_err(|e| e.to_string())?;
    let valid = TextChange {
        range: Some(Range {
            start: Position {
                line: 0,
                character: 1,
            },
            end: Position {
                line: 0,
                character: 3,
            },
        }),
        range_length: Some(2),
        text: "水".into(),
    };
    let mut invalid = valid.clone();
    invalid.range.as_mut().unwrap().start.character = 99;
    ensure(
        doc.apply_changes(1, &[valid.clone(), invalid]).is_err(),
        "invalid edit accepted",
    )?;
    ensure(
        doc.version() == 0 && doc.text() == "a😀\r\nb",
        "rollback failed",
    )?;
    doc.apply_changes(1, &[valid]).map_err(|e| e.to_string())?;
    ensure(
        doc.text() == "a水\r\nb" && doc.apply_changes(1, &[]).is_err(),
        "edit/stale version mismatch",
    )?;
    Ok(())
}
fn stats(samples: Vec<f64>) -> Value {
    let mut ordered = samples.clone();
    ordered.sort_by(f64::total_cmp);
    let percentile =
        |p: f64| ordered[((ordered.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    json!({"unit":"ns", "samples":samples, "median":percentile(0.5), "p95":percentile(0.95), "p99":percentile(0.99), "percentile_method":"nearest rank"})
}
fn lexer(name: &str, repetitions: usize) -> Result<Value> {
    let owned = corpus(name);
    let sources: Vec<&str> = if name == "malformed" {
        MALFORMED.to_vec()
    } else {
        vec![&owned]
    };
    let bytes: usize = sources.iter().map(|s| s.len()).sum();
    let batch = if bytes < 1024 { 32 } else { 1 };
    for source in &sources {
        partition(source, &lex(source).map_err(|e| e.to_string())?)?;
    }
    let mut samples = Vec::new();
    for n in 0..WARMUP + repetitions {
        let start = Instant::now();
        for _ in 0..batch {
            for source in &sources {
                black_box(lex(black_box(source)).map_err(|e| e.to_string())?);
            }
        }
        let elapsed = start.elapsed().as_nanos() as f64 / batch as f64;
        if n >= WARMUP {
            samples.push(elapsed);
        }
    }
    let times = stats(samples);
    Ok(
        json!({"workload":format!("lexer_{name}"), "corpus_bytes":bytes, "corpus_cases":sources.len(), "batch_size":batch, "warmup_batches":WARMUP, "timing":"one complete corpus pass: lex allocation, scanning and result drop; batch-average latency", "median_mib_per_second":bytes as f64 / (1024.0*1024.0) / (times["median"].as_f64().unwrap()/1e9), "latency":times}),
    )
}
fn edits(name: &str, repetitions: usize) -> Result<Value> {
    let lines = if name == "small" { 8 } else { 32768 };
    let source = "val 名 = \"😀 café\"\r\n".repeat(lines);
    let positions = [0, lines / 2, lines - 1].map(|line| Position {
        line: line as u32,
        character: 4,
    });
    let offsets = positions.map(|p| position_to_offset(&source, p).unwrap());
    let changes: Vec<_> = ["水", "火"]
        .into_iter()
        .map(|text| {
            positions.map(|start| TextChange {
                range: Some(Range {
                    start,
                    end: Position {
                        character: 5,
                        ..start
                    },
                }),
                range_length: Some(1),
                text: text.into(),
            })
        })
        .collect();
    let mut doc = Document::new(0, source.clone()).map_err(|e| e.to_string())?;
    let mut expected = source.clone();
    let mut samples = Vec::new();
    for n in 0..WARMUP + repetitions {
        let location = n % 3;
        let change = &changes[(n / 3) % 2][location];
        let start = Instant::now();
        doc.apply_changes((n + 1) as i32, std::slice::from_ref(change))
            .map_err(|e| e.to_string())?;
        let elapsed = start.elapsed().as_nanos() as f64;
        expected.replace_range(offsets[location]..offsets[location] + 3, &change.text);
        ensure(
            doc.text() == expected && doc.version() == (n + 1) as i32,
            "rapid edit result mismatch",
        )?;
        if n >= WARMUP {
            samples.push(elapsed);
        }
    }
    Ok(
        json!({"workload":format!("document_core_edits_{name}"),"corpus_bytes":source.len(),"batch_size":1,"warmup_operations":WARMUP,"timing":"Document::apply_changes only; fixed-byte Unicode replacements cycling beginning/middle/end; validation outside timer", "latency":stats(samples)}),
    )
}
fn request(id: usize, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn note(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}
fn start(server: &Path) -> Result<Process> {
    Process::spawn(&mut Command::new(server), BUDGET)
}
fn initialize(p: &mut Process) -> Result<()> {
    p.send(&request(0, "initialize", json!({"capabilities":{}})))?;
    let response = p.receive()?;
    ensure(
        response["id"] == 0
            && response["result"]["capabilities"]["textDocumentSync"]["change"] == 2
            && response.get("error").is_none(),
        "initialize mismatch",
    )?;
    p.send(&note("initialized", json!({})))
}
fn shutdown(p: &mut Process) -> Result<()> {
    p.send(&request(99999, "shutdown", Value::Null))?;
    let response = p.receive()?;
    ensure(
        response["id"] == 99999
            && response.get("result") == Some(&Value::Null)
            && response.get("error").is_none(),
        "shutdown mismatch",
    )?;
    p.send(&note("exit", Value::Null))?;
    let (status, stderr) = p.finish()?;
    ensure(status.success() && stderr == 0, "unclean server exit")
}
fn roundtrip(p: &mut Process, id: usize) -> Result<()> {
    p.send(&request(id, "bench/unsupported", json!({})))?;
    let response = p.receive()?;
    ensure(
        response["id"] == id
            && response["error"]["code"] == -32601
            && response.get("result").is_none(),
        "dispatch roundtrip mismatch",
    )
}
fn server_checks(server: &Path) -> Result<()> {
    let mut p = start(server)?;
    initialize(&mut p)?;
    p.raw(b"Content-Length: 1\r\n\r\n{".to_vec())?;
    ensure(
        p.receive()?["error"]["code"] == -32700,
        "JSON parse error mismatch",
    )?;
    p.send(&note("textDocument/didOpen", json!({"textDocument":{"uri":"untitled:bench","languageId":"kotlin","version":0,"text":"a😀\r\nb"}})))?;
    p.send(&note("textDocument/didChange", json!({"textDocument":{"uri":"untitled:bench","version":1},"contentChanges":[{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"rangeLength":2,"text":"水"}]})))?;
    roundtrip(&mut p, 1)?;
    shutdown(&mut p)?;
    for invalid in [
        b"Content-Length: -1\r\n\r\n".as_slice(),
        b"Content-Length: 999999999\r\n\r\n",
        b"Content-Length: 1\n\n0",
        b"Content-Length: 8\r\n\r\n{}",
    ] {
        let mut p = start(server)?;
        p.raw(invalid.to_vec())?;
        let (status, stderr) = p.finish()?;
        ensure(
            status.code() == Some(1) && stderr > 0,
            "fatal framing failure mismatch",
        )?;
    }
    Ok(())
}
fn server_times(server: &Path, repetitions: usize) -> Result<Vec<Value>> {
    let mut startup = Vec::new();
    for _ in 0..repetitions {
        let begin = Instant::now();
        let mut p = start(server)?;
        initialize(&mut p)?;
        startup.push(begin.elapsed().as_nanos() as f64);
        shutdown(&mut p)?;
    }
    let mut p = start(server)?;
    initialize(&mut p)?;
    let mut samples = Vec::new();
    for n in 0..WARMUP + repetitions {
        let start = Instant::now();
        roundtrip(&mut p, n + 1)?;
        if n >= WARMUP {
            samples.push(start.elapsed().as_nanos() as f64);
        }
    }
    shutdown(&mut p)?;
    Ok(vec![
        json!({"workload":"server_startup_initialize","timing":"spawn through initialize response validation and initialized write; includes harness pipe/thread setup; excludes shutdown","warmup_operations":0,"latency":stats(startup)}),
        json!({"workload":"server_framed_dispatch_roundtrip","timing":"serialized unknown request through matching -32601 response, including harness channels/JSON/framing; no semantic work","warmup_operations":WARMUP,"latency":stats(samples)}),
    ])
}
fn worker(args: &[String]) -> Result<Value> {
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 1 => {
            core_checks()?;
            Ok(json!({"correctness":"passed"}))
        }
        Some("lex" | "edits") if args.len() == 3 => {
            let reps = repetitions(&args[2])?;
            if !["small", "large", "short_tokens", "malformed"].contains(&args[1].as_str()) {
                return Err("unknown corpus".into());
            }
            if args[0] == "lex" {
                lexer(&args[1], reps)
            } else {
                ensure(
                    ["small", "large"].contains(&args[1].as_str()),
                    "unknown edit corpus",
                )?;
                edits(&args[1], reps)
            }
        }
        _ => Err("invalid worker arguments".into()),
    }
}
fn repetitions(value: &str) -> Result<usize> {
    let value = value.parse::<usize>().map_err(|e| e.to_string())?;
    ensure((5..=1001).contains(&value), "repetitions must be 5..=1001")?;
    Ok(value)
}
fn call_worker(args: &[&str]) -> Result<Value> {
    let mut command = Command::new(env::current_exe().map_err(|e| e.to_string())?);
    command.arg("--worker").args(args);
    let mut p = Process::spawn(&mut command, BUDGET)?;
    let result = p.receive()?;
    let (status, stderr) = p.finish()?;
    ensure(status.success() && stderr == 0, "worker failed")?;
    Ok(result)
}
fn run(args: &[String]) -> Result<Value> {
    if args.len() != 4 || args[0] != "run" {
        return Err("usage: ktlsp-baseline run ABSOLUTE_SERVER METADATA_JSON REPETITIONS".into());
    }
    let server = Path::new(&args[1]);
    ensure(server.is_absolute(), "server path must be absolute")?;
    let metadata: Value = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    ensure(
        metadata["product_sha"] == CHECKPOINT && metadata["profile"] == "release",
        "checkpoint/release metadata mismatch",
    )?;
    ensure(
        option_env!("KTLSP_BENCH_CHECKPOINT") == Some(CHECKPOINT),
        "rebuild inside pinned archive with KTLSP_BENCH_CHECKPOINT",
    )?;
    for field in [
        "toolchain",
        "os",
        "cpu",
        "harness_revision",
        "server_sha256",
        "competing_workload",
        "timing_status",
    ] {
        ensure(
            metadata[field]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty()),
            &format!("missing metadata {field}"),
        )?;
    }
    let reps = repetitions(&args[3])?;
    call_worker(&["check"])?;
    server_checks(server)?;
    let mut results = Vec::new();
    for name in ["small", "large", "short_tokens", "malformed"] {
        results.push(call_worker(&["lex", name, &args[3]])?);
    }
    for name in ["small", "large"] {
        results.push(call_worker(&["edits", name, &args[3]])?);
    }
    results.extend(server_times(server, reps)?);
    Ok(
        json!({"schema_version":1,"status":"passed","metadata":metadata,"repetitions":reps,"correctness":"core checks and server controlled failures passed before timing","memory":{"status":"unavailable","reason":"no portable standard-library peak RSS counter; no allocation or retained-memory claims"},"results":results}),
    )
}
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--worker") {
        return match worker(&args[1..]) {
            Ok(value) => {
                let result = ktlsp_protocol::write_frame(
                    &mut io::stdout().lock(),
                    &serde_json::to_vec(&value).unwrap(),
                );
                if result.is_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    match run(&args) {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!(
                "{}",
                json!({"schema_version":1,"status":"failed","error":error})
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_corpus_and_rollback() {
        core_checks().unwrap();
    }
    #[test]
    fn statistics_and_input_bounds() {
        let value = stats(vec![5., 1., 3., 2., 4.]);
        assert_eq!(value["median"], 3.);
        assert_eq!(value["p95"], 5.);
        assert!(repetitions("0").is_err());
        assert!(repetitions("1002").is_err());
        assert!(repetitions("101").is_ok());
        for args in [
            vec!["check", "unexpected"],
            vec!["edits", "malformed", "5"],
            vec!["edits", "short_tokens", "5"],
            vec!["lex", "unknown", "5"],
        ] {
            assert!(worker(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
    #[test]
    fn rapid_edits_and_lexer_workloads() {
        for name in ["small", "large"] {
            edits(name, 5).unwrap();
        }
        for name in ["small", "large", "short_tokens", "malformed"] {
            lexer(name, 5).unwrap();
        }
    }
}
