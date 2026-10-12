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
    let mut child = Command::new(env!("CARGO_BIN_EXE_ktlsp"))
        .env("PATH", "")
        .env_remove("JAVA_HOME")
        .env_remove("JDK_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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

#[test]
fn native_subprocess_lifecycle_unicode_changes_and_stdout_cleanliness() {
    let mut messages = vec![request(json!("pre-init"), "textDocument/hover", json!({}))];
    messages.extend(start());
    messages.push(notification("textDocument/didOpen", json!({"textDocument":{"uri":"untitled:unicode","languageId":"kotlin","version":0,"text":"a😀\r\nb"}})));
    messages.push(notification("textDocument/didChange", json!({"textDocument":{"uri":"untitled:unicode","version":1},"contentChanges":[{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"rangeLength":2,"text":"水"}]})));
    messages.push(request(
        json!(u64::MAX),
        "textDocument/completion",
        json!({}),
    ));
    messages.extend(end());
    let output = execute(encode(&messages), true);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = decode(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[1]["id"], 0);
    assert_eq!(responses[2]["id"], json!(u64::MAX));
    assert_eq!(responses[2]["error"]["code"], -32601);
    assert_eq!(responses[3]["result"], Value::Null);
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
        if argument.contains("help") {
            assert!(stdout.contains("stdio"));
            assert!(stdout.contains("JVM"));
        }
        if argument.contains("version") {
            assert!(stdout.contains("0.1.0"));
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_ktlsp"))
        .arg("--unsupported")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
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
