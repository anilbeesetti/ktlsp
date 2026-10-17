//! Real parser/public consumer acceptance; NOT RUN locally under #96.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    local_value_binding::{LocalBindingError, ValueLookup, bind_local_value, lookup_value},
    lowering::{lower_declarations, reference_site},
    structural_regions::build_regions,
};
use std::sync::atomic::AtomicBool;

fn parse(document: &str, source: &str) -> SnapshotResult {
    let snapshot = SourceSnapshot::new(document, 7, FileKind::Source, source).unwrap();
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap()
}

fn selected(result: &SnapshotResult) -> usize {
    let tree = result.tree_for(result.snapshot()).unwrap();
    tree.elements()
        .iter()
        .position(|e| {
            e.kind == "REFERENCE_EXPRESSION"
                && e.parent
                    .is_some_and(|p| tree.elements()[p].kind == "PROPERTY")
        })
        .unwrap()
}

fn imported(
    request_source: &str,
    target_sources: &[&str],
    expected: Result<usize, LocalBindingError>,
) {
    let request = parse("Uses.kt", request_source);
    let lower = lower_declarations(&request, request.snapshot()).unwrap();
    let regions = build_regions(&lower, &request, request.snapshot()).unwrap();
    let site = reference_site(&lower, &request, request.snapshot(), selected(&request)).unwrap();
    let ValueLookup::Outer(outer) = lookup_value(&regions, &site, request.snapshot()).unwrap() else {
        panic!("expected imported continuation");
    };
    // The original requesting continuation PRECEDES the actual target producers.
    // This exercises the real worker's delayed-directory borrowing seam.
    let targets: Vec<_> = target_sources
        .iter()
        .enumerate()
        .map(|(i, source)| parse(&format!("Target{i}.kt"), source))
        .collect();
    let target_lowers: Vec<_> = targets
        .iter()
        .map(|r| lower_declarations(r, r.snapshot()).unwrap())
        .collect();
    let target_regions: Vec<_> = targets
        .iter()
        .zip(&target_lowers)
        .map(|(r, l)| build_regions(l, r, r.snapshot()).unwrap())
        .collect();
    let mut files = vec![(&regions, &request, request.snapshot())];
    files.extend(
        target_regions
            .iter()
            .zip(&targets)
            .map(|(d, r)| (d, r, r.snapshot())),
    );
    let result = outer.resolve_import(&files);
    match expected {
        Ok(index) => {
            let binding = result.unwrap();
            assert!(std::ptr::eq(binding.site(), &site));
            assert!(std::ptr::eq(binding.directory(), &target_regions[index]));
            assert!(std::ptr::eq(binding.result(), &targets[index]));
            let declaration = &target_lowers[index].declarations()[0];
            assert!(std::ptr::eq(binding.target(), declaration));
            assert!(std::ptr::eq(binding.import(), &lower.imports()[0]));
            let tree = binding
                .result()
                .tree_for(binding.directory().snapshot())
                .unwrap();
            let range = binding.target().name_range();
            assert_eq!(&tree.source()[range.clone()], binding.target().raw_name());
            let start = tree.position(range.start).unwrap();
            let end = tree.position(range.end).unwrap();
            assert_eq!(end.line, start.line);
            assert_eq!(
                end.character - start.character,
                binding.target().raw_name().encode_utf16().count() as u32
            );
            assert_eq!(
                binding.result().snapshot().document(),
                format!("Target{index}.kt")
            );
        }
        Err(error) => assert_eq!(result.unwrap_err(), error),
    }
}

#[test]
fn two_original_sources_late_producer_and_quoted_component_boundaries() {
    let _serial = functional::serial();
    imported(
        "package app\nimport values.answer\nfun use() { val result = answer }",
        &["package values\nval answer = 1"],
        Ok(0),
    );
    imported(
        "package app\nimport values.answer\nfun use() { val result = answer }",
        &[
            "package values\nval answer = 1;\n",
            "package unrelated\nval ignored = 2;\n",
        ],
        Ok(0),
    );
    imported(
        "package app\nimport `values.space`.`answer😀`\nfun use() { val result = `answer😀` }",
        &[
            "package values.space\nval `answer😀` = 1",
            "package `values.space`\n// 😀\nval `answer😀` = 1",
        ],
        Ok(1),
    );
    imported(
        "import values.`answer`\nfun use() { val result = answer }",
        &["package `values`\nval answer = 1"],
        Ok(0),
    );
    imported(
        "import values.answer\nfun use() { val result = answer }",
        &["package `values.answer`\nval answer = 1"],
        Err(LocalBindingError::NoEligibleLocal),
    );
}

#[test]
fn duplicate_imports_candidates_and_nonproperty_namespace_competition_refuse() {
    let _serial = functional::serial();
    let request = "import values.answer\nfun use() { val result = answer }";
    imported(
        "import values.answer\nimport `values`.`answer`\nfun use() { val result = answer }",
        &["package values\nval answer = 1"],
        Err(LocalBindingError::AmbiguousLocal),
    );
    imported(
        request,
        &["package values\nval answer = 1\nval `answer` = 2"],
        Err(LocalBindingError::AmbiguousLocal),
    );
    imported(
        request,
        &[
            "package values\nval answer = 1",
            "package `values`\nval `answer` = 2",
        ],
        Err(LocalBindingError::AmbiguousLocal),
    );
    imported(
        request,
        &[
            "package values\nval answer = 1",
            "package values\nclass answer {}",
        ],
        Err(LocalBindingError::UnsupportedContext),
    );
    imported(
        "import values.answer as answer\nfun use() { val result = answer }",
        &["package values\nval answer = 1"],
        Err(LocalBindingError::UnsupportedContext),
    );
    imported(
        "import values.*\nfun use() { val result = answer }",
        &["package values\nval answer = 1"],
        Err(LocalBindingError::NoEligibleLocal),
    );
    imported(
        request,
        &["package values\nval answer = 1", "package unrelated\nval ignored"],
        Err(LocalBindingError::UnsupportedContext),
    );
}

#[test]
fn existing_local_precedence_self_future_parameters_and_unrelated_calls() {
    let _serial = functional::serial();
    for (body, expected) in [
        (
            "fun use() { val answer = 1; val result = answer }",
            Some("PROPERTY"),
        ),
        (
            "fun use(answer: Int) { val answer = answer }",
            Some("VALUE_PARAMETER"),
        ),
        (
            "fun use(answer: Int) { val result = answer; val answer = 1 }",
            Some("VALUE_PARAMETER"),
        ),
        ("fun use() { val answer = answer }", None),
        ("fun use() { val result = answer; val answer = 1 }", None),
    ] {
        let request = parse("Uses.kt", &format!("import values.answer\n{body}"));
        let lower = lower_declarations(&request, request.snapshot()).unwrap();
        let regions = build_regions(&lower, &request, request.snapshot()).unwrap();
        let site = reference_site(&lower, &request, request.snapshot(), selected(&request)).unwrap();
        match (
            lookup_value(&regions, &site, request.snapshot()).unwrap(),
            expected,
        ) {
            (ValueLookup::Local(binding), Some(kind)) => {
                let old = bind_local_value(&regions, &site, request.snapshot()).unwrap();
                assert_eq!(binding.target().kind(), kind);
                assert!(std::ptr::eq(binding.target(), old.target()));
            }
            (ValueLookup::Outer(_), None) => assert_eq!(
                bind_local_value(&regions, &site, request.snapshot()).unwrap_err(),
                LocalBindingError::NoEligibleLocal
            ),
            (actual, expected) => panic!("unexpected lookup: {actual:?}, {expected:?}"),
        }
    }
    for body in [
        "fun use() { val result = answer; unknown() }",
        "fun use() { unknown(); val result = answer }",
        "fun use() { val result = answer; later(); fun later() {} }",
    ] {
        let request = parse("Uses.kt", &format!("import values.answer\n{body}"));
        let lower = lower_declarations(&request, request.snapshot()).unwrap();
        let regions = build_regions(&lower, &request, request.snapshot()).unwrap();
        let site = reference_site(&lower, &request, request.snapshot(), selected(&request)).unwrap();
        assert_eq!(
            lookup_value(&regions, &site, request.snapshot()).unwrap_err(),
            LocalBindingError::NoEligibleLocal
        );
    }
}

#[test]
fn foreign_equal_byte_and_late_stale_producers_never_publish_prefix() {
    let _serial = functional::serial();
    let request = parse(
        "Uses.kt",
        "import values.answer\nfun use() { val result = answer }",
    );
    let lower = lower_declarations(&request, request.snapshot()).unwrap();
    let regions = build_regions(&lower, &request, request.snapshot()).unwrap();
    let site = reference_site(&lower, &request, request.snapshot(), selected(&request)).unwrap();
    let target = parse("Definitions.kt", "package values\nval answer = 1");
    let target_lower = lower_declarations(&target, target.snapshot()).unwrap();
    let target_regions = build_regions(&target_lower, &target, target.snapshot()).unwrap();
    let foreign = parse("Definitions.kt", target.snapshot().source());
    let late = parse("Other.kt", "package unrelated\nval ignored = 1");
    let late_lower = lower_declarations(&late, late.snapshot()).unwrap();
    let late_regions = build_regions(&late_lower, &late, late.snapshot()).unwrap();
    let stale = SourceSnapshot::new("Other.kt", 8, FileKind::Source, late.snapshot().source()).unwrap();
    for (files, expected) in [
        (
            vec![
                (&regions, &request, request.snapshot()),
                (&target_regions, &foreign, foreign.snapshot()),
            ],
            LocalBindingError::InvalidOrigin,
        ),
        (
            vec![
                (&regions, &request, request.snapshot()),
                (&target_regions, &target, target.snapshot()),
                (&late_regions, &late, &stale),
            ],
            LocalBindingError::StaleSnapshot,
        ),
        (
            vec![(&target_regions, &target, target.snapshot())],
            LocalBindingError::InvalidOrigin,
        ),
        (
            vec![
                (&regions, &request, request.snapshot()),
                (&target_regions, &target, target.snapshot()),
                (&regions, &request, request.snapshot()),
            ],
            LocalBindingError::InvalidOrigin,
        ),
    ] {
        let ValueLookup::Outer(outer) = lookup_value(&regions, &site, request.snapshot()).unwrap()
        else {
            panic!("expected outer")
        };
        assert_eq!(outer.resolve_import(&files).unwrap_err(), expected);
    }
    let foreign_lower = lower_declarations(&foreign, foreign.snapshot()).unwrap();
    let foreign_regions = build_regions(&foreign_lower, &foreign, foreign.snapshot()).unwrap();
    assert_eq!(
        lookup_value(&foreign_regions, &site, request.snapshot()).unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    let stale_request =
        SourceSnapshot::new("Uses.kt", 8, FileKind::Source, request.snapshot().source()).unwrap();
    assert_eq!(
        lookup_value(&regions, &site, &stale_request).unwrap_err(),
        LocalBindingError::StaleSnapshot
    );
}
