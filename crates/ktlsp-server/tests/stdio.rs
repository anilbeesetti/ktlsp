use ktlsp_protocol::{read_frame, write_frame};
use ktlsp_server::run;
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    process::{Command, Stdio},
    thread,
};

fn request(id: Value, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}
fn frame(payload: &[u8], output: &mut Vec<u8>) {
    write_frame(output, payload).unwrap();
}
fn encode(messages: &[Value]) -> Vec<u8> {
    let mut output = Vec::new();
    for message in messages {
        frame(&serde_json::to_vec(message).unwrap(), &mut output);
    }
    output
}
fn decode(output: &[u8]) -> Vec<Value> {
    let mut reader = Cursor::new(output);
    let mut messages = Vec::new();
    while let Some(payload) = read_frame(&mut reader).unwrap() {
        messages.push(serde_json::from_slice(&payload).unwrap());
    }
    messages
}
fn start() -> Vec<Value> {
    vec![
        request(json!(0), "initialize", json!({"capabilities":{}})),
        notification("initialized", json!({})),
    ]
}
fn end() -> Vec<Value> {
    vec![
        request(json!("shutdown"), "shutdown", Value::Null),
        notification("exit", Value::Null),
    ]
}
fn execute(input: Vec<u8>, fragmented: bool) -> std::process::Output {
    execute_with_arguments(input, fragmented, &[])
}
fn execute_with_arguments(
    input: Vec<u8>,
    fragmented: bool,
    arguments: &[&str],
) -> std::process::Output {
    let mut child = spawn(arguments);
    let mut stdin = child.stdin.take().unwrap();
    let writer = thread::spawn(move || {
        if fragmented {
            for chunk in input.chunks(3) {
                if stdin.write_all(chunk).is_err() {
                    break;
                }
            }
        } else {
            let _ = stdin.write_all(&input);
        }
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    output
}

fn spawn(arguments: &[&str]) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_ktlsp"))
        .args(arguments)
        .env("PATH", "")
        .env_remove("JAVA_HOME")
        .env_remove("JDK_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn native_subprocess_lifecycle_unicode_changes_and_stdout_cleanliness() {
    let mut messages = vec![request(json!("pre-init"), "textDocument/hover", json!({}))];
    messages.extend(start());
    messages.push(notification("textDocument/didOpen", json!({"textDocument":{"uri":"untitled:unicode","languageId":"kotlin","version":0,"text":"a😀\r\nb"}})));
    messages.push(notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:unicode","version":1},"contentChanges":[{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"rangeLength":2,"text":"水"}]})));
    messages.push(request(
        json!(u64::MAX),
        "completionItem/resolve",
        json!({}),
    ));
    messages.extend(end());
    let output = execute(encode(&messages), true);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 6);
    for response in &responses[2..4] {
        assert_eq!(response["method"], "textDocument/publishDiagnostics");
    }
    let responses: Vec<_> = responses
        .into_iter()
        .filter(|r| r.get("id").is_some())
        .collect();
    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[1]["id"], 0);
    assert_eq!(responses[2]["id"], json!(u64::MAX));
    assert_eq!(responses[2]["error"]["code"], -32601);
    assert_eq!(responses[3]["result"], Value::Null);
}

#[test]
fn explicit_stdio_and_default_launch_use_utf16_after_encoding_fallback() {
    let messages = [
        request(
            json!("init"),
            "initialize",
            json!({"capabilities":{"general":{"positionEncodings":["utf-8","utf-32","future-encoding"]}}}),
        ),
        notification("initialized", json!({})),
        notification(
            "textDocument/didOpen",
            json!({"textDocument":{"uri":"untitled:fallback","languageId":"kotlin","version":1,"text":"val s = \"😀\"; {\r\n}\r\n"}}),
        ),
        notification(
            "textDocument/didChange",
            json!({"textDocument":{"uri":"untitled:fallback","version":2},"contentChanges":[{"range":{"start":{"line":0,"character":9},"end":{"line":0,"character":11}},"rangeLength":2,"text":"x"}]}),
        ),
        request(
            json!("fold"),
            "textDocument/foldingRange",
            json!({"textDocument":{"uri":"untitled:fallback"}}),
        ),
        request(json!("shutdown"), "shutdown", Value::Null),
        notification("exit", Value::Null),
    ];
    for arguments in [&[][..], &["--stdio"][..]] {
        let output = execute_with_arguments(encode(&messages), true, arguments);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let responses = decode(&output.stdout);
        assert_eq!(responses.len(), 5);
        let capabilities = &responses[0]["result"]["capabilities"];
        assert_eq!(capabilities["positionEncoding"], "utf-16");
        assert_eq!(capabilities["textDocumentSync"]["change"], 2);
        assert_eq!(capabilities["foldingRangeProvider"], true);
        for response in &responses[1..3] {
            assert_eq!(response["method"], "textDocument/publishDiagnostics");
            assert_eq!(response["params"]["diagnostics"], json!([]));
        }
        assert_eq!(responses[3]["id"], "fold");
        assert_eq!(
            responses[3]["result"],
            json!([{"startLine":0,"endLine":1,"startCharacter":13,"endCharacter":1,"kind":"region"}])
        );
        assert_eq!(responses[4]["id"], "shutdown");
        assert_eq!(responses[4]["result"], Value::Null);
    }
}

#[test]
fn json_parse_errors_are_recoverable_including_invalid_utf8_and_depth() {
    let mut input = Vec::new();
    for payload in [b"{".as_slice(), b"", b"\xff", b"null", b"[]"] {
        frame(payload, &mut input);
    }
    let nested = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    frame(nested.as_bytes(), &mut input);
    input.extend(encode(&start()));
    input.extend(encode(&end()));
    let output = execute(input, false);
    assert!(output.status.success());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 8);
    for index in [0, 1, 2, 5] {
        assert_eq!(responses[index]["error"]["code"], -32700);
    }
    for index in [3, 4] {
        assert_eq!(responses[index]["error"]["code"], -32600);
    }
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_notifications_log_only_to_stderr_and_do_not_stop_session() {
    let mut messages = start();
    messages.extend([
        notification("textDocument/didOpen", json!({})),
        json!({"method":"textDocument/didChange","params":{}}),
        notification(
            "textDocument/didClose",
            json!({"textDocument":{"uri":"relative"}}),
        ),
        notification("unknown", json!({})),
    ]);
    messages.extend(end());
    let output = execute(encode(&messages), false);
    assert!(output.status.success());
    assert_eq!(decode(&output.stdout).len(), 2);
    let errors = String::from_utf8(output.stderr).unwrap();
    assert_eq!(errors.lines().count(), 3);
    assert!(errors.lines().all(|line| line.starts_with("ktlsp: ")));
}

#[test]
fn eof_and_early_exit_statuses_follow_shutdown_state() {
    for messages in [vec![], vec![notification("exit", Value::Null)], start()] {
        assert_eq!(execute(encode(&messages), false).status.code(), Some(1));
    }
    let mut shutdown = start();
    shutdown.push(request(json!(1), "shutdown", Value::Null));
    assert!(execute(encode(&shutdown), false).status.success());
}

#[test]
fn fatal_framing_never_attempts_resynchronization() {
    for invalid in [
        b"Content-Length: 8\r\n\r\n{}".as_slice(),
        b"Content-Length: -1\r\n\r\n",
        b"Content-Length: 999999999\r\n\r\n",
        b"Content-Length: 1\n\n0",
        b"Content-Length: ",
    ] {
        let output = execute(invalid.to_vec(), false);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let mut input = b"Content-Length: nope\r\n\r\n".to_vec();
    input.extend(encode(&start()));
    let output = execute(input, false);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn cli_help_version_and_unknown_arguments_work_without_java_path() {
    for argument in ["--help", "-h", "--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ktlsp"))
            .arg(argument)
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("ktlsp"));
        if argument.contains("version") {
            assert!(stdout.contains("0.1.0"));
        }
    }
    for arguments in [
        &["--unsupported"][..],
        &["--stdio", "--unsupported"][..],
        &["--stdio", "--stdio"][..],
        &["--help", "--stdio"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ktlsp"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn generated_framed_shapes_crash_corpus_remains_live() {
    let values = [
        Value::Null,
        json!(false),
        json!(1.5),
        json!({}),
        json!([]),
        json!("😀"),
        json!(u64::MAX),
    ];
    let mut messages = start();
    for value in values {
        for key in ["jsonrpc", "id", "method", "params"] {
            let mut message = request(json!(9), "unknown", json!({}));
            message[key] = value.clone();
            messages.push(message);
        }
    }
    messages.extend(end());
    let output = execute(encode(&messages), true);
    assert!(output.status.success());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 30);
    assert_eq!(responses.last().unwrap()["id"], "shutdown");
}

#[test]
fn write_and_flush_errors_are_controlled() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut errors = Vec::new();
    assert_eq!(
        run(&mut Cursor::new(encode(&start())), &mut Broken, &mut errors),
        1
    );
    assert!(!errors.is_empty());
    struct FlushBroken(Vec<u8>);
    impl Write for FlushBroken {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.extend(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }
    assert_eq!(
        run(
            &mut Cursor::new(encode(&start())),
            &mut FlushBroken(Vec::new()),
            &mut errors
        ),
        1
    );
}

#[test]
fn syntax_providers_work_over_fragmented_native_stdio_and_clear_after_changes_and_close() {
    let mut messages = start();
    messages.extend([
        notification("textDocument/didOpen", json!({"textDocument":{"uri":"untitled:folding","languageId":"kotlin","version":1,"text":"val s = \"😀 }\"; {\r\n}\r\n]"}})),
        request(json!("fold"), "textDocument/foldingRange", json!({"textDocument":{"uri":"untitled:folding"}})),
        notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:folding","version":2},"contentChanges":[{"range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"text":""}]})),
        notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:folding","version":3},"contentChanges":[{"text":"\"unterminated"}]})),
        notification("textDocument/didClose", json!({"textDocument":{"uri":"untitled:folding"}})),
        request(json!("closed"), "textDocument/foldingRange", json!({"textDocument":{"uri":"untitled:folding"}})),
    ]);
    messages.extend(end());
    let output = execute(encode(&messages), true);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 8);
    assert_eq!(
        responses[1]["params"]["diagnostics"][0]["code"],
        "structural"
    );
    assert_eq!(
        responses[2]["result"],
        json!([{"startLine":0,"endLine":1,"startCharacter":16,"endCharacter":1,"kind":"region"}])
    );
    assert_eq!(responses[3]["params"]["diagnostics"], json!([]));
    assert_eq!(responses[4]["params"]["diagnostics"][0]["code"], "lexical");
    assert_eq!(responses[5]["params"]["diagnostics"], json!([]));
    assert_eq!(responses[6]["error"]["code"], -32602);
    assert_eq!(responses[7]["id"], "shutdown");
}

#[test]
fn provider_limits_and_large_source_recover_over_native_stdio() {
    let mut messages = start();
    messages.extend([
        notification("textDocument/didOpen", json!({"textDocument":{"uri":"untitled:large-folding","languageId":"kotlin","version":1,"text":"{\n}\n".repeat(30_000)}})),
        request(json!("many"), "textDocument/foldingRange", json!({"textDocument":{"uri":"untitled:large-folding"}})),
        notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:large-folding","version":2},"contentChanges":[{"text":"{".repeat(1025)}]})),
        request(json!("limit"), "textDocument/foldingRange", json!({"textDocument":{"uri":"untitled:large-folding"}})),
        notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:large-folding","version":3},"contentChanges":[{"text":"{\n}\n"}]})),
        request(json!("recovered"), "textDocument/foldingRange", json!({"textDocument":{"uri":"untitled:large-folding"}})),
    ]);
    messages.extend(end());
    let output = execute(encode(&messages), false);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 8);
    assert_eq!(responses[2]["result"].as_array().unwrap().len(), 10_000);
    assert_eq!(
        responses[3]["params"]["diagnostics"][0]["code"],
        "analysis-limit"
    );
    assert_eq!(responses[4]["error"]["code"], -32803);
    assert_eq!(responses[5]["params"]["diagnostics"], json!([]));
    assert_eq!(responses[6]["result"].as_array().unwrap().len(), 1);
}

#[test]
fn real_cli_semantic_tokens_decode_full_range_and_committed_edit() {
    let uri = "untitled:semantic-wire";
    let params = json!({"textDocument":{"uri":uri}});
    let mut range = params.clone();
    range["range"] = json!({"start":{"line":1,"character":2},"end":{"line":2,"character":1}});
    let messages = [
        request(
            json!("init"),
            "initialize",
            json!({"capabilities":{"textDocument":{
                "diagnostic":{},
                "semanticTokens":{"requests":{"full":{"delta":true},"range":{}},"formats":["relative"],
                    "tokenTypes":["number","comment","keyword","operator","string"],"tokenModifiers":[]}
            }}}),
        ),
        notification("initialized", json!({})),
        notification(
            "textDocument/didOpen",
            json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":"val x = 1\r\n/*😀\rnext*/"}}),
        ),
        request(
            json!("full"),
            "textDocument/semanticTokens/full",
            params.clone(),
        ),
        request(json!(u64::MAX), "textDocument/semanticTokens/range", range),
        notification(
            "textDocument/didChange",
            json!({"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":8},"end":{"line":0,"character":9}},"text":"\"😀\""}]}),
        ),
        request(
            json!("edited"),
            "textDocument/semanticTokens/full",
            params.clone(),
        ),
        request(json!("pull"), "textDocument/diagnostic", params),
        request(json!("shutdown"), "shutdown", Value::Null),
        notification("exit", Value::Null),
    ];
    let output = execute(encode(&messages), true);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let replies = decode(&output.stdout);
    assert_eq!(
        replies.iter().map(|reply| &reply["id"]).collect::<Vec<_>>(),
        vec![
            &json!("init"),
            &json!("full"),
            &json!(u64::MAX),
            &json!("edited"),
            &json!("pull"),
            &json!("shutdown"),
        ]
    );
    assert!(replies.iter().all(|reply| reply.get("method").is_none()));
    let options = &replies[0]["result"]["capabilities"]["semanticTokensProvider"];
    assert_eq!(options["full"], true);
    assert_eq!(options["range"], true);
    let decode_tokens = |reply: &Value| {
        assert!(reply["result"].get("resultId").is_none());
        let mut line = 0;
        let mut column = 0;
        let data = reply["result"]["data"].as_array().unwrap();
        assert_eq!(data.len() % 5, 0);
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
    let full = decode_tokens(&replies[1]);
    let clipped = decode_tokens(&replies[2]);
    let edited = decode_tokens(&replies[3]);
    assert_eq!(
        full,
        vec![
            json!([0, 0, 3, "keyword"]),
            json!([0, 6, 1, "operator"]),
            json!([0, 8, 1, "number"]),
            json!([1, 0, 4, "comment"]),
            json!([2, 0, 6, "comment"])
        ]
    );
    assert_eq!(
        clipped,
        vec![json!([1, 2, 2, "comment"]), json!([2, 0, 1, "comment"])]
    );
    assert_eq!(
        edited,
        vec![
            json!([0, 0, 3, "keyword"]),
            json!([0, 6, 1, "operator"]),
            json!([0, 8, 4, "string"]),
            json!([1, 0, 4, "comment"]),
            json!([2, 0, 6, "comment"])
        ]
    );
    assert_eq!(replies[4]["result"], json!({"kind":"full","items":[]}));
}

#[test]
fn real_stdio_structural_action_applies_received_edit_and_observes_fresh_diagnostics() {
    use std::{
        io::BufReader,
        sync::mpsc,
        time::{Duration, Instant},
    };
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(spawn(&["--stdio"]));
    let mut input = child.0.stdin.take().unwrap();
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let (sender, receiver) = mpsc::sync_channel(8);
    let reader = thread::spawn(move || {
        for _ in 0..8 {
            let Some(payload) = read_frame(&mut output).unwrap() else {
                break;
            };
            let reply: Value = serde_json::from_slice(&payload).unwrap();
            if sender.send(reply).is_err() {
                break;
            }
        }
    });
    let mut send = |message: Value| {
        input
            .write_all(&encode(std::slice::from_ref(&message)))
            .unwrap();
        input.flush().unwrap();
    };
    let receive = || {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("structural action stdio reply timeout")
    };
    let uri = "untitled:fix";
    let source = "val s = \"😀\"\r\n)";
    send(request(
        json!("init"),
        "initialize",
        json!({"capabilities":{
            "workspace":{"workspaceEdit":{"documentChanges":true}},
            "textDocument":{"codeAction":{"codeActionLiteralSupport":{"codeActionKind":{"valueSet":["quickfix"]}}},
                "publishDiagnostics":{"versionSupport":true}}
        }}),
    ));
    assert_eq!(
        receive()["result"]["capabilities"]["codeActionProvider"],
        json!({"codeActionKinds":["quickfix"],"resolveProvider":false})
    );
    send(notification("initialized", json!({})));
    send(notification(
        "textDocument/didOpen",
        json!({"textDocument":{"uri":uri,"languageId":"kotlin","version":7,"text":source}}),
    ));
    let before = receive();
    assert_eq!(before["method"], "textDocument/publishDiagnostics");
    assert_eq!(before["params"]["version"], 7);
    let range = json!({"start":{"line":1,"character":0},"end":{"line":1,"character":1}});
    assert_eq!(
        before["params"]["diagnostics"],
        json!([{
            "range":range,"severity":1,"source":"ktlsp-lexical-structural",
            "code":"structural","message":"unexpected or mismatched closing delimiter"
        }])
    );
    let action_params =
        json!({"textDocument":{"uri":uri},"range":range,"context":{"diagnostics":[]}});
    send(request(
        json!("action"),
        "textDocument/codeAction",
        action_params.clone(),
    ));
    let action = receive();
    assert_eq!(action["id"], "action");
    assert_eq!(
        action["result"],
        json!([{
            "title":"Remove unmatched trailing ')'","kind":"quickfix",
            "edit":{"documentChanges":[{
                "textDocument":{"uri":uri,"version":7},"edits":[{"range":range,"newText":""}]
            }]}
        }])
    );
    let document_edit = &action["result"][0]["edit"]["documentChanges"][0];
    let edit = &document_edit["edits"][0];
    let change = ktlsp_text::TextChange {
        range: Some(serde_json::from_value(edit["range"].clone()).unwrap()),
        range_length: None,
        text: edit["newText"].as_str().unwrap().into(),
    };
    let mut applied = ktlsp_text::Document::new(
        document_edit["textDocument"]["version"].as_i64().unwrap() as i32,
        source.into(),
    )
    .unwrap();
    applied
        .apply_changes(8, std::slice::from_ref(&change))
        .unwrap();
    assert_eq!(applied.text(), "val s = \"😀\"\r\n");
    assert_eq!(applied.version(), 8);
    send(notification(
        "textDocument/didChange",
        json!({
            "textDocument":{"uri":document_edit["textDocument"]["uri"],"version":applied.version()},
            "contentChanges":[{"range":change.range,"text":change.text}]
        }),
    ));
    let fresh = receive();
    assert_eq!(fresh["method"], "textDocument/publishDiagnostics");
    assert_eq!(
        fresh["params"],
        json!({"uri":uri,"version":8,"diagnostics":[]})
    );
    // The old selection is now outside current text: use the actual remaining caret.
    let mut next = action_params;
    next["range"] = json!({"start":{"line":1,"character":0},"end":{"line":1,"character":0}});
    send(request(json!("next"), "textDocument/codeAction", next));
    let next = receive();
    assert_eq!(next["id"], "next");
    assert_eq!(next["result"], json!([]));
    send(request(json!("shutdown"), "shutdown", Value::Null));
    let shutdown = receive();
    assert_eq!(shutdown["id"], "shutdown");
    assert_eq!(shutdown["result"], Value::Null);
    send(notification("exit", Value::Null));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "structural action stdio exit timeout"
        );
        thread::sleep(Duration::from_millis(2));
    }
    reader.join().unwrap();
    assert!(receiver.try_recv().is_err());
    let mut errors = String::new();
    std::io::Read::read_to_string(child.0.stderr.as_mut().unwrap(), &mut errors).unwrap();
    assert!(errors.is_empty(), "{errors}");
}
