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

fn token_capabilities(requests: Value) -> Value {
    json!({"textDocument":{"semanticTokens":{
        "requests":requests,"formats":["relative"],
        "tokenTypes":["number","string","operator","keyword","comment"],
        "tokenModifiers":[]
    }}})
}
fn token_session(capabilities: Value) -> (Session, Value) {
    let mut session = Session::new(); // No worker: lexical tokens must remain usable.
    let reply = request(
        &mut session,
        "initialize",
        json!({"capabilities":capabilities}),
    );
    assert_eq!(
        reply["result"]["capabilities"]["foldingRangeProvider"],
        true
    );
    notify(&mut session, "initialized", json!({}));
    (
        session,
        reply["result"]["capabilities"]["semanticTokensProvider"].clone(),
    )
}
fn tokens(session: &mut Session, range: Option<Value>) -> Value {
    let mut params = json!({"textDocument":{"uri":URI}});
    let method = if let Some(range) = range {
        params["range"] = range;
        "textDocument/semanticTokens/range"
    } else {
        "textDocument/semanticTokens/full"
    };
    request(session, method, params)
}
fn token_range(start: (u32, u32), end: (u32, u32)) -> Value {
    json!({"start":{"line":start.0,"character":start.1},"end":{"line":end.0,"character":end.1}})
}
fn decoded_tokens(reply: &Value, options: &Value) -> Vec<(u64, u64, u64, String)> {
    let data = reply["result"]["data"]
        .as_array()
        .expect("successful token data");
    assert!(reply["result"].get("resultId").is_none());
    assert_eq!(data.len() % 5, 0);
    let mut line = 0;
    let mut column = 0;
    let mut previous_end = (0, 0);
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
            let length = token[2].as_u64().unwrap();
            assert!(length > 0);
            assert!((line, column) >= previous_end);
            previous_end = (line, column + length);
            assert_eq!(token[4], 0);
            let kind = options["legend"]["tokenTypes"][token[3].as_u64().unwrap() as usize]
                .as_str()
                .unwrap()
                .to_owned();
            (line, column, length, kind)
        })
        .collect()
}
fn expected_tokens(tokens: &[(u64, u64, u64, &str)]) -> Vec<(u64, u64, u64, String)> {
    tokens
        .iter()
        .map(|&(line, column, length, kind)| (line, column, length, kind.into()))
        .collect()
}

#[test]
fn semantic_tokens_decode_real_lexer_categories_and_opaque_templates() {
    let (mut session, options) =
        token_session(token_capabilities(json!({"full":true,"range":true})));
    open(
        &mut session,
        "#!/bin/kt\nval `x😀` = 12 + \"${f(\"x\", { 1 })}\" //😀\n'a' id . , : ; §",
    );
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[
            (0, 0, 9, "comment"),
            (1, 0, 3, "keyword"),
            (1, 10, 1, "operator"),
            (1, 12, 2, "number"),
            (1, 15, 1, "operator"),
            (1, 17, 18, "string"),
            (1, 36, 4, "comment"),
            (2, 0, 3, "string"),
        ])
    );
    assert_eq!(options["legend"]["tokenModifiers"], json!([]));
    assert_eq!(
        tokens(&mut session, Some(token_range((1, 13), (1, 14))))["result"]["data"],
        json!([1, 13, 1, 3, 0])
    );
}

#[test]
fn semantic_tokens_split_mixed_terminators_and_clip_utf16_ranges() {
    let (mut session, options) = token_session(token_capabilities(json!({"full":{},"range":{}})));
    open(&mut session, "/*😀\r\n\rx\nz*/\n\"\"\"a😀\rb\r\nc\n\"\"\"");
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[
            (0, 0, 4, "comment"),
            (2, 0, 1, "comment"),
            (3, 0, 3, "comment"),
            (4, 0, 6, "string"),
            (5, 0, 1, "string"),
            (6, 0, 1, "string"),
            (7, 0, 3, "string"),
        ])
    );
    assert_eq!(
        decoded_tokens(
            &tokens(&mut session, Some(token_range((0, 2), (3, 1)))),
            &options
        ),
        expected_tokens(&[
            (0, 2, 2, "comment"),
            (2, 0, 1, "comment"),
            (3, 0, 1, "comment"),
        ])
    );
    assert_eq!(
        decoded_tokens(
            &tokens(&mut session, Some(token_range((4, 4), (7, 2)))),
            &options
        ),
        expected_tokens(&[
            (4, 4, 2, "string"),
            (5, 0, 1, "string"),
            (6, 0, 1, "string"),
            (7, 0, 2, "string"),
        ])
    );
    for range in [
        token_range((0, 3), (0, 4)),
        token_range((4, 5), (7, 0)),
        token_range((8, 0), (8, 0)),
        token_range((0, 5), (1, 0)),
        token_range((3, 1), (0, 0)),
    ] {
        assert_eq!(tokens(&mut session, Some(range))["error"]["code"], -32602);
    }
    for range in [
        token_range((0, 2), (0, 2)),
        token_range((1, 0), (1, 0)),
        token_range((7, 3), (7, 3)),
    ] {
        assert_eq!(
            tokens(&mut session, Some(range))["result"],
            json!({"data":[]})
        );
    }
}

#[test]
fn semantic_token_negotiation_gates_each_request_and_intersects_types() {
    for (requests, full, range) in [
        (json!({"full":true}), true, false),
        (json!({"range":{}}), false, true),
        (json!({"full":{"delta":true},"range":true}), true, true),
    ] {
        let mut capabilities = token_capabilities(requests);
        capabilities["textDocument"]["semanticTokens"]["tokenTypes"] =
            json!(["variable", "number", "string"]);
        let (mut session, options) = token_session(capabilities);
        open(&mut session, "val name = 1 + \"x\"");
        for (is_range, enabled) in [(false, full), (true, range)] {
            let reply = tokens(&mut session, is_range.then(|| token_range((0, 0), (0, 18))));
            if enabled {
                assert_eq!(
                    decoded_tokens(&reply, &options),
                    expected_tokens(&[(0, 11, 1, "number"), (0, 15, 3, "string")])
                );
            } else {
                assert_eq!(reply["error"]["code"], -32601);
            }
        }
        assert_eq!(
            options["full"],
            if full { json!(true) } else { Value::Null }
        );
        assert_eq!(
            options["range"],
            if range { json!(true) } else { Value::Null }
        );
        assert_eq!(
            request(
                &mut session,
                "textDocument/semanticTokens/full/delta",
                json!({})
            )["error"]["code"],
            -32601
        );
        assert!(session.pending_requests() == 0);
    }
}

#[test]
fn malformed_or_incompatible_semantic_capabilities_only_disable_tokens() {
    let valid = token_capabilities(json!({"full":true}));
    let mut profiles = vec![json!({}), json!({"textDocument":{"semanticTokens":true}})];
    for (key, values) in [
        (
            "formats",
            vec![
                json!([]),
                json!(["absolute"]),
                json!(["relative", 1]),
                Value::Null,
            ],
        ),
        (
            "tokenTypes",
            vec![json!(["variable"]), json!(["number", 1]), Value::Null],
        ),
        ("tokenModifiers", vec![json!([1]), Value::Null]),
        (
            "requests",
            vec![
                json!({}),
                json!({"full":false}),
                json!({"full":1}),
                json!({"range":[]}),
                json!({"full":{"delta":1}}),
                json!({"full":true,"range":null}),
            ],
        ),
        ("multilineTokenSupport", vec![json!(1)]),
    ] {
        for value in values {
            let mut capabilities = valid.clone();
            capabilities["textDocument"]["semanticTokens"][key] = value;
            profiles.push(capabilities);
        }
    }
    for capabilities in profiles {
        let (mut session, options) = token_session(capabilities);
        assert!(options.is_null());
        open(&mut session, "{\n}");
        assert_eq!(folding(&mut session)["result"].as_array().unwrap().len(), 1);
        assert_eq!(tokens(&mut session, None)["error"]["code"], -32601);
        assert_eq!(
            tokens(&mut session, Some(token_range((0, 0), (1, 1))))["error"]["code"],
            -32601
        );
    }
}

#[test]
fn semantic_tokens_track_committed_edits_and_retain_rejected_version() {
    let (mut session, options) =
        token_session(token_capabilities(json!({"full":true,"range":true})));
    open(&mut session, "val x = \"😀\"");
    let before = tokens(&mut session, None);
    assert_eq!(
        decoded_tokens(&before, &options),
        expected_tokens(&[
            (0, 0, 3, "keyword"),
            (0, 6, 1, "operator"),
            (0, 8, 4, "string")
        ])
    );
    // The first change would succeed; the split-surrogate second change rolls both back.
    notify(
        &mut session,
        "textDocument/didChange",
        json!({"textDocument":{"uri":URI,"version":2},"contentChanges":[
            {"range":token_range((0,0),(0,3)),"text":"var"},
            {"range":token_range((0,10),(0,11)),"text":"x"}
        ]}),
    );
    assert_eq!(session.document(URI).unwrap().version(), 1);
    assert_eq!(tokens(&mut session, None), before);
    assert!(session.drain_errors().len() == 1);
    change(&mut session, 2, "\n//😀\r\n42");
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[(1, 0, 4, "comment"), (2, 0, 2, "number")])
    );
    change(&mut session, 2, "false");
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[(1, 0, 4, "comment"), (2, 0, 2, "number")])
    );
    notify(
        &mut session,
        "textDocument/didClose",
        json!({"textDocument":{"uri":URI}}),
    );
    assert_eq!(tokens(&mut session, None)["error"]["code"], -32602);
    open(&mut session, "");
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
    change(&mut session, 2, "identifier `quoted` . , : ;");
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
}

#[test]
fn semantic_token_invalid_requests_are_atomic_invalid_params() {
    let (mut session, _) = token_session(token_capabilities(json!({"full":true,"range":true})));
    open(&mut session, "\"😀\"");
    for params in [
        Value::Null,
        json!({}),
        json!({"textDocument":null}),
        json!({"textDocument":{"uri":"relative"}}),
        json!({"textDocument":{"uri":"untitled:missing"}}),
    ] {
        for method in [
            "textDocument/semanticTokens/full",
            "textDocument/semanticTokens/range",
        ] {
            assert_eq!(
                request(&mut session, method, params.clone())["error"]["code"],
                -32602
            );
        }
    }
    for range in [
        Value::Null,
        json!({}),
        token_range((0, 2), (0, 3)),
        token_range((0, 0), (0, 5)),
        json!({"start":{"line":-1,"character":0},"end":{"line":0,"character":0}}),
    ] {
        assert_eq!(tokens(&mut session, Some(range))["error"]["code"], -32602);
    }
    assert_eq!(
        request(
            &mut session,
            "textDocument/semanticTokens/range",
            json!({"textDocument":{"uri":URI}})
        )["error"]["code"],
        -32602
    );
    assert_eq!(session.document(URI).unwrap().version(), 1);
}

#[test]
fn semantic_token_fragment_and_lexer_resource_boundaries_fail_whole_queries() {
    let (mut session, options) =
        token_session(token_capabilities(json!({"full":true,"range":true})));
    open(&mut session, &"1 ".repeat(10_000));
    let reply = tokens(&mut session, None);
    let decoded = decoded_tokens(&reply, &options);
    assert_eq!(decoded.len(), 10_000);
    assert_eq!(decoded.first().unwrap(), &(0, 0, 1, "number".into()));
    assert_eq!(decoded.last().unwrap(), &(0, 19_998, 1, "number".into()));
    assert!(serde_json::to_vec(&reply["result"]).unwrap().len() <= 1024 * 1024);
    change(&mut session, 2, &"1 ".repeat(10_001));
    let failed = tokens(&mut session, None);
    assert_eq!(failed["error"]["code"], -32803);
    assert!(failed.get("result").is_none());
    assert_eq!(
        decoded_tokens(
            &tokens(&mut session, Some(token_range((0, 20_000), (0, 20_001)))),
            &options
        ),
        expected_tokens(&[(0, 20_000, 1, "number")])
    );
    change(&mut session, 3, &format!("/*{}*/", "a\n".repeat(9_999)));
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options).len(),
        10_000
    );
    change(&mut session, 4, &format!("/*{}*/", "a\n".repeat(10_000)));
    assert_eq!(tokens(&mut session, None)["error"]["code"], -32803);
    change(&mut session, 5, &"a ".repeat(500_000));
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
    change(&mut session, 6, &"a ".repeat(500_001));
    assert_eq!(tokens(&mut session, None)["error"]["code"], -32803);
    change(&mut session, 7, &" ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES));
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
    change(
        &mut session,
        8,
        &" ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES + 1),
    );
    assert_eq!(session.document(URI).unwrap().version(), 7);
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
    change(&mut session, 8, &"§ ".repeat(256));
    assert_eq!(tokens(&mut session, None)["result"], json!({"data":[]}));
    change(&mut session, 9, &"§ ".repeat(257));
    assert_eq!(tokens(&mut session, None)["error"]["code"], -32803);
    change(
        &mut session,
        10,
        &format!("{}{}", "/*".repeat(128), "*/".repeat(128)),
    );
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[(0, 0, 512, "comment")])
    );
    change(
        &mut session,
        11,
        &format!("{}{}", "/*".repeat(129), "*/".repeat(129)),
    );
    assert_eq!(tokens(&mut session, None)["error"]["code"], -32803);
    change(&mut session, 12, "7");
    assert_eq!(
        decoded_tokens(&tokens(&mut session, None), &options),
        expected_tokens(&[(0, 0, 1, "number")])
    );
}

#[test]
fn invalid_semantic_ranges_take_precedence_over_resource_exhaustion() {
    let (mut session, _) = token_session(token_capabilities(json!({"full":true,"range":true})));
    open(&mut session, &format!("{}\"😀\"", "1 ".repeat(10_001)));
    for end in [(0, 20_007), (1, 0), (0, 20_004)] {
        assert_eq!(
            tokens(&mut session, Some(token_range((0, 0), end)))["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        tokens(&mut session, Some(token_range((0, 0), (0, 20_006))))["error"]["code"],
        -32803
    );
    change(&mut session, 2, &format!("{}\"😀\"", "§ ".repeat(257)));
    for end in [(0, 519), (1, 0), (0, 516)] {
        assert_eq!(
            tokens(&mut session, Some(token_range((0, 0), end)))["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        tokens(&mut session, Some(token_range((0, 0), (0, 518))))["error"]["code"],
        -32803
    );
}
