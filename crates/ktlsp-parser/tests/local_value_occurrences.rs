//! Real hosted production-parser consumers, never synthetic CST acceptance.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    local_value_binding::{LocalBindingError, bind_local_value, local_value_occurrences},
    lowering::{lower_declarations, reference_site},
    structural_regions::build_regions,
};
use std::{fmt::Write as _, ops::Range, sync::atomic::AtomicBool};

fn parse(source: &str) -> SnapshotResult {
    let snapshot = SourceSnapshot::new("local-occurrences", 7, FileKind::Source, source).unwrap();
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap()
}

fn accepted(
    source: &str,
    target_ordinal: usize,
    target_range: Range<usize>,
    ranges: &[Range<usize>],
) {
    let result = parse(source);
    let current = result.snapshot();
    let parsed = result.tree_for(current).unwrap();
    assert!(parsed.diagnostics().is_empty());
    let lowered = lower_declarations(&result, current).unwrap();
    assert!(lowered.errors().is_empty());
    let directory = build_regions(&lowered, &result, current).unwrap();
    let target = &lowered.declarations()[target_ordinal];
    let found = local_value_occurrences(&directory, &result, target, current).unwrap();
    assert!(std::ptr::eq(found.directory(), &directory));
    assert!(std::ptr::eq(found.target(), target));
    assert_eq!(found.target().name_range(), target_range);
    assert_eq!(
        found
            .occurrences()
            .iter()
            .map(|s| s.range())
            .collect::<Vec<_>>(),
        ranges
    );
    assert_eq!(found.body().cst_index(), target.cst_parent_index().unwrap());
    for site in found.occurrences() {
        assert!(std::ptr::eq(site.origin(), &lowered));
        assert_eq!(site.raw_spelling(), &source[site.range()]);
        assert_eq!(
            site.raw_spelling().as_ptr(),
            parsed.source()[site.range()].as_ptr()
        );
        assert_eq!(site.semantic_spelling(), target.semantic_spelling());
        let original = reference_site(&lowered, &result, current, site.expression_index()).unwrap();
        assert_eq!(site.token_index(), original.token_index());
        assert!(std::ptr::eq(
            site.containing_property(),
            original.containing_property()
        ));
        assert!(std::ptr::eq(site.function(), original.function()));
        let binding = bind_local_value(&directory, site, current).unwrap();
        assert!(std::ptr::eq(binding.target(), target));
        assert!(std::ptr::eq(binding.body(), found.body()));
    }
}

#[test]
fn production_complete_original_occurrences_and_unused_targets() {
    let _serial = functional::serial();
    let source = "fun f() { val x = 1; val a = x; val a = 2; val b = x; val c = a; val unused = 3 }\nfun g() { val x = 1; val y = x }\n";
    accepted(source, 1, 14..15, &[29..30, 51..52]);
    accepted(source, 6, 69..75, &[]);
    accepted(source, 8, 96..97, std::slice::from_ref(&(111..112)));
    accepted(
        "fun f() { val `x` = 1; val a = x; val b = `x` }\n",
        1,
        14..17,
        &[31..32, 42..45],
    );
    let unicode = "fun f() { val `🦀` = 1; val y = `🦀` }\n";
    accepted(unicode, 1, 14..20, std::slice::from_ref(&(34..40)));
    assert_eq!(
        ktlsp_text::offset_to_position(unicode, 34).unwrap(),
        ktlsp_text::Position {
            line: 0,
            character: 32
        }
    );
    assert_eq!(
        ktlsp_text::offset_to_position(unicode, 40).unwrap(),
        ktlsp_text::Position {
            line: 0,
            character: 36
        }
    );
    accepted(
        "fun f() { val before = x; val x = x; val after = x }\n",
        2,
        30..31,
        std::slice::from_ref(&(49..50)),
    );
    accepted("fun f() { val x = x }\n", 1, 14..15, &[]);
    accepted("fun f() { val y = x; val x = 1 }\n", 2, 25..26, &[]);
}

#[test]
fn production_origin_and_all_snapshot_dimensions_are_authenticated() {
    let _serial = functional::serial();
    let source = "fun f() { val x: Long = 1; val y: Int = x }\n";
    let result = parse(source);
    let snapshot = result.snapshot();
    let lowered = lower_declarations(&result, snapshot).unwrap();
    let directory = build_regions(&lowered, &result, snapshot).unwrap();
    let target = &lowered.declarations()[1];
    let foreign_lower = lower_declarations(&result, snapshot).unwrap();
    let foreign_directory = build_regions(&foreign_lower, &result, snapshot).unwrap();
    assert_eq!(
        local_value_occurrences(
            &directory,
            &result,
            &foreign_lower.declarations()[1],
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    assert_eq!(
        local_value_occurrences(&foreign_directory, &result, target, snapshot).unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    let foreign_result = parse(source);
    assert_eq!(
        local_value_occurrences(&directory, &foreign_result, target, snapshot).unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    for current in [
        SourceSnapshot::new("other-document", 7, FileKind::Source, source).unwrap(),
        SourceSnapshot::new("local-occurrences", 8, FileKind::Source, source).unwrap(),
        SourceSnapshot::new("local-occurrences", 7, FileKind::Script, source).unwrap(),
        SourceSnapshot::new(
            "local-occurrences",
            7,
            FileKind::Source,
            source.replace("1", "2"),
        )
        .unwrap(),
    ] {
        assert_eq!(
            local_value_occurrences(&directory, &result, target, &current).unwrap_err(),
            LocalBindingError::StaleSnapshot
        );
    }
}

#[test]
fn production_ambiguity_unsupported_context_and_resource_are_atomic() {
    let _serial = functional::serial();
    for (source, ordinal, expected) in [
        (
            "fun f() { val x: Long = 1; val a: Int = x; val `x`: Other = 2 }\n",
            1,
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f() { val x = 1; val a = x; val `x` = 2 }\n",
            1,
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f() { val x = 1; val `x` = 2 }\n",
            1,
            LocalBindingError::AmbiguousLocal,
        ),
        (
            "fun f() { val x = 1; fun nested() {} }\n",
            1,
            LocalBindingError::UnsupportedContext,
        ),
        ("val x = 1\n", 0, LocalBindingError::UnsupportedContext),
        ("fun f() {}\n", 0, LocalBindingError::UnsupportedContext),
        (
            "fun f() { val x = 1; val café = 2 }\n",
            1,
            LocalBindingError::UnsupportedSpelling,
        ),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let lowered = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lowered, &result, current).unwrap();
        assert_eq!(
            local_value_occurrences(
                &directory,
                &result,
                &lowered.declarations()[ordinal],
                current
            )
            .unwrap_err(),
            expected,
            "{source}"
        );
    }
    let mut source = String::from("fun f() { val x: Long = 1;");
    for i in 0..200 {
        write!(source, " val use{i}: Int = x;").unwrap();
    }
    source.push_str(" }\n");
    let result = parse(&source);
    let current = result.snapshot();
    let lowered = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lowered, &result, current).unwrap();
    assert_eq!(
        local_value_occurrences(&directory, &result, &lowered.declarations()[1], current)
            .unwrap_err(),
        LocalBindingError::Resource
    );
}

#[test]
fn typed_local_annotations_are_syntax_not_occurrences_or_inferred_types() {
    use ktlsp_parser::{
        integer_initializer_types::{
            IntegerInitializerError, IntegerType, infer_integer_initializer,
        },
        lowering::ReferenceSiteError,
    };
    let _serial = functional::serial();
    for newline in ["\n", "\r\n"] {
        let type_name = if newline == "\n" { "Long" } else { "`Long`" };
        let source = format!(
            "import elsewhere.Int{newline}class Int {{}}{newline}fun f() {{{newline} val `🦀`: {type_name} = 1; val y: Int = `🦀`; val z = y; val a = 2; val b: `🦀` = a{newline}}}"
        );
        let result = parse(&source);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        assert!(tree.diagnostics().is_empty());
        let annotation_nodes: Vec<_> = tree.elements().iter().enumerate()
            .filter(|(_, node)| matches!(node.kind.as_str(), "TYPE_REFERENCE" | "USER_TYPE" | "REFERENCE_EXPRESSION" | "COLON"))
            .map(|(index, node)| serde_json::json!({
                "index":index,"kind":node.kind,"token":node.token,
                "start":node.start,"end":node.end,"parent":node.parent,"subtreeEnd":node.subtree_end
            })).collect();
        let producer_proof = serde_json::json!({"kind":"typed-local-producer","source":source,"nodes":annotation_nodes});
        std::io::Write::write_all(
            &mut std::io::stderr().lock(),
            format!("{producer_proof}\n").as_bytes(),
        )
        .unwrap();
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        for (ordinal, reference, expected_type) in [
            (2, source.find("= `🦀`").unwrap() + 2, None),
            (3, source.find("= y").unwrap() + 2, None),
            (5, source.rfind("= a").unwrap() + 2, Some(IntegerType::Int)),
        ] {
            let target = &lower.declarations()[ordinal];
            let found = local_value_occurrences(&directory, &result, target, snapshot).unwrap();
            assert_eq!(found.occurrences().len(), 1);
            let site = &found.occurrences()[0];
            assert_eq!(site.range().start, reference);
            assert_eq!(
                site.containing_property().cst_index(),
                lower.declarations()[ordinal + 1].cst_index()
            );
            assert!(std::ptr::eq(found.target(), target));
            assert_eq!(&source[target.name_range()], target.raw_name());
            let binding = bind_local_value(&directory, site, snapshot).unwrap();
            assert!(std::ptr::eq(binding.target(), target));
            match expected_type {
                Some(ty) => assert_eq!(
                    infer_integer_initializer(&binding, snapshot)
                        .unwrap()
                        .default_type(),
                    ty
                ),
                None => assert_eq!(
                    infer_integer_initializer(&binding, snapshot).unwrap_err(),
                    IntegerInitializerError::UnsupportedContext
                ),
            }
            let proof = serde_json::json!({
                "kind":"typed-local-production",
                "source":source,
                "target":target.raw_name(),
                "targetRange":[target.name_range().start,target.name_range().end],
                "siteRange":[site.range().start,site.range().end],
                "siteUtf16":[ktlsp_text::offset_to_position(&source, site.range().start).unwrap(),ktlsp_text::offset_to_position(&source, site.range().end).unwrap()],
                "declaration": &source[target.range()],
                "integerDefault":expected_type.map(|ty|format!("{ty:?}")),
                "integerRefusal":expected_type.is_none(),
                "originalTarget":std::ptr::eq(binding.target(), target),
            });
            std::io::Write::write_all(
                &mut std::io::stderr().lock(),
                format!("{proof}\n").as_bytes(),
            )
            .unwrap();
        }
        for (index, node) in tree.elements().iter().enumerate() {
            if node.kind == "REFERENCE_EXPRESSION"
                && node
                    .parent
                    .is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
            {
                assert_eq!(
                    reference_site(&lower, &result, snapshot, index).unwrap_err(),
                    ReferenceSiteError::UnsupportedContext
                );
            }
        }
    }
}

#[test]
fn typed_local_unsupported_annotations_refuse_the_whole_projection() {
    let _serial = functional::serial();
    for annotation in [
        "pkg.Int",
        "Int?",
        "List<Int>",
        "@A Int",
        "(Int)",
        "() -> Int",
        "Int & Any",
        "dynamic",
        "",
        ": Int",
        "Int: Long",
        "= Int",
    ] {
        for typed_first in [true, false] {
            let typed = format!("val x: {annotation} = 1;");
            let source = if typed_first {
                format!("fun f() {{ {typed} val y = x }}")
            } else {
                format!("fun f() {{ val before = 1; val y = before; {typed} }}")
            };
            let result = parse(&source);
            assert!(
                lower_declarations(&result, result.snapshot()).is_err(),
                "{source}"
            );
        }
    }
    for source in [
        "fun f() { val x Int = 1; val y = x }",
        "fun f() { val x: Int; val y = x }",
        "fun f() { val x = 1: Int; val y = x }",
        "fun f() { val x: Int = = 1; val y = x }",
    ] {
        let result = parse(source);
        assert!(
            lower_declarations(&result, result.snapshot()).is_err(),
            "{source}"
        );
    }
}
