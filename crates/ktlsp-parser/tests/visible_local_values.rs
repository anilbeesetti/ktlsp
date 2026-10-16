//! Real producer/public API acceptance; execute only in ROOT-admitted hosted CI.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    local_value_binding::{LocalBindingError, bind_local_value, visible_local_values},
    lowering::{lower_declarations, reference_site},
    structural_regions::build_regions,
};
use std::{fmt::Write as _, sync::atomic::AtomicBool};

fn parse(source: &str) -> SnapshotResult {
    let snapshot = SourceSnapshot::new("visible-values", 7, FileKind::Source, source).unwrap();
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap()
}

fn accepted(source: &str, queries: &[(usize, &[usize])]) {
    let result = parse(source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    assert!(tree.diagnostics().is_empty(), "{source}");
    let lower = lower_declarations(&result, current).unwrap();
    assert!(lower.errors().is_empty());
    let directory = build_regions(&lower, &result, current).unwrap();
    for &(offset, ordinals) in queries {
        let index = tree
            .elements()
            .iter()
            .position(|node| node.kind == "REFERENCE_EXPRESSION" && node.start == offset)
            .unwrap();
        let found = visible_local_values(&directory, &result, index, current).unwrap();
        let original = reference_site(&lower, &result, current, index).unwrap();
        assert!(std::ptr::eq(found.directory(), &directory));
        assert!(std::ptr::eq(found.site().origin(), &lower));
        assert_eq!(found.site().expression_index(), index);
        assert_eq!(found.site().token_index(), original.token_index());
        assert_eq!(found.site().range(), original.range());
        assert_eq!(found.site().raw_spelling(), original.raw_spelling());
        assert_eq!(
            found.site().raw_spelling().as_ptr(),
            tree.source()[original.range()].as_ptr()
        );
        assert!(std::ptr::eq(
            found.site().containing_property(),
            original.containing_property()
        ));
        assert!(std::ptr::eq(found.site().function(), original.function()));
        assert_eq!(found.body().cst_index(), original.body_index());
        assert!(
            directory
                .regions()
                .iter()
                .any(|body| std::ptr::eq(body, found.body()))
        );
        assert_eq!(found.values().len(), ordinals.len());
        assert!(
            found
                .values()
                .windows(2)
                .all(|pair| pair[0].cst_index() < pair[1].cst_index())
        );
        for (&value, &ordinal) in found.values().iter().zip(ordinals) {
            let declaration = &lower.declarations()[ordinal];
            assert!(std::ptr::eq(value, declaration));
            assert_eq!(value.raw_name(), &source[value.name_range()]);
            assert_eq!(
                value.raw_name().as_ptr(),
                tree.source()[value.name_range()].as_ptr()
            );
            assert_eq!(value.owner(), found.body().introducer());
            assert!(value.range().start <= value.name_range().start);
            assert!(value.name_range().end <= value.range().end);
            if value.kind() == "VALUE_PARAMETER" {
                assert_eq!(value.cst_parent_kind(), "VALUE_PARAMETER_LIST");
                assert!(value.range().end <= found.body().range().start);
            } else {
                let membership = directory.memberships()[ordinal].region();
                assert!(std::ptr::eq(&directory.regions()[membership], found.body()));
                assert_eq!(value.cst_parent_index(), Some(found.body().cst_index()));
                assert!(value.range().end <= found.site().range().start);
                assert!(value.cst_index() < found.site().containing_property().cst_index());
            }
        }
    }
}

#[test]
fn original_order_shadow_points_unknown_query_unused_parameters_and_siblings() {
    let _serial = functional::serial();
    let source = "fun f(p: Int, unused: Long) { val before = missing; val p = p; val after = missing }\nfun g(p: Int) { val y = missing }\nclass Other {}\nval outside = 1\n";
    accepted(
        source,
        &[
            (source.find("= missing").unwrap() + 2, &[1, 2]),
            (source.find("= p").unwrap() + 2, &[1, 2, 3]),
            (
                source.find("after = missing").unwrap() + "after = ".len(),
                &[2, 3, 4],
            ),
            (source.rfind("= missing").unwrap() + 2, &[7]),
        ],
    );
    let source = "fun f() { val first = unknown; val x = 1; val after = unknown }\n";
    accepted(
        source,
        &[
            (source.find("unknown").unwrap(), &[]),
            (source.rfind("unknown").unwrap(), &[1, 2]),
        ],
    );
    let source = "fun f() { val y = unknown; val later = 1; val `later` = 2 }\nfun g(x: Int, x: Int) { val a = 1 }\n";
    accepted(source, &[(source.find("unknown").unwrap(), &[])]);
    // Ambiguity of an unknown query's own future-only spelling is not ambiguity
    // of a visible candidate; do not accidentally bind the query first.
    let source = "fun f() { val x = 1; val y = later; val later = 1; val `later` = 2 }\n";
    accepted(source, &[(source.find("= later").unwrap() + 2, &[1])]);
    let source = "fun f() { val y = unknown; fun nested() {} }\n";
    accepted(source, &[(source.find("unknown").unwrap(), &[])]);
}

#[test]
fn quoted_semantic_shadowing_crlf_non_bmp_and_header_names_are_not_values() {
    let _serial = functional::serial();
    let source = "import elsewhere.Int\r\nclass Int {}\r\nfun f(`🦀`: `Long`, unused: Int) {\r\n val before: Int = `🦀`; val `🦀`: Int = `🦀`; val after = unknown\r\n}\r\n";
    accepted(
        source,
        &[
            (source.find("= `🦀`").unwrap() + 2, &[2, 3]),
            (source.rfind("= `🦀`").unwrap() + 2, &[2, 3, 4]),
            (source.find("unknown").unwrap(), &[3, 4, 5]),
        ],
    );
    let source = "fun f(`p`: Int) { val p = p; val y = `p` }\n";
    accepted(
        source,
        &[
            (source.find("= p").unwrap() + 2, &[1]),
            (source.find("= `p`").unwrap() + 2, &[2]),
        ],
    );
}

#[test]
fn visible_ambiguity_and_unsupported_whole_bodies_are_atomic() {
    let _serial = functional::serial();
    for (source, expected) in [
        (
            "fun f(p: Int, `p`: Int) { val y = unknown }\n",
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f(p: Int, p: Int) { val p = 1; val y = unknown }\n",
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f() { val x = 1; val y = unknown; val `x` = 2 }\n",
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f(p: Int) { val p = 1; val y = unknown; val p = 2 }\n",
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f() { val a = 1; val x = 1; val x = 2; val y = unknown }\n",
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f(café: Int) { val y = unknown }\n",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun f() { val café = 1; val y = unknown }\n",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun f() { val x = 1; val y = café }\n",
            LocalBindingError::UnsupportedSpelling,
        ),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        let index = tree
            .elements()
            .iter()
            .position(|node| {
                node.kind == "REFERENCE_EXPRESSION"
                    && &source[node.start..node.end]
                        == if source.contains("= café") {
                            "café"
                        } else {
                            "unknown"
                        }
            })
            .unwrap();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        assert_eq!(
            visible_local_values(&directory, &result, index, current).unwrap_err(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn unsupported_whole_bodies_refuse_before_a_directory_can_be_created() {
    let _serial = functional::serial();
    for (source, rejected_kind, rejected_token) in [
        (
            "fun f() { val y = unknown; var z = 1 }\n",
            "VAR_KEYWORD",
            true,
        ),
        (
            "fun f() { val y = unknown; val z = 1 + 2 }\n",
            "BINARY_EXPRESSION",
            false,
        ),
        ("fun f() { val y = unknown; val z = }\n", "FUN", false),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        // The producer did retain the query, but reference-bearing nonordinary
        // functions are refused by the existing lowerer, before any directory.
        assert!(
            tree.elements()
                .iter()
                .any(|node| node.kind == "REFERENCE_EXPRESSION"
                    && &source[node.start..node.end] == "unknown")
        );
        let ktlsp_parser::lowering::LoweringError::Unsupported { element } =
            lower_declarations(&result, current).unwrap_err()
        else {
            panic!("unexpected admission refusal for {source}")
        };
        let rejected = &tree.elements()[element];
        assert_eq!(rejected.kind, rejected_kind, "{source}");
        assert_eq!(rejected.token, rejected_token, "{source}");
        assert!(rejected.start < rejected.end);
        assert!(rejected.end <= source.len());
    }
}

#[test]
fn origin_snapshot_dimensions_invalid_and_header_indices_are_refused() {
    let _serial = functional::serial();
    let source = "fun f(p: Long) { val x: Int = missing }\n";
    let result = parse(source);
    let current = result.snapshot();
    let lower = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lower, &result, current).unwrap();
    let tree = result.tree_for(current).unwrap();
    let index = tree
        .elements()
        .iter()
        .position(|node| {
            node.kind == "REFERENCE_EXPRESSION" && node.start == source.find("missing").unwrap()
        })
        .unwrap();
    assert_eq!(
        bind_local_value(
            &directory,
            &reference_site(&lower, &result, current, index).unwrap(),
            current
        )
        .unwrap_err(),
        LocalBindingError::NoEligibleLocal
    );
    assert_eq!(
        visible_local_values(&directory, &result, index, current)
            .unwrap()
            .values()
            .len(),
        1
    );
    let foreign = parse(source);
    let foreign_lower = lower_declarations(&foreign, foreign.snapshot()).unwrap();
    let foreign_directory = build_regions(&foreign_lower, &foreign, foreign.snapshot()).unwrap();
    for (directory, result) in [(&directory, &foreign), (&foreign_directory, &result)] {
        assert_eq!(
            visible_local_values(directory, result, index, current).unwrap_err(),
            LocalBindingError::InvalidOrigin
        );
    }
    for current in [
        SourceSnapshot::new("other", 7, FileKind::Source, source).unwrap(),
        SourceSnapshot::new("visible-values", 8, FileKind::Source, source).unwrap(),
        SourceSnapshot::new("visible-values", 7, FileKind::Script, source).unwrap(),
        SourceSnapshot::new(
            "visible-values",
            7,
            FileKind::Source,
            source.replace("missing", "changed"),
        )
        .unwrap(),
    ] {
        assert_eq!(
            visible_local_values(&directory, &result, index, &current).unwrap_err(),
            LocalBindingError::StaleSnapshot
        );
    }
    let mut refused = 0;
    for (index, node) in tree.elements().iter().enumerate() {
        if node.kind == "IDENTIFIER"
            || node.kind == "FUN"
            || node.kind == "VALUE_PARAMETER"
            || (node.kind == "REFERENCE_EXPRESSION"
                && node.start != source.find("missing").unwrap())
        {
            assert_eq!(
                visible_local_values(&directory, &result, index, current).unwrap_err(),
                LocalBindingError::UnsupportedContext
            );
            refused += 1;
        }
    }
    assert!(refused > 4);
    assert_eq!(
        visible_local_values(&directory, &result, usize::MAX, current).unwrap_err(),
        LocalBindingError::UnsupportedContext
    );
}

#[test]
fn actual_large_body_refuses_work_atomically_without_a_prefix() {
    let _serial = functional::serial();
    let mut source = String::from("fun f() {");
    for ordinal in 0..200 {
        write!(source, " val x{ordinal} = 1;").unwrap();
    }
    source.push_str(" val last = unknown }\n");
    let result = parse(&source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    let lower = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lower, &result, current).unwrap();
    let index = tree
        .elements()
        .iter()
        .position(|node| node.kind == "REFERENCE_EXPRESSION")
        .unwrap();
    assert_eq!(
        visible_local_values(&directory, &result, index, current).unwrap_err(),
        LocalBindingError::Resource
    );
}
