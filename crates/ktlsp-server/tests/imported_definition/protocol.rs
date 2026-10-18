use super::*;

// Opt-in receipts contain only this test's actual transport and owned fixtures.
fn observe(event: Value) {
    if std::env::var_os("KTLSP_NATIVE_IO_OBSERVE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        let _ = writeln!(std::io::stderr(), "KTLSP_IMPORTED_DEFINITION_IO_V1 {event}");
    }
}

// Only wrap the existing wire's cleanup: retain actual exit/stderr receipts if
// a semantic assertion or a transport read panics before ordinary shutdown.
struct ExitReceipt(SignatureWire);
impl std::ops::Deref for ExitReceipt {
    type Target = SignatureWire;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ExitReceipt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ExitReceipt {
    fn drop(&mut self) {
        if let Some(child) = self.0.child.take() {
            self.0.input.take();
            match child.wait_with_output() {
                Ok(output) => observe(json!({
                    "cleanupExit":{"code":output.status.code(),"success":output.status.success()},
                    "stderrBytes":output.stderr,
                })),
                Err(error) => observe(json!({"cleanupWaitError":error.to_string()})),
            }
        }
    }
}

fn send(wire: &mut SignatureWire, message: Value) -> Value {
    observe(json!({"request":message}));
    let before = wire.responses.len();
    let response = wire.send(message);
    for actual in &wire.responses[before..] {
        observe(json!({"response":actual}));
    }
    response
}

fn params(uri: &str, source: &str, offset: usize) -> Value {
    json!({"textDocument":{"uri":uri},"position":offset_to_position(source,offset).unwrap()})
}

fn query(wire: &mut SignatureWire, id: &str, method: &str, params: Value) -> Value {
    send(
        wire,
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
    )
}

fn opened(wire: &mut SignatureWire, uri: &str, version: i32, source: &str) {
    send(
        wire,
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"kotlin","version":version,"text":source}}}),
    );
}

fn closed(wire: &mut SignatureWire, uri: &str) {
    send(
        wire,
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
    );
}

// Ranges are derived from the PRE-edit text. Independently splice bytes in
// descending order, rather than using the server's Document edit implementation.
fn changed(
    wire: &mut SignatureWire,
    uri: &str,
    version: i32,
    source: &mut String,
    edits: &[(std::ops::Range<usize>, &str)],
) {
    let mut previous = source.len();
    let changes: Vec<Value> = edits.iter().map(|(bytes, replacement)| {
        assert!(bytes.start <= bytes.end && bytes.end <= previous);
        previous = bytes.start;
        json!({"range":references_range(source,bytes.start,bytes.end-bytes.start),"text":replacement})
    }).collect();
    for (bytes, replacement) in edits {
        source.replace_range(bytes.clone(), replacement);
    }
    send(
        wire,
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":version},"contentChanges":changes}}),
    );
    observe(json!({"overlay":{"uri":uri,"version":version,"text":source}}));
}

fn definition(
    wire: &mut SignatureWire,
    id: &str,
    requester: &str,
    source: &str,
    use_offset: usize,
    target: &str,
    target_range: Value,
) {
    let reply = query(
        wire,
        id,
        "textDocument/definition",
        params(requester, source, use_offset),
    );
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        reply["result"],
        json!({"uri":target,"range":target_range}),
        "{reply}"
    );
}

fn refused_definition(wire: &mut SignatureWire, id: &str, uri: &str, source: &str, offset: usize) {
    let reply = query(
        wire,
        id,
        "textDocument/definition",
        params(uri, source, offset),
    );
    assert_eq!(reply["error"]["code"], -32803, "{reply}");
    assert!(reply.get("result").is_none(), "{reply}");
}

// Reuse the parent's child, framing and cleanup owner. Log the genuine exit and
// stderr BEFORE asserting them, including on a failed hosted acceptance run.
fn finish(mut wire: ExitReceipt) {
    let stopped = query(&mut wire, "stop", "shutdown", Value::Null);
    send(&mut wire, json!({"jsonrpc":"2.0","method":"exit"}));
    wire.input.take();
    while let Some(frame) = ktlsp_protocol::read_frame(&mut wire.reader).unwrap() {
        let response: Value = serde_json::from_slice(&frame).unwrap();
        observe(json!({"response":response}));
        wire.responses.push(response);
    }
    let output = wire.child.take().unwrap().wait_with_output().unwrap();
    observe(
        json!({"exit":{"code":output.status.code(),"success":output.status.success()},
        "stderrBytes":output.stderr}),
    );
    assert!(stopped.get("error").is_none(), "{stopped}");
    assert_eq!(stopped["result"], Value::Null, "{stopped}");
    assert_eq!(output.status.code(), Some(0), "{:?}", output.status);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn imported_property_definition_initial_waiter_ranged_edits_reopen_reload_and_local_paths() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "imported-definition", &["src"]);
    let original_target = "// 😀\r\npackage values\r\nval `answer😀` = 1\r\n";
    let original_requester = "// 🦀\r\npackage app\r\nimport values.`answer😀`\r\nfun use() {\r\n val result = `answer😀`\r\n}\r\nfun local() {\r\n val `answer😀` = 2\r\n val chosen = `answer😀`\r\n}\r\nfun parameter(`answer😀`: Long) {\r\n val chosen = `answer😀`\r\n}\r\nfun <T> legacy(p: T) { fun nested() {}; nested(); val preserved = p }\r\n";
    std::fs::write(project.join("src/Definitions.kt"), original_target).unwrap();
    std::fs::write(project.join("src/Uses.kt"), original_requester).unwrap();
    let root = source_directory_uri(&project);
    let target_uri = format!("{root}/src/Definitions.kt");
    let target_alias = format!("{root}/src/%44efinitions.kt");
    let requester_uri = format!("{root}/src/Uses.kt");
    let requester_alias = format!("{root}/src/%55ses.kt");
    let options = source_options(&[&root]);
    observe(json!({"initialSources":{
        target_uri.clone():std::fs::read_to_string(project.join("src/Definitions.kt")).unwrap(),
        requester_uri.clone():std::fs::read_to_string(project.join("src/Uses.kt")).unwrap()},
        "model":std::fs::read_to_string(project.join("workspace.json")).unwrap()}));
    let mut wire = ExitReceipt(SignatureWire::new(&package));
    let initialized = query(
        &mut wire,
        "init",
        "initialize",
        json!({
        "initializationOptions":options,
        "capabilities":{"general":{"positionEncodings":["utf-8"]},
            "workspace":{"workspaceEdit":{"documentChanges":true}},
            "textDocument":{"documentSymbol":{"hierarchicalDocumentSymbolSupport":true},
                "hover":{"contentFormat":["plaintext"]},"rename":{"prepareSupport":true},"diagnostic":{}}}}),
    );
    send(
        &mut wire,
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
    );
    opened(&mut wire, &requester_alias, 1, original_requester);
    // This is the FIRST post-initialize request: no workspaceSymbol, readiness
    // query, sleep, retry or warmup can hide the initial-import waiter path.
    definition(
        &mut wire,
        "initial-import",
        &requester_alias,
        original_requester,
        original_requester.find("= `answer😀`").unwrap() + 2,
        &target_uri,
        span(original_target, "`answer😀`"),
    );
    assert_eq!(
        initialized["result"]["capabilities"]["definitionProvider"], true,
        "{initialized}"
    );
    assert_eq!(
        initialized["result"]["capabilities"]["positionEncoding"],
        "utf-16"
    );

    opened(&mut wire, &target_alias, 1, original_target);
    let mut target = original_target.to_owned();
    let prefix = "// moved 🦀\r\n";
    changed(&mut wire, &target_alias, 2, &mut target, &[(0..0, prefix)]);
    assert_eq!(target, format!("{prefix}{original_target}"));
    definition(
        &mut wire,
        "moved-target",
        &requester_alias,
        original_requester,
        original_requester.find("= `answer😀`").unwrap() + 2,
        &target_uri,
        span(&target, "`answer😀`"),
    );
    assert_ne!(
        span(&target, "`answer😀`"),
        span(original_target, "`answer😀`")
    );

    let name = target.find("`answer😀`").unwrap();
    changed(
        &mut wire,
        &target_alias,
        3,
        &mut target,
        &[(name..name + "`answer😀`".len(), "`moved🦀`")],
    );
    assert_eq!(
        target,
        format!(
            "{prefix}{}",
            original_target.replace("`answer😀`", "`moved🦀`")
        )
    );
    let mut requester = original_requester.to_owned();
    let imported = requester.find("`answer😀`").unwrap();
    let used = requester.find("= `answer😀`").unwrap() + 2;
    changed(
        &mut wire,
        &requester_alias,
        2,
        &mut requester,
        &[
            (used..used + "`answer😀`".len(), "`moved🦀`"),
            (imported..imported + "`answer😀`".len(), "`moved🦀`"),
        ],
    );
    assert_eq!(
        requester,
        original_requester.replacen("`answer😀`", "`moved🦀`", 2)
    );
    definition(
        &mut wire,
        "renamed-import-and-use",
        &requester_alias,
        &requester,
        requester.find("= `moved🦀`").unwrap() + 2,
        &target_uri,
        span(&target, "`moved🦀`"),
    );

    closed(&mut wire, &target_alias);
    refused_definition(
        &mut wire,
        "closed-discards-renamed-overlay",
        &requester_alias,
        &requester,
        requester.find("= `moved🦀`").unwrap() + 2,
    );
    let imported = requester.find("`moved🦀`").unwrap();
    let used = requester.find("= `moved🦀`").unwrap() + 2;
    changed(
        &mut wire,
        &requester_alias,
        3,
        &mut requester,
        &[
            (used..used + "`moved🦀`".len(), "`answer😀`"),
            (imported..imported + "`moved🦀`".len(), "`answer😀`"),
        ],
    );
    assert_eq!(requester, original_requester);
    definition(
        &mut wire,
        "closed-original-source",
        &requester_alias,
        &requester,
        requester.find("= `answer😀`").unwrap() + 2,
        &target_uri,
        span(original_target, "`answer😀`"),
    );
    let reopened = format!("{prefix}{original_target}");
    opened(&mut wire, &target_alias, 1, &reopened);
    definition(
        &mut wire,
        "reopened-new-incarnation",
        &requester_alias,
        &requester,
        requester.find("= `answer😀`").unwrap() + 2,
        &target_uri,
        span(&reopened, "`answer😀`"),
    );
    closed(&mut wire, &target_alias);
    definition(
        &mut wire,
        "closed-again-original-source",
        &requester_alias,
        &requester,
        requester.find("= `answer😀`").unwrap() + 2,
        &target_uri,
        span(original_target, "`answer😀`"),
    );
    assert_eq!(
        std::fs::read_to_string(project.join("src/Definitions.kt")).unwrap(),
        original_target
    );

    // Imported names must not displace an eligible original local or parameter.
    let local = requester.find("val `answer😀`").unwrap() + 4;
    let local_use = requester.find("val chosen = `answer😀`").unwrap() + "val chosen = ".len();
    let parameter = requester.find("parameter(`answer😀`").unwrap() + "parameter(".len();
    let parameter_use = requester.rfind("val chosen = `answer😀`").unwrap() + "val chosen = ".len();
    for (id, usage, declaration) in [
        ("local-shadow", local_use, local),
        ("parameter-shadow", parameter_use, parameter),
    ] {
        definition(
            &mut wire,
            id,
            &requester_alias,
            &requester,
            usage,
            &requester_alias,
            references_range(&requester, declaration, "`answer😀`".len()),
        );
    }
    let type_declaration = requester.find("<T>").unwrap() + 1;
    definition(
        &mut wire,
        "preserved-type-definition",
        &requester_alias,
        &requester,
        requester.find("p: T").unwrap() + 3,
        &requester_alias,
        references_range(&requester, type_declaration, 1),
    );
    definition(
        &mut wire,
        "preserved-call-definition",
        &requester_alias,
        &requester,
        requester.rfind("nested()").unwrap(),
        &requester_alias,
        span(&requester, "nested"),
    );
    let hover = query(
        &mut wire,
        "local-hover",
        "textDocument/hover",
        params(&requester_alias, &requester, local_use),
    );
    assert_eq!(
        hover["result"],
        json!({"contents":{"kind":"plaintext","value":"val `answer😀` = 2"},
        "range":references_range(&requester,local_use,"`answer😀`".len())}),
        "{hover}"
    );
    let mut references = params(&requester_alias, &requester, local_use);
    references["context"] = json!({"includeDeclaration":true});
    let references = query(
        &mut wire,
        "local-references",
        "textDocument/references",
        references,
    );
    assert_eq!(
        references["result"],
        json!([
        {"uri":requester_alias,"range":references_range(&requester,local,"`answer😀`".len())},
        {"uri":requester_alias,"range":references_range(&requester,local_use,"`answer😀`".len())}]),
        "{references}"
    );
    let symbols = query(
        &mut wire,
        "local-symbols",
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":requester_alias}}),
    );
    let local_symbol = symbols["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "local")
        .unwrap();
    assert_eq!(
        local_symbol["children"][0]["selectionRange"],
        references_range(&requester, local, "`answer😀`".len()),
        "{symbols}"
    );

    // Additions are discovered only by the existing explicit manual reload.
    let late = "// new 😀\r\npackage extras\r\nval arrived = 3\r\n";
    let late_uri = format!("{root}/src/Late.kt");
    std::fs::write(project.join("src/Late.kt"), late).unwrap();
    observe(
        json!({"addedSource":{"uri":late_uri,"text":std::fs::read_to_string(project.join("src/Late.kt")).unwrap()}}),
    );
    let import_start = requester.find("values.`answer😀`").unwrap();
    let used = requester.find("= `answer😀`").unwrap() + 2;
    changed(
        &mut wire,
        &requester_alias,
        4,
        &mut requester,
        &[
            (used..used + "`answer😀`".len(), "arrived"),
            (
                import_start..import_start + "values.`answer😀`".len(),
                "extras.arrived",
            ),
        ],
    );
    assert_eq!(
        requester,
        original_requester
            .replacen("values.`answer😀`", "extras.arrived", 1)
            .replacen("= `answer😀`", "= arrived", 1)
    );
    refused_definition(
        &mut wire,
        "addition-not-yet-captured",
        &requester_alias,
        &requester,
        requester.find("= arrived").unwrap() + 2,
    );
    let reloaded = send(&mut wire, source_reload(90, &options));
    assert!(reloaded.get("error").is_none(), "{reloaded}");
    assert_eq!(reloaded["result"], Value::Null, "{reloaded}");
    definition(
        &mut wire,
        "manual-reload-admits-addition",
        &requester_alias,
        &requester,
        requester.find("= arrived").unwrap() + 2,
        &late_uri,
        span(late, "arrived"),
    );
    definition(
        &mut wire,
        "reload-preserves-local-shadow",
        &requester_alias,
        &requester,
        requester.find("val chosen = `answer😀`").unwrap() + "val chosen = ".len(),
        &requester_alias,
        references_range(
            &requester,
            requester.find("val `answer😀`").unwrap() + 4,
            "`answer😀`".len(),
        ),
    );
    finish(wire);
}
