use ktlsp_server::{MAX_ERROR_RECORDS, MAX_OPEN_BYTES, MAX_OPEN_DOCUMENTS, Session, State};
use serde_json::{Value, json};

fn request(session: &mut Session, id: Value, method: &str, params: Value) -> Value {
    let responses =
        session.handle(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
    assert_eq!(responses.len(), 1);
    responses.into_iter().next().unwrap()
}
fn notify(session: &mut Session, method: &str, params: Value) {
    for output in session.handle(json!({"jsonrpc":"2.0","method":method,"params":params})) {
        assert_eq!(output["method"], "textDocument/publishDiagnostics");
        assert!(output.get("id").is_none());
    }
}
fn initialize(session: &mut Session) -> Value {
    request(session, json!(1), "initialize", json!({"capabilities":{}}))
}
fn running() -> Session {
    let mut session = Session::new();
    assert!(initialize(&mut session).get("result").is_some());
    notify(&mut session, "initialized", json!({}));
    assert_eq!(session.state(), State::Running);
    session
}
fn open(session: &mut Session, uri: &str, version: i32, text: &str) {
    notify(
        session,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":version,"text":text}}),
    );
}
fn change(session: &mut Session, uri: &str, version: i32, changes: Value) {
    notify(
        session,
        "textDocument/didChange",
        json!({"textDocument":{"uri":uri,"version":version},"contentChanges":changes}),
    );
}
fn code(response: &Value) -> i64 {
    response["error"]["code"].as_i64().unwrap()
}

#[test]
fn lifecycle_states_and_honest_capabilities() {
    let mut session = Session::default();
    assert_eq!(
        code(&request(&mut session, json!(0), "hover", json!({}))),
        -32002
    );
    let result = initialize(&mut session);
    let capabilities = &result["result"]["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert_eq!(capabilities["foldingRangeProvider"], true);
    assert_eq!(capabilities["textDocumentSync"]["openClose"], true);
    assert_eq!(capabilities["textDocumentSync"]["change"], 2);
    assert_eq!(capabilities["textDocumentSync"]["save"], false);
    assert!(capabilities.get("signatureHelpProvider").is_none());
    assert_eq!(
        result["result"]["serverInfo"],
        json!({"name":"ktlsp","version":"0.1.0"})
    );
    assert_eq!(session.state(), State::Initializing);
    assert_eq!(
        code(&request(&mut session, json!(2), "shutdown", Value::Null)),
        -32002
    );
    open(&mut session, "untitled:test", 0, "ignored");
    assert_eq!(session.open_document_count(), 0);
    assert_eq!(session.drain_errors().len(), 1);
    notify(&mut session, "initialized", json!({}));
    assert_eq!(code(&initialize(&mut session)), -32600);
    for method in [
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/signatureHelp",
        "textDocument/didOpen",
    ] {
        assert_eq!(
            code(&request(&mut session, json!(3), method, json!({}))),
            -32601
        );
    }
    assert_eq!(
        request(&mut session, json!(4), "shutdown", Value::Null)["result"],
        Value::Null
    );
    assert_eq!(session.state(), State::Shutdown);
    assert_eq!(
        code(&request(&mut session, json!(5), "initialize", json!({}))),
        -32600
    );
    open(&mut session, "untitled:after-shutdown", 0, "ignored");
    assert_eq!(session.open_document_count(), 0);
    notify(&mut session, "exit", Value::Null);
    assert_eq!(session.exit_code(), Some(0));
    assert_eq!(session.state(), State::Exited);
    assert!(session.handle(json!({"bad":"after exit"})).is_empty());
}

#[test]
fn early_exit_fails_in_each_pre_shutdown_state() {
    for state in 0..3 {
        let mut session = Session::new();
        if state >= 1 {
            initialize(&mut session);
        }
        if state >= 2 {
            notify(&mut session, "initialized", json!({}));
        }
        notify(&mut session, "exit", Value::Null);
        assert_eq!(session.exit_code(), Some(1));
    }
}

#[test]
fn ids_are_preserved_without_integer_coercion() {
    let mut session = running();
    for id in [
        json!(0),
        json!(i64::MIN),
        json!(i64::MAX),
        json!(u64::MAX),
        json!(""),
        json!("😀request"),
    ] {
        let response = request(&mut session, id.clone(), "unknown", json!({}));
        assert_eq!(response["id"], id);
        assert_eq!(code(&response), -32601);
    }
    for id in [
        json!(1.0),
        json!(0.5),
        json!({}),
        json!([]),
        Value::Null,
        json!(true),
    ] {
        let response = request(&mut session, id, "unknown", json!({}));
        assert_eq!(response["id"], Value::Null);
        assert_eq!(code(&response), -32600);
    }
}

#[test]
fn malformed_envelopes_and_valid_client_responses() {
    let mut session = running();
    for message in [
        json!([]),
        json!([{"jsonrpc":"2.0","id":1,"method":"shutdown"}]),
        json!({}),
        json!({"method":"shutdown","id":2}),
        json!({"jsonrpc":"1.0","id":3,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","id":4,"method":false}),
        json!({"jsonrpc":"2.0","id":5,"method":"shutdown","result":null}),
        json!({"jsonrpc":"2.0","id":6,"error":{"code":"bad","message":"bad"}}),
    ] {
        let responses = session.handle(message);
        assert_eq!(responses.len(), 1);
        assert_eq!(code(&responses[0]), -32600);
        assert_eq!(responses[0]["id"], Value::Null);
    }
    for message in [
        json!({"jsonrpc":"2.0","id":1,"result":null}),
        json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
    ] {
        assert!(session.handle(message).is_empty());
    }
    assert_eq!(session.state(), State::Running);
}

#[test]
fn initialization_validation_is_atomic_and_retryable() {
    let invalid = [
        json!({}),
        json!({"capabilities":null}),
        json!({"capabilities":{"general":false}}),
        json!({"capabilities":{"general":{"positionEncodings":["utf-16",8]}}}),
        json!({"capabilities":{},"processId":1.5}),
        json!({"capabilities":{},"rootUri":"relative/file.kt"}),
        json!({"capabilities":{},"workspaceFolders":[{"uri":"file:///tmp","name":null}]}),
    ];
    for params in invalid {
        let mut session = Session::new();
        assert_eq!(
            code(&request(&mut session, json!(1), "initialize", params)),
            -32602
        );
        assert_eq!(session.state(), State::Uninitialized);
        assert!(session.configuration().workspace_folders.is_empty());
        assert!(initialize(&mut session).get("result").is_some());
    }
    let mut session = Session::new();
    let params = json!({"capabilities":{"general":{"positionEncodings":["utf-8","utf-16"]}},"processId":null,"rootUri":"file:///missing%20path","workspaceFolders":[{"uri":"untitled:workspace","name":"workspace"}],"unknown":{"accepted":true}});
    assert!(
        request(&mut session, json!(1), "initialize", params)
            .get("result")
            .is_some()
    );
    assert_eq!(
        session.configuration().root_uri.as_deref(),
        Some("file:///missing%20path")
    );
    assert_eq!(
        session.configuration().workspace_folders[0].uri,
        "untitled:workspace"
    );
}

#[test]
fn position_encoding_preferences_always_allow_utf16_fallback() {
    for capabilities in [
        json!({}),
        json!({"general":{}}),
        json!({"general":{"positionEncodings":[]}}),
        json!({"general":{"positionEncodings":["utf-8"]}}),
        json!({"general":{"positionEncodings":["utf-32"]}}),
        json!({"general":{"positionEncodings":["future-encoding"]}}),
        json!({"general":{"positionEncodings":["utf-8","utf-32","future-encoding"]}}),
        json!({"general":{"positionEncodings":["utf-8","utf-16"]}}),
    ] {
        let mut session = Session::new();
        let result = request(
            &mut session,
            json!("init"),
            "initialize",
            json!({"capabilities":capabilities}),
        );
        let capabilities = &result["result"]["capabilities"];
        assert_eq!(capabilities["positionEncoding"], "utf-16");
        assert_eq!(capabilities["foldingRangeProvider"], true);
        assert_eq!(capabilities["textDocumentSync"]["openClose"], true);
        assert_eq!(capabilities["textDocumentSync"]["change"], 2);
        assert_eq!(capabilities["textDocumentSync"]["save"], false);
        notify(&mut session, "initialized", json!({}));
        open(&mut session, "untitled:fallback", 1, "a😀b");
        change(
            &mut session,
            "untitled:fallback",
            2,
            json!([{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"rangeLength":2,"text":"水"}]),
        );
        assert_eq!(
            session.document("untitled:fallback").unwrap().text(),
            "a水b"
        );
        assert!(session.drain_errors().is_empty());
    }
}

#[test]
fn malformed_position_encoding_lists_are_rejected_without_initializing() {
    for (encodings, message) in [
        (Value::Null, "positionEncodings must be an array"),
        (json!(false), "positionEncodings must be an array"),
        (json!(8), "positionEncodings must be an array"),
        (json!("utf-16"), "positionEncodings must be an array"),
        (json!({}), "positionEncodings must be an array"),
        (
            json!(["utf-16", 8]),
            "positionEncodings entries must be strings",
        ),
        (json!([null]), "positionEncodings entries must be strings"),
        (json!([false]), "positionEncodings entries must be strings"),
        (json!([{}]), "positionEncodings entries must be strings"),
        (json!([[]]), "positionEncodings entries must be strings"),
    ] {
        let mut session = Session::new();
        let response = request(
            &mut session,
            json!("malformed"),
            "initialize",
            json!({"capabilities":{"general":{"positionEncodings":encodings}}}),
        );
        assert_eq!(response["id"], "malformed");
        assert_eq!(code(&response), -32602);
        assert_eq!(response["error"]["message"], message);
        assert_eq!(session.state(), State::Uninitialized);
        assert!(initialize(&mut session).get("result").is_some());
    }
}

#[test]
fn malformed_lifecycle_params_do_not_advance_state() {
    let mut session = Session::new();
    notify(&mut session, "initialized", json!({}));
    initialize(&mut session);
    notify(&mut session, "initialized", json!([]));
    assert_eq!(session.state(), State::Initializing);
    notify(&mut session, "initialized", json!({}));
    assert_eq!(
        code(&request(&mut session, json!(2), "shutdown", json!([]))),
        -32602
    );
    assert_eq!(session.state(), State::Running);
    notify(&mut session, "exit", json!("invalid"));
    assert_eq!(session.exit_code(), None);
    assert_eq!(session.drain_errors().len(), 3);
}

#[test]
fn unicode_sequential_edits_and_atomic_rollback() {
    let mut session = running();
    let uri = "untitled:unicode";
    open(&mut session, uri, -1, "a😀\r\nb");
    change(
        &mut session,
        uri,
        0,
        json!([
            {"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"rangeLength":2,"text":"x\n"},
            {"range":{"start":{"line":1,"character":0},"end":{"line":2,"character":1}},"rangeLength":3,"text":"水"}
        ]),
    );
    assert_eq!(session.document(uri).unwrap().text(), "ax\n水");
    assert_eq!(session.document(uri).unwrap().version(), 0);
    let before = session.document(uri).unwrap().clone();
    change(
        &mut session,
        uri,
        1,
        json!([{"text":"😀"},{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":2}},"text":"bad"}]),
    );
    assert_eq!(session.document(uri), Some(&before));
    assert_eq!(session.open_bytes(), before.text().len());
    assert_eq!(session.drain_errors().len(), 1);
    change(&mut session, uri, 1, json!([{"text":"accepted"}]));
    change(&mut session, uri, 2, json!([]));
    assert_eq!(session.document(uri).unwrap().version(), 2);
}

#[test]
fn duplicate_stale_unknown_close_and_save_behavior() {
    let mut session = running();
    open(&mut session, "file:///does/not/exist.kt", 5, "a");
    open(&mut session, "file:///does/not/exist.kt", 99, "bad");
    change(
        &mut session,
        "file:///does/not/exist.kt",
        5,
        json!([{"text":"stale"}]),
    );
    change(
        &mut session,
        "file:///unopened.kt",
        1,
        json!([{"text":"unknown"}]),
    );
    notify(
        &mut session,
        "textDocument/didSave",
        json!({"textDocument":{"uri":"file:///does/not/exist.kt"},"text":"not applied"}),
    );
    assert_eq!(
        session
            .document("file:///does/not/exist.kt")
            .unwrap()
            .text(),
        "a"
    );
    assert_eq!(
        session
            .document("file:///does/not/exist.kt")
            .unwrap()
            .version(),
        5
    );
    assert_eq!(session.drain_errors().len(), 3);
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":"file:///unknown"}}),
    );
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":"file:///does/not/exist.kt"}}),
    );
    assert_eq!(session.open_bytes(), 0);
    assert_eq!(session.open_document_count(), 0);
    assert!(session.drain_errors().is_empty());
}

#[test]
fn complete_notification_schemas_and_no_error_responses() {
    let mut session = running();
    open(&mut session, "untitled:test", 1, "same");
    let before = session.document("untitled:test").unwrap().clone();
    for changes in [
        json!({}),
        json!([{}]),
        json!([{"text":0}]),
        json!([{"text":"bad","range":null}]),
        json!([{"text":"bad","rangeLength":null}]),
        json!([{"text":"bad","rangeLength":-1}]),
        json!([{"text":"bad","range":{"start":{"line":-1,"character":0},"end":{"line":0,"character":0}}}]),
    ] {
        change(&mut session, "untitled:test", 2, changes);
        assert_eq!(session.document("untitled:test"), Some(&before));
    }
    for method in [
        "textDocument/didOpen",
        "textDocument/didChange",
        "textDocument/didClose",
        "textDocument/didSave",
        "$/cancelRequest",
    ] {
        for params in [Value::Null, json!([]), json!({}), json!(false)] {
            notify(&mut session, method, params);
        }
    }
    assert!(
        session
            .handle(json!({"method":"textDocument/didClose","params":{}}))
            .is_empty()
    );
    assert!(
        session
            .handle(json!({"jsonrpc":"2.0","method":4}))
            .is_empty()
    );
    assert_eq!(session.document("untitled:test"), Some(&before));
    assert_eq!(session.drain_errors().len(), 29);
}

#[test]
fn uri_generic_syntax_and_exact_identity() {
    let mut session = running();
    for uri in [
        "file:///tmp/a%20b.kt",
        "untitled:Untitled-1",
        "custom+scheme://user:pass@host:42/path?x=y#z",
        "http://[::1]:80/a",
        "x://[vF.a:b]/",
        "x:",
        "FILE:///tmp/a",
        "file:///tmp/a",
    ] {
        open(&mut session, uri, 0, "");
        assert!(session.document(uri).is_some(), "{uri}");
    }
    assert_eq!(session.open_document_count(), 8);
    for uri in [
        "",
        "relative.kt",
        "1bad:thing",
        "file:///raw space",
        "x:%",
        "x:%GG",
        "x://[bad]/",
        "x://host:abc",
        "x://a@b@c/",
        "x:/a[bad]",
        "x:/😀",
        "x:a#b#c",
        "x:\0",
    ] {
        open(&mut session, uri, 0, "");
        assert!(session.document(uri).is_none(), "{uri}");
    }
    assert_eq!(session.drain_errors().len(), 13);
}

#[test]
fn document_count_limit_and_close_recovery() {
    let mut session = running();
    for index in 0..MAX_OPEN_DOCUMENTS {
        open(&mut session, &format!("untitled:{index}"), 0, "");
    }
    open(&mut session, "untitled:excess", 0, "");
    assert_eq!(session.open_document_count(), MAX_OPEN_DOCUMENTS);
    assert!(session.document("untitled:excess").is_none());
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":"untitled:0"}}),
    );
    open(&mut session, "untitled:excess", 0, "");
    assert!(session.document("untitled:excess").is_some());
}

#[test]
fn aggregate_limit_rejects_open_and_change_without_consuming_versions() {
    let mut session = running();
    let big = "x".repeat(ktlsp_text::MAX_DOCUMENT_BYTES);
    for index in 0..8 {
        open(&mut session, &format!("untitled:{index}"), 0, &big);
    }
    assert_eq!(session.open_bytes(), MAX_OPEN_BYTES);
    open(&mut session, "untitled:empty", 0, "");
    open(&mut session, "untitled:excess", 0, "x");
    assert!(session.document("untitled:excess").is_none());
    change(&mut session, "untitled:empty", 1, json!([{"text":"x"}]));
    assert_eq!(session.document("untitled:empty").unwrap().version(), 0);
    assert_eq!(session.open_bytes(), MAX_OPEN_BYTES);
    change(&mut session, "untitled:0", 1, json!([{"text":""}]));
    change(&mut session, "untitled:empty", 1, json!([{"text":"x"}]));
    assert_eq!(session.document("untitled:empty").unwrap().text(), "x");
    assert_eq!(session.open_bytes(), MAX_OPEN_BYTES - big.len() + 1);
}

#[test]
fn notification_error_records_are_bounded_and_unknown_notifications_ignored() {
    let mut session = running();
    for _ in 0..MAX_ERROR_RECORDS + 100 {
        notify(&mut session, "textDocument/didChange", json!({}));
    }
    notify(&mut session, "unknown/notification", json!(false));
    notify(&mut session, "$/cancelRequest", json!({"id":u64::MAX}));
    let errors = session.drain_errors();
    assert_eq!(errors.len(), MAX_ERROR_RECORDS);
    assert_eq!(
        errors.last().unwrap(),
        "additional notification errors suppressed"
    );
    assert!(session.drain_errors().is_empty());
}

#[test]
fn generated_hostile_shapes_never_mutate_or_crash() {
    let values = [
        Value::Null,
        json!(true),
        json!(-1),
        json!(u64::MAX),
        json!(1.5),
        json!("😀"),
        json!([]),
        json!({}),
        json!({"uri":"untitled:unopened"}),
    ];
    let mut session = running();
    open(&mut session, "untitled:x", 0, "😀\r\n");
    let before = session.document("untitled:x").unwrap().clone();
    for value in values {
        for key in ["jsonrpc", "id", "method", "params"] {
            let mut message = json!({"jsonrpc":"2.0","id":1,"method":"unknown","params":{}});
            message[key] = value.clone();
            session.handle(message);
        }
        for method in [
            "textDocument/didOpen",
            "textDocument/didChange",
            "textDocument/didClose",
            "textDocument/didSave",
        ] {
            notify(&mut session, method, value.clone());
            notify(&mut session, method, json!({"textDocument":value}));
        }
    }
    assert_eq!(session.document("untitled:x"), Some(&before));
}

#[test]
fn document_size_change_count_version_and_language_validation() {
    let mut session = running();
    open(&mut session, "untitled:test", 0, "stable");
    let before = session.document("untitled:test").unwrap().clone();
    change(
        &mut session,
        "untitled:test",
        1,
        json!(vec![json!({"text":"bad"}); ktlsp_text::MAX_CHANGES + 1]),
    );
    assert_eq!(session.document("untitled:test"), Some(&before));
    change(
        &mut session,
        "untitled:test",
        1,
        json!(vec![json!({"text":"ok"}); ktlsp_text::MAX_CHANGES]),
    );
    assert_eq!(session.document("untitled:test").unwrap().text(), "ok");
    let oversized = "x".repeat(ktlsp_text::MAX_DOCUMENT_BYTES + 1);
    open(&mut session, "untitled:oversized", 0, &oversized);
    assert!(session.document("untitled:oversized").is_none());
    change(
        &mut session,
        "untitled:test",
        2,
        json!([{"text":oversized}]),
    );
    assert_eq!(session.document("untitled:test").unwrap().version(), 1);
    for version in [json!(i64::MAX), json!(u64::MAX), json!(1.5), Value::Null] {
        notify(
            &mut session,
            "textDocument/didChange",
            json!({"textDocument":{"uri":"untitled:test","version":version},"contentChanges":[{"text":"bad"}]}),
        );
        assert_eq!(session.document("untitled:test").unwrap().version(), 1);
    }
    for language in [json!("java"), Value::Null, json!(0)] {
        notify(
            &mut session,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":"untitled:wrong-language","languageId":language,"version":0,"text":""}}),
        );
        assert!(session.document("untitled:wrong-language").is_none());
    }
}

#[test]
fn initialized_requires_object_params_and_is_retryable() {
    let mut session = Session::new();
    initialize(&mut session);
    notify(&mut session, "initialized", Value::Null);
    session.handle(json!({"jsonrpc":"2.0","method":"initialized"}));
    assert_eq!(session.state(), State::Initializing);
    assert_eq!(session.drain_errors().len(), 2);
    notify(&mut session, "initialized", json!({"extra":"accepted"}));
    assert_eq!(session.state(), State::Running);
}
