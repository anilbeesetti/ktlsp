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
// Shared with the signature-help owned-work fixture below. Its original
// deadline, polling cadence and mutation-before-poll ordering remain unchanged.
fn collect_owned(session: &mut Session) -> Vec<Value> {
    let mut responses = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while session.pending_requests() != 0 && Instant::now() < deadline {
        responses.extend(session.poll());
        if session.pending_requests() != 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    responses
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
    fn source_only() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "ktlsp-outline-package-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn new(include_worker: bool) -> Self {
        let package = Self::source_only();
        std::fs::copy(
            env!("CARGO_BIN_EXE_ktlsp"),
            package
                .0
                .join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
        if include_worker {
            std::fs::copy(worker(), package.0.join(worker().file_name().unwrap())).unwrap();
        }
        package
    }
    fn exchange(&self, messages: &[Value]) -> Vec<Value> {
        exchange_directory(&self.0, messages)
    }
}
// This directory is borrowed: only Package owns cleanup of its exclusive creation.
fn exchange_directory<'a>(
    directory: &Path,
    messages: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
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
    let mut responses: Vec<Value> = Vec::new();
    for message in messages {
        let payload = serde_json::to_vec(message).unwrap();
        ktlsp_protocol::write_frame(&mut input, &payload).unwrap();
        input.flush().unwrap();
        // Requests really finish before later edits/shutdown. Bulk-writing
        // the entire session would now legitimately cancel/stale symbols.
        if let Some(id) = message.get("id") {
            loop {
                let frame = ktlsp_protocol::read_frame(&mut reader).unwrap().unwrap();
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

fn source_model(roots: &[&str]) -> Value {
    json!({"modules":[{"name":"app","dependencies":[{"type":"moduleSource"}],
        "contentRoots":[{"path":"<WORKSPACE>/","sourceRoots":roots.iter().map(|root|
            json!({"path":format!("<WORKSPACE>/{root}"),"type":"kotlin-source"})).collect::<Vec<_>>()}]}],
        "libraries":[]})
}

fn source_project(package: &Package, name: &str, roots: &[&str]) -> PathBuf {
    let project = package.0.join(name);
    std::fs::create_dir(&project).unwrap();
    for root in roots {
        std::fs::create_dir_all(project.join(root)).unwrap();
    }
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(roots)).unwrap(),
    )
    .unwrap();
    std::fs::canonicalize(project).unwrap()
}

// Only our existing ASCII fixture directories; Unicode leaf names are literal
// escaped suffixes below, not a second implementation of the production codec.
fn source_directory_uri(directory: &Path) -> String {
    let native = directory.to_str().unwrap();
    let path = native
        .strip_prefix("\\\\?\\")
        .unwrap_or(native)
        .replace('\\', "/");
    assert!(path.is_ascii());
    let path = path.replace(' ', "%20");
    format!(
        "file://{}{path}",
        if path.starts_with('/') { "" } else { "/" }
    )
}

fn source_options(projects: &[&str]) -> Value {
    json!({"projects":projects.iter().map(|path| json!({"type":"json","path":path})).collect::<Vec<_>>()})
}

fn source_initialize(options: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "initializationOptions":options,"capabilities":{"textDocument":{
            "documentSymbol":{"hierarchicalDocumentSymbolSupport":true},"diagnostic":{}}}}})
}

fn source_symbols(id: i32, uri: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":uri}}})
}

fn source_reload(id: i32, options: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"intellij/reloadWorkspace","params":{"initializationOptions":options}})
}

fn source_reply(replies: &[Value], id: i32) -> &Value {
    let mut matching = replies.iter().filter(|reply| reply["id"] == id);
    let reply = matching
        .next()
        .unwrap_or_else(|| panic!("missing {id}: {replies:?}"));
    assert!(matching.next().is_none(), "duplicate {id}: {replies:?}");
    reply
}

fn assert_source_class(replies: &[Value], id: i32, source: &str, name: &str) {
    let reply = source_reply(replies, id);
    assert!(reply.get("error").is_none(), "{reply}");
    let symbols = reply["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(symbols.len(), 1, "{reply}");
    assert_eq!(symbols[0]["name"], name, "{reply}");
    assert_eq!(symbols[0]["kind"], 5, "{reply}");
    assert_eq!(
        symbols[0]["range"],
        span(source, source.trim_end()),
        "{reply}"
    );
    assert_eq!(symbols[0]["selectionRange"], span(source, name), "{reply}");
}

fn assert_source_refused(replies: &[Value], id: i32, code: i32) {
    let reply = source_reply(replies, id);
    assert_eq!(reply["error"]["code"], code, "{reply}");
    assert!(reply.get("result").is_none(), "{reply}");
}

#[test]
fn json_import_cancel_replies_once_reuses_ids_and_retains_owned_work() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "cancelled", &["src"]);
    std::fs::write(project.join("src/A.kt"), "class Candidate\n").unwrap();
    let project_uri = source_directory_uri(&project);
    let uri = format!("{project_uri}/src/A.kt");
    let mut init = source_initialize(&json!({}));
    init["params"]["workspaceFolders"] = json!([{"uri":project_uri,"name":"owned"}]);
    let messages = [
        init,
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{"event":{
            "removed":[{"uri":project_uri,"name":"owned"}],
            "added":[{"uri":format!("{project_uri}/future"),"name":"future"},{"uri":"relative","name":"invalid"}]
        }}}),
        source_reload(2, &source_options(&[&project_uri])),
        source_symbols(3, &uri),
        source_symbols(3, &uri),
        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":2}}),
        source_symbols(3, &uri),
    ];
    let mut session = Session::with_parser(worker()).unwrap();
    let replies = session.handle(messages[0].clone());
    assert!(replies[0].get("error").is_none(), "{replies:?}");
    session.handle(messages[1].clone());
    session.handle(messages[2].clone());
    // The invalid final addition must not remove the old folder or retain a prefix.
    assert_eq!(
        session.configuration().workspace_folders,
        vec![ktlsp_server::WorkspaceFolder {
            uri: project_uri,
            name: "owned".into(),
        }]
    );
    session.drain_errors();
    assert!(session.handle(messages[3].clone()).is_empty());
    assert!(session.handle(messages[4].clone()).is_empty());
    let duplicate = session.handle(messages[5].clone());
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert_eq!(duplicate[0]["id"], Value::Null);
    assert_eq!(duplicate[0]["error"]["code"], -32600);
    let cancelled = session.handle(messages[6].clone());
    assert_eq!(cancelled.len(), 2, "{cancelled:?}");
    assert_source_refused(&cancelled, 2, -32800);
    assert_source_refused(&cancelled, 3, -32800);
    // Completion may be ready, but the import remains owned until collected/joined.
    assert_eq!(session.pending_requests(), 1);
    let reused = session.handle(messages[7].clone());
    assert_eq!(reused.len(), 1, "{reused:?}");
    assert_source_refused(&reused, 3, -32803);
    let (late, clean) = session.close();
    assert!(clean, "cancelled native import did not prove owned closure");
    assert!(
        late.is_empty(),
        "cancelled requests published twice: {late:?}"
    );
    assert_eq!(session.pending_requests(), 0);
}

#[test]
fn packaged_json_initial_unopened_symbols_alias_overlay_edits_close_and_outside_refusal() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "project", &["src"]);
    let disk = "class Ω\r\n";
    let overlay = "class Overlay\r\n";
    let edited = "class OverlayEdited\r\n";
    std::fs::write(project.join("src/空 😀.kt"), disk).unwrap();
    std::fs::write(package.0.join("Outside.kt"), "class OutsideDisk\n").unwrap();
    let project_uri = source_directory_uri(&project);
    let uri = format!("{project_uri}/src/%E7%A9%BA%20%F0%9F%98%80.kt");
    let alias = uri.replace("%E7", "%e7");
    let outside = format!(
        "{}/Outside.kt",
        source_directory_uri(&std::fs::canonicalize(&package.0).unwrap())
    );
    let options = source_options(&[&project_uri]);
    let messages = [
        source_initialize(&options),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        // The first consumer is immediate, not a status/readiness probe.
        source_symbols(2, &uri),
        source_symbols(3, &alias),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":alias,"languageId":"kotlin","version":1,"text":overlay}}}),
        source_symbols(4, &uri),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":alias,"version":2},"contentChanges":[{"range":span(overlay,"Overlay"),"rangeLength":7,"text":"OverlayEdited"}]}}),
        source_symbols(5, &uri),
        source_symbols(6, &alias),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":alias}}}),
        source_symbols(7, &uri),
        source_symbols(8, &outside),
        source_symbols(9, "untitled:unopened"),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":outside,"languageId":"kotlin","version":1,"text":"class OutsideOverlay\n"}}}),
        source_symbols(10, &uri),
        source_symbols(11, &outside),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":outside}}}),
        source_symbols(12, &uri),
        json!({"jsonrpc":"2.0","id":13,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert_eq!(replies.len(), 13, "{replies:?}");
    assert_eq!(
        source_reply(&replies, 1)["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    for id in [2, 3, 7, 12] {
        assert_source_class(&replies, id, disk, "Ω");
    }
    assert_source_class(&replies, 4, overlay, "Overlay");
    for id in [5, 6] {
        assert_source_class(&replies, id, edited, "OverlayEdited");
    }
    for id in [8, 9, 10] {
        assert_source_refused(&replies, id, -32803);
    }
    assert_source_class(&replies, 11, "class OutsideOverlay\n", "OutsideOverlay");
    assert_eq!(
        std::fs::read_to_string(project.join("src/空 😀.kt")).unwrap(),
        disk
    );
    assert_eq!(source_reply(&replies, 13)["result"], Value::Null);
}

#[test]
fn packaged_json_disk_and_model_mutation_require_reload_to_successor_authority() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "project", &["src"]);
    std::fs::create_dir(project.join("next")).unwrap();
    let before = "class Before\n";
    let changed = "class DiskChanged\n";
    let successor = "class Successor\n";
    let added = "class Added\n";
    std::fs::write(project.join("src/A.kt"), before).unwrap();
    std::fs::write(project.join("next/B.kt"), successor).unwrap();
    let project_uri = source_directory_uri(&project);
    let old_uri = format!("{project_uri}/src/A.kt");
    let next_uri = format!("{project_uri}/next/B.kt");
    let added_uri = format!("{project_uri}/next/Added.kt");
    let options = source_options(&[&project_uri]);
    let mut init = source_initialize(&options);
    init["params"]["workspaceFolders"] = json!([{"uri":project_uri,"name":"owned"}]);
    let messages = [
        init,
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        source_symbols(2, &old_uri),
        source_symbols(3, &old_uri),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":old_uri,"languageId":"kotlin","version":1,"text":"class Unsaved\n"}}}),
        source_symbols(4, &old_uri),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":old_uri}}}),
        source_symbols(5, &old_uri),
        source_reload(6, &options),
        source_symbols(7, &old_uri),
        source_symbols(8, &old_uri),
        source_symbols(9, &next_uri),
        source_reload(10, &options),
        source_symbols(11, &old_uri),
        source_symbols(12, &next_uri),
        json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{"event":{
            "added":[],"removed":[{"uri":project_uri,"name":"owned"}]}}}),
        source_symbols(13, &next_uri),
        source_reload(14, &options),
        source_symbols(15, &next_uri),
        source_symbols(16, &added_uri),
        source_reload(17, &options),
        source_symbols(18, &added_uri),
        source_symbols(19, &added_uri),
        source_reload(20, &options),
        source_symbols(21, &added_uri),
        source_symbols(22, &next_uri),
        json!({"jsonrpc":"2.0","id":23,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = exchange_directory(
        &package.0,
        messages
            .iter()
            .inspect(|message| match message["id"].as_i64() {
                // Iterator advancement occurs only after the preceding request's
                // matching actual response; no sleeps, polling or fake acknowledgments.
                Some(3) => {
                    std::fs::write(project.join("src/A.kt"), changed).unwrap();
                }
                Some(8) => {
                    std::fs::write(
                        project.join("workspace.json"),
                        serde_json::to_vec(&source_model(&["next"])).unwrap(),
                    )
                    .unwrap();
                }
                Some(16) => {
                    std::fs::write(project.join("next/Added.kt"), added).unwrap();
                }
                Some(19) => {
                    std::fs::remove_file(project.join("next/Added.kt")).unwrap();
                }
                _ => {}
            }),
    );
    assert_eq!(replies.len(), 23, "{replies:?}");
    assert_source_class(&replies, 2, before, "Before");
    assert_source_class(&replies, 4, "class Unsaved\n", "Unsaved");
    for id in [3, 5, 8, 19] {
        assert_source_refused(&replies, id, -32801);
    }
    for id in [6, 10, 14, 17, 20] {
        let reply = source_reply(&replies, id);
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["result"], Value::Null);
    }
    assert_source_class(&replies, 7, changed, "DiskChanged");
    for id in [9, 11, 13, 16, 21] {
        assert_source_refused(&replies, id, -32803);
    }
    for id in [12, 15, 22] {
        assert_source_class(&replies, id, successor, "Successor");
    }
    assert_source_class(&replies, 18, added, "Added");
    assert_eq!(source_reply(&replies, 23)["result"], Value::Null);
}

#[test]
fn packaged_json_late_root_and_meaningful_unsupported_import_never_publish_prefix_or_history() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let late = source_project(&package, "late", &["first"]);
    std::fs::write(late.join("first/Prefix.kt"), "class Prefix\n").unwrap();
    std::fs::write(
        late.join("workspace.json"),
        serde_json::to_vec(&source_model(&["first", "missing-last"])).unwrap(),
    )
    .unwrap();
    let historical = source_project(&package, "historical", &["src"]);
    std::fs::write(historical.join("src/Old.kt"), "class Historical\n").unwrap();
    let candidate = source_project(&package, "candidate", &["src"]);
    std::fs::write(candidate.join("src/New.kt"), "class Candidate\n").unwrap();
    let unsupported = source_project(&package, "unsupported", &["src"]);
    let mut unsupported_model = source_model(&["src"]);
    unsupported_model["modules"][0]["externalProjectPath"] = json!("/meaningful/original");
    let unsupported_bytes = serde_json::to_vec(&unsupported_model).unwrap();
    std::fs::write(unsupported.join("workspace.json"), &unsupported_bytes).unwrap();
    let late_uri = source_directory_uri(&late);
    let historical_uri = source_directory_uri(&historical);
    let candidate_uri = source_directory_uri(&candidate);
    let unsupported_uri = source_directory_uri(&unsupported);
    let prefix = format!("{late_uri}/first/Prefix.kt");
    let old = format!("{historical_uri}/src/Old.kt");
    let new = format!("{candidate_uri}/src/New.kt");
    let late_options = source_options(&[&late_uri]);
    let historical_options = source_options(&[&historical_uri]);
    let messages = [
        source_initialize(&historical_options),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        source_symbols(2, &old),
        source_reload(3, &late_options),
        source_symbols(4, &prefix),
        source_symbols(5, &old),
        source_reload(6, &historical_options),
        source_symbols(7, &old),
        source_reload(8, &source_options(&[&candidate_uri, &unsupported_uri])),
        source_symbols(9, &new),
        source_symbols(10, &old),
        source_reload(11, &source_options(&[&candidate_uri])),
        source_symbols(12, &new),
        json!({"jsonrpc":"2.0","id":13,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    assert_eq!(replies.len(), 13, "{replies:?}");
    for id in [3, 4, 5, 8, 9, 10] {
        assert_source_refused(&replies, id, -32803);
    }
    for id in [6, 11] {
        let reply = source_reply(&replies, id);
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["result"], Value::Null);
    }
    for id in [2, 7] {
        assert_source_class(&replies, id, "class Historical\n", "Historical");
    }
    assert_source_class(&replies, 12, "class Candidate\n", "Candidate");
    assert_eq!(
        std::fs::read(unsupported.join("workspace.json")).unwrap(),
        unsupported_bytes
    );
    assert_eq!(source_reply(&replies, 13)["result"], Value::Null);
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

// Only the installed pair produces provider results here: no Session or CST oracle.
fn basic_extracted_exchange(directory: &Path) -> Value {
    let scratch = Package::source_only();
    let project = source_project(&scratch, "basic", &["src"]);
    assert!(!project.starts_with(directory));
    let disk = "class DiskOnly\r\n";
    let path = project.join("src/A.kt");
    std::fs::write(&path, disk).unwrap();
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/A.kt");
    let source =
        "// 😀\r\n// Native\r\nfun host() {\r\n val `old😀` = 1\r\n val use = `old😀`\r\n}\r\n";
    let mut wire = SignatureWire::in_directory(directory);
    let mut requests = Vec::new();
    let mut send = |id: Option<&str>, method: &str, params: Value| {
        let mut message = json!({"jsonrpc":"2.0","method":method,"params":params});
        if let Some(id) = id {
            message["id"] = json!(id);
        }
        requests.push(message.clone());
        wire.send(message)
    };
    let result = |response: Value, expected: Value| {
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"], expected, "{response}");
    };
    let document = json!({"textDocument":{"uri":uri}});
    let at = |text: &str, offset| json!({"textDocument":{"uri":uri},"position":offset_to_position(text,offset).unwrap()});
    let initialized = send(
        Some("init"),
        "initialize",
        json!({"initializationOptions":source_options(&[&root]),"capabilities":{
            "general":{"positionEncodings":["utf-8","utf-32"]},
            "workspace":{"symbol":{"symbolKind":{"valueSet":(1..=26).collect::<Vec<_>>()}},
                "workspaceEdit":{"documentChanges":true}},
            "textDocument":{"diagnostic":{},"hover":{"contentFormat":["plaintext"]},
                "documentSymbol":{"hierarchicalDocumentSymbolSupport":true},
                "rename":{"prepareSupport":true},"signatureHelp":{},
                "semanticTokens":{"requests":{"full":true,"range":true},"formats":["relative"],
                    "tokenTypes":["number","comment","keyword","operator","string"],"tokenModifiers":[]}}
        }}),
    );
    let caps = &initialized["result"]["capabilities"];
    assert_eq!(caps["positionEncoding"], "utf-16");
    assert_eq!(caps["textDocumentSync"]["change"], 2);
    for provider in [
        "foldingRangeProvider",
        "documentSymbolProvider",
        "workspaceSymbolProvider",
        "definitionProvider",
        "referencesProvider",
    ] {
        assert_eq!(caps[provider], true, "{caps}");
    }
    assert_eq!(caps["renameProvider"], json!({"prepareProvider":true}));
    assert_eq!(caps["completionProvider"], json!({"resolveProvider":false}));
    assert_eq!(caps["signatureHelpProvider"], json!({}));
    let options = &caps["semanticTokensProvider"];
    assert_eq!(options["full"], true);
    assert_eq!(options["range"], true);
    assert_eq!(options["legend"]["tokenModifiers"], json!([]));
    send(None, "initialized", json!({}));
    let disk_symbols = json!([{"name":"DiskOnly","kind":5,
        "location":{"uri":uri,"range":span(disk,"class DiskOnly")}}]);
    result(
        send(
            Some("disk"),
            "workspace/symbol",
            json!({"query":"DiskOnly"}),
        ),
        disk_symbols.clone(),
    );
    send(
        None,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}),
    );
    result(
        send(Some("valid"), "textDocument/diagnostic", document.clone()),
        json!({"kind":"full","items":[]}),
    );
    // CRLF and astral UTF-16 replacement must commit before fresh token results.
    let emoji_range = json!({"start":{"line":0,"character":3},"end":{"line":0,"character":5}});
    send(
        None,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[
            {"range":emoji_range,"rangeLength":2,"text":"x"},
            {"range":{"start":{"line":6,"character":0},"end":{"line":6,"character":0}},"text":")"}]}),
    );
    result(
        send(Some("broken"), "textDocument/diagnostic", document.clone()),
        json!({"kind":"full","items":[{"range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},
            "severity":1,"source":"ktlsp-lexical-structural","code":"structural",
            "message":"unexpected or mismatched closing delimiter"}]}),
    );
    send(
        None,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":3},"contentChanges":[
            {"range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"text":""}]}),
    );
    let corrected = source.replacen('😀', "x", 1);
    result(
        send(Some("cleared"), "textDocument/diagnostic", document.clone()),
        json!({"kind":"full","items":[]}),
    );
    result(
        send(Some("folds"), "textDocument/foldingRange", document.clone()),
        json!([
            {"startLine":0,"startCharacter":0,"endLine":1,"endCharacter":9,"kind":"comment"},
            {"startLine":2,"startCharacter":11,"endLine":5,"endCharacter":1,"kind":"region"}
        ]),
    );
    let outline = |text: &str, name: &str| {
        json!([{"name":"host","kind":12,"range":span(text,text.trim_end()),
        "selectionRange":span(text,"host"),"children":[
            {"name":name,"kind":7,"range":span(text,&format!("val {name} = 1")),
                "selectionRange":span(text,name),"children":[]},
            {"name":"use","kind":7,"range":span(text,&format!("val use = {name}")),
                "selectionRange":span(text,"use"),"children":[]}
        ]}])
    };
    result(
        send(
            Some("outline"),
            "textDocument/documentSymbol",
            document.clone(),
        ),
        outline(&corrected, "`old😀`"),
    );
    let decode = |response: Value| {
        assert!(response.get("error").is_none(), "{response}");
        assert!(response["result"].get("resultId").is_none());
        let data = response["result"]["data"].as_array().unwrap();
        assert_eq!(data.len() % 5, 0);
        let mut line = 0;
        let mut column = 0;
        data.as_chunks::<5>()
            .0
            .iter()
            .map(|token| {
                let delta = token[0].as_u64().unwrap();
                line += delta;
                column = if delta == 0 {
                    column + token[1].as_u64().unwrap()
                } else {
                    token[1].as_u64().unwrap()
                };
                assert_eq!(token[4], 0);
                json!([
                    line,
                    column,
                    token[2],
                    options["legend"]["tokenTypes"][token[3].as_u64().unwrap() as usize]
                ])
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        decode(send(
            Some("tokens"),
            "textDocument/semanticTokens/full",
            document.clone()
        )),
        vec![
            json!([0, 0, 4, "comment"]),
            json!([1, 0, 9, "comment"]),
            json!([2, 0, 3, "keyword"]),
            json!([3, 1, 3, "keyword"]),
            json!([3, 13, 1, "operator"]),
            json!([3, 15, 1, "number"]),
            json!([4, 1, 3, "keyword"]),
            json!([4, 9, 1, "operator"])
        ]
    );
    assert_eq!(
        decode(send(
            Some("clipped"),
            "textDocument/semanticTokens/range",
            json!({"textDocument":{"uri":uri},"range":{
                "start":{"line":0,"character":2},"end":{"line":1,"character":4}}})
        )),
        vec![json!([0, 2, 2, "comment"]), json!([1, 0, 4, "comment"])]
    );
    let workspace_local = |text: &str, name: &str| {
        json!([{"name":name,"kind":7,"containerName":"host",
            "location":{"uri":uri,"range":span(text,&format!("val {name} = 1"))}}])
    };
    result(
        send(Some("literal"), "workspace/symbol", json!({"query":"old"})),
        workspace_local(&corrected, "`old😀`"),
    );
    for (id, query) in [
        ("case", "Old"),
        ("absent", "NeverPresent"),
        ("overlay", "DiskOnly"),
    ] {
        result(
            send(Some(id), "workspace/symbol", json!({"query":query})),
            json!([]),
        );
    }
    let use_start = corrected.rfind("`old😀`").unwrap();
    let use_range = references_range(&corrected, use_start, "`old😀`".len());
    send(
        None,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":4},"contentChanges":[
            {"range":use_range,"text":"`olSuffix`"}]}),
    );
    let incomplete = corrected.replacen("val use = `old😀`", "val use = `olSuffix`", 1);
    let caret = incomplete.find("`olSuffix`").unwrap() + 3;
    let completion = send(
        Some("complete"),
        "textDocument/completion",
        at(&incomplete, caret),
    );
    assert!(completion.get("error").is_none(), "{completion}");
    assert_eq!(completion["result"]["isIncomplete"], true);
    assert_eq!(completion["result"]["items"].as_array().unwrap().len(), 1);
    let item = &completion["result"]["items"][0];
    assert_eq!(item["label"], "`old😀`");
    assert_eq!(item["filterText"], "old😀");
    assert_eq!(item["textEdit"]["range"], span(&incomplete, "`olSuffix`"));
    let (completed, completed_use) = apply_completion(&incomplete, item, caret);
    assert_eq!(completed, corrected);
    send(
        None,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":5},"contentChanges":[
            {"range":item["textEdit"]["range"],"text":item["textEdit"]["newText"]}]}),
    );
    result(
        send(
            Some("rebound"),
            "textDocument/definition",
            at(&completed, completed_use),
        ),
        json!({"uri":uri,"range":span(&completed,"`old😀`")}),
    );
    let declaration = completed.find("`old😀`").unwrap();
    result(
        send(
            Some("prepare"),
            "textDocument/prepareRename",
            at(&completed, declaration),
        ),
        json!({"range":span(&completed,"`old😀`"),"placeholder":"old😀"}),
    );
    let mut params = at(&completed, declaration);
    params["newName"] = json!("Fresh");
    let rename = send(Some("rename"), "textDocument/rename", params);
    let renamed = apply_original_rename(&completed, &rename, &uri, 5);
    assert_eq!(renamed, completed.replace("`old😀`", "Fresh"));
    let edits = rename["result"]["documentChanges"][0]["edits"]
        .as_array()
        .unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["range"], span(&completed, "`old😀`"));
    assert_eq!(
        edits[1]["range"],
        references_range(&completed, completed_use, "`old😀`".len())
    );
    let changes = edits
        .iter()
        .rev()
        .map(|edit| json!({"range":edit["range"],"text":edit["newText"]}))
        .collect::<Vec<_>>();
    send(
        None,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":6},"contentChanges":changes}),
    );
    let fresh_use = renamed.rfind("Fresh").unwrap();
    result(
        send(
            Some("fresh-definition"),
            "textDocument/definition",
            at(&renamed, fresh_use),
        ),
        json!({"uri":uri,"range":span(&renamed,"Fresh")}),
    );
    let mut params = at(&renamed, fresh_use);
    params["context"] = json!({"includeDeclaration":true});
    result(
        send(Some("fresh-references"), "textDocument/references", params),
        json!([{"uri":uri,"range":span(&renamed,"Fresh")},
            {"uri":uri,"range":references_range(&renamed,fresh_use,"Fresh".len())}]),
    );
    result(
        send(
            Some("fresh-outline"),
            "textDocument/documentSymbol",
            document.clone(),
        ),
        outline(&renamed, "Fresh"),
    );
    result(
        send(Some("new"), "workspace/symbol", json!({"query":"Fresh"})),
        workspace_local(&renamed, "Fresh"),
    );
    for (id, query) in [("old", "old"), ("fresh-case", "fresh")] {
        result(
            send(Some(id), "workspace/symbol", json!({"query":query})),
            json!([]),
        );
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
    send(None, "textDocument/didClose", document.clone());
    result(
        send(
            Some("closed"),
            "workspace/symbol",
            json!({"query":"DiskOnly"}),
        ),
        disk_symbols,
    );
    result(
        send(
            Some("closed-overlay"),
            "workspace/symbol",
            json!({"query":"Fresh"}),
        ),
        json!([]),
    );
    let signature = "fun <T> outer(p: T) {\n fun send(to: T) {}\n send(to = p)\n}\n";
    send(
        None,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":signature}}),
    );
    let signature_response = send(
        Some("signature"),
        "textDocument/signatureHelp",
        navigation_params(signature, signature.rfind('=').unwrap() + 1),
    );
    result(
        signature_response,
        json!({"signatures":[{
            "label":"send(to: T)","parameters":[{"label":"to: T"}]}],
            "activeSignature":0,"activeParameter":0}),
    );
    let (responses, status, stderr) = wire.terminate(true);
    requests.push(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
    requests.push(json!({"jsonrpc":"2.0","method":"exit"}));
    assert!(
        responses.iter().map(|reply| reply.get("id")).eq(requests
            .iter()
            .filter_map(|message| message.get("id"))
            .map(Some)),
        "exact request order, no push diagnostics or extra output: {responses:?}"
    );
    let receipt = json!({"source_project":project,"workspace_model":source_model(&["src"]),
        "disk":disk,"corrected":corrected,"completion_input":incomplete,
        "completion_applied":completed,"rename_applied":renamed,
        "requests":requests,"responses":responses,"exit_code":status.code(),
        "stderr":String::from_utf8(stderr).unwrap()});
    drop(scratch);
    assert!(!project.exists(), "source-only scratch cleanup");
    receipt
}

fn extracted_lifecycle_exchange(directory: &Path, invalid_position: bool) -> Value {
    let mut wire = SignatureWire::in_directory(directory);
    let mut requests = vec![
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{
            "capabilities":{"textDocument":{"diagnostic":{}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
    ];
    if invalid_position {
        requests.extend([
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"// 😀\r\nfun f() {}\r\n"}}}),
            json!({"jsonrpc":"2.0","id":"split-surrogate","method":"textDocument/definition","params":{
                "textDocument":{"uri":URI},"position":{"line":0,"character":4}}}),
            json!({"jsonrpc":"2.0","id":"usable","method":"textDocument/diagnostic","params":{"textDocument":{"uri":URI}}}),
        ]);
    }
    for message in &requests {
        let response = wire.send(message.clone());
        match message["id"].as_str() {
            Some("split-surrogate") => {
                assert_eq!(response["error"]["code"], -32602, "{response}");
                assert!(response.get("result").is_none());
            }
            Some("usable") => assert_eq!(response["result"], json!({"kind":"full","items":[]})),
            Some("init") => assert_eq!(
                response["result"]["capabilities"]["positionEncoding"],
                "utf-16"
            ),
            _ => {}
        }
    }
    let (responses, status, stderr) = wire.terminate(invalid_position);
    requests.push(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
    if invalid_position {
        requests.push(json!({"jsonrpc":"2.0","method":"exit"}));
    }
    assert!(
        responses.iter().map(|reply| reply.get("id")).eq(requests
            .iter()
            .filter_map(|message| message.get("id"))
            .map(Some))
    );
    json!({"requests":requests,"responses":responses,"exit_code":status.code(),
        "stderr":String::from_utf8(stderr).unwrap()})
}

#[test]
fn packaged_basic_editing_same_live_edits_and_lifecycle() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // Ordinary hosted CI exercises the same wire assertions using copied real
    // binaries. This is not extracted-archive acceptance or archive provenance.
    let package = Package::new(true);
    let directory = package.0.clone();
    let server = directory.join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX));
    let worker = directory.join(format!(
        "ktlsp-parser-worker{}",
        std::env::consts::EXE_SUFFIX
    ));
    let basic = basic_extracted_exchange(&directory);
    let invalid_coordinates = extracted_lifecycle_exchange(&directory, true);
    let shutdown_eof = extracted_lifecycle_exchange(&directory, false);
    assert!(directory.is_dir() && server.is_file() && worker.is_file());
    let receipt = serde_json::to_string(&json!({
        "proof_kind":"copied-native-pair-not-extracted-archive",
        "directory":directory,"server":server,"worker":worker,
        "environment":{"PATH":"","JAVA_HOME":null,"JDK_HOME":null},
        "basic":basic,"invalid_coordinates":invalid_coordinates,"shutdown_eof":shutdown_eof,
        "borrowed_directory_retained_until_owner_drop":true
    }))
    .unwrap();
    assert!(
        receipt.len() <= 65536,
        "paired smoke receipt exceeds 64 KiB"
    );
    drop(package);
    assert!(!directory.exists(), "exclusive package owner cleanup");
    println!("PAIRED_BASIC_EDITING_SMOKE_RECEIPT={receipt}");
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
        assert_eq!(output.status.code(), Some(0), "{:?}", output.status);
        assert!(output.stderr.is_empty(), "{:?}", output.stderr);
        let stdout = String::from_utf8(output.stdout).unwrap();
        if argument == "--version" {
            assert_eq!(
                stdout.trim_end(),
                concat!("ktlsp ", env!("CARGO_PKG_VERSION"))
            );
        } else {
            for expected in [
                "native Kotlin language server",
                "Usage: ktlsp [--stdio | --help | --version]",
                "UTF-16 incremental document synchronization",
                "no JVM or JDK",
                "Protocol messages use stdout; errors use stderr.",
            ] {
                assert!(stdout.contains(expected), "{stdout}");
            }
        }
        cli.push(json!({"argument":argument,"stdout":stdout,"stderr":"","exit_code":output.status.code()}));
    }
    let wire = parameter_shadowing_exchange(&directory);
    let basic = basic_extracted_exchange(&directory);
    let invalid_coordinates = extracted_lifecycle_exchange(&directory, true);
    let shutdown_eof = extracted_lifecycle_exchange(&directory, false);
    assert!(directory.is_dir() && executable.is_file() && worker.is_file());
    let receipt = serde_json::to_string(&json!({
        "extracted_directory":directory,
        "server":executable,
        "worker":worker,
        "provenance":provenance,
        "environment":{"PATH":"","JAVA_HOME":null,"JDK_HOME":null},
        "cli":cli,
        "wire":wire,
        "basic":basic,
        "invalid_coordinates":invalid_coordinates,
        "shutdown_eof":shutdown_eof,
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
        ("fun <`T`> f(p: T): T {}", true),
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
fn local_annotation_type_namespace_actual_cli_all_anchors_edits_and_rebinding() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"rename":{"prepareSupport":true}}
    }}});
    for source in [
        "fun <T> outer(){ fun target(p:T){} }",
        "fun <T, Spare> outer(){ fun target(p:T){} }",
        "// 😀\r\nfun <T, Spare> outer(T:T){ fun before(p:T,q:Int){}; val x:T=T; unknown(noSuch=x,noSuch=2L); fun after(p:`T`){}; before(); val y=x }\r\n",
        "fun <Int, Spare> outer(){ fun before(p:Int){}; unknown(label=1); fun after(p:`Int`){} }",
        "// 😀\r\nfun <`🦀`, Spare> outer(){ fun before(p:`🦀`){}; fun after(p:`🦀`){} }\r\n",
    ] {
        let start = source.find('<').unwrap() + 1;
        let raw = &source[start..source.find([',', '>']).unwrap()];
        let declaration = start..start + raw.len();
        let annotations: Vec<_> = source
            .match_indices(&format!(":{raw}"))
            .map(|(i, _)| i + 1..i + 1 + raw.len())
            .chain(if raw == "T" {
                source
                    .match_indices(":`T`")
                    .map(|(i, _)| i + 1..i + 4)
                    .collect::<Vec<_>>()
            } else if raw == "Int" {
                source
                    .match_indices(":`Int`")
                    .map(|(i, _)| i + 1..i + 6)
                    .collect::<Vec<_>>()
            } else {
                vec![]
            })
            .collect();
        let mut annotations = annotations;
        annotations.sort_by_key(|r| r.start);
        let spare = source.find("Spare").map(|i| i..i + 5);
        let anchors: Vec<_> = std::iter::once(declaration.clone())
            .chain(annotations.clone())
            .chain(spare.clone())
            .collect();
        for name in ["Fresh", "out", "reified"] {
            let mut messages = vec![
                init.clone(),
                json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
                json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
            ];
            for (i, anchor) in anchors.iter().enumerate() {
                for (role, method, params) in [
                    (
                        "definition",
                        "textDocument/definition",
                        navigation_params(source, anchor.start),
                    ),
                    (
                        "refs",
                        "textDocument/references",
                        references_params(source, anchor.start, true),
                    ),
                    (
                        "uses",
                        "textDocument/references",
                        references_params(source, anchor.start, false),
                    ),
                    (
                        "highlights",
                        "textDocument/documentHighlight",
                        navigation_params(source, anchor.start),
                    ),
                    (
                        "prepare",
                        "textDocument/prepareRename",
                        navigation_params(source, anchor.start),
                    ),
                    (
                        "rename",
                        "textDocument/rename",
                        rename_params(source, anchor.start, name),
                    ),
                ] {
                    messages.push(json!({"jsonrpc":"2.0","id":format!("{role}-{i}"),"method":method,"params":params}));
                }
            }
            for (id, offset, new_name) in [
                ("capture", Some(declaration.start), "Int"),
                ("unused-capture", spare.as_ref().map(|r| r.start), "Int"),
                ("context", Some(annotations[0].start), "where"),
            ]
            .into_iter()
            .filter_map(|(id, offset, name)| offset.map(|offset| (id, offset, name)))
            {
                messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/rename","params":rename_params(source,offset,new_name)}));
            }
            if source.contains("unknown(") {
                for (id, offset) in [
                    ("callee", source.find("unknown(").unwrap()),
                    (
                        "value",
                        source
                            .find("noSuch=x")
                            .map(|i| i + 7)
                            .unwrap_or_else(|| source.find("label=1").unwrap() + 6),
                    ),
                ] {
                    messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition","params":navigation_params(source,offset)}));
                }
            }
            messages.extend([
                json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
                json!({"jsonrpc":"2.0","method":"exit"}),
            ]);
            let replies = package.exchange(&messages);
            let reply = |id: &str| replies.iter().find(|r| r["id"] == id).unwrap();
            assert_eq!(
                reply("init")["result"]["capabilities"]["positionEncoding"],
                "utf-16"
            );
            let full: Vec<_> = std::iter::once(declaration.clone())
                .chain(annotations.clone())
                .collect();
            for (i, anchor) in anchors.iter().enumerate() {
                let unused = spare.as_ref() == Some(anchor);
                let partition = if unused {
                    vec![anchor.clone()]
                } else {
                    full.clone()
                };
                let range = |r: &std::ops::Range<usize>| references_range(source, r.start, r.len());
                assert_eq!(
                    reply(&format!("refs-{i}"))["result"],
                    json!(
                        partition
                            .iter()
                            .map(|r| json!({"uri":URI,"range":range(r)}))
                            .collect::<Vec<_>>()
                    )
                );
                assert_eq!(
                    reply(&format!("uses-{i}"))["result"],
                    json!(
                        partition
                            .iter()
                            .skip(1)
                            .map(|r| json!({"uri":URI,"range":range(r)}))
                            .collect::<Vec<_>>()
                    )
                );
                assert_eq!(
                    reply(&format!("highlights-{i}"))["result"],
                    json!(
                        partition
                            .iter()
                            .enumerate()
                            .map(|(j, r)| json!({"range":range(r),"kind":if j==0 {3}else{2}}))
                            .collect::<Vec<_>>()
                    )
                );
                assert_eq!(
                    reply(&format!("prepare-{i}"))["result"],
                    json!({"range":range(anchor),"placeholder":if unused {"Spare"}else{raw.trim_matches('`')}})
                );
                if annotations.contains(anchor) {
                    assert_eq!(
                        reply(&format!("definition-{i}"))["result"],
                        json!({"uri":URI,"range":range(&declaration)})
                    );
                } else {
                    // Existing declaration-definition behavior remains null;
                    // declaration anchors are positive only for occurrence APIs.
                    assert_eq!(reply(&format!("definition-{i}"))["result"], Value::Null);
                }
                let rename = reply(&format!("rename-{i}"));
                let edited = apply_original_rename(source, rename, URI, 7);
                let mut replay = vec![
                    init.clone(),
                    json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
                    json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
                ];
                let edited_type = if unused { raw.trim_matches('`') } else { name };
                let edited_target = edited.find('<').unwrap() + 1;
                let edited_annotation_ranges: Vec<_> = edited
                    .match_indices(&format!(":{edited_type}"))
                    .map(|(j, _)| j + 1..j + 1 + edited_type.len())
                    .chain(if unused && !raw.starts_with('`') {
                        edited
                            .match_indices(&format!(":`{edited_type}`"))
                            .map(|(j, _)| j + 1..j + 3 + edited_type.len())
                            .collect()
                    } else {
                        vec![]
                    })
                    .collect();
                // For the unused-target edit, query the untouched original
                // annotations by their actual unchanged raw token spelling.
                let mut edited_annotation_ranges = if unused && raw.starts_with('`') {
                    edited
                        .match_indices(&format!(":{raw}"))
                        .map(|(j, _)| j + 1..j + 1 + raw.len())
                        .collect()
                } else {
                    edited_annotation_ranges
                };
                edited_annotation_ranges.sort_by_key(|r| r.start);
                assert_eq!(edited_annotation_ranges.len(), annotations.len());
                for (j, r) in edited_annotation_ranges.iter().enumerate() {
                    replay.push(json!({"jsonrpc":"2.0","id":format!("type-{j}"),"method":"textDocument/definition","params":navigation_params(&edited,r.start)}));
                }
                if unused {
                    replay.push(json!({"jsonrpc":"2.0","id":"unused-rebound","method":"textDocument/references",
                        "params":references_params(&edited,edited.find(name).unwrap(),true)}));
                }
                replay.extend([
                    json!({"jsonrpc":"2.0","id":"refs","method":"textDocument/references","params":references_params(&edited,edited_target,true)}),
                    json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
                    json!({"jsonrpc":"2.0","method":"exit"}),
                ]);
                let rebound = package.exchange(&replay);
                for j in 0..annotations.len() {
                    let actual = rebound
                        .iter()
                        .find(|r| r["id"] == format!("type-{j}"))
                        .unwrap();
                    assert_eq!(
                        actual["result"],
                        json!({"uri":URI,"range":references_range(&edited,edited_target,if unused {raw.len()}else{name.len()})})
                    );
                }
                let edited_target_len = if unused { raw.len() } else { name.len() };
                let expected_rebound: Vec<_> =
                    std::iter::once(edited_target..edited_target + edited_target_len)
                        .chain(edited_annotation_ranges.clone())
                        .map(|r| {
                            json!({
                                "uri":URI,"range":references_range(&edited,r.start,r.len())
                            })
                        })
                        .collect();
                assert_eq!(
                    rebound.iter().find(|r| r["id"] == "refs").unwrap()["result"],
                    json!(expected_rebound)
                );
                if unused {
                    assert_eq!(
                        rebound
                            .iter()
                            .find(|r| r["id"] == "unused-rebound")
                            .unwrap()["result"],
                        json!([{"uri":URI,"range":references_range(&edited,edited.find(name).unwrap(),name.len())}])
                    );
                }
            }
            assert_eq!(reply("context")["error"]["code"], -32803);
            if source.contains("q:Int") {
                for id in ["capture", "unused-capture"] {
                    assert_eq!(reply(id)["error"]["code"], -32803);
                    assert!(reply(id).get("result").is_none());
                }
            }
            if source.contains("unknown(") {
                assert_eq!(reply("callee")["error"]["code"], -32803);
            }
        }
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
        ("fun <`T`> f(){fun target(p:`T`){} }", "T", true),
        (
            "fun <`where`> f(){fun target(p:`where`){} }",
            "where",
            false,
        ),
        (
            "fun <`dynamic`> f(){fun target(p:`dynamic`){} }",
            "dynamic",
            false,
        ),
        (
            "fun <`suspend`> f(){fun target(p:`suspend`){} }",
            "suspend",
            false,
        ),
        (
            "fun <T, Spare> f(){fun target(p:T){};fun other(p:Int){} }",
            "Int",
            false,
        ),
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
    let rich = "fun <T> f(){ fun before(p:T){}; unknown(label=1); fun after(q:T){} }";
    open(&mut session, rich);
    let response = request(
        &mut session,
        "textDocument/rename",
        rename_params(rich, rich.find("p:T").unwrap() + 2, &"A".repeat(33000)),
    );
    assert_eq!(response["error"]["code"], -32803);
    assert!(response.get("result").is_none());
    assert_eq!(session.document(URI).unwrap().text(), rich);
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
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
    for (source, anchor) in [
        ("fun <T> f(p:T){val x:T=p}", 5),
        ("fun <T> f(){fun target(p:T){}; unknown(label=1)}", 25),
    ] {
        for method in [
            "textDocument/references",
            "textDocument/prepareRename",
            "textDocument/rename",
        ] {
            for action in ["cancel", "change", "reopen"] {
                let mut session = running_rename();
                open(&mut session, source);
                let mut params = references_params(source, anchor, true);
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
        let longest = suffix
            .split(|character| character != '`')
            .map(str::len)
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(3.max(longest + 1));
        format!("{fence}kotlin\n{suffix}\n{fence}")
    } else {
        suffix.to_owned()
    };
    let value = if let Some(prose) = prose {
        let escaped = if markdown {
            prose
                .chars()
                .flat_map(|character| {
                    let prefix = (character.is_ascii_punctuation()).then_some('\\');
                    prefix.into_iter().chain(std::iter::once(character))
                })
                .collect::<String>()
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
            json!({"jsonrpc":"2.0","id":"documentation-init","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":initial}}}),
        ];
        let mut expected = Vec::new();
        let mut query = |messages: &mut Vec<Value>,
                         source: &str,
                         id: &str,
                         name: &str,
                         suffix: &str,
                         prose: Option<&str>| {
            messages.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/hover","params":navigation_params(source,source.find(name).unwrap())}));
            expected.push((
                id.to_owned(),
                documentation_expected(source, name, suffix, prose, markdown),
            ));
        };
        query(
            &mut messages,
            initial,
            "initial",
            "`old😀`",
            "\r\npublic fun `old😀`() {}",
            Some(" old 😀e\u{301} "),
        );
        let mut document = ktlsp_text::Document::new(1, initial.to_owned()).unwrap();
        let start = document.text().find("old 😀").unwrap();
        documentation_ranged_change(
            &mut messages,
            &mut document,
            2,
            start..start + "old 😀e\u{301}".len(),
            "new 😀e\u{301}",
        );
        query(
            &mut messages,
            document.text(),
            "doc-edited",
            "`old😀`",
            "\r\npublic fun `old😀`() {}",
            Some(" new 😀e\u{301} "),
        );
        let start = document.text().find("`old😀`").unwrap();
        documentation_ranged_change(
            &mut messages,
            &mut document,
            3,
            start..start + "`old😀`".len(),
            "`renamed🦀`",
        );
        let start = document.text().find("public ").unwrap();
        documentation_ranged_change(&mut messages, &mut document, 4, start..start + 7, "");
        query(
            &mut messages,
            document.text(),
            "name-prefix-edited",
            "`renamed🦀`",
            "\r\nfun `renamed🦀`() {}",
            Some(" new 😀e\u{301} "),
        );
        let start = document.text().find("/** new").unwrap();
        let end = start + "/** new 😀e\u{301} */\r\n".len();
        let moved = document.text()[start..end].to_owned();
        documentation_ranged_change(&mut messages, &mut document, 5, start..end, "");
        let start = document.text().find("/** second */\r\n").unwrap();
        documentation_ranged_change(
            &mut messages,
            &mut document,
            6,
            start..start + "/** second */\r\n".len(),
            &moved,
        );
        // The old owner has no direct doc and retains the legacy whole-tree
        // refusal while another declaration still owns documentation.
        messages.push(json!({"jsonrpc":"2.0","id":"old-owner-docless","method":"textDocument/hover","params":navigation_params(document.text(),document.text().find("`renamed🦀`").unwrap())}));
        query(
            &mut messages,
            document.text(),
            "new-owner",
            "second",
            "\r\nfun second() {}",
            Some(" new 😀e\u{301} "),
        );
        let start = document.text().find("/** new").unwrap();
        documentation_ranged_change(
            &mut messages,
            &mut document,
            7,
            start..start + moved.len(),
            "",
        );
        query(
            &mut messages,
            document.text(),
            "removed",
            "second",
            "fun second() {}",
            None,
        );
        query(
            &mut messages,
            document.text(),
            "removed-old-owner",
            "`renamed🦀`",
            "// 😀\r\nfun `renamed🦀`() {}",
            None,
        );
        messages.push(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
        let reopened = format!("{}\r\n/** reopened */\r\nfun third() {{}}", document.text());
        messages.push(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":reopened}}}));
        query(
            &mut messages,
            &reopened,
            "reopened",
            "third",
            "\r\nfun third() {}",
            Some(" reopened "),
        );
        messages.push(json!({"jsonrpc":"2.0","id":"reopened-old-owner","method":"textDocument/hover","params":navigation_params(&reopened,reopened.find("second").unwrap())}));
        messages.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let replies = package.exchange(&messages);
        assert_eq!(replies[0]["result"]["capabilities"]["hoverProvider"], true);
        for (id, result) in expected {
            let response = replies
                .iter()
                .find(|response| response["id"] == id)
                .unwrap();
            assert_eq!(response["result"], result, "{id}: {response}");
        }
        for id in ["old-owner-docless", "reopened-old-owner"] {
            let response = replies
                .iter()
                .find(|response| response["id"] == id)
                .unwrap();
            assert_eq!(response["error"]["code"], -32803, "{id}: {response}");
            assert!(response.get("result").is_none());
        }
        let missing = Package::new(false).exchange(&messages);
        assert!(
            missing[0]["result"]["capabilities"]
                .get("hoverProvider")
                .is_none()
        );
        assert!(
            missing
                .iter()
                .filter(|response| response.get("id").is_some()
                    && response["id"] != "documentation-init"
                    && response["id"] != "stop")
                .all(|response| response["error"]["code"] == -32601)
        );
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
            json!({"jsonrpc":"2.0","id":"documentation-security","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":u64::MAX,"method":"textDocument/hover","params":navigation_params(source,source.find("safe").unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies
            .iter()
            .find(|response| response["id"] == u64::MAX)
            .unwrap();
        assert_eq!(
            response["result"]["contents"]["kind"],
            if markdown { "markdown" } else { "plaintext" }
        );
        assert_eq!(
            response["result"]["contents"]["value"],
            if markdown {
                expected.to_owned()
            } else {
                format!("{prose}\n\n\nfun safe(){{ val /* ```` */ x=1 }}")
            }
        );
        assert_eq!(
            response["result"]["range"],
            references_range(source, source.find("safe").unwrap(), 4)
        );
    }
    let cases = [
        ("/** [Other] */\nfun target(){}", "target"),
        ("/** [label](command:run) */\nfun target(){}", "target"),
        (
            "/** ![image](data:text/html,evil) */\nfun target(){}",
            "target",
        ),
        ("/** [r]: ../relative */\nfun target(){}", "target"),
        ("/** `code` */\nfun target(){}", "target"),
        ("/** ```\ncode\n``` */\nfun target(){}", "target"),
        ("/** ~~~\ncode\n~~~ */\nfun target(){}", "target"),
        ("/** @param p docs */\nfun target(p:Long){}", "target"),
        ("/** text */\n@Anno fun target(){}", "target"),
        ("/** text */\npublic fun target(x: List<Long>){}", "target"),
        ("fun outer(){ /** text */ val x=1; val y=x }", "x }"),
        ("/** text */\nfun <T> target(p:T){}", "T)"),
        (
            "fun outer(){ /** text */ fun target(){}; target() }",
            "target() }",
        ),
        (
            "/** text */\nval target=1\nfun f(){val x=target}",
            "target}",
        ),
    ];
    for (source, needle) in cases {
        let messages = [
            json!({"jsonrpc":"2.0","id":"documentation-refusal","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"refuse","method":"textDocument/hover","params":navigation_params(source,source.rfind(needle).unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies
            .iter()
            .find(|response| response["id"] == "refuse")
            .unwrap();
        assert_eq!(response["error"]["code"], -32803, "{source}: {response}");
        assert!(response.get("result").is_none());
    }
}

#[test]
fn documentation_profiles_keep_disabled_clients_and_legacy_docless_fence_acceptance() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "/** attached */\nfun target(){}";
    for profile in [
        json!({"contentFormat":[]}),
        json!({"contentFormat":false}),
        json!({"contentFormat":[7]}),
        json!({"contentFormat":["unknown"]}),
    ] {
        let messages = [
            json!({"jsonrpc":"2.0","id":"documentation-disabled","method":"initialize","params":{"capabilities":{"textDocument":{"hover":profile}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"disabled","method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        assert!(
            replies[0]["result"]["capabilities"]
                .get("hoverProvider")
                .is_none()
        );
        assert_eq!(
            replies
                .iter()
                .find(|response| response["id"] == "disabled")
                .unwrap()["error"]["code"],
            -32601
        );
    }
    let source = format!("val /*{}*/ target=1", "`".repeat(45000));
    let messages = [
        json!({"jsonrpc":"2.0","id":"documentation-docless","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"docless","method":"textDocument/hover","params":navigation_params(&source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    let response = replies
        .iter()
        .find(|response| response["id"] == "docless")
        .unwrap();
    let fence = "`".repeat(45001);
    assert_eq!(
        response["result"]["contents"]["value"],
        format!("{fence}kotlin\n{source}\n{fence}"),
        "complete docless hover response: {response}"
    );
    // This previously supported docless output is larger than the managed
    // documentation String ceiling and must not be sent through that new cap.
    assert!(
        response["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .len()
            > 131072
    );
}

#[test]
fn documentation_eligibility_preserves_many_comment_docless_hover() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = format!("{}fun named() {{}}", "//x\n".repeat(3000));
    let name = source.find("named").unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":"documentation-comments","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":"comments","method":"textDocument/hover","params":navigation_params(&source,name)}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = package.exchange(&messages);
    let response = replies.iter().find(|r| r["id"] == "comments").unwrap();
    assert_eq!(response["result"]["contents"]["kind"], "markdown");
    assert_eq!(
        response["result"]["contents"]["value"],
        format!("```kotlin\n{source}\n```")
    );
    assert_eq!(
        response["result"]["range"],
        references_range(&source, name, 5)
    );
}

#[test]
fn documentation_hover_keeps_cancel_edit_reopen_and_rejected_edit_publication_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "/** original 😀 */\r\nfun `target😀`() {}";
    for action in ["cancel", "change", "reopen", "rejected-change"] {
        let mut session = running(json!({}));
        open(&mut session, source);
        let params = navigation_params(source, source.find("`target").unwrap());
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
            assert!(
                replies
                    .iter()
                    .all(|response| response["id"] != "doc-pending")
            );
            let deadline = Instant::now() + Duration::from_secs(15);
            let response = loop {
                let responses = session.poll();
                if let Some(response) = responses
                    .into_iter()
                    .find(|response| response["id"] == "doc-pending")
                {
                    break response;
                }
                assert!(
                    Instant::now() < deadline,
                    "real documentation worker did not finish"
                );
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(
                response["result"],
                documentation_expected(
                    source,
                    "`target😀`",
                    "\r\nfun `target😀`() {}",
                    Some(" original 😀 "),
                    false
                )
            );
        } else {
            let pending: Vec<_> = replies
                .iter()
                .filter(|response| response["id"] == "doc-pending")
                .collect();
            assert_eq!(pending.len(), 1, "{action}: {replies:?}");
            assert_eq!(
                pending[0]["error"]["code"],
                if action == "cancel" { -32800 } else { -32801 }
            );
            assert!(pending[0].get("result").is_none());
        }
        let (remaining, clean) = session.close();
        assert!(clean, "{action}");
        assert!(
            remaining
                .iter()
                .all(|response| response["id"] != "doc-pending")
        );
    }
}

#[test]
fn documentation_paired_cli_preserves_paragraphs_original_line_endings_and_name_boundaries() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for newline in ["\n", "\r\n", "\r"] {
        let doc =
            format!("/**{newline} * first 😀e\u{301}\t{newline} *{newline} * para{newline} */");
        let prose = format!("{newline}  first 😀e\u{301}\t{newline} {newline}  para{newline} ");
        let suffix = format!("{newline}{newline}// between{newline}public fun `same😀`() {{}}");
        let source = format!("/** earlier */{newline}{doc}{suffix}{newline}");
        let name = source.find("`same😀`").unwrap();
        let mut messages = vec![
            json!({"jsonrpc":"2.0","id":"documentation-lines","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["markdown"]}}}}}),
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
        let response = replies
            .iter()
            .find(|response| response["id"] == "name")
            .unwrap();
        assert_eq!(
            response["result"],
            documentation_expected(&source, "`same😀`", &suffix, Some(&prose), true)
        );
        for id in ["name-end", "body"] {
            assert_eq!(
                replies
                    .iter()
                    .find(|response| response["id"] == id)
                    .unwrap()["result"],
                Value::Null
            );
        }
        assert!(
            !response["result"]["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("earlier")
        );
    }
}

#[test]
fn documentation_paired_cli_complete_output_accounts_real_large_ids_without_losing_reply() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "/** literal */\nfun target(){}";
    let accepted_id = "a".repeat(100000);
    let refused_id = "\"".repeat(175000);
    let messages = [
        json!({"jsonrpc":"2.0","id":"documentation-id-output","method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":accepted_id,"method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":refused_id,"method":"textDocument/hover","params":navigation_params(source,source.find("target").unwrap())}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let replies = Package::new(true).exchange(&messages);
    let accepted = replies
        .iter()
        .find(|response| response["id"] == accepted_id)
        .unwrap();
    assert_eq!(
        accepted["result"],
        documentation_expected(
            source,
            "target",
            "\nfun target(){}",
            Some(" literal "),
            false
        )
    );
    assert!(serde_json::to_vec(accepted).unwrap().len() + 32 <= 1024 * 1024);
    let refused = replies
        .iter()
        .find(|response| response["id"] == refused_id)
        .unwrap();
    assert_eq!(refused["error"]["code"], -32803);
    assert!(refused.get("result").is_none());
    assert_eq!(
        replies
            .iter()
            .filter(|response| response["id"] == refused_id)
            .count(),
        1
    );
}

#[test]
fn documentation_paired_cli_selects_original_class_property_parameter_and_nested_names() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for (source, name, suffix, prose) in [
        (
            "/** class prose */\nclass Same {}",
            "Same",
            "\nclass Same {}",
            " class prose ",
        ),
        (
            "/** property prose */\nval `same😀`=1",
            "`same😀`",
            "\nval `same😀`=1",
            " property prose ",
        ),
        (
            "fun f(/** parameter prose */ p:Long){}",
            "p:Long",
            " p:Long",
            " parameter prose ",
        ),
        (
            "fun same(){/** inner prose */ fun same(){}}",
            "same(){}",
            " fun same(){}",
            " inner prose ",
        ),
        (
            "fun f(){/** local prose */ val x=1;val y=x}",
            "x=1",
            " val x=1",
            " local prose ",
        ),
    ] {
        let start = source.rfind(name).unwrap();
        let name_len = if name == "p:Long" || name == "x=1" {
            1
        } else if name == "same(){}" {
            4
        } else {
            name.len()
        };
        let messages = [
            json!({"jsonrpc":"2.0","id":"documentation-kinds","method":"initialize","params":{"capabilities":{"textDocument":{"hover":{"contentFormat":["plaintext"]}}}}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"name","method":"textDocument/hover","params":navigation_params(source,start)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        let response = replies
            .iter()
            .find(|response| response["id"] == "name")
            .unwrap();
        assert_eq!(
            response["result"],
            json!({
                "contents":{"kind":"plaintext","value":format!("{prose}\n\n{suffix}")},
                "range":references_range(source,start,name_len)
            }),
            "{source}: {response}"
        );
    }
}

type RecursiveCliLayout = (
    Vec<std::ops::Range<usize>>,
    Vec<(std::ops::Range<usize>, Option<usize>)>,
);

fn recursive_cli_layout(source: &str, renamed: Option<(usize, &str)>) -> RecursiveCliLayout {
    let targets = [
        ("outer", "T"),
        ("outer", "Spare"),
        ("inner", "U"),
        ("deep", "`🦀`"),
        ("peer", "U"),
        ("shadow", "T"),
        ("other", "T"),
        ("returnOnly", "Only"),
    ];
    let declarations = targets
        .iter()
        .enumerate()
        .map(|(ordinal, (owner, raw))| {
            let end = source.find(&format!("> {owner}(")).unwrap();
            let start = source[..end].rfind('<').unwrap() + 1;
            let raw = if let Some((selected, name)) = renamed
                && selected == ordinal
            {
                name
            } else {
                raw
            };
            let offset = source[start..end].find(raw).unwrap() + start;
            offset..offset + raw.len()
        })
        .collect();
    let annotations = [
        ("root", Some(0)),
        ("@outer", Some(0)),
        ("before", Some(0)),
        ("a", Some(0)),
        ("b", Some(2)),
        ("@inner", Some(0)),
        ("inherited", Some(0)),
        ("c", Some(0)),
        ("@middle", Some(0)),
        ("d", Some(3)),
        ("e", Some(2)),
        ("f", None),
        ("@deep", Some(3)),
        ("own", Some(3)),
        ("g", Some(0)),
        ("@side", None),
        ("h", Some(4)),
        ("i", Some(0)),
        ("@peer", Some(4)),
        ("j", Some(5)),
        ("@shadow", Some(5)),
        ("k", Some(0)),
        ("@after", Some(0)),
        ("last", Some(0)),
        ("z", Some(6)),
        ("@other", Some(6)),
        ("@returnOnly", Some(7)),
        ("@unmatched", None),
    ]
    .iter()
    .map(|(label, target)| {
        let offset = if let Some(function) = label.strip_prefix('@') {
            let name = source.find(&format!(" {function}(")).unwrap();
            let close = name + source[name..].find(')').unwrap() + 1;
            assert_eq!(&source[close..close + 1], ":");
            close + 1
        } else {
            let pattern = format!("{label}:");
            source
                .match_indices(&pattern)
                .find(|(i, _)| {
                    *i == 0
                        || !source.as_bytes()[i - 1].is_ascii_alphanumeric()
                            && source.as_bytes()[i - 1] != b'_'
                })
                .unwrap()
                .0
                + pattern.len()
        };
        let raw = &source[offset..];
        let length = if let Some(quoted) = raw.strip_prefix('`') {
            quoted.find('`').unwrap() + 2
        } else {
            raw.find([',', ')', '=', ';', ' ', '{']).unwrap()
        };
        (offset..offset + length, *target)
    })
    .collect();
    (declarations, annotations)
}

struct RecursiveProducer {
    declarations: Vec<(String, Option<usize>)>,
    annotations: Vec<(usize, Option<usize>)>,
}

fn assert_recursive_cli_source(source: &str, revision: i64) -> RecursiveProducer {
    use ktlsp_parser::{
        FileKind, Parser, SourceSnapshot,
        local_value_binding::{
            LocalBindingError, TypeParameterAnchor, bind_lexical_type_parameter,
            type_parameter_occurrences,
        },
        lowering::lower_declarations,
        structural_regions::build_regions,
    };
    let snapshot = SourceSnapshot::new(URI, revision, FileKind::Source, source).unwrap();
    let result = Parser::new(worker())
        .unwrap()
        .parse_snapshot(&snapshot, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    assert!(tree.diagnostics().is_empty());
    let lower = lower_declarations(&result, &snapshot).unwrap();
    let directory = build_regions(&lower, &result, &snapshot).unwrap();
    let declarations: Vec<_> = lower
        .declarations()
        .iter()
        .map(|d| (d.kind().to_owned(), d.owner()))
        .collect();
    let mut annotations = Vec::new();
    let mut annotation_sites = Vec::new();
    for (index, element) in tree.elements().iter().enumerate().filter(|(_, e)| {
        e.kind == "REFERENCE_EXPRESSION" && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
    }) {
        let annotated_index = tree.elements()
            [tree.elements()[element.parent.unwrap()].parent.unwrap()]
        .parent
        .unwrap();
        let annotated = lower
            .declarations()
            .iter()
            .position(|d| d.cst_index() == annotated_index)
            .unwrap();
        let bound = bind_lexical_type_parameter(&directory, &result, index, &snapshot);
        let target = match bound {
            Ok(binding) => {
                assert!(std::ptr::eq(binding.reference(), element));
                assert!(std::ptr::eq(
                    binding.annotated(),
                    &lower.declarations()[annotated]
                ));
                assert!(std::ptr::eq(
                    binding.name_token(),
                    &tree.elements()[index + 1]
                ));
                let target = lower
                    .declarations()
                    .iter()
                    .position(|d| std::ptr::eq(d, binding.target()))
                    .unwrap();
                let function = lower
                    .declarations()
                    .iter()
                    .position(|d| std::ptr::eq(d, binding.function()))
                    .unwrap();
                assert_eq!(lower.declarations()[target].owner(), Some(function));
                assert_eq!(binding.body().introducer(), Some(function));
                Some(target)
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    LocalBindingError::NoEligibleTypeParameter
                        | LocalBindingError::AmbiguousTypeParameter
                ));
                None
            }
        };
        annotations.push((annotated, target));
        annotation_sites.push((index, target));
    }
    for (ordinal, target) in lower
        .declarations()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.kind() == "TYPE_PARAMETER")
    {
        let found = match type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(target),
            &snapshot,
        ) {
            Ok(found) => found,
            Err(error) => {
                assert_eq!(error, LocalBindingError::AmbiguousTypeParameter);
                continue;
            }
        };
        let expected: Vec<_> = annotation_sites
            .iter()
            .filter(|(_, target)| *target == Some(ordinal))
            .map(|(index, _)| *index)
            .collect();
        assert_eq!(
            found
                .occurrences()
                .iter()
                .map(|a| a.reference_index())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(std::ptr::eq(found.target(), target));
    }
    RecursiveProducer {
        declarations,
        annotations,
    }
}

#[test]
fn recursive_type_scope_actual_paired_cli_all_anchors_and_full_version8_replay() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "// 😀\r\nfun <T, Spare> outer(root:T):T{ val before:T=root; fun <U> inner(a:`T`,b:U):T{ val inherited:T=a; fun middle(c:T):T{ fun <`🦀`> deep(d:`🦀`,e:U,f:Missing):`🦀`{ val own:`🦀`=d; unknown(noSuch=d); }; fun side(g:T):Missing{} }; fun <U> peer(h:U,i:T):U{} }; fun <T> shadow(j:T):T{}; fun after(k:T):T{}; val last:T=root }\r\nfun <T> other(z:T):T{}\r\nfun <Only> returnOnly():Only{}\r\nfun unmatched():Missing{}\r\n";
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"rename":{"prepareSupport":true}}
    }}});
    let (declarations, annotations) = recursive_cli_layout(source, None);
    let mut anchors: Vec<_> = declarations
        .iter()
        .cloned()
        .enumerate()
        .map(|(t, r)| (r, t, false, "Fresh"))
        .chain(
            annotations
                .iter()
                .filter_map(|(r, t)| t.map(|t| (r.clone(), t, true, "Fresh"))),
        )
        .collect();
    let return_only = annotations
        .iter()
        .find(|(_, t)| *t == Some(7))
        .unwrap()
        .0
        .clone();
    for name in ["out", "reified"] {
        anchors.push((return_only.clone(), 7, true, name));
    }
    let mut requests = vec![
        init.clone(),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
    ];
    for (i, (anchor, _, _, name)) in anchors.iter().enumerate() {
        for (role, method, params) in [
            (
                "definition",
                "textDocument/definition",
                navigation_params(source, anchor.start),
            ),
            (
                "refs",
                "textDocument/references",
                references_params(source, anchor.start, true),
            ),
            (
                "uses",
                "textDocument/references",
                references_params(source, anchor.start, false),
            ),
            (
                "highlights",
                "textDocument/documentHighlight",
                navigation_params(source, anchor.start),
            ),
            (
                "prepare",
                "textDocument/prepareRename",
                navigation_params(source, anchor.start),
            ),
            (
                "rename",
                "textDocument/rename",
                rename_params(source, anchor.start, name),
            ),
        ] {
            requests.push(
                json!({"jsonrpc":"2.0","id":format!("{role}-{i}"),"method":method,"params":params}),
            );
        }
    }
    for (id, offset, name) in [
        ("outer-capture", declarations[0].start, "U"),
        ("unused-capture", declarations[1].start, "Missing"),
        ("inner-capture", declarations[2].start, "T"),
        (
            "unmatched",
            annotations
                .iter()
                .find(|(_, t)| t.is_none())
                .unwrap()
                .0
                .start,
            "Fresh",
        ),
        ("callee", source.find("unknown(").unwrap(), "Fresh"),
        ("label", source.find("noSuch=").unwrap(), "Fresh"),
        ("value", source.find("noSuch=d").unwrap() + 7, "Fresh"),
    ] {
        requests.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/rename","params":rename_params(source,offset,name)}));
    }
    for name in ["where", "dynamic", "suspend"] {
        requests.push(
            json!({"jsonrpc":"2.0","id":name,"method":"textDocument/rename",
            "params":rename_params(source,return_only.start,name)}),
        );
    }
    for (id, character) in [("split-surrogate", 4), ("interior-crlf", 6)] {
        requests.push(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/prepareRename",
            "params":{"textDocument":{"uri":URI},"position":{"line":0,"character":character}}}),
        );
    }
    let unmatched_return = annotations.last().unwrap().0.start;
    for method in [
        "definition",
        "references",
        "documentHighlight",
        "prepareRename",
        "rename",
    ] {
        let params = match method {
            "references" => references_params(source, unmatched_return, true),
            "rename" => rename_params(source, unmatched_return, "Fresh"),
            _ => navigation_params(source, unmatched_return),
        };
        requests.push(
            json!({"jsonrpc":"2.0","id":format!("unmatched-return-{method}"),
            "method":format!("textDocument/{method}"),"params":params}),
        );
    }
    requests.extend([
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ]);
    let replies = package.exchange(&requests);
    let reply = |id: &str| replies.iter().find(|r| r["id"] == id).unwrap();
    let original_producer = assert_recursive_cli_source(source, 7);
    for (i, (anchor, target, is_use, name)) in anchors.iter().enumerate() {
        let partition: Vec<_> = std::iter::once(declarations[*target].clone())
            .chain(
                annotations
                    .iter()
                    .filter(|(_, t)| *t == Some(*target))
                    .map(|(r, _)| r.clone()),
            )
            .collect();
        let range = |r: &std::ops::Range<usize>| references_range(source, r.start, r.len());
        let locations = |skip: usize| {
            json!(
                partition
                    .iter()
                    .skip(skip)
                    .map(|r| json!({"uri":URI,"range":range(r)}))
                    .collect::<Vec<_>>()
            )
        };
        assert_eq!(reply(&format!("refs-{i}"))["result"], locations(0));
        assert_eq!(reply(&format!("uses-{i}"))["result"], locations(1));
        assert_eq!(
            reply(&format!("highlights-{i}"))["result"],
            json!(
                partition
                    .iter()
                    .enumerate()
                    .map(|(j, r)| json!({"range":range(r),"kind":if j==0 {3}else{2}}))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            reply(&format!("prepare-{i}"))["result"],
            json!({
                "range":range(anchor),"placeholder":source[declarations[*target].clone()].trim_matches('`')
            })
        );
        assert_eq!(
            reply(&format!("definition-{i}"))["result"],
            if *is_use {
                json!({"uri":URI,"range":range(&declarations[*target])})
            } else {
                Value::Null
            }
        );
        let actual_rename = reply(&format!("rename-{i}"));
        let edited = apply_original_rename(source, actual_rename, URI, 7);
        let (edited_declarations, edited_annotations) =
            recursive_cli_layout(&edited, Some((*target, name)));
        // Preserve the complete source outside actual returned edit coordinates.
        let mut expected = source.to_owned();
        for r in partition.iter().rev() {
            expected.replace_range(r.clone(), name);
        }
        assert_eq!(edited, expected);
        let mut replay = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
        ];
        for (j, (annotation, _)) in edited_annotations.iter().enumerate() {
            replay.push(json!({"jsonrpc":"2.0","id":format!("definition-{j}"),"method":"textDocument/definition","params":navigation_params(&edited,annotation.start)}));
        }
        for (j, declaration) in edited_declarations.iter().enumerate() {
            replay.push(json!({"jsonrpc":"2.0","id":format!("refs-{j}"),"method":"textDocument/references","params":references_params(&edited,declaration.start,true)}));
        }
        replay.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let rebound = package.exchange(&replay);
        for (j, (_, binding)) in edited_annotations.iter().enumerate() {
            if let Some(t) = binding {
                let actual = &rebound
                    .iter()
                    .find(|r| r["id"] == format!("definition-{j}"))
                    .unwrap()["result"];
                assert_eq!(
                    *actual,
                    json!({"uri":URI,"range":references_range(&edited,edited_declarations[*t].start,edited_declarations[*t].len())})
                );
            } else {
                let response = rebound
                    .iter()
                    .find(|r| r["id"] == format!("definition-{j}"))
                    .unwrap();
                assert_eq!(response["error"]["code"], -32803);
                assert!(response.get("result").is_none());
            }
        }
        for (j, declaration) in edited_declarations.iter().enumerate() {
            let partition: Vec<_> = std::iter::once(declaration.clone())
                .chain(
                    edited_annotations
                        .iter()
                        .filter(|(_, t)| *t == Some(j))
                        .map(|(r, _)| r.clone()),
                )
                .collect();
            assert_eq!(
                rebound
                    .iter()
                    .find(|r| r["id"] == format!("refs-{j}"))
                    .unwrap()["result"],
                json!(
                    partition
                        .iter()
                        .map(
                            |r| json!({"uri":URI,"range":references_range(&edited,r.start,r.len())})
                        )
                        .collect::<Vec<_>>()
                )
            );
        }
        let edited_producer = assert_recursive_cli_source(&edited, 8);
        assert_eq!(edited_producer.declarations, original_producer.declarations);
        assert_eq!(edited_producer.annotations, original_producer.annotations);
    }
    for id in [
        "outer-capture",
        "unused-capture",
        "inner-capture",
        "unmatched",
        "callee",
        "label",
        "value",
    ] {
        assert_eq!(reply(id)["error"]["code"], -32803, "{id}");
    }
    for name in ["where", "dynamic", "suspend"] {
        assert_eq!(reply(name)["error"]["code"], -32803);
    }
    for id in ["split-surrogate", "interior-crlf"] {
        assert_eq!(reply(id)["error"]["code"], -32602);
    }
    for method in [
        "definition",
        "references",
        "documentHighlight",
        "prepareRename",
        "rename",
    ] {
        assert_eq!(
            reply(&format!("unmatched-return-{method}"))["error"]["code"],
            -32803
        );
    }
}

#[test]
fn recursive_type_capture_unused_inner_and_safe_sibling_actual_cli() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for (source, needle, name, accepted) in [
        ("fun <T> outer(){ fun <U> inner():T{} }", "<T", "U", false),
        ("fun <T> outer(){ fun <U> inner():T{} }", "<U", "T", false),
        (
            "fun <T> outer(){ fun middle(){ fun unused():Captured{} } }",
            "<T",
            "Captured",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(){ fun unused():Captured{} } }",
            "<U",
            "Captured",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner():U{}; fun sibling():T{} }",
            "<T",
            "U",
            true,
        ),
        (
            "fun <T> outer(){ fun <U> inner():U{}; fun sibling():T{} }",
            "<U",
            "T",
            true,
        ),
        (
            "fun <T> outer(){ fun middle(){ fun <U,`U`> unused(){} } }",
            "<T",
            "Fresh",
            false,
        ),
        ("fun <T> outer(){ fun <U,V> inner():U{} }", "<U", "V", false),
    ] {
        let offset = source.find(needle).unwrap() + 1;
        let requests = vec![
            json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
                "workspace":{"workspaceEdit":{"documentChanges":true}},
                "textDocument":{"rename":{"prepareSupport":true}}
            }}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename","params":rename_params(source,offset,name)}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&requests);
        let response = replies.iter().find(|r| r["id"] == "rename").unwrap();
        let original_producer = assert_recursive_cli_source(source, 7);
        if accepted {
            let edited = apply_original_rename(source, response, URI, 7);
            let mut expected = source.to_owned();
            let use_needle = if needle == "<T" {
                "sibling():T"
            } else {
                "inner():U"
            };
            let use_offset = source.find(use_needle).unwrap() + use_needle.len() - 1;
            expected.replace_range(use_offset..use_offset + 1, name);
            expected.replace_range(offset..offset + 1, name);
            assert_eq!(edited, expected);
            let mut replay = vec![
                requests[0].clone(),
                requests[1].clone(),
                json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
            ];
            let expected_partitions: Vec<_> = [("outer", "sibling():"), ("inner", "inner():")]
                .iter()
                .map(|(owner, label)| {
                    let end = edited.find(&format!("> {owner}(")).unwrap();
                    let declaration = edited[..end].rfind('<').unwrap() + 1;
                    let annotation = edited.find(label).unwrap() + label.len();
                    (declaration, annotation)
                })
                .collect();
            for (i, (declaration, annotation)) in expected_partitions.iter().enumerate() {
                replay.push(json!({"jsonrpc":"2.0","id":format!("definition-{i}"),"method":"textDocument/definition","params":navigation_params(&edited,*annotation)}));
                replay.push(json!({"jsonrpc":"2.0","id":format!("refs-{i}"),"method":"textDocument/references","params":references_params(&edited,*declaration,true)}));
            }
            replay.extend([requests[4].clone(), requests[5].clone()]);
            let rebound = package.exchange(&replay);
            for (i, (declaration, annotation)) in expected_partitions.iter().enumerate() {
                assert_eq!(
                    rebound
                        .iter()
                        .find(|r| r["id"] == format!("definition-{i}"))
                        .unwrap()["result"],
                    json!({"uri":URI,"range":references_range(&edited,*declaration,1)})
                );
                assert_eq!(
                    rebound
                        .iter()
                        .find(|r| r["id"] == format!("refs-{i}"))
                        .unwrap()["result"],
                    json!([
                        {"uri":URI,"range":references_range(&edited,*declaration,1)},
                        {"uri":URI,"range":references_range(&edited,*annotation,1)}
                    ])
                );
            }
            let edited_producer = assert_recursive_cli_source(&edited, 8);
            assert_eq!(edited_producer.declarations, original_producer.declarations);
            assert_eq!(edited_producer.annotations, original_producer.annotations);
        } else {
            assert_eq!(response["error"]["code"], -32803);
            assert!(response.get("result").is_none());
        }
    }
}

fn label_producer_packet(source: &str, revision: i64, accepted: bool) -> Value {
    use ktlsp_parser::{
        FileKind, Parser, SourceSnapshot,
        local_value_binding::{bind_lexical_type_parameter, bind_local_argument_label},
        lowering::lower_declarations,
        structural_regions::build_regions,
    };
    let snapshot = SourceSnapshot::new(URI, revision, FileKind::Source, source).unwrap();
    let result = Parser::new(worker())
        .unwrap()
        .parse_snapshot(&snapshot, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    assert!(tree.diagnostics().is_empty());
    let lower = lower_declarations(&result, &snapshot).unwrap();
    let directory = build_regions(&lower, &result, &snapshot).unwrap();
    let label_index = tree
        .elements()
        .iter()
        .position(|e| {
            e.kind == "REFERENCE_EXPRESSION"
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == "VALUE_ARGUMENT_NAME")
        })
        .unwrap();
    let element_index = |element: &ktlsp_parser::Element| {
        tree.elements()
            .iter()
            .position(|e| std::ptr::eq(e, element))
            .unwrap()
    };
    let declaration_index = |declaration: &ktlsp_parser::lowering::Declaration<'_>| {
        lower
            .declarations()
            .iter()
            .position(|d| std::ptr::eq(d, declaration))
            .unwrap()
    };
    let binding = bind_local_argument_label(&directory, &result, label_index, &snapshot);
    let proof = if accepted {
        let binding = binding.unwrap();
        let call = binding.call();
        let parameters = binding.parameters();
        assert!(std::ptr::eq(call.origin(), &lower));
        assert!(std::ptr::eq(parameters.origin(), &lower));
        assert_eq!(call.arguments().len(), 1);
        assert_eq!(parameters.parameters().len(), 1);
        let argument = &call.arguments()[0];
        let label = argument.label().unwrap();
        assert!(std::ptr::eq(
            label.reference(),
            &tree.elements()[label_index]
        ));
        assert!(std::ptr::eq(
            parameters.parameters()[0].declaration(),
            binding.target()
        ));
        assert_eq!(
            binding.target().owner(),
            Some(declaration_index(parameters.function()))
        );
        assert_eq!(
            parameters.function().owner(),
            Some(declaration_index(call.function()))
        );
        assert_eq!(
            binding.argument_parameter().owner(),
            Some(declaration_index(call.function()))
        );
        assert!(std::ptr::eq(
            binding.formal_annotation().annotated(),
            binding.target()
        ));
        assert!(std::ptr::eq(
            binding.argument_annotation().annotated(),
            binding.argument_parameter()
        ));
        assert_eq!(binding.type_parameter().kind(), "TYPE_PARAMETER");
        for annotation in [binding.formal_annotation(), binding.argument_annotation()] {
            let rebound = bind_lexical_type_parameter(
                &directory,
                &result,
                annotation.reference_index(),
                &snapshot,
            )
            .unwrap();
            assert!(std::ptr::eq(rebound.reference(), annotation.reference()));
            assert!(std::ptr::eq(rebound.target(), binding.type_parameter()));
        }
        json!({
            "target":declaration_index(binding.target()),
            "typeParameter":declaration_index(binding.type_parameter()),
            "argumentParameter":declaration_index(binding.argument_parameter()),
            "outerFunction":declaration_index(call.function()),
            "localFunction":declaration_index(parameters.function()),
            "call":element_index(call.call()),"callee":element_index(call.callee()),
            "calleeName":element_index(call.callee_name()),"callBody":element_index(call.body()),
            "argumentList":element_index(call.argument_list()),
            "callParens":[element_index(call.open_paren()),element_index(call.close_paren())],
            "argument":{"ordinal":argument.ordinal(),"node":element_index(argument.node()),
                "value":element_index(argument.expression()),"valueToken":element_index(argument.value_token()),
                "comma":argument.comma().map(element_index)},
            "label":{"node":element_index(label.node()),"reference":element_index(label.reference()),
                "name":element_index(label.name()),"equals":element_index(label.equals())},
            "formal":{"ordinal":parameters.parameters()[0].ordinal(),
                "annotation":element_index(parameters.parameters()[0].annotation()),
                "comma":parameters.parameters()[0].comma().map(element_index)},
            "formalList":element_index(parameters.parameter_list()),
            "formalParens":[element_index(parameters.open_paren()),element_index(parameters.close_paren())],
            "formalBody":element_index(parameters.body()),
            "annotations":([binding.formal_annotation(),binding.argument_annotation()].map(|a| json!({
                "range":a.range()
            })))
        })
    } else {
        binding.unwrap_err();
        return Value::Null;
    };
    json!({
        "declarations":lower.declarations().iter().map(|d| json!({
            "nameRange":d.name_range()
        })).collect::<Vec<_>>(),
        "proof":proof
    })
}

#[test]
fn lexical_label_definition_actual_cli_type_rename_replay_and_adversarial_edits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let init = json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},
        "workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"rename":{"prepareSupport":true},"diagnostic":{}}
    }}});
    for (source, formal, label, value, callee) in [
        (
            "fun <T> outer(p: T) {\n    fun send(to: T) {}\n    send(to = p)\n}\n",
            "to",
            "to = p",
            "p)",
            "send(to =",
        ),
        (
            "// 😀\r\nfun <`T🦀`> outer(`p😀`: `T🦀`) {\r\n fun send(`to😀`: `T🦀`) {}\r\n send(/* before */ `to😀` /* label */ = /* value */ `p😀`)\r\n}\r\n",
            "`to😀`",
            "`to😀` /* label */",
            "`p😀`)",
            "send(/*",
        ),
    ] {
        let label_offset = source.find(label).unwrap();
        let formal_offset = source.find(formal).unwrap();
        let value_offset = source.find(value).unwrap();
        let callee_offset = source.find(callee).unwrap();
        let type_offset = source.find('<').unwrap() + 1;
        let original = label_producer_packet(source, 7, true);
        let mut requests = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"label","method":"textDocument/definition","params":navigation_params(source,label_offset)}),
            json!({"jsonrpc":"2.0","id":"type-rename","method":"textDocument/rename","params":rename_params(source,type_offset,"Fresh")}),
        ];
        for (i, annotation) in original["proof"]["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let offset = annotation["range"]["start"].as_u64().unwrap() as usize;
            requests.push(json!({"jsonrpc":"2.0","id":format!("annotation-{i}"),
                "method":"textDocument/definition","params":navigation_params(source,offset)}));
        }
        for (role, offset) in [
            ("label", label_offset),
            ("value", value_offset),
            ("callee", callee_offset),
        ] {
            for method in [
                "hover",
                "references",
                "documentHighlight",
                "prepareRename",
                "rename",
                "completion",
            ] {
                let mut params = references_params(source, offset, true);
                params["newName"] = json!("freshValue");
                requests.push(json!({"jsonrpc":"2.0","id":format!("{role}-{method}"),
                    "method":format!("textDocument/{method}"),"params":params}));
            }
        }
        for (role, offset) in [("value", value_offset), ("callee", callee_offset)] {
            requests.push(json!({"jsonrpc":"2.0","id":format!("{role}-definition"),
                "method":"textDocument/definition","params":navigation_params(source,offset)}));
        }
        for (id, offset) in [
            ("token-end", label_offset + formal.len()),
            ("eof", source.len()),
        ] {
            requests.push(
                json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition",
                "params":navigation_params(source,offset)}),
            );
        }
        if source.starts_with("//") {
            for (id, character) in [("split-surrogate", 4), ("interior-crlf", 6)] {
                requests.push(json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition",
                    "params":{"textDocument":{"uri":URI},"position":{"line":0,"character":character}}}));
            }
        }
        requests.extend([
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ]);
        let responses = package.exchange(&requests);
        assert!(
            responses
                .iter()
                .filter_map(|r| r.get("id"))
                .eq(requests.iter().filter_map(|r| r.get("id")))
        );
        let reply = |id: &str| responses.iter().find(|r| r["id"] == id).unwrap();
        assert_eq!(
            reply("init")["result"]["capabilities"]["positionEncoding"],
            "utf-16"
        );
        assert_eq!(
            reply("label")["result"],
            json!({"uri":URI,"range":references_range(source,formal_offset,formal.len())})
        );
        let type_declaration = original["proof"]["typeParameter"].as_u64().unwrap() as usize;
        let type_range = &original["declarations"][type_declaration]["nameRange"];
        let type_start = type_range["start"].as_u64().unwrap() as usize;
        let type_end = type_range["end"].as_u64().unwrap() as usize;
        for i in 0..2 {
            assert_eq!(
                reply(&format!("annotation-{i}"))["result"],
                json!({"uri":URI,"range":references_range(source,type_start,type_end-type_start)})
            );
        }
        for (role, _) in [
            ("label", label_offset),
            ("value", value_offset),
            ("callee", callee_offset),
        ] {
            for method in [
                "hover",
                "references",
                "documentHighlight",
                "prepareRename",
                "rename",
                "completion",
            ] {
                let response = reply(&format!("{role}-{method}"));
                assert_eq!(response["error"]["code"], -32803, "{response}");
                assert!(response.get("result").is_none());
            }
        }
        for id in ["value-definition", "callee-definition"] {
            assert_eq!(reply(id)["error"]["code"], -32803);
            assert!(reply(id).get("result").is_none());
        }
        for id in ["token-end", "eof"] {
            assert_eq!(reply(id)["result"], Value::Null);
            assert!(reply(id).get("error").is_none());
        }
        if source.starts_with("//") {
            for id in ["split-surrogate", "interior-crlf"] {
                assert_eq!(reply(id)["error"]["code"], -32602);
            }
        }
        let edited = apply_original_rename(source, reply("type-rename"), URI, 7);
        let type_ranges: Vec<_> =
            std::iter::once(original["proof"]["typeParameter"].as_u64().unwrap() as usize)
                .map(|i| original["declarations"][i]["nameRange"].clone())
                .chain(
                    original["proof"]["annotations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|a| a["range"].clone()),
                )
                .map(|r| r["start"].as_u64().unwrap() as usize..r["end"].as_u64().unwrap() as usize)
                .collect();
        let mut expected = source.to_owned();
        let mut sorted = type_ranges;
        sorted.sort_by_key(|r| r.start);
        assert_eq!(
            reply("type-rename")["result"]["documentChanges"][0]["edits"],
            json!(
                sorted
                    .iter()
                    .map(|r| json!({"range":references_range(source,r.start,r.len()),
                "newText":"Fresh"}))
                    .collect::<Vec<_>>()
            )
        );
        for range in sorted.into_iter().rev() {
            expected.replace_range(range, "Fresh");
        }
        assert_eq!(edited, expected);
        let edited_producer = label_producer_packet(&edited, 8, true);
        for field in [
            "target",
            "typeParameter",
            "argumentParameter",
            "outerFunction",
            "localFunction",
            "call",
            "callee",
            "calleeName",
            "callBody",
            "argumentList",
            "callParens",
            "argument",
            "label",
            "formal",
            "formalList",
            "formalParens",
            "formalBody",
        ] {
            assert_eq!(
                edited_producer["proof"][field], original["proof"][field],
                "{field}"
            );
        }
        let mut replay = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":8,"text":edited}}}),
            json!({"jsonrpc":"2.0","id":"label","method":"textDocument/definition","params":navigation_params(&edited,edited.find(label).unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        for (i, annotation) in edited_producer["proof"]["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let offset = annotation["range"]["start"].as_u64().unwrap() as usize;
            replay.insert(
                4 + i,
                json!({"jsonrpc":"2.0","id":format!("annotation-{i}"),
                "method":"textDocument/definition","params":navigation_params(&edited,offset)}),
            );
        }
        let replay_responses = package.exchange(&replay);
        assert_eq!(
            replay_responses[1]["result"],
            json!({"uri":URI,
            "range":references_range(&edited,edited.find(formal).unwrap(),formal.len())})
        );
        for i in 0..2 {
            let response = replay_responses
                .iter()
                .find(|r| r["id"] == format!("annotation-{i}"))
                .unwrap();
            assert_eq!(
                response["result"],
                json!({"uri":URI,"range":references_range(&edited,type_start,"Fresh".len())})
            );
        }
    }
    let source = "fun <T, U> outer(p: T) { fun send(to: T) {}; send(to = p) }";
    for (name, start, end, replacement) in [
        (
            "distinct-binder",
            source.find("to: T").unwrap() + 4,
            source.find("to: T").unwrap() + 5,
            "U",
        ),
        (
            "future-shadow",
            source.rfind('}').unwrap(),
            source.rfind('}').unwrap(),
            "; val p = 1 ",
        ),
        (
            "other-arity",
            source.find("send(to =").unwrap(),
            source.find("send(to =").unwrap(),
            "fun send() {}; ",
        ),
    ] {
        let range = references_range(source, start, end - start);
        let edit = ktlsp_text::TextChange {
            range: Some(serde_json::from_value(range.clone()).unwrap()),
            range_length: None,
            text: replacement.to_owned(),
        };
        let mut document = ktlsp_text::Document::new(7, source.to_owned()).unwrap();
        document.apply_changes(8, &[edit]).unwrap();
        let edited = document.text();
        let requests = vec![
            init.clone(),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
            json!({"jsonrpc":"2.0","id":"before","method":"textDocument/definition","params":navigation_params(source,source.find("to =").unwrap())}),
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":8},"contentChanges":[{"range":range,"text":replacement}]}}),
            json!({"jsonrpc":"2.0","id":"after","method":"textDocument/definition","params":navigation_params(edited,edited.find("to =").unwrap())}),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let responses = package.exchange(&requests);
        assert_eq!(
            responses[1]["result"],
            json!({"uri":URI,"range":references_range(source,source.find("to:").unwrap(),2)})
        );
        assert_eq!(
            responses[2]["error"]["code"], -32803,
            "{name}: {responses:?}"
        );
        assert!(responses[2].get("result").is_none());
        label_producer_packet(edited, 8, false);
    }
}

#[test]
fn lexical_label_definition_keeps_selectors_stale_cancel_and_reopen_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "// 😀\r\nfun <T> outer(p:T){ fun send(`to😀`:T){}; send(`to😀`=p) }\r\n";
    let label = source.rfind("`to😀`").unwrap();
    let target = source.find("`to😀`").unwrap();
    let expected = json!({"uri":URI,"range":references_range(source,target,"`to😀`".len())});
    let mut session = running_rename();
    open(&mut session, source);
    for (within, _) in "`to😀`".char_indices() {
        let response = request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, label + within),
        );
        assert_eq!(response["result"], expected, "{response}");
    }
    for offset in [label + "`to😀`".len(), source.len()] {
        let response = request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, offset),
        );
        assert_eq!(response["result"], Value::Null);
        assert!(response.get("error").is_none());
    }
    for character in [4, 6] {
        let response = request(
            &mut session,
            "textDocument/definition",
            json!({"textDocument":{"uri":URI},"position":{"line":0,"character":character}}),
        );
        assert_eq!(response["error"]["code"], -32602);
    }
    change(&mut session, 1, "fun stale(){}");
    notify(
        &mut session,
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":2},
        "contentChanges":[{"range":{"start":{"line":0,"character":4},"end":{"line":0,"character":5}},"text":"bad"}]}),
    );
    assert_eq!(
        request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, label)
        )["result"],
        expected
    );
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(
        request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, label)
        )["error"]["code"],
        -32602
    );
    open(&mut session, source);
    assert_eq!(
        request(
            &mut session,
            "textDocument/definition",
            navigation_params(source, label)
        )["result"],
        expected
    );
    assert!(session.close().1);
    for action in ["cancel", "change", "reopen", "rejected-change"] {
        let mut session = running_rename();
        open(&mut session, source);
        assert!(
            session
                .handle(json!({"jsonrpc":"2.0","id":"label-pending",
            "method":"textDocument/definition","params":navigation_params(source,label)}))
                .is_empty()
        );
        let mut replies = match action {
            "cancel" => session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"label-pending"}})),
            "change" => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":source}]}})),
            "reopen" => {
                let replies = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose",
                    "params":{"textDocument":{"uri":URI}}}));
                open(&mut session,source);
                replies
            }
            _ => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange",
                "params":{"textDocument":{"uri":URI,"version":1},"contentChanges":[{"text":"fun stale(){}"}]}})),
        };
        if action == "rejected-change" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while replies.is_empty() && Instant::now() < deadline {
                replies.extend(session.poll());
                if replies.is_empty() {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            assert_eq!(replies.len(), 1, "{replies:?}");
            assert_eq!(replies[0]["result"], expected);
        } else {
            assert_eq!(replies.len(), 1, "{replies:?}");
            assert_eq!(
                replies[0]["error"]["code"],
                if action == "cancel" { -32800 } else { -32801 }
            );
            assert!(replies[0].get("result").is_none());
        }
        assert_eq!(replies[0]["id"], "label-pending");
        let (remaining, clean) = session.close();
        assert!(clean);
        assert!(remaining.iter().all(|r| r["id"] != "label-pending"));
    }
}

fn signature_producer_packet(source: &str, revision: i64, cursor: usize) -> Value {
    use ktlsp_parser::{
        FileKind, Parser, SourceSnapshot, local_value_binding::local_call_signature,
        lowering::lower_declarations, structural_regions::build_regions,
    };
    let snapshot = SourceSnapshot::new(URI, revision, FileKind::Source, source).unwrap();
    let (parsed, clean) = Parser::new(worker())
        .unwrap()
        .parse_snapshot_with_cleanup(&snapshot, &std::sync::atomic::AtomicBool::new(false));
    let mut packet = json!({"source":source,"revision":revision,"cursor":cursor,"cleanup":clean});
    let result = parsed.unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    packet["elements"] = json!(tree.elements());
    packet["diagnostics"] = json!(tree.diagnostics());
    let lowered = lower_declarations(&result, &snapshot);
    let Ok(lowered) = lowered else {
        return packet;
    };
    packet["declarations"] =
        json!(lowered.declarations().iter().map(|d| json!({
        "kind":d.kind(),"name":d.raw_name(),"range":d.range(),"nameRange":d.name_range(),
        "owner":d.owner(),"cst":d.cst_index(),"parent":d.cst_parent_index(),
        "incomplete":d.incomplete()
    })).collect::<Vec<_>>());
    let directory = build_regions(&lowered, &result, &snapshot);
    let Ok(directory) = directory else {
        return packet;
    };
    packet["memberships"] = json!(
        directory
            .memberships()
            .iter()
            .map(|m| m.region())
            .collect::<Vec<_>>()
    );
    packet["regions"] = json!(directory.regions().iter().map(|r| json!({
        "cst":r.cst_index(),"kind":r.kind(),"range":r.range(),"parent":r.parent(),"introducer":r.introducer()
    })).collect::<Vec<_>>());
    let proof = local_call_signature(&directory, &result, &snapshot, cursor);
    if let Ok(Some(proof)) = proof {
        let binding = proof.binding();
        let call = binding.call();
        let parameters = binding.parameters();
        let arg = &call.arguments()[0];
        let label = arg.label().unwrap();
        let declaration = |d: &ktlsp_parser::lowering::Declaration<'_>| {
            lowered
                .declarations()
                .iter()
                .position(|original| std::ptr::eq(original, d))
                .unwrap()
        };
        let element = |e: &ktlsp_parser::Element| {
            tree.elements()
                .iter()
                .position(|original| std::ptr::eq(original, e))
                .unwrap()
        };
        packet["proof"] = json!({
            "function":declaration(parameters.function()),"outer":declaration(call.function()),
            "formal":declaration(binding.target()),"argumentParameter":declaration(binding.argument_parameter()),
            "typeParameter":declaration(binding.type_parameter()),
            "call":element(call.call()),"callee":element(call.callee()),"calleeName":element(call.callee_name()),
            "body":element(call.body()),"list":element(call.argument_list()),
            "open":element(call.open_paren()),"close":element(call.close_paren()),
            "argument":element(arg.node()),"value":element(arg.expression()),"valueToken":element(arg.value_token()),
            "label":element(label.node()),"labelReference":element(label.reference()),
            "labelName":element(label.name()),"equals":element(label.equals()),
            "comma":arg.comma().map(element),
            "formalList":element(parameters.parameter_list()),"formalBody":element(parameters.body()),
            "formalOpen":element(parameters.open_paren()),"formalClose":element(parameters.close_paren()),
            "formalAnnotation":element(parameters.parameters()[0].annotation()),
            "formalComma":parameters.parameters()[0].comma().map(element),
            "annotations":([binding.argument_annotation(),binding.formal_annotation()].map(|a| json!({
                "annotated":declaration(a.annotated()),"reference":a.reference_index(),
                "token":element(a.name_token()),"range":a.range()
            }))),
            "cursor":proof.cursor(),"signatureRange":proof.signature_range(),"parameterRange":proof.parameter_range()
        });
        let labels = proof.into_labels(20);
        if let Ok((signature, parameter)) = labels {
            packet["result"] = json!({"signatures":[{"label":signature,"parameters":[{"label":parameter}]}],
                "activeSignature":0,"activeParameter":0});
        }
    }
    packet
}

// The real packaged CLI stays alive while genuine returned rename edits are
// independently applied and sent back as didChange. No replay-only substitute.
struct SignatureWire {
    child: Option<std::process::Child>,
    input: Option<std::process::ChildStdin>,
    reader: std::io::BufReader<std::process::ChildStdout>,
    responses: Vec<Value>,
}
impl SignatureWire {
    fn new(package: &Package) -> Self {
        Self::in_directory(&package.0)
    }
    fn in_directory(directory: &Path) -> Self {
        let mut child =
            Command::new(directory.join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX)))
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
        let input = child.stdin.take();
        let reader = std::io::BufReader::new(child.stdout.take().unwrap());
        Self {
            child: Some(child),
            input,
            reader,
            responses: Vec::new(),
        }
    }
    fn send(&mut self, message: Value) -> Value {
        let input = self.input.as_mut().unwrap();
        ktlsp_protocol::write_frame(input, &serde_json::to_vec(&message).unwrap()).unwrap();
        input.flush().unwrap();
        if let Some(id) = message.get("id") {
            loop {
                let frame = ktlsp_protocol::read_frame(&mut self.reader)
                    .unwrap()
                    .unwrap();
                let response: Value = serde_json::from_slice(&frame).unwrap();
                let matched = response.get("id") == Some(id);
                self.responses.push(response.clone());
                if matched {
                    return response;
                }
            }
        }
        Value::Null
    }
    fn finish(self) -> Vec<Value> {
        self.terminate(true).0
    }
    fn terminate(mut self, exit: bool) -> (Vec<Value>, std::process::ExitStatus, Vec<u8>) {
        let stopped = self.send(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
        assert!(stopped.get("error").is_none(), "{stopped}");
        assert_eq!(stopped["result"], Value::Null);
        if exit {
            self.send(json!({"jsonrpc":"2.0","method":"exit"}));
        }
        self.input.take();
        while let Some(frame) = ktlsp_protocol::read_frame(&mut self.reader).unwrap() {
            self.responses.push(serde_json::from_slice(&frame).unwrap());
        }
        let output = self.child.take().unwrap().wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{:?}", output.status);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        (
            std::mem::take(&mut self.responses),
            output.status,
            output.stderr,
        )
    }
}
impl Drop for SignatureWire {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            self.input.take(); // EOF retains the server's ordinary owned cleanup.
            child.wait_with_output().unwrap();
        }
    }
}

fn signature_open(wire: &mut SignatureWire, source: &str, profile: Value) -> Value {
    let initialize = wire.send(json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{
        "general":{"positionEncodings":["utf-8"]},"workspace":{"workspaceEdit":{"documentChanges":true}},
        "textDocument":{"diagnostic":{},"rename":{"prepareSupport":true},"signatureHelp":profile}
    }}}));
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}}),
    );
    initialize
}
fn signature_request(
    wire: &mut SignatureWire,
    id: Value,
    source: &str,
    cursor: usize,
    context: Option<Value>,
) -> Value {
    let mut params = navigation_params(source, cursor);
    if let Some(context) = context {
        params["context"] = context;
    }
    wire.send(
        json!({"jsonrpc":"2.0","id":id,"method":"textDocument/signatureHelp","params":params}),
    )
}
fn signature_element<'a>(packet: &'a Value, role: &str) -> &'a Value {
    &packet["elements"][packet["proof"][role].as_u64().unwrap() as usize]
}
fn signature_assert_source(packet: &Value, response: &Value) {
    assert_eq!(packet["cleanup"], true);
    assert_eq!(packet["diagnostics"], json!([]));
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"], packet["result"]);
    let source = packet["source"].as_str().unwrap();
    let range = |key: &str| {
        let range = &packet["proof"][key];
        range["start"].as_u64().unwrap() as usize..range["end"].as_u64().unwrap() as usize
    };
    let signature = range("signatureRange");
    let formal = range("parameterRange");
    assert_eq!(
        response["result"]["signatures"][0]["label"],
        &source[signature.clone()]
    );
    assert_eq!(
        response["result"]["signatures"][0]["parameters"][0]["label"],
        &source[formal.clone()]
    );
    let function = &packet["declarations"][packet["proof"]["function"].as_u64().unwrap() as usize];
    let parameter = &packet["declarations"][packet["proof"]["formal"].as_u64().unwrap() as usize];
    assert_eq!(
        signature.start,
        function["nameRange"]["start"].as_u64().unwrap() as usize
    );
    assert_eq!(
        signature.end,
        signature_element(packet, "formalClose")["end"]
            .as_u64()
            .unwrap() as usize
    );
    assert_eq!(
        formal.start,
        parameter["range"]["start"].as_u64().unwrap() as usize
    );
    assert_eq!(
        formal.end,
        parameter["range"]["end"].as_u64().unwrap() as usize
    );
    assert!(signature.start <= formal.start && formal.end <= signature.end);
    assert!(
        response["result"]["signatures"][0]
            .get("documentation")
            .is_none()
    );
    assert!(response["result"].get("range").is_none());
}

#[test]
fn manual_signature_help_actual_source_cursor_profiles_and_same_session_type_edits() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    for (source, marker, profile) in [
        (
            "fun <T> outer(p: T) {\n fun send(to: T) {}\n send(to = p)\n}\n",
            "send(",
            json!({}),
        ),
        (
            "fun <T> outer(p:T){ fun send(to: /* quote \" slash \\ tab \t */ T){}; send(to=p) }",
            "send(",
            json!({}),
        ),
        (
            "fun <`T`> outer(`p`:T){ fun `send`(`to`:`T`){}; send(to=p) }",
            "send(",
            json!({"signatureInformation":{"parameterInformation":{"labelOffsetSupport":false}}}),
        ),
        (
            "// 😀\r\nfun <`T🦀`> outer(`p😀`:`T🦀`){\r\n fun `send🦀`(\r\n /* formal */ `to😀` : /* type */ `T🦀`,\r\n ){}\r\n `send🦀`(/* label */ `to😀` /* eq */ = /* value */ `p😀`)\r\n}\r\n",
            "`send🦀`(",
            json!({"signatureInformation":{"parameterInformation":{"labelOffsetSupport":true},"activeParameterSupport":true}}),
        ),
        (
            "fun <T> outer(p:T){\r fun send(\r to:T\r ){}\r send(\r to=p\r )\r}\r",
            "send(",
            json!({}),
        ),
        (
            "fun <T> outer(p:T){ fun send(to:T,){}; send(to=p, /* next */) }",
            "send(",
            json!({}),
        ),
        (
            "fun <T> outer(p:T){ fun <U,send,p> unused(){}; fun send(to:T){}; send(to=p) }",
            "send(",
            json!({}),
        ),
        (
            "import elsewhere.send\nclass send {}\nfun send(){}\nfun <Int> outer(p:Int){ fun send(to:`Int`){}; send(to=p) }",
            "send(",
            json!({}),
        ),
    ] {
        let call_start = source.rfind(marker).unwrap() + marker.len();
        // '=' ends before value trivia, never at the opening slash of the
        // original immediately-following label comment.
        let cursor = source.rfind('=').unwrap() + 1;
        let original = signature_producer_packet(source, 7, cursor);
        let mut wire = SignatureWire::new(&package);
        let initialized = signature_open(&mut wire, source, profile);
        let first = signature_request(&mut wire, json!(u64::MAX), source, cursor, None);
        let context = json!({"triggerKind":3,"isRetrigger":true,"triggerCharacter":",",
            "activeSignatureHelp":{"signatures":[{"label":"FOREIGN","parameters":[{"label":[0,7]}],"activeParameter":99}],
                "activeSignature":99,"activeParameter":99}});
        let retrigger =
            signature_request(&mut wire, json!("retrigger"), source, cursor, Some(context));
        let mut positions = vec![0, source.len()];
        for role in [
            "calleeName",
            "open",
            "labelName",
            "equals",
            "valueToken",
            "close",
        ] {
            let token = signature_element(&original, role);
            positions.push(token["start"].as_u64().unwrap() as usize);
            positions.push(token["end"].as_u64().unwrap() as usize);
        }
        if let Some(comma) = original["proof"]["comma"].as_u64() {
            let token = &original["elements"][comma as usize];
            positions.extend([
                token["start"].as_u64().unwrap() as usize,
                token["end"].as_u64().unwrap() as usize,
            ]);
        }
        for token in original["elements"].as_array().unwrap().iter().filter(|e| {
            e["token"] == true
                && matches!(e["kind"].as_str(), Some("EOL_COMMENT" | "BLOCK_COMMENT"))
        }) {
            let start = token["start"].as_u64().unwrap() as usize;
            let end = token["end"].as_u64().unwrap() as usize;
            if call_start <= start {
                positions.extend([start, start + 1, end]);
            }
        }
        positions.sort_unstable();
        positions.dedup();
        let mut selected = Vec::new();
        for (index, offset) in positions.into_iter().enumerate() {
            if offset_to_position(source, offset).is_ok() {
                selected.push((
                    offset,
                    signature_request(
                        &mut wire,
                        json!(format!("cursor-{index}")),
                        source,
                        offset,
                        None,
                    ),
                ));
            }
        }
        let rename = wire.send(
            json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename",
            "params":rename_params(source, source.find('<').unwrap()+1, "Fresh")}),
        );
        // Every returned original-coordinate edit is applied independently, in
        // reverse order for sequential didChange coordinates, to actual v7.
        let edited = apply_original_rename(source, &rename, URI, 7);
        let edits = rename["result"]["documentChanges"][0]["edits"]
            .as_array()
            .unwrap();
        let content_changes = edits
            .iter()
            .rev()
            .map(|edit| json!({"range":edit["range"],"text":edit["newText"]}))
            .collect::<Vec<_>>();
        wire.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":8},"contentChanges":content_changes}}),
        );
        let edited_cursor = edited.rfind('=').unwrap() + 1;
        let edited_producer = signature_producer_packet(&edited, 8, edited_cursor);
        let after = signature_request(
            &mut wire,
            json!("edited"),
            &edited,
            edited_cursor,
            Some(json!({"triggerKind":2,"triggerCharacter":"(","isRetrigger":false})),
        );
        let responses = wire.finish();
        assert_eq!(
            initialized["result"]["capabilities"]["signatureHelpProvider"],
            json!({})
        );
        assert_eq!(
            initialized["result"]["capabilities"]["positionEncoding"],
            "utf-16"
        );
        signature_assert_source(&original, &first);
        signature_assert_source(&original, &retrigger);
        let open = signature_element(&original, "open")["end"]
            .as_u64()
            .unwrap() as usize;
        let close = signature_element(&original, "close")["start"]
            .as_u64()
            .unwrap() as usize;
        let comma_end = original["proof"]["comma"].as_u64().map(|index| {
            original["elements"][index as usize]["end"]
                .as_u64()
                .unwrap() as usize
        });
        for (offset, response) in selected {
            let comment = original["elements"].as_array().unwrap().iter().any(|e| {
                e["token"] == true
                    && matches!(e["kind"].as_str(), Some("EOL_COMMENT" | "BLOCK_COMMENT"))
                    && e["start"].as_u64().unwrap() as usize <= offset
                    && offset < e["end"].as_u64().unwrap() as usize
            });
            if open <= offset
                && offset <= close
                && !comment
                && comma_end.is_none_or(|end| offset < end)
            {
                signature_assert_source(&original, &response);
            } else {
                assert_eq!(
                    response["result"],
                    Value::Null,
                    "cursor={offset}: {response}"
                );
                assert!(response.get("error").is_none());
            }
        }
        signature_assert_source(&edited_producer, &after);
        let type_ordinal = edited_producer["proof"]["typeParameter"].as_u64().unwrap() as usize;
        assert_eq!(
            edited_producer["declarations"][type_ordinal]["name"],
            "Fresh"
        );
        assert_ne!(first["result"], after["result"]);
        assert_eq!(edits.len(), 3);
        let ids = responses
            .iter()
            .filter_map(|r| r.get("id"))
            .collect::<Vec<_>>();
        let mut unique = ids.clone();
        unique.sort_by_key(|id| id.to_string());
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "one response per exact ID");
    }
}

#[test]
fn manual_signature_help_actual_adversarial_changes_and_unsupported_families() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "fun <T,U> outer(p:T){ fun send(to:T){}; send(to=p) }";
    let cursor = source.rfind("send(").unwrap() + 5;
    let original = signature_producer_packet(source, 7, cursor);
    for (name, start, end, replacement) in [
        (
            "distinct-formal",
            source.find("to:T").unwrap() + 3,
            source.find("to:T").unwrap() + 4,
            "U",
        ),
        (
            "future-argument-shadow",
            source.rfind('}').unwrap(),
            source.rfind('}').unwrap(),
            "; val p=1 ",
        ),
        (
            "other-arity-competitor",
            source.rfind("send(").unwrap(),
            source.rfind("send(").unwrap(),
            "fun send(){}; ",
        ),
        (
            "damaged-call",
            source.rfind("=p").unwrap() + 1,
            source.rfind("=p").unwrap() + 2,
            "",
        ),
    ] {
        let range = references_range(source, start, end - start);
        let mut edited = source.to_owned();
        edited.replace_range(start..end, replacement);
        let edited_cursor = edited.rfind("send(").unwrap() + 5;
        let producer = signature_producer_packet(&edited, 8, edited_cursor);
        let mut wire = SignatureWire::new(&package);
        signature_open(&mut wire, source, json!({}));
        let before = signature_request(&mut wire, json!("before"), source, cursor, None);
        wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":URI,"version":8},"contentChanges":[{"range":range,"text":replacement}]}}));
        let after = signature_request(&mut wire, json!("after"), &edited, edited_cursor, None);
        wire.finish();
        signature_assert_source(&original, &before);
        assert_eq!(after["error"]["code"], -32803, "{name}: {after}");
        assert!(after.get("result").is_none());
        assert!(producer.get("result").is_none(), "{producer}");
    }
    let canonical = "fun <T> outer(p:T){ fun send(to:T){}; send(to=p) }";
    let mut unsupported = vec![
        (
            "nominal",
            "fun outer(p:Int){ fun send(to:Int){}; send(to=p) }".to_owned(),
            "send(",
        ),
        (
            "same-spelled-distinct-binder",
            "fun <T> outer(p:T){ fun <`T`> send(to:T){}; send(to=p) }".to_owned(),
            "send(",
        ),
        (
            "future-target",
            "fun <T> outer(p:T){ send(to=p); fun send(to:T){} }".to_owned(),
            "send(",
        ),
        (
            "member-owner",
            "class C{ fun <T> outer(p:T){ fun send(to:T){}; send(to=p) } }".to_owned(),
            "send(",
        ),
        (
            "nested-call-owner",
            "fun <T> outer(p:T){ fun nested(){ fun send(to:T){}; send(to=p) } }".to_owned(),
            "send(",
        ),
        (
            "local-val-argument",
            "fun <T> outer(p:T){ fun send(to:T){}; val q:T=p; send(to=q) }".to_owned(),
            "send(",
        ),
        (
            "constructor",
            canonical.replace("fun send(to:T){}", "class send"),
            "send(",
        ),
    ];
    for (name, argument) in [
        ("zero", ""),
        ("positional", "p"),
        ("multiple", "to=p,to=p"),
        ("unknown-label", "wrong=p"),
        ("mixed", "to=p,p"),
        ("literal", "to=1"),
        ("long-literal", "to=2L"),
        ("spread", "to=*p"),
        ("unresolved", "to=missing"),
        ("parenthesized", "to=(p)"),
        ("nested-expression", "to=send()"),
    ] {
        unsupported.push((
            name,
            canonical.replace("send(to=p)", &format!("send({argument})")),
            "send(",
        ));
    }
    for (name, header) in [
        ("two-formals", "fun send(to:T,other:T){}"),
        ("default", "fun send(to:T=p){}"),
        ("vararg", "fun send(vararg to:T){}"),
        ("generic", "fun <U> send(to:T){}"),
        ("extension", "fun T.send(to:T){}"),
        ("return", "fun send(to:T):T{}"),
        ("nonempty", "fun send(to:T){ val q=1 }"),
        ("modifier", "suspend fun send(to:T){}"),
    ] {
        unsupported.push((name, canonical.replace("fun send(to:T){}", header), "send("));
    }
    for (name, call, marker) in [
        ("qualified", "obj.send(to=p)", "send("),
        ("safe", "obj?.send(to=p)", "send("),
        ("type-arguments", "send<T>(to=p)", "send<T>("),
        ("chained", "send(to=p)()", "send("),
        ("incomplete-open", "send(", "send("),
        ("incomplete-value", "send(to=", "send("),
    ] {
        unsupported.push((name, canonical.replace("send(to=p)", call), marker));
    }
    for (name, source, marker) in unsupported {
        // For a nested unsupported invocation select the outer argument list,
        // never accidentally ask about the inner empty call.
        let call = source
            .find("; send(")
            .map(|offset| offset + 2)
            .or_else(|| source.find("send(to=p)"))
            .unwrap_or_else(|| source.rfind(marker).unwrap());
        let cursor = call + marker.len();
        let producer = signature_producer_packet(&source, 7, cursor);
        let mut wire = SignatureWire::new(&package);
        signature_open(&mut wire, &source, json!({}));
        let response = signature_request(&mut wire, json!(name), &source, cursor, None);
        wire.finish();
        assert_eq!(response["error"]["code"], -32803, "{name}: {response}");
        assert!(response.get("result").is_none());
        assert!(producer.get("result").is_none(), "{name}: {producer}");
    }
}

#[test]
fn manual_signature_help_validates_context_utf16_and_open_source_authority() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let source = "// 😀\r\nfun <T> outer(p:T){ fun send(to:T){}; send(to=p) }\r\n";
    let cursor = source.rfind("send(").unwrap() + 5;
    let original = signature_producer_packet(source, 7, cursor);
    let mut wire = SignatureWire::new(&package);
    signature_open(&mut wire, source, json!({}));
    let mut invalid = Vec::new();
    for (i, context) in [
        Value::Null,json!([]),json!({}),json!({"triggerKind":0,"isRetrigger":false}),
        json!({"triggerKind":4,"isRetrigger":false}),json!({"triggerKind":1,"isRetrigger":0}),
        json!({"triggerKind":1.5,"isRetrigger":false}),json!({"triggerKind":2,"isRetrigger":false,"triggerCharacter":1}),
        json!({"triggerKind":1,"isRetrigger":false,"activeSignatureHelp":null}),
        json!({"triggerKind":1,"isRetrigger":false,"activeSignatureHelp":{"signatures":{}}}),
        json!({"triggerKind":1,"isRetrigger":false,"activeSignatureHelp":{"signatures":[{"label":1}]}}),
        json!({"triggerKind":1,"isRetrigger":false,"activeSignatureHelp":{"signatures":[{"label":"x","parameters":[{"label":[2,1]}]}]}}),
        json!({"triggerKind":1,"isRetrigger":false,"activeSignatureHelp":{"signatures":[],"activeSignature":-1}}),
    ].into_iter().enumerate() {
        invalid.push(signature_request(&mut wire,json!(format!("context-{i}")),source,cursor,Some(context)));
    }
    for (id, position) in [
        ("split", json!({"line":0,"character":4})),
        ("crlf", json!({"line":0,"character":6})),
        ("line", json!({"line":99,"character":0})),
        ("negative", json!({"line":-1,"character":0})),
    ] {
        invalid.push(wire.send(
            json!({"jsonrpc":"2.0","id":id,"method":"textDocument/signatureHelp",
            "params":{"textDocument":{"uri":URI},"position":position}}),
        ));
    }
    let unchanged = signature_request(&mut wire, json!("unchanged"), source, cursor, None);
    let mut foreign = navigation_params(source, cursor);
    foreign["textDocument"]["uri"] = json!("untitled:foreign");
    let foreign = wire.send(json!({"jsonrpc":"2.0","id":"foreign","method":"textDocument/signatureHelp","params":foreign}));
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
    let closed = signature_request(&mut wire, json!("closed"), source, cursor, None);
    wire.finish();
    for response in invalid.into_iter().chain([foreign, closed]) {
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(response.get("result").is_none());
    }
    signature_assert_source(&original, &unchanged);
    let package = Package::new(false);
    let mut wire = SignatureWire::new(&package);
    let initialized = signature_open(&mut wire, source, json!({}));
    let unavailable = signature_request(&mut wire, json!("unavailable"), source, cursor, None);
    wire.finish();
    assert!(
        initialized["result"]["capabilities"]
            .get("signatureHelpProvider")
            .is_none()
    );
    assert_eq!(unavailable["error"]["code"], -32601);
}

#[test]
fn manual_signature_help_actual_pending_work_keeps_owned_join_and_incarnation_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let source = "fun <T> outer(p:T){ fun send(to:T){}; send(to=p) }";
    let cursor = source.rfind("send(").unwrap() + 5;
    let producer = signature_producer_packet(source, 7, cursor);
    for action in [
        "cancel",
        "change",
        "reopen",
        "rejected-change",
        "rejected-range",
        "shutdown",
        "close",
    ] {
        let mut session = running_rename();
        let open_message = json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":URI,"languageId":"kotlin","version":7,"text":source}}});
        session.handle(open_message.clone());
        let request_message = json!({"jsonrpc":"2.0","id":"help-owned","method":"textDocument/signatureHelp",
            "params":navigation_params(source,cursor)});
        let initial = session.handle(request_message.clone());
        let before = session.pending_requests();
        let mutation = match action {
            "cancel" => {
                json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"help-owned"}})
            }
            "change" | "rejected-change" | "rejected-range" => {
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":URI,"version":if action=="rejected-change"{7}else{8}},
                "contentChanges":if action=="rejected-range" {
                    json!([{"range":{"start":{"line":99,"character":0},"end":{"line":99,"character":1}},"text":"bad"}])
                } else { json!([{"text":source}]) }}})
            }
            "reopen" => {
                json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}})
            }
            "shutdown" => json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown"}),
            _ => Value::Null,
        };
        let mut responses = if action == "close" {
            Vec::new()
        } else {
            session.handle(mutation.clone())
        };
        let retained = session.pending_requests();
        if action == "reopen" {
            session.handle(open_message.clone());
        }
        let fresh_request = json!({"jsonrpc":"2.0","id":"help-reopened","method":"textDocument/signatureHelp",
            "params":navigation_params(source,cursor)});
        let mut fresh = Vec::new();
        if action == "reopen" {
            fresh.extend(session.handle(fresh_request.clone()));
            let deadline = Instant::now() + Duration::from_secs(3);
            while fresh.is_empty() && Instant::now() < deadline {
                fresh.extend(session.poll());
                if fresh.is_empty() {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        }
        let (closing, clean) = if action.starts_with("rejected-") {
            responses.extend(collect_owned(&mut session));
            session.close()
        } else {
            session.close()
        };
        responses.extend(closing);
        let after = session.pending_requests();
        session.drain_errors();
        assert!(initial.is_empty());
        assert_eq!(before, 1);
        assert_eq!(retained, 1, "work remains owned until actual join");
        assert!(clean);
        assert_eq!(after, 0);
        let help = responses
            .iter()
            .filter(|response| response["id"] == "help-owned")
            .collect::<Vec<_>>();
        assert_eq!(help.len(), 1, "{responses:?}");
        if action.starts_with("rejected-") {
            signature_assert_source(&producer, help[0]);
        } else {
            assert_eq!(
                help[0]["error"]["code"],
                if matches!(action, "change" | "reopen") {
                    -32801
                } else {
                    -32800
                }
            );
            assert!(help[0].get("result").is_none());
        }
        if action == "shutdown" {
            assert_eq!(
                responses
                    .iter()
                    .filter(|response| response["id"] == "shutdown")
                    .count(),
                1
            );
        }
        if action == "reopen" {
            assert_eq!(fresh.len(), 1, "{fresh:?}");
            assert_eq!(fresh[0]["id"], "help-reopened");
            signature_assert_source(&producer, &fresh[0]);
        }
    }
}

// Kept in this integration target so the packaged binaries, transport and
// exclusive Package cleanup are the same ones exercised by document symbols.
mod workspace_symbols {
    use super::*;
    use ktlsp_import::source_inventory::{Budget, Limits, load};
    use std::sync::atomic::AtomicBool;

    // Reuse the real child and PR306 cleanup, permitting concurrent writes.
    struct SignatureWire {
        inner: super::SignatureWire,
    }
    impl SignatureWire {
        fn new(package: &Package) -> Self {
            Self {
                inner: super::SignatureWire::new(package),
            }
        }
        fn send(&mut self, message: Value) -> Value {
            self.inner.send(message)
        }
        fn batch(&mut self, messages: &[Value]) {
            let input = self.inner.input.as_mut().unwrap();
            for message in messages {
                ktlsp_protocol::write_frame(input, &serde_json::to_vec(message).unwrap()).unwrap();
            }
            input.flush().unwrap();
        }
        fn read(&mut self) -> Value {
            let frame = ktlsp_protocol::read_frame(&mut self.inner.reader)
                .unwrap()
                .unwrap();
            let response: Value = serde_json::from_slice(&frame).unwrap();
            self.inner.responses.push(response.clone());
            response
        }
        fn terminal(mut self, action: &str) -> Vec<Value> {
            match action {
                "shutdown" => self.batch(&[
                    json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
                    json!({"jsonrpc":"2.0","method":"exit"}),
                ]),
                "exit" => self.batch(&[json!({"jsonrpc":"2.0","method":"exit"})]),
                "eof" => {}
                _ => panic!("unknown terminal fixture"),
            }
            self.inner.input.take();
            while let Some(frame) = ktlsp_protocol::read_frame(&mut self.inner.reader).unwrap() {
                self.inner
                    .responses
                    .push(serde_json::from_slice(&frame).unwrap());
            }
            let output = self.inner.child.take().unwrap().wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(if action == "shutdown" { 0 } else { 1 })
            );
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            std::mem::take(&mut self.inner.responses)
        }
        fn broken_output(mut self) {
            struct OwnedChild(Option<std::process::Child>);
            impl Drop for OwnedChild {
                fn drop(&mut self) {
                    if let Some(child) = self.0.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
            let mut child = OwnedChild(self.inner.child.take());
            let mut input = self.inner.input.take().unwrap();
            // Close ONLY stdout's read end, retaining the real child and stdin.
            drop(self.inner);
            let force = json!({"jsonrpc":"2.0","id":"forcewrite","method":"unknown","params":{}});
            ktlsp_protocol::write_frame(&mut input, &serde_json::to_vec(&force).unwrap()).unwrap();
            input.flush().unwrap();
            let output = child.0.take().unwrap().wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(
                !output.stderr.is_empty(),
                "broken output must report transport failure"
            );
            drop(input); // stdin stayed open through actual child termination.
        }
        fn finish(self) -> Vec<Value> {
            self.inner.finish()
        }
        fn finish_expected_errors(mut self, expected: &[&str]) -> Vec<Value> {
            self.send(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
            self.send(json!({"jsonrpc":"2.0","method":"exit"}));
            self.inner.input.take();
            while let Some(frame) = ktlsp_protocol::read_frame(&mut self.inner.reader).unwrap() {
                self.inner
                    .responses
                    .push(serde_json::from_slice(&frame).unwrap());
            }
            let output = self.inner.child.take().unwrap().wait_with_output().unwrap();
            assert!(output.status.success(), "{:?}", output.status);
            let stderr = std::str::from_utf8(&output.stderr).unwrap();
            assert_eq!(stderr.lines().collect::<Vec<_>>(), expected);
            std::mem::take(&mut self.inner.responses)
        }
    }

    fn query(id: Value, text: &str) -> Value {
        json!({"jsonrpc":"2.0","id":id,"method":"workspace/symbol","params":{"query":text}})
    }
    fn initialization(options: &Value, capabilities: Value) -> Value {
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{
            "initializationOptions":options,"capabilities":capabilities}})
    }
    fn full_profile() -> Value {
        json!({"workspace":{"symbol":{"symbolKind":{"valueSet":(1..=26).collect::<Vec<_>>()}}},
            "textDocument":{"diagnostic":{},"documentSymbol":{"symbolKind":{"valueSet":[1]}}}})
    }
    fn begin(wire: &mut SignatureWire, options: &Value, capabilities: Value) -> Value {
        let response = wire.send(initialization(options, capabilities));
        assert_eq!(
            response["result"]["capabilities"]["workspaceSymbolProvider"],
            true
        );
        assert!(
            response["result"]["capabilities"]
                .get("workspaceSymbol")
                .is_none()
        );
        wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
        response
    }
    fn result(response: &Value, expected: &Value) {
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(&response["result"], expected, "{response}");
    }
    fn refused(response: &Value, code: i32) {
        assert_eq!(response["error"]["code"], code, "{response}");
        assert!(response.get("result").is_none(), "{response}");
    }
    fn symbol(
        source: &str,
        uri: &str,
        name: &str,
        declaration: &str,
        kind: u32,
        owner: Option<&str>,
    ) -> Value {
        let mut value = json!({"name":name,"kind":kind,"location":{"uri":uri,"range":span(source,declaration)}});
        if let Some(owner) = owner {
            value["containerName"] = json!(owner);
        }
        value
    }
    fn single(source: &str, uri: &str, name: &str) -> Value {
        json!([symbol(source, uri, name, source.trim_end(), 5, None)])
    }
    fn filtered(all: &Value, text: &str) -> Value {
        Value::Array(
            all.as_array()
                .unwrap()
                .iter()
                .filter(|item| item["name"].as_str().unwrap().contains(text))
                .cloned()
                .collect(),
        )
    }
    fn opened(wire: &mut SignatureWire, uri: &str, text: &str) {
        wire.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
            "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":text}}}),
        );
    }
    fn closed(wire: &mut SignatureWire, uri: &str) {
        wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}));
    }
    fn reload(wire: &mut SignatureWire, id: i32, options: &Value) -> Value {
        wire.send(source_reload(id, options))
    }
    fn producer(source: &str, uri: &str, revision: i64, expected: &Value) {
        use ktlsp_parser::{FileKind, Parser, SourceSnapshot};
        // Same authenticated snapshot and supervised cleanup as the existing
        // signature producer; semantic lowering is not the outline inventory.
        let snapshot = SourceSnapshot::new(uri, revision, FileKind::Source, source).unwrap();
        let (parsed, clean) = Parser::new(worker())
            .unwrap()
            .parse_snapshot_with_cleanup(&snapshot, &AtomicBool::new(false));
        assert!(clean, "actual producer must prove child cleanup");
        let parsed = parsed.unwrap();
        let tree = parsed.tree_for(&snapshot).unwrap();
        assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
        for symbol in expected.as_array().unwrap() {
            assert_eq!(symbol["location"]["uri"], uri);
            let range: Range = serde_json::from_value(symbol["location"]["range"].clone()).unwrap();
            let start = position_to_offset(source, range.start).unwrap();
            let end = position_to_offset(source, range.end).unwrap();
            assert!(
                tree.elements()
                    .iter()
                    .any(|node| !node.token && node.start == start && node.end == end),
                "full declaration range must be a genuine CST node: {symbol}"
            );
            let spelling = symbol["name"].as_str().unwrap();
            assert!(
                tree.elements().iter().any(|token| token.token
                    && token.start >= start
                    && token.end <= end
                    && source.get(token.start..token.end) == Some(spelling)),
                "projected spelling must originate in an actual producer token: {symbol}"
            );
        }
    }

    struct Fixture {
        options: Value,
        expected: Value,
    }
    fn fixture(package: &Package, modern: bool) -> Fixture {
        let first = source_project(package, "first", &["src", "src/nested", "tests"]);
        let second = source_project(package, "second", &["lib"]);
        let mut model = source_model(&["src", "src/nested"]);
        model["modules"].as_array_mut().unwrap().push(json!({
            "name":"tests","dependencies":[{"type":"moduleSource"}],
            "contentRoots":[{"path":"<WORKSPACE>/","sourceRoots":[{"path":"<WORKSPACE>/tests","type":"kotlin-test"}]}]
        }));
        std::fs::write(
            first.join("workspace.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        let a = "class Ω {\r\n    fun `δ 😀`() = 1\r\n    val text = \"fun phantom() {}\"\r\n    class Nested\r\n}\r\n";
        let b = "object Same {}\nenum class Mode { READY, DONE }\nfun Same() = 1\n";
        let c = "interface Same {}\ntypealias Alias=String\n";
        let d = "class Same\nfun lowerCase() = 1\n";
        for (project, file, source) in [
            (&first, "src/空 😀.kt", a),
            (&first, "src/nested/B.kt", b),
            (&first, "tests/Test.kt", c),
            (&second, "lib/D.kt", d),
        ] {
            std::fs::write(project.join(file), source).unwrap();
        }
        std::fs::write(first.join("src/Ignored.java"), "class JavaGhost {}").unwrap();
        std::fs::write(first.join("src/Ignored.kts"), "class ScriptGhost").unwrap();
        let first_uri = source_directory_uri(&first);
        let second_uri = source_directory_uri(&second);
        let uris = [
            format!("{first_uri}/src/%E7%A9%BA%20%F0%9F%98%80.kt"),
            format!("{first_uri}/src/nested/B.kt"),
            format!("{first_uri}/tests/Test.kt"),
            format!("{second_uri}/lib/D.kt"),
        ];
        let declarations = [
            json!([
                symbol(a, &uris[0], "Ω", a.trim_end(), 5, None),
                symbol(a, &uris[0], "`δ 😀`", "fun `δ 😀`() = 1", 6, Some("Ω")),
                symbol(
                    a,
                    &uris[0],
                    "text",
                    "val text = \"fun phantom() {}\"",
                    7,
                    Some("Ω")
                ),
                symbol(a, &uris[0], "Nested", "class Nested", 5, Some("Ω"))
            ]),
            json!([
                symbol(
                    b,
                    &uris[1],
                    "Same",
                    "object Same {}",
                    if modern { 19 } else { 5 },
                    None
                ),
                symbol(
                    b,
                    &uris[1],
                    "Mode",
                    "enum class Mode { READY, DONE }",
                    10,
                    None
                ),
                symbol(
                    b,
                    &uris[1],
                    "READY",
                    "READY,",
                    if modern { 22 } else { 10 },
                    Some("Mode")
                ),
                symbol(
                    b,
                    &uris[1],
                    "DONE",
                    "DONE",
                    if modern { 22 } else { 10 },
                    Some("Mode")
                ),
                symbol(b, &uris[1], "Same", "fun Same() = 1", 12, None)
            ]),
            json!([
                symbol(c, &uris[2], "Same", "interface Same {}", 11, None),
                symbol(c, &uris[2], "Alias", "typealias Alias=String", 5, None)
            ]),
            json!([
                symbol(d, &uris[3], "Same", "class Same", 5, None),
                symbol(d, &uris[3], "lowerCase", "fun lowerCase() = 1", 12, None)
            ]),
        ];
        let paths = [
            first.join("src/空 😀.kt"),
            first.join("src/nested/B.kt"),
            first.join("tests/Test.kt"),
            second.join("lib/D.kt"),
        ];
        let mut budget = Budget::new(Limits::default()).unwrap();
        let cancelled = AtomicBool::new(false);
        let mut expected = Vec::new();
        for project in [&first, &second] {
            let inventory = load(project, &mut budget, &cancelled).unwrap();
            for entry in inventory.entries() {
                if let Some(source) = entry.source() {
                    let index = paths.iter().position(|path| path == entry.path()).unwrap();
                    expected.extend(declarations[index].as_array().unwrap().iter().cloned());
                    producer(source, &uris[index], 0, &declarations[index]);
                    if index == 1 {
                        assert_eq!(entry.memberships().len(), 2, "nested original associations");
                    }
                }
            }
        }
        assert_eq!(expected.len(), 13);
        Fixture {
            options: source_options(&[&first_uri, &second_uri]),
            expected: json!(expected),
        }
    }

    #[test]
    fn packaged_complete_inventory_profiles_literal_matching_and_exact_locations() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        for (profile, modern) in [
            (
                json!({"textDocument":{"diagnostic":{},"documentSymbol":{"symbolKind":{"valueSet":[1]}}}}),
                false,
            ),
            (
                json!({"workspace":{"symbol":{"symbolKind":{"valueSet":[5,6,7,10,11,12]}}},
                "textDocument":{"diagnostic":{},"documentSymbol":{"symbolKind":{"valueSet":[19,22]}}}}),
                false,
            ),
            (full_profile(), true),
        ] {
            let package = Package::new(true);
            let fixture = fixture(&package, modern);
            let mut messages = vec![
                initialization(&fixture.options, profile),
                json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            ];
            let queries = [
                "",
                "NotPresent",
                " ",
                "δ",
                "😀",
                "`",
                "`δ 😀`",
                "Same",
                "same",
                "lowerCase",
                "lowercase",
                "Ω",
                "Ω.",
                "phantom",
                "JavaGhost",
                "ScriptGhost",
                "\t",
                "e\u{301}",
            ];
            for (index, text) in queries.iter().enumerate() {
                messages.push(query(json!(index + 2), text));
            }
            messages.push(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
            messages.push(json!({"jsonrpc":"2.0","method":"exit"}));
            let replies = package.exchange(&messages);
            assert_eq!(replies.len(), queries.len() + 2);
            assert_eq!(
                replies[0]["result"]["capabilities"]["workspaceSymbolProvider"],
                true
            );
            assert_eq!(
                replies[0]["result"]["capabilities"]["positionEncoding"],
                "utf-16"
            );
            for (index, text) in queries.iter().enumerate() {
                result(
                    source_reply(&replies, (index + 2) as i32),
                    &filtered(&fixture.expected, text),
                );
            }
            let all = &source_reply(&replies, 2)["result"];
            let astral = all
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == "`δ 😀`")
                .unwrap();
            assert_eq!(
                astral["location"]["range"],
                json!({"start":{"line":1,"character":4},"end":{"line":1,"character":20}})
            );
            assert_eq!(astral["containerName"], "Ω");
            assert!(
                all.as_array()
                    .unwrap()
                    .iter()
                    .all(|s| s.get("data").is_none()
                        && s.get("children").is_none()
                        && s["location"].get("range").is_some())
            );
        }
    }

    #[test]
    fn packaged_missing_worker_and_strict_workspace_parameters_are_independent() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(false);
        let messages = [
            initialization(&json!({}), full_profile()),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            query(json!(2), ""),
            json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let replies = package.exchange(&messages);
        assert!(
            replies[0]["result"]["capabilities"]
                .get("workspaceSymbolProvider")
                .is_none()
        );
        refused(source_reply(&replies, 2), -32601);
        let package = Package::new(true);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &json!({}), full_profile());
        refused(&wire.send(query(json!("unavailable"), "")), -32803);
        result(&reload(&mut wire, 2, &source_options(&[])), &Value::Null);
        for (index, params) in [
            Value::Null,
            json!({}),
            json!({"query":null}),
            json!({"query":1}),
            json!({"query":[]}),
            json!({"query":"" ,"workDoneToken":null}),
            json!({"query":"","workDoneToken":true}),
            json!({"query":"","workDoneToken":1.5}),
            json!({"query":"","partialResultToken":{}}),
            json!({"query":"","partialResultToken":[]}),
            json!({"query":"","partialResultToken":2147483648_i64}),
        ]
        .into_iter()
        .enumerate()
        {
            let request = json!({"jsonrpc":"2.0","id":format!("bad-{index}"),"method":"workspace/symbol","params":params});
            refused(&wire.send(request), -32602);
        }
        for (index, params) in [
            json!({"query":"","workDoneToken":"work","partialResultToken":0}),
            json!({"query":"","workDoneToken":-2147483648_i64,"partialResultToken":"partial"}),
        ]
        .into_iter()
        .enumerate()
        {
            let request = json!({"jsonrpc":"2.0","id":format!("tokens-{index}"),"method":"workspace/symbol","params":params});
            result(&wire.send(request), &json!([]));
        }
        let responses = wire.finish();
        assert!(
            responses
                .iter()
                .all(|response| response.get("method").is_none()),
            "no fake progress"
        );
    }

    #[test]
    fn packaged_authoritative_alias_incremental_edits_rejections_close_and_reopen() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "overlays", &["src"]);
        let disk = "class Disk\r\n";
        let initial = "class `Ω 😀`\r\n";
        std::fs::write(project.join("src/空 😀.kt"), disk).unwrap();
        let project_uri = source_directory_uri(&project);
        let uri = format!("{project_uri}/src/%E7%A9%BA%20%F0%9F%98%80.kt");
        let alias = uri.replace("%E7", "%e7");
        let outside = format!("{project_uri}/Outside.kt");
        let options = source_options(&[&project_uri]);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &options, full_profile());
        result(
            &wire.send(query(json!("disk"), "")),
            &single(disk, &uri, "Disk"),
        );
        opened(&mut wire, &alias, initial);
        result(
            &wire.send(query(json!("alias"), "")),
            &single(initial, &uri, "`Ω 😀`"),
        );
        opened(&mut wire, &uri, "class Competing\n");
        result(
            &wire.send(query(json!("competing"), "")),
            &single(initial, &uri, "`Ω 😀`"),
        );
        let mut document = ktlsp_text::Document::new(1, initial.to_owned()).unwrap();
        let native = ktlsp_text::TextChange {
            range: Some(serde_json::from_value(span(initial, "`Ω 😀`")).unwrap()),
            range_length: Some(6),
            text: "`Edited 🦀`".into(),
        };
        document
            .apply_changes(2, std::slice::from_ref(&native))
            .unwrap();
        let change = json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":alias,"version":2},"contentChanges":[{
                "range":native.range,"rangeLength":native.range_length,"text":native.text}]}});
        wire.send(change);
        let edited = document.text();
        result(
            &wire.send(query(json!("edited"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":alias,"version":2},"contentChanges":[{"text":"class RejectedVersion"}]}}));
        result(
            &wire.send(query(json!("version-preserved"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":alias,"version":3},"contentChanges":[{
                "range":{"start":{"line":0,"character":999},"end":{"line":0,"character":999}},"text":"RejectedRange"}]}}));
        result(
            &wire.send(query(json!("range-preserved"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        opened(&mut wire, "untitled:outside", "class UntitledLeak");
        result(
            &wire.send(query(json!("nonfile-ignored"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        opened(&mut wire, &outside, "class OutsideLeak");
        refused(&wire.send(query(json!("outside-refused"), "")), -32803);
        closed(&mut wire, &outside);
        result(
            &wire.send(query(json!("outside-closed"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        closed(&mut wire, &alias);
        result(
            &wire.send(query(json!("disk-restored"), "")),
            &single(disk, &uri, "Disk"),
        );
        opened(&mut wire, &alias, edited);
        result(
            &wire.send(query(json!("identical-reopened"), "")),
            &single(edited, &uri, "`Edited 🦀`"),
        );
        closed(&mut wire, &alias);
        std::fs::write(project.join("src/空 😀.kt"), "class ChangedDisk\r\n").unwrap();
        refused(
            &wire.send(query(json!("closed-disk-stale"), "NotPresent")),
            -32801,
        );
        result(&reload(&mut wire, 3, &options), &Value::Null);
        result(
            &wire.send(query(json!("fresh-disk"), "")),
            &single("class ChangedDisk\r\n", &uri, "ChangedDisk"),
        );
        wire.finish_expected_errors(&[
            "ktlsp: an alias of this imported document is already open",
            "ktlsp: version 2 must exceed current version 2",
            "ktlsp: invalid UTF-16 position Position { line: 0, character: 999 }",
        ]);
        producer(disk, &uri, 0, &single(disk, &uri, "Disk"));
        producer(initial, &alias, 1, &single(initial, &alias, "`Ω 😀`"));
        producer(edited, &alias, 2, &single(edited, &alias, "`Edited 🦀`"));
        assert_eq!(
            std::fs::read_to_string(project.join("src/空 😀.kt")).unwrap(),
            "class ChangedDisk\r\n"
        );
    }

    #[test]
    fn packaged_nonmatching_source_and_model_mutations_invalidate_negative_answers() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "freshness", &["src"]);
        let path = project.join("src/A.kt");
        std::fs::write(&path, "class Before\n").unwrap();
        let root = source_directory_uri(&project);
        let uri = format!("{root}/src/A.kt");
        let added_uri = format!("{root}/src/Added.kt");
        let options = source_options(&[&root]);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &options, full_profile());
        result(
            &wire.send(query(json!("negative-before"), "NeverMatches")),
            &json!([]),
        );
        std::fs::write(&path, "class Changed\n").unwrap();
        refused(
            &wire.send(query(json!("changed-negative"), "NeverMatches")),
            -32801,
        );
        result(&reload(&mut wire, 2, &options), &Value::Null);
        result(
            &wire.send(query(json!("changed"), "")),
            &single("class Changed\n", &uri, "Changed"),
        );
        std::fs::remove_file(&path).unwrap();
        refused(
            &wire.send(query(json!("deleted-negative"), "NeverMatches")),
            -32801,
        );
        result(&reload(&mut wire, 3, &options), &Value::Null);
        result(
            &wire.send(query(json!("empty-after-delete"), "")),
            &json!([]),
        );
        std::fs::write(project.join("src/Added.kt"), "class Added\n").unwrap();
        result(
            &wire.send(query(json!("addition-not-admitted"), "")),
            &json!([]),
        );
        result(&reload(&mut wire, 4, &options), &Value::Null);
        result(
            &wire.send(query(json!("addition-admitted"), "")),
            &single("class Added\n", &added_uri, "Added"),
        );
        let original_model = std::fs::read(project.join("workspace.json")).unwrap();
        let mut changed_model = original_model;
        changed_model.push(b' ');
        std::fs::write(project.join("workspace.json"), &changed_model).unwrap();
        refused(
            &wire.send(query(json!("changed-model-negative"), "NeverMatches")),
            -32801,
        );
        result(&reload(&mut wire, 5, &options), &Value::Null);
        result(
            &wire.send(query(json!("model-recovered"), "")),
            &single("class Added\n", &added_uri, "Added"),
        );
        wire.send(
            json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{
            "event":{"removed":[],"added":[{"uri":root,"name":"freshness"}]}}}),
        );
        refused(
            &wire.send(query(json!("folder-invalidated"), "NeverMatches")),
            -32803,
        );
        result(&reload(&mut wire, 6, &options), &Value::Null);
        result(
            &wire.send(query(json!("folder-recovered"), "")),
            &single("class Added\n", &added_uri, "Added"),
        );
        result(&reload(&mut wire, 7, &source_options(&[])), &Value::Null);
        result(&wire.send(query(json!("clear"), "")), &json!([]));
        result(&reload(&mut wire, 8, &options), &Value::Null);
        result(
            &wire.send(query(json!("clear-recovery"), "")),
            &single("class Added\n", &added_uri, "Added"),
        );
        wire.finish();
    }

    #[test]
    fn packaged_zero_source_and_all_overlay_models_still_require_original_model_proof() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        for overlay in [false, true] {
            let package = Package::new(true);
            let project = source_project(&package, "models", &["src"]);
            let root = source_directory_uri(&project);
            let uri = format!("{root}/src/A.kt");
            if overlay {
                std::fs::write(project.join("src/A.kt"), "class Disk\n").unwrap();
            }
            let options = source_options(&[&root]);
            let mut wire = SignatureWire::new(&package);
            begin(&mut wire, &options, full_profile());
            result(
                &wire.send(query(json!("original"), "NeverMatches")),
                &json!([]),
            );
            if overlay {
                opened(&mut wire, &uri, "class Overlay\n");
                std::fs::remove_file(project.join("src/A.kt")).unwrap();
                result(
                    &wire.send(query(json!("overlay-not-disk"), "")),
                    &single("class Overlay\n", &uri, "Overlay"),
                );
            }
            let before = std::fs::read(project.join("workspace.json")).unwrap();
            let mut after = before;
            after.push(b'\n');
            std::fs::write(project.join("workspace.json"), &after).unwrap();
            refused(
                &wire.send(query(json!("model-stale"), "NeverMatches")),
                -32801,
            );
            if overlay {
                closed(&mut wire, &uri);
            }
            result(&reload(&mut wire, 2, &options), &Value::Null);
            result(&wire.send(query(json!("recovered-empty"), "")), &json!([]));
            wire.finish();
        }
    }

    #[test]
    fn packaged_late_root_project_and_parser_failure_never_return_a_prefix_or_history() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let good = source_project(&package, "good", &["src"]);
        std::fs::write(good.join("src/A.kt"), "class Good\n").unwrap();
        let late = source_project(&package, "late", &["first"]);
        std::fs::write(late.join("first/A.kt"), "class Prefix\n").unwrap();
        let late_model = source_model(&["first", "missing-last"]);
        std::fs::write(
            late.join("workspace.json"),
            serde_json::to_vec(&late_model).unwrap(),
        )
        .unwrap();
        let broken = source_project(&package, "broken", &["src"]);
        std::fs::write(broken.join("src/Z.kt"), "fun broken(").unwrap();
        let bad_project = source_project(&package, "bad-project", &["src"]);
        let mut bad_model = source_model(&["src"]);
        bad_model["modules"][0]["externalProjectPath"] = json!("/meaningful/original");
        std::fs::write(
            bad_project.join("workspace.json"),
            serde_json::to_vec(&bad_model).unwrap(),
        )
        .unwrap();
        let good_root = source_directory_uri(&good);
        let late_root = source_directory_uri(&late);
        let broken_root = source_directory_uri(&broken);
        let bad_root = source_directory_uri(&bad_project);
        let good_options = source_options(&[&good_root]);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &good_options, full_profile());
        let expected = single("class Good\n", &format!("{good_root}/src/A.kt"), "Good");
        result(&wire.send(query(json!("good"), "")), &expected);
        for (index, options, import_fails) in [
            (2, source_options(&[&late_root]), true),
            (3, source_options(&[&good_root, &bad_root]), true),
            (4, source_options(&[&good_root, &broken_root]), false),
        ] {
            let response = reload(&mut wire, index, &options);
            if import_fails {
                refused(&response, -32803);
            } else {
                result(&response, &Value::Null);
            }
            refused(
                &wire.send(query(json!(format!("no-prefix-{index}")), "")),
                -32803,
            );
            refused(
                &wire.send(query(
                    json!(format!("negative-no-prefix-{index}")),
                    "NotPresent",
                )),
                -32803,
            );
            result(&reload(&mut wire, index + 10, &good_options), &Value::Null);
            result(
                &wire.send(query(json!(format!("recovery-{index}")), "")),
                &expected,
            );
        }
        wire.finish();
    }

    #[test]
    fn packaged_workspace_replays_genuine_returned_rename_edits_in_the_same_live_overlay() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "rename", &["src"]);
        let disk = "class DiskOnly\n";
        std::fs::write(project.join("src/A.kt"), disk).unwrap();
        let root = source_directory_uri(&project);
        let uri = format!("{root}/src/A.kt");
        let source = "fun host() { val `old😀` = 1; val use = `old😀` }\r\n";
        let mut profile = full_profile();
        profile["workspace"]["workspaceEdit"] = json!({"documentChanges":true});
        profile["textDocument"]["rename"] = json!({"prepareSupport":true});
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &source_options(&[&root]), profile);
        result(
            &wire.send(query(json!("disk"), "")),
            &single(disk, &uri, "DiskOnly"),
        );
        opened(&mut wire, &uri, source);
        let expected = |text: &str, name: &str| {
            json!([
                symbol(text, &uri, "host", text.trim_end(), 12, None),
                symbol(
                    text,
                    &uri,
                    name,
                    &format!("val {name} = 1"),
                    7,
                    Some("host")
                ),
                symbol(
                    text,
                    &uri,
                    "use",
                    &format!("val use = {name}"),
                    7,
                    Some("host")
                )
            ])
        };
        result(
            &wire.send(query(json!("before"), "")),
            &expected(source, "`old😀`"),
        );
        let mut params = navigation_params(source, source.find("`old😀`").unwrap());
        params["textDocument"]["uri"] = json!(uri);
        params["newName"] = json!("Fresh");
        let rename = wire.send(
            json!({"jsonrpc":"2.0","id":"rename","method":"textDocument/rename","params":params}),
        );
        let edited = apply_original_rename(source, &rename, &uri, 1);
        assert_eq!(edited, source.replace("`old😀`", "Fresh"));
        let changes = rename["result"]["documentChanges"][0]["edits"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|edit| json!({"range":edit["range"],"text":edit["newText"]}))
            .collect::<Vec<_>>();
        wire.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":2},"contentChanges":changes}}),
        );
        result(
            &wire.send(query(json!("after"), "")),
            &expected(&edited, "Fresh"),
        );
        result(&wire.send(query(json!("old-negative"), "old")), &json!([]));
        result(
            &wire.send(query(json!("new-positive"), "Fresh")),
            &filtered(&expected(&edited, "Fresh"), "Fresh"),
        );
        assert_eq!(
            std::fs::read_to_string(project.join("src/A.kt")).unwrap(),
            disk
        );
        wire.finish();
        producer(source, &uri, 1, &expected(source, "`old😀`"));
        producer(&edited, &uri, 2, &expected(&edited, "Fresh"));
    }

    fn many_project(package: &Package, count: usize) -> (PathBuf, Value, Value) {
        let project = source_project(package, "many", &["src"]);
        let root = source_directory_uri(&project);
        let mut sources = Vec::new();
        for index in 0..count {
            let source = format!("class Member{index}\n");
            let path = project.join(format!("src/M{index}.kt"));
            std::fs::write(&path, &source).unwrap();
            sources.push(json!({"path":path,"name":format!("Member{index}"),
                "uri":format!("{root}/src/M{index}.kt")}));
        }
        let mut budget = Budget::new(Limits::default()).unwrap();
        let inventory = load(&project, &mut budget, &AtomicBool::new(false)).unwrap();
        let expected = inventory
            .entries()
            .iter()
            .filter_map(|entry| {
                entry.source().map(|source| {
                    let member = sources
                        .iter()
                        .find(|s| s["path"] == json!(entry.path()))
                        .unwrap();
                    let projected = symbol(
                        source,
                        member["uri"].as_str().unwrap(),
                        member["name"].as_str().unwrap(),
                        source.trim_end(),
                        5,
                        None,
                    );
                    producer(
                        source,
                        member["uri"].as_str().unwrap(),
                        0,
                        &json!([projected.clone()]),
                    );
                    projected
                })
            })
            .collect::<Vec<_>>();
        (project, source_options(&[&root]), json!(expected))
    }

    #[test]
    fn packaged_concurrent_workspace_cancellation_duplicate_id_and_same_id_reuse_are_real_stdio() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let (_project, options, expected) = many_project(&package, 32);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &options, full_profile());
        result(&wire.send(query(json!("baseline"), "")), &expected);
        let messages = [
            query(json!("reuse"), ""),
            query(json!("reuse"), ""),
            json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"reuse"}}),
            query(json!("reuse"), ""),
        ];
        // Genuine concurrent framed writes; no worker gates/probes or sleeps.
        wire.batch(&messages);
        let replies = [wire.read(), wire.read(), wire.read()];
        assert_eq!(replies[0]["id"], Value::Null);
        refused(&replies[0], -32600);
        assert_eq!(replies[1]["id"], "reuse");
        refused(&replies[1], -32800);
        assert_eq!(replies[2]["id"], "reuse");
        result(&replies[2], &expected);
        result(&wire.send(query(json!("recovered"), "")), &expected);
        let all = wire.finish();
        assert_eq!(
            all.iter().filter(|reply| reply["id"] == "reuse").count(),
            2,
            "no old completion after ID reuse"
        );
        assert_eq!(all.len(), 7, "no late duplicate/progress output: {all:?}");
    }

    fn session_for(options: &Value) -> Session {
        let mut session = Session::with_parser(worker()).unwrap();
        let initialized = request(
            &mut session,
            "initialize",
            json!({
            "initializationOptions":options,"capabilities":full_profile()}),
        );
        assert_eq!(
            initialized["result"]["capabilities"]["workspaceSymbolProvider"],
            true
        );
        notify(&mut session, "initialized", json!({}));
        session
    }
    fn drain(session: &mut Session) -> Vec<Value> {
        let responses = collect_owned(session);
        assert_eq!(session.pending_requests(), 0, "owned work did not join");
        responses
    }

    #[test]
    fn actual_workspace_owned_slot_retention_global_capacity_and_recovery() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let (_project, options, expected) = many_project(&package, 32);
        let mut session = session_for(&options);
        result(
            &request(&mut session, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        let mut other_session = session_for(&options);
        result(
            &request(&mut other_session, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        for index in 0..ktlsp_server::MAX_PENDING_PARSER_REQUESTS {
            let message = query(json!(format!("owned-{index}")), "");
            assert!(session.handle(message).is_empty());
        }
        let before = session.pending_requests();
        assert_eq!(before, ktlsp_server::MAX_PENDING_PARSER_REQUESTS);
        let overflow = query(json!("overflow"), "");
        let response = session.handle(overflow);
        assert_eq!(response.len(), 1);
        refused(&response[0], -32803);
        let global = query(json!("global-overflow"), "");
        let global_response = other_session.handle(global);
        // A completed handle may legitimately be reaped on this admission.
        // Never turn scheduling into a fake global-saturation assertion.
        let global_denied = !global_response.is_empty();
        if global_denied {
            assert_eq!(global_response.len(), 1);
            refused(&global_response[0], -32803);
        } else {
            assert_eq!(other_session.pending_requests(), 1);
            let cancel = json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"global-overflow"}});
            let response = other_session.handle(cancel);
            assert_eq!(response.len(), 1);
            refused(&response[0], -32800);
            assert_eq!(
                other_session.pending_requests(),
                1,
                "admitted aggregate remains owned until join"
            );
        }
        for index in 0..ktlsp_server::MAX_PENDING_PARSER_REQUESTS {
            let cancel = json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":format!("owned-{index}")}});
            let response = session.handle(cancel);
            assert_eq!(response.len(), 1);
            refused(&response[0], -32800);
        }
        let retained = session.pending_requests();
        assert_eq!(
            retained, before,
            "cancellation releases IDs, not owned aggregate slots"
        );
        let blocked_request = query(json!("owned-0"), "");
        let blocked = session.handle(blocked_request);
        assert_eq!(blocked.len(), 1);
        refused(&blocked[0], -32803);
        let late = drain(&mut session);
        assert!(late.is_empty(), "cancelled IDs replied twice: {late:?}");
        assert!(
            drain(&mut other_session).is_empty(),
            "cancelled cross-Session work replied twice"
        );
        result(
            &request(&mut other_session, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        assert!(other_session.close().1);
        result(
            &request(&mut session, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        let (closing, clean) = session.close();
        assert!(closing.is_empty());
        assert!(clean);
        assert_eq!(session.pending_requests(), 0);
        let mut recovered = session_for(&options);
        result(
            &request(&mut recovered, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        assert!(recovered.close().1);
    }

    #[test]
    fn packaged_workspace_terminal_shutdown_exit_eof_and_broken_output_keep_real_owned_cleanup() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        for action in ["shutdown", "exit", "eof", "broken-output"] {
            let package = Package::new(true);
            let (_project, options, expected) = many_project(&package, 32);
            let mut wire = SignatureWire::new(&package);
            begin(&mut wire, &options, full_profile());
            result(&wire.send(query(json!("baseline"), "")), &expected);
            wire.batch(&[query(json!("terminal-active"), "")]);
            if action == "broken-output" {
                wire.broken_output();
                continue;
            }
            let responses = wire.terminal(action);
            let active = responses
                .iter()
                .filter(|r| r["id"] == "terminal-active")
                .collect::<Vec<_>>();
            assert_eq!(active.len(), 1, "{responses:?}");
            refused(active[0], -32800);
            if action == "shutdown" {
                let stop = responses
                    .iter()
                    .filter(|r| r["id"] == "stop")
                    .collect::<Vec<_>>();
                assert_eq!(stop.len(), 1);
                result(stop[0], &Value::Null);
            }
            assert_eq!(
                responses.len(),
                if action == "shutdown" { 4 } else { 3 },
                "no duplicate late completion"
            );
        }
    }

    #[test]
    fn actual_workspace_import_waiter_cancel_duplicate_supersession_and_recovery() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let (_project, options, expected) = many_project(&package, 32);
        let mut session = session_for(&source_options(&[]));
        result(
            &request(
                &mut session,
                "intellij/reloadWorkspace",
                json!({
            "initializationOptions":source_options(&[])}),
            ),
            &Value::Null,
        );
        result(
            &request(&mut session, "workspace/symbol", json!({"query":""})),
            &json!([]),
        );
        let import = source_reload(20, &options);
        let waiter = query(json!("waiter"), "");
        let other = query(json!("other"), "");
        let cancel = json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"waiter"}});
        assert!(session.handle(import).is_empty());
        assert!(session.handle(waiter.clone()).is_empty());
        assert!(session.handle(other).is_empty());
        let duplicate = session.handle(waiter.clone());
        assert_eq!(duplicate.len(), 1);
        assert_eq!(duplicate[0]["id"], Value::Null);
        refused(&duplicate[0], -32600);
        let cancelled = session.handle(cancel);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0]["id"], "waiter");
        refused(&cancelled[0], -32800);
        assert_eq!(
            session.pending_requests(),
            2,
            "one shared import and the other waiter"
        );
        let joined = drain(&mut session);
        result(source_reply(&joined, 20), &Value::Null);
        let other_reply = joined
            .iter()
            .filter(|r| r["id"] == "other")
            .collect::<Vec<_>>();
        assert_eq!(other_reply.len(), 1);
        result(other_reply[0], &expected);
        assert_eq!(joined.len(), 2, "cancelled waiter cannot complete twice");
        assert!(session.handle(waiter).is_empty());
        let reused = drain(&mut session);
        assert_eq!(reused.len(), 1);
        result(&reused[0], &expected);
        let first = source_reload(21, &options);
        let dependent = query(json!("superseded-waiter"), "");
        assert!(session.handle(first).is_empty());
        assert!(session.handle(dependent).is_empty());
        let successor = source_reload(22, &source_options(&[]));
        let superseded = session.handle(successor);
        assert_eq!(
            superseded.len(),
            3,
            "superseded import/waiter and immediate explicit-clear result"
        );
        refused(source_reply(&superseded, 21), -32800);
        refused(
            superseded
                .iter()
                .find(|r| r["id"] == "superseded-waiter")
                .unwrap(),
            -32801,
        );
        result(source_reply(&superseded, 22), &Value::Null);
        let joined = drain(&mut session);
        assert!(
            joined.is_empty(),
            "cancelled owned import cannot publish after clear"
        );
        result(
            &request(&mut session, "workspace/symbol", json!({"query":""})),
            &json!([]),
        );
        result(
            &request(
                &mut session,
                "intellij/reloadWorkspace",
                json!({"initializationOptions":options}),
            ),
            &Value::Null,
        );
        result(
            &request(&mut session, "workspace/symbol", json!({"query":""})),
            &expected,
        );
        let (late, clean) = session.close();
        assert!(late.is_empty());
        assert!(clean);
        assert_eq!(session.pending_requests(), 0);
    }

    #[test]
    fn actual_workspace_overlay_incarnation_rejected_mutations_and_shutdown_publication() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "incarnations", &["src"]);
        let source = "class `Ω 😀`\r\n";
        std::fs::write(project.join("src/A.kt"), source).unwrap();
        let root = source_directory_uri(&project);
        let uri = format!("{root}/src/A.kt");
        let options = source_options(&[&root]);
        for action in [
            "open",
            "change",
            "close",
            "reopen",
            "folder",
            "reload",
            "rejected-version",
            "rejected-range",
            "shutdown",
            "transport-close",
        ] {
            let mut session = session_for(&options);
            result(
                &request(&mut session, "workspace/symbol", json!({"query":""})),
                &single(source, &uri, "`Ω 😀`"),
            );
            let open_message = json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}});
            if action != "open" {
                session.handle(open_message.clone());
            }
            let request_message = query(json!("pending"), "");
            assert!(session.handle(request_message).is_empty());
            assert_eq!(session.pending_requests(), 1);
            let mutation = match action {
                "change" | "rejected-version" | "rejected-range" => json!({
                    "jsonrpc":"2.0","method":"textDocument/didChange","params":{
                    "textDocument":{"uri":uri,"version":if action=="rejected-version" {1} else {2}},
                    "contentChanges":if action=="rejected-range" {
                        json!([{"range":{"start":{"line":0,"character":10},"end":{"line":0,"character":10}},"text":"bad"}])
                    } else {json!([{"text":source}])}}}),
                "close" | "reopen" => {
                    json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}})
                }
                "open" => open_message.clone(),
                "folder" => {
                    json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{
                    "event":{"removed":[],"added":[{"uri":root,"name":"incarnations"}]}}})
                }
                "reload" => source_reload(40, &options),
                "shutdown" => json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown"}),
                _ => Value::Null,
            };
            let mut responses = if action == "transport-close" {
                Vec::new()
            } else {
                session.handle(mutation)
            };
            let retained = session.pending_requests();
            assert_eq!(
                retained,
                if action == "reload" { 2 } else { 1 },
                "old work stays owned; reload additionally owns its actual import"
            );
            if action == "reopen" {
                session.handle(open_message);
            }
            if action.starts_with("rejected-") {
                responses.extend(drain(&mut session));
            }
            let (closing, clean) = session.close();
            responses.extend(closing);
            let relevant = responses
                .iter()
                .filter(|r| r["id"] == "pending")
                .collect::<Vec<_>>();
            assert_eq!(relevant.len(), 1, "{action}: {responses:?}");
            if action.starts_with("rejected-") {
                result(relevant[0], &single(source, &uri, "`Ω 😀`"));
            } else {
                refused(
                    relevant[0],
                    if matches!(
                        action,
                        "open" | "change" | "close" | "reopen" | "folder" | "reload"
                    ) {
                        -32801
                    } else {
                        -32800
                    },
                );
            }
            assert!(clean);
            assert_eq!(session.pending_requests(), 0);
            if action == "shutdown" {
                result(
                    responses.iter().find(|r| r["id"] == "shutdown").unwrap(),
                    &Value::Null,
                );
            }
            if action == "reload" {
                refused(source_reply(&responses, 40), -32800);
            }
            session.drain_errors();
        }
    }

    #[test]
    fn packaged_workspace_query_byte_boundary_and_kind_refusal_are_whole_request_errors() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "bounds", &["src"]);
        let root = source_directory_uri(&project);
        let options = source_options(&[&root]);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &options, full_profile());
        let exact = "δ".repeat(4096);
        assert_eq!(exact.len(), 8192);
        result(&wire.send(query(json!("exact-query"), &exact)), &json!([]));
        refused(
            &wire.send(query(json!("exceeded-query"), &(exact + "x"))),
            -32803,
        );
        result(&wire.send(query(json!("query-recovery"), "")), &json!([]));
        wire.finish();
        std::fs::write(project.join("src/A.kt"), "class ClassKind\n").unwrap();
        let mut wire = SignatureWire::new(&package);
        begin(
            &mut wire,
            &options,
            json!({"workspace":{"symbol":{"symbolKind":{"valueSet":[1]}}},
            "textDocument":{"diagnostic":{},"documentSymbol":{"symbolKind":{"valueSet":[5]}}}}),
        );
        refused(&wire.send(query(json!("unsupported-kind"), "")), -32803);
        refused(
            &wire.send(query(json!("negative-unsupported-kind"), "NotPresent")),
            -32803,
        );
        wire.finish();
        for profile in [
            json!(false),
            json!({"symbolKind":null}),
            json!({"symbolKind":{"valueSet":null}}),
            json!({"symbolKind":{"valueSet":[0]}}),
            json!({"symbolKind":{"valueSet":[-1]}}),
            json!({"symbolKind":{"valueSet":[1.5]}}),
            json!({"symbolKind":{"valueSet":["5"]}}),
            json!({"symbolKind":{"valueSet":[2147483648_i64]}}),
        ] {
            let message = initialization(
                &options,
                json!({
                "workspace":{"symbol":profile},"textDocument":{"diagnostic":{}}}),
            );
            // Invalid initialization never commits running-session authority.
            let mut session = Session::with_parser(worker()).unwrap();
            let responses = session.handle(message);
            assert_eq!(responses.len(), 1);
            refused(&responses[0], -32602);
            assert!(session.close().1);
        }
    }

    #[test]
    fn packaged_aggregate_declarations_exact_limit_and_late_exceeded_member_are_atomic() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "aggregate", &["src"]);
        let source = (0..1024)
            .map(|i| format!("val p{i} = {i}\n"))
            .collect::<String>();
        for index in 0..16 {
            std::fs::write(project.join(format!("src/M{index}.kt")), &source).unwrap();
        }
        let root = source_directory_uri(&project);
        let options = source_options(&[&root]);
        let inventory = load(
            &project,
            &mut Budget::new(Limits::default()).unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut expected = Vec::new();
        for entry in inventory.entries() {
            if let Some(bytes) = entry.source() {
                let file = entry.path().file_name().unwrap().to_str().unwrap();
                let uri = format!("{root}/src/{file}");
                for index in 0..1024 {
                    expected.push(symbol(
                        bytes,
                        &uri,
                        &format!("p{index}"),
                        &format!("val p{index} = {index}"),
                        7,
                        None,
                    ));
                }
            }
        }
        assert_eq!(expected.len(), 16384);
        let expected = json!(expected);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &options, full_profile());
        // Declaration and escaped-envelope ceilings are independent: retain
        // exact declaration admission but return sixteen complete locations.
        result(
            &wire.send(query(json!("exact-narrow"), "p1023")),
            &filtered(&expected, "p1023"),
        );
        result(
            &wire.send(query(json!("exact-negative"), "NeverMatches")),
            &json!([]),
        );
        std::fs::write(project.join("src/Overflow.kt"), "class Overflow\n").unwrap();
        result(
            &wire.send(query(json!("unadmitted-addition"), "Overflow")),
            &json!([]),
        );
        result(&reload(&mut wire, 2, &options), &Value::Null);
        refused(&wire.send(query(json!("exceeded-all"), "")), -32803);
        refused(
            &wire.send(query(json!("exceeded-negative"), "NeverMatches")),
            -32803,
        );
        std::fs::remove_file(project.join("src/Overflow.kt")).unwrap();
        result(&reload(&mut wire, 3, &options), &Value::Null);
        result(
            &wire.send(query(json!("aggregate-recovery"), "p1023")),
            &filtered(&expected, "p1023"),
        );
        wire.finish();
    }

    #[test]
    fn packaged_shared_import_waiter_cancellation_preserves_other_real_workspace_work() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let (_project, options, expected) = many_project(&package, 32);
        let mut wire = SignatureWire::new(&package);
        begin(&mut wire, &source_options(&[]), full_profile());
        result(&reload(&mut wire, 2, &source_options(&[])), &Value::Null);
        let messages = [
            source_reload(30, &options),
            query(json!("cancel-waiter"), ""),
            json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"cancel-waiter"}}),
            query(json!("other-consumer"), ""),
        ];
        wire.batch(&messages);
        let responses = [wire.read(), wire.read(), wire.read()];
        result(source_reply(&responses, 30), &Value::Null);
        let cancelled = responses
            .iter()
            .filter(|r| r["id"] == "cancel-waiter")
            .collect::<Vec<_>>();
        assert_eq!(cancelled.len(), 1);
        refused(cancelled[0], -32800);
        let other = responses
            .iter()
            .filter(|r| r["id"] == "other-consumer")
            .collect::<Vec<_>>();
        assert_eq!(other.len(), 1);
        result(other[0], &expected);
        let all = wire.finish();
        assert_eq!(all.iter().filter(|r| r["id"] == "cancel-waiter").count(), 1);
    }

    fn framed_send(wire: &mut super::SignatureWire, message: &Value) -> Value {
        let payload = serde_json::to_vec(message).unwrap();
        let input = wire.input.as_mut().unwrap();
        ktlsp_protocol::write_frame(input, &payload).unwrap();
        input.flush().unwrap();
        if let Some(id) = message.get("id") {
            loop {
                let payload = ktlsp_protocol::read_frame(&mut wire.reader)
                    .unwrap()
                    .unwrap();
                let response: Value = serde_json::from_slice(&payload).unwrap();
                let matched = response.get("id") == Some(id);
                wire.responses.push(response.clone());
                if matched || response.get("id") == Some(&Value::Null) {
                    return response;
                }
            }
        }
        Value::Null
    }

    #[test]
    fn packaged_actual_id_failure_envelopes_preserve_stale_refusal_and_recovery() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let package = Package::new(true);
        let project = source_project(&package, "id-envelope", &["src"]);
        let path = project.join("src/A.kt");
        let before = "class Before\n";
        let after = "class Changed\n";
        std::fs::write(&path, before).unwrap();
        let root = source_directory_uri(&project);
        let uri = format!("{root}/src/A.kt");
        let options = source_options(&[&root]);
        // Use the shared wire owner with null-ID refusal matching for requests
        // whose complete ID cannot fit a response.
        let mut wire = super::SignatureWire::new(&package);
        let requests = [
            initialization(&options, full_profile()),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            query(json!("baseline"), ""),
        ];
        for message in &requests {
            let response = framed_send(&mut wire, message);
            if message["id"] == "baseline" {
                result(&response, &single(before, &uri, "Before"));
            }
        }
        std::fs::write(&path, after).unwrap();
        // Two JSON quotes plus this ASCII string leave EXACTLY the frozen
        // 512-byte failure envelope, while the valid request itself fits.
        let id = "s".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES - 512 - 2);
        let message = query(json!(id), "NeverMatches");
        assert!(serde_json::to_vec(&message).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        let response = framed_send(&mut wire, &message);
        assert_eq!(response["id"], message["id"]);
        refused(&response, -32801);
        assert!(serde_json::to_vec(&response).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        let reload_message = source_reload(2, &options);
        result(&framed_send(&mut wire, &reload_message), &Value::Null);
        let before_model = std::fs::read(project.join("workspace.json")).unwrap();
        let mut after_model = before_model;
        after_model.push(b' ');
        std::fs::write(project.join("workspace.json"), &after_model).unwrap();
        let message = query(
            json!("m".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES - 512 - 2)),
            "NeverMatches",
        );
        assert!(serde_json::to_vec(&message).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        let response = framed_send(&mut wire, &message);
        assert_eq!(response["id"], message["id"]);
        refused(&response, -32801);
        assert!(serde_json::to_vec(&response).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        // The original valid-frame counterexample now refuses BEFORE source
        // work, with a compact error that can echo the complete original ID.
        let message = query(
            json!("r".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES - 128)),
            "",
        );
        assert!(serde_json::to_vec(&message).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        let response = framed_send(&mut wire, &message);
        assert_eq!(response["id"], message["id"]);
        refused(&response, -32803);
        assert_eq!(response["error"]["message"], "limit");
        assert!(serde_json::to_vec(&response).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
        // Parameter errors cannot bypass the failure-envelope admission. The
        // first frame admits its ID and retains InvalidParams; the second is
        // still valid framed input but must take the compact resource refusal.
        for (reserve, code) in [(514, -32602), (80, -32803)] {
            let message = json!({"jsonrpc":"2.0",
                "id":"q".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES - reserve),
                "method":"workspace/symbol","params":{"query":null}});
            assert!(
                serde_json::to_vec(&message).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES
            );
            let response = framed_send(&mut wire, &message);
            assert_eq!(response["id"], message["id"]);
            refused(&response, code);
            if code == -32803 {
                assert_eq!(response["error"]["message"], "limit");
            }
            assert!(
                serde_json::to_vec(&response).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES
            );
        }
        // A request without params has a smaller envelope than even the compact
        // error. Echo IDs only while the complete response fits; an unreplyable
        // ID is InvalidRequest/null, never a fatal oversized output frame.
        for (reserve, code) in [(67, -32803), (66, -32600), (53, -32600)] {
            let message = json!({"jsonrpc":"2.0",
                "id":"n".repeat(ktlsp_protocol::MAX_PAYLOAD_BYTES - reserve),
                "method":"workspace/symbol"});
            let input_bytes = serde_json::to_vec(&message).unwrap().len();
            assert!(input_bytes <= ktlsp_protocol::MAX_PAYLOAD_BYTES);
            if reserve == 53 {
                assert_eq!(input_bytes, ktlsp_protocol::MAX_PAYLOAD_BYTES);
            }
            let response = framed_send(&mut wire, &message);
            refused(&response, code);
            if code == -32600 {
                assert_eq!(response["id"], Value::Null);
                assert_eq!(
                    response["error"]["message"],
                    "workspace request id cannot fit a response"
                );
            } else {
                assert_eq!(response["id"], message["id"]);
                assert_eq!(response["error"]["message"], "limit");
                assert_eq!(
                    serde_json::to_vec(&response).unwrap().len(),
                    ktlsp_protocol::MAX_PAYLOAD_BYTES
                );
            }
            assert!(
                serde_json::to_vec(&response).unwrap().len() <= ktlsp_protocol::MAX_PAYLOAD_BYTES
            );
        }
        let reload_message = source_reload(3, &options);
        result(&framed_send(&mut wire, &reload_message), &Value::Null);
        let recovery = query(json!("same-session-recovery"), "");
        result(
            &framed_send(&mut wire, &recovery),
            &single(after, &uri, "Changed"),
        );
        let stop = json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"});
        result(&framed_send(&mut wire, &stop), &Value::Null);
        let exit = json!({"jsonrpc":"2.0","method":"exit"});
        framed_send(&mut wire, &exit);
        wire.input.take();
        while let Some(payload) = ktlsp_protocol::read_frame(&mut wire.reader).unwrap() {
            wire.responses
                .push(serde_json::from_slice(&payload).unwrap());
        }
        let output = wire.child.take().unwrap().wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output.status);
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
