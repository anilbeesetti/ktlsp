use ktlsp_server::Session;
use serde_json::{Value, json};
use std::time::Instant;

const URI: &str = "untitled:providers";
fn request(session: &mut Session, method: &str, params: Value) -> Value {
    session
        .handle(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .remove(0)
}
fn notify(session: &mut Session, method: &str, params: Value) -> Vec<Value> {
    session.handle(json!({"jsonrpc":"2.0","method":method,"params":params}))
}
fn running(capabilities: Value) -> Session {
    let mut session = Session::new();
    let initialized = request(
        &mut session,
        "initialize",
        json!({"capabilities":capabilities}),
    );
    assert_eq!(
        initialized["result"]["capabilities"]["foldingRangeProvider"],
        true
    );
    notify(&mut session, "initialized", json!({}));
    session
}
fn open(session: &mut Session, text: &str) -> Value {
    let output = notify(
        session,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":text}}),
    );
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["method"], "textDocument/publishDiagnostics");
    assert!(output[0].get("id").is_none());
    output[0]["params"].clone()
}
fn change(session: &mut Session, version: i32, text: &str) -> Vec<Value> {
    notify(
        session,
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":version},"contentChanges":[{"text":text}]}),
    )
}
fn folding(session: &mut Session) -> Value {
    request(
        session,
        "textDocument/foldingRange",
        json!({"textDocument":{"uri":URI}}),
    )
}

#[test]
fn committed_official_fixture_comparison_keeps_protocol_differences_visible() {
    let transcript: Value = serde_json::from_str(include_str!(
        "../../../docs/research/oracle/lifecycle-folding/transcript.json"
    ))
    .unwrap();
    let entries = transcript.as_array().unwrap();
    let initialize = entries
        .iter()
        .find(|e| e["message"]["method"] == "initialize")
        .unwrap();
    let official = entries
        .iter()
        .find(|e| e["direction"] == "server" && e["message"]["id"] == 2)
        .unwrap()["message"]["result"]
        .clone();
    let source = include_str!("../../../docs/research/oracle/lifecycle-folding/Main.kt");
    assert_eq!(
        official,
        json!([{"startLine":0,"endLine":2,"startCharacter":11,"endCharacter":1,"kind":"region","collapsedText":"{...}"}])
    );
    let mut session = running(initialize["message"]["params"]["capabilities"].clone());
    assert!(
        notify(
            &mut session,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":source}}),
        )
        .is_empty()
    );
    let native = folding(&mut session)["result"].clone();
    assert_eq!(native, json!([{"startLine":0,"endLine":2,"kind":"region"}]));
    assert_ne!(native, official); // Character/collapsedText mismatch is deliberate, not normalized away.
    let mut session = running(json!({}));
    open(&mut session, source);
    let native = folding(&mut session)["result"].clone();
    assert_eq!(
        native,
        json!([{"startLine":0,"endLine":2,"startCharacter":11,"endCharacter":1,"kind":"region"}])
    );
}

#[test]
fn opaque_strings_templates_characters_backticks_and_comments_never_expose_braces() {
    let text = "fun f() {\n val s = \"${f(\"x\", { 1 })} } [ (\"\n val c = '}'\n val `}` = 1 // }\n /* { /* ) */ [\n } */\n val raw = \"\"\"{\n ${f(\"x\", { 1 })}\n }\"\"\"\n}\n";
    let mut session = running(json!({}));
    assert_eq!(open(&mut session, text)["diagnostics"], json!([]));
    let folds = folding(&mut session)["result"].as_array().unwrap().clone();
    assert_eq!(folds.len(), 3);
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[0]["endLine"], 9);
    assert_eq!(folds[1]["kind"], "comment");
    assert_eq!(folds[1]["startLine"], 4);
    assert_eq!(folds[2]["startLine"], 6);
}

#[test]
fn mixed_multiline_delimiters_and_single_line_pairs() {
    let mut session = running(json!({}));
    assert_eq!(
        open(&mut session, "{} () []\n{\n (\n [\n ]\n )\n}\n")["diagnostics"],
        json!([])
    );
    assert_eq!(
        folding(&mut session)["result"],
        json!([
            {"startLine":1,"endLine":6,"startCharacter":0,"endCharacter":1,"kind":"region"},
            {"startLine":2,"endLine":5,"startCharacter":1,"endCharacter":2,"kind":"region"},
            {"startLine":3,"endLine":4,"startCharacter":1,"endCharacter":2,"kind":"region"}
        ])
    );
}

#[test]
fn malformed_nesting_discards_pending_opens_without_crossing_folds() {
    let mut session = running(json!({}));
    let params = open(&mut session, "{\n (\n }\n )\n [");
    assert_eq!(params["diagnostics"].as_array().unwrap().len(), 5);
    assert_eq!(folding(&mut session)["result"], json!([]));
    for d in params["diagnostics"].as_array().unwrap() {
        assert_eq!(d["source"], "ktlsp-lexical-structural");
        assert_eq!(d["code"], "structural");
    }
    assert_eq!(
        change(&mut session, 2, "{\n}\n")[0]["params"]["diagnostics"],
        json!([])
    );
    assert_eq!(folding(&mut session)["result"].as_array().unwrap().len(), 1);
}

#[test]
fn utf16_non_bmp_crlf_lone_cr_and_lexical_ranges() {
    let mut session = running(json!({}));
    let text = "val s = \"😀\"; {\r\n}\r(\n)\nval bad = \"😀\\q\"";
    let params = open(&mut session, text);
    assert_eq!(params["diagnostics"][0]["code"], "lexical");
    assert_eq!(
        params["diagnostics"][0]["range"],
        json!({"start":{"line":4,"character":13},"end":{"line":4,"character":15}})
    );
    assert_eq!(
        folding(&mut session)["result"],
        json!([
            {"startLine":0,"endLine":1,"startCharacter":14,"endCharacter":1,"kind":"region"},
            {"startLine":2,"endLine":3,"startCharacter":0,"endCharacter":1,"kind":"region"}
        ])
    );
}

#[test]
fn diagnostics_replace_clear_and_follow_negotiated_version_support() {
    let mut session =
        running(json!({"textDocument":{"publishDiagnostics":{"versionSupport":true}}}));
    let params = open(&mut session, "{");
    assert_eq!(params["version"], 1);
    assert_eq!(params["diagnostics"].as_array().unwrap().len(), 1);
    assert!(change(&mut session, 1, "{}").is_empty());
    assert_eq!(session.document(URI).unwrap().text(), "{");
    let cleared = change(&mut session, 2, "{}");
    assert_eq!(cleared[0]["params"]["diagnostics"], json!([]));
    assert_eq!(cleared[0]["params"]["version"], 2);
    change(&mut session, 3, "\"unterminated");
    let closed = notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(closed[0]["params"]["diagnostics"], json!([]));
    assert!(closed[0]["params"].get("version").is_none());
    assert_eq!(folding(&mut session)["error"]["code"], -32602);
    let mut session = running(json!({}));
    assert!(open(&mut session, "}").get("version").is_none());
}

#[test]
fn limits_are_explicit_and_document_changes_still_commit_and_recover() {
    for text in [
        "{".repeat(1025),
        "😀".repeat(257),
        "/*".repeat(129),
        "()".repeat(500_001),
    ] {
        let mut session = running(json!({}));
        open(&mut session, "{");
        let updated = change(&mut session, 2, &text);
        assert_eq!(session.document(URI).unwrap().version(), 2);
        assert_eq!(
            updated[0]["params"]["diagnostics"][0]["code"],
            "analysis-limit"
        );
        assert_eq!(
            updated[0]["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(folding(&mut session)["error"]["code"], -32803);
        assert_eq!(
            change(&mut session, 3, "{}")[0]["params"]["diagnostics"],
            json!([])
        );
        assert_eq!(folding(&mut session)["result"], json!([]));
    }
}

#[test]
fn structural_diagnostics_are_capped_with_a_suppression_marker() {
    let mut session = running(json!({}));
    let params = open(&mut session, &")".repeat(100_000));
    let diagnostics = params["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 256);
    assert_eq!(diagnostics[255]["code"], "structural-diagnostic-limit");
    assert_eq!(folding(&mut session)["result"], json!([]));
}

#[test]
fn client_line_only_and_range_limit_including_zero() {
    for (limit, expected) in [(0, 0), (1, 1), (2, 2), (100_000, 3)] {
        let mut session = running(
            json!({"textDocument":{"foldingRange":{"lineFoldingOnly":true,"rangeLimit":limit}}}),
        );
        open(&mut session, "{} () []\n{\n}\n(\n)\n[\n]\n");
        let result = folding(&mut session)["result"].clone();
        let folds = result.as_array().unwrap();
        assert_eq!(folds.len(), expected);
        for fold in folds {
            assert!(fold.get("startCharacter").is_none());
            assert!(fold.get("endCharacter").is_none());
        }
    }
}

#[test]
fn invalid_capabilities_and_request_params_are_rejected_without_state_changes() {
    for profile in [
        json!({"textDocument":null}),
        json!({"textDocument":{"foldingRange":null}}),
        json!({"textDocument":{"foldingRange":{"lineFoldingOnly":1}}}),
        json!({"textDocument":{"foldingRange":{"rangeLimit":-1}}}),
        json!({"textDocument":{"foldingRange":{"rangeLimit":2147483648u64}}}),
        json!({"textDocument":{"foldingRange":{"rangeLimit":1.5}}}),
        json!({"textDocument":{"publishDiagnostics":{"versionSupport":null}}}),
    ] {
        let mut session = Session::new();
        assert_eq!(
            request(&mut session, "initialize", json!({"capabilities":profile}))["error"]["code"],
            -32602
        );
        assert!(
            request(&mut session, "initialize", json!({"capabilities":{}}))
                .get("result")
                .is_some()
        );
    }
    let mut session = running(json!({}));
    for params in [
        Value::Null,
        json!({}),
        json!({"textDocument":null}),
        json!({"textDocument":{"uri":5}}),
        json!({"textDocument":{"uri":"relative"}}),
    ] {
        assert_eq!(
            request(&mut session, "textDocument/foldingRange", params)["error"]["code"],
            -32602
        );
    }
    assert_eq!(folding(&mut session)["error"]["code"], -32602);
}

#[test]
fn many_folds_and_long_unicode_line_have_bounded_outputs() {
    let now = Instant::now();
    let mut session = running(json!({}));
    let text = "{\n}\n".repeat(50_000);
    assert_eq!(open(&mut session, &text)["diagnostics"], json!([]));
    let folds = folding(&mut session)["result"].clone();
    assert_eq!(folds.as_array().unwrap().len(), 10_000);
    assert_eq!(folds[0]["startLine"], 0);
    assert_eq!(folds[9999]["endLine"], 19_999);
    let multiline_elapsed = now.elapsed();
    let long_line = format!(
        "val s = \"{}\"; {}\n{{\n}}",
        "😀".repeat(200_000),
        "()".repeat(200_000)
    );
    let now = Instant::now();
    assert_eq!(
        change(&mut session, 2, &long_line)[0]["params"]["diagnostics"],
        json!([])
    );
    assert_eq!(folding(&mut session)["result"].as_array().unwrap().len(), 1);
    eprintln!(
        "many-fold open+query: {multiline_elapsed:?}; long-line change+query: {:?}",
        now.elapsed()
    );
}

#[test]
fn generated_arbitrary_utf8_provider_corpus_stays_live_with_valid_ranges() {
    let fragments = [
        "{", "}", "(", ")", "[", "]", "/*", "*/", "\"", "\"\"\"", "$", "${", "'", "`", "\\", "😀",
        "é", "\r\n", "\r", "\n", "x", "0x_", " ",
    ];
    let mut seed = 0x1234abcd_u64;
    let mut session = running(json!({}));
    open(&mut session, "");
    for version in 2..502 {
        let mut text = String::new();
        for _ in 0..100 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(fragments[(seed >> 32) as usize % fragments.len()]);
        }
        let output = change(&mut session, version, &text);
        for diagnostic in output[0]["params"]["diagnostics"].as_array().unwrap() {
            let range: ktlsp_text::Range =
                serde_json::from_value(diagnostic["range"].clone()).unwrap();
            assert!(range.start <= range.end);
            ktlsp_text::position_to_offset(&text, range.start).unwrap();
            ktlsp_text::position_to_offset(&text, range.end).unwrap();
        }
        assert!(folding(&mut session).get("result").is_some());
    }
}

#[test]
fn negotiated_pull_reports_current_utf16_diagnostics_without_duplicate_push() {
    let mut session = Session::new();
    let initialized = request(
        &mut session,
        "initialize",
        json!({"capabilities":{"textDocument":{"diagnostic":{}}}}),
    );
    let provider = &initialized["result"]["capabilities"]["diagnosticProvider"];
    assert_eq!(provider["interFileDependencies"], false);
    assert_eq!(provider["workspaceDiagnostics"], false);
    notify(&mut session, "initialized", json!({}));
    assert!(
        notify(
            &mut session,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"\"😀\" ]"}}),
        )
        .is_empty()
    );
    let params = json!({"textDocument":{"uri":URI},"previousResultId":"client-token"});
    let report = request(&mut session, "textDocument/diagnostic", params.clone());
    assert_eq!(report["result"]["kind"], "full");
    assert!(report["result"].get("resultId").is_none());
    assert_eq!(report["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["result"]["items"][0]["range"],
        json!({"start":{"line":0,"character":5},"end":{"line":0,"character":6}})
    );
    assert!(change(&mut session, 2, "").is_empty());
    assert_eq!(
        request(&mut session, "textDocument/diagnostic", params.clone())["result"],
        json!({"kind":"full","items":[]})
    );
    assert!(change(&mut session, 2, "]").is_empty());
    assert_eq!(
        request(&mut session, "textDocument/diagnostic", params.clone())["result"]["items"],
        json!([])
    );
    assert!(
        notify(
            &mut session,
            "textDocument/didClose",
            json!({"textDocument":{"uri":URI}}),
        )
        .is_empty()
    );
    assert_eq!(
        request(&mut session, "textDocument/diagnostic", params.clone())["error"]["code"],
        -32602
    );
    assert!(
        notify(
            &mut session,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"]"}}),
        )
        .is_empty()
    );
    let report = request(&mut session, "textDocument/diagnostic", params);
    assert_eq!(
        report["result"]["items"][0]["range"],
        json!({"start":{"line":0,"character":0},"end":{"line":0,"character":1}})
    );
}

#[test]
fn pull_negotiation_is_atomic_and_legacy_push_remains_available() {
    for diagnostic in [
        Value::Null,
        json!(true),
        json!({"dynamicRegistration":0}),
        json!({"relatedDocumentSupport":"true"}),
    ] {
        let mut session = Session::new();
        assert_eq!(
            request(
                &mut session,
                "initialize",
                json!({"capabilities":{"textDocument":{"diagnostic":diagnostic}}}),
            )["error"]["code"],
            -32602
        );
        assert_eq!(session.state(), ktlsp_server::State::Uninitialized);
        let initialized = request(&mut session, "initialize", json!({"capabilities":{}}));
        assert!(
            initialized["result"]["capabilities"]
                .get("diagnosticProvider")
                .is_none()
        );
        notify(&mut session, "initialized", json!({}));
        assert_eq!(
            open(&mut session, "]")["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            request(&mut session, "textDocument/diagnostic", json!({}))["error"]["code"],
            -32601
        );
    }
}

#[test]
fn pull_limits_are_visible_and_invalid_queries_do_not_change_the_document() {
    let mut session = running(json!({"textDocument":{"diagnostic":{}}}));
    assert!(notify(
        &mut session,
        "textDocument/didOpen",
        json!({"textDocument":{"uri":URI,"languageId":"kotlin","version":1,"text":"(".repeat(1025)}}),
    )
    .is_empty());
    let report = request(
        &mut session,
        "textDocument/diagnostic",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(report["result"]["kind"], "full");
    assert_eq!(report["result"]["items"][0]["code"], "analysis-limit");
    for params in [
        Value::Null,
        json!({}),
        json!({"textDocument":{"uri":"untitled:missing"}}),
        json!({"textDocument":{"uri":URI},"identifier":1}),
        json!({"textDocument":{"uri":URI},"previousResultId":null}),
    ] {
        assert_eq!(
            request(&mut session, "textDocument/diagnostic", params)["error"]["code"],
            -32602
        );
    }
    assert_eq!(session.document(URI).unwrap().version(), 1);
    assert!(change(&mut session, 2, "").is_empty());
    assert_eq!(
        request(
            &mut session,
            "textDocument/diagnostic",
            json!({"textDocument":{"uri":URI}}),
        )["result"],
        json!({"kind":"full","items":[]})
    );
}
