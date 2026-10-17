//! Real imported-source publication and owned-work regressions; source-only under #96.
use super::*;

fn definition(id: i32, uri: &str, source: &str, offset: usize) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"textDocument/definition","params":{
        "textDocument":{"uri":uri},"position":offset_to_position(source, offset).unwrap()}})
}

fn assert_definition(reply: &Value, uri: &str, source: &str, offset: usize, width: usize) {
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(reply["result"], json!({"uri":uri,"range":references_range(source,offset,width)}), "{reply}");
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
        ("fun f() { val answer = 2; val use = answer }", Some("val answer")),
        ("fun f(`answer`: Int) { val use = answer }", Some("`answer`")),
        ("fun f(`answer`: Int) { val answer = answer }", Some("`answer`")),
        ("fun f(answer: Int) { val use = answer; val answer = 2 }", Some("answer: Int")),
        ("fun f() { val answer = answer }", Some("")),
        ("fun f() { val use = answer; val answer = 2 }", Some("")),
        ("fun f() { missing(); val use = answer }", None),
        ("fun f() { val use = answer; missing() }", None),
        ("fun f() { val use = answer; var later = 1 }", None),
        ("fun f() { val answer = 1; val `answer` = 2; val use = answer }", None),
        ("fun f() { val use = answer + 1 }", None),
    ].into_iter().enumerate() {
        let source = format!("package app\nimport values.answer\n{body}\n");
        if index == 0 {
            wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
                "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}));
        } else {
            wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
                "textDocument":{"uri":uri,"version":index+1},"contentChanges":[{"text":source}]}}));
        }
        let reference = source.find("= answer").unwrap() + 2;
        let reply = wire.send(definition(index as i32 + 2, &uri, &source, reference));
        match local {
            None => assert_refused(&reply, -32803),
            Some("") => assert_definition(&reply, &target_uri, target, target.find("`answer`").unwrap(), "`answer`".len()),
            Some(marker) => {
                let start = source.find(marker).unwrap() + if marker.starts_with("val ") { 4 } else { 0 };
                let width = if marker.starts_with('`') { "`answer`".len() } else { "answer".len() };
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
    std::fs::write(project.join("late/Z.kt"), "package unrelated\nval other = 2\n").unwrap();
    let root = source_directory_uri(&project);
    let uri = format!("{root}/src/Uses.kt");
    let target_uri = format!("{root}/src/A.kt");
    let options = source_options(&[&root]);
    let mut wire = SignatureWire::new(&package);
    let init = wire.send(source_initialize(&options));
    assert!(init.get("error").is_none(), "{init}");
    wire.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}));
    let reference = source.rfind("answer").unwrap();
    let first = wire.send(definition(2, &uri, source, reference));
    assert_definition(&first, &target_uri, good, good.find("answer").unwrap(), 6);
    for (index, (target, late)) in [
        (good, "package `values`\nval `answer` = 2\n"),
        ("package values\nprivate val answer = 1\n", "package unrelated\nval other = 2\n"),
        ("package values\nvar answer = 1\n", "package unrelated\nval other = 2\n"),
        (good, "package unrelated\nval other =\n"),
    ].into_iter().enumerate() {
        write_member(&project, "A.kt", target);
        std::fs::write(project.join("late/Z.kt"), late).unwrap();
        let reload = wire.send(source_reload(10 + index as i32 * 2, &options));
        assert!(reload.get("error").is_none(), "{reload}");
        let reply = wire.send(definition(11 + index as i32 * 2, &uri, source, reference));
        assert_refused(&reply, -32803);
    }
    write_member(&project, "A.kt", good);
    std::fs::write(project.join("late/Z.kt"), "package unrelated\nval other = 2\n").unwrap();
    let reloaded = wire.send(source_reload(20, &options));
    assert!(reloaded.get("error").is_none(), "{reloaded}");
    for (index, imports) in ["import values.answer\nimport values.answer", "import values.answer\nimport `values`.`answer`"].into_iter().enumerate() {
        let ambiguous = format!("package app\n{imports}\nfun f() {{ val use = answer }}\n");
        wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
            "textDocument":{"uri":uri,"version":index+2},"contentChanges":[{"text":ambiguous}]}}));
        let reply = wire.send(definition(21 + index as i32, &uri, &ambiguous, ambiguous.rfind("answer").unwrap()));
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
    let absent = source.replace("values.answer", "values.absent").replace("= answer", "= absent");
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
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"kotlin","version":1,"text":source}}}));
    let first = wire.send(definition(2, &uri, source, source.rfind("answer").unwrap()));
    assert_definition(&first, &target_uri, target, target.find("answer").unwrap(), 6);
    // The changed file never matched the import. Both positive and negative
    // lookups must verify it, rather than publish a target or a cached absence.
    write_member(&project, "Z.kt", "package unrelated\nval other = 3\n");
    let stale = wire.send(definition(3, &uri, source, source.rfind("answer").unwrap()));
    assert_refused(&stale, -32801);
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":absent}]}}));
    let negative = wire.send(definition(4, &uri, &absent, absent.rfind("absent").unwrap()));
    assert_refused(&negative, -32801);
    let reload = wire.send(source_reload(5, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let negative = wire.send(definition(6, &uri, &absent, absent.rfind("absent").unwrap()));
    assert_refused(&negative, -32803);
    // A byte-only model edit still invalidates original model authority.
    let mut model = serde_json::to_vec(&source_model(&["src"])).unwrap();
    model.push(b'\n');
    std::fs::write(project.join("workspace.json"), model).unwrap();
    let stale = wire.send(definition(7, &uri, &absent, absent.rfind("absent").unwrap()));
    assert_refused(&stale, -32801);
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":uri,"version":3},"contentChanges":[{"text":source}]}}));
    let stale = wire.send(definition(8, &uri, source, source.rfind("answer").unwrap()));
    assert_refused(&stale, -32801);
    let reload = wire.send(source_reload(9, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let recovered = wire.send(definition(10, &uri, source, source.rfind("answer").unwrap()));
    assert_definition(&recovered, &target_uri, target, target.find("answer").unwrap(), 6);
    // A failed replacement must revoke the historical snapshot, not keep it.
    std::fs::write(project.join("workspace.json"), serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap()).unwrap();
    assert_refused(&wire.send(source_reload(11, &options)), -32803);
    assert_refused(&wire.send(definition(12, &uri, source, source.rfind("answer").unwrap())), -32803);
    std::fs::write(project.join("workspace.json"), serde_json::to_vec(&source_model(&["src"])).unwrap()).unwrap();
    let reload = wire.send(source_reload(13, &options));
    assert!(reload.get("error").is_none(), "{reload}");
    let recovered = wire.send(definition(14, &uri, source, source.rfind("answer").unwrap()));
    assert_definition(&recovered, &target_uri, target, target.find("answer").unwrap(), 6);
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
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":alias,"languageId":"kotlin","version":1,"text":source}}}));
    let reference = source.rfind("`answer😀`").unwrap();
    let name = "`answer😀`";
    let reply = wire.send(definition(2, &alias, source, reference));
    assert_definition(&reply, &target_uri, disk, disk.find(name).unwrap(), name.len());
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target_alias,"languageId":"kotlin","version":1,"text":disk}}}));
    let moved = disk.replacen("val ", "// 😀e\u{301}\r\nval /* 🦀 */ ", 1);
    let insertion = disk.find("val ").unwrap();
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_alias,"version":2},"contentChanges":[{
            "range":references_range(disk,insertion,4),"rangeLength":4,"text":"// 😀e\u{301}\r\nval /* 🦀 */ "}]}}));
    let reply = wire.send(definition(3, &alias, source, reference));
    assert_definition(&reply, &target_uri, &moved, moved.find(name).unwrap(), name.len());
    let prefix = "// requester 😀e\u{301}\r\n";
    let edited_source = format!("{prefix}{source}");
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":alias,"version":2},"contentChanges":[{
            "range":references_range(source,0,0),"rangeLength":0,"text":prefix}]}}));
    let source = edited_source.as_str();
    let reference = source.rfind(name).unwrap();
    let reply = wire.send(definition(4, &alias, source, reference));
    assert_definition(&reply, &target_uri, &moved, moved.find(name).unwrap(), name.len());
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{
        "textDocument":{"uri":target_alias,"version":3},"contentChanges":[{"text":"package values\nval gone = 1\n"}]}}));
    assert_refused(&wire.send(definition(6, &alias, source, reference)), -32803);
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":target_alias}}}));
    let reply = wire.send(definition(7, &alias, source, reference));
    assert_definition(&reply, &target_uri, disk, disk.find(name).unwrap(), name.len());
    wire.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":target_uri,"languageId":"kotlin","version":1,"text":moved}}}));
    let reply = wire.send(definition(8, &alias, source, reference));
    assert_definition(&reply, &target_uri, &moved, moved.find(name).unwrap(), name.len());
    assert_eq!(std::fs::read_to_string(project.join("src/値.kt")).unwrap(), disk);
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
    assert_definition(source_reply(&initial, 2), &target_uri, target, target.find("answer").unwrap(), 6);
    assert_eq!(session.pending_requests(), 0, "{initial:?}");
    // All native work uses one eight-slot owner: symbols and imported definitions
    // compete, and a cancelled ID is freed without freeing its unjoined job.
    for id in 10..18 {
        let replies = session.handle(if id % 2 == 0 { definition(id, &uri, source, reference) } else { source_symbols(id, &uri) });
        assert!(replies.is_empty(), "{replies:?}");
    }
    assert_eq!(session.pending_requests(), 8);
    let duplicate = session.handle(definition(10, &uri, source, reference));
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert_eq!(duplicate[0]["id"], Value::Null, "{duplicate:?}");
    assert_refused(&duplicate[0], -32600);
    let cancelled = session.handle(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":10}}));
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
            assert_definition(reply, &target_uri, target, target.find("answer").unwrap(), 6);
        } else {
            assert!(reply.get("error").is_none(), "{reply}");
            assert_eq!(reply["result"][0]["name"], "f", "{reply}");
        }
    }
    assert_eq!(session.pending_requests(), 0, "{completed:?}");
    let mut reused = session.handle(definition(10, &uri, source, reference));
    reused.extend(collect_owned(&mut session));
    assert_eq!(reused.len(), 1, "{reused:?}");
    assert_definition(source_reply(&reused, 10), &target_uri, target, target.find("answer").unwrap(), 6);
    // An accepted nonmatching overlay edit changes owner-visible epoch, even
    // if the background worker already finished before the FIRST poll.
    let nonmatching_uri = format!("{root}/src/Z.kt");
    open_uri(&mut session, &nonmatching_uri, "package unrelated\nval other = 2\n");
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
    assert!(source_reply(&reloaded, 31).get("error").is_none(), "{reloaded:?}");
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
    assert_definition(source_reply(&recovered, 33), &target_uri, target, target.find("answer").unwrap(), 6);
    open_uri(&mut session, &target_uri, target);
    for (index, action) in ["change", "rejected-version", "rejected-range", "reopen"].into_iter().enumerate() {
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
            assert_definition(reply, &target_uri, target, target.find("answer").unwrap(), 6);
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
    std::fs::write(project.join("workspace.json"), serde_json::to_vec(&source_model(&["src", "missing-last"])).unwrap()).unwrap();
    let local_source = "package app\nimport values.answer\nfun f() { val local = 1; val use = local; val remote = answer }\n";
    let mut failed_import = start_source_session(&options, &uri, local_source);
    let mut replies = failed_import.handle(definition(60, &uri, local_source, local_source.find("= local").unwrap() + 2));
    replies.extend(failed_import.handle(definition(61, &uri, local_source, local_source.rfind("answer").unwrap())));
    replies.extend(collect_owned(&mut failed_import));
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_definition(source_reply(&replies, 60), &uri, local_source, local_source.find("local =").unwrap(), 5);
    assert_refused(source_reply(&replies, 61), -32803);
    let (closed, clean) = failed_import.close();
    assert!(clean, "{closed:?}");
    assert!(closed.is_empty(), "{closed:?}");
    assert_eq!(failed_import.pending_requests(), 0);
}
