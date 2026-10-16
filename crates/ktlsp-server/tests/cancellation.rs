//! Hosted-only under #96: this executes the real Parser supervisor with a
//! controlled failing Rust peer. It proves cancellation mechanics, not CST or
//! official Kotlin behavior; actual CST assertions remain in document_symbols.
use ktlsp_protocol::{read_frame, write_frame};
use ktlsp_server::{MAX_PENDING_PARSER_REQUESTS, Session};
use serde_json::{Value, json};
use std::{
    io::{BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Mutex,
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const URI: &str = "untitled:cancellation";
static SERIAL: Mutex<()> = Mutex::new(());
fn request(id: Value, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn notify(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}
fn symbols(id: Value) -> Value {
    request(
        id,
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":URI}}),
    )
}
fn cancel(id: Value) -> Value {
    notify("$/cancelRequest", json!({"id":id}))
}
fn open(token: &str) -> Value {
    notify(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":format!("// hold:{token}\nclass C")}}),
    )
}
fn change(version: i32, token: &str) -> Value {
    notify(
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":format!("// hold:{token}\nclass C")}]}),
    )
}
fn error(response: &Value, id: Value, code: i32) {
    assert_eq!(response["id"], id, "{response}");
    assert_eq!(response["error"]["code"], code, "{response}");
    assert!(response.get("result").is_none(), "{response}");
}
struct Package(PathBuf);
impl Package {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("ktlsp-cancel-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let package = Self(directory);
        let output = Command::new("rustc")
            .args(["--edition=2024", "-o"])
            .arg(package.worker())
            .arg(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/support/cancellation_worker.rs"),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::copy(env!("CARGO_BIN_EXE_ktlsp"), package.server()).unwrap();
        package
    }
    fn worker(&self) -> PathBuf {
        self.0.join(format!(
            "ktlsp-parser-worker{}",
            std::env::consts::EXE_SUFFIX
        ))
    }
    fn server(&self) -> PathBuf {
        self.0
            .join(format!("ktlsp{}", std::env::consts::EXE_SUFFIX))
    }
    fn started(&self, token: &str) {
        let deadline = Instant::now() + Duration::from_millis(450);
        while !self.0.join(format!("started-{token}")).exists() {
            assert!(
                Instant::now() < deadline,
                "controlled worker did not start: {token}"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn release(&self, token: &str) {
        std::fs::write(self.0.join(format!("release-{token}")), b"").unwrap();
    }
    fn session(&self, token: &str) -> Session {
        let mut session = Session::with_parser(self.worker()).unwrap();
        assert_eq!(
            session.handle(request(
                json!(0),
                "initialize",
                json!({"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}},"textDocument":{"diagnostic":{},"rename":{"prepareSupport":true}}}})
            ))[0]["result"]["capabilities"]["documentSymbolProvider"],
            true
        );
        assert!(session.handle(notify("initialized", json!({}))).is_empty());
        assert!(session.handle(open(token)).is_empty());
        session
    }
}
impl Drop for Package {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn collect(session: &mut Session) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut responses = Vec::new();
    while session.pending_requests() != 0 {
        responses.extend(session.poll());
        assert!(Instant::now() < deadline, "pending work did not finish");
        thread::sleep(Duration::from_millis(1));
    }
    responses
}

#[test]
fn controlled_worker_cancellation_ids_invalidations_shutdown_and_capacity() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let package = Package::new();
    let mut session = package.session("first");
    let exact = json!(u64::MAX);
    assert!(session.handle(symbols(exact.clone())).is_empty());
    package.started("first");
    for message in [
        cancel(json!("18446744073709551615")),
        cancel(json!(8)),
        cancel(Value::Null),
        notify("$/cancelRequest", json!({})),
        json!({"method":"$/cancelRequest","params":{"id":u64::MAX}}),
    ] {
        assert!(session.handle(message).is_empty());
    }
    assert_eq!(session.pending_requests(), 1);
    assert_eq!(session.drain_errors().len(), 3);
    let duplicate = session.handle(symbols(exact.clone()));
    error(&duplicate[0], Value::Null, -32600);
    let responses = session.handle(cancel(exact.clone()));
    assert_eq!(responses.len(), 1);
    error(&responses[0], exact.clone(), -32800);
    assert!(session.handle(cancel(exact.clone())).is_empty());
    // The cancelled job remains retained, but its response ID is reusable.
    assert!(session.handle(symbols(exact.clone())).is_empty());
    let deadline = Instant::now() + ktlsp_parser::TIMEOUT / 2;
    while session.pending_requests() > 1 {
        assert!(session.poll().is_empty());
        assert!(
            Instant::now() < deadline,
            "cancellation waited for the parser deadline"
        );
        thread::sleep(Duration::from_millis(1));
    }
    package.release("first");
    let responses = collect(&mut session);
    assert_eq!(
        responses.len(),
        1,
        "no second reply for the cancelled generation"
    );
    error(&responses[0], exact, -32803);
    assert!(session.poll().is_empty());

    for (token, mutation) in [
        ("edit", change(2, "newedit")),
        (
            "close",
            notify("textDocument/didClose", json!({"textDocument":{"uri":URI}})),
        ),
    ] {
        let mut session = package.session(token);
        assert!(session.handle(symbols(json!(token))).is_empty());
        package.started(token);
        // Rejected edits must not invalidate or cancel the authoritative snapshot.
        assert!(session.handle(change(1, "rejected")).is_empty());
        assert_eq!(session.drain_errors().len(), 1);
        assert!(session.poll().is_empty());
        let responses = session.handle(mutation);
        assert_eq!(responses.len(), 1);
        error(&responses[0], json!(token), -32801);
        if token == "close" {
            // Identical URI, version AND bytes still denote a new incarnation.
            assert!(session.handle(open(token)).is_empty());
        }
        assert!(collect(&mut session).is_empty());
        assert!(session.close().1);
    }

    for (token, terminal) in [
        (
            "shutdown",
            request(json!("shutdown-request"), "shutdown", Value::Null),
        ),
        ("exit", notify("exit", Value::Null)),
    ] {
        let mut session = package.session(token);
        assert!(session.handle(symbols(json!(token))).is_empty());
        package.started(token);
        let responses = session.handle(terminal);
        error(&responses[0], json!(token), -32800);
        if token == "shutdown" {
            assert_eq!(responses.len(), 2);
            assert_eq!(responses[1]["id"], "shutdown-request");
            assert_eq!(responses[1]["result"], Value::Null);
        }
        assert!(collect(&mut session).is_empty());
        assert!(session.close().1);
    }

    let mut session = package.session("capacity");
    for id in 0..MAX_PENDING_PARSER_REQUESTS {
        assert!(session.handle(symbols(json!(id))).is_empty());
    }
    assert_eq!(session.pending_requests(), MAX_PENDING_PARSER_REQUESTS);
    error(
        &session.handle(symbols(json!("overflow")))[0],
        json!("overflow"),
        -32803,
    );
    let (responses, clean) = session.close();
    assert_eq!(responses.len(), MAX_PENDING_PARSER_REQUESTS);
    assert!(
        responses
            .iter()
            .all(|response| response["error"]["code"] == -32800)
    );
    assert!(clean);
    assert_eq!(session.pending_requests(), 0);
    // Global thread capacity really becomes reusable after cleanup.
    let mut recovered = package.session("recovered");
    assert!(recovered.handle(symbols(json!(0))).is_empty());
    assert!(recovered.close().1);
    let mut owners = Vec::new();
    for id in 0..MAX_PENDING_PARSER_REQUESTS {
        let mut owner = package.session(&format!("owner{id}"));
        assert!(owner.handle(symbols(json!(id))).is_empty());
        owners.push(owner);
    }
    let mut next = package.session("afterdrop");
    error(
        &next.handle(symbols(json!("globaloverflow")))[0],
        json!("globaloverflow"),
        -32803,
    );
    // Dropping Sessions signals cancellation but does not free unfinished
    // global handles early. Later admission must reap before reusing capacity.
    drop(owners);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let responses = next.handle(symbols(json!("afterdrop")));
        if responses.is_empty() {
            break;
        }
        error(&responses[0], json!("afterdrop"), -32803);
        assert!(
            Instant::now() < deadline,
            "dropped Session threads did not become reclaimable"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(next.close().1);
}

struct Client {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
}
impl Client {
    fn new(package: &Package) -> Self {
        Self::with_reply_limit(package, None)
    }
    fn with_reply_limit(package: &Package, reply_limit: Option<usize>) -> Self {
        let mut child = Command::new(package.server())
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .env_remove("JDK_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        // Bounded observation channel; killing/waiting the child closes stdout.
        let (sender, replies) = mpsc::sync_channel(16);
        let reader = thread::spawn(move || {
            let mut count = 0;
            while let Some(payload) = read_frame(&mut output).unwrap() {
                if sender
                    .send(serde_json::from_slice(&payload).unwrap())
                    .is_err()
                {
                    break;
                }
                count += 1;
                if reply_limit == Some(count) {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            input,
            replies,
            reader: Some(reader),
        };
        client.send(request(
            json!(0),
            "initialize",
            json!({"capabilities":{"workspace":{"workspaceEdit":{"documentChanges":true}},"textDocument":{"diagnostic":{},"rename":{"prepareSupport":true}}}}),
        ));
        assert_eq!(
            client.reply()["result"]["capabilities"]["documentSymbolProvider"],
            true
        );
        client.send(notify("initialized", json!({})));
        client
    }
    fn send(&mut self, value: Value) {
        let input = self.input.as_mut().unwrap();
        write_frame(input, &serde_json::to_vec(&value).unwrap()).unwrap();
        input.flush().unwrap();
    }
    fn reply(&self) -> Value {
        self.replies
            .recv_timeout(Duration::from_secs(2))
            .expect("interactive stdio response timeout")
    }
    fn finish(self, expected: i32) {
        self.finish_with_errors(expected, false);
    }
    fn finish_with_errors(mut self, expected: i32, errors_expected: bool) {
        // Keep stdin open while waiting: exit must not hide a blocked reader join.
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "server failed to exit with stdin held open"
            );
            thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(status.code(), Some(expected));
        self.input.take();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
        assert!(
            self.replies.try_recv().is_err(),
            "unexpected duplicate/notification output"
        );
        let mut errors = String::new();
        std::io::Read::read_to_string(self.child.stderr.as_mut().unwrap(), &mut errors).unwrap();
        assert_eq!(!errors.is_empty(), errors_expected, "{errors}");
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The receiver is dropped by the struct; don't join a sender that might
        // be waiting on its bounded observation channel during panic cleanup.
    }
}

#[test]
fn interactive_stdio_cancels_active_work_and_cleans_shutdown_exit_and_eof() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let package = Package::new();
    let mut client = Client::new(&package);
    client.send(open("wire"));
    client.send(symbols(json!("active")));
    package.started("wire");
    client.send(cancel(json!("wrong")));
    client.send(cancel(json!("active")));
    let response = client.reply();
    error(&response, json!("active"), -32800);
    // A responsive reply while the controlled worker is still gated is the
    // consumer-visible proof, not a source/schema assertion.
    assert!(!package.0.join("release-wire").exists());
    client.send(symbols(json!("active")));
    let mut shutdown = Vec::new();
    write_frame(
        &mut shutdown,
        &serde_json::to_vec(&request(json!("stop"), "shutdown", Value::Null)).unwrap(),
    )
    .unwrap();
    client
        .input
        .as_mut()
        .unwrap()
        .write_all(&shutdown[..3])
        .unwrap();
    client.input.as_mut().unwrap().flush().unwrap();
    package.release("wire");
    // Completion must not wait for the rest of an unrelated partial header.
    error(&client.reply(), json!("active"), -32803);
    client
        .input
        .as_mut()
        .unwrap()
        .write_all(&shutdown[3..])
        .unwrap();
    client.input.as_mut().unwrap().flush().unwrap();
    assert_eq!(client.reply()["id"], "stop");
    client.send(notify("exit", Value::Null));
    client.finish(0);

    for (token, shutdown) in [("wireshutdown", true), ("wireexit", false)] {
        let mut client = Client::new(&package);
        client.send(open(token));
        client.send(symbols(json!(u64::MAX)));
        package.started(token);
        if shutdown {
            client.send(request(json!("stop"), "shutdown", Value::Null));
            error(&client.reply(), json!(u64::MAX), -32800);
            assert_eq!(client.reply()["id"], "stop");
        } else {
            client.send(notify("exit", Value::Null));
            error(&client.reply(), json!(u64::MAX), -32800);
        }
        if shutdown {
            client.send(notify("exit", Value::Null));
        }
        client.finish(if shutdown { 0 } else { 1 });
    }
    let mut client = Client::new(&package);
    client.send(open("wireeof"));
    client.send(symbols(json!("eof")));
    package.started("wireeof");
    client.input.take();
    error(&client.reply(), json!("eof"), -32800);
    client.finish(1);
    let mut broken = Client::with_reply_limit(&package, Some(1));
    broken.reader.take().unwrap().join().unwrap(); // drop the only stdout reader
    broken.send(open("brokenoutput"));
    broken.send(symbols(json!("broken")));
    package.started("brokenoutput");
    broken.send(request(json!("forcewrite"), "unknown", Value::Null));
    broken.finish_with_errors(1, true);
    #[cfg(target_os = "linux")]
    for entry in std::fs::read_dir(&package.0).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with("started-") {
            let pid: u32 = std::fs::read_to_string(entry.path())
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                !PathBuf::from(format!("/proc/{pid}")).exists(),
                "worker {pid} was not reaped"
            );
        }
    }
}

#[test]
fn navigation_uses_shared_wire_ids_cancellation_and_snapshot_guards() {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let package = Package::new();
    for (method, token) in [
        ("textDocument/definition", "definition"),
        ("textDocument/hover", "hover"),
        ("textDocument/references", "references"),
        ("textDocument/documentHighlight", "highlight"),
        ("textDocument/prepareRename", "prepare-rename"),
        ("textDocument/rename", "rename"),
    ] {
        let navigation = |id: Value| {
            request(
                id,
                method,
                json!({"textDocument":{"uri":URI},"position":{"line":1,"character":0},"context":{"includeDeclaration":true},"newName":"fresh"}),
            )
        };
        let mut client = Client::new(&package);
        client.send(open(token));
        client.send(navigation(json!(u64::MAX)));
        package.started(token);
        client.send(symbols(json!(u64::MAX)));
        error(&client.reply(), Value::Null, -32600);
        client.send(cancel(json!("18446744073709551615")));
        client.send(cancel(json!(u64::MAX)));
        error(&client.reply(), json!(u64::MAX), -32800);
        client.send(navigation(json!(u64::MAX))); // Reuse does not reuse the cancelled generation.
        client.send(request(json!("stop"), "shutdown", Value::Null));
        error(&client.reply(), json!(u64::MAX), -32800);
        assert_eq!(client.reply()["id"], "stop");
        client.send(notify("exit", Value::Null));
        client.finish(0);

        for (suffix, mutation) in [
            ("edit", change(2, "new")),
            (
                "reopen",
                notify("textDocument/didClose", json!({"textDocument":{"uri":URI}})),
            ),
        ] {
            let token = format!("{token}{suffix}");
            let mut session = package.session(&token);
            assert!(session.handle(navigation(json!("stale"))).is_empty());
            package.started(&token);
            assert!(session.handle(change(1, "rejected")).is_empty());
            assert_eq!(session.drain_errors().len(), 1);
            assert!(
                session.poll().is_empty(),
                "rejected edit leaves original work active"
            );
            let responses = session.handle(mutation);
            assert_eq!(responses.len(), 1);
            error(&responses[0], json!("stale"), -32801);
            if suffix == "reopen" {
                assert!(session.handle(open(&token)).is_empty()); // Identical bytes/version, new incarnation.
            }
            assert!(collect(&mut session).is_empty());
            assert!(session.close().1);
        }

        for (suffix, terminal) in [("exit", "exit"), ("eof", "eof")] {
            let token = format!("{token}{suffix}");
            let mut client = Client::new(&package);
            client.send(open(&token));
            client.send(navigation(json!(u64::MAX)));
            package.started(&token);
            if terminal == "exit" {
                client.send(notify("exit", Value::Null));
            } else {
                client.input.take();
            }
            error(&client.reply(), json!(u64::MAX), -32800);
            client.finish(1);
        }
    }
}
