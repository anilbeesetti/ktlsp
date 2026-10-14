use ktlsp_protocol::{read_frame, write_frame};
use ktlsp_server::Session;
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    process::{Command, Stdio},
    thread,
};

const URI: &str = "untitled:line-comments";

fn request(session: &mut Session, method: &str, params: Value) -> Value {
    session
        .handle(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .remove(0)
}

fn notify(session: &mut Session, method: &str, params: Value) -> Vec<Value> {
    session.handle(json!({"jsonrpc":"2.0","method":method,"params":params}))
}

fn open(text: &str, capabilities: Value) -> (Session, Value) {
    let mut session = Session::new();
    assert_eq!(
        request(
            &mut session,
            "initialize",
            json!({"capabilities":capabilities})
        )["result"]["capabilities"]["foldingRangeProvider"],
        true
    );
    notify(&mut session, "initialized", json!({}));
    let published = notify(
        &mut session,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":text}}),
    );
    assert_eq!(published.len(), 1);
    assert_eq!(published[0]["method"], "textDocument/publishDiagnostics");
    (session, published[0]["params"]["diagnostics"].clone())
}

fn folding(session: &mut Session) -> Value {
    request(
        session,
        "textDocument/foldingRange",
        json!({"textDocument":{"uri":URI}}),
    )
}

fn fold(kind: &str, start_line: u32, start: u32, end_line: u32, end: u32) -> Value {
    json!({"startLine":start_line,"startCharacter":start,"endLine":end_line,"endCharacter":end,"kind":kind})
}

fn check(text: &str, expected: Value) {
    let (mut session, diagnostics) = open(text, json!({}));
    assert_eq!(diagnostics, json!([]), "{text:?}");
    assert_eq!(folding(&mut session)["result"], expected, "{text:?}");
}

#[test]
fn adjacent_comments_eof_indentation_and_utf16_have_literal_ranges() {
    for newline in ["\n", "\r\n", "\r"] {
        for suffix in ["", newline] {
            check(
                &format!("// a{newline}// b{suffix}"),
                json!([fold("comment", 0, 0, 1, 4)]),
            );
        }
    }
    for text in ["", "// a", "// a\n"] {
        check(text, json!([]));
    }
    for (text, expected) in [
        ("//\n//\n", fold("comment", 0, 0, 1, 2)),
        ("///a\n////b", fold("comment", 0, 0, 1, 5)),
        (" \t// 😀  \r\n\t// z\r//\r\n", fold("comment", 0, 2, 2, 2)),
        ("\u{c}// a\n // 😀  ", fold("comment", 0, 1, 1, 8)),
    ] {
        check(text, json!([expected]));
    }
}

#[test]
fn blank_code_inline_and_opaque_tokens_separate_groups() {
    check(
        "// a\n// b\n\n// c\ncode // inline\n// d\n// e\n",
        json!([fold("comment", 0, 0, 1, 4), fold("comment", 5, 0, 6, 4)]),
    );
    check("// a\n \t\n// b", json!([]));
    check("x // a\n// b\nx // c\n// d", json!([]));
    check("x // a\n// b\n// c", json!([fold("comment", 1, 0, 2, 4)]));
    check(
        "/* a\n*/ // inline\n// lone\n\"\"\"a\n\"\"\" // inline\n// lone",
        json!([fold("comment", 0, 0, 1, 2), fold("region", 3, 0, 4, 3)]),
    );
    check(
        "/* // a\n// b */\n\"\"\"// c\n// d\"\"\"\n\"// e\"\n",
        json!([fold("comment", 0, 0, 1, 7), fold("region", 2, 0, 3, 7)]),
    );
    check(
        "'x' // inline\n// a\n// b\n`x` // inline\n// { \"\n// } \"",
        json!([fold("comment", 1, 0, 2, 4), fold("comment", 4, 0, 5, 6)]),
    );
}

#[test]
fn first_completed_candidates_share_limits_before_output_sorting() {
    let text = "{\n // a\n // b\n /* c\n d */\n \"\"\"e\n f\"\"\"\n}\n";
    let all = json!([
        fold("region", 0, 0, 7, 1),
        fold("comment", 1, 1, 2, 5),
        fold("comment", 3, 1, 4, 5),
        fold("region", 5, 1, 6, 5)
    ]);
    check(text, all.clone());
    for (limit, expected) in [
        (0, json!([])),
        (1, json!([fold("comment", 1, 1, 2, 5)])),
        (
            2,
            json!([fold("comment", 1, 1, 2, 5), fold("comment", 3, 1, 4, 5)]),
        ),
        (
            3,
            json!([
                fold("comment", 1, 1, 2, 5),
                fold("comment", 3, 1, 4, 5),
                fold("region", 5, 1, 6, 5)
            ]),
        ),
        (4, all),
    ] {
        for line_only in [false, true] {
            let (mut session, diagnostics) = open(
                text,
                json!({"textDocument":{"foldingRange":{"rangeLimit":limit,"lineFoldingOnly":line_only}}}),
            );
            assert_eq!(diagnostics, json!([]));
            let mut expected = expected.clone();
            if line_only {
                for fold in expected.as_array_mut().unwrap() {
                    fold.as_object_mut().unwrap().remove("startCharacter");
                    fold.as_object_mut().unwrap().remove("endCharacter");
                }
            }
            assert_eq!(folding(&mut session)["result"], expected);
        }
    }
    for (text, expected) in [
        ("{\n// a\n// b\n}", fold("comment", 1, 0, 2, 4)),
        ("// a\n// b\n/* c\nd */", fold("comment", 0, 0, 1, 4)),
        ("// a\n// b\n\"\"\"c\nd\"\"\"", fold("comment", 0, 0, 1, 4)),
    ] {
        let (mut session, diagnostics) = open(
            text,
            json!({"textDocument":{"foldingRange":{"rangeLimit":1}}}),
        );
        assert_eq!(diagnostics, json!([]));
        assert_eq!(folding(&mut session)["result"], json!([expected]));
    }
}

#[test]
fn existing_kind_negotiation_limit_and_line_only_behavior_are_explicit() {
    for capabilities in [
        json!({}),
        json!({"textDocument":{"foldingRange":{"foldingRangeKind":{"valueSet":["comment","region"]}}}}),
        json!({"textDocument":{"foldingRange":{"foldingRangeKind":{"valueSet":["region"]}}}}),
    ] {
        let (mut session, diagnostics) = open("// 😀\n// z", capabilities);
        assert_eq!(diagnostics, json!([]));
        assert_eq!(
            folding(&mut session)["result"],
            json!([fold("comment", 0, 0, 1, 4)])
        );
    }
    let (mut session, diagnostics) = open(
        "// 😀\n// z",
        json!({"textDocument":{"foldingRange":{"lineFoldingOnly":true}}}),
    );
    assert_eq!(diagnostics, json!([]));
    assert_eq!(
        folding(&mut session)["result"],
        json!([{"startLine":0,"endLine":1,"kind":"comment"}])
    );
}

#[test]
fn diagnostics_survive_zero_budget_and_recover_with_versioned_changes() {
    let expected_diagnostic = json!([{
        "range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},
        "severity":1,"source":"ktlsp-lexical-structural","code":"structural",
        "message":"unexpected or mismatched closing delimiter"
    }]);
    for (capabilities, expected) in [
        (json!({}), json!([fold("comment", 0, 0, 1, 4)])),
        (
            json!({"textDocument":{"foldingRange":{"rangeLimit":0}}}),
            json!([]),
        ),
    ] {
        let (mut session, diagnostics) = open("// a\n// b\n)", capabilities);
        assert_eq!(diagnostics, expected_diagnostic);
        assert_eq!(folding(&mut session)["result"], expected);
    }
    let (mut session, _) = open("// a\n// b\n)", json!({}));
    let changed = notify(
        &mut session,
        "textDocument/didChange",
        json!({
            "textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":"// x\n// y"}]
        }),
    );
    assert_eq!(changed[0]["params"]["diagnostics"], json!([]));
    let stale = notify(
        &mut session,
        "textDocument/didChange",
        json!({
            "textDocument":{"uri":URI,"version":1},"contentChanges":[{"text":"// stale"}]
        }),
    );
    assert!(stale.is_empty());
    assert_eq!(session.document(URI).unwrap().version(), 2);
    assert_eq!(session.document(URI).unwrap().text(), "// x\n// y");
    assert_eq!(
        folding(&mut session)["result"],
        json!([fold("comment", 0, 0, 1, 4)])
    );
    let closed = notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(closed[0]["params"]["diagnostics"], json!([]));
    assert_eq!(folding(&mut session)["error"]["code"], -32602);
}

#[test]
fn many_groups_long_group_and_long_indentation_keep_bounded_output() {
    let (mut session, diagnostics) = open(&"// a\n// b\n\n".repeat(10_001), json!({}));
    assert_eq!(diagnostics, json!([]));
    let response = folding(&mut session);
    let folds = response["result"].as_array().unwrap();
    assert_eq!(folds.len(), 10_000);
    assert_eq!(folds[0], fold("comment", 0, 0, 1, 4));
    assert_eq!(folds[9999], fold("comment", 29997, 0, 29998, 4));
    check(
        &"// a\n".repeat(20_000),
        json!([fold("comment", 0, 0, 19999, 4)]),
    );
    check(
        &format!("{}// 😀\n// z", "\t".repeat(200_000)),
        json!([fold("comment", 0, 200000, 1, 4)]),
    );
}

#[test]
fn fragmented_native_stdio_folds_and_replaces_comment_groups_without_java() {
    let messages = [
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{"textDocument":{"foldingRange":{"lineFoldingOnly":true,"rangeLimit":1}}}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"{\r\n// 😀\r\n// z\r\n}"}}}),
        json!({"jsonrpc":"2.0","id":"comments","method":"textDocument/foldingRange","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI,"version":2},"contentChanges":[{"text":"// one\n"}]}}),
        json!({"jsonrpc":"2.0","id":"single","method":"textDocument/foldingRange","params":{"textDocument":{"uri":URI}}}),
        json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown","params":null}),
        json!({"jsonrpc":"2.0","method":"exit","params":null}),
    ];
    let mut input = Vec::new();
    for message in messages {
        write_frame(&mut input, &serde_json::to_vec(&message).unwrap()).unwrap();
    }
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
        for chunk in input.chunks(3) {
            stdin.write_all(chunk).unwrap();
        }
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let mut reader = Cursor::new(output.stdout);
    let mut responses = Vec::<Value>::new();
    while let Some(payload) = read_frame(&mut reader).unwrap() {
        responses.push(serde_json::from_slice(&payload).unwrap());
    }
    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["id"], "init");
    assert_eq!(
        responses[0]["result"]["capabilities"]["foldingRangeProvider"],
        true
    );
    let published = json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":URI,"diagnostics":[]}});
    assert_eq!(responses[1], published);
    assert_eq!(
        responses[2],
        json!({"jsonrpc":"2.0","id":"comments","result":[{"startLine":1,"endLine":2,"kind":"comment"}]})
    );
    assert_eq!(responses[3], published);
    assert_eq!(
        responses[4],
        json!({"jsonrpc":"2.0","id":"single","result":[]})
    );
    assert_eq!(
        responses[5],
        json!({"jsonrpc":"2.0","id":"shutdown","result":null})
    );
}
