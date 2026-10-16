//! Real trusted-worker CST consumers. Build BOTH binaries before running this test.
//! No synthetic worker replies, parser bypass, or local #96 reliability claim.
use ktlsp_server::Session;
use ktlsp_text::{Range, offset_to_position, position_to_offset};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

static SERIAL: Mutex<()> = Mutex::new(());
const URI: &str = "untitled:outline";
fn worker() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ktlsp")).with_file_name(format!(
        "ktlsp-parser-worker{}",
        std::env::consts::EXE_SUFFIX
    ))
}
fn request(session: &mut Session, method: &str, params: Value) -> Value {
    let mut responses =
        session.handle(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while responses.is_empty() && std::time::Instant::now() < deadline {
        responses.extend(session.poll());
        if responses.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    assert_eq!(responses.len(), 1, "{responses:?}");
    responses.remove(0)
}
fn notify(session: &mut Session, method: &str, params: Value) {
    session.handle(json!({"jsonrpc":"2.0","method":method,"params":params}));
}
fn initialize(session: &mut Session, profile: Value) -> Value {
    let response = request(
        session,
        "initialize",
        json!({"capabilities":{"textDocument":{"documentSymbol":profile}}}),
    );
    notify(session, "initialized", json!({}));
    response
}
fn running(profile: Value) -> Session {
    let mut session =
        Session::with_parser(worker()).expect("build ktlsp-parser-worker beside ktlsp first");
    assert_eq!(
        initialize(&mut session, profile)["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    session
}
fn open_uri(session: &mut Session, uri: &str, text: &str) {
    notify(
        session,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":text}}),
    );
}
fn open(session: &mut Session, text: &str) {
    open_uri(session, URI, text);
}
fn outline(session: &mut Session) -> Value {
    request(
        session,
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":URI}}),
    )
}
fn change(session: &mut Session, version: i32, text: &str) {
    notify(
        session,
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":text}]}),
    );
}
fn span(source: &str, spelling: &str) -> Value {
    let start = source.find(spelling).unwrap();
    json!(Range {
        start: offset_to_position(source, start).unwrap(),
        end: offset_to_position(source, start + spelling.len()).unwrap()
    })
}
fn assert_containment(symbol: &Value, enclosing: Option<Range>) {
    let range: Range = serde_json::from_value(symbol["range"].clone()).unwrap();
    let selection: Range = serde_json::from_value(symbol["selectionRange"].clone()).unwrap();
    assert!(
        range.start <= selection.start
            && selection.start < selection.end
            && selection.end <= range.end
    );
    if let Some(parent) = enclosing {
        assert!(parent.start <= range.start && range.end <= parent.end);
    }
    for child in symbol["children"].as_array().unwrap() {
        assert_containment(child, Some(range));
    }
}

#[test]
fn actual_cst_hierarchy_preserves_names_ranges_and_opaque_mentions() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "// class Ghost {}\r\nclass Ω {\r\n    fun `δ 😀`() = 1\r\n    val text = \"fun phantom() {}\"\r\n    class Nested\r\n}\r\n";
    let mut session = running(json!({"hierarchicalDocumentSymbolSupport":true}));
    open(&mut session, source);
    let response = outline(&mut session);
    let symbols = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{response}"));
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0]["name"], "Ω");
    assert_eq!(symbols[0]["kind"], 5);
    // The producer binds the adjacent leading comment into the declaration range.
    assert_eq!(symbols[0]["range"], span(source, source.trim_end()));
    assert_eq!(symbols[0]["selectionRange"], span(source, "Ω"));
    let children = symbols[0]["children"].as_array().unwrap();
    assert_eq!(
        children
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["`δ 😀`", "text", "Nested"]
    );
    assert_eq!(children[0]["kind"], 6);
    assert_eq!(
        children[0]["selectionRange"],
        json!({"start":{"line":2,"character":8},"end":{"line":2,"character":14}})
    );
    assert_eq!(children[0]["range"], span(source, "fun `δ 😀`() = 1"));
    assert_containment(&symbols[0], None);
}

#[test]
fn actual_cst_flat_profile_kinds_and_container_names() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "interface I {}\nobject O {}\nenum class Mode { READY, DONE }\nfun top() = 1\nval value=1\ntypealias Alias=String\n";
    let mut session = running(json!({})); // No hierarchy support means SymbolInformation[].
    open(&mut session, source);
    let response = outline(&mut session);
    let symbols = response["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{response}"));
    assert_eq!(
        symbols
            .iter()
            .map(|s| (s["name"].as_str().unwrap(), s["kind"].as_u64().unwrap()))
            .collect::<Vec<_>>(),
        [
            ("I", 11),
            ("O", 5),
            ("Mode", 10),
            ("READY", 10),
            ("DONE", 10),
            ("top", 12),
            ("value", 7),
            ("Alias", 5)
        ]
    );
    assert_eq!(symbols[3]["containerName"], "Mode");
    assert_eq!(symbols[3]["location"]["uri"], URI);
    assert_eq!(symbols[4]["location"]["range"], span(source, "DONE"));
    assert!(
        symbols
            .iter()
            .all(|s| s.get("children").is_none() && s.get("selectionRange").is_none())
    );
    let mut modern = running(
        json!({"hierarchicalDocumentSymbolSupport":true,"symbolKind":{"valueSet":[5,10,11,12,7,19,22]}}),
    );
    open(&mut modern, source);
    let response = outline(&mut modern);
    assert_eq!(response["result"][1]["kind"], 19);
    assert_eq!(response["result"][2]["children"][0]["kind"], 22);
}

#[test]
fn actual_cst_explicit_constructors_properties_and_local_declarations() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "class Box constructor(val item: Int) {\n constructor() : this(1) {}\n fun method() { val local = 1; class Nested }\n}\n";
    let mut session = running(json!({"hierarchicalDocumentSymbolSupport":true}));
    open(&mut session, source);
    let response = outline(&mut session);
    let class = &response["result"][0];
    assert_eq!(class["name"], "Box", "{response}");
    let children = class["children"].as_array().unwrap();
    assert_eq!(
        children
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["constructor", "constructor", "method"]
    );
    assert_eq!(children[0]["kind"], 9);
    assert_eq!(children[0]["children"][0]["name"], "item");
    assert_eq!(children[1]["kind"], 9);
    assert_eq!(children[2]["children"][0]["name"], "local");
    assert_eq!(children[2]["children"][1]["name"], "Nested");
    assert_containment(class, None);
}

#[test]
fn actual_cst_revisions_close_reopen_empty_and_rejection_are_not_cached() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({"hierarchicalDocumentSymbolSupport":true}));
    open(&mut session, "class First");
    assert_eq!(outline(&mut session)["result"][0]["name"], "First");
    change(&mut session, 2, "class Second");
    assert_eq!(outline(&mut session)["result"][0]["name"], "Second");
    change(&mut session, 2, "class Stale");
    assert_eq!(outline(&mut session)["result"][0]["name"], "Second");
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(outline(&mut session)["error"]["code"], -32602);
    open(&mut session, "class Reopened"); // Version 1 must not alias the old open.
    assert_eq!(outline(&mut session)["result"][0]["name"], "Reopened");
    for (version, source) in [
        (2, "fun broken("),
        (3, "fun f() { val (left, right) = pair }"),
        (4, "class C { companion object {} }"),
    ] {
        change(&mut session, version, source);
        let response = outline(&mut session);
        assert_eq!(response["error"]["code"], -32803, "{response}");
        assert!(response.get("result").is_none());
    }
    change(&mut session, 5, "");
    assert_eq!(outline(&mut session)["result"], json!([]));
    change(&mut session, 6, "// fun fake() {}\n/* class Fake */");
    assert_eq!(outline(&mut session)["result"], json!([]));
    change(
        &mut session,
        7,
        &" ".repeat(ktlsp_parser::MAX_SOURCE_BYTES + 1),
    );
    assert_eq!(outline(&mut session)["error"]["code"], -32803);
    change(&mut session, 8, "class Healthy");
    assert_eq!(outline(&mut session)["result"][0]["name"], "Healthy");
}

#[test]
fn actual_cst_nameless_declarations_fail_but_anonymous_expressions_remain_valid() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({"hierarchicalDocumentSymbolSupport":true}));
    open(&mut session, "fun () = 1");
    for (version, source) in [(2, "fun () = 1"), (3, "class C { fun () = 1 }")] {
        change(&mut session, version, source);
        let response = outline(&mut session);
        assert_eq!(response["error"]["code"], -32803);
        assert_eq!(
            response["error"]["message"],
            "document symbols require a complete supported declaration tree"
        );
        assert!(response.get("result").is_none());
    }
    change(&mut session, 4, "val callback = fun () = 1");
    let response = outline(&mut session);
    assert_eq!(response["result"].as_array().unwrap().len(), 1);
    assert_eq!(response["result"][0]["name"], "callback");
    assert_eq!(response["result"][0]["children"], json!([]));
}

#[test]
fn actual_cst_bounds_fail_whole_outline_instead_of_truncating() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({"hierarchicalDocumentSymbolSupport":true}));
    let many = (0..1024)
        .map(|i| format!("class C{i}\n"))
        .collect::<String>();
    open(&mut session, &many);
    assert_eq!(
        outline(&mut session)["result"].as_array().unwrap().len(),
        1024
    );
    change(&mut session, 2, &format!("{many}class Overflow\n"));
    let response = outline(&mut session);
    assert_eq!(response["error"]["code"], -32803);
    assert_eq!(
        response["error"]["message"],
        "document symbol output or nesting limit exceeded"
    );
    let nested = format!("{}{}", "class C {".repeat(32), "}".repeat(32));
    change(&mut session, 3, &nested);
    assert_eq!(outline(&mut session)["result"][0]["name"], "C");
    change(&mut session, 4, &format!("class Outer {{{nested}}}"));
    let response = outline(&mut session);
    assert_eq!(response["error"]["code"], -32803);
    assert_eq!(
        response["error"]["message"],
        "document symbol output or nesting limit exceeded"
    );
    let source_limit = format!(
        "class Exact{}",
        " ".repeat(ktlsp_parser::MAX_SOURCE_BYTES - "class Exact".len())
    );
    change(&mut session, 5, &source_limit);
    assert_eq!(outline(&mut session)["result"][0]["name"], "Exact");
    let mut flat = running(json!({}));
    let long_uri = format!("untitled:{}", "a".repeat(8000));
    let source = (0..32).map(|i| format!("class C{i}\n")).collect::<String>();
    open_uri(&mut flat, &long_uri, &source);
    let response = request(
        &mut flat,
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":long_uri}}),
    );
    assert_eq!(response["error"]["code"], -32803);
    assert_eq!(
        response["error"]["message"],
        "document symbol rendered output limit exceeded"
    );
    let mut limited = running(json!({"symbolKind":{"valueSet":[12]}}));
    open(&mut limited, "class NotAFunction");
    assert_eq!(outline(&mut limited)["error"]["code"], -32803);
}

#[test]
fn provider_is_explicit_and_parser_failures_are_errors() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = Session::new();
    let response = initialize(&mut session, json!({}));
    assert!(
        response["result"]["capabilities"]
            .get("documentSymbolProvider")
            .is_none()
    );
    open(&mut session, "class C");
    assert_eq!(outline(&mut session)["error"]["code"], -32601);
    assert_eq!(
        request(&mut session, "textDocument/documentSymbol", json!({}))["error"]["code"],
        -32601
    );
    assert!(Session::with_parser("relative-worker").is_err());
    assert!(Session::with_parser(worker().with_file_name("ktlsp-no-such-worker")).is_err());
    // A real but wrong executable fails the actual supervised parser protocol.
    let mut failed = Session::with_parser(env!("CARGO_BIN_EXE_ktlsp")).unwrap();
    initialize(&mut failed, json!({}));
    open(&mut failed, "class C");
    assert_eq!(outline(&mut failed)["error"]["code"], -32803);
    for profile in [
        json!({"hierarchicalDocumentSymbolSupport":"yes"}),
        json!({"symbolKind":{"valueSet":[0]}}),
        json!({"symbolKind":null}),
    ] {
        let mut invalid = Session::new();
        assert_eq!(initialize(&mut invalid, profile)["error"]["code"], -32602);
    }
}

struct Package(PathBuf);
impl Package {
    fn new(include_worker: bool) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "ktlsp-outline-package-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::copy(
            env!("CARGO_BIN_EXE_ktlsp"),
            directory.join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
        if include_worker {
            std::fs::copy(worker(), directory.join(worker().file_name().unwrap())).unwrap();
        }
        Self(directory)
    }
    fn exchange(&self, messages: &[Value]) -> Vec<Value> {
        exchange_directory(&self.0, messages)
    }
}
// This directory is borrowed: only Package owns cleanup of its exclusive creation.
fn exchange_directory(directory: &Path, messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(directory.join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX)))
        .arg("--stdio")
        .env("PATH", "")
        .env_remove("JAVA_HOME")
        .env_remove("JDK_HOME")
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    // TEMP397: retain complete original hosted packets until ROOT and OWNER
    // directly inspect the producer/CLI evidence; never print expected echoes.
    let capture397 = messages.iter().any(|message| {
        message["id"].as_str().is_some_and(|id| id.starts_with("TEMP397-"))
    });
    let mut responses: Vec<Value> = Vec::new();
    for message in messages {
        let payload = serde_json::to_vec(message).unwrap();
        if capture397 {
            eprintln!("TEMP397 ACTUAL CLI REQUEST {}", String::from_utf8_lossy(&payload));
        }
        ktlsp_protocol::write_frame(&mut input, &payload).unwrap();
        input.flush().unwrap();
        // Requests really finish before later edits/shutdown. Bulk-writing
        // the entire session would now legitimately cancel/stale symbols.
        if let Some(id) = message.get("id") {
            loop {
                let frame = ktlsp_protocol::read_frame(&mut reader).unwrap().unwrap();
                if capture397 {
                    eprintln!("TEMP397 ACTUAL CLI RESPONSE {}", String::from_utf8_lossy(&frame));
                }
                let response: Value = serde_json::from_slice(&frame).unwrap();
                let matched = response.get("id") == Some(id);
                responses.push(response);
                if matched {
                    break;
                }
            }
        }
    }
    drop(input);
    while let Some(frame) = ktlsp_protocol::read_frame(&mut reader).unwrap() {
        if capture397 {
            eprintln!("TEMP397 ACTUAL CLI RESPONSE {}", String::from_utf8_lossy(&frame));
        }
        responses.push(serde_json::from_slice(&frame).unwrap());
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    responses
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_stdio_packaged_sibling_and_missing_worker_capabilities() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"class Outer { fun inner() = 1 }"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let packaged = Package::new(true).exchange(&messages);
    assert_eq!(
        packaged[0]["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    let symbols = packaged.iter().find(|m| m["id"] == 2).unwrap();
    assert_eq!(symbols["result"][0]["name"], "Outer", "{symbols}");
    assert_eq!(symbols["result"][0]["children"][0]["name"], "inner");
    let missing = Package::new(false).exchange(&messages);
    assert!(
        missing[0]["result"]["capabilities"]
            .get("documentSymbolProvider")
            .is_none()
    );
    assert_eq!(
        missing.iter().find(|m| m["id"] == 2).unwrap()["error"]["code"],
        -32601
    );
}

#[test]
fn packaged_stdio_combines_symbols_pull_and_mandatory_utf16_fallback() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{"general":{"positionEncodings":["utf-8"]},"textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true},"diagnostic":{}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"class Combined { val item = 1 }"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":"\"😀\" ]"}]}}),
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = Package::new(true).exchange(&messages);
    assert_eq!(replies.len(), 6, "{replies:?}");
    assert!(replies.iter().all(|reply| reply.get("id").is_some()));
    let capabilities = &replies[0]["result"]["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(
        capabilities["diagnosticProvider"]["interFileDependencies"],
        false
    );
    assert_eq!(
        capabilities["diagnosticProvider"]["workspaceDiagnostics"],
        false
    );
    let symbols = replies.iter().find(|reply| reply["id"] == 2).unwrap();
    assert_eq!(symbols["result"][0]["name"], "Combined");
    assert_eq!(symbols["result"][0]["children"][0]["name"], "item");
    assert_eq!(
        replies.iter().find(|reply| reply["id"] == 3).unwrap()["result"],
        json!({"kind":"full","items":[]})
    );
    let diagnostics = replies.iter().find(|reply| reply["id"] == 4).unwrap();
    assert_eq!(diagnostics["result"]["kind"], "full");
    assert_eq!(
        diagnostics["result"]["items"][0]["range"],
        json!({"start":{"line":0,"character":5},"end":{"line":0,"character":6}})
    );
    let rejected_symbols = replies.iter().find(|reply| reply["id"] == 5).unwrap();
    assert_eq!(rejected_symbols["error"]["code"], -32803);
}

fn navigation_params(source: &str, offset: usize) -> Value {
    json!({"textDocument":{"uri":URI},"position":offset_to_position(source, offset).unwrap()})
}

#[test]
fn actual_local_binding_definition_hover_ranges_and_refusals() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    let source = "// 😀\r\nfun f() {\r\n val `x 😀` = 1\r\n val y = `x 😀`\r\n}";
    open(&mut session, source);
    let reference = source.rfind("`x 😀`").unwrap();
    let params = navigation_params(source, reference + 1);
    let definition = request(&mut session, "textDocument/definition", params.clone());
    assert_eq!(
        definition["result"],
        json!({"uri":URI,"range":span(source, "`x 😀`")})
    );
    let hover = request(&mut session, "textDocument/hover", params);
    assert_eq!(
        hover["result"],
        json!({
            "contents":{"kind":"plaintext","value":"val `x 😀` = 1"},
            "range":{"start":{"line":3,"character":9},"end":{"line":3,"character":15}}
        })
    );
    // End-exclusive reference selection, and no invented result on a declaration.
    for offset in [source.find("`x 😀`").unwrap(), reference + "`x 😀`".len()] {
        assert_eq!(
            request(
                &mut session,
                "textDocument/definition",
                navigation_params(source, offset)
            )["result"],
            Value::Null
        );
    }
    for position in [
        json!({"line":0,"character":4}),
        json!({"line":50,"character":0}),
        json!({"line":1,"character":-1}),
        json!({"line":1}),
        Value::Null,
    ] {
        let response = request(
            &mut session,
            "textDocument/hover",
            json!({"textDocument":{"uri":URI},"position":position}),
        );
        assert_eq!(response["error"]["code"], -32602, "{response}");
    }
    for (version, source) in [
        (2, "fun f() { val y = unknown }"),
        (3, "fun f() { val y = x; val x = 1 }"),
        (4, "fun f() { val x = 1; val x = 2; val y = x }"),
        (5, "fun f() { var x = 1; val y = x }"),
        (6, "fun f(vararg arg: Int) { val x = 1; val y = x }"),
        (7, "fun f() { val x: Int? = 1; val y = x }"),
        (8, "fun f() { val x = 1; val y = x + 1 }"),
        (9, "val x = 1; val y = x"),
        (10, "fun f() { val x = x }"),
    ] {
        change(&mut session, version, source);
        let reference = if source.contains("unknown") {
            source.find("unknown").unwrap()
        } else {
            source.rfind("= x").unwrap() + 2
        };
        for method in ["textDocument/definition", "textDocument/hover"] {
            let response = request(&mut session, method, navigation_params(source, reference));
            assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
            assert!(response.get("result").is_none());
        }
    }
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(
        request(
            &mut session,
            "textDocument/definition",
            json!({"textDocument":{"uri":URI},"position":{"line":0,"character":0}})
        )["error"]["code"],
        -32602
    );
}

#[test]
fn actual_packaged_stdio_navigation_negotiation_and_existing_providers() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source =
        "// 😀\r\nclass Combined { val item = 1 }\r\nfun f() { val `x😀` = 1; val y = `x😀` }";
    let assert_ids = |replies: &[Value]| {
        let expected = [
            json!(1),
            json!("definition"),
            json!(u64::MAX),
            json!(4),
            json!(5),
            json!(6),
        ];
        assert_eq!(
            replies.len(),
            expected.len(),
            "no unsolicited/push output: {replies:?}"
        );
        for id in expected {
            assert_eq!(
                replies
                    .iter()
                    .filter(|reply| reply.get("id") == Some(&id))
                    .count(),
                1,
                "each original request ID exactly once: {id}: {replies:?}"
            );
        }
    };
    for (profile, expected_kind) in [
        (json!({}), Some("plaintext")),
        (
            json!({"contentFormat":["unknown","markdown","plaintext"]}),
            Some("markdown"),
        ),
        (
            json!({"contentFormat":["plaintext","markdown"]}),
            Some("plaintext"),
        ),
        (json!({"contentFormat":["unknown"]}), None),
        (json!({"contentFormat":[]}), None),
        (json!({"contentFormat":false}), None),
        (json!({"contentFormat":[false,"markdown"]}), None),
        (json!({"contentFormat":["plaintext",null]}), None),
        (json!(false), None),
        (Value::Null, None),
    ] {
        let params = navigation_params(source, source.rfind("`x😀`").unwrap() + 1);
        let messages = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{"general":{"positionEncodings":["utf-8"]},"textDocument":{"hover":profile,"documentSymbol":{"hierarchicalDocumentSymbolSupport":true},"diagnostic":{}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"definition","method":"textDocument/definition","params":params}),
            json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/hover","params":params}),
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
            json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = Package::new(true).exchange(&messages);
        assert_ids(&replies);
        let caps = &replies[0]["result"]["capabilities"];
        assert_eq!(caps["positionEncoding"], "utf-16");
        assert_eq!(caps["definitionProvider"], true);
        assert_eq!(caps.get("hoverProvider").is_some(), expected_kind.is_some());
        let definition = replies.iter().find(|r| r["id"] == "definition").unwrap();
        assert_eq!(
            definition["result"],
            json!({"uri":URI,"range":span(source, "`x😀`")}),
            "profile={profile}; selected definition reply={definition}"
        );
        let hover = replies.iter().find(|r| r["id"] == u64::MAX).unwrap();
        if let Some(kind) = expected_kind {
            assert_eq!(hover["result"]["contents"]["kind"], kind, "{hover}");
            let expected = if kind == "markdown" {
                "```kotlin\nval `x😀` = 1\n```"
            } else {
                "val `x😀` = 1"
            };
            assert_eq!(hover["result"]["contents"]["value"], expected);
            assert_eq!(
                hover["result"]["range"],
                json!({"start":{"line":2,"character":33},"end":{"line":2,"character":38}})
            );
        } else {
            assert_eq!(hover["error"]["code"], -32601);
        }
        let symbols = replies.iter().find(|r| r["id"] == 4).unwrap();
        assert_eq!(symbols["result"][0]["name"], "Combined");
        assert_eq!(symbols["result"][0]["children"][0]["name"], "item");
        assert_eq!(
            replies.iter().find(|r| r["id"] == 5).unwrap()["result"],
            json!({"kind":"full","items":[]})
        );
        let missing = Package::new(false).exchange(&messages);
        assert_ids(&missing);
        let caps = &missing[0]["result"]["capabilities"];
        assert!(caps.get("definitionProvider").is_none() && caps.get("hoverProvider").is_none());
        for id in [json!("definition"), json!(u64::MAX)] {
            assert_eq!(
                missing.iter().find(|r| r["id"] == id).unwrap()["error"]["code"],
                -32601
            );
        }
    }
}

#[test]
fn actual_hover_preserves_declaration_without_initializer_chasing_or_markdown_escape() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = Session::with_parser(worker()).unwrap();
    let initialized = request(
        &mut session,
        "initialize",
        json!({"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}),
    );
    assert_eq!(initialized["result"]["capabilities"]["hoverProvider"], true);
    notify(&mut session, "initialized", json!({}));
    let declaration = "val /*\r\n````\r\n[untrusted](https://invalid.test)\r\n*/ `x` = 1";
    let source = format!("fun f() {{ {declaration}; val y = `x`; val z = y }}");
    open(&mut session, &source);
    let reference = source.rfind("`x`").unwrap();
    let hover = request(
        &mut session,
        "textDocument/hover",
        navigation_params(&source, reference),
    );
    assert_eq!(
        hover["result"]["contents"]["value"],
        format!("`````kotlin\n{declaration}\n`````"),
        "{hover}"
    );
    let reference = source.rfind("= y").unwrap() + 2;
    let params = navigation_params(&source, reference);
    let definition = request(&mut session, "textDocument/definition", params.clone());
    assert_eq!(
        definition["result"],
        json!({"uri":URI,"range":span(&source, "y")})
    );
    let hover = request(&mut session, "textDocument/hover", params);
    assert_eq!(
        hover["result"]["contents"]["value"], "```kotlin\nval y = `x`\n```",
        "{hover}"
    );
    let mut failed = Session::with_parser(env!("CARGO_BIN_EXE_ktlsp")).unwrap();
    initialize(&mut failed, json!({}));
    open(&mut failed, "fun f() { val x = 1; val y = x }");
    for method in ["textDocument/definition", "textDocument/hover"] {
        let response = request(
            &mut failed,
            method,
            json!({"textDocument":{"uri":URI},"position":{"line":0,"character":29}}),
        );
        assert_eq!(response["error"]["code"], -32803, "{response}");
    }
}

fn references_params(source: &str, offset: usize, include: bool) -> Value {
    let mut params = navigation_params(source, offset);
    params["context"] = json!({"includeDeclaration":include});
    params
}
fn references_range(source: &str, offset: usize, width: usize) -> Value {
    json!(Range {
        start: offset_to_position(source, offset).unwrap(),
        end: offset_to_position(source, offset + width).unwrap(),
    })
}

#[test]
fn references_actual_paired_cli_declaration_use_utf16_and_existing_providers() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "// 😀\r\nfun f() {\r\n val `x😀` = 1\r\n val y = `x😀`\r\n val z = `x😀`\r\n val unused = 2\r\n}\r\n";
    let anchors: Vec<_> = source.match_indices("`x😀`").map(|(i, _)| i).collect();
    let locations: Vec<_> = anchors
        .iter()
        .map(|&offset| json!({"uri":URI,"range":references_range(source, offset, "`x😀`".len())}))
        .collect();
    let highlights: Vec<_> = anchors.iter().enumerate().map(|(i, &offset)| {
        json!({"range":references_range(source, offset, "`x😀`".len()),"kind":if i == 0 {3} else {2}})
    }).collect();
    // Incompatible hover must not disable either new provider.
    let mut messages = vec![
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":[]},"diagnostic":{},"documentSymbol":{"hierarchicalDocumentSymbolSupport":true}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
    ];
    let mut expected = vec![(json!("init"), Value::Null)];
    for (index, &offset) in anchors.iter().enumerate() {
        for include in [false, true] {
            let id = json!(format!("refs-{index}-{include}"));
            messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references","params":references_params(source, offset + 1, include)}));
            expected.push((
                id,
                json!(if include {
                    &locations[..]
                } else {
                    &locations[1..]
                }),
            ));
        }
        let id = if index == 0 {
            json!(u64::MAX)
        } else {
            json!(format!("highlight-{index}"))
        };
        messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/documentHighlight","params":navigation_params(source, offset + 1)}));
        expected.push((id, json!(highlights)));
    }
    let unused = source.find("unused").unwrap();
    for include in [false, true] {
        let id = json!(format!("unused-{include}"));
        messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references","params":references_params(source, unused, include)}));
        expected.push((
            id,
            if include {
                json!([{"uri":URI,"range":references_range(source, unused, 6)}])
            } else {
                json!([])
            },
        ));
    }
    messages.push(json!({"jsonrpc":"2.0","id":"unused-highlight","method":"textDocument/documentHighlight","params":navigation_params(source, unused)}));
    expected.push((
        json!("unused-highlight"),
        json!([{"range":references_range(source, unused, 6),"kind":3}]),
    ));
    messages.push(json!({"jsonrpc":"2.0","id":"definition","method":"textDocument/definition","params":navigation_params(source, anchors[1])}));
    expected.push((json!("definition"), locations[0].clone()));
    messages.push(json!({"jsonrpc":"2.0","id":"pull","method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}));
    expected.push((json!("pull"), json!({"kind":"full","items":[]})));
    messages.push(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
    expected.push((json!("stop"), Value::Null));
    messages.push(json!({"jsonrpc":"2.0","method":"exit"}));
    let replies = Package::new(true).exchange(&messages);
    assert_eq!(
        replies.len(),
        expected.len(),
        "no duplicate/push output: {replies:?}"
    );
    for (id, payload) in &expected {
        let matching: Vec<_> = replies
            .iter()
            .filter(|reply| reply.get("id") == Some(id))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "original ID exactly once: {id}: {replies:?}"
        );
        if id != "init" {
            assert_eq!(matching[0]["result"], *payload, "{id}: {replies:?}");
            assert!(matching[0].get("error").is_none());
        }
    }
    let caps = &replies[0]["result"]["capabilities"];
    assert_eq!(caps["referencesProvider"], true);
    assert_eq!(caps["documentHighlightProvider"], true);
    assert_eq!(caps["definitionProvider"], true);
    assert_eq!(caps["documentSymbolProvider"], true);
    assert!(caps.get("hoverProvider").is_none());
    let missing = Package::new(false).exchange(&messages);
    assert_eq!(missing.len(), expected.len());
    assert!(
        missing[0]["result"]["capabilities"]
            .get("referencesProvider")
            .is_none()
    );
    assert!(
        missing[0]["result"]["capabilities"]
            .get("documentHighlightProvider")
            .is_none()
    );
    for (id, _) in expected
        .iter()
        .filter(|(id, _)| id != "init" && id != "pull" && id != "stop")
    {
        let matching: Vec<_> = missing
            .iter()
            .filter(|reply| reply.get("id") == Some(id))
            .collect();
        assert_eq!(matching.len(), 1, "{id}: {missing:?}");
        assert_eq!(matching[0]["error"]["code"], -32601);
    }
}

#[test]
fn typed_local_navigation_and_references_compose_over_paired_stdio() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // The final annotation deliberately spells the value target's name.
    // It must never become a value occurrence or a resolved builtin type.
    let source = "// 😀\r\nclass Int {}\r\nfun f() {\r\n val `x😀`: Long = 1\r\n val y: Int = `x😀`\r\n val a = 2\r\n val b: `x😀` = a\r\n}\r\n";
    let declaration = json!({"start":{"line":3,"character":5},"end":{"line":3,"character":10}});
    let occurrence = json!({"start":{"line":4,"character":14},"end":{"line":4,"character":19}});
    let use_params = json!({"textDocument":{"uri":URI},"position":{"line":4,"character":15}});
    let messages = [
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["plaintext"]},"diagnostic":{}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"typed-definition","method":"textDocument/definition","params":use_params}),
        json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/hover","params":use_params}),
        json!({"jsonrpc":"2.0","id":"typed-references","method":"textDocument/references","params":{"textDocument":{"uri":URI},"position":{"line":3,"character":6},"context":{"includeDeclaration":true}}}),
        json!({"jsonrpc":"2.0","id":"typed-highlights","method":"textDocument/documentHighlight","params":use_params}),
        json!({"jsonrpc":"2.0","id":"untyped-definition","method":"textDocument/definition","params":{"textDocument":{"uri":URI},"position":{"line":6,"character":16}}}),
        json!({"jsonrpc":"2.0","id":"pull","method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = Package::new(true).exchange(&messages);
    assert!(replies.iter().all(|reply| reply.get("id").is_some()));
    assert!(
        replies
            .iter()
            .map(|reply| &reply["id"])
            .eq(messages.iter().filter_map(|message| message.get("id"))),
        "original IDs exactly once; no duplicate or push output: {replies:?}"
    );
    let caps = &replies[0]["result"]["capabilities"];
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["referencesProvider"], true);
    assert_eq!(caps["documentHighlightProvider"], true);
    let expected = [
        (
            json!("typed-definition"),
            json!({"uri":URI,"range":declaration}),
        ),
        (
            json!(u64::MAX),
            json!({"contents":{"kind":"plaintext","value":"val `x😀`: Long = 1"},"range":occurrence}),
        ),
        (
            json!("typed-references"),
            json!([{"uri":URI,"range":declaration},{"uri":URI,"range":occurrence}]),
        ),
        (
            json!("typed-highlights"),
            json!([{"range":declaration,"kind":3},{"range":occurrence,"kind":2}]),
        ),
        (
            json!("untyped-definition"),
            json!({"uri":URI,"range":{"start":{"line":5,"character":5},"end":{"line":5,"character":6}}}),
        ),
        (json!("pull"), json!({"kind":"full","items":[]})),
        (json!("stop"), Value::Null),
    ];
    for (id, payload) in expected {
        let reply = replies.iter().find(|reply| reply["id"] == id).unwrap();
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["result"], payload, "{reply}");
    }
}

#[test]
fn parameter_shadowing_and_native_providers_compose_over_paired_stdio() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    parameter_shadowing_exchange(&package.0);
}

fn parameter_shadowing_exchange(directory: &Path) -> Value {
    let source = "// 😀\r\nclass Int {}\r\nfun f(`p🦀`: Long, unused: Int,) {\r\n val a: `p🦀` = `p🦀`\r\n val `p🦀` = `p🦀`\r\n val b = `p🦀`\r\n}\r\nfun g(`p🦀`: Int) {\r\n val c = `p🦀`\r\n}\r\n";
    let at = |line, character| json!({"textDocument":{"uri":URI},"position":{"line":line,"character":character}});
    let refs = |line, character, include| json!({"textDocument":{"uri":URI},"position":{"line":line,"character":character},"context":{"includeDeclaration":include}});
    let range = |line, start, end| json!({"start":{"line":line,"character":start},"end":{"line":line,"character":end}});
    let parameter = range(2, 6, 11);
    let before = range(3, 16, 21);
    let self_initializer = range(4, 13, 18);
    let local = range(4, 5, 10);
    let after = range(5, 9, 14);
    let unused = range(2, 19, 25);
    let other = range(7, 6, 11);
    let other_use = range(8, 9, 14);
    let messages = [
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["plaintext"]},"diagnostic":{}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"parameter-definition","method":"textDocument/definition","params":at(4,14)}),
        json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/hover","params":at(3,17)}),
        json!({"jsonrpc":"2.0","id":"parameter-declaration-references","method":"textDocument/references","params":refs(2,7,true)}),
        json!({"jsonrpc":"2.0","id":"parameter-use-references","method":"textDocument/references","params":refs(3,17,false)}),
        json!({"jsonrpc":"2.0","id":"parameter-highlights","method":"textDocument/documentHighlight","params":at(4,14)}),
        json!({"jsonrpc":"2.0","id":"local-definition","method":"textDocument/definition","params":at(5,10)}),
        json!({"jsonrpc":"2.0","id":"local-references","method":"textDocument/references","params":refs(4,6,true)}),
        json!({"jsonrpc":"2.0","id":"local-highlights","method":"textDocument/documentHighlight","params":at(5,10)}),
        json!({"jsonrpc":"2.0","id":"unused-references","method":"textDocument/references","params":refs(2,20,false)}),
        json!({"jsonrpc":"2.0","id":"unused-declaration","method":"textDocument/references","params":refs(2,20,true)}),
        json!({"jsonrpc":"2.0","id":"unused-highlights","method":"textDocument/documentHighlight","params":at(2,20)}),
        json!({"jsonrpc":"2.0","id":"other-function","method":"textDocument/references","params":refs(8,10,true)}),
        json!({"jsonrpc":"2.0","id":"annotation-is-not-value","method":"textDocument/definition","params":at(3,9)}),
        json!({"jsonrpc":"2.0","id":"pull","method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = exchange_directory(directory, &messages);
    assert!(replies.iter().all(|reply| reply.get("id").is_some()));
    assert!(
        replies
            .iter()
            .map(|reply| &reply["id"])
            .eq(messages.iter().filter_map(|message| message.get("id"))),
        "original IDs exactly once; no duplicate or push output: {replies:?}"
    );
    let capabilities = &replies[0]["result"]["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    for provider in [
        "documentSymbolProvider",
        "definitionProvider",
        "hoverProvider",
        "referencesProvider",
        "documentHighlightProvider",
    ] {
        assert_eq!(capabilities[provider], true, "{capabilities}");
    }
    let expected = [
        (
            json!("parameter-definition"),
            json!({"uri":URI,"range":parameter}),
        ),
        (
            json!(u64::MAX),
            json!({"contents":{"kind":"plaintext","value":"`p🦀`: Long"},"range":before}),
        ),
        (
            json!("parameter-declaration-references"),
            json!([
                {"uri":URI,"range":parameter},{"uri":URI,"range":before},{"uri":URI,"range":self_initializer}
            ]),
        ),
        (
            json!("parameter-use-references"),
            json!([
                {"uri":URI,"range":before},{"uri":URI,"range":self_initializer}
            ]),
        ),
        (
            json!("parameter-highlights"),
            json!([
                {"range":parameter,"kind":3},{"range":before,"kind":2},{"range":self_initializer,"kind":2}
            ]),
        ),
        (json!("local-definition"), json!({"uri":URI,"range":local})),
        (
            json!("local-references"),
            json!([{"uri":URI,"range":local},{"uri":URI,"range":after}]),
        ),
        (
            json!("local-highlights"),
            json!([{"range":local,"kind":3},{"range":after,"kind":2}]),
        ),
        (json!("unused-references"), json!([])),
        (
            json!("unused-declaration"),
            json!([{"uri":URI,"range":unused}]),
        ),
        (
            json!("unused-highlights"),
            json!([{"range":unused,"kind":3}]),
        ),
        (
            json!("other-function"),
            json!([{"uri":URI,"range":other},{"uri":URI,"range":other_use}]),
        ),
        (json!("pull"), json!({"kind":"full","items":[]})),
        (json!("stop"), Value::Null),
    ];
    for (id, payload) in expected {
        let reply = replies.iter().find(|reply| reply["id"] == id).unwrap();
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["result"], payload, "{reply}");
    }
    let annotation = replies
        .iter()
        .find(|reply| reply["id"] == "annotation-is-not-value")
        .unwrap();
    assert_eq!(annotation["error"]["code"], -32803);
    assert!(annotation.get("result").is_none());
    json!({"requests":messages,"responses":replies,"exit_code":0,"stderr":""})
}

#[test]
#[ignore = "requires an explicitly admitted extracted native package; never part of default native tests"]
fn extracted_archive_smoke() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let directory = PathBuf::from(
        std::env::var_os("KTLSP_EXTRACTED_PACKAGE_DIR")
            .expect("KTLSP_EXTRACTED_PACKAGE_DIR must name the verified extracted package"),
    );
    assert!(
        directory.is_absolute() && directory.is_dir(),
        "{directory:?}"
    );
    let directory = directory.canonicalize().unwrap();
    let executable = directory.join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX));
    let worker = directory.join(format!(
        "ktlsp-parser-worker{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(executable.is_file() && worker.is_file());
    let provenance: Value =
        serde_json::from_slice(&std::fs::read(directory.join("provenance.json")).unwrap()).unwrap();
    let mut cli = Vec::new();
    for argument in ["--help", "--version"] {
        let output = Command::new(&executable)
            .arg(argument)
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .env_remove("JDK_HOME")
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.status);
        assert!(output.stderr.is_empty(), "{:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).unwrap();
        if argument == "--version" {
            assert_eq!(
                stdout.trim_end(),
                concat!("ktlsp ", env!("CARGO_PKG_VERSION"))
            );
        }
        cli.push(json!({"argument":argument,"stdout":stdout,"stderr":"","exit_code":output.status.code()}));
    }
    let wire = parameter_shadowing_exchange(&directory);
    assert!(directory.is_dir() && executable.is_file() && worker.is_file());
    let receipt = serde_json::to_string(&json!({
        "extracted_directory":directory,
        "server":executable,
        "worker":worker,
        "provenance":provenance,
        "environment":{"PATH":"","JAVA_HOME":null,"JDK_HOME":null},
        "cli":cli,
        "wire":wire,
        "borrowed_directory_retained":true
    }))
    .unwrap();
    assert!(
        receipt.len() <= 65536,
        "artifact smoke receipt exceeds 64 KiB"
    );
    // Captured permanently in the separate evidence ZIP, never inserted into its own TAR.
    println!("EXTRACTED_ARCHIVE_SMOKE_RECEIPT={receipt}");
}

#[test]
fn references_authenticated_targets_semantic_spelling_isolation_and_no_chasing() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "fun f() { val `x` = 1; val other = 2; val other = 3; val a = x; val b = `x`; val c = a }\nfun g() { val x = 4; val y = x }";
    let mut session = running(json!({}));
    open(&mut session, source);
    let declaration = source.find("`x`").unwrap();
    let uses = [
        source.find("= x").unwrap() + 2,
        source.find("= `x`").unwrap() + 2,
    ];
    let expected = json!([
        {"uri":URI,"range":references_range(source, declaration, 3)},
        {"uri":URI,"range":references_range(source, uses[0], 1)},
        {"uri":URI,"range":references_range(source, uses[1], 3)}
    ]);
    for offset in [declaration, uses[0], uses[1]] {
        assert_eq!(
            request(
                &mut session,
                "textDocument/references",
                references_params(source, offset, true)
            )["result"],
            expected
        );
    }
    let a = source.find("val a").unwrap() + 4;
    let a_use = source.find("= a").unwrap() + 2;
    assert_eq!(
        request(
            &mut session,
            "textDocument/references",
            references_params(source, a_use, true)
        )["result"],
        json!([{"uri":URI,"range":references_range(source,a,1)},{"uri":URI,"range":references_range(source,a_use,1)}])
    );
    let g_target = source.rfind("val x").unwrap() + 4;
    let g_use = source.rfind("= x").unwrap() + 2;
    assert_eq!(
        request(
            &mut session,
            "textDocument/documentHighlight",
            navigation_params(source, g_use)
        )["result"],
        json!([{"range":references_range(source,g_target,1),"kind":3},{"range":references_range(source,g_use,1),"kind":2}])
    );
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        assert_eq!(
            request(
                &mut session,
                method,
                references_params(source, source.find(" { ").unwrap() + 2, true)
            )["result"],
            Value::Null
        );
        for offset in [declaration + 3, uses[0] + 1, uses[1] + 3] {
            assert_eq!(
                request(
                    &mut session,
                    method,
                    references_params(source, offset, true)
                )["result"],
                Value::Null,
                "token end is exclusive"
            );
        }
    }
    assert!(session.close().1);
}

#[test]
fn references_invalid_params_unsupported_contexts_and_resource_fail_atomically() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    let source = "// 😀\r\nfun f() { val x = 1; val y = x }";
    open(&mut session, source);
    let offset = source.rfind("= x").unwrap() + 2;
    for context in [
        Value::Null,
        json!({}),
        json!({"includeDeclaration":null}),
        json!({"includeDeclaration":1}),
        json!({"includeDeclaration":"true"}),
        json!([]),
    ] {
        let mut params = navigation_params(source, offset);
        params["context"] = context;
        assert_eq!(
            request(&mut session, "textDocument/references", params)["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        request(
            &mut session,
            "textDocument/references",
            navigation_params(source, offset)
        )["error"]["code"],
        -32602
    );
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        for position in [
            Value::Null,
            json!({"line":1}),
            json!({"line":1,"character":"1"}),
            json!({"line":-1,"character":0}),
            json!({"line":0,"character":4}),
            json!({"line":100,"character":0}),
        ] {
            assert_eq!(
                request(
                    &mut session,
                    method,
                    json!({"textDocument":{"uri":URI},"position":position,"context":{"includeDeclaration":true}})
                )["error"]["code"],
                -32602
            );
        }
        for params in [
            json!({}),
            json!({"textDocument":null}),
            json!({"textDocument":{"uri":42}}),
            json!({"textDocument":{"uri":"relative"},"position":{"line":0,"character":0},"context":{"includeDeclaration":true}}),
            json!({"textDocument":{"uri":"untitled:closed"},"position":{"line":0,"character":0},"context":{"includeDeclaration":true}}),
        ] {
            assert_eq!(
                request(&mut session, method, params)["error"]["code"],
                -32602
            );
        }
    }
    for (i, (source, spelling)) in [
        ("fun f() { val x = 1; val x = 2; val y = x }", "x"),
        ("fun f() { val x = 1; val `x` = 2; val y = x }", "x"),
        ("fun f() { val y = x; val x = 1 }", "x;"),
        ("fun f() { val x = x }", "x }"),
        ("fun f() { val y = unknown }", "unknown"),
        ("fun f(vararg arg: Int) { val x = 1; val y = x }", "x"),
        ("fun f() { var x = 1; val y = x }", "x"),
        ("val x = 1; val y = x", "x"),
        ("fun f() { val x = 1; val y = x.member }", "x.member"),
        ("fun f() { val x = 1; val y = x() }", "x()"),
        ("fun f() { val x = 1; val y = x + 1 }", "x +"),
    ]
    .into_iter()
    .enumerate()
    {
        change(&mut session, i as i32 + 2, source);
        let offset = source.rfind(spelling).unwrap();
        for method in ["textDocument/references", "textDocument/documentHighlight"] {
            let response = request(
                &mut session,
                method,
                references_params(source, offset, true),
            );
            assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
            assert!(response.get("result").is_none());
        }
    }
    // Unsupported declaration anchors are errors too, never fabricated [].
    change(&mut session, 20, "val global = 1");
    assert_eq!(
        request(
            &mut session,
            "textDocument/references",
            references_params("val global = 1", 4, false)
        )["error"]["code"],
        -32803
    );
    let large = format!(
        "fun f() {{ val x = 1;{} }}",
        " ".repeat(ktlsp_parser::MAX_SOURCE_BYTES)
    );
    change(&mut session, 21, &large);
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        assert_eq!(
            request(&mut session, method, references_params(&large, 14, true))["error"]["code"],
            -32803
        );
    }
    for (i, (source, spelling)) in [
        ("fun f() { val x = 1; val `x` = 2 }", "x"),
        ("fun f(arg: Int = 1) { val x = 1 }", "arg"),
        ("class C { val member = 1 }", "member"),
    ]
    .into_iter()
    .enumerate()
    {
        change(&mut session, i as i32 + 22, source);
        let offset = source.find(spelling).unwrap();
        for method in ["textDocument/references", "textDocument/documentHighlight"] {
            let response = request(
                &mut session,
                method,
                references_params(source, offset, false),
            );
            assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
            assert!(response.get("result").is_none());
        }
    }
    let source = format!(
        "fun f() {{ val x = 1; {} }}",
        (0..200)
            .map(|i| format!("val use{i} = x; "))
            .collect::<String>()
    );
    change(&mut session, 30, &source);
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        let response = request(&mut session, method, references_params(&source, 14, true));
        assert_eq!(response["error"]["code"], -32803, "{response}");
        assert!(
            response.get("result").is_none(),
            "work refusal cannot return a partial set"
        );
    }
    assert!(session.close().1);
}

#[test]
fn references_revisions_reopen_process_failure_and_rendered_uri_budget() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    let first = "fun f() { val first = 1; val y = first }";
    open(&mut session, first);
    let last = "fun f() { val last = 1; val y = last }";
    change(&mut session, 2, last);
    change(&mut session, 2, first); // Rejected stale edit preserves current bytes.
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        let response = request(
            &mut session,
            method,
            references_params(last, last.rfind("last").unwrap(), true),
        );
        assert!(response["result"].is_array(), "{response}");
        assert_eq!(response["result"][0]["range"], span(last, "last"));
    }
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        assert_eq!(
            request(&mut session, method, references_params(last, 14, true))["error"]["code"],
            -32602
        );
    }
    open(&mut session, first);
    assert_eq!(
        request(
            &mut session,
            "textDocument/references",
            references_params(first, 14, true)
        )["result"][0]["range"],
        span(first, "first")
    );
    assert!(session.close().1);

    let mut failed = Session::with_parser(env!("CARGO_BIN_EXE_ktlsp")).unwrap();
    initialize(&mut failed, json!({}));
    open(&mut failed, first);
    for method in ["textDocument/references", "textDocument/documentHighlight"] {
        let response = request(&mut failed, method, references_params(first, 14, true));
        assert_eq!(response["error"]["code"], -32803, "{response}");
        assert!(response.get("result").is_none());
    }
    assert!(failed.close().1);

    let long_uri = format!("untitled:{}", "a".repeat(8000));
    let source = format!(
        "fun f() {{ val x = 1; {} }}",
        (0..24)
            .map(|i| format!("val use{i} = x; "))
            .collect::<String>()
    );
    let mut session = running(json!({}));
    open_uri(&mut session, &long_uri, &source);
    let mut params = references_params(&source, 14, true);
    params["textDocument"]["uri"] = json!(long_uri);
    let response = request(&mut session, "textDocument/references", params.clone());
    assert_eq!(response["error"]["code"], -32803, "{response}");
    assert!(response.get("result").is_none());
    // Highlight ranges do not repeat URIs and remain usable on the same snapshot.
    let response = request(&mut session, "textDocument/documentHighlight", params);
    assert_eq!(response["result"].as_array().unwrap().len(), 25);
    assert!(session.close().1);
}

#[test]
fn typed_local_original_definition_hover_and_annotation_refusals() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    let source = "// 😀\r\nclass Int {}\r\nfun f() {\r\n val `x 😀`: `Long` = 1\r\n val y: Int = `x 😀`\r\n val z = y\r\n}";
    open(&mut session, source);
    for (needle, declaration, value) in [
        ("`x 😀`", "`x 😀`", "val `x 😀`: `Long` = 1"),
        ("y", "y", "val y: Int = `x 😀`"),
    ] {
        let reference = source.rfind(needle).unwrap();
        let params = navigation_params(source, reference);
        let definition = request(&mut session, "textDocument/definition", params.clone());
        assert_eq!(
            definition["result"],
            json!({"uri":URI,"range":span(source, declaration)})
        );
        let hover = request(&mut session, "textDocument/hover", params);
        assert_eq!(
            hover["result"],
            json!({
                "contents":{"kind":"plaintext","value":value},
                "range":{
                    "start":offset_to_position(source, reference).unwrap(),
                    "end":offset_to_position(source, reference + needle.len()).unwrap()
                }
            })
        );
    }
    for annotation in ["`Long`", "Int ="] {
        let response = request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, source.find(annotation).unwrap()),
        );
        assert_eq!(response["error"]["code"], -32803, "{response}");
    }
    for (index, annotation) in [
        "pkg.Int",
        "Int?",
        "List<Int>",
        "@A Int",
        "(Int)",
        "() -> Int",
        "Int & Any",
        "dynamic",
        "",
        ": Int",
        "Int: Long",
        "= Int",
    ]
    .iter()
    .enumerate()
    {
        let source = format!("fun f() {{ val x: {annotation} = 1; val y = x }}");
        change(&mut session, index as i32 + 2, &source);
        let params = navigation_params(&source, source.rfind("= x").unwrap() + 2);
        for method in ["textDocument/definition", "textDocument/hover"] {
            let response = request(&mut session, method, params.clone());
            assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
            assert!(response.get("result").is_none());
        }
    }
}

fn rename_capabilities(prepare: bool) -> Value {
    json!({"workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"rename":{"prepareSupport":prepare},"diagnostic":{}}})
}
fn running_rename() -> Session {
    let mut session = Session::with_parser(worker()).unwrap();
    let response = request(
        &mut session,
        "initialize",
        json!({"capabilities":rename_capabilities(true)}),
    );
    assert_eq!(
        response["result"]["capabilities"]["renameProvider"],
        json!({"prepareProvider":true})
    );
    notify(&mut session, "initialized", json!({}));
    session
}
fn rename_params(source: &str, offset: usize, name: &str) -> Value {
    let mut params = navigation_params(source, offset);
    params["newName"] = json!(name);
    params
}
fn apply_original_rename(source: &str, response: &Value, uri: &str, version: i32) -> String {
    assert!(response.get("error").is_none(), "{response}");
    let changes = response["result"]["documentChanges"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0]["textDocument"],
        json!({"uri":uri,"version":version})
    );
    let edits = changes[0]["edits"].as_array().unwrap();
    let mut original = Vec::new();
    for edit in edits {
        let range: Range = serde_json::from_value(edit["range"].clone()).unwrap();
        let start = ktlsp_text::position_to_offset(source, range.start).unwrap();
        let end = ktlsp_text::position_to_offset(source, range.end).unwrap();
        assert!(start < end);
        assert!(
            original
                .last()
                .is_none_or(|(previous, _): &(std::ops::Range<usize>, &str)| previous.end <= start)
        );
        original.push((start..end, edit["newText"].as_str().unwrap()));
    }
    // Workspace edits use original coordinates; reverse order turns them into
    // the sequential native TextChange batch without inventing replacement text.
    let changes = edits
        .iter()
        .rev()
        .map(|edit| ktlsp_text::TextChange {
            range: Some(serde_json::from_value(edit["range"].clone()).unwrap()),
            range_length: None,
            text: edit["newText"].as_str().unwrap().to_owned(),
        })
        .collect::<Vec<_>>();
    let mut document = ktlsp_text::Document::new(version, source.to_owned()).unwrap();
    document.apply_changes(version + 1, &changes).unwrap();
    document.text().to_owned()
}

#[test]
fn local_val_rename_prepares_applies_and_rebinds_over_real_paired_stdio() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "// 🦀 x \"x\"\r\nfun f(p: T) {\r\n val `x🦀`: `x🦀` = 1\r\n val a = `x🦀`\r\n val b = a\r\n}\r\nfun g() { val `x🦀` = 2; val c = `x🦀` }\r\n";
    let declaration = source.find("val `x🦀`").unwrap() + 4;
    let read = source.find("= `x🦀`").unwrap() + 2;
    let range = |offset| references_range(source, offset, "`x🦀`".len());
    let messages = [
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":rename_capabilities(true)}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"prepare-declaration","method":"textDocument/prepareRename","params":navigation_params(source,declaration)}),
        json!({"jsonrpc":"2.0","id":"prepare-use","method":"textDocument/prepareRename","params":navigation_params(source,read)}),
        json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/rename","params":rename_params(source,read,"fresh")}),
        json!({"jsonrpc":"2.0","id":"rename-declaration","method":"textDocument/rename","params":rename_params(source,declaration,"fresh")}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert!(
        replies
            .iter()
            .map(|r| &r["id"])
            .eq(messages.iter().filter_map(|m| m.get("id")))
    );
    assert_eq!(
        replies[0]["result"]["capabilities"]["renameProvider"],
        json!({"prepareProvider":true})
    );
    assert_eq!(
        replies[1]["result"],
        json!({"range":range(declaration),"placeholder":"x🦀"})
    );
    assert_eq!(
        replies[2]["result"],
        json!({"range":range(read),"placeholder":"x🦀"})
    );
    let expected = json!({"documentChanges":[{
        "textDocument":{"uri":URI,"version":7},
        "edits":[{"range":range(declaration),"newText":"fresh"},{"range":range(read),"newText":"fresh"}]
    }]});
    assert_eq!(replies[3]["result"], expected);
    assert_eq!(replies[4]["result"], expected);
    let edited = apply_original_rename(source, &replies[3], URI, 7);
    assert_eq!(
        edited,
        "// 🦀 x \"x\"\r\nfun f(p: T) {\r\n val fresh: `x🦀` = 1\r\n val a = fresh\r\n val b = a\r\n}\r\nfun g() { val `x🦀` = 2; val c = `x🦀` }\r\n"
    );
    let declaration = edited.find("val fresh").unwrap() + 4;
    let read = edited.find("= fresh").unwrap() + 2;
    let replay = [
        messages[0].clone(),
        messages[1].clone(),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
        json!({"jsonrpc":"2.0","id":"rebound","method":"textDocument/references","params":references_params(&edited,read,true)}),
        json!({"jsonrpc":"2.0","id":"definition","method":"textDocument/definition","params":navigation_params(&edited,read)}),
        messages[7].clone(),
        messages[8].clone(),
    ];
    let rebound = package.exchange(&replay);
    assert!(
        rebound
            .iter()
            .map(|r| &r["id"])
            .eq(replay.iter().filter_map(|m| m.get("id")))
    );
    assert_eq!(
        rebound[1]["result"],
        json!([
            {"uri":URI,"range":references_range(&edited,declaration,5)},
            {"uri":URI,"range":references_range(&edited,read,5)}
        ])
    );
    assert_eq!(
        rebound[2]["result"],
        json!({"uri":URI,"range":references_range(&edited,declaration,5)})
    );
}

#[test]
fn local_rename_quote_equivalence_keeps_shadow_partitions_and_unused_edit() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    let source = "fun f(p: T) { val before = p; val `p` = p; val after = `p`; val unused = 1 }";
    open(&mut session, source);
    let declaration = source.find("`p`").unwrap();
    let read = source.rfind("`p`").unwrap();
    for (name, expected) in [
        (
            "p",
            "fun f(p: T) { val before = p; val p = p; val after = p; val unused = 1 }",
        ),
        (
            "fresh",
            "fun f(p: T) { val before = p; val fresh = p; val after = fresh; val unused = 1 }",
        ),
    ] {
        let renamed = request(
            &mut session,
            "textDocument/rename",
            rename_params(source, declaration, name),
        );
        assert_eq!(
            renamed["result"]["documentChanges"][0]["edits"],
            json!([
                {"range":references_range(source,declaration,3),"newText":name},
                {"range":references_range(source,read,3),"newText":name}
            ])
        );
        let edited = apply_original_rename(source, &renamed, URI, 1);
        assert_eq!(edited, expected);
        change(&mut session, 2, &edited);
        let renamed_read = edited.rfind(&format!("= {name}")).unwrap() + 2;
        let refs = request(
            &mut session,
            "textDocument/references",
            references_params(&edited, renamed_read, true),
        );
        assert_eq!(refs["result"].as_array().unwrap().len(), 2, "{refs}");
        // Restore a genuinely newer version before the next original-coordinate request.
        notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        );
        open(&mut session, source);
    }
    let unused = source.find("unused").unwrap();
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(source, unused, "alone"),
    );
    assert_eq!(
        response["result"]["documentChanges"][0]["edits"],
        json!([{"range":references_range(source,unused,6),"newText":"alone"}])
    );
    assert!(session.close().1);
}

#[test]
fn local_rename_rejects_conflicts_parameters_invalid_names_and_positions() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    let source = "// 😀\r\nfun f() { val x = 1; val a = x }";
    open(&mut session, source);
    let offset = source.find("val x").unwrap() + 4;
    for name in [
        Value::Null,
        json!(1),
        json!(true),
        json!(""),
        json!("__"),
        json!("1x"),
        json!("fun"),
        json!("a.b"),
        json!("a/b"),
        json!("a\\b"),
        json!("x\n"),
        json!("`x`"),
        json!("é"),
        json!("x😀"),
        json!("a\0"),
        json!("a-b"),
        json!("a".repeat(65537)),
    ] {
        let mut params = navigation_params(source, offset);
        params["newName"] = name;
        let response = request(&mut session, "textDocument/rename", params);
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(response.get("result").is_none());
    }
    let response = request(
        &mut session,
        "textDocument/rename",
        navigation_params(source, offset),
    );
    assert_eq!(response["error"]["code"], -32602);
    for method in ["textDocument/prepareRename", "textDocument/rename"] {
        for params in [
            json!({"textDocument":{"uri":URI},"position":{"line":0,"character":4},"newName":"fresh"}),
            json!({"textDocument":{"uri":URI},"position":{"line":200,"character":0},"newName":"fresh"}),
            json!({"textDocument":{"uri":URI},"position":{"line":1,"character":-1},"newName":"fresh"}),
            json!({"textDocument":{"uri":"untitled:closed"},"position":{"line":0,"character":0},"newName":"fresh"}),
            json!({"textDocument":{"uri":"relative"},"position":{"line":0,"character":0},"newName":"fresh"}),
        ] {
            let response = request(&mut session, method, params);
            assert_eq!(response["error"]["code"], -32602, "{response}");
            assert!(response.get("result").is_none());
        }
    }
    for (i, text) in [
        "fun f() { val x = 1; val y = 2; val a = x }",
        "fun f() { val y = 2; val x = 1; val a = x }",
        "fun f(y: T) { val x = 1; val a = x }",
        "fun f() { val x = 1; val a = y }",
        "fun f() { val x = 1; val a = `y` }",
        "fun f() { val x = 1; val a = x; val y = 2 }",
        "fun f() { val x = 1; val a = call() }",
        "fun f() { val x = 1; val x = 2 }",
        "fun f() { val x = 1; val a = x }\nval text = \"x\"",
    ]
    .iter()
    .enumerate()
    {
        change(&mut session, i as i32 + 2, text);
        let offset = text.find("val x").unwrap() + 4;
        let response = request(
            &mut session,
            "textDocument/rename",
            rename_params(text, offset, "y"),
        );
        assert_eq!(response["error"]["code"], -32803, "{text}: {response}");
        assert!(response.get("result").is_none());
    }
    let text = "fun f(p: T) { val a = p }\nfun caller() = f(p = 1)";
    change(&mut session, 20, text);
    for offset in [text.find("p:").unwrap(), text.find("= p").unwrap() + 2] {
        for method in ["textDocument/prepareRename", "textDocument/rename"] {
            let response = request(&mut session, method, rename_params(text, offset, "fresh"));
            assert_eq!(response["error"]["code"], -32803, "{response}");
            assert!(response.get("result").is_none());
        }
    }
    assert!(session.close().1);
}

#[test]
fn local_rename_capability_negotiation_is_strict_and_versioned_only() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for (profile, expected) in [
        (json!({}), Value::Null),
        (
            json!({"workspace":{"workspaceEdit":{"documentChanges":false}}}),
            Value::Null,
        ),
        (rename_capabilities(false), json!(true)),
        (rename_capabilities(true), json!({"prepareProvider":true})),
    ] {
        let mut session = Session::with_parser(worker()).unwrap();
        let initialized = request(&mut session, "initialize", json!({"capabilities":profile}));
        assert_eq!(
            initialized["result"]["capabilities"]["renameProvider"],
            expected
        );
        notify(&mut session, "initialized", json!({}));
        if expected.is_null() {
            for method in ["textDocument/prepareRename", "textDocument/rename"] {
                assert_eq!(
                    request(&mut session, method, json!({}))["error"]["code"],
                    -32601
                );
            }
        } else if expected == true {
            assert_eq!(
                request(&mut session, "textDocument/prepareRename", json!({}))["error"]["code"],
                -32601
            );
        }
        assert!(session.close().1);
    }
    for profile in [
        json!({"workspace":null}),
        json!({"workspace":{"workspaceEdit":[]}}),
        json!({"workspace":{"workspaceEdit":{"documentChanges":"true"}}}),
        json!({"textDocument":{"rename":null}}),
        json!({"textDocument":{"rename":{"prepareSupport":1}}}),
    ] {
        let mut session = Session::with_parser(worker()).unwrap();
        let response = request(&mut session, "initialize", json!({"capabilities":profile}));
        assert_eq!(response["error"]["code"], -32602);
        // A malformed profile must leave initialize retryable.
        assert_eq!(
            request(
                &mut session,
                "initialize",
                json!({"capabilities":rename_capabilities(true)})
            )["result"]["capabilities"]["renameProvider"],
            json!({"prepareProvider":true})
        );
        assert!(session.close().1);
    }
    let mut missing = Session::new();
    assert!(
        request(
            &mut missing,
            "initialize",
            json!({"capabilities":rename_capabilities(true)})
        )["result"]["capabilities"]
            .get("renameProvider")
            .is_none()
    );
    notify(&mut missing, "initialized", json!({}));
    assert_eq!(
        request(&mut missing, "textDocument/rename", json!({}))["error"]["code"],
        -32601
    );
}

#[test]
fn local_rename_growth_and_semantic_work_fail_without_partial_edits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    let base = "fun f() { val x = 1; val a = x }";
    open(&mut session, base);
    let offset = base.find("val x").unwrap() + 4;
    // Repeated newName would expand past 64 KiB despite fitting name validation
    // and the single semantic budget. The original document is never mutated.
    let name = "a".repeat(33000);
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(base, offset, &name),
    );
    assert_eq!(response["error"]["code"], -32803, "{response}");
    assert!(response.get("result").is_none());
    assert_eq!(session.document(URI).unwrap().text(), base);
    let name = "a".repeat(65536);
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(base, offset, &name),
    );
    assert_eq!(response["error"]["code"], -32803, "{response}");
    assert!(response.get("result").is_none());
    let oversized = format!("{base}{}", " ".repeat(65536));
    change(&mut session, 2, &oversized);
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(&oversized, offset, "fresh"),
    );
    assert_eq!(response["error"]["code"], -32803, "{response}");
    assert!(response.get("result").is_none());
    assert_eq!(
        session.pending_requests(),
        0,
        "oversized source is refused before admission"
    );
    assert!(session.close().1);
}

#[test]
fn local_rename_refuses_quote_removal_that_merges_keyword_and_identifier() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    let source = "fun f(){val`x`=1;val a=`x`}";
    open(&mut session, source);
    for offset in [source.find("`x`").unwrap(), source.rfind("`x`").unwrap()] {
        for method in ["textDocument/prepareRename", "textDocument/rename"] {
            for name in ["x", "fresh"] {
                let response = request(&mut session, method, rename_params(source, offset, name));
                assert_eq!(response["error"]["code"], -32803, "{response}");
                assert!(response.get("result").is_none());
                assert_eq!(session.document(URI).unwrap().text(), source);
            }
        }
    }
    assert!(session.close().1);
}

#[test]
fn declaration_name_hover_actual_worker_and_paired_cli() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let property = "val /*\r\n````\r\n*/ `same😀` = 1";
    let function = "fun same(`same😀`: Long) {\r\n val `same😀` = 1\r\n val read = `same😀`\r\n}";
    let source = format!("// 😀\r\nclass same {{}}\r\n{property}\r\n{function}\r\n");
    let parameter = source.find("`same😀`: Long").unwrap();
    let local = source.find("val `same😀`").unwrap();
    let cases = [
        (
            source.find("class same").unwrap() + 6,
            "same",
            "// 😀\r\nclass same {}",
        ),
        (
            source.find(property).unwrap() + property.find("`same😀`").unwrap(),
            "`same😀`",
            property,
        ),
        (source.find(function).unwrap() + 4, "same", function),
        (parameter, "`same😀`", "`same😀`: Long"),
        (local + 4, "`same😀`", "val `same😀` = 1"),
    ];
    let package = Package::new(true);
    for (profile, markdown) in [
        (json!({}), false),
        (
            json!({"contentFormat":["unknown","markdown","plaintext"]}),
            true,
        ),
        (json!({"contentFormat":["plaintext","markdown"]}), false),
    ] {
        let mut session = Session::with_parser(worker()).unwrap();
        let init = json!({"capabilities":{"textDocument":{"hover":profile,"diagnostic":{}}}});
        assert_eq!(
            request(&mut session, "initialize", init.clone())["result"]["capabilities"]["hoverProvider"],
            true
        );
        notify(&mut session, "initialized", json!({}));
        open(&mut session, &source);
        let mut messages = vec![
            json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":init}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        ];
        let mut expected = Vec::new();
        for (index, (start, name, declaration)) in cases.iter().enumerate() {
            let range = json!(Range {
                start: offset_to_position(&source, *start).unwrap(),
                end: offset_to_position(&source, start + name.len()).unwrap(),
            });
            let value = if markdown {
                let longest = declaration.split(|c| c != '`').map(str::len).max().unwrap();
                let fence = "`".repeat(3.max(longest + 1));
                format!("{fence}kotlin\n{declaration}\n{fence}")
            } else {
                declaration.to_string()
            };
            let result = json!({"contents":{"kind":if markdown {"markdown"} else {"plaintext"},"value":value},"range":range});
            // Both endpoints inside the actual token select the same declaration.
            for offset in [*start, start + name.len() - 1] {
                let params = navigation_params(&source, offset);
                let hover = request(&mut session, "textDocument/hover", params.clone());
                assert_eq!(hover["result"], result, "{hover}");
                let definition = request(&mut session, "textDocument/definition", params);
                assert_eq!(definition["result"], Value::Null, "{definition}");
            }
            let id = if index == 0 {
                json!(u64::MAX)
            } else {
                json!(format!("decl-{index}"))
            };
            messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":navigation_params(&source,*start)}));
            expected.push((id, result));
            let end = request(
                &mut session,
                "textDocument/hover",
                navigation_params(&source, start + name.len()),
            );
            assert_eq!(end["result"], Value::Null, "{end}");
        }
        for offset in [
            0,
            source.find("class").unwrap(),
            source.find('{').unwrap(),
            source.len(),
        ] {
            let response = request(
                &mut session,
                "textDocument/hover",
                navigation_params(&source, offset),
            );
            assert_eq!(response["result"], Value::Null, "{response}");
        }
        messages.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        assert!(
            replies
                .iter()
                .map(|reply| &reply["id"])
                .eq(messages.iter().filter_map(|message| message.get("id"))),
            "{replies:?}"
        );
        for (id, result) in expected {
            let reply = replies.iter().find(|reply| reply["id"] == id).unwrap();
            assert_eq!(reply["result"], result, "{reply}");
        }
    }
}

#[test]
fn declaration_hover_refusals_do_not_fall_back_to_enclosing_names() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    for (source, needle) in [
        ("fun f(p: Long) { val x = missing }", "missing"),
        ("fun f(p: Long) { val x: Long = p }", "Long"),
        ("fun f() { val x = call() }", "call"),
        ("fun f() { val broken = }", "broken"),
        ("fun f() { var mutable = 1 }", "mutable"),
    ] {
        open(&mut session, source);
        let reply = request(
            &mut session,
            "textDocument/hover",
            navigation_params(source, source.rfind(needle).unwrap()),
        );
        assert_eq!(reply["error"]["code"], -32803, "{source}: {reply}");
        assert!(reply.get("result").is_none());
        notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        );
    }
    // Non-name positions retain the cheap null path even for unsupported trees.
    let source = "fun f() { var mutable = call() }";
    open(&mut session, source);
    for offset in [0, source.find('{').unwrap(), source.len()] {
        let reply = request(
            &mut session,
            "textDocument/hover",
            navigation_params(source, offset),
        );
        assert_eq!(reply["result"], Value::Null, "{reply}");
    }
}

#[test]
fn declaration_hover_paired_lf_boundaries_negotiation_and_refusals() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "class Box {}\nfun f(p: Long) { val x = 1; val y = x }\n";
    let at = |offset| navigation_params(source, offset);
    let package = Package::new(true);
    for profile in [
        json!({}),
        json!({"contentFormat":["markdown"]}),
        json!({"contentFormat":[]}),
        json!({"contentFormat":[false,"plaintext"]}),
    ] {
        let enabled = profile.get("contentFormat").is_none()
            || profile["contentFormat"] == json!(["markdown"]);
        let mut messages = vec![
            json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile,"diagnostic":{}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"name","method":"textDocument/hover","params":at(6)}),
            json!({"jsonrpc":"2.0","id":"definition","method":"textDocument/definition","params":at(6)}),
        ];
        for (id, offset) in [
            ("end", 9),
            ("keyword", 0),
            ("space", 5),
            ("brace", 10),
            ("eof", source.len()),
        ] {
            messages.push(
                json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":at(offset)}),
            );
        }
        messages.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        assert!(
            replies
                .iter()
                .map(|reply| &reply["id"])
                .eq(messages.iter().filter_map(|message| message.get("id")))
        );
        assert_eq!(
            replies[0]["result"]["capabilities"]
                .get("hoverProvider")
                .is_some(),
            enabled
        );
        for reply in &replies[1..replies.len() - 1] {
            if reply["id"] == "definition" {
                assert_eq!(reply["result"], Value::Null, "{reply}");
            } else if !enabled {
                assert_eq!(reply["error"]["code"], -32601, "{reply}");
            } else if reply["id"] == "name" {
                let markdown = profile["contentFormat"] == json!(["markdown"]);
                assert_eq!(
                    reply["result"],
                    json!({"contents":{"kind":if markdown {"markdown"} else {"plaintext"},"value":if markdown {"```kotlin\nclass Box {}\n```"} else {"class Box {}"}},"range":{"start":{"line":0,"character":6},"end":{"line":0,"character":9}}}),
                    "{reply}"
                );
            } else {
                assert_eq!(reply["result"], Value::Null, "{reply}");
            }
        }
        let missing = Package::new(false).exchange(&messages);
        assert!(
            missing[0]["result"]["capabilities"]
                .get("hoverProvider")
                .is_none()
        );
        assert!(
            missing[1..missing.len() - 1]
                .iter()
                .all(|reply| reply["error"]["code"] == -32601)
        );
    }
    let source = "fun f(p: Long) { val x = missing }";
    let messages = [
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"diagnostic":{}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"refuse","method":"textDocument/hover","params":navigation_params(source,source.find("missing").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert_eq!(replies[1]["error"]["code"], -32803, "{replies:?}");
    assert!(replies[1].get("result").is_none());
}

fn apply_completion(source: &str, item: &Value, caret: usize) -> (String, usize) {
    let edit = &item["textEdit"];
    let range: Range = serde_json::from_value(edit["range"].clone()).unwrap();
    let start = position_to_offset(source, range.start).unwrap();
    let end = position_to_offset(source, range.end).unwrap();
    assert_eq!(range.start.line, range.end.line);
    assert!(start <= caret && caret <= end);
    let raw = item["label"].as_str().unwrap();
    assert_eq!(item["kind"], 6);
    assert_eq!(edit["newText"], raw);
    assert_eq!(item.as_object().unwrap().len(), 4);
    let mut applied = source.to_owned();
    applied.replace_range(start..end, raw);
    (applied, start)
}

#[test]
fn completion_real_items_replace_original_tokens_and_rebind_every_candidate() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // Unknown but complete tokens, including a quoted astral prefix and CRLF.
    // Every item is applied, reparsed and actually bound, not just counted.
    for (token, relative, expected) in [
        ("coSuffix", 2, vec!["count", "`counter`"]),
        ("co", 2, vec!["count", "`counter`"]),
        (
            "coSuffix",
            0,
            vec!["param", "count", "`counter`", "`😀target`"],
        ),
        ("`coSuffix`", 3, vec!["count", "`counter`"]),
        ("`co`", 4, vec!["count", "`counter`"]),
        ("`co`", 3, vec!["count", "`counter`"]),
        ("`co`", 1, vec!["param", "count", "`counter`", "`😀target`"]),
        ("`co`", 0, vec!["param", "count", "`counter`", "`😀target`"]),
        ("`😀suffix`", 5, vec!["`😀target`"]),
        ("Co", 2, vec![]),
        ("zzz", 3, vec![]),
    ] {
        let source = format!(
            "// 😀\r\nfun f(param: Long) {{\r\n val count: Int = 1\r\n val `counter` = 2\r\n val `😀target` = 3\r\n val chosen = {token}\r\n}}\r\nfun g() {{ val sibling = 4 }}\r\n"
        );
        let start = source.find(&format!("= {token}")).unwrap() + 2;
        let caret = start + relative;
        let mut session = running(json!({}));
        open(&mut session, &source);
        let response = request(
            &mut session,
            "textDocument/completion",
            navigation_params(&source, caret),
        );
        assert_eq!(response["result"]["isIncomplete"], true, "{response}");
        let items = response["result"]["items"].as_array().unwrap();
        assert_eq!(
            items
                .iter()
                .map(|item| item["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected,
            "{source}: {response}"
        );
        for (version, item) in (2..).zip(items) {
            let raw = item["label"].as_str().unwrap();
            assert_eq!(item["filterText"], raw.trim_matches('`'));
            assert_eq!(
                item["textEdit"]["range"],
                references_range(&source, start, token.len())
            );
            let (applied, use_start) = apply_completion(&source, item, caret);
            assert_eq!(&applied[use_start..use_start + raw.len()], raw);
            assert!(!applied.contains("Suffix") && !applied.contains("suffix"));
            change(&mut session, version, &applied);
            let bound = request(
                &mut session,
                "textDocument/definition",
                navigation_params(&applied, use_start),
            );
            assert_eq!(
                bound["result"],
                json!({"uri":URI,"range":span(&applied, raw)}),
                "{bound}"
            );
        }
        assert!(session.close().1);
    }
}

#[test]
fn completion_visibility_self_initializers_shadowing_and_future_ambiguity_are_atomic() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for (source, expected_target) in [
        ("fun f(`p`: Int) { val p = p; val after = p }", "`p`"),
        ("fun f(`p`: Int) { val p = 1; val after = pSuffix }", "p"),
        (
            "fun f() { val first = missing; val later = 1; val `later` = 2 }",
            "",
        ),
        ("fun f() { val own = own }", ""),
        (
            "fun g(other: Int) { val sibling = 1 }\nfun f() { val y = missing }",
            "",
        ),
    ] {
        let token = if source.contains("pSuffix") {
            "pSuffix"
        } else if source.contains("= p;") {
            "p"
        } else if source.contains("missing") {
            "missing"
        } else {
            "own"
        };
        let start = source.find(&format!("= {token}")).unwrap() + 2;
        let mut session = running(json!({}));
        open(&mut session, source);
        let response = request(
            &mut session,
            "textDocument/completion",
            navigation_params(source, start),
        );
        assert_eq!(response["result"]["isIncomplete"], true, "{response}");
        let items = response["result"]["items"].as_array().unwrap();
        if expected_target.is_empty() {
            assert!(items.is_empty(), "{response}");
        } else {
            assert_eq!(items.len(), 1, "{response}");
            let item = &items[0];
            assert_eq!(item["label"], expected_target);
            let (applied, use_start) = apply_completion(source, item, start);
            change(&mut session, 2, &applied);
            let bound = request(
                &mut session,
                "textDocument/definition",
                navigation_params(&applied, use_start),
            );
            let target_start = if expected_target == "p" {
                applied.find("val p").unwrap() + 4
            } else {
                applied.find(expected_target).unwrap()
            };
            assert_eq!(
                bound["result"],
                json!({"uri":URI,"range":references_range(&applied,target_start,expected_target.len())}),
                "{bound}"
            );
        }
        assert!(session.close().1);
    }
    let mut session = running(json!({}));
    open(&mut session, "");
    for (i, (source, needle)) in [
        ("fun f(p: Int, `p`: Int) { val y = unknown }", "unknown"),
        (
            "fun f() { val x = 1; val y = unknown; val `x` = 2 }",
            "unknown",
        ),
        (
            "fun f() { val a = 1; val x = 1; val x = 2; val y = unknown }",
            "unknown",
        ),
        ("fun f() { val x = 1; val y = }", "= }"),
        ("fun f() { val x = 1; val y = `co }", "`co"),
        ("fun f() { val x = 1; val y = co", "co"),
        ("fun f() { val x = 1; co }", "co"),
        ("fun f() { val x = 1; val y = co.member }", "co.member"),
        ("fun f() { val x = 1; val y = co() }", "co()"),
        ("val global = 1; val y = co", "co"),
        ("fun f() { val x = 1; val y: Type = co }", "Type"),
        ("fun f() { val café = 1; val y = co }", "co"),
        ("fun f() { val x = 1; val y = co; var later = 1 }", "co"),
        ("fun f(p: Int = 1) { val y = co }", "co"),
    ]
    .into_iter()
    .enumerate()
    {
        change(&mut session, i as i32 + 2, source);
        let response = request(
            &mut session,
            "textDocument/completion",
            navigation_params(source, source.rfind(needle).unwrap()),
        );
        assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
        assert!(response.get("result").is_none());
    }
    assert!(session.close().1);
}

#[test]
fn completion_params_resource_refusal_and_reopen_do_not_invent_empty_results() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running(json!({}));
    let source = "// 😀\r\nfun f() { val count = 1; val y = co }";
    open(&mut session, source);
    for params in [
        json!({}),
        json!({"textDocument":{"uri":"untitled:closed"},"position":{"line":0,"character":0}}),
        json!({"textDocument":{"uri":URI},"position":{"line":0,"character":4}}),
        json!({"textDocument":{"uri":URI},"position":{"line":99,"character":0}}),
        json!({"textDocument":{"uri":URI},"position":{"line":-1,"character":0}}),
        json!({"textDocument":{"uri":URI},"position":{"line":1,"character":"2"}}),
    ] {
        assert_eq!(
            request(&mut session, "textDocument/completion", params)["error"]["code"],
            -32602
        );
    }
    for (i, source) in [
        format!(
            "fun f() {{ val y = co; {} }}",
            " ".repeat(ktlsp_parser::MAX_SOURCE_BYTES)
        ),
        format!(
            "fun f() {{ {} val y = co }}",
            (0..200)
                .map(|i| format!("val value{i} = 1; "))
                .collect::<String>()
        ),
        format!("fun f() {{ val `{}` = 1; val y = co }}", "x".repeat(60_000)),
    ]
    .into_iter()
    .enumerate()
    {
        change(&mut session, i as i32 + 2, &source);
        let response = request(
            &mut session,
            "textDocument/completion",
            navigation_params(&source, source.rfind("co").unwrap()),
        );
        assert_eq!(response["error"]["code"], -32803, "{response}");
        assert!(response.get("result").is_none());
    }
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(
        request(
            &mut session,
            "textDocument/completion",
            navigation_params(source, source.rfind("co").unwrap())
        )["error"]["code"],
        -32602
    );
    let reopened = "fun f(fresh: Long) { val y = fr }";
    open(&mut session, reopened);
    let response = request(
        &mut session,
        "textDocument/completion",
        navigation_params(reopened, reopened.rfind("fr").unwrap() + 2),
    );
    assert_eq!(
        response["result"]["items"][0]["label"], "fresh",
        "{response}"
    );
    assert_eq!(response["result"]["items"].as_array().unwrap().len(), 1);
    assert!(session.close().1);
}

#[test]
fn completion_actual_paired_cli_application_rebinding_and_hover_independence() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "// 😀\r\nfun f(`count😀`: Long) {\r\n val chosen = `couSuffix`\r\n}\r\n";
    let start = source.find("`couSuffix`").unwrap();
    let caret = start + 4;
    let params = navigation_params(source, caret);
    let init = json!({"jsonrpc":"2.0","id":"initialize","method":"initialize","params":{"capabilities":{"general":{"positionEncodings":["utf-8"]},"textDocument":{"hover":{"contentFormat":false},"diagnostic":{}}}}});
    let messages = vec![
        init.clone(),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/completion","params":params}),
        json!({"jsonrpc":"2.0","id":"hover","method":"textDocument/hover","params":navigation_params(source, caret)}),
        json!({"jsonrpc":"2.0","id":"resolve","method":"completionItem/resolve","params":{}}),
        json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown","params":null}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let package = Package::new(true);
    let replies = package.exchange(&messages);
    assert_eq!(
        replies.len(),
        messages
            .iter()
            .filter(|message| message.get("id").is_some())
            .count(),
        "{replies:?}"
    );
    assert!(replies.iter().all(|reply| reply.get("method").is_none()));
    assert!(
        replies
            .iter()
            .filter_map(|reply| reply.get("id"))
            .eq(messages.iter().filter_map(|message| message.get("id")))
    );
    let caps = &replies[0]["result"]["capabilities"];
    assert_eq!(caps["completionProvider"], json!({"resolveProvider":false}));
    assert_eq!(caps["positionEncoding"], "utf-16");
    assert!(caps.get("hoverProvider").is_none());
    let response = &replies[1];
    assert_eq!(response["id"], json!(u64::MAX));
    assert_eq!(response["result"]["isIncomplete"], true, "{response}");
    assert_eq!(response["result"]["items"].as_array().unwrap().len(), 1);
    let item = &response["result"]["items"][0];
    assert_eq!(item["label"], "`count😀`");
    assert_eq!(item["filterText"], "count😀");
    assert_eq!(
        item["textEdit"]["range"],
        references_range(source, start, "`couSuffix`".len())
    );
    let (applied, use_start) = apply_completion(source, item, caret);
    assert_eq!(applied, source.replace("`couSuffix`", "`count😀`"));
    let bound_messages = [
        init,
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":applied}}}),
        json!({"jsonrpc":"2.0","id":"rebound","method":"textDocument/definition","params":navigation_params(&applied,use_start)}),
        json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown","params":null}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let rebound = package.exchange(&bound_messages);
    assert_eq!(rebound.len(), 3, "{rebound:?}");
    assert_eq!(rebound[1]["id"], "rebound");
    assert_eq!(
        rebound[1]["result"],
        json!({"uri":URI,"range":span(&applied,"`count😀`")}),
        "{rebound:?}"
    );
    for reply in &replies[2..4] {
        assert_eq!(reply["error"]["code"], -32601);
    }
    let missing = Package::new(false).exchange(&messages);
    assert_eq!(missing.len(), replies.len());
    assert!(
        missing[0]["result"]["capabilities"]
            .get("completionProvider")
            .is_none()
    );
    assert_eq!(missing[1]["error"]["code"], -32601);
    assert_eq!(missing[1]["id"], json!(u64::MAX));
}

#[test]
fn lexical_generic_definition_preserves_all_eight_paired_cli_consumers_and_edits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // Source-derived expectations; producer parentage/ranges are not invented.
    let source = "// 😀\r\nimport elsewhere.T\r\nclass T {}\r\nfun <T, `U🦀`,> f(p: T) {\r\n val x: `U🦀` = p\r\n val use = x\r\n}\r\nfun <T> g(p: T) { val other = p }\r\n";
    let type_t = source.find("<T").unwrap() + 1;
    let type_u = source.find("`U🦀`").unwrap();
    let parameter = source.find("p: T").unwrap();
    let annotation_t = parameter + 3;
    let local = source.find("val x").unwrap() + 4;
    let annotation_u = source.find(": `U🦀`").unwrap() + 2;
    let parameter_use = source.find("= p").unwrap() + 2;
    let local_use = source.find("= x").unwrap() + 2;
    let other_type = source.rfind("<T").unwrap() + 1;
    let other_annotation = source.rfind("p: T").unwrap() + 3;
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true},"rename":{"prepareSupport":true},"hover":{"contentFormat":["plaintext"]},"diagnostic":{}}
    }}});
    let messages = [
        init.clone(),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"annotation-t","method":"textDocument/definition","params":navigation_params(source,annotation_t)}),
        json!({"jsonrpc":"2.0","id":"annotation-u","method":"textDocument/definition","params":navigation_params(source,annotation_u)}),
        json!({"jsonrpc":"2.0","id":"other-type","method":"textDocument/definition","params":navigation_params(source,other_annotation)}),
        json!({"jsonrpc":"2.0","id":"value-p","method":"textDocument/definition","params":navigation_params(source,parameter_use)}),
        json!({"jsonrpc":"2.0","id":"value-x","method":"textDocument/definition","params":navigation_params(source,local_use)}),
        json!({"jsonrpc":"2.0","id":"hover","method":"textDocument/hover","params":navigation_params(source,local_use)}),
        json!({"jsonrpc":"2.0","id":"references","method":"textDocument/references","params":references_params(source,local_use,true)}),
        json!({"jsonrpc":"2.0","id":"parameter-references","method":"textDocument/references","params":references_params(source,parameter,true)}),
        json!({"jsonrpc":"2.0","id":"highlights","method":"textDocument/documentHighlight","params":navigation_params(source,local_use)}),
        json!({"jsonrpc":"2.0","id":"symbols","method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":"completion","method":"textDocument/completion","params":navigation_params(source,local_use)}),
        json!({"jsonrpc":"2.0","id":"prepare","method":"textDocument/prepareRename","params":navigation_params(source,local_use)}),
        json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename","params":rename_params(source,local_use,"T")}),
        json!({"jsonrpc":"2.0","id":"type-hover","method":"textDocument/hover","params":navigation_params(source,annotation_t)}),
        json!({"jsonrpc":"2.0","id":"type-references","method":"textDocument/references","params":references_params(source,type_t,true)}),
        json!({"jsonrpc":"2.0","id":"type-prepare","method":"textDocument/prepareRename","params":navigation_params(source,type_t)}),
        json!({"jsonrpc":"2.0","id":"type-rename","method":"textDocument/rename","params":rename_params(source,annotation_t,"Fresh")}),
        json!({"jsonrpc":"2.0","id":"type-definition-method","method":"textDocument/typeDefinition","params":navigation_params(source,annotation_t)}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let package = Package::new(true);
    let replies = package.exchange(&messages);
    assert!(
        replies
            .iter()
            .filter_map(|r| r.get("id"))
            .eq(messages.iter().filter_map(|m| m.get("id"))),
        "{replies:?}"
    );
    let reply = |id: &str| replies.iter().find(|r| r["id"] == id).unwrap();
    let capabilities = &reply("init")["result"]["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert!(capabilities.get("typeDefinitionProvider").is_none());
    for (id, start, len) in [
        ("annotation-t", type_t, 1),
        ("annotation-u", type_u, "`U🦀`".len()),
        ("other-type", other_type, 1),
        ("value-p", parameter, 1),
        ("value-x", local, 1),
    ] {
        assert_eq!(
            reply(id)["result"],
            json!({"uri":URI,"range":references_range(source,start,len)}),
            "{id}: {:?}",
            reply(id)
        );
    }
    assert_eq!(
        reply("hover")["result"],
        json!({
            "contents":{"kind":"plaintext","value":"val x: `U🦀` = p"},
            "range":references_range(source,local_use,1),
        })
    );
    assert_eq!(
        reply("references")["result"],
        json!([
            {"uri":URI,"range":references_range(source,local,1)},
            {"uri":URI,"range":references_range(source,local_use,1)},
        ])
    );
    assert_eq!(
        reply("parameter-references")["result"],
        json!([
            {"uri":URI,"range":references_range(source,parameter,1)},
            {"uri":URI,"range":references_range(source,parameter_use,1)},
        ])
    );
    assert_eq!(
        reply("highlights")["result"],
        json!([
            {"range":references_range(source,local,1),"kind":3},
            {"range":references_range(source,local_use,1),"kind":2},
        ])
    );
    let symbols = reply("symbols")["result"].as_array().unwrap();
    assert_eq!(
        symbols
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["T", "f", "g"]
    );
    assert_eq!(
        symbols[1]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["x", "use"]
    );
    assert_eq!(
        symbols[1]["children"][0]["selectionRange"],
        references_range(source, local, 1)
    );
    assert_eq!(
        reply("prepare")["result"],
        json!({"range":references_range(source,local_use,1),"placeholder":"x"})
    );
    assert_eq!(reply("type-hover")["error"]["code"], -32803);
    assert!(reply("type-hover").get("result").is_none());
    assert_eq!(
        reply("type-references")["result"],
        json!([
            {"uri":URI,"range":references_range(source,type_t,1)},
            {"uri":URI,"range":references_range(source,annotation_t,1)},
        ])
    );
    assert_eq!(
        reply("type-prepare")["result"],
        json!({"range":references_range(source,type_t,1),"placeholder":"T"})
    );
    assert_eq!(
        apply_original_rename(source, reply("type-rename"), URI, 7),
        source
            .replacen("<T,", "<Fresh,", 1)
            .replacen("p: T", "p: Fresh", 1)
    );
    assert_eq!(reply("type-definition-method")["error"]["code"], -32601);
    let items = reply("completion")["result"]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["p", "x"]
    );
    let (completed, completed_use) = apply_completion(source, &items[0], local_use);
    assert_eq!(completed, source.replacen("= x", "= p", 1));
    let renamed = apply_original_rename(source, reply("rename"), URI, 7);
    assert_eq!(
        renamed,
        source
            .replacen("val x:", "val T:", 1)
            .replacen("= x", "= T", 1)
    );
    // Both real edits are applied and fed to the paired CLI. Type annotations,
    // the generic list and the unrelated function retain their original bytes.
    for (text, use_start, target, len) in [
        (&completed, completed_use, parameter, 1),
        (&renamed, renamed.find("= T").unwrap() + 2, local, 1),
    ] {
        let rebound_messages = [
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":text}}}),
            json!({"jsonrpc":"2.0","id":"rebound-value","method":"textDocument/definition","params":navigation_params(text,use_start)}),
            json!({"jsonrpc":"2.0","id":"preserved-type","method":"textDocument/definition","params":navigation_params(text,annotation_t)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let rebound = package.exchange(&rebound_messages);
        assert_eq!(rebound.len(), 4, "{rebound:?}");
        assert_eq!(rebound[1]["id"], "rebound-value");
        assert_eq!(
            rebound[1]["result"],
            json!({"uri":URI,"range":references_range(text,target,len)})
        );
        assert_eq!(rebound[2]["id"], "preserved-type");
        assert_eq!(
            rebound[2]["result"],
            json!({"uri":URI,"range":references_range(text,type_t,1)})
        );
    }
}

#[test]
fn generic_annotation_definition_is_lexical_unique_and_definition_only() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    for (source, accepted) in [
        (
            "import elsewhere.T\nimport elsewhere.X as T\nclass T {}\nfun <`T`> f(p: T) { val x: `T` = p }",
            true,
        ),
        ("fun <T, `T`> f(p: T) {}", false),
        ("fun <T, U, `T`> f(p: T) {}", false),
        ("class T {}\nfun <U> f(p: T) {}", false),
        ("fun <T> f() {}\nfun <U> g(p: T) {}", false),
        ("fun <T: Any> f(p: T) {}", false),
        ("fun <T> f(p: T): T {}", false),
    ] {
        open(&mut session, source);
        let annotation = source.find("p: T").unwrap() + 3;
        let response = request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, annotation),
        );
        if accepted {
            let target = source.find("`T`").unwrap();
            assert_eq!(
                response["result"],
                json!({"uri":URI,"range":references_range(source,target,3)})
            );
        } else {
            assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
            assert!(response.get("result").is_none());
        }
        notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        );
    }
    assert!(session.close().1);
}

#[test]
fn local_empty_call_definition_and_mixed_values_use_actual_paired_cli_edits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true},"rename":{"prepareSupport":true},"hover":{"contentFormat":["plaintext"]},"diagnostic":{}}
    }}});
    let minimal = "fun outer(){ fun target(){}; target() }";
    let messages = [
        init.clone(),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":minimal}}}),
        json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(minimal,minimal.rfind("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert_eq!(replies[1]["id"], "callee");
    assert_eq!(
        replies[1]["result"],
        json!({"uri":URI,"range":references_range(minimal,minimal.find("target").unwrap(),6)})
    );

    let source = "// 🦀\r\nimport elsewhere.target\r\nfun target() {}\r\nfun <T> outer(p: T){ fun `target`(){}; val x=1; target(); val y=x; val z=p; val unused=2 }\r\n";
    let target = source.find("`target`").unwrap();
    let callee = source.rfind("target()").unwrap();
    let parameter = source.find("p: T").unwrap();
    let annotation = parameter + 3;
    let type_parameter = source.find("<T").unwrap() + 1;
    let local = source.find("val x").unwrap() + 4;
    let value_use = source.find("=x").unwrap() + 1;
    let parameter_use = source.find("=p").unwrap() + 1;
    let messages = [
        init.clone(),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(source,callee)}),
        json!({"jsonrpc":"2.0","id":"annotation","method":"textDocument/definition","params":navigation_params(source,annotation)}),
        json!({"jsonrpc":"2.0","id":"value","method":"textDocument/definition","params":navigation_params(source,value_use)}),
        json!({"jsonrpc":"2.0","id":"parameter","method":"textDocument/definition","params":navigation_params(source,parameter_use)}),
        json!({"jsonrpc":"2.0","id":"hover","method":"textDocument/hover","params":navigation_params(source,value_use)}),
        json!({"jsonrpc":"2.0","id":"references","method":"textDocument/references","params":references_params(source,value_use,true)}),
        json!({"jsonrpc":"2.0","id":"highlights","method":"textDocument/documentHighlight","params":navigation_params(source,value_use)}),
        json!({"jsonrpc":"2.0","id":"symbols","method":"textDocument/documentSymbol","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":"completion","method":"textDocument/completion","params":navigation_params(source,value_use)}),
        json!({"jsonrpc":"2.0","id":"prepare","method":"textDocument/prepareRename","params":navigation_params(source,value_use)}),
        json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename","params":rename_params(source,value_use,"fresh")}),
        json!({"jsonrpc":"2.0","id":"conflict","method":"textDocument/rename","params":rename_params(source,value_use,"target")}),
        json!({"jsonrpc":"2.0","id":"call-references","method":"textDocument/references","params":references_params(source,callee,true)}),
        json!({"jsonrpc":"2.0","id":"call-hover","method":"textDocument/hover","params":navigation_params(source,callee)}),
        json!({"jsonrpc":"2.0","id":"call-rename","method":"textDocument/rename","params":rename_params(source,callee,"fresh")}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert!(
        replies
            .iter()
            .filter_map(|r| r.get("id"))
            .eq(messages.iter().filter_map(|r| r.get("id")))
    );
    let reply = |id: &str| replies.iter().find(|r| r["id"] == id).unwrap();
    for (id, start, len) in [
        ("callee", target, 8),
        ("annotation", type_parameter, 1),
        ("value", local, 1),
        ("parameter", parameter, 1),
    ] {
        assert_eq!(
            reply(id)["result"],
            json!({"uri":URI,"range":references_range(source,start,len)}),
            "{id}: {:?}",
            reply(id)
        );
    }
    assert_eq!(
        reply("hover")["result"],
        json!({"contents":{"kind":"plaintext","value":"val x=1"},"range":references_range(source,value_use,1)})
    );
    assert_eq!(
        reply("references")["result"],
        json!([
            {"uri":URI,"range":references_range(source,local,1)},
            {"uri":URI,"range":references_range(source,value_use,1)}
        ])
    );
    assert_eq!(
        reply("highlights")["result"],
        json!([
            {"range":references_range(source,local,1),"kind":3},
            {"range":references_range(source,value_use,1),"kind":2}
        ])
    );
    let symbols = reply("symbols")["result"].as_array().unwrap();
    assert_eq!(
        symbols
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["target", "outer"]
    );
    assert_eq!(
        symbols[1]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["`target`", "x", "y", "z", "unused"]
    );
    assert_eq!(
        symbols[1]["children"][0]["selectionRange"],
        references_range(source, target, 8)
    );
    assert_eq!(
        reply("prepare")["result"],
        json!({"range":references_range(source,value_use,1),"placeholder":"x"})
    );
    for id in ["conflict", "call-references", "call-hover", "call-rename"] {
        assert_eq!(reply(id)["error"]["code"], -32803, "{id}: {:?}", reply(id));
        assert!(reply(id).get("result").is_none());
    }
    let items = reply("completion")["result"]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["p", "x"]
    );
    let renamed = apply_original_rename(source, reply("rename"), URI, 7);
    assert_eq!(
        renamed,
        source
            .replace("val x=", "val fresh=")
            .replace("=x", "=fresh")
    );
    // Rebind changed completion/rename edits and preserve the original callee.
    let completion = items.iter().find(|item| item["label"] == "p").unwrap();
    let (completed, completed_offset) = apply_completion(source, completion, value_use);
    let renamed_offset = renamed.find("=fresh").unwrap() + 1;
    for (text, offset, value_target, len) in [
        (completed, completed_offset, parameter, 1),
        (renamed, renamed_offset, local, 5),
    ] {
        let messages = [
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":text}}}),
            json!({"jsonrpc":"2.0","id":"value","method":"textDocument/definition","params":navigation_params(&text,offset)}),
            json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(&text,text.rfind("target()").unwrap())}),
            json!({"jsonrpc":"2.0","id":"annotation","method":"textDocument/definition","params":navigation_params(&text,annotation)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let rebound = package.exchange(&messages);
        assert_eq!(
            rebound[1]["result"],
            json!({"uri":URI,"range":references_range(&text,value_target,len)})
        );
        assert_eq!(
            rebound[2]["result"],
            json!({"uri":URI,"range":references_range(&text,target,8)})
        );
        assert_eq!(
            rebound[3]["result"],
            json!({"uri":URI,"range":references_range(&text,type_parameter,1)})
        );
    }
}

#[test]
fn local_call_definitions_refuse_unproved_whole_bodies_on_actual_cli() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for source in [
        "fun outer(){ target(); fun target(){} }",
        "fun outer(){ fun target(){}; fun `target`(){}; target() }",
        "fun outer(){ fun target(){}; fun target(x: Int){}; target() }",
        "fun outer(){ fun target(){}; class target; target() }",
        "fun outer(target: Int){ fun target(){}; target() }",
        "fun outer(){ fun target(){}; val target=1; target() }",
        "fun outer(){ fun target(){ target() }; target() }",
        "fun outer(){ fun target(){}; target(1) }",
        "fun outer(){ fun target(){}; target()() }",
        "fun outer(){ fun target(){}; target<Int>() }",
        "fun outer(){ fun target(){}; target(); unknown() }",
    ] {
        let callee = if source.starts_with("fun outer(){ target()") {
            source.find("target").unwrap()
        } else {
            source.rfind("target").unwrap()
        };
        let messages = [
            json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(source,callee)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let reply = replies
            .iter()
            .find(|reply| reply.get("id").and_then(Value::as_str) == Some("callee"))
            .expect("callee response");
        assert_eq!(reply["error"]["code"], -32803, "{source}: {reply:?}");
        assert!(reply.get("result").is_none());
    }
}

#[test]
fn type_parameter_versioned_rename_actual_cli_rebinds_types_values_and_local_callees() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "// 😀\r\nimport elsewhere.T\r\nclass T {}\r\nfun <T, Spare> f(T:T,U:Int){ fun Fresh(){}; Fresh(); val before=T; val T:T=T; val after:T=T; val n:Int=U }\r\nfun <T> g(p:T){val other:T=p}\r\n";
    let declaration = source.find("<T").unwrap() + 1;
    let unused = source.find("Spare").unwrap();
    let annotations = [
        source.find("T:T").unwrap() + 2,
        source.find("val T:T").unwrap() + 6,
        source.find("after:T").unwrap() + 6,
    ];
    let parameter = source.find("T:T").unwrap();
    let local = source.find("val T:T").unwrap() + 4;
    let values = [
        source.find("before=T").unwrap() + 7,
        source.find("T:T=T").unwrap() + 4,
        source.find("after:T=T").unwrap() + 8,
        source.find("n:Int=U").unwrap() + 6,
    ];
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"rename":{"prepareSupport":true},"diagnostic":{}}
    }}});
    for name in ["Fresh", "U", "out", "reified"] {
        let mut messages = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
        ];
        for (i, offset) in std::iter::once(declaration).chain(annotations).enumerate() {
            messages.push(json!({"jsonrpc":"2.0","id":format!("refs-{i}"),"method":"textDocument/references","params":references_params(source,offset,true)}));
            messages.push(json!({"jsonrpc":"2.0","id":format!("prepare-{i}"),"method":"textDocument/prepareRename","params":navigation_params(source,offset)}));
            if i != 0 {
                messages.push(json!({"jsonrpc":"2.0","id":format!("type-{i}"),"method":"textDocument/definition","params":navigation_params(source,offset)}));
            }
        }
        for (id, offset, include) in [
            ("unused", unused, false),
            ("unused-declaration", unused, true),
            ("without-declaration", annotations[0], false),
        ] {
            messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/references","params":references_params(source,offset,include)}));
        }
        for (i, offset) in values.into_iter().enumerate() {
            messages.push(json!({"jsonrpc":"2.0","id":format!("value-{i}"),"method":"textDocument/definition","params":navigation_params(source,offset)}));
        }
        messages.extend([
            json!({"jsonrpc":"2.0","id":"highlights","method":"textDocument/documentHighlight","params":navigation_params(source,annotations[2])}),
            json!({"jsonrpc":"2.0","id":"unused-prepare","method":"textDocument/prepareRename","params":navigation_params(source,unused)}),
            json!({"jsonrpc":"2.0","id":"unused-rename","method":"textDocument/rename","params":rename_params(source,unused,"UnusedFresh")}),
            json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(source,source.rfind("Fresh()").unwrap())}),
            json!({"jsonrpc":"2.0","id":"capture","method":"textDocument/rename","params":rename_params(source,declaration,"Int")}),
            json!({"jsonrpc":"2.0","id":"unused-capture","method":"textDocument/rename","params":rename_params(source,unused,"Int")}),
            json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename","params":rename_params(source,annotations[1],name)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        assert!(
            replies
                .iter()
                .filter_map(|r| r.get("id"))
                .eq(messages.iter().filter_map(|r| r.get("id")))
        );
        let reply = |id: &str| replies.iter().find(|r| r["id"] == id).unwrap();
        assert_eq!(
            reply("init")["result"]["capabilities"]["positionEncoding"],
            "utf-16"
        );
        let ranges = std::iter::once(declaration)
            .chain(annotations)
            .map(|offset| references_range(source, offset, 1))
            .collect::<Vec<_>>();
        let locations = ranges
            .iter()
            .map(|range| json!({"uri":URI,"range":range}))
            .collect::<Vec<_>>();
        for (i, offset) in std::iter::once(declaration).chain(annotations).enumerate() {
            assert_eq!(reply(&format!("refs-{i}"))["result"], json!(locations));
            assert_eq!(
                reply(&format!("prepare-{i}"))["result"],
                json!({"range":references_range(source,offset,1),"placeholder":"T"})
            );
            if i != 0 {
                assert_eq!(
                    reply(&format!("type-{i}"))["result"],
                    json!({"uri":URI,"range":ranges[0]})
                );
            }
        }
        assert_eq!(reply("unused")["result"], json!([]));
        assert_eq!(
            reply("unused-declaration")["result"],
            json!([{"uri":URI,"range":references_range(source,unused,5)}])
        );
        assert_eq!(
            reply("unused-prepare")["result"],
            json!({"range":references_range(source,unused,5),"placeholder":"Spare"})
        );
        assert_eq!(
            reply("unused-rename")["result"],
            json!({"documentChanges":[{
                "textDocument":{"uri":URI,"version":7},
                "edits":[{"range":references_range(source,unused,5),"newText":"UnusedFresh"}]
            }]})
        );
        assert_eq!(
            reply("without-declaration")["result"],
            json!(&locations[1..])
        );
        assert_eq!(
            reply("highlights")["result"],
            json!(
                ranges
                    .iter()
                    .enumerate()
                    .map(|(i, range)| json!({"range":range,"kind":if i==0 {3} else {2}}))
                    .collect::<Vec<_>>()
            )
        );
        for (i, target) in [parameter, parameter, local, source.find("U:Int").unwrap()]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                reply(&format!("value-{i}"))["result"],
                json!({"uri":URI,"range":references_range(source,target,1)})
            );
        }
        assert_eq!(
            reply("callee")["result"],
            json!({"uri":URI,"range":references_range(source,source.find("Fresh()").unwrap(),5)})
        );
        for id in ["capture", "unused-capture"] {
            assert_eq!(reply(id)["error"]["code"], -32803);
            assert!(reply(id).get("result").is_none());
        }
        let edited = apply_original_rename(source, reply("rename"), URI, 7);
        assert_eq!(
            reply("rename")["result"]["documentChanges"][0]["edits"],
            json!(
                ranges
                    .iter()
                    .map(|range| json!({"range":range,"newText":name}))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            edited,
            source
                .replacen("<T,", &format!("<{name},"), 1)
                .replacen("T:T", &format!("T:{name}"), 1)
                .replacen("val T:T", &format!("val T:{name}"), 1)
                .replacen("after:T", &format!("after:{name}"), 1)
        );
        let mut replay = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
        ];
        let edited_annotations = [
            edited.find(&format!("T:{name}")).unwrap() + 2,
            edited.find(&format!("val T:{name}")).unwrap() + 6,
            edited.find(&format!("after:{name}")).unwrap() + 6,
        ];
        for (i, offset) in edited_annotations.into_iter().enumerate() {
            replay.push(json!({"jsonrpc":"2.0","id":format!("type-{i}"),"method":"textDocument/definition","params":navigation_params(&edited,offset)}));
        }
        let edited_values = [
            edited.find("before=T").unwrap() + 7,
            edited.find(&format!("val T:{name}=T")).unwrap() + format!("val T:{name}=").len(),
            edited.find(&format!("after:{name}=T")).unwrap() + format!("after:{name}=").len(),
            edited.find("n:Int=U").unwrap() + 6,
        ];
        for (i, offset) in edited_values.into_iter().enumerate() {
            replay.push(json!({"jsonrpc":"2.0","id":format!("value-{i}"),"method":"textDocument/definition","params":navigation_params(&edited,offset)}));
        }
        replay.extend([
            json!({"jsonrpc":"2.0","id":"callee","method":"textDocument/definition","params":navigation_params(&edited,edited.rfind("Fresh()").unwrap())}),
            json!({"jsonrpc":"2.0","id":"references","method":"textDocument/references","params":references_params(&edited,declaration,true)}),
            json!({"jsonrpc":"2.0","id":"rename-again","method":"textDocument/rename","params":rename_params(&edited,declaration,"Again")}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let rebound = package.exchange(&replay);
        assert!(
            rebound
                .iter()
                .filter_map(|r| r.get("id"))
                .eq(replay.iter().filter_map(|r| r.get("id")))
        );
        let reply = |id: &str| rebound.iter().find(|r| r["id"] == id).unwrap();
        for i in 0..3 {
            assert_eq!(
                reply(&format!("type-{i}"))["result"],
                json!({"uri":URI,"range":references_range(&edited,declaration,name.len())})
            );
        }
        for (i, target) in [
            edited.find(&format!("T:{name}")).unwrap(),
            edited.find(&format!("T:{name}")).unwrap(),
            edited.find(&format!("val T:{name}")).unwrap() + 4,
            edited.find("U:Int").unwrap(),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                reply(&format!("value-{i}"))["result"],
                json!({"uri":URI,"range":references_range(&edited,target,1)})
            );
        }
        assert_eq!(
            reply("callee")["result"],
            json!({"uri":URI,"range":references_range(&edited,edited.find("Fresh()").unwrap(),5)})
        );
        assert_eq!(reply("references")["result"], json!(std::iter::once(declaration).chain(edited_annotations)
            .map(|offset| json!({"uri":URI,"range":references_range(&edited,offset,name.len())})).collect::<Vec<_>>()));
        assert_eq!(
            reply("rename-again")["result"]["documentChanges"][0]["textDocument"],
            json!({"uri":URI,"version":8})
        );
    }
}

#[test]
fn type_parameter_wire_quote_context_capture_growth_negotiation_and_reopen() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let mut session = running_rename();
    for (source, name, accepted) in [
        (
            "// 😀\r\nfun <`🦀`, Spare> f(p:`🦀`){ val x:`🦀`=p }\r\n",
            "Fresh",
            true,
        ),
        ("fun <`T`> f(p:`T`){}", "T", true),
        ("fun <`where`> f(p:`where`){}", "where", false),
        ("fun <`dynamic`> f(p:`dynamic`){}", "dynamic", false),
        ("fun <`suspend`> f(p:`suspend`){}", "suspend", false),
        ("fun <T,U,`T`> f(p:T){}", "Fresh", false),
        ("fun <T> f(p:U){}", "U", false),
        ("fun <Spare> f(p:Int){}", "Int", false),
    ] {
        open(&mut session, source);
        let offset = source.find('<').unwrap() + 1;
        let reply = request(
            &mut session,
            "textDocument/rename",
            rename_params(source, offset, name),
        );
        if accepted {
            let edited = apply_original_rename(source, &reply, URI, 1);
            let token = if source.contains("🦀") {
                "`🦀`"
            } else {
                "`T`"
            };
            assert_eq!(edited, source.replace(token, name));
        } else {
            assert_eq!(reply["error"]["code"], -32803, "{reply}");
            assert!(reply.get("result").is_none());
        }
        notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        );
    }
    let source = "fun <T> f(p:T){}";
    open(&mut session, source);
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(source, 5, &"A".repeat(33000)),
    );
    assert_eq!(response["error"]["code"], -32803);
    assert!(response.get("result").is_none());
    assert_eq!(session.document(URI).unwrap().text(), source);
    change(&mut session, 2, source);
    change(&mut session, 2, "fun <Wrong> f(){}"); // rejected current-version change
    let prepared = request(
        &mut session,
        "textDocument/prepareRename",
        navigation_params(source, 5),
    );
    assert_eq!(
        prepared["result"],
        json!({"range":references_range(source,5,1),"placeholder":"T"})
    );
    let renamed = request(
        &mut session,
        "textDocument/rename",
        rename_params(source, 5, "Fresh"),
    );
    assert_eq!(
        renamed["result"]["documentChanges"][0]["textDocument"],
        json!({"uri":URI,"version":2})
    );
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    open(&mut session, source);
    let reopened = request(
        &mut session,
        "textDocument/rename",
        rename_params(source, 5, "Fresh"),
    );
    assert_eq!(
        reopened["result"]["documentChanges"][0]["textDocument"],
        json!({"uri":URI,"version":1})
    );
    assert!(session.close().1);
    // Existing provider negotiation precedes all semantic roles, not a type-only
    // unversioned fallback. A real worker is configured but never admitted here.
    let mut unnegotiated = running(json!({}));
    open(&mut unnegotiated, source);
    for method in ["textDocument/prepareRename", "textDocument/rename"] {
        assert_eq!(
            request(&mut unnegotiated, method, rename_params(source, 5, "Fresh"))["error"]["code"],
            -32601
        );
    }
    assert!(unnegotiated.close().1);
}

#[test]
fn type_role_queries_keep_atomic_cancel_stale_incarnation_and_output_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "fun <T> f(p:T){val x:T=p}";
    for method in [
        "textDocument/references",
        "textDocument/prepareRename",
        "textDocument/rename",
    ] {
        for action in ["cancel", "change", "reopen"] {
            let mut session = running_rename();
            open(&mut session, source);
            let mut params = references_params(source, 5, true);
            params["newName"] = json!("Fresh");
            assert!(
                session
                    .handle(
                        json!({"jsonrpc":"2.0","id":"type-pending","method":method,"params":params})
                    )
                    .is_empty()
            );
            let messages = match action {
                "cancel" => session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"type-pending"}})),
                "change" => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":source}]}})),
                _ => {
                    let replies = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
                    open(&mut session,source); // same bytes/version, different incarnation
                    replies
                }
            };
            assert_eq!(messages.len(), 1, "{method}/{action}: {messages:?}");
            assert_eq!(messages[0]["id"], "type-pending");
            assert_eq!(
                messages[0]["error"]["code"],
                if action == "cancel" { -32800 } else { -32801 }
            );
            assert!(messages[0].get("result").is_none());
            let (remaining, clean) = session.close();
            assert!(clean, "{method}/{action}");
            assert!(
                remaining
                    .iter()
                    .all(|message| message["id"] != "type-pending")
            );
        }
    }
    let long_uri = format!("untitled:{}", "a".repeat(8000));
    let source = format!(
        "fun <T> f(){{{}}}",
        (0..24)
            .map(|i| format!("val x{i}:T=1;"))
            .collect::<String>()
    );
    let mut session = running_rename();
    open_uri(&mut session, &long_uri, &source);
    let mut params = references_params(&source, 5, true);
    params["textDocument"]["uri"] = json!(long_uri);
    let response = request(&mut session, "textDocument/references", params.clone());
    assert_eq!(response["error"]["code"], -32803);
    assert!(response.get("result").is_none());
    let response = request(&mut session, "textDocument/documentHighlight", params);
    assert_eq!(response["result"].as_array().unwrap().len(), 25);
    assert!(session.close().1);
}

fn documentation_ranged_change(
    messages: &mut Vec<Value>,
    document: &mut ktlsp_text::Document,
    version: i32,
    bytes: std::ops::Range<usize>,
    replacement: &str,
) {
    let range = references_range(document.text(), bytes.start, bytes.end - bytes.start);
    let change = json!({"range":range,"text":replacement});
    // Apply the exact same UTF-16 ranged change to the real test-side document,
    // then transmit it to the packaged process rather than substituting opens.
    let native = ktlsp_text::TextChange {
        range: Some(serde_json::from_value(range).unwrap()),
        range_length: None,
        text: replacement.to_owned(),
    };
    document.apply_changes(version, &[native]).unwrap();
    messages.push(json!({
        "jsonrpc":"2.0","method":"textDocument/didChange",
        "params":{"textDocument":{"uri":URI,"version":version},"contentChanges":[change]}
    }));
}

fn documentation_expected(
    source: &str,
    name: &str,
    suffix: &str,
    prose: Option<&str>,
    markdown: bool,
) -> Value {
    let name_start = source.find(name).unwrap();
    let declaration = if markdown {
        let longest = suffix.split(|character| character != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat(3.max(longest + 1));
        format!("{fence}kotlin\n{suffix}\n{fence}")
    } else {
        suffix.to_owned()
    };
    let value = if let Some(prose) = prose {
        let escaped = if markdown {
            prose.chars().flat_map(|character| {
                let prefix = (character.is_ascii_punctuation()).then_some('\\');
                prefix.into_iter().chain(std::iter::once(character))
            }).collect::<String>()
        } else {
            prose.to_owned()
        };
        format!("{escaped}\n\n{declaration}")
    } else {
        declaration
    };
    json!({
        "contents":{"kind":if markdown {"markdown"} else {"plaintext"},"value":value},
        "range":references_range(source,name_start,name.len())
    })
}

#[test]
fn documentation_paired_cli_applies_utf16_doc_name_prefix_move_remove_and_reopen() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let initial = "// 😀\r\n/** old 😀e\u{301} */\r\npublic fun `old😀`() {}\r\n/** second */\r\nfun second() {}\r\n";
    let package = Package::new(true);
    for (profile, markdown) in [
        (json!({}), false),
        (json!({"contentFormat":["markdown","plaintext"]}), true),
        (json!({"contentFormat":["plaintext","markdown"]}), false),
    ] {
        let mut messages = vec![
            json!({"jsonrpc":"2.0","id":"TEMP397-init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":initial}}}),
        ];
        let mut expected = Vec::new();
        let mut query = |messages: &mut Vec<Value>, source: &str, id: &str, name: &str, suffix: &str, prose: Option<&str>| {
            messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":navigation_params(source,source.find(name).unwrap())}));
            expected.push((id.to_owned(), documentation_expected(source,name,suffix,prose,markdown)));
        };
        query(&mut messages,initial,"initial","`old😀`","\r\npublic fun `old😀`() {}",Some(" old 😀e\u{301} "));
        let mut document = ktlsp_text::Document::new(1, initial.to_owned()).unwrap();
        let start = document.text().find("old 😀").unwrap();
        documentation_ranged_change(&mut messages,&mut document,2,start..start+"old 😀e\u{301}".len(),"new 😀e\u{301}");
        query(&mut messages,document.text(),"doc-edited","`old😀`","\r\npublic fun `old😀`() {}",Some(" new 😀e\u{301} "));
        let start = document.text().find("`old😀`").unwrap();
        documentation_ranged_change(&mut messages,&mut document,3,start..start+"`old😀`".len(),"`renamed🦀`");
        let start = document.text().find("public ").unwrap();
        documentation_ranged_change(&mut messages,&mut document,4,start..start+7,"");
        query(&mut messages,document.text(),"name-prefix-edited","`renamed🦀`","\r\nfun `renamed🦀`() {}",Some(" new 😀e\u{301} "));
        let start = document.text().find("/** new").unwrap();
        let end = start + "/** new 😀e\u{301} */\r\n".len();
        let moved = document.text()[start..end].to_owned();
        documentation_ranged_change(&mut messages,&mut document,5,start..end,"");
        let start = document.text().find("/** second */\r\n").unwrap();
        documentation_ranged_change(&mut messages,&mut document,6,start..start+"/** second */\r\n".len(),&moved);
        // The old owner has no direct doc and retains the legacy whole-tree
        // refusal while another declaration still owns documentation.
        messages.push(json!({"jsonrpc":"2.0","id":"old-owner-docless","method":"textDocument/hover","params":navigation_params(document.text(),document.text().find("`renamed🦀`").unwrap())}));
        query(&mut messages,document.text(),"new-owner","second","\r\nfun second() {}",Some(" new 😀e\u{301} "));
        let start = document.text().find("/** new").unwrap();
        documentation_ranged_change(&mut messages,&mut document,7,start..start+moved.len(),"");
        query(&mut messages,document.text(),"removed","second","fun second() {}",None);
        query(&mut messages,document.text(),"removed-old-owner","`renamed🦀`","// 😀\r\nfun `renamed🦀`() {}",None);
        messages.push(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
        let reopened = format!("{}\r\n/** reopened */\r\nfun third() {{}}",document.text());
        messages.push(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":reopened}}}));
        query(&mut messages,&reopened,"reopened","third","\r\nfun third() {}",Some(" reopened "));
        messages.push(json!({"jsonrpc":"2.0","id":"reopened-old-owner","method":"textDocument/hover","params":navigation_params(&reopened,reopened.find("second").unwrap())}));
        messages.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        assert_eq!(replies[0]["result"]["capabilities"]["hoverProvider"],true);
        for (id, result) in expected {
            let response = replies.iter().find(|response| response["id"] == id).unwrap();
            assert_eq!(response["result"],result,"{id}: {response}");
        }
        for id in ["old-owner-docless", "reopened-old-owner"] {
            let response = replies.iter().find(|response| response["id"] == id).unwrap();
            assert_eq!(response["error"]["code"],-32803,"{id}: {response}");
            assert!(response.get("result").is_none());
        }
        let missing = Package::new(false).exchange(&messages);
        assert!(missing[0]["result"]["capabilities"].get("hoverProvider").is_none());
        assert!(missing.iter().filter(|response| response.get("id").is_some() && response["id"] != "TEMP397-init" && response["id"] != "stop").all(|response| response["error"]["code"] == -32601));
    }
}

#[test]
fn documentation_paired_cli_literal_security_dynamic_fences_and_semantic_refusals() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let prose = " <b>&amp; javascript:alert(1) command:go data:text/html file:C:\\tmp ";
    let source = "/** <b>&amp; javascript:alert(1) command:go data:text/html file:C:\\tmp */\nfun safe(){ val /* ```` */ x=1 }";
    let expected = " \\<b\\>\\&amp\\; javascript\\:alert\\(1\\) command\\:go data\\:text\\/html file\\:C\\:\\\\tmp \n\n`````kotlin\n\nfun safe(){ val /* ```` */ x=1 }\n`````";
    for (profile, markdown) in [
        (json!({"contentFormat":["markdown","plaintext"]}), true),
        (json!({"contentFormat":["plaintext","markdown"]}), false),
    ] {
        let messages = [
            json!({"jsonrpc":"2.0","id":"TEMP397-security","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/hover","params":navigation_params(source,source.find("safe").unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies.iter().find(|response| response["id"] == u64::MAX).unwrap();
        assert_eq!(response["result"]["contents"]["kind"],if markdown {"markdown"} else {"plaintext"});
        assert_eq!(response["result"]["contents"]["value"],if markdown {expected.to_owned()} else {format!("{prose}\n\n\nfun safe(){{ val /* ```` */ x=1 }}")});
        assert_eq!(response["result"]["range"],references_range(source,source.find("safe").unwrap(),4));
    }
    let cases = [
        ("/** [Other] */\nfun target(){}", "target"),
        ("/** [label](command:run) */\nfun target(){}", "target"),
        ("/** ![image](data:text/html,evil) */\nfun target(){}", "target"),
        ("/** [r]: ../relative */\nfun target(){}", "target"),
        ("/** `code` */\nfun target(){}", "target"),
        ("/** ```\ncode\n``` */\nfun target(){}", "target"),
        ("/** ~~~\ncode\n~~~ */\nfun target(){}", "target"),
        ("/** @param p docs */\nfun target(p:Long){}", "target"),
        ("/** text */\n@Anno fun target(){}", "target"),
        ("/** text */\npublic fun target(x: List<Long>){}", "target"),
        ("fun outer(){ /** text */ val x=1; val y=x }", "x }"),
        ("/** text */\nfun <T> target(p:T){}", "T)"),
        ("fun outer(){ /** text */ fun target(){}; target() }", "target() }"),
        ("/** text */\nval target=1\nfun f(){val x=target}", "target}"),
    ];
    for (source, needle) in cases {
        let messages = [
            json!({"jsonrpc":"2.0","id":"TEMP397-refusal","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"refuse","method":"textDocument/hover","params":navigation_params(source,source.rfind(needle).unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies.iter().find(|response| response["id"] == "refuse").unwrap();
        assert_eq!(response["error"]["code"],-32803,"{source}: {response}");
        assert!(response.get("result").is_none());
    }
}

#[test]
fn documentation_profiles_keep_disabled_clients_and_legacy_docless_fence_acceptance() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "/** attached */\nfun target(){}";
    for profile in [json!({"contentFormat":[]}),json!({"contentFormat":false}),json!({"contentFormat":[7]}),json!({"contentFormat":["unknown"]})] {
        let messages = [
            json!({"jsonrpc":"2.0","id":"TEMP397-disabled","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"disabled","method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        assert!(replies[0]["result"]["capabilities"].get("hoverProvider").is_none());
        assert_eq!(replies.iter().find(|response| response["id"] == "disabled").unwrap()["error"]["code"],-32601);
    }
    let source = format!("val /*{}*/ target=1","`".repeat(45000));
    let messages = [
        json!({"jsonrpc":"2.0","id":"TEMP397-docless","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"docless","method":"textDocument/hover","params":navigation_params(&source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    let response = replies.iter().find(|response| response["id"] == "docless").unwrap();
    let fence = "`".repeat(45001);
    assert_eq!(response["result"]["contents"]["value"],format!("{fence}kotlin\n{source}\n{fence}"));
    // This previously supported docless output is larger than the managed
    // documentation String ceiling and must not be sent through that new cap.
    assert!(response["result"]["contents"]["value"].as_str().unwrap().len() > 131072);
}

#[test]
fn documentation_eligibility_preserves_many_comment_docless_hover() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = format!("{}fun named() {{}}", "//x\n".repeat(3000));
    let name = source.find("named").unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":"TEMP397-comments","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"comments","method":"textDocument/hover","params":navigation_params(&source,name)}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    let response = replies.iter().find(|r| r["id"] == "comments").unwrap();
    assert_eq!(response["result"]["contents"]["kind"], "markdown");
    assert_eq!(response["result"]["contents"]["value"], format!("```kotlin\n{source}\n```"));
    assert_eq!(response["result"]["range"], references_range(&source,name,5));
}

#[test]
fn documentation_hover_keeps_cancel_edit_reopen_and_rejected_edit_publication_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "/** original 😀 */\r\nfun `target😀`() {}";
    for action in ["cancel","change","reopen","rejected-change"] {
        let mut session = running(json!({}));
        open(&mut session,source);
        let params = navigation_params(source,source.find("`target").unwrap());
        assert!(session.handle(json!({"jsonrpc":"2.0","id":"doc-pending","method":"textDocument/hover","params":params})).is_empty());
        let replies = match action {
            "cancel" => session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"doc-pending"}})),
            "change" => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":4},"end":{"line":0,"character":12}},"text":"replaced"}]}})),
            "reopen" => {
                let replies = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
                open(&mut session,source); // same URI/version/bytes, new incarnation
                replies
            }
            _ => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":14},"end":{"line":0,"character":15}},"text":"split surrogate"}]}})),
        };
        if action == "rejected-change" {
            assert!(replies.iter().all(|response| response["id"] != "doc-pending"));
            let deadline = Instant::now() + Duration::from_secs(15);
            let response = loop {
                let responses = session.poll();
                if let Some(response) = responses.into_iter().find(|response| response["id"] == "doc-pending") {
                    break response;
                }
                assert!(Instant::now() < deadline,"real documentation worker did not finish");
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(response["result"],documentation_expected(source,"`target😀`","\r\nfun `target😀`() {}",Some(" original 😀 "),false));
        } else {
            let pending: Vec<_> = replies.iter().filter(|response| response["id"] == "doc-pending").collect();
            assert_eq!(pending.len(),1,"{action}: {replies:?}");
            assert_eq!(pending[0]["error"]["code"],if action == "cancel" {-32800} else {-32801});
            assert!(pending[0].get("result").is_none());
        }
        let (remaining, clean) = session.close();
        assert!(clean,"{action}");
        assert!(remaining.iter().all(|response| response["id"] != "doc-pending"));
    }
}

#[test]
fn documentation_paired_cli_preserves_paragraphs_original_line_endings_and_name_boundaries() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for newline in ["\n","\r\n","\r"] {
        let doc = format!("/**{newline} * first 😀e\u{301}\t{newline} *{newline} * para{newline} */");
        let prose = format!("{newline}  first 😀e\u{301}\t{newline} {newline}  para{newline} ");
        let suffix = format!("{newline}{newline}// between{newline}public fun `same😀`() {{}}");
        let source = format!("/** earlier */{newline}{doc}{suffix}{newline}");
        let name = source.find("`same😀`").unwrap();
        let mut messages = vec![
            json!({"jsonrpc":"2.0","id":"TEMP397-lines","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"name","method":"textDocument/hover","params":navigation_params(&source,name)}),
            json!({"jsonrpc":"2.0","id":"name-end","method":"textDocument/hover","params":navigation_params(&source,name+"`same😀`".len())}),
            json!({"jsonrpc":"2.0","id":"body","method":"textDocument/hover","params":navigation_params(&source,source.find("{}").unwrap())}),
        ];
        messages.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        let response = replies.iter().find(|response| response["id"] == "name").unwrap();
        assert_eq!(response["result"],documentation_expected(&source,"`same😀`",&suffix,Some(&prose),true));
        for id in ["name-end","body"] {
            assert_eq!(replies.iter().find(|response| response["id"] == id).unwrap()["result"],Value::Null);
        }
        assert!(!response["result"]["contents"]["value"].as_str().unwrap().contains("earlier"));
    }
}

#[test]
fn documentation_paired_cli_complete_output_accounts_real_large_ids_without_losing_reply() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "/** literal */\nfun target(){}";
    let accepted_id = "a".repeat(100000);
    let refused_id = "\"".repeat(175000);
    let messages = [
        json!({"jsonrpc":"2.0","id":"TEMP397-id-output","method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":accepted_id,"method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":refused_id,"method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = Package::new(true).exchange(&messages);
    let accepted = replies.iter().find(|response| response["id"] == accepted_id).unwrap();
    assert_eq!(accepted["result"],documentation_expected(source,"target","\nfun target(){}",Some(" literal "),false));
    assert!(serde_json::to_vec(accepted).unwrap().len() + 32 <= 1024 * 1024);
    let refused = replies.iter().find(|response| response["id"] == refused_id).unwrap();
    assert_eq!(refused["error"]["code"],-32803);
    assert!(refused.get("result").is_none());
    assert_eq!(replies.iter().filter(|response| response["id"] == refused_id).count(),1);
}

#[test]
fn documentation_paired_cli_selects_original_class_property_parameter_and_nested_names() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for (source, name, suffix, prose) in [
        ("/** class prose */\nclass Same {}", "Same", "\nclass Same {}", " class prose "),
        ("/** property prose */\nval `same😀`=1", "`same😀`", "\nval `same😀`=1", " property prose "),
        ("fun f(/** parameter prose */ p:Long){}", "p:Long", " p:Long", " parameter prose "),
        ("fun same(){/** inner prose */ fun same(){}}", "same(){}", " fun same(){}", " inner prose "),
        ("fun f(){/** local prose */ val x=1;val y=x}", "x=1", " val x=1", " local prose "),
    ] {
        let start = source.rfind(name).unwrap();
        let name_len = if name == "p:Long" || name == "x=1" {1} else if name == "same(){}" {4} else {name.len()};
        let messages = [
            json!({"jsonrpc":"2.0","id":"TEMP397-kinds","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["plaintext"]}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"name","method":"textDocument/hover","params":navigation_params(source,start)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies.iter().find(|response| response["id"] == "name").unwrap();
        assert_eq!(response["result"],json!({
            "contents":{"kind":"plaintext","value":format!("{prose}\n\n{suffix}")},
            "range":references_range(source,start,name_len)
        }),"{source}: {response}");
    }
}
