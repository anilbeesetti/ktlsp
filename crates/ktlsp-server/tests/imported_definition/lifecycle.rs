//! Real imported-source publication and owned-work regressions; source-only under #96.
use super::*;

fn definition(id: i32, uri: &str, source: &str, offset: usize) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition","params":{
        "textDocument":{"uri":uri},"position":offset_to_position(source, offset).unwrap()}})
}

fn assert_definition(reply: &Value, uri: &str, source: &str, offset: usize, width: usize) {
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        reply["result"],
        json!({"uri":uri,"range":references_range(source,offset,width)}),
        "{reply}"
    );
}

fn assert_refused(reply: &Value, code: i32) {
    assert_eq!(reply["error"]["code"], code, "{reply}");
    assert!(reply.get("result").is_none(), "{reply}");
}

fn write_member(project: &Path, name: &str, source: &str) {
    std::fs::write(project.join("src").join(name), source).unwrap();
}

fn start_source_session(options: &Value, uri: &str, source: &str) -> Session {
    let mut session = Session::with_parser(worker()).unwrap();
    let initialized = session.handle(source_initialize(options));
    assert_eq!(initialized.len(), 1, "{initialized:?}");
    assert!(initialized[0].get("error").is_none(), "{initialized:?}");
    notify(&mut session, "initialized", json!({}));
    open_uri(&mut session, uri, source);
    session
}

fn hover(id: i32, uri: &str, source: &str, offset: usize) -> Value {
    let mut message = definition(id, uri, source, offset);
    message["method"] = json!("textDocument/hover");
    message
}

fn assert_hover(reply: &Value, declaration: &str, requester: &str, offset: usize, width: usize) {
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        reply["result"],
        json!({"contents":{"kind":"plaintext","value":declaration},
            "range":references_range(requester,offset,width)}),
        "{reply}"
    );
}

fn settled(session: &mut Session, message: Value) -> Vec<Value> {
    let mut replies = session.handle(message);
    replies.extend(collect_owned(session));
    assert_eq!(session.pending_requests(), 0, "{replies:?}");
    replies
}

#[test]
fn imported_definition_requires_whole_outer_proof_and_preserves_eligible_local_precedence() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "proof", &["src"]);
    let target = "package values\nval `answer` = 1\n";
    write_member(&project, "Definitions.kt", target);
    write_member(&project, "Uses.kt", "package app\nfun empty() {}\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/Definitions.kt");
    let mut wire = SignatureWire::new(&package);
    let init = wire.send(source_initialize(&source_options(&[&root])));
    assert!(init.get("error").is_none(), "{init}");
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    // None means refusal; an empty target marker denotes the original imported declaration.
    for (index, (body, local)) in [
        ("fun f() { val use = answer }", Some("")),
        (
            "fun f() { val answer = 2; val use = answer }",
            Some("val answer"),
        ),
        (
            "fun f(`answer`: Int) { val use = answer }",
            Some("`answer`"),
        ),
        (
            "fun f(`answer`: Int) { val answer = answer }",
            Some("`answer`"),
        ),
        (
            "fun f(answer: Int) { val use = answer; val answer = 2 }",
            Some("answer: Int"),
        ),
        ("fun f() { val answer = answer }", Some("")),
        ("fun f() { val use = answer; val answer = 2 }", Some("")),
        ("fun f() { missing(); val use = answer }", None),
        ("fun f() { val use = answer; missing() }", None),
        ("fun f() { val use = answer; var later = 1 }", None),
        (
            "fun f() { val answer = 1; val `answer` = 2; val use = answer }",
            None,
        ),
        ("fun f() { val use = answer + 1 }", None),
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!("package app\nimport values.answer\n{body}\n");
        if index == 0 {
            wire.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}),
            );
        } else {
            wire.send(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":uri,"version":index+1},"contentChanges":[{"text":source}]}}),
            );
        }
        let reference = source.find("= answer").unwrap() + 2;
        let reply = wire.send(definition(index as i32 + 2, &uri, &source, reference));
        match local {
            None => assert_refused(&reply, -32803),
            Some("") => assert_definition(
                &reply,
                &target_uri,
                target,
                target.find("`answer`").unwrap(),
                "`answer`".len(),
            ),
            Some(marker) => {
                let start =
                    source.find(marker).unwrap() + if marker.starts_with("val ") { 4 } else { 0 };
                let width = if marker.starts_with('`') {
                    "`answer`".len()
                } else {
                    "answer".len()
                };
                assert_definition(&reply, &uri, &source, start, width);
            }
        }
    }
    let replies = wire.finish();
    assert_eq!(replies.len(), 14, "{replies:?}");
}

#[test]
fn imported_definition_rejects_duplicate_and_late_candidate_evidence_without_prefix_answers() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "candidates", &["src", "late"]);
    let good = "package values\nval answer = 1\n";
    let source = "package app\nimport values.answer\nfun f() { val use = answer }\n";
    write_member(&project, "A.kt", good);
    write_member(&project, "Uses.kt", source);
    // Original root order, not filesystem entry order, puts this candidate last.
    std::fs::write(
        project.join("late/Z.kt"),
        "package unrelated\nval other = 2\n",
    )
    .unwrap();
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/A.kt");
    let options = source_options(&[&root]);
    let mut wire = SignatureWire::new(&package);
    let init = wire.send(source_initialize(&options));
    assert!(init.get("error").is_none(), "{init}");
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}),
    );
    let reference = source.rfind("answer").unwrap();
    let first = wire.send(definition(2, &uri, source, reference));
    assert_definition(&first, &target_uri, good, good.find("answer").unwrap(), 6);
    for (index, (target, late)) in [
        (good, "package `values`\nval `answer` = 2\n"),
        (
            "package values\nprivate val answer = 1\n",
            "package unrelated\nval other = 2\n",
        ),
        (
            "package values\nvar answer = 1\n",
            "package unrelated\nval other = 2\n",
        ),
        (good, "package unrelated\nval other =\n"),
    ]
    .into_iter()
    .enumerate()
    {
        write_member(&project, "A.kt", target);
        std::fs::write(project.join("late/Z.kt"), late).unwrap();
        let reload = wire.send(source_reload(10 + index as i32 * 2, &options));
        assert!(reload.get("error").is_none(), "{reload}");
        let reply = wire.send(definition(11 + index as i32 * 2, &uri, source, reference));
        assert_refused(&reply, -32803);
    }
    write_member(&project, "A.kt", good);
    std::fs::write(
        project.join("late/Z.kt"),
        "package unrelated\nval other = 2\n",
    )
    .unwrap();
    let reloaded = wire.send(source_reload(20, &options));
    assert!(reloaded.get("error").is_none(), "{reloaded}");
    for (index, imports) in [
        "import values.answer\nimport values.answer",
        "import values.answer\nimport `values`.`answer`",
    ]
    .into_iter()
    .enumerate()
    {
        let ambiguous = format!("package app\n{imports}\nfun f() {{ val use = answer }}\n");
        wire.send(
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":index+2},"contentChanges":[{"text":ambiguous}]}}),
        );
        let reply = wire.send(definition(
            21 + index as i32,
            &uri,
            &ambiguous,
            ambiguous.rfind("answer").unwrap(),
        ));
        assert_refused(&reply, -32803);
    }
    let replies = wire.finish();
    assert_eq!(replies.len(), 14, "{replies:?}");
}

#[test]
fn imported_definition_checks_nonmatching_closed_sources_models_and_reload_successors() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "freshness", &["src"]);
    let target = "package values\nval answer = 1\n";
    let source = "package app\nimport values.answer\nfun f() { val use = answer }\n";
    let absent = source
        .replace("values.answer", "values.absent")
        .replace("= answer", "= absent");
    write_member(&project, "A.kt", target);
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Z.kt", "package unrelated\nval other = 2\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/A.kt");
    let options = source_options(&[&root]);
    let mut wire = SignatureWire::new(&package);
    let init = wire.send(source_initialize(&options));
    assert!(init.get("error").is_none(), "{init}");
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}),
    );
    let first = wire.send(definition(2, &uri, source, source.rfind("answer").unwrap()));
    assert_definition(
        &first,
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    // The changed file never matched the import. Both positive and negative
    // lookups must verify it, rather than publish a target or a cached absence.
    write_member(&project, "Z.kt", "package unrelated\nval other = 3\n");
    let stale = wire.send(definition(3, &uri, source, source.rfind("answer").unwrap()));
    assert_refused(&stale, -32801);
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":absent}]}}),
    );
    let negative = wire.send(definition(
        4,
        &uri,
        &absent,
        absent.rfind("absent").unwrap(),
    ));
    assert_refused(&negative, -32801);
    let reload = wire.send(source_reload(5, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let negative = wire.send(definition(
        6,
        &uri,
        &absent,
        absent.rfind("absent").unwrap(),
    ));
    assert_refused(&negative, -32803);
    // A byte-only model edit still invalidates original model authority.
    let mut model = serde_json::to_vec(&source_model(&["src"])).unwrap();
    model.push(b'\n');
    std::fs::write(project.join("workspace.json"), model).unwrap();
    let stale = wire.send(definition(
        7,
        &uri,
        &absent,
        absent.rfind("absent").unwrap(),
    ));
    assert_refused(&stale, -32801);
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":source}]}}),
    );
    let stale = wire.send(definition(8, &uri, source, source.rfind("answer").unwrap()));
    assert_refused(&stale, -32801);
    let reload = wire.send(source_reload(9, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let recovered = wire.send(definition(
        10,
        &uri,
        source,
        source.rfind("answer").unwrap(),
    ));
    assert_definition(
        &recovered,
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    // A failed replacement must revoke the historical snapshot, not keep it.
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap(),
    )
    .unwrap();
    assert_refused(&wire.send(source_reload(11, &options)), -32803);
    assert_refused(
        &wire.send(definition(
            12,
            &uri,
            source,
            source.rfind("answer").unwrap(),
        )),
        -32803,
    );
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(&["src"])).unwrap(),
    )
    .unwrap();
    let reload = wire.send(source_reload(13, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let recovered = wire.send(definition(
        14,
        &uri,
        source,
        source.rfind("answer").unwrap(),
    ));
    assert_definition(
        &recovered,
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    let replies = wire.finish();
    assert_eq!(replies.len(), 15, "{replies:?}");
}

#[test]
fn imported_definition_uses_actual_target_overlay_utf16_and_canonical_member_uri() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::new(true);
    let project = source_project(&package, "overlays", &["src"]);
    let disk = "package values\r\nval `answer😀` = 1\r\n";
    let source = "package app\r\nimport values.`answer😀`\r\nfun f() { val use = `answer😀` }\r\n";
    write_member(&project, "空.kt", source);
    write_member(&project, "値.kt", disk);
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/%E7%A9%BA.kt");
    let alias = uri.replace("%E7%A9%BA", "%e7%a9%ba");
    let target_uri = format!("{root}/src/%E5%80%A4.kt");
    let target_alias = target_uri.replace("%E5%80%A4", "%e5%80%a4");
    let mut wire = SignatureWire::new(&package);
    let init = wire.send(source_initialize(&source_options(&[&root])));
    assert!(init.get("error").is_none(), "{init}");
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":alias,"languageId":"kotlin","version":1,"text":source}}}),
    );
    let reference = source.rfind("`answer😀`").unwrap();
    let name = "`answer😀`";
    let reply = wire.send(definition(2, &alias, source, reference));
    assert_definition(
        &reply,
        &target_uri,
        disk,
        disk.find(name).unwrap(),
        name.len(),
    );
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target_alias,"languageId":"kotlin","version":1,"text":disk}}}),
    );
    let moved = disk.replacen("val ", "// 😀e\u{301}\r\nval /* 🦀 */ ", 1);
    let insertion = disk.find("val ").unwrap();
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_alias,"version":2},"contentChanges":[{
            "range":references_range(disk,insertion,4),"rangeLength":4,"text":"// 😀e\u{301}\r\nval /* 🦀 */ "}]}}));
    let reply = wire.send(definition(3, &alias, source, reference));
    assert_definition(
        &reply,
        &target_uri,
        &moved,
        moved.find(name).unwrap(),
        name.len(),
    );
    let prefix = "// requester 😀e\u{301}\r\n";
    let edited_source = format!("{prefix}{source}");
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":alias,"version":2},"contentChanges":[{
            "range":references_range(source,0,0),"rangeLength":0,"text":prefix}]}}),
    );
    let source = edited_source.as_str();
    let reference = source.rfind(name).unwrap();
    let reply = wire.send(definition(4, &alias, source, reference));
    assert_definition(
        &reply,
        &target_uri,
        &moved,
        moved.find(name).unwrap(),
        name.len(),
    );
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_alias,"version":3},"contentChanges":[{"text":"package values\nval gone = 1\n"}]}}));
    assert_refused(&wire.send(definition(6, &alias, source, reference)), -32803);
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":target_alias}}}));
    let reply = wire.send(definition(7, &alias, source, reference));
    assert_definition(
        &reply,
        &target_uri,
        disk,
        disk.find(name).unwrap(),
        name.len(),
    );
    wire.send(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target_uri,"languageId":"kotlin","version":1,"text":moved}}}),
    );
    let reply = wire.send(definition(8, &alias, source, reference));
    assert_definition(
        &reply,
        &target_uri,
        &moved,
        moved.find(name).unwrap(),
        name.len(),
    );
    assert_eq!(
        std::fs::read_to_string(project.join("src/値.kt")).unwrap(),
        disk
    );
    let (replies, status, stderr) = wire.terminate(false); // shutdown, then real EOF instead of exit.
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert!(stderr.is_empty(), "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(replies.len(), 8, "{replies:?}");
}

#[test]
fn imported_definition_joins_cancelled_work_before_slot_reuse_and_revalidates_before_poll() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "owned", &["src"]);
    let target = "package values\nval answer = 1\n";
    let source = "package app\nimport values.answer\nfun f() { val use = answer }\n";
    write_member(&project, "A.kt", target);
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Z.kt", "package unrelated\nval other = 2\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/A.kt");
    let options = source_options(&[&root]);
    let reference = source.rfind("answer").unwrap();
    let mut session = start_source_session(&options, &uri, source);
    // This is the first actual consumer; it waits on the original initial import.
    let mut initial = session.handle(definition(2, &uri, source, reference));
    initial.extend(collect_owned(&mut session));
    assert_definition(
        source_reply(&initial, 2),
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    assert_eq!(session.pending_requests(), 0, "{initial:?}");
    // All native work uses one eight-slot owner: symbols and imported definitions
    // compete, and a cancelled ID is freed without freeing its unjoined job.
    for id in 10..18 {
        let replies = session.handle(if id % 2 == 0 {
            definition(id, &uri, source, reference)
        } else {
            source_symbols(id, &uri)
        });
        assert!(replies.is_empty(), "{replies:?}");
    }
    assert_eq!(session.pending_requests(), 8);
    let duplicate = session.handle(definition(10, &uri, source, reference));
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert_eq!(duplicate[0]["id"], Value::Null, "{duplicate:?}");
    assert_refused(&duplicate[0], -32600);
    let cancelled =
        session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":10}}));
    assert_refused(source_reply(&cancelled, 10), -32800);
    assert_eq!(cancelled.len(), 1, "{cancelled:?}");
    assert_eq!(session.pending_requests(), 8);
    let exhausted = session.handle(definition(10, &uri, source, reference));
    assert_refused(source_reply(&exhausted, 10), -32803);
    assert_eq!(exhausted.len(), 1, "{exhausted:?}");
    let completed = collect_owned(&mut session);
    assert_eq!(completed.len(), 7, "{completed:?}");
    for id in 11..18 {
        let reply = source_reply(&completed, id);
        if id % 2 == 0 {
            assert_definition(
                reply,
                &target_uri,
                target,
                target.find("answer").unwrap(),
                6,
            );
        } else {
            assert!(reply.get("error").is_none(), "{reply}");
            assert_eq!(reply["result"][0]["name"], "f", "{reply}");
        }
    }
    assert_eq!(session.pending_requests(), 0, "{completed:?}");
    let mut reused = session.handle(definition(10, &uri, source, reference));
    reused.extend(collect_owned(&mut session));
    assert_eq!(reused.len(), 1, "{reused:?}");
    assert_definition(
        source_reply(&reused, 10),
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    // An accepted nonmatching overlay edit changes owner-visible epoch, even
    // if the background worker already finished before the FIRST poll.
    let nonmatching_uri = format!("{root}/src/Z.kt");
    open_uri(
        &mut session,
        &nonmatching_uri,
        "package unrelated\nval other = 2\n",
    );
    let admitted = session.handle(definition(30, &uri, source, reference));
    assert!(admitted.is_empty(), "{admitted:?}");
    let mut stale = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":nonmatching_uri,"version":2},"contentChanges":[{"text":"package unrelated\nval other = 3\n"}]}}));
    stale.extend(collect_owned(&mut session));
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert_refused(source_reply(&stale, 30), -32801);
    let mut reloaded = session.handle(source_reload(31, &options));
    reloaded.extend(collect_owned(&mut session));
    assert_eq!(reloaded.len(), 1, "{reloaded:?}");
    assert!(
        source_reply(&reloaded, 31).get("error").is_none(),
        "{reloaded:?}"
    );
    // Equal bytes/version after close+reopen still constitute a new incarnation.
    let admitted = session.handle(definition(32, &uri, source, reference));
    assert!(admitted.is_empty(), "{admitted:?}");
    let mut stale = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}));
    open_uri(&mut session, &uri, source);
    stale.extend(collect_owned(&mut session));
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert_refused(source_reply(&stale, 32), -32801);
    let mut recovered = session.handle(definition(33, &uri, source, reference));
    recovered.extend(collect_owned(&mut session));
    assert_definition(
        source_reply(&recovered, 33),
        &target_uri,
        target,
        target.find("answer").unwrap(),
        6,
    );
    open_uri(&mut session, &target_uri, target);
    for (index, action) in ["change", "rejected-version", "rejected-range", "reopen"]
        .into_iter()
        .enumerate()
    {
        let id = 40 + index as i32;
        let admitted = session.handle(definition(id, &uri, source, reference));
        assert!(admitted.is_empty(), "{action}: {admitted:?}");
        let mut replies = match action {
            "reopen" => {
                let replies = session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
                    "textDocument":{"uri":target_uri}}}));
                open_uri(&mut session, &target_uri, target);
                replies
            }
            _ => session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":target_uri,"version":if action == "rejected-range" { 3 } else { 2 }},
                "contentChanges":if action == "rejected-range" {
                    json!([{"range":{"start":{"line":99,"character":0},"end":{"line":99,"character":1}},"text":"bad"}])
                } else {
                    json!([{"text":if action == "change" { target } else { "package values\nval gone = 1\n" }}])
                }}})),
        };
        replies.extend(collect_owned(&mut session));
        assert_eq!(replies.len(), 1, "{action}: {replies:?}");
        let reply = source_reply(&replies, id);
        if action.starts_with("rejected-") {
            assert_definition(
                reply,
                &target_uri,
                target,
                target.find("answer").unwrap(),
                6,
            );
        } else {
            // Byte-identical accepted edits and close/reopen still invalidate
            // target version/incarnation captured before the first poll.
            assert_refused(reply, -32801);
        }
        assert_eq!(session.pending_requests(), 0, "{action}: {replies:?}");
    }
    let admitted = session.handle(definition(34, &uri, source, reference));
    assert!(admitted.is_empty(), "{admitted:?}");
    let (closed, clean) = session.close();
    assert!(clean, "{closed:?}");
    assert_eq!(closed.len(), 1, "{closed:?}");
    assert_refused(source_reply(&closed, 34), -32800);
    assert_eq!(session.pending_requests(), 0, "{closed:?}");
    assert!(session.poll().is_empty());
    // Import failure is not permission to disable the already-authenticated
    // local path. Both consumers start before any poll of this failed attempt.
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap(),
    )
    .unwrap();
    let local_source = "package app\nimport values.answer\nfun f() { val local = 1; val use = local; val remote = answer }\n";
    let mut failed_import = start_source_session(&options, &uri, local_source);
    let mut replies = failed_import.handle(definition(
        60,
        &uri,
        local_source,
        local_source.find("= local").unwrap() + 2,
    ));
    replies.extend(failed_import.handle(definition(
        61,
        &uri,
        local_source,
        local_source.rfind("answer").unwrap(),
    )));
    replies.extend(collect_owned(&mut failed_import));
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_definition(
        source_reply(&replies, 60),
        &uri,
        local_source,
        local_source.find("local =").unwrap(),
        5,
    );
    assert_refused(source_reply(&replies, 61), -32803);
    let (closed, clean) = failed_import.close();
    assert!(clean, "{closed:?}");
    assert!(closed.is_empty(), "{closed:?}");
    assert_eq!(failed_import.pending_requests(), 0);
}

#[test]
fn queued_local_type_and_call_definitions_ignore_unrelated_files_but_keep_explicit_cancellation() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "queued-definitions", &["src"]);
    let source = "package app\nfun <T> f(p:T) { val local = 1; val use = local; fun nested() {}; nested() }\n";
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Other.kt", "package app\nval other = 1\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let other = format!("{root}/src/Other.kt");
    let options = source_options(&[&root]);
    let local_use = source.find("= local").unwrap() + 2;
    let local_target = source.find("local =").unwrap();
    let mut session = start_source_session(&options, &uri, source);
    let mut replies = Vec::new();
    for (id, offset) in [
        (10, local_use),
        (11, source.find("p:T").unwrap() + 2),
        (12, source.rfind("nested()").unwrap()),
    ] {
        replies.extend(session.handle(definition(id, &uri, source, offset)));
    }
    // pending_import remains owned until FIRST poll, even if its thread has
    // finished. These three consumers are deterministically initial waiters.
    assert_eq!(session.pending_requests(), 4);
    replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":other,"languageId":"kotlin","version":1,"text":"package app\nval other = 1\n"}}})));
    replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":other,"version":2},"contentChanges":[{"text":"package app\nval other = 2\n"}]}})));
    replies.extend(session.handle(
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":other}}}),
    ));
    replies.extend(collect_owned(&mut session));
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert_definition(source_reply(&replies, 10), &uri, source, local_target, 5);
    assert_definition(
        source_reply(&replies, 11),
        &uri,
        source,
        source.find("<T>").unwrap() + 1,
        1,
    );
    assert_definition(
        source_reply(&replies, 12),
        &uri,
        source,
        source.find("nested()").unwrap(),
        6,
    );
    let (closed, clean) = session.close();
    assert!(clean, "{closed:?}");
    assert!(closed.is_empty(), "{closed:?}");

    let mut cancelled = start_source_session(&options, &uri, source);
    let mut replies = cancelled.handle(definition(60, &uri, source, local_use));
    replies.extend(
        cancelled.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":60}})),
    );
    replies.extend(collect_owned(&mut cancelled));
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_refused(source_reply(&replies, 60), -32800);
    let mut reused = cancelled.handle(definition(60, &uri, source, local_use));
    reused.extend(collect_owned(&mut cancelled));
    assert_eq!(reused.len(), 1, "{reused:?}");
    assert_definition(source_reply(&reused, 60), &uri, source, local_target, 5);
    let (closed, clean) = cancelled.close();
    assert!(clean, "{closed:?}");
    assert!(closed.is_empty(), "{closed:?}");

    let mut superseded = start_source_session(&options, &uri, source);
    let mut replies = superseded.handle(definition(70, &uri, source, local_use));
    replies.extend(superseded.handle(source_reload(71, &options)));
    replies.extend(collect_owned(&mut superseded));
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_refused(source_reply(&replies, 70), -32801);
    assert_refused(source_reply(&replies, 71), -32803);
    let (closed, clean) = superseded.close();
    assert!(clean, "{closed:?}");
    assert!(closed.is_empty(), "{closed:?}");
}

#[test]
fn imported_hover_reads_target_overlay_but_keeps_requester_utf16_range_through_edits_and_renames() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "hover-overlays", &["src"]);
    let name = "`answer😀`";
    let disk = "package values\r\nval `answer😀` = 17\r\n";
    let source = "package app\r\nimport values.`answer😀`\r\nfun f() { val use = `answer😀` }\r\n";
    write_member(&project, "Target.kt", disk);
    write_member(&project, "Uses.kt", source);
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/Target.kt");
    let options = source_options(&[&root]);
    let mut session = start_source_session(&options, &uri, source);
    let replies = settled(
        &mut session,
        hover(2, &uri, source, source.rfind(name).unwrap()),
    );
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_hover(
        source_reply(&replies, 2),
        "val `answer😀` = 17",
        source,
        source.rfind(name).unwrap(),
        name.len(),
    );

    open_uri(&mut session, &target_uri, disk);
    // Change declaration contents and coordinates independently of the requester.
    let replacement = "// target 😀e\u{301}\r\nval /* 🦀 */ `answer😀` = 29";
    let target = format!("package values\r\n{replacement}\r\n");
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_uri,"version":2},"contentChanges":[{
            "range":references_range(disk,disk.find("val ").unwrap(),"val `answer😀` = 17".len()),
            "rangeLength":"val `answer😀` = 17".encode_utf16().count(),"text":replacement}]}})
            )
            .is_empty()
    );
    let replies = settled(
        &mut session,
        hover(3, &uri, source, source.rfind(name).unwrap()),
    );
    assert_hover(
        source_reply(&replies, 3),
        "val /* 🦀 */ `answer😀` = 29",
        source,
        source.rfind(name).unwrap(),
        name.len(),
    );
    let prefix = "// requester 😀e\u{301}\r\n";
    let requester = format!("{prefix}{source}");
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":2},"contentChanges":[{
            "range":references_range(source,0,0),"rangeLength":0,"text":prefix}]}})
            )
            .is_empty()
    );
    let replies = settled(
        &mut session,
        hover(4, &uri, &requester, requester.rfind(name).unwrap()),
    );
    assert_hover(
        source_reply(&replies, 4),
        "val /* 🦀 */ `answer😀` = 29",
        &requester,
        requester.rfind(name).unwrap(),
        name.len(),
    );

    let renamed = "`renamed🦀`";
    let renamed_target = target.replace(name, renamed);
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_uri,"version":3},"contentChanges":[{
            "range":references_range(&target,target.find(name).unwrap(),name.len()),
            "rangeLength":name.encode_utf16().count(),"text":renamed}]}})
            )
            .is_empty()
    );
    let replies = settled(
        &mut session,
        hover(5, &uri, &requester, requester.rfind(name).unwrap()),
    );
    assert_refused(source_reply(&replies, 5), -32803);
    let import = requester.find(name).unwrap();
    let reference = requester.rfind(name).unwrap();
    let renamed_requester = requester.replace(name, renamed);
    // Reverse source order makes both ranges genuine coordinates in the old text.
    assert!(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[
            {"range":references_range(&requester,reference,name.len()),"rangeLength":name.encode_utf16().count(),"text":renamed},
            {"range":references_range(&requester,import,name.len()),"rangeLength":name.encode_utf16().count(),"text":renamed}]}})).is_empty());
    let replies = settled(
        &mut session,
        hover(
            6,
            &uri,
            &renamed_requester,
            renamed_requester.rfind(renamed).unwrap(),
        ),
    );
    assert_hover(
        source_reply(&replies, 6),
        "val /* 🦀 */ `renamed🦀` = 29",
        &renamed_requester,
        renamed_requester.rfind(renamed).unwrap(),
        renamed.len(),
    );
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":target_uri}}})
            )
            .is_empty()
    );
    let replies = settled(
        &mut session,
        hover(
            7,
            &uri,
            &renamed_requester,
            renamed_requester.rfind(renamed).unwrap(),
        ),
    );
    assert_refused(source_reply(&replies, 7), -32803);
    open_uri(&mut session, &target_uri, &renamed_target);
    let replies = settled(
        &mut session,
        hover(
            8,
            &uri,
            &renamed_requester,
            renamed_requester.rfind(renamed).unwrap(),
        ),
    );
    assert_hover(
        source_reply(&replies, 8),
        "val /* 🦀 */ `renamed🦀` = 29",
        &renamed_requester,
        renamed_requester.rfind(renamed).unwrap(),
        renamed.len(),
    );
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{
        "textDocument":{"uri":uri}}})
            )
            .is_empty()
    );
    open_uri(&mut session, &uri, &renamed_requester);
    let replies = settled(
        &mut session,
        hover(
            9,
            &uri,
            &renamed_requester,
            renamed_requester.rfind(renamed).unwrap(),
        ),
    );
    assert_hover(
        source_reply(&replies, 9),
        "val /* 🦀 */ `renamed🦀` = 29",
        &renamed_requester,
        renamed_requester.rfind(renamed).unwrap(),
        renamed.len(),
    );
    assert_eq!(
        std::fs::read_to_string(project.join("src/Target.kt")).unwrap(),
        disk
    );
    assert_eq!(
        std::fs::read_to_string(project.join("src/Uses.kt")).unwrap(),
        source
    );
    let (closed, clean) = session.close();
    assert!(clean && closed.is_empty(), "{closed:?}");
}

#[test]
fn imported_hover_refuses_complete_negative_source_model_and_candidate_evidence_until_reload() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "hover-freshness", &["src"]);
    let target = "package values\nval answer = 17\n";
    let source = "package app\nimport values.answer\nfun f() { val use = answer }\n";
    let negative = source.replace("answer", "absent");
    let other = "package unrelated\nval other = 2\n";
    write_member(&project, "A.kt", target);
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Z.kt", other);
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let options = source_options(&[&root]);
    let mut session = start_source_session(&options, &uri, source);
    let replies = settled(
        &mut session,
        hover(2, &uri, source, source.rfind("answer").unwrap()),
    );
    assert_hover(
        source_reply(&replies, 2),
        "val answer = 17",
        source,
        source.rfind("answer").unwrap(),
        6,
    );
    write_member(&project, "Z.kt", "package unrelated\nval other = 3\n");
    for (id, text, version) in [(3, source, 2), (4, negative.as_str(), 3)] {
        assert!(
            session
                .handle(
                    json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":text}]}})
                )
                .is_empty()
        );
        let replies = settled(
            &mut session,
            hover(id, &uri, text, text.rfind("= ").unwrap() + 2),
        );
        assert_refused(source_reply(&replies, id), -32801);
    }
    let replies = settled(&mut session, source_reload(5, &options));
    assert!(
        source_reply(&replies, 5).get("error").is_none(),
        "{replies:?}"
    );
    let replies = settled(
        &mut session,
        hover(6, &uri, &negative, negative.rfind("absent").unwrap()),
    );
    assert_refused(source_reply(&replies, 6), -32803);
    let mut model = serde_json::to_vec(&source_model(&["src"])).unwrap();
    model.push(b'\n');
    std::fs::write(project.join("workspace.json"), model).unwrap();
    let replies = settled(
        &mut session,
        hover(7, &uri, &negative, negative.rfind("absent").unwrap()),
    );
    assert_refused(source_reply(&replies, 7), -32801);
    assert!(
        session
            .handle(
                json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":4},"contentChanges":[{"text":source}]}})
            )
            .is_empty()
    );
    let replies = settled(
        &mut session,
        hover(8, &uri, source, source.rfind("answer").unwrap()),
    );
    assert_refused(source_reply(&replies, 8), -32801);
    for (index, (candidate, late)) in [
        (target, "package `values`\nval `answer` = 33\n"),
        ("package values\nprivate val answer = 17\n", other),
        ("package values\nvar answer = 17\n", other),
        ("package values\nval gone = 17\n", other),
        (target, "package unrelated\nval other =\n"),
        (target, other),
    ]
    .into_iter()
    .enumerate()
    {
        write_member(&project, "A.kt", candidate);
        write_member(&project, "Z.kt", late);
        let id = 10 + index as i32 * 2;
        let replies = settled(&mut session, source_reload(id, &options));
        assert!(
            source_reply(&replies, id).get("error").is_none(),
            "{replies:?}"
        );
        let replies = settled(
            &mut session,
            hover(id + 1, &uri, source, source.rfind("answer").unwrap()),
        );
        if index == 5 {
            assert_hover(
                source_reply(&replies, id + 1),
                "val answer = 17",
                source,
                source.rfind("answer").unwrap(),
                6,
            );
        } else {
            assert_refused(source_reply(&replies, id + 1), -32803);
        }
    }
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap(),
    )
    .unwrap();
    let replies = settled(&mut session, source_reload(30, &options));
    assert_refused(source_reply(&replies, 30), -32803);
    let replies = settled(
        &mut session,
        hover(31, &uri, source, source.rfind("answer").unwrap()),
    );
    assert_refused(source_reply(&replies, 31), -32803);
    std::fs::write(
        project.join("workspace.json"),
        serde_json::to_vec(&source_model(&["src"])).unwrap(),
    )
    .unwrap();
    let replies = settled(&mut session, source_reload(32, &options));
    assert!(
        source_reply(&replies, 32).get("error").is_none(),
        "{replies:?}"
    );
    let replies = settled(
        &mut session,
        hover(33, &uri, source, source.rfind("answer").unwrap()),
    );
    assert_hover(
        source_reply(&replies, 33),
        "val answer = 17",
        source,
        source.rfind("answer").unwrap(),
        6,
    );
    let (closed, clean) = session.close();
    assert!(clean && closed.is_empty(), "{closed:?}");
}

#[test]
fn queued_imported_local_and_declaration_hovers_keep_query_format_and_unrelated_notifications() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "hover-waiters", &["src"]);
    let declaration = "val /*\r\n``````\r\n[foreign](https://invalid.test)\r\n*/ `answer😀` = 17";
    let target = format!("package values\r\n{declaration}\r\n");
    let source = "package app\r\nimport values.`answer😀`\r\nfun f() { val local = 2; val use = local; val remote = `answer😀` }\r\n";
    write_member(&project, "A.kt", &target);
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Other.kt", "package app\nval other = 1\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let other = format!("{root}/src/Other.kt");
    let options = source_options(&[&root]);
    for (format, failed) in [
        ("plaintext", false),
        ("markdown", false),
        ("markdown", true),
    ] {
        if failed {
            std::fs::write(
                project.join("workspace.json"),
                serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap(),
            )
            .unwrap();
        }
        let mut session = Session::with_parser(worker()).unwrap();
        let mut init = source_initialize(&options);
        init["params"]["capabilities"]["textDocument"]["hover"] = json!({"contentFormat":[format]});
        let initialized = session.handle(init);
        assert_eq!(initialized.len(), 1, "{initialized:?}");
        assert!(initialized[0].get("error").is_none(), "{initialized:?}");
        notify(&mut session, "initialized", json!({}));
        open_uri(&mut session, &uri, source);
        let mut replies = Vec::new();
        for (id, offset) in [
            (10, source.rfind("`answer😀`").unwrap()),
            (11, source.find("= local").unwrap() + 2),
            (12, source.find("local =").unwrap()),
        ] {
            replies.extend(session.handle(hover(id, &uri, source, offset)));
        }
        assert!(replies.is_empty(), "{format}/{failed}: {replies:?}");
        assert_eq!(session.pending_requests(), 4);
        for message in [
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":other,"languageId":"kotlin","version":1,"text":"package app\nval other = 1\n"}}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":other,"version":2},"contentChanges":[{"text":"package app\nval other = 3\n"}]}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":other}}}),
        ] {
            replies.extend(session.handle(message));
        }
        replies.extend(collect_owned(&mut session));
        assert_eq!(session.pending_requests(), 0, "{replies:?}");
        assert_eq!(replies.len(), 3, "{format}/{failed}: {replies:?}");
        for (id, offset) in [
            (11, source.find("= local").unwrap() + 2),
            (12, source.find("local =").unwrap()),
        ] {
            let reply = source_reply(&replies, id);
            let value = if format == "markdown" {
                "```kotlin\nval local = 2\n```"
            } else {
                "val local = 2"
            };
            assert!(reply.get("error").is_none(), "{reply}");
            assert_eq!(
                reply["result"],
                json!({"contents":{"kind":format,"value":value},
                "range":references_range(source,offset,5)}),
                "{reply}"
            );
        }
        let reply = source_reply(&replies, 10);
        if failed {
            assert_refused(reply, -32803);
        } else {
            let value = if format == "markdown" {
                format!("```````kotlin\n{declaration}\n```````")
            } else {
                declaration.to_owned()
            };
            assert!(reply.get("error").is_none(), "{reply}");
            assert_eq!(
                reply["result"],
                json!({"contents":{"kind":format,"value":value},
                "range":references_range(source,source.rfind("`answer😀`").unwrap(),"`answer😀`".len())}),
                "{reply}"
            );
        }
        let (closed, clean) = session.close();
        assert!(clean && closed.is_empty(), "{closed:?}");
    }
}

#[test]
fn queued_hover_requester_freshness_cancellation_reuse_and_reload_are_not_lost() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "hover-revocation", &["src"]);
    let target = "package values\nval answer = 17\n";
    let source = "package app\nimport values.answer\nfun f() { val local = 2; val use = local; val remote = answer }\n";
    write_member(&project, "A.kt", target);
    write_member(&project, "Uses.kt", source);
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let options = source_options(&[&root]);
    for action in [
        "version",
        "incarnation",
        "cancel",
        "reload",
        "configuration",
    ] {
        let mut session = start_source_session(&options, &uri, source);
        let mut replies = session.handle(hover(10, &uri, source, source.rfind("answer").unwrap()));
        replies.extend(session.handle(hover(
            11,
            &uri,
            source,
            source.find("= local").unwrap() + 2,
        )));
        assert!(replies.is_empty(), "{action}: {replies:?}");
        assert_eq!(session.pending_requests(), 3);
        match action {
            "version" => replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":source.replace("local = 2", "local = 9")}]}}))),
            "incarnation" => {
                replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}})));
                replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                    "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}})));
            }
            "cancel" => {
                for id in [10, 11] {
                    replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":id}})));
                }
            }
            "reload" => replies.extend(session.handle(source_reload(12, &options))),
            "configuration" => replies.extend(session.handle(json!({"jsonrpc":"2.0","method":"workspace/didChangeWorkspaceFolders","params":{
                "event":{"added":[{"uri":root,"name":"owned"}],"removed":[]}}}))),
            _ => unreachable!(),
        }
        replies.extend(collect_owned(&mut session));
        assert_eq!(session.pending_requests(), 0, "{replies:?}");
        assert_eq!(
            replies.len(),
            if action == "reload" { 3 } else { 2 },
            "{action}: {replies:?}"
        );
        for id in [10, 11] {
            assert_refused(
                source_reply(&replies, id),
                if action == "cancel" { -32800 } else { -32801 },
            );
        }
        if action == "reload" {
            // Supersession cannot replace an import whose owner has not joined.
            assert_refused(source_reply(&replies, 12), -32803);
        }
        if matches!(action, "reload" | "configuration") {
            let replies = settled(&mut session, source_reload(13, &options));
            assert!(
                source_reply(&replies, 13).get("error").is_none(),
                "{replies:?}"
            );
        }
        let current = if action == "version" {
            source.replace("local = 2", "local = 9")
        } else {
            source.to_owned()
        };
        let replies = settled(
            &mut session,
            hover(11, &uri, &current, current.find("= local").unwrap() + 2),
        );
        assert_eq!(replies.len(), 1, "{replies:?}");
        assert_hover(
            source_reply(&replies, 11),
            if action == "version" {
                "val local = 9"
            } else {
                "val local = 2"
            },
            &current,
            current.find("= local").unwrap() + 2,
            5,
        );
        let replies = settled(
            &mut session,
            hover(10, &uri, &current, current.rfind("answer").unwrap()),
        );
        assert_eq!(replies.len(), 1, "{replies:?}");
        assert_hover(
            source_reply(&replies, 10),
            "val answer = 17",
            &current,
            current.rfind("answer").unwrap(),
            6,
        );
        let (closed, clean) = session.close();
        assert!(clean && closed.is_empty(), "{closed:?}");
    }
}

#[test]
fn imported_hover_shares_eight_owned_slots_and_revalidates_before_publication_and_close() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let package = Package::source_only();
    let project = source_project(&package, "hover-owned", &["src"]);
    let target = "package values\nval answer = 17\n";
    let source = "package app\nimport values.answer\nfun f() { val use = answer }\n";
    write_member(&project, "A.kt", target);
    write_member(&project, "Uses.kt", source);
    write_member(&project, "Z.kt", "package unrelated\nval other = 2\n");
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/A.kt");
    let other_uri = format!("{root}/src/Z.kt");
    let options = source_options(&[&root]);
    let reference = source.rfind("answer").unwrap();
    let mut session = start_source_session(&options, &uri, source);
    let replies = settled(&mut session, hover(2, &uri, source, reference));
    assert_hover(
        source_reply(&replies, 2),
        "val answer = 17",
        source,
        reference,
        6,
    );
    for id in 10..18 {
        let replies = session.handle(match id % 3 {
            0 => definition(id, &uri, source, reference),
            1 => hover(id, &uri, source, reference),
            _ => source_symbols(id, &uri),
        });
        assert!(replies.is_empty(), "{replies:?}");
    }
    assert_eq!(session.pending_requests(), 8);
    let duplicate = session.handle(hover(10, &uri, source, reference));
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert_eq!(duplicate[0]["id"], Value::Null, "{duplicate:?}");
    assert_refused(&duplicate[0], -32600);
    let cancelled =
        session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":10}}));
    assert_eq!(cancelled.len(), 1, "{cancelled:?}");
    assert_refused(source_reply(&cancelled, 10), -32800);
    assert_eq!(session.pending_requests(), 8);
    let exhausted = session.handle(hover(10, &uri, source, reference));
    assert_eq!(exhausted.len(), 1, "{exhausted:?}");
    assert_refused(source_reply(&exhausted, 10), -32803);
    let replies = collect_owned(&mut session);
    assert_eq!(session.pending_requests(), 0, "{replies:?}");
    assert_eq!(replies.len(), 7, "{replies:?}");
    for id in 11..18 {
        let reply = source_reply(&replies, id);
        match id % 3 {
            0 => assert_definition(
                reply,
                &target_uri,
                target,
                target.find("answer").unwrap(),
                6,
            ),
            1 => assert_hover(reply, "val answer = 17", source, reference, 6),
            _ => {
                assert!(reply.get("error").is_none(), "{reply}");
                assert_eq!(reply["result"][0]["name"], "f", "{reply}");
            }
        }
    }
    let replies = settled(&mut session, hover(10, &uri, source, reference));
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_hover(
        source_reply(&replies, 10),
        "val answer = 17",
        source,
        reference,
        6,
    );
    open_uri(&mut session, &target_uri, target);
    open_uri(
        &mut session,
        &other_uri,
        "package unrelated\nval other = 2\n",
    );
    for (index, changed) in [&target_uri, &other_uri, &uri].into_iter().enumerate() {
        let id = 30 + index as i32;
        assert!(
            session
                .handle(hover(id, &uri, source, reference))
                .is_empty()
        );
        let text = if changed == &target_uri {
            "package values\nval answer = 99\n"
        } else if changed == &other_uri {
            "package unrelated\nval other = 3\n"
        } else {
            source
        };
        let replies = settled(
            &mut session,
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":changed,"version":2},"contentChanges":[{"text":text}]}}),
        );
        assert_eq!(replies.len(), 1, "{replies:?}");
        assert_refused(source_reply(&replies, id), -32801);
    }
    let replies = settled(&mut session, hover(40, &uri, source, reference));
    assert_hover(
        source_reply(&replies, 40),
        "val answer = 99",
        source,
        reference,
        6,
    );
    assert!(
        session
            .handle(hover(41, &uri, source, reference))
            .is_empty()
    );
    let (closed, clean) = session.close();
    assert!(clean, "{closed:?}");
    assert_eq!(closed.len(), 1, "{closed:?}");
    assert_refused(source_reply(&closed, 41), -32800);
    assert_eq!(session.pending_requests(), 0);
    assert!(session.poll().is_empty());
}
