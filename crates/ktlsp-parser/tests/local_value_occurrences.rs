//! Real hosted production-parser consumers, never synthetic CST acceptance.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    local_value_binding::{
        LocalBindingError, LocalValueAnchor, bind_local_value, local_value_occurrences,
    },
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
    let found = local_value_occurrences(
        &directory,
        &result,
        LocalValueAnchor::Declaration(target),
        current,
    )
    .unwrap();
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
    assert_eq!(found.body().introducer(), target.owner());
    if target.kind() == "PROPERTY" {
        assert_eq!(found.body().cst_index(), target.cst_parent_index().unwrap());
    } else {
        assert_eq!(target.kind(), "VALUE_PARAMETER");
        assert_eq!(target.cst_parent_kind(), "VALUE_PARAMETER_LIST");
        assert_eq!(directory.memberships()[target_ordinal].region(), 0);
        assert!(target.range().end <= found.body().range().start);
    }
    if let Some(first) = found.occurrences().first() {
        let by_use = local_value_occurrences(
            &directory,
            &result,
            LocalValueAnchor::ReferenceExpression(first.expression_index()),
            current,
        )
        .unwrap();
        assert!(std::ptr::eq(by_use.target(), target));
        assert_eq!(
            by_use
                .occurrences()
                .iter()
                .map(|site| site.range())
                .collect::<Vec<_>>(),
            ranges
        );
    }
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
        if target.kind() == "VALUE_PARAMETER" {
            assert_eq!(
                ktlsp_parser::integer_initializer_types::infer_integer_initializer(&binding, current)
                    .unwrap_err(),
                ktlsp_parser::integer_initializer_types::IntegerInitializerError::UnsupportedContext
            );
        }
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
    let tree = result.tree_for(snapshot).unwrap();
    let reference = tree
        .elements()
        .iter()
        .position(|node| {
            node.kind == "REFERENCE_EXPRESSION"
                && node
                    .parent
                    .is_some_and(|parent| tree.elements()[parent].kind == "PROPERTY")
        })
        .unwrap();
    let foreign_lower = lower_declarations(&result, snapshot).unwrap();
    let foreign_directory = build_regions(&foreign_lower, &result, snapshot).unwrap();
    assert_eq!(
        local_value_occurrences(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&foreign_lower.declarations()[1]),
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    assert_eq!(
        local_value_occurrences(
            &foreign_directory,
            &result,
            LocalValueAnchor::Declaration(target),
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    let foreign_result = parse(source);
    assert_eq!(
        local_value_occurrences(
            &directory,
            &foreign_result,
            LocalValueAnchor::Declaration(target),
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    assert_eq!(
        local_value_occurrences(
            &directory,
            &foreign_result,
            LocalValueAnchor::ReferenceExpression(reference),
            snapshot
        )
        .unwrap_err(),
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
        for anchor in [
            LocalValueAnchor::Declaration(target),
            LocalValueAnchor::ReferenceExpression(reference),
        ] {
            assert_eq!(
                local_value_occurrences(&directory, &result, anchor, &current).unwrap_err(),
                LocalBindingError::StaleSnapshot
            );
        }
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
                LocalValueAnchor::Declaration(&lowered.declarations()[ordinal]),
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
        local_value_occurrences(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&lowered.declarations()[1]),
            current
        )
        .unwrap_err(),
        LocalBindingError::Resource
    );
    assert_eq!(
        ktlsp_parser::local_value_binding::guarded_local_value_rename(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&lowered.declarations()[1]),
            current,
            Some("fresh"),
        )
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
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        for (ordinal, reference, expected_type) in [
            (2, source.find("= `🦀`").unwrap() + 2, None),
            (3, source.find("= y").unwrap() + 2, None),
            (5, source.rfind("= a").unwrap() + 2, Some(IntegerType::Int)),
        ] {
            let target = &lower.declarations()[ordinal];
            let found = local_value_occurrences(
                &directory,
                &result,
                LocalValueAnchor::Declaration(target),
                snapshot,
            )
            .unwrap();
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

#[test]
fn ordinary_parameters_and_preceding_locals_partition_shadowed_value_uses() {
    let _serial = functional::serial();
    let source =
        "fun f(p: T, unused: Q) { val a = p; val p = p; val b = p }\nfun g(p: T) { val a = p }\n";
    let p = source.find("p: T").unwrap();
    let unused = source.find("unused").unwrap();
    let local = source.find("val p").unwrap() + 4;
    let other = source.rfind("p: T").unwrap();
    let uses: Vec<_> = source
        .match_indices("= p")
        .map(|(offset, _)| offset + 2..offset + 3)
        .collect();
    accepted(source, 1, p..p + 1, &uses[..2]);
    accepted(source, 2, unused..unused + 6, &[]);
    accepted(source, 4, local..local + 1, &uses[2..3]);
    accepted(source, 7, other..other + 1, &uses[3..]);

    // Unrelated duplicate parameters must not invalidate an unambiguous local.
    let unrelated = "fun f(p: T, `p`: Q) { val x = 1; val y = x }";
    let x = unrelated.find("x =").unwrap();
    let reference = unrelated.rfind("= x").unwrap() + 2;
    accepted(
        unrelated,
        3,
        x..x + 1,
        std::slice::from_ref(&(reference..reference + 1)),
    );
}

#[test]
fn quoted_crlf_parameters_keep_header_ownership_and_exclude_type_references() {
    let _serial = functional::serial();
    let source = "// 😀\r\nfun f(`p🦀`: `Long`, unused: Int,) {\r\n val y: `p🦀` = `p🦀`\r\n}\r\n";
    let name = "`p🦀`";
    let declaration = source.find(name).unwrap();
    let reference = source.rfind(name).unwrap();
    let result = parse(source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    let lowered = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lowered, &result, current).unwrap();
    accepted(
        source,
        1,
        declaration..declaration + name.len(),
        std::slice::from_ref(&(reference..reference + name.len())),
    );
    for (index, node) in tree.elements().iter().enumerate() {
        if node.kind == "REFERENCE_EXPRESSION"
            && node
                .parent
                .is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
        {
            assert_eq!(
                local_value_occurrences(
                    &directory,
                    &result,
                    LocalValueAnchor::ReferenceExpression(index),
                    current
                )
                .unwrap_err(),
                LocalBindingError::UnsupportedContext
            );
        }
    }
}

#[test]
fn parameter_ambiguity_and_unsupported_headers_never_publish_partial_bindings() {
    let _serial = functional::serial();
    for source in [
        "fun f(p: T, `p`: Q) { val use = p }",
        "fun f(p: T) { val p = 1; val `p` = 2; val use = p }",
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let lowered = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lowered, &result, current).unwrap();
        assert_eq!(
            local_value_occurrences(
                &directory,
                &result,
                LocalValueAnchor::Declaration(&lowered.declarations()[1]),
                current
            )
            .unwrap_err(),
            LocalBindingError::AmbiguousLocal,
            "{source}"
        );
    }
    for source in [
        "fun f(p) { val use = p }",
        "fun f(p: T = 1) { val use = p }",
        "fun f(vararg p: T) { val use = p }",
        "fun f(val p: T) { val use = p }",
        "fun f(@A p: T) { val use = p }",
        "fun f(p: pkg.T) { val use = p }",
        "fun f(p: T?) { val use = p }",
        "fun f(p: List<T>) { val use = p }",
        "fun f(p: () -> T) { val use = p }",
        "fun T.f(p: T) { val use = p }",
        "fun <T> f(p: T) { val use = p }",
        "fun f(p: T): T { val use = p }",
        "fun f(p: T) = p",
        "class C { fun f(p: T) { val use = p } }",
        "fun f(p: T, , q: Q) { val use = p }",
    ] {
        let result = parse(source);
        assert!(
            lower_declarations(&result, result.snapshot()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn rename_guard_preserves_original_partitions_and_refuses_capture() {
    use ktlsp_parser::local_value_binding::guarded_local_value_rename;
    let _serial = functional::serial();
    for (source, ordinal, name, expected) in [
        ("fun f() { val x = 1; val a = x }", 1, "fresh", true),
        ("fun f() { val unused = 1 }", 1, "fresh", true),
        ("fun f() { val x = 1; val y = 2 }", 1, "y", false),
        ("fun f() { val y = 2; val x = 1 }", 2, "y", false),
        ("fun f(y: T) { val x = 1 }", 2, "y", false),
        ("fun f() { val x = 1; val a = `y` }", 1, "y", false),
        (
            "fun f() { val before = y; val x = 1; val a = x }",
            2,
            "y",
            true,
        ),
        ("fun f() { val x = y; val a = x }", 1, "y", true),
        ("fun f() { val x = 1; val `y` = 2 }", 1, "y", false),
        ("fun f(p: T) { val x = p }", 1, "fresh", false),
        (
            "fun f(p: T) { val before = p; val `p` = p; val after = p }",
            3,
            "p",
            true,
        ),
        (
            "fun f(p: T) { val before = p; val `p` = p; val after = p }",
            3,
            "fresh",
            true,
        ),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let anchor = LocalValueAnchor::Declaration(&lower.declarations()[ordinal]);
        let found = guarded_local_value_rename(&directory, &result, anchor, current, Some(name));
        assert_eq!(found.is_ok(), expected, "{source}: {found:?}");
        if let Ok(found) = found {
            if let Some(site) = found.occurrences().first() {
                let by_use = guarded_local_value_rename(
                    &directory,
                    &result,
                    LocalValueAnchor::ReferenceExpression(site.expression_index()),
                    current,
                    Some(name),
                )
                .unwrap();
                assert!(std::ptr::eq(by_use.target(), found.target()));
                assert_eq!(
                    by_use
                        .occurrences()
                        .iter()
                        .map(|s| s.range())
                        .collect::<Vec<_>>(),
                    found
                        .occurrences()
                        .iter()
                        .map(|s| s.range())
                        .collect::<Vec<_>>()
                );
            }
            let mut edited = source.to_owned();
            let mut ranges = vec![found.target().name_range()];
            ranges.extend(found.occurrences().iter().map(|s| s.range()));
            let original_count = ranges.len();
            for range in ranges.into_iter().rev() {
                edited.replace_range(range, name);
            }
            let reparsed = parse(&edited);
            let new_lower = lower_declarations(&reparsed, reparsed.snapshot()).unwrap();
            let new_directory = build_regions(&new_lower, &reparsed, reparsed.snapshot()).unwrap();
            let rebound = guarded_local_value_rename(
                &new_directory,
                &reparsed,
                LocalValueAnchor::Declaration(&new_lower.declarations()[ordinal]),
                reparsed.snapshot(),
                None,
            )
            .unwrap();
            assert_eq!(rebound.occurrences().len() + 1, original_count);
            assert_eq!(rebound.target().semantic_spelling(), Some(name));
        }
    }
}

#[test]
fn rename_guard_authenticates_origin_snapshot_and_resource() {
    use ktlsp_parser::local_value_binding::guarded_local_value_rename;
    let _serial = functional::serial();
    let source = "fun f() { val x = 1; val a = x }";
    let result = parse(source);
    let current = result.snapshot();
    let lower = lower_declarations(&result, current).unwrap();
    let foreign = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lower, &result, current).unwrap();
    assert_eq!(
        guarded_local_value_rename(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&foreign.declarations()[1]),
            current,
            Some("fresh")
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    let stale = SourceSnapshot::new("local-occurrences", 8, FileKind::Source, source).unwrap();
    assert_eq!(
        guarded_local_value_rename(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&lower.declarations()[1]),
            &stale,
            Some("fresh")
        )
        .unwrap_err(),
        LocalBindingError::StaleSnapshot
    );
    // Valid shape, but its validation alone cannot fit the one semantic budget.
    let huge = "a".repeat(ktlsp_parser::MAX_SOURCE_BYTES);
    assert_eq!(
        guarded_local_value_rename(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&lower.declarations()[1]),
            current,
            Some(&huge)
        )
        .unwrap_err(),
        LocalBindingError::Resource
    );
}

#[test]
fn rename_quote_removal_refuses_keyword_token_merging_before_preparation() {
    use ktlsp_parser::local_value_binding::guarded_local_value_rename;
    let _serial = functional::serial();
    for (source, safe) in [
        ("fun f(){val`x`=1;val a=`x`}", false),
        ("fun f(){val `x`=1;val a=`x`}", true),
        ("fun f(){val/*comment*/`x`=1;val a=`x`}", true),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let original = local_value_occurrences(
            &directory,
            &result,
            LocalValueAnchor::Declaration(&lower.declarations()[1]),
            current,
        )
        .unwrap();
        assert_eq!(original.occurrences().len(), 1);
        for anchor in [
            LocalValueAnchor::Declaration(original.target()),
            LocalValueAnchor::ReferenceExpression(original.occurrences()[0].expression_index()),
        ] {
            for name in [None, Some("x"), Some("fresh")] {
                let guarded =
                    guarded_local_value_rename(&directory, &result, anchor, current, name);
                if !safe {
                    assert_eq!(guarded.unwrap_err(), LocalBindingError::UnsupportedContext);
                } else {
                    let guarded = guarded.unwrap();
                    if let Some(name) = name {
                        let mut edited = source.to_owned();
                        for range in guarded
                            .occurrences()
                            .iter()
                            .map(|s| s.range())
                            .rev()
                            .chain(std::iter::once(guarded.target().name_range()))
                        {
                            edited.replace_range(range, name);
                        }
                        let reparsed = parse(&edited);
                        let lower = lower_declarations(&reparsed, reparsed.snapshot()).unwrap();
                        let directory =
                            build_regions(&lower, &reparsed, reparsed.snapshot()).unwrap();
                        let rebound = guarded_local_value_rename(
                            &directory,
                            &reparsed,
                            LocalValueAnchor::Declaration(&lower.declarations()[1]),
                            reparsed.snapshot(),
                            None,
                        )
                        .unwrap();
                        assert_eq!(rebound.occurrences().len(), 1);
                        assert_eq!(rebound.target().semantic_spelling(), Some(name));
                    }
                }
            }
        }
    }
}
