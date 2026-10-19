//! Real native worker and actual stdio consumers. Build all three binaries first.
#[path = "support/stdio_client.rs"]
mod stdio_client;
use ktlsp_server::Session;
use ktlsp_text::{Range, offset_to_position};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use stdio_client::Client;

static SERIAL: Mutex<()> = Mutex::new(());
const URI: &str = "untitled:compiler.kt";
fn binary(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ktlsp"))
        .with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
fn request(id: Value, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}
fn initialize() -> Value {
    request(
        json!("init"),
        "initialize",
        json!({
            "initializationOptions":{"nativeAnalysis":"standalone"},
            "capabilities":{"textDocument":{"diagnostic":{}}}
        }),
    )
}
fn open(uri: &str, source: &str) -> Value {
    notification(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}),
    )
}
fn change(version: i32, changes: Value) -> Value {
    notification(
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":version},"contentChanges":changes}),
    )
}
fn diagnostic(id: Value) -> Value {
    request(
        id,
        "textDocument/diagnostic",
        json!({"textDocument":{"uri":URI}}),
    )
}
fn exchange(client: &mut Client, message: Value) -> Value {
    let id = message["id"].clone();
    client.send(message);
    let response = client.receive();
    assert_eq!(
        response["id"], id,
        "unexpected notification or response: {response}"
    );
    response
}
fn start(executable: impl AsRef<Path>) -> Client {
    let mut client = Client::new(executable);
    let initialized = exchange(&mut client, initialize());
    assert_eq!(
        initialized["result"]["capabilities"]["diagnosticProvider"],
        json!({"interFileDependencies":false,"workspaceDiagnostics":false})
    );
    client.send(notification("initialized", json!({})));
    client
}
fn finish(mut client: Client) {
    assert_eq!(
        exchange(
            &mut client,
            request(json!("shutdown"), "shutdown", Value::Null)
        )["result"],
        Value::Null
    );
    client.send(notification("exit", Value::Null));
    client.finish();
}
fn items(response: &Value) -> &[Value] {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["kind"], "full", "{response}");
    response["result"]["items"].as_array().unwrap()
}
fn span(source: &str, spelling: &str) -> Value {
    let start = source.find(spelling).unwrap();
    json!(Range {
        start: offset_to_position(source, start).unwrap(),
        end: offset_to_position(source, start + spelling.len()).unwrap(),
    })
}

#[test]
fn actual_cli_inferred_return_mismatch_clears_after_var_arithmetic_edit_and_rebinding() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut client = start(env!("CARGO_BIN_EXE_ktlsp"));
    let source = "fun inferred() = 1\r\nfun mismatch(): String { var n = inferred(); n = n + 2; return n }\r\n";
    client.send(open(URI, source));
    let wrong = exchange(&mut client, diagnostic(json!("wrong")));
    let errors = items(&wrong);
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic["source"] == "krusty"
                && diagnostic["severity"] == 1
                && diagnostic["message"].as_str().is_some_and(|message| {
                    message.contains("mismatch")
                        && message.contains("expected 'String'")
                        && message.contains("actual 'Int'")
                })
        }),
        "{wrong}"
    );
    assert!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic["source"] == "krusty")
            .all(|diagnostic| diagnostic.get("code").is_none())
    );
    client.send(change(
        2,
        json!([{"range":span(source, "String"),"text":"Int"}]),
    ));
    let corrected = exchange(&mut client, diagnostic(json!("corrected")));
    assert!(items(&corrected).is_empty(), "{corrected}");

    // The same spellings now bind different declarations and inferred types.
    let rebound = "fun inferred() = \"value\"\r\nfun mismatch(): String { var n = inferred(); n = \"replacement\"; return n }\r\n";
    client.send(change(3, json!([{"text":rebound}])));
    let fresh = exchange(&mut client, diagnostic(json!("rebound")));
    assert!(items(&fresh).is_empty(), "{fresh}");
    client.send(change(
        4,
        json!([{"range":span(rebound, "String"),"text":"Int"}]),
    ));
    let wrong_again = exchange(&mut client, diagnostic(json!("wrong-again")));
    assert!(
        items(&wrong_again).iter().any(|diagnostic| {
            diagnostic["source"] == "krusty"
                && diagnostic["severity"] == 1
                && diagnostic["message"].as_str().is_some_and(|message| {
                    message.contains("mismatch")
                        && message.contains("expected 'Int'")
                        && message.contains("actual 'String'")
                })
        }),
        "{wrong_again}"
    );
    finish(client);
}

#[test]
fn actual_cli_original_unicode_crlf_ranges_merge_structural_diagnostics_and_refuse_limits() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut client = start(env!("CARGO_BIN_EXE_ktlsp"));
    let source = "fun ordinary(): Int { var n = 1; n = n + 2; return n }\r\nfun unicode(): Int { /* 😀 */ return \"水\" }\r\n";
    client.send(open(URI, source));
    let wrong = exchange(&mut client, diagnostic(json!("unicode")));
    let mismatch = items(&wrong)
        .iter()
        .find(|diagnostic| {
            diagnostic["source"] == "krusty"
                && diagnostic["severity"] == 1
                && diagnostic["message"].as_str().is_some_and(|message| {
                    message.contains("mismatch")
                        && message.contains("expected 'Int'")
                        && message.contains("actual 'String'")
                })
        })
        .unwrap_or_else(|| panic!("{wrong}"));
    assert_eq!(mismatch["range"], span(source, "\"水\""));
    client.send(change(2, json!([{"range":mismatch["range"],"text":"3"}])));
    let fixed = exchange(&mut client, diagnostic(json!("fixed")));
    assert!(items(&fixed).is_empty(), "{fixed}");
    client.send(change(
        3,
        json!([{"text":"fun ordinary(): Int { var n = 1; n = n + 2; return n }\r\n)\r\n"}]),
    ));
    let malformed = exchange(&mut client, diagnostic(json!("malformed")));
    assert!(
        items(&malformed)
            .iter()
            .any(|diagnostic| diagnostic["source"] == "ktlsp-lexical-structural"),
        "{malformed}"
    );
    assert!(
        items(&malformed)
            .iter()
            .any(|diagnostic| diagnostic["source"] == "krusty"),
        "{malformed}"
    );
    client.send(change(
        4,
        json!([{"text":" ".repeat(ktlsp_parser::MAX_SOURCE_BYTES + 1)}]),
    ));
    let limit = exchange(&mut client, diagnostic(json!("limit")));
    assert_eq!(limit["error"]["code"], -32803, "{limit}");
    assert!(limit.get("result").is_none());
    client.send(change(
        5,
        json!([{"text":"fun recovered(): Int { var n = 1; n = n + 2; return n }"}]),
    ));
    let recovered = exchange(&mut client, diagnostic(json!("limit")));
    assert!(items(&recovered).is_empty(), "{recovered}");
    for uri in [
        "file:///compiler-script.kts",
        "untitled:compiler-script%2e%6bts",
    ] {
        client.send(open(uri, "val n = 1"));
        let script = exchange(
            &mut client,
            request(
                json!("script"),
                "textDocument/diagnostic",
                json!({"textDocument":{"uri":uri}}),
            ),
        );
        assert_eq!(script["error"]["code"], -32803);
        assert!(script.get("result").is_none());
    }
    finish(client);
}

struct Package(PathBuf);
impl Package {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("ktlsp-compiler-{}-{unique}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let package = Self(directory);
        for name in ["ktlsp", "ktlsp-parser-worker", "ktlsp-analysis-worker"] {
            let source = binary(name);
            std::fs::copy(&source, package.0.join(source.file_name().unwrap())).unwrap();
        }
        package
    }
    fn binary(&self, name: &str) -> PathBuf {
        self.0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn actual_cli_worker_disappearance_is_failure_not_empty_success() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let package = Package::new();
    let mut client = start(package.binary("ktlsp"));
    std::fs::remove_file(package.binary("ktlsp-analysis-worker")).unwrap();
    client.send(open(URI, "fun wrong(): String = 1"));
    let failed = exchange(&mut client, diagnostic(json!("failed")));
    assert_eq!(failed["error"]["code"], -32803, "{failed}");
    assert!(failed.get("result").is_none());
    finish(client);
}

fn collect(session: &mut Session) -> Vec<Value> {
    let mut responses = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while session.pending_requests() != 0 && Instant::now() < deadline {
        responses.extend(session.poll());
        if session.pending_requests() != 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert_eq!(session.pending_requests(), 0);
    responses
}

#[test]
fn real_owned_analysis_preserves_invalid_edit_ids_close_reopen_and_cancellation() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut session = Session::with_workers(
        binary("ktlsp-parser-worker"),
        binary("ktlsp-analysis-worker"),
    )
    .unwrap();
    session.handle(initialize());
    session.handle(notification("initialized", json!({})));
    let reload = session.handle(request(
        json!("reload"),
        "intellij/reloadWorkspace",
        json!({"initializationOptions":{"projects":[{"type":"unsupported","path":"untitled:project"}]}}),
    ));
    assert_eq!(reload[0]["error"]["code"], -32602);
    assert_eq!(session.pending_requests(), 0);
    let source = "fun wrong(): Int { /* 😀 */ return \"水\" }\r\n";
    assert!(session.handle(open(URI, source)).is_empty());
    let mut invalid = diagnostic(json!("invalid"));
    invalid["params"]["previousResultId"] = json!(false);
    assert_eq!(session.handle(invalid)[0]["error"]["code"], -32602);
    assert_eq!(session.pending_requests(), 0);
    assert!(session.handle(diagnostic(json!("same"))).is_empty());
    let duplicate = session.handle(diagnostic(json!("same")));
    assert_eq!(duplicate[0]["id"], Value::Null);
    assert_eq!(duplicate[0]["error"]["code"], -32600);
    let emoji = source.find('😀').unwrap();
    let mut interior = offset_to_position(source, emoji).unwrap();
    interior.character += 1;
    assert!(
        session
            .handle(change(
                2,
                json!([{"range":{"start":interior,"end":interior},"text":"x"}])
            ))
            .is_empty()
    );
    assert_eq!(session.document(URI).unwrap().text(), source);
    assert_eq!(session.document(URI).unwrap().version(), 1);
    assert_eq!(session.drain_errors().len(), 1);
    let wrong = collect(&mut session);
    assert_eq!(wrong.len(), 1);
    assert_eq!(wrong[0]["id"], "same");
    assert!(
        items(&wrong[0])
            .iter()
            .any(|diagnostic| diagnostic["source"] == "krusty")
    );

    assert!(session.handle(diagnostic(json!("same"))).is_empty());
    let closed = session.handle(notification(
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    ));
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0]["error"]["code"], -32801);
    assert!(
        session
            .handle(open(
                URI,
                "fun healthy(): Int { var n = 1; n = n + 2; return n }"
            ))
            .is_empty()
    );
    assert!(
        collect(&mut session).is_empty(),
        "closed incarnation must never publish late"
    );
    assert!(session.handle(diagnostic(json!("same"))).is_empty());
    let reopened = collect(&mut session);
    assert_eq!(reopened.len(), 1);
    assert!(items(&reopened[0]).is_empty(), "{reopened:?}");
    assert!(session.handle(diagnostic(json!("cancel"))).is_empty());
    let cancelled = session.handle(notification("$/cancelRequest", json!({"id":"cancel"})));
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0]["error"]["code"], -32800);
    assert!(collect(&mut session).is_empty());
    assert!(session.close().1);
}

#[test]
fn explicit_parser_only_constructor_retains_lexical_pull_mode() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut session = Session::with_parser(binary("ktlsp-parser-worker")).unwrap();
    let mut initialize = initialize();
    initialize["params"]["initializationOptions"] = json!({});
    session.handle(initialize);
    session.handle(notification("initialized", json!({})));
    assert!(
        session
            .handle(open(URI, "fun wrong(): String = 1"))
            .is_empty()
    );
    let response = session.handle(diagnostic(json!("lexical")));
    assert_eq!(response.len(), 1);
    assert!(items(&response[0]).is_empty());
    assert_eq!(session.pending_requests(), 0);
    assert!(session.close().1);
}

#[test]
fn installed_worker_does_not_activate_native_diagnostics_without_explicit_mode() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut client = Client::new(env!("CARGO_BIN_EXE_ktlsp"));
    let mut initialize = initialize();
    initialize["params"]["initializationOptions"] = json!({});
    assert!(exchange(&mut client, initialize).get("error").is_none());
    client.send(notification("initialized", json!({})));
    client.send(open(URI, "fun project(): String = externalDependency()"));
    let lexical = exchange(&mut client, diagnostic(json!("default")));
    assert!(items(&lexical).is_empty(), "{lexical}");
    finish(client);
}

#[test]
fn native_mode_is_literal_requires_workers_and_refuses_projects_before_work() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    for mode in [
        json!(true),
        json!(null),
        json!("workspace"),
        json!("Standalone"),
    ] {
        let mut session = Session::new();
        let mut initialize = initialize();
        initialize["params"]["initializationOptions"]["nativeAnalysis"] = mode;
        let rejected = session.handle(initialize);
        assert_eq!(rejected[0]["error"]["code"], -32602);
        assert_eq!(session.state(), ktlsp_server::State::Uninitialized);
        assert_eq!(session.pending_requests(), 0);
    }
    let mut missing = Session::with_parser(binary("ktlsp-parser-worker")).unwrap();
    assert_eq!(missing.handle(initialize())[0]["error"]["code"], -32803);
    assert_eq!(missing.state(), ktlsp_server::State::Uninitialized);
    let mut configured = Session::with_workers(
        binary("ktlsp-parser-worker"),
        binary("ktlsp-analysis-worker"),
    )
    .unwrap();
    let mut project = initialize();
    project["params"]["initializationOptions"]["projects"] =
        json!([{"type":"unsupported","path":"untitled:project"}]);
    let rejected = configured.handle(project);
    assert_eq!(rejected[0]["error"]["code"], -32602);
    assert_eq!(
        rejected[0]["error"]["message"],
        "standalone compiler analysis cannot load workspace projects"
    );
    assert_eq!(configured.state(), ktlsp_server::State::Uninitialized);
    assert_eq!(configured.pending_requests(), 0);
    let mut no_pull = initialize();
    no_pull["params"]["capabilities"] = json!({});
    assert_eq!(configured.handle(no_pull)[0]["error"]["code"], -32602);
    assert_eq!(configured.pending_requests(), 0);
}
