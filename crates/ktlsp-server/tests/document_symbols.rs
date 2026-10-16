//! Real trusted-worker CST consumers. Build BOTH binaries before running this test.
//! No synthetic worker replies, parser bypass, or local #96 reliability claim.
use ktlsp_server::Session;
use ktlsp_text::{Range, offset_to_position};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Mutex,
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
    session
        .handle(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .remove(0)
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
        let mut child = Command::new(
            self.0
                .join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX)),
        )
        .env("PATH", "")
        .env_remove("JAVA_HOME")
        .env_remove("JDK_HOME")
        .current_dir(&self.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
        let mut input = Vec::new();
        for message in messages {
            ktlsp_protocol::write_frame(&mut input, &serde_json::to_vec(message).unwrap()).unwrap();
        }
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output.status);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut reader = std::io::Cursor::new(output.stdout);
        let mut responses = Vec::new();
        while let Some(frame) = ktlsp_protocol::read_frame(&mut reader).unwrap() {
            responses.push(serde_json::from_slice(&frame).unwrap());
        }
        responses
    }
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
    writeln!(
        std::io::stdout().lock(),
        "DOCUMENT_SYMBOL_STDIO_SMOKE {}",
        json!({"initialize": &packaged[0], "documentSymbol": symbols})
    )
    .unwrap();
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
