//! Finite nullable receiver expectations frozen before source implementation or runtime.
use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS"
    )
}

fn projection(tree: &Parsed) -> Value {
    let elements = tree.elements();
    Value::Array(
        elements
            .iter()
            .filter(|e| {
                !e.token
                    && (declaration(&e.kind)
                        || matches!(
                            e.kind.as_str(),
                            "TYPE_PARAMETER_LIST"
                                | "TYPE_PARAMETER"
                                | "TYPE_REFERENCE"
                                | "NULLABLE_TYPE"
                                | "USER_TYPE"
                                | "DYNAMIC_TYPE"
                                | "REFERENCE_EXPRESSION"
                                | "VALUE_PARAMETER_LIST"
                                | "VALUE_PARAMETER"
                                | "TYPE_ARGUMENT_LIST"
                                | "TYPE_PROJECTION"
                                | "ANNOTATION_ENTRY"
                                | "PROPERTY_ACCESSOR"
                        ))
            })
            .map(|e| {
                let mut parent = e.parent;
                let owner = loop {
                    match parent {
                        Some(id) if declaration(&elements[id].kind) => {
                            break Some((elements[id].start, elements[id].end));
                        }
                        Some(id) => parent = elements[id].parent,
                        None => break None,
                    }
                };
                json!([
                    e.kind,
                    e.start,
                    e.end,
                    elements[e.parent.unwrap()].kind,
                    owner
                ])
            })
            .collect(),
    )
}

fn names(tree: &Parsed) -> Value {
    let elements = tree.elements();
    Value::Array(
        elements
            .iter()
            .filter(|e| {
                e.token
                    && e.kind == "IDENTIFIER"
                    && e.parent.is_some_and(|p| declaration(&elements[p].kind))
            })
            .map(|e| {
                json!([
                    elements[e.parent.unwrap()].start,
                    e.start,
                    e.end,
                    &tree.source()[e.start..e.end]
                ])
            })
            .collect(),
    )
}

fn diagnostics(tree: &Parsed) -> Value {
    Value::Array(
        tree.diagnostics()
            .iter()
            .map(|d| json!([d.start, d.end, d.message]))
            .collect(),
    )
}

fn positive(tree: &Parsed, case: &Value) {
    let id = case["id"].as_str().unwrap();
    assert_eq!(projection(tree), case["projection"], "{id}");
    assert_eq!(names(tree), case["declaration_names"], "{id}");
    assert_eq!(diagnostics(tree), case["diagnostics"], "{id}");
    for expected in case["receiver_tokens"].as_array().unwrap() {
        let start = expected[0].as_u64().unwrap() as usize;
        let end = expected[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && (e.start, e.end) == (start, end)
                && e.kind == expected[2].as_str().unwrap()
                && &tree.source()[start..end] == expected[3].as_str().unwrap()
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == expected[4].as_str().unwrap())),
            "{id}"
        );
    }
    for expected in case["raw_receiver_tokens"].as_array().unwrap() {
        let start = expected[0].as_u64().unwrap() as usize;
        let end = expected[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && (e.start, e.end) == (start, end)
                && e.kind == expected[2].as_str().unwrap()
                && &tree.source()[start..end] == expected[3].as_str().unwrap()),
            "{id}"
        );
    }
    if let Some(range) = case["type_parameter_name"].as_array() {
        let start = range[0].as_u64().unwrap() as usize;
        let end = range[1].as_u64().unwrap() as usize;
        assert!(
            tree.elements().iter().any(|e| e.token
                && e.kind == "IDENTIFIER"
                && (e.start, e.end) == (start, end)
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == "TYPE_PARAMETER")),
            "{id}"
        );
    }
    for (offset, expected) in case["positions"].as_object().unwrap() {
        let offset = offset.parse::<usize>().unwrap();
        if expected.as_str() == Some("INVALID_CRLF_MIDPOINT") {
            assert!(tree.position(offset).is_err(), "{id}: {offset}");
        } else {
            let actual = tree.position(offset).unwrap();
            assert_eq!(
                json!([actual.line, actual.character]),
                *expected,
                "{id}: {offset}"
            );
        }
    }
    for offset in case["invalid_utf8_offsets"].as_array().unwrap() {
        assert!(
            tree.position(offset.as_u64().unwrap() as usize).is_err(),
            "{id}"
        );
    }
}

fn compatibility(tree: &Parsed, case: &Value) {
    let id = case["id"].as_str().unwrap();
    let elements = tree.elements();
    let source = tree.source();
    if id == "nullable-decline-qualified" {
        // Only this frozen exclusion is superseded by the exact two-component nullable seed.
        let expected: Value = serde_json::from_str(r####"{"id":"two-nullable-class-lf-top","source":"class Box<T\nfun pkg.String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,41,"KT_FILE",null],["TYPE_REFERENCE",16,27,"FUN",[12,41]],["NULLABLE_TYPE",16,27,"TYPE_REFERENCE",[12,41]],["USER_TYPE",16,26,"NULLABLE_TYPE",[12,41]],["USER_TYPE",16,19,"USER_TYPE",[12,41]],["REFERENCE_EXPRESSION",16,19,"USER_TYPE",[12,41]],["REFERENCE_EXPRESSION",20,26,"USER_TYPE",[12,41]],["VALUE_PARAMETER_LIST",35,37,"FUN",[12,41]]],"declaration_names":[[0,6,9,"Box"],[12,28,35,"healthy"]],"diagnostics":[[11,11,"Missing '>'"]],"type_parameter_name":[10,11],"receiver_tokens":[[16,19,"IDENTIFIER","pkg","REFERENCE_EXPRESSION"],[19,20,"DOT",".","USER_TYPE"],[20,26,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[26,27,"QUEST","?","NULLABLE_TYPE"],[27,28,"DOT",".","FUN"]],"raw_header_tokens":[[12,15,"FUN_KEYWORD","fun",1],[15,16,"WHITE_SPACE"," ",2],[16,19,"IDENTIFIER","pkg",3],[19,20,"DOT",".",4],[20,26,"IDENTIFIER","String",5],[26,27,"QUEST","?",6],[27,28,"DOT",".",7],[28,35,"IDENTIFIER","healthy",8],[35,36,"LPAR","(",9],[36,37,"RPAR",")",10]],"scratch_receiver_tokens":["IDENTIFIER","DOT","IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"15":[1,3],"16":[1,4],"19":[1,7],"20":[1,8],"26":[1,14],"27":[1,15],"28":[1,16],"35":[1,23],"36":[1,24],"37":[1,25],"41":[1,29]},"invalid_utf8_offsets":[],"probe_accounting":{"lpar_raw_ordinal":9,"lpar_end_from_probe_start":24},"body_owner":null,"bytes":41,"source_sha256":"3f15755be73b9e8c660004bc877ccf0c0140be22e5e96cb99d5838b00c4d8bcf","expectation_basis":"Source/marker derivation only; no Kotlin lexer/parser/compiler/runtime output","raw_receiver_tokens":[[12,15,"FUN_KEYWORD","fun",1],[15,16,"WHITE_SPACE"," ",2],[16,19,"IDENTIFIER","pkg",3],[19,20,"DOT",".",4],[20,26,"IDENTIFIER","String",5],[26,27,"QUEST","?",6],[27,28,"DOT",".",7],[28,35,"IDENTIFIER","healthy",8],[35,36,"LPAR","(",9],[36,37,"RPAR",")",10]]}"####).unwrap();
        assert_eq!(source, expected["source"].as_str().unwrap(), "{id}");
        positive(tree, &expected);
        return;
    }

    assert!(!tree.diagnostics().is_empty(), "{id}");
    if matches!(
        id,
        "nullable-decline-safe-call-expression" | "nullable-decline-safe-call-elvis-expression"
    ) {
        // Ordinary-property recovery preserves the original expression lookalike ownership guard.
        let supplement: Value = serde_json::from_str(PROPERTY_OWNERSHIP).unwrap();
        let expected = supplement["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"].as_str() == Some(id))
            .unwrap();
        assert_eq!(source, expected["source"].as_str().unwrap(), "{id}");
        assert_eq!(projection(tree), expected["projection"], "{id}");
        assert_eq!(names(tree), expected["declaration_names"], "{id}");
        assert_eq!(diagnostics(tree), expected["diagnostics"], "{id}");
        assert!(
            !elements.iter().any(|e| !e.token && e.kind == "FUN"),
            "{id}"
        );
        for node in expected["expression_nodes"].as_array().unwrap() {
            let matches: Vec<_> = elements
                .iter()
                .filter(|e| {
                    !e.token
                        && e.kind == node[0].as_str().unwrap()
                        && e.start == node[1].as_u64().unwrap() as usize
                        && e.end == node[2].as_u64().unwrap() as usize
                })
                .collect();
            assert_eq!(matches.len(), 1, "{id}: {node}");
            let parent = &elements[matches[0].parent.unwrap()];
            assert_eq!(
                json!([parent.kind, parent.start, parent.end]),
                json!([node[3], node[4][0], node[4][1]]),
                "{id}: {node}"
            );
        }
        for (offset, position) in expected["positions"].as_object().unwrap() {
            let actual = tree.position(offset.parse::<usize>().unwrap()).unwrap();
            assert_eq!(
                json!([actual.line, actual.character]),
                *position,
                "{id}: {offset}"
            );
        }
        return;
    }
    match case["invariant"].as_str().unwrap() {
        "no-complete-top-level-fun" => {
            let start = case["candidate_start"].as_u64().unwrap() as usize;
            assert!(
                !elements.iter().any(|e| !e.token
                    && e.kind == "FUN"
                    && (e.start, e.end) == (start, source.len())
                    && e.parent.is_some_and(|p| elements[p].kind == "KT_FILE")),
                "{id}"
            );
        }
        "class-end-greater-than-11" => {
            let class = elements
                .iter()
                .find(|e| !e.token && e.kind == "CLASS")
                .unwrap();
            assert_eq!(class.start, 0, "{id}");
            assert!(class.end > 11, "{id}");
        }
        "aggregate-reset" => {
            let class = elements
                .iter()
                .find(|e| !e.token && e.kind == "CLASS")
                .unwrap();
            assert!(
                class.end > case["declined_start"].as_u64().unwrap() as usize,
                "{id}"
            );
            let start = case["healthy_start"].as_u64().unwrap() as usize;
            assert!(
                elements.iter().any(|e| !e.token
                    && e.kind == "FUN"
                    && (e.start, e.end) == (start, source.len())
                    && e.parent.is_some_and(|p| elements[p].kind == "KT_FILE")),
                "{id}"
            );
        }
        "header-only-recovery" => {
            for (kind, start, end) in [
                ("CLASS", 0, 11),
                ("TYPE_PARAMETER_LIST", 9, 11),
                ("TYPE_PARAMETER", 10, 11),
                ("FUN", 12, source.len()),
            ] {
                assert!(
                    elements
                        .iter()
                        .any(|e| !e.token && e.kind == kind && (e.start, e.end) == (start, end)),
                    "{id}"
                );
            }
            assert!(
                elements.iter().any(|e| !e.token
                    && e.kind == "FUN"
                    && (e.start, e.end) == (12, source.len())
                    && e.parent.is_some_and(|p| elements[p].kind == "KT_FILE")),
                "{id}"
            );
        }
        "object-no-classifier-recovery" => {
            // The object callsite passes false; this control promises only the common partition/identity checks.
            assert_eq!(id, "nullable-nonclass-object");
        }
        other => panic!("unknown frozen invariant: {other}"),
    }
}

#[test]
fn frozen_nullable_receiver_contract_through_production_snapshot_worker() {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(cases.len(), 85);
    assert_eq!(
        cases
            .iter()
            .filter(|c| c.get("projection").is_some())
            .count(),
        32
    );
    assert_eq!(
        cases
            .iter()
            .filter_map(|c| c["positions"].as_object())
            .map(|p| p.len())
            .sum::<usize>(),
        491
    );
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        assert_eq!(
            source.len(),
            case["bytes"].as_u64().unwrap() as usize,
            "{id}"
        );
        let snapshot = SourceSnapshot::new(id, 1, FileKind::Source, source).unwrap();
        let accepted = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert_eq!(accepted.snapshot(), &snapshot);
        let tree = accepted.tree_for(&snapshot).unwrap();
        assert_eq!(tree.source(), source);
        let mut cursor = 0;
        let mut reconstructed = String::new();
        for e in tree.elements().iter().filter(|e| e.token) {
            assert_eq!(e.start, cursor, "{id}");
            assert!(
                source.is_char_boundary(e.start) && source.is_char_boundary(e.end),
                "{id}"
            );
            reconstructed.push_str(&source[e.start..e.end]);
            cursor = e.end;
        }
        assert_eq!(cursor, source.len(), "{id}");
        assert_eq!(reconstructed, source, "{id}");
        if case.get("projection").is_some() {
            positive(tree, case);
        } else {
            compatibility(tree, case);
        }
        let original_elements = serde_json::to_value(tree.elements()).unwrap();
        let original_diagnostics = serde_json::to_value(tree.diagnostics()).unwrap();
        for mismatch in [
            SourceSnapshot::new(format!("{id}-other"), 1, FileKind::Source, source).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Source, format!("{source} ")).unwrap(),
            SourceSnapshot::new(id, 1, FileKind::Script, source).unwrap(),
        ] {
            assert_eq!(
                accepted.tree_for(&mismatch).unwrap_err(),
                ParserError::StaleSnapshot,
                "{id}"
            );
        }
        let healthy = SourceSnapshot::new(id, 2, FileKind::Source, "fun healthy() = 2").unwrap();
        assert_eq!(
            accepted.tree_for(&healthy).unwrap_err(),
            ParserError::StaleSnapshot
        );
        let followup = parser
            .parse_snapshot(&healthy, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(followup.snapshot(), &healthy);
        let next = followup.tree_for(&healthy).unwrap();
        assert_eq!(next.source(), healthy.source());
        assert_eq!(
            projection(next),
            json!([
                ["FUN", 0, 17, "KT_FILE", null],
                ["VALUE_PARAMETER_LIST", 11, 13, "FUN", [0, 17]]
            ])
        );
        assert_eq!(names(next), json!([[0, 4, 11, "healthy"]]));
        assert!(next.diagnostics().is_empty());
        assert_eq!(accepted.snapshot(), &snapshot);
        assert_eq!(
            serde_json::to_value(accepted.tree_for(&snapshot).unwrap().elements()).unwrap(),
            original_elements,
            "{id}"
        );
        assert_eq!(
            serde_json::to_value(accepted.tree_for(&snapshot).unwrap().diagnostics()).unwrap(),
            original_diagnostics,
            "{id}"
        );
        assert_eq!(snapshot.source(), source);
        assert_eq!(accepted.tree_for(&snapshot).unwrap().source(), source);
    }
}

// Selected-field literal copy from reviewed contractv3 fixtures SHA25673b12ce096beac2b65ff08f4053d95fbd031e5fc4f66932352ba039a88059e58.
const FIXTURES: &str = r####"[
{"id":"nullable-class-lf-top","source":"class Box<T\nfun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,37,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,37]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,37]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,37]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,37]],["VALUE_PARAMETER_LIST",31,33,"FUN",[12,37]]],"declaration_names":[[0,6,9,"Box"],[12,24,31,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":37,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"80c153bf832b75fd97196ec8bcf656d1a20333914f2d4eb4ab253878c2db14a2","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"31":[1,19],"33":[1,21],"37":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-lf-class","source":"class Host {\nclass Box<T\nfun String?.healthy() = 2\n}","projection":[["CLASS",0,52,"KT_FILE",null],["CLASS",13,24,"CLASS_BODY",[0,52]],["TYPE_PARAMETER_LIST",22,24,"CLASS",[13,24]],["TYPE_PARAMETER",23,24,"TYPE_PARAMETER_LIST",[13,24]],["FUN",25,50,"CLASS_BODY",[0,52]],["TYPE_REFERENCE",29,36,"FUN",[25,50]],["NULLABLE_TYPE",29,36,"TYPE_REFERENCE",[25,50]],["USER_TYPE",29,35,"NULLABLE_TYPE",[25,50]],["REFERENCE_EXPRESSION",29,35,"USER_TYPE",[25,50]],["VALUE_PARAMETER_LIST",44,46,"FUN",[25,50]]],"declaration_names":[[0,6,10,"Host"],[13,19,22,"Box"],[25,37,44,"healthy"]],"type_parameter_name":[23,24],"parameter_name":null,"diagnostics":[[24,24,"Missing '>'"]],"bytes":52,"receiver_tokens":[[29,35,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[35,36,"QUEST","?","NULLABLE_TYPE"],[36,37,"DOT",".","FUN"]],"raw_receiver_tokens":[[29,35,"IDENTIFIER","String"],[35,36,"QUEST","?"],[36,37,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"71532627ea4632c52dd97225c310f7963e74df2454a5041321f2e4e6a8ee1e00","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"19":[1,6],"22":[1,9],"23":[1,10],"24":[1,11],"25":[2,0],"29":[2,4],"35":[2,10],"36":[2,11],"37":[2,12],"44":[2,19],"46":[2,21],"50":[2,25],"52":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-lf-local","source":"fun outer() {\nclass Box<T\nfun String?.healthy() = 2\n}","projection":[["FUN",0,53,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,53]],["CLASS",14,25,"BLOCK",[0,53]],["TYPE_PARAMETER_LIST",23,25,"CLASS",[14,25]],["TYPE_PARAMETER",24,25,"TYPE_PARAMETER_LIST",[14,25]],["FUN",26,51,"BLOCK",[0,53]],["TYPE_REFERENCE",30,37,"FUN",[26,51]],["NULLABLE_TYPE",30,37,"TYPE_REFERENCE",[26,51]],["USER_TYPE",30,36,"NULLABLE_TYPE",[26,51]],["REFERENCE_EXPRESSION",30,36,"USER_TYPE",[26,51]],["VALUE_PARAMETER_LIST",45,47,"FUN",[26,51]]],"declaration_names":[[0,4,9,"outer"],[14,20,23,"Box"],[26,38,45,"healthy"]],"type_parameter_name":[24,25],"parameter_name":null,"diagnostics":[[25,25,"Missing '>'"]],"bytes":53,"receiver_tokens":[[30,36,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[36,37,"QUEST","?","NULLABLE_TYPE"],[37,38,"DOT",".","FUN"]],"raw_receiver_tokens":[[30,36,"IDENTIFIER","String"],[36,37,"QUEST","?"],[37,38,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"1175a7f547c257fed5a56b8719b7e8c1c72067e6c32edbf9c26e8b5cf32e996e","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"20":[1,6],"23":[1,9],"24":[1,10],"25":[1,11],"26":[2,0],"30":[2,4],"36":[2,10],"37":[2,11],"38":[2,12],"45":[2,19],"47":[2,21],"51":[2,25],"53":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-lf-top","source":"interface Box<T\nfun String?.healthy() = 2","projection":[["CLASS",0,15,"KT_FILE",null],["TYPE_PARAMETER_LIST",13,15,"CLASS",[0,15]],["TYPE_PARAMETER",14,15,"TYPE_PARAMETER_LIST",[0,15]],["FUN",16,41,"KT_FILE",null],["TYPE_REFERENCE",20,27,"FUN",[16,41]],["NULLABLE_TYPE",20,27,"TYPE_REFERENCE",[16,41]],["USER_TYPE",20,26,"NULLABLE_TYPE",[16,41]],["REFERENCE_EXPRESSION",20,26,"USER_TYPE",[16,41]],["VALUE_PARAMETER_LIST",35,37,"FUN",[16,41]]],"declaration_names":[[0,10,13,"Box"],[16,28,35,"healthy"]],"type_parameter_name":[14,15],"parameter_name":null,"diagnostics":[[15,15,"Missing '>'"]],"bytes":41,"receiver_tokens":[[20,26,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[26,27,"QUEST","?","NULLABLE_TYPE"],[27,28,"DOT",".","FUN"]],"raw_receiver_tokens":[[20,26,"IDENTIFIER","String"],[26,27,"QUEST","?"],[27,28,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"51b902b9bc0494eb2e6249d5551837a50dd68a4b824cbe1abc994b23dee1acbd","positions":{"0":[0,0],"10":[0,10],"13":[0,13],"14":[0,14],"15":[0,15],"16":[1,0],"20":[1,4],"26":[1,10],"27":[1,11],"28":[1,12],"35":[1,19],"37":[1,21],"41":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-lf-class","source":"class Host {\ninterface Box<T\nfun String?.healthy() = 2\n}","projection":[["CLASS",0,56,"KT_FILE",null],["CLASS",13,28,"CLASS_BODY",[0,56]],["TYPE_PARAMETER_LIST",26,28,"CLASS",[13,28]],["TYPE_PARAMETER",27,28,"TYPE_PARAMETER_LIST",[13,28]],["FUN",29,54,"CLASS_BODY",[0,56]],["TYPE_REFERENCE",33,40,"FUN",[29,54]],["NULLABLE_TYPE",33,40,"TYPE_REFERENCE",[29,54]],["USER_TYPE",33,39,"NULLABLE_TYPE",[29,54]],["REFERENCE_EXPRESSION",33,39,"USER_TYPE",[29,54]],["VALUE_PARAMETER_LIST",48,50,"FUN",[29,54]]],"declaration_names":[[0,6,10,"Host"],[13,23,26,"Box"],[29,41,48,"healthy"]],"type_parameter_name":[27,28],"parameter_name":null,"diagnostics":[[28,28,"Missing '>'"]],"bytes":56,"receiver_tokens":[[33,39,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[39,40,"QUEST","?","NULLABLE_TYPE"],[40,41,"DOT",".","FUN"]],"raw_receiver_tokens":[[33,39,"IDENTIFIER","String"],[39,40,"QUEST","?"],[40,41,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"f75bb709970fe2f3d826c23f305740af17e0d490d09151a3c8f224767edc4110","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"23":[1,10],"26":[1,13],"27":[1,14],"28":[1,15],"29":[2,0],"33":[2,4],"39":[2,10],"40":[2,11],"41":[2,12],"48":[2,19],"50":[2,21],"54":[2,25],"56":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-lf-local","source":"fun outer() {\ninterface Box<T\nfun String?.healthy() = 2\n}","projection":[["FUN",0,57,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,57]],["CLASS",14,29,"BLOCK",[0,57]],["TYPE_PARAMETER_LIST",27,29,"CLASS",[14,29]],["TYPE_PARAMETER",28,29,"TYPE_PARAMETER_LIST",[14,29]],["FUN",30,55,"BLOCK",[0,57]],["TYPE_REFERENCE",34,41,"FUN",[30,55]],["NULLABLE_TYPE",34,41,"TYPE_REFERENCE",[30,55]],["USER_TYPE",34,40,"NULLABLE_TYPE",[30,55]],["REFERENCE_EXPRESSION",34,40,"USER_TYPE",[30,55]],["VALUE_PARAMETER_LIST",49,51,"FUN",[30,55]]],"declaration_names":[[0,4,9,"outer"],[14,24,27,"Box"],[30,42,49,"healthy"]],"type_parameter_name":[28,29],"parameter_name":null,"diagnostics":[[29,29,"Missing '>'"]],"bytes":57,"receiver_tokens":[[34,40,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[40,41,"QUEST","?","NULLABLE_TYPE"],[41,42,"DOT",".","FUN"]],"raw_receiver_tokens":[[34,40,"IDENTIFIER","String"],[40,41,"QUEST","?"],[41,42,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"480388e945099720796fb67988ec489b95c1e02e46e9f36d719fdfea47af90aa","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"24":[1,10],"27":[1,13],"28":[1,14],"29":[1,15],"30":[2,0],"34":[2,4],"40":[2,10],"41":[2,11],"42":[2,12],"49":[2,19],"51":[2,21],"55":[2,25],"57":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-crlf-top","source":"class Box<T\r\nfun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",13,38,"KT_FILE",null],["TYPE_REFERENCE",17,24,"FUN",[13,38]],["NULLABLE_TYPE",17,24,"TYPE_REFERENCE",[13,38]],["USER_TYPE",17,23,"NULLABLE_TYPE",[13,38]],["REFERENCE_EXPRESSION",17,23,"USER_TYPE",[13,38]],["VALUE_PARAMETER_LIST",32,34,"FUN",[13,38]]],"declaration_names":[[0,6,9,"Box"],[13,25,32,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":38,"receiver_tokens":[[17,23,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[23,24,"QUEST","?","NULLABLE_TYPE"],[24,25,"DOT",".","FUN"]],"raw_receiver_tokens":[[17,23,"IDENTIFIER","String"],[23,24,"QUEST","?"],[24,25,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"739fa0628d4bda54ae6c287c90cf20e62a275a480e5881b71327e7aa43e1339e","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":"INVALID_CRLF_MIDPOINT","13":[1,0],"17":[1,4],"23":[1,10],"24":[1,11],"25":[1,12],"32":[1,19],"34":[1,21],"38":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-crlf-class","source":"class Host {\nclass Box<T\r\nfun String?.healthy() = 2\n}","projection":[["CLASS",0,53,"KT_FILE",null],["CLASS",13,24,"CLASS_BODY",[0,53]],["TYPE_PARAMETER_LIST",22,24,"CLASS",[13,24]],["TYPE_PARAMETER",23,24,"TYPE_PARAMETER_LIST",[13,24]],["FUN",26,51,"CLASS_BODY",[0,53]],["TYPE_REFERENCE",30,37,"FUN",[26,51]],["NULLABLE_TYPE",30,37,"TYPE_REFERENCE",[26,51]],["USER_TYPE",30,36,"NULLABLE_TYPE",[26,51]],["REFERENCE_EXPRESSION",30,36,"USER_TYPE",[26,51]],["VALUE_PARAMETER_LIST",45,47,"FUN",[26,51]]],"declaration_names":[[0,6,10,"Host"],[13,19,22,"Box"],[26,38,45,"healthy"]],"type_parameter_name":[23,24],"parameter_name":null,"diagnostics":[[24,24,"Missing '>'"]],"bytes":53,"receiver_tokens":[[30,36,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[36,37,"QUEST","?","NULLABLE_TYPE"],[37,38,"DOT",".","FUN"]],"raw_receiver_tokens":[[30,36,"IDENTIFIER","String"],[36,37,"QUEST","?"],[37,38,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"2edb3412b99959a633ddbbe8f3142271172e2b341d4fea8a4e1473ae60da9bde","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"19":[1,6],"22":[1,9],"23":[1,10],"24":[1,11],"25":"INVALID_CRLF_MIDPOINT","26":[2,0],"30":[2,4],"36":[2,10],"37":[2,11],"38":[2,12],"45":[2,19],"47":[2,21],"51":[2,25],"53":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-crlf-local","source":"fun outer() {\nclass Box<T\r\nfun String?.healthy() = 2\n}","projection":[["FUN",0,54,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,54]],["CLASS",14,25,"BLOCK",[0,54]],["TYPE_PARAMETER_LIST",23,25,"CLASS",[14,25]],["TYPE_PARAMETER",24,25,"TYPE_PARAMETER_LIST",[14,25]],["FUN",27,52,"BLOCK",[0,54]],["TYPE_REFERENCE",31,38,"FUN",[27,52]],["NULLABLE_TYPE",31,38,"TYPE_REFERENCE",[27,52]],["USER_TYPE",31,37,"NULLABLE_TYPE",[27,52]],["REFERENCE_EXPRESSION",31,37,"USER_TYPE",[27,52]],["VALUE_PARAMETER_LIST",46,48,"FUN",[27,52]]],"declaration_names":[[0,4,9,"outer"],[14,20,23,"Box"],[27,39,46,"healthy"]],"type_parameter_name":[24,25],"parameter_name":null,"diagnostics":[[25,25,"Missing '>'"]],"bytes":54,"receiver_tokens":[[31,37,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[37,38,"QUEST","?","NULLABLE_TYPE"],[38,39,"DOT",".","FUN"]],"raw_receiver_tokens":[[31,37,"IDENTIFIER","String"],[37,38,"QUEST","?"],[38,39,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"de498f70a9809e533cfd86e15846f83274f01ac82fa1b1f379b4dc3d0374f3a5","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"20":[1,6],"23":[1,9],"24":[1,10],"25":[1,11],"26":"INVALID_CRLF_MIDPOINT","27":[2,0],"31":[2,4],"37":[2,10],"38":[2,11],"39":[2,12],"46":[2,19],"48":[2,21],"52":[2,25],"54":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-crlf-top","source":"interface Box<T\r\nfun String?.healthy() = 2","projection":[["CLASS",0,15,"KT_FILE",null],["TYPE_PARAMETER_LIST",13,15,"CLASS",[0,15]],["TYPE_PARAMETER",14,15,"TYPE_PARAMETER_LIST",[0,15]],["FUN",17,42,"KT_FILE",null],["TYPE_REFERENCE",21,28,"FUN",[17,42]],["NULLABLE_TYPE",21,28,"TYPE_REFERENCE",[17,42]],["USER_TYPE",21,27,"NULLABLE_TYPE",[17,42]],["REFERENCE_EXPRESSION",21,27,"USER_TYPE",[17,42]],["VALUE_PARAMETER_LIST",36,38,"FUN",[17,42]]],"declaration_names":[[0,10,13,"Box"],[17,29,36,"healthy"]],"type_parameter_name":[14,15],"parameter_name":null,"diagnostics":[[15,15,"Missing '>'"]],"bytes":42,"receiver_tokens":[[21,27,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[27,28,"QUEST","?","NULLABLE_TYPE"],[28,29,"DOT",".","FUN"]],"raw_receiver_tokens":[[21,27,"IDENTIFIER","String"],[27,28,"QUEST","?"],[28,29,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"c9abfba017ed6e6bca100e524833b8bc791f68caac6154caa6c042da6cfb2aa0","positions":{"0":[0,0],"10":[0,10],"13":[0,13],"14":[0,14],"15":[0,15],"16":"INVALID_CRLF_MIDPOINT","17":[1,0],"21":[1,4],"27":[1,10],"28":[1,11],"29":[1,12],"36":[1,19],"38":[1,21],"42":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-crlf-class","source":"class Host {\ninterface Box<T\r\nfun String?.healthy() = 2\n}","projection":[["CLASS",0,57,"KT_FILE",null],["CLASS",13,28,"CLASS_BODY",[0,57]],["TYPE_PARAMETER_LIST",26,28,"CLASS",[13,28]],["TYPE_PARAMETER",27,28,"TYPE_PARAMETER_LIST",[13,28]],["FUN",30,55,"CLASS_BODY",[0,57]],["TYPE_REFERENCE",34,41,"FUN",[30,55]],["NULLABLE_TYPE",34,41,"TYPE_REFERENCE",[30,55]],["USER_TYPE",34,40,"NULLABLE_TYPE",[30,55]],["REFERENCE_EXPRESSION",34,40,"USER_TYPE",[30,55]],["VALUE_PARAMETER_LIST",49,51,"FUN",[30,55]]],"declaration_names":[[0,6,10,"Host"],[13,23,26,"Box"],[30,42,49,"healthy"]],"type_parameter_name":[27,28],"parameter_name":null,"diagnostics":[[28,28,"Missing '>'"]],"bytes":57,"receiver_tokens":[[34,40,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[40,41,"QUEST","?","NULLABLE_TYPE"],[41,42,"DOT",".","FUN"]],"raw_receiver_tokens":[[34,40,"IDENTIFIER","String"],[40,41,"QUEST","?"],[41,42,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"355237910f0b4d00d131eae4ee560268896f2863f3cb74c49b0e5b71b91b6750","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"23":[1,10],"26":[1,13],"27":[1,14],"28":[1,15],"29":"INVALID_CRLF_MIDPOINT","30":[2,0],"34":[2,4],"40":[2,10],"41":[2,11],"42":[2,12],"49":[2,19],"51":[2,21],"55":[2,25],"57":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-crlf-local","source":"fun outer() {\ninterface Box<T\r\nfun String?.healthy() = 2\n}","projection":[["FUN",0,58,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,58]],["CLASS",14,29,"BLOCK",[0,58]],["TYPE_PARAMETER_LIST",27,29,"CLASS",[14,29]],["TYPE_PARAMETER",28,29,"TYPE_PARAMETER_LIST",[14,29]],["FUN",31,56,"BLOCK",[0,58]],["TYPE_REFERENCE",35,42,"FUN",[31,56]],["NULLABLE_TYPE",35,42,"TYPE_REFERENCE",[31,56]],["USER_TYPE",35,41,"NULLABLE_TYPE",[31,56]],["REFERENCE_EXPRESSION",35,41,"USER_TYPE",[31,56]],["VALUE_PARAMETER_LIST",50,52,"FUN",[31,56]]],"declaration_names":[[0,4,9,"outer"],[14,24,27,"Box"],[31,43,50,"healthy"]],"type_parameter_name":[28,29],"parameter_name":null,"diagnostics":[[29,29,"Missing '>'"]],"bytes":58,"receiver_tokens":[[35,41,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[41,42,"QUEST","?","NULLABLE_TYPE"],[42,43,"DOT",".","FUN"]],"raw_receiver_tokens":[[35,41,"IDENTIFIER","String"],[41,42,"QUEST","?"],[42,43,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"f13714dc202b1e5242ea4371118a5d3590afeba942414ec43781611eb5cbc1b2","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"24":[1,10],"27":[1,13],"28":[1,14],"29":[1,15],"30":"INVALID_CRLF_MIDPOINT","31":[2,0],"35":[2,4],"41":[2,10],"42":[2,11],"43":[2,12],"50":[2,19],"52":[2,21],"56":[2,25],"58":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-cr-top","source":"class Box<T\rfun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,37,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,37]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,37]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,37]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,37]],["VALUE_PARAMETER_LIST",31,33,"FUN",[12,37]]],"declaration_names":[[0,6,9,"Box"],[12,24,31,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":37,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"1ecbfe242f178099da283765ac497165f5bd7f181139ec094931f295ca002a38","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"31":[1,19],"33":[1,21],"37":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-cr-class","source":"class Host {\nclass Box<T\rfun String?.healthy() = 2\n}","projection":[["CLASS",0,52,"KT_FILE",null],["CLASS",13,24,"CLASS_BODY",[0,52]],["TYPE_PARAMETER_LIST",22,24,"CLASS",[13,24]],["TYPE_PARAMETER",23,24,"TYPE_PARAMETER_LIST",[13,24]],["FUN",25,50,"CLASS_BODY",[0,52]],["TYPE_REFERENCE",29,36,"FUN",[25,50]],["NULLABLE_TYPE",29,36,"TYPE_REFERENCE",[25,50]],["USER_TYPE",29,35,"NULLABLE_TYPE",[25,50]],["REFERENCE_EXPRESSION",29,35,"USER_TYPE",[25,50]],["VALUE_PARAMETER_LIST",44,46,"FUN",[25,50]]],"declaration_names":[[0,6,10,"Host"],[13,19,22,"Box"],[25,37,44,"healthy"]],"type_parameter_name":[23,24],"parameter_name":null,"diagnostics":[[24,24,"Missing '>'"]],"bytes":52,"receiver_tokens":[[29,35,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[35,36,"QUEST","?","NULLABLE_TYPE"],[36,37,"DOT",".","FUN"]],"raw_receiver_tokens":[[29,35,"IDENTIFIER","String"],[35,36,"QUEST","?"],[36,37,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"098d6aecbb7407e3b4256f373658461bcd26da856bae34b1302b0bebb71cd3cd","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"19":[1,6],"22":[1,9],"23":[1,10],"24":[1,11],"25":[2,0],"29":[2,4],"35":[2,10],"36":[2,11],"37":[2,12],"44":[2,19],"46":[2,21],"50":[2,25],"52":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-class-cr-local","source":"fun outer() {\nclass Box<T\rfun String?.healthy() = 2\n}","projection":[["FUN",0,53,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,53]],["CLASS",14,25,"BLOCK",[0,53]],["TYPE_PARAMETER_LIST",23,25,"CLASS",[14,25]],["TYPE_PARAMETER",24,25,"TYPE_PARAMETER_LIST",[14,25]],["FUN",26,51,"BLOCK",[0,53]],["TYPE_REFERENCE",30,37,"FUN",[26,51]],["NULLABLE_TYPE",30,37,"TYPE_REFERENCE",[26,51]],["USER_TYPE",30,36,"NULLABLE_TYPE",[26,51]],["REFERENCE_EXPRESSION",30,36,"USER_TYPE",[26,51]],["VALUE_PARAMETER_LIST",45,47,"FUN",[26,51]]],"declaration_names":[[0,4,9,"outer"],[14,20,23,"Box"],[26,38,45,"healthy"]],"type_parameter_name":[24,25],"parameter_name":null,"diagnostics":[[25,25,"Missing '>'"]],"bytes":53,"receiver_tokens":[[30,36,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[36,37,"QUEST","?","NULLABLE_TYPE"],[37,38,"DOT",".","FUN"]],"raw_receiver_tokens":[[30,36,"IDENTIFIER","String"],[36,37,"QUEST","?"],[37,38,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"2a9f637564bb79b8668ace41c7fb93d7b0fadcbfe047f202029d4ca2b3c50262","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"20":[1,6],"23":[1,9],"24":[1,10],"25":[1,11],"26":[2,0],"30":[2,4],"36":[2,10],"37":[2,11],"38":[2,12],"45":[2,19],"47":[2,21],"51":[2,25],"53":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-cr-top","source":"interface Box<T\rfun String?.healthy() = 2","projection":[["CLASS",0,15,"KT_FILE",null],["TYPE_PARAMETER_LIST",13,15,"CLASS",[0,15]],["TYPE_PARAMETER",14,15,"TYPE_PARAMETER_LIST",[0,15]],["FUN",16,41,"KT_FILE",null],["TYPE_REFERENCE",20,27,"FUN",[16,41]],["NULLABLE_TYPE",20,27,"TYPE_REFERENCE",[16,41]],["USER_TYPE",20,26,"NULLABLE_TYPE",[16,41]],["REFERENCE_EXPRESSION",20,26,"USER_TYPE",[16,41]],["VALUE_PARAMETER_LIST",35,37,"FUN",[16,41]]],"declaration_names":[[0,10,13,"Box"],[16,28,35,"healthy"]],"type_parameter_name":[14,15],"parameter_name":null,"diagnostics":[[15,15,"Missing '>'"]],"bytes":41,"receiver_tokens":[[20,26,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[26,27,"QUEST","?","NULLABLE_TYPE"],[27,28,"DOT",".","FUN"]],"raw_receiver_tokens":[[20,26,"IDENTIFIER","String"],[26,27,"QUEST","?"],[27,28,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"c13ac16854305c91a25d17e6a2802fbe4ff844981a1ab13d820a346a4b44fb2f","positions":{"0":[0,0],"10":[0,10],"13":[0,13],"14":[0,14],"15":[0,15],"16":[1,0],"20":[1,4],"26":[1,10],"27":[1,11],"28":[1,12],"35":[1,19],"37":[1,21],"41":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-cr-class","source":"class Host {\ninterface Box<T\rfun String?.healthy() = 2\n}","projection":[["CLASS",0,56,"KT_FILE",null],["CLASS",13,28,"CLASS_BODY",[0,56]],["TYPE_PARAMETER_LIST",26,28,"CLASS",[13,28]],["TYPE_PARAMETER",27,28,"TYPE_PARAMETER_LIST",[13,28]],["FUN",29,54,"CLASS_BODY",[0,56]],["TYPE_REFERENCE",33,40,"FUN",[29,54]],["NULLABLE_TYPE",33,40,"TYPE_REFERENCE",[29,54]],["USER_TYPE",33,39,"NULLABLE_TYPE",[29,54]],["REFERENCE_EXPRESSION",33,39,"USER_TYPE",[29,54]],["VALUE_PARAMETER_LIST",48,50,"FUN",[29,54]]],"declaration_names":[[0,6,10,"Host"],[13,23,26,"Box"],[29,41,48,"healthy"]],"type_parameter_name":[27,28],"parameter_name":null,"diagnostics":[[28,28,"Missing '>'"]],"bytes":56,"receiver_tokens":[[33,39,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[39,40,"QUEST","?","NULLABLE_TYPE"],[40,41,"DOT",".","FUN"]],"raw_receiver_tokens":[[33,39,"IDENTIFIER","String"],[39,40,"QUEST","?"],[40,41,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"530e9a787d4d2f7101ab94b9d8d4526a39538d00e1daa13ff3a2c24f06359bb0","positions":{"0":[0,0],"6":[0,6],"10":[0,10],"13":[1,0],"23":[1,10],"26":[1,13],"27":[1,14],"28":[1,15],"29":[2,0],"33":[2,4],"39":[2,10],"40":[2,11],"41":[2,12],"48":[2,19],"50":[2,21],"54":[2,25],"56":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-interface-cr-local","source":"fun outer() {\ninterface Box<T\rfun String?.healthy() = 2\n}","projection":[["FUN",0,57,"KT_FILE",null],["VALUE_PARAMETER_LIST",9,11,"FUN",[0,57]],["CLASS",14,29,"BLOCK",[0,57]],["TYPE_PARAMETER_LIST",27,29,"CLASS",[14,29]],["TYPE_PARAMETER",28,29,"TYPE_PARAMETER_LIST",[14,29]],["FUN",30,55,"BLOCK",[0,57]],["TYPE_REFERENCE",34,41,"FUN",[30,55]],["NULLABLE_TYPE",34,41,"TYPE_REFERENCE",[30,55]],["USER_TYPE",34,40,"NULLABLE_TYPE",[30,55]],["REFERENCE_EXPRESSION",34,40,"USER_TYPE",[30,55]],["VALUE_PARAMETER_LIST",49,51,"FUN",[30,55]]],"declaration_names":[[0,4,9,"outer"],[14,24,27,"Box"],[30,42,49,"healthy"]],"type_parameter_name":[28,29],"parameter_name":null,"diagnostics":[[29,29,"Missing '>'"]],"bytes":57,"receiver_tokens":[[34,40,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[40,41,"QUEST","?","NULLABLE_TYPE"],[41,42,"DOT",".","FUN"]],"raw_receiver_tokens":[[34,40,"IDENTIFIER","String"],[40,41,"QUEST","?"],[41,42,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"851e8915b8e2b7a7b88be4be67cd1d7ea896a725f8266d715f974ece80daf473","positions":{"0":[0,0],"4":[0,4],"9":[0,9],"11":[0,11],"14":[1,0],"24":[1,10],"27":[1,13],"28":[1,14],"29":[1,15],"30":[2,0],"34":[2,4],"40":[2,10],"41":[2,11],"42":[2,12],"49":[2,19],"51":[2,21],"55":[2,25],"57":[3,1]},"invalid_utf8_offsets":[]},
{"id":"nullable-empty","source":"class Box<\nfun String?.healthy() = 2","projection":[["CLASS",0,10,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,10,"CLASS",[0,10]],["FUN",11,36,"KT_FILE",null],["TYPE_REFERENCE",15,22,"FUN",[11,36]],["NULLABLE_TYPE",15,22,"TYPE_REFERENCE",[11,36]],["USER_TYPE",15,21,"NULLABLE_TYPE",[11,36]],["REFERENCE_EXPRESSION",15,21,"USER_TYPE",[11,36]],["VALUE_PARAMETER_LIST",30,32,"FUN",[11,36]]],"declaration_names":[[0,6,9,"Box"],[11,23,30,"healthy"]],"type_parameter_name":null,"parameter_name":null,"diagnostics":[[10,10,"Type parameter declaration expected"]],"bytes":36,"receiver_tokens":[[15,21,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[21,22,"QUEST","?","NULLABLE_TYPE"],[22,23,"DOT",".","FUN"]],"raw_receiver_tokens":[[15,21,"IDENTIFIER","String"],[21,22,"QUEST","?"],[22,23,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"fedbba155f3c8b1c69fa19ac9a2ca9ff14881a4621aad125402087455edc7f16","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[1,0],"15":[1,4],"21":[1,10],"22":[1,11],"23":[1,12],"30":[1,19],"32":[1,21],"36":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-comma","source":"class Box<T,\nfun String?.healthy() = 2","projection":[["CLASS",0,12,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,12,"CLASS",[0,12]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,12]],["FUN",13,38,"KT_FILE",null],["TYPE_REFERENCE",17,24,"FUN",[13,38]],["NULLABLE_TYPE",17,24,"TYPE_REFERENCE",[13,38]],["USER_TYPE",17,23,"NULLABLE_TYPE",[13,38]],["REFERENCE_EXPRESSION",17,23,"USER_TYPE",[13,38]],["VALUE_PARAMETER_LIST",32,34,"FUN",[13,38]]],"declaration_names":[[0,6,9,"Box"],[13,25,32,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[12,12,"Type parameter declaration expected"]],"bytes":38,"receiver_tokens":[[17,23,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[23,24,"QUEST","?","NULLABLE_TYPE"],[24,25,"DOT",".","FUN"]],"raw_receiver_tokens":[[17,23,"IDENTIFIER","String"],[23,24,"QUEST","?"],[24,25,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"ffeffd87fd3742c8a11226ad324b3507a79168a47ccc3a6d1f55aea0a94ef292","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[0,12],"13":[1,0],"17":[1,4],"23":[1,10],"24":[1,11],"25":[1,12],"32":[1,19],"34":[1,21],"38":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-annotated-prefix","source":"class Box<T\n@A public fun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,47,"KT_FILE",null],["ANNOTATION_ENTRY",12,14,"MODIFIER_LIST",[12,47]],["TYPE_REFERENCE",13,14,"CONSTRUCTOR_CALLEE",[12,47]],["USER_TYPE",13,14,"TYPE_REFERENCE",[12,47]],["REFERENCE_EXPRESSION",13,14,"USER_TYPE",[12,47]],["TYPE_REFERENCE",26,33,"FUN",[12,47]],["NULLABLE_TYPE",26,33,"TYPE_REFERENCE",[12,47]],["USER_TYPE",26,32,"NULLABLE_TYPE",[12,47]],["REFERENCE_EXPRESSION",26,32,"USER_TYPE",[12,47]],["VALUE_PARAMETER_LIST",41,43,"FUN",[12,47]]],"declaration_names":[[0,6,9,"Box"],[12,34,41,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":47,"receiver_tokens":[[26,32,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[32,33,"QUEST","?","NULLABLE_TYPE"],[33,34,"DOT",".","FUN"]],"raw_receiver_tokens":[[26,32,"IDENTIFIER","String"],[32,33,"QUEST","?"],[33,34,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"aec52e4b383ebd9eb004a2e14fe3bb3f734e840e0aaa71bfdfc1a2970325abb0","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"13":[1,1],"14":[1,2],"26":[1,14],"32":[1,20],"33":[1,21],"34":[1,22],"41":[1,29],"43":[1,31],"47":[1,35]},"invalid_utf8_offsets":[]},
{"id":"nullable-same-line","source":"class Box<T fun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,37,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,37]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,37]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,37]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,37]],["VALUE_PARAMETER_LIST",31,33,"FUN",[12,37]]],"declaration_names":[[0,6,9,"Box"],[12,24,31,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":37,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"5dbb9c6fb594bb1b6e2da504f60f6b7dc73a266d67ac3ae8a4880474bbc80d4d","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[0,12],"16":[0,16],"22":[0,22],"23":[0,23],"24":[0,24],"31":[0,31],"33":[0,33],"37":[0,37]},"invalid_utf8_offsets":[]},
{"id":"nullable-unicode-backtick-name","source":"class Box<T\nfun String?.`healthy 🦀é`() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,47,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,47]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,47]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,47]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,47]],["VALUE_PARAMETER_LIST",41,43,"FUN",[12,47]]],"declaration_names":[[0,6,9,"Box"],[12,24,41,"`healthy 🦀é`"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"invalid_utf8_offsets":[34,35,36,39],"bytes":47,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"014ae028c9957ccde2f057acc4a312ab637e380c93c01fa4b618a1fa5b7e7d50","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"41":[1,26],"43":[1,28],"47":[1,32]}},
{"id":"nullable-intact-class","source":"class Box<T>\nfun String?.healthy() = 2","projection":[["CLASS",0,12,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,12,"CLASS",[0,12]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,12]],["FUN",13,38,"KT_FILE",null],["TYPE_REFERENCE",17,24,"FUN",[13,38]],["NULLABLE_TYPE",17,24,"TYPE_REFERENCE",[13,38]],["USER_TYPE",17,23,"NULLABLE_TYPE",[13,38]],["REFERENCE_EXPRESSION",17,23,"USER_TYPE",[13,38]],["VALUE_PARAMETER_LIST",32,34,"FUN",[13,38]]],"declaration_names":[[0,6,9,"Box"],[13,25,32,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[],"bytes":38,"receiver_tokens":[[17,23,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[23,24,"QUEST","?","NULLABLE_TYPE"],[24,25,"DOT",".","FUN"]],"raw_receiver_tokens":[[17,23,"IDENTIFIER","String"],[23,24,"QUEST","?"],[24,25,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"767b53f104bc8f3b654716bf5c03d4fa070b9afe8635594ade01afce79c9c6b2","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[0,12],"13":[1,0],"17":[1,4],"23":[1,10],"24":[1,11],"25":[1,12],"32":[1,19],"34":[1,21],"38":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-intact-interface","source":"interface Box<T>\nfun String?.healthy() = 2","projection":[["CLASS",0,16,"KT_FILE",null],["TYPE_PARAMETER_LIST",13,16,"CLASS",[0,16]],["TYPE_PARAMETER",14,15,"TYPE_PARAMETER_LIST",[0,16]],["FUN",17,42,"KT_FILE",null],["TYPE_REFERENCE",21,28,"FUN",[17,42]],["NULLABLE_TYPE",21,28,"TYPE_REFERENCE",[17,42]],["USER_TYPE",21,27,"NULLABLE_TYPE",[17,42]],["REFERENCE_EXPRESSION",21,27,"USER_TYPE",[17,42]],["VALUE_PARAMETER_LIST",36,38,"FUN",[17,42]]],"declaration_names":[[0,10,13,"Box"],[17,29,36,"healthy"]],"type_parameter_name":[14,15],"parameter_name":null,"diagnostics":[],"bytes":42,"receiver_tokens":[[21,27,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[27,28,"QUEST","?","NULLABLE_TYPE"],[28,29,"DOT",".","FUN"]],"raw_receiver_tokens":[[21,27,"IDENTIFIER","String"],[27,28,"QUEST","?"],[28,29,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"a9f4ff7af4abb56be966a3f7deff5a4700c7d7300f03f80696c8cae3c9e3501b","positions":{"0":[0,0],"10":[0,10],"13":[0,13],"14":[0,14],"15":[0,15],"16":[0,16],"17":[1,0],"21":[1,4],"27":[1,10],"28":[1,11],"29":[1,12],"36":[1,19],"38":[1,21],"42":[1,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-leading-trivia","source":"class Box<\n\n/*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ \n\nfun String?.healthy() = 2","projection":[["CLASS",0,10,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,10,"CLASS",[0,10]],["FUN",13214,13239,"KT_FILE",null],["TYPE_REFERENCE",13218,13225,"FUN",[13214,13239]],["NULLABLE_TYPE",13218,13225,"TYPE_REFERENCE",[13214,13239]],["USER_TYPE",13218,13224,"NULLABLE_TYPE",[13214,13239]],["REFERENCE_EXPRESSION",13218,13224,"USER_TYPE",[13214,13239]],["VALUE_PARAMETER_LIST",13233,13235,"FUN",[13214,13239]]],"declaration_names":[[0,6,9,"Box"],[13214,13226,13233,"healthy"]],"type_parameter_name":null,"diagnostics":[[10,10,"Type parameter declaration expected"]],"bytes":13239,"receiver_tokens":[[13218,13224,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[13224,13225,"QUEST","?","NULLABLE_TYPE"],[13225,13226,"DOT",".","FUN"]],"raw_receiver_tokens":[[13218,13224,"IDENTIFIER","String"],[13224,13225,"QUEST","?"],[13225,13226,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"e669ceac33317d66c16be9477e1514ddbda24733f7506ad3680bbf3389d74b67","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"13214":[4,0],"13218":[4,4],"13224":[4,10],"13225":[4,11],"13226":[4,12],"13233":[4,19],"13235":[4,21],"13239":[4,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-spaced-question-dot","source":"class Box<T\nfun String ? .healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,39,"KT_FILE",null],["TYPE_REFERENCE",16,24,"FUN",[12,39]],["NULLABLE_TYPE",16,24,"TYPE_REFERENCE",[12,39]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,39]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,39]],["VALUE_PARAMETER_LIST",33,35,"FUN",[12,39]]],"declaration_names":[[0,6,9,"Box"],[12,26,33,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":39,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[23,24,"QUEST","?","NULLABLE_TYPE"],[25,26,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"WHITE_SPACE"," "],[23,24,"QUEST","?"],[24,25,"WHITE_SPACE"," "],[25,26,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","QUEST","DOT","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"ab94faa848512434737fe40f922904b00e4e0982ae6341e18d4b3c6851296509","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"25":[1,13],"26":[1,14],"33":[1,21],"35":[1,23],"39":[1,27]},"invalid_utf8_offsets":[]},
{"id":"nullable-comment-question-dot","source":"class Box<T\nfun String?/*x*/.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,42,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,42]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,42]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,42]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,42]],["VALUE_PARAMETER_LIST",36,38,"FUN",[12,42]]],"declaration_names":[[0,6,9,"Box"],[12,29,36,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":42,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[28,29,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,28,"BLOCK_COMMENT","/*x*/"],[28,29,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","QUEST","DOT","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"85f9c168da10a157847e2cfb1dba8e953f3ff7d1e82823d22298afacb9772a35","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"28":[1,16],"29":[1,17],"36":[1,24],"38":[1,26],"42":[1,30]},"invalid_utf8_offsets":[]},
{"id":"nullable-raw256","source":"class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ String?.healthy () = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,782,"KT_FILE",null],["TYPE_REFERENCE",760,767,"FUN",[12,782]],["NULLABLE_TYPE",760,767,"TYPE_REFERENCE",[12,782]],["USER_TYPE",760,766,"NULLABLE_TYPE",[12,782]],["REFERENCE_EXPRESSION",760,766,"USER_TYPE",[12,782]],["VALUE_PARAMETER_LIST",776,778,"FUN",[12,782]]],"declaration_names":[[0,6,9,"Box"],[12,768,775,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":782,"receiver_tokens":[[760,766,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[766,767,"QUEST","?","NULLABLE_TYPE"],[767,768,"DOT",".","FUN"]],"raw_receiver_tokens":[[760,766,"IDENTIFIER","String"],[766,767,"QUEST","?"],[767,768,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"9d8cad551505bdbd3b81a326ae81fac3530fb6183dc4d8ed28a9de3c5e84ae01","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"760":[1,748],"766":[1,754],"767":[1,755],"768":[1,756],"775":[1,763],"776":[1,764],"778":[1,766],"782":[1,770]},"invalid_utf8_offsets":[],"probe_accounting":{"lpar_raw_ordinal":256,"copied_raw_tokens":256,"recognizer":true}},
{"id":"nullable-byte4096","source":"class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,4113,"KT_FILE",null],["TYPE_REFERENCE",4092,4099,"FUN",[12,4113]],["NULLABLE_TYPE",4092,4099,"TYPE_REFERENCE",[12,4113]],["USER_TYPE",4092,4098,"NULLABLE_TYPE",[12,4113]],["REFERENCE_EXPRESSION",4092,4098,"USER_TYPE",[12,4113]],["VALUE_PARAMETER_LIST",4107,4109,"FUN",[12,4113]]],"declaration_names":[[0,6,9,"Box"],[12,4100,4107,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":4113,"receiver_tokens":[[4092,4098,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[4098,4099,"QUEST","?","NULLABLE_TYPE"],[4099,4100,"DOT",".","FUN"]],"raw_receiver_tokens":[[4092,4098,"IDENTIFIER","String"],[4098,4099,"QUEST","?"],[4099,4100,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"9098adbf5fe227694fea6053c6944b6bfb8e250ef6d50a9563da48c40f6b6687","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"4092":[1,4080],"4098":[1,4086],"4099":[1,4087],"4100":[1,4088],"4107":[1,4095],"4109":[1,4097],"4113":[1,4101]},"invalid_utf8_offsets":[],"probe_accounting":{"lpar_end_from_probe_start":4096,"copied_raw_tokens":9,"recognizer":true}},
{"id":"nullable-lazy-body","source":"class Box<T\nfun String?.healthy() { 2 }","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,39,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,39]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,39]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,39]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,39]],["VALUE_PARAMETER_LIST",31,33,"FUN",[12,39]]],"declaration_names":[[0,6,9,"Box"],[12,24,31,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":39,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"7f3db01c790f3c2a82010169ec843198df389f3e0bb63cd13eafe8c432e23046","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"31":[1,19],"33":[1,21],"39":[1,27]},"invalid_utf8_offsets":[]},
{"id":"nullable-duplicate-names","source":"class Box<T\nfun String?.healthy() = 2\nfun String?.healthy() = 2","projection":[["CLASS",0,11,"KT_FILE",null],["TYPE_PARAMETER_LIST",9,11,"CLASS",[0,11]],["TYPE_PARAMETER",10,11,"TYPE_PARAMETER_LIST",[0,11]],["FUN",12,37,"KT_FILE",null],["TYPE_REFERENCE",16,23,"FUN",[12,37]],["NULLABLE_TYPE",16,23,"TYPE_REFERENCE",[12,37]],["USER_TYPE",16,22,"NULLABLE_TYPE",[12,37]],["REFERENCE_EXPRESSION",16,22,"USER_TYPE",[12,37]],["VALUE_PARAMETER_LIST",31,33,"FUN",[12,37]],["FUN",38,63,"KT_FILE",null],["TYPE_REFERENCE",42,49,"FUN",[38,63]],["NULLABLE_TYPE",42,49,"TYPE_REFERENCE",[38,63]],["USER_TYPE",42,48,"NULLABLE_TYPE",[38,63]],["REFERENCE_EXPRESSION",42,48,"USER_TYPE",[38,63]],["VALUE_PARAMETER_LIST",57,59,"FUN",[38,63]]],"declaration_names":[[0,6,9,"Box"],[12,24,31,"healthy"],[38,50,57,"healthy"]],"type_parameter_name":[10,11],"parameter_name":null,"diagnostics":[[11,11,"Missing '>'"]],"bytes":63,"receiver_tokens":[[16,22,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[22,23,"QUEST","?","NULLABLE_TYPE"],[23,24,"DOT",".","FUN"],[42,48,"IDENTIFIER","String","REFERENCE_EXPRESSION"],[48,49,"QUEST","?","NULLABLE_TYPE"],[49,50,"DOT",".","FUN"]],"raw_receiver_tokens":[[16,22,"IDENTIFIER","String"],[22,23,"QUEST","?"],[23,24,"DOT","."],[42,48,"IDENTIFIER","String"],[48,49,"QUEST","?"],[49,50,"DOT","."]],"scratch_receiver_tokens":["IDENTIFIER","SAFE_ACCESS","IDENTIFIER","LPAR"],"expectation_basis":"source-authored selected C13b projection; independently traced functions.rs disabled joining/truncated receiver and types.rs nullable wrapping; not runtime output","source_sha256":"78016b3598b5b8cf3fe4abc920744b8c5d95b2fd364dcf024601f6d16255fa82","positions":{"0":[0,0],"6":[0,6],"9":[0,9],"10":[0,10],"11":[0,11],"12":[1,0],"16":[1,4],"22":[1,10],"23":[1,11],"24":[1,12],"31":[1,19],"33":[1,21],"37":[1,25],"38":[2,0],"42":[2,4],"48":[2,10],"49":[2,11],"50":[2,12],"57":[2,19],"59":[2,21],"63":[2,25]},"invalid_utf8_offsets":[]},
{"id":"nullable-incomplete-bound-0-lf","source":"class Box<T : List<Int\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":23,"bytes":48,"source_sha256":"37a3035485226f407d77a29d1bcfff407645c4b77f8361e1c7606c3e109cc287"},
{"id":"nullable-incomplete-bound-0-crlf","source":"class Box<T : List<Int\r\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":24,"bytes":49,"source_sha256":"4e4acda9121693ef25a6c488d25521812602d3eeff1983098f89e38da62de013"},
{"id":"nullable-incomplete-bound-0-cr","source":"class Box<T : List<Int\rfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":23,"bytes":48,"source_sha256":"68747c66a902c2d4822790ed338e0364fc36eec615d23a5e5e9d4b468994b038"},
{"id":"nullable-incomplete-bound-1-lf","source":"class Box<T : (Int\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"e65ff45647335c0b6399c9c8ceca2cdaf6b9535128972b1b4f229dd961dc21a8"},
{"id":"nullable-incomplete-bound-1-crlf","source":"class Box<T : (Int\r\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":20,"bytes":45,"source_sha256":"15d917754f70c269d057da041b9289b30a454105dcb0c237d221910a760b02bd"},
{"id":"nullable-incomplete-bound-1-cr","source":"class Box<T : (Int\rfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"283b41935a55e1d5ea0e141be73f1af3f66e57596a2c3e062cedf4c65af4fa7e"},
{"id":"nullable-incomplete-bound-2-lf","source":"class Box<T : Int.\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"e09e3f40620c48b37253b13899fa1c7cde964d715620fbd316f45253117f059a"},
{"id":"nullable-incomplete-bound-2-crlf","source":"class Box<T : Int.\r\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":20,"bytes":45,"source_sha256":"0f24c2c01cbdf12df59a02f5ec41e88de9348a1917e05ecbc59ec8dff5a7e92f"},
{"id":"nullable-incomplete-bound-2-cr","source":"class Box<T : Int.\rfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"2b409c65c315cf90735eb35b64657d2f6528a3074bad633e13595677f26c5410"},
{"id":"nullable-incomplete-bound-3-lf","source":"class Box<T : Int &\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":20,"bytes":45,"source_sha256":"da8d836c60a605deaac18c3f536f6a3cde2b3c8fd25dec2e0a952ec7008c5332"},
{"id":"nullable-incomplete-bound-3-crlf","source":"class Box<T : Int &\r\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":21,"bytes":46,"source_sha256":"13a060d4869ab2d2937fd1071926a2bbe2eafa6c9d251fd09057f5b631ca853b"},
{"id":"nullable-incomplete-bound-3-cr","source":"class Box<T : Int &\rfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":20,"bytes":45,"source_sha256":"3df4491e8b45e17bde49a94750c1f6aa6b5b4214dacb2eaa82ac484b19ff8f76"},
{"id":"nullable-incomplete-bound-4-lf","source":"class Box<T : @A(1\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"65ec076290e6b6adfa6fbb086e3338ec3dc9ea59e75dc030e4401aab9d0ae691"},
{"id":"nullable-incomplete-bound-4-crlf","source":"class Box<T : @A(1\r\nfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":20,"bytes":45,"source_sha256":"589248cb9d22d5f2f274d75024624dda404a369849fd22a1d3721248b7e8190d"},
{"id":"nullable-incomplete-bound-4-cr","source":"class Box<T : @A(1\rfun String?.healthy() = 2","reason":"incomplete/errorful bound cannot become eligible","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":19,"bytes":44,"source_sha256":"8a8e58fd0f973957138b41255e74e675476c85a47eccec29bf9c9d165b47a73e"},
{"id":"nullable-interface-bound-0","source":"interface Box<T : List<Int\nfun String?.healthy() = 2","reason":"same completion proof applies to interface","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":27,"bytes":52,"source_sha256":"3789fe0ec8a5d42c3b9b146e6370439fbb576bbbf59d6cb193fd576a03fde724"},
{"id":"nullable-interface-bound-1","source":"interface Box<T : (Int\nfun String?.healthy() = 2","reason":"same completion proof applies to interface","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":23,"bytes":48,"source_sha256":"b5654cb18ade2d475bf35ded0f6bddc023bec0ccc5443ddf8002b31014fb7b83"},
{"id":"nullable-interface-bound-2","source":"interface Box<T : Int.\nfun String?.healthy() = 2","reason":"same completion proof applies to interface","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":23,"bytes":48,"source_sha256":"4645b95d6f99e16ac98d441f691e73b76cb86cb65fd14013e422fdeff8dc5665"},
{"id":"nullable-interface-bound-3","source":"interface Box<T : Int &\nfun String?.healthy() = 2","reason":"same completion proof applies to interface","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":24,"bytes":49,"source_sha256":"41e36abfd62f0cc84b859417fcb7b068df91b0c4e018696310d3882b63305a8e"},
{"id":"nullable-interface-bound-4","source":"interface Box<T : @A(1\nfun String?.healthy() = 2","reason":"same completion proof applies to interface","invariant":"no-complete-top-level-fun","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","candidate_start":23,"bytes":48,"source_sha256":"9ea7f7c1567be99cdccf3eaf6b32aa96d725f1352f6421cae789e57def38f669"},
{"id":"nullable-decline-qualified","source":"class Box<T\nfun pkg.String?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":41,"source_sha256":"3f15755be73b9e8c660004bc877ccf0c0140be22e5e96cb99d5838b00c4d8bcf"},
{"id":"nullable-decline-long-qualified","source":"class Box<T\nfun a.pkg.String?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":43,"source_sha256":"0fae122575e67b223e6202ab9d25bd6c391c9d369e7bb645dadd8067349d1b9d"},
{"id":"nullable-decline-generic","source":"class Box<T\nfun String<Int>?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":42,"source_sha256":"4b0e7eeff14d4bd7aceade50bc8a49a5986a7ec7afde295ecbb7a687108be17e"},
{"id":"nullable-decline-parenthesized","source":"class Box<T\nfun (String)?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":39,"source_sha256":"3ce1171f11d2d9c90bfd10744b39d0a05db5e77271918bfa16b086a2b7ad5017"},
{"id":"nullable-decline-receiver-annotation","source":"class Box<T\nfun @A String?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":40,"source_sha256":"d003cf250ab285f50697f00d0420107689fe3071212c2fd4cab49b03b05bb907"},
{"id":"nullable-decline-generic-function","source":"class Box<T\nfun <U> String?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":41,"source_sha256":"05c3ab443063b51731dff95e765e5820e56bc48054eb35ea0a78a9b1821fae0a"},
{"id":"nullable-decline-double-question","source":"class Box<T\nfun String??.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":38,"source_sha256":"215aae3972293198f29d5eef783b1c840b4780ec2f7b82d1f6f97a9ebe5c37dd"},
{"id":"nullable-decline-double-dot","source":"class Box<T\nfun String?..healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":38,"source_sha256":"816a18f0eb2657874fe072ee181afb79bfc607e5c514f00cb962b46b7e1aa002"},
{"id":"nullable-decline-missing-name","source":"class Box<T\nfun String?.() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":30,"source_sha256":"bee2cdb2ef7ec950ebb177aef31c7849c15476e22250719d47f588762676855d"},
{"id":"nullable-decline-anonymous","source":"class Box<T\nfun String?() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":29,"source_sha256":"af6c1cfc3197d1ad1b307754c67b6cf11a506ac83656a4f0f1cd6a4e6149e051"},
{"id":"nullable-decline-missing-receiver","source":"class Box<T\nfun ?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":31,"source_sha256":"4dd2622edb91ea56f1fd3f11f98c745814473c3690689155fd18902189d57526"},
{"id":"nullable-decline-missing-dot","source":"class Box<T\nfun String?healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":36,"source_sha256":"c21fb31ccb57a4ed4fb6ab9e7073716eeaed0b6f34b8b8a54fcef68cd7803b0e"},
{"id":"nullable-decline-question-lpar","source":"class Box<T\nfun String?(","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":24,"source_sha256":"85bf0b42659e461f0c94a81cf8dd55be15e39211f61e6dc264f6562b05ef78bc"},
{"id":"nullable-decline-elvis-adjacent","source":"class Box<T\nfun String?:healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":37,"source_sha256":"b6fda32d7d9c4b2b88281cb4f3942f76587fec8e45379757ba3497ad226e46ca"},
{"id":"nullable-decline-elvis-spaced","source":"class Box<T\nfun String? : .healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":40,"source_sha256":"1d49071318ecd0b5ba8be49ed194d57f8ba694ccd15c1ac944eac6c02678470f"},
{"id":"nullable-decline-elvis-comment","source":"class Box<T\nfun String?/*x*/:.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":43,"source_sha256":"e607d95cbc45f2f65a28a7fa07676634a37986bceb68cfeff891d3fbd800a83f"},
{"id":"nullable-decline-bad-unicode","source":"class Box<T\nfun String?🦀.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":41,"source_sha256":"ff8216ff7646cfff764837b92040398d05d4ffdc82531ec6f666a01b7e779c42"},
{"id":"nullable-decline-unclosed-comment","source":"class Box<T\nfun String?/* .healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":40,"source_sha256":"bbc2ac801d84fd317c968e99ec6be23cf31746b00ac409d8fa64897e40aed78b"},
{"id":"nullable-decline-unfinished-modifier","source":"class Box<T\n@A(\nfun String?.healthy() = 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":41,"source_sha256":"9144e953c5a6be2b027cbd127385414f361e74ec6d91039ad260b0ab73a3952b"},
{"id":"nullable-decline-safe-call-expression","source":"class Box<T\nval value = receiver?.healthy()","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":43,"source_sha256":"fd47508713e386d56160b7ce9499adca91c65c11f9d1004b22f180cd90087501"},
{"id":"nullable-decline-safe-call-elvis-expression","source":"class Box<T\nval value = receiver?.healthy() ?: 2","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":48,"source_sha256":"85d38c287a821ecb2736533b01ad27c7f0685204b0383d242f3bb2edb71dcdda"},
{"id":"nullable-decline-partial-fun","source":"class Box<T\nfun","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":15,"source_sha256":"9641927c6e4a8f1cf32ba45977c2da3b3bcc463afc49e27b521bfbd6db7c8adf"},
{"id":"nullable-decline-partial-receiver","source":"class Box<T\nfun String","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":22,"source_sha256":"936ea6939a4cabf00a9b3e805f502c5a2b9fcc8dc4aba598b728e783f91a5442"},
{"id":"nullable-decline-partial-question","source":"class Box<T\nfun String?","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":23,"source_sha256":"bd7f4feca77579fe02ed6c4bf47752100b67a4405fa64725e277f93f576a141e"},
{"id":"nullable-decline-partial-separator","source":"class Box<T\nfun String?.","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":24,"source_sha256":"08c4893ea2310d84ea6d0892c0ced721bccfa03f79607ec51e1bb44a90d8ede1"},
{"id":"nullable-decline-partial-name","source":"class Box<T\nfun String?.healthy","reason":"outside first-identifier single nullable marker/DOT/name/LPAR path","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":31,"source_sha256":"db4b7ac77ae2b7628713d5191d272766f246d00253ed019bbcc9fac4b6008643"},
{"id":"nullable-lpar-no-rpar","source":"class Box<T\nfun String?.healthy(","reason":"recognizer accepts LPAR; downstream reports incomplete parameters","invariant":"header-only-recovery","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","class_end":11,"fun_start":12,"recognizer":true,"bytes":32,"source_sha256":"22cffe653f0463c671ca2197f79a2c4fb32536e926bce0dbff44f3bc0bbdb54b"},
{"id":"nullable-raw257","source":"class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*//*y*/ String?.healthy () = 2","reason":"LPAR257 outside256 raw copy","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","probe_accounting":{"lpar_raw_ordinal":257,"copied_raw_tokens":256,"recognizer":false},"bytes":787,"source_sha256":"6916a04e1007e083413ab3fcb7ebc2ef7e2bb921696e859a706b33247658f503"},
{"id":"nullable-byte4097","source":"class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ String?.healthy() = 2","reason":"LPAR end4097 outside4096 bytes","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","probe_accounting":{"lpar_end_from_probe_start":4097,"copied_raw_tokens":8,"recognizer":false},"bytes":4114,"source_sha256":"7d9b2fc4948e464458c15d80985de44f4372be9e51984a43df21d7a954909945"},
{"id":"nullable-raw-question256-dot257","source":"class Box<T\nfun /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*/ /*x*//*a*//*b*//*c*//*d*/ String?.healthy() = 2","reason":"copied QUEST256 cannot see excluded DOT257","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","probe_accounting":{"quest_raw_ordinal":256,"dot_raw_ordinal":257,"copied_raw_tokens":256,"recognizer":false},"bytes":801,"source_sha256":"b90ff15ca18ff6cbfa2ade57f37b107425eacff7cea937649b916d5636d5906e"},
{"id":"nullable-byte-question4096-dot4097","source":"class Box<T\nfun /*xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx*/ String?.healthy() = 2","reason":"copied QUEST cannot join excluded DOT","invariant":"class-end-greater-than-11","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","probe_accounting":{"quest_end_from_probe_start":4096,"dot_end_from_probe_start":4097,"copied_raw_tokens":6,"recognizer":false},"bytes":4122,"source_sha256":"6398b59f347d957117c8aa8af82b24c10cddd53b84838e991a3092625245362f"},
{"id":"nullable-aggregate-debit-resets-next-list","source":"class Box<@A T0,\n@A T1,\n@A T2,\n@A T3,\n@A T4,\n@A T5,\n@A T6,\n@A T7,\n@A T8,\n@A T9,\n@A T10,\n@A T11,\n@A T12,\n@A T13,\n@A T14,\n@A T15,\n@A T16,\n@A T17,\n@A T18,\n@A T19,\n@A T20,\n@A T21,\n@A T22,\n@A T23,\n@A T24,\n@A T25,\n@A T26,\n@A T27,\n@A T28,\n@A T29,\n@A T30,\n@A T31,\n@A T32,\n@A T33,\n@A T34,\n@A T35,\n@A T36,\n@A T37,\n@A T38,\n@A T39,\n@A T40,\n@A T41,\n@A T42,\n@A T43,\n@A T44,\n@A T45,\n@A T46,\n@A T47,\n@A T48,\n@A T49,\n@A T50,\n@A T51,\n@A T52,\n@A T53,\n@A T54,\n@A T55,\n@A T56,\n@A T57,\n@A T58,\n@A T59,\n@A T60,\n@A T61,\n@A T62,\n@A T63,\nfun String?.declined() = 1\nclass Fresh<T\nfun String?.healthy() = 2","reason":"first16 eligible windows debit16x256=4096; later classifier resets4096","invariant":"aggregate-reset","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","declined_start":512,"healthy_start":553,"bytes":578,"source_sha256":"2c5567d5e80332d9b3697c7d78fbb16b95f562a90fecb26cf87b2bf330f03d06"},
{"id":"nullable-nonclass-object","source":"object Box<T\nfun String?.healthy() = 2","reason":"!object callsite remains unchanged","invariant":"object-no-classifier-recovery","diagnostics":"nonempty","full_projection":"not specified; finite compatibility invariant only","bytes":38,"source_sha256":"d63302e8fd64d15c756cf98d58c0643b9e590d89ffd69fd69790b504380d5fec"}
]"####;

// Source-derived supplemental ownership contract SHA256: bf8a8e17aee78e01cc429385e6cf421c77ca4fef6000992d48b196a1df548bc0
const PROPERTY_OWNERSHIP: &str = r####"{
  "status": "SUPPLEMENTAL CONTRACT ONLY; SOURCE HOLD pending root scope admission",
  "current_head": "851901e5d219d14a3f0d2a2b90d7c1f01ce5484c",
  "frozen_property_contract_sha256": "2305fb9abb52d67356ad67527f3c3d5c622725a1d07753bf099c82fc0004bc8f",
  "expectation_basis": "independent source grammar in calls.rs/operations.rs/properties.rs and original ASCII byte/UTF16 arithmetic; failure log exposes boundary assertion only; no parser/native output used",
  "cases": [
    {
      "id": "nullable-decline-safe-call-expression",
      "source": "class Box<T\nval value = receiver?.healthy()",
      "bytes": 43,
      "source_sha256": "fd47508713e386d56160b7ce9499adca91c65c11f9d1004b22f180cd90087501",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "PROPERTY",
          12,
          43,
          "KT_FILE",
          null
        ],
        [
          "REFERENCE_EXPRESSION",
          24,
          32,
          "SAFE_ACCESS_EXPRESSION",
          [
            12,
            43
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          41,
          "CALL_EXPRESSION",
          [
            12,
            43
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          16,
          21,
          "value"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "expression_nodes": [
        [
          "SAFE_ACCESS_EXPRESSION",
          24,
          43,
          "PROPERTY",
          [
            12,
            43
          ]
        ],
        [
          "CALL_EXPRESSION",
          34,
          43,
          "SAFE_ACCESS_EXPRESSION",
          [
            24,
            43
          ]
        ],
        [
          "VALUE_ARGUMENT_LIST",
          41,
          43,
          "CALL_EXPRESSION",
          [
            34,
            43
          ]
        ]
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "24": [
          1,
          12
        ],
        "32": [
          1,
          20
        ],
        "34": [
          1,
          22
        ],
        "41": [
          1,
          29
        ],
        "43": [
          1,
          31
        ]
      },
      "no_fun_nodes": true
    },
    {
      "id": "nullable-decline-safe-call-elvis-expression",
      "source": "class Box<T\nval value = receiver?.healthy() ?: 2",
      "bytes": 48,
      "source_sha256": "85d38c287a821ecb2736533b01ad27c7f0685204b0383d242f3bb2edb71dcdda",
      "projection": [
        [
          "CLASS",
          0,
          11,
          "KT_FILE",
          null
        ],
        [
          "TYPE_PARAMETER_LIST",
          9,
          11,
          "CLASS",
          [
            0,
            11
          ]
        ],
        [
          "TYPE_PARAMETER",
          10,
          11,
          "TYPE_PARAMETER_LIST",
          [
            0,
            11
          ]
        ],
        [
          "PROPERTY",
          12,
          48,
          "KT_FILE",
          null
        ],
        [
          "REFERENCE_EXPRESSION",
          24,
          32,
          "SAFE_ACCESS_EXPRESSION",
          [
            12,
            48
          ]
        ],
        [
          "REFERENCE_EXPRESSION",
          34,
          41,
          "CALL_EXPRESSION",
          [
            12,
            48
          ]
        ]
      ],
      "declaration_names": [
        [
          0,
          6,
          9,
          "Box"
        ],
        [
          12,
          16,
          21,
          "value"
        ]
      ],
      "diagnostics": [
        [
          11,
          11,
          "Missing '>'"
        ]
      ],
      "expression_nodes": [
        [
          "BINARY_EXPRESSION",
          24,
          48,
          "PROPERTY",
          [
            12,
            48
          ]
        ],
        [
          "SAFE_ACCESS_EXPRESSION",
          24,
          43,
          "BINARY_EXPRESSION",
          [
            24,
            48
          ]
        ],
        [
          "CALL_EXPRESSION",
          34,
          43,
          "SAFE_ACCESS_EXPRESSION",
          [
            24,
            43
          ]
        ],
        [
          "VALUE_ARGUMENT_LIST",
          41,
          43,
          "CALL_EXPRESSION",
          [
            34,
            43
          ]
        ],
        [
          "OPERATION_REFERENCE",
          44,
          46,
          "BINARY_EXPRESSION",
          [
            24,
            48
          ]
        ],
        [
          "INTEGER_CONSTANT",
          47,
          48,
          "BINARY_EXPRESSION",
          [
            24,
            48
          ]
        ]
      ],
      "positions": {
        "0": [
          0,
          0
        ],
        "6": [
          0,
          6
        ],
        "9": [
          0,
          9
        ],
        "10": [
          0,
          10
        ],
        "11": [
          0,
          11
        ],
        "12": [
          1,
          0
        ],
        "16": [
          1,
          4
        ],
        "21": [
          1,
          9
        ],
        "24": [
          1,
          12
        ],
        "32": [
          1,
          20
        ],
        "34": [
          1,
          22
        ],
        "41": [
          1,
          29
        ],
        "43": [
          1,
          31
        ],
        "44": [
          1,
          32
        ],
        "46": [
          1,
          34
        ],
        "47": [
          1,
          35
        ],
        "48": [
          1,
          36
        ]
      },
      "no_fun_nodes": true
    }
  ],
  "preserve": "all85 historical nullable JSON bytes and all other assertions/dispatch; no product/vendor/budget/fixture changes",
  "original_intent": "initializer safe-call/Elvis expression remains PROPERTY-owned, never promoted into FUN or sibling selector declaration",
  "scope_request": "only crates/ktlsp-parser/tests/recovery_nullable_extension_functions.rs two named stable IDs + adjacent failure/scope erratum in already-owned local/vendor review notes; root admission required"
}
"####;
