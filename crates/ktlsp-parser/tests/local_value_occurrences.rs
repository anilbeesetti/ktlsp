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
    parse_revision(source, 7)
}

fn parse_revision(source: &str, revision: i64) -> SnapshotResult {
    let snapshot =
        SourceSnapshot::new("local-occurrences", revision, FileKind::Source, source).unwrap();
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
    accepted("fun f() { val x = 1; fun nested() {} }\n", 1, 14..15, &[]);
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
            "fun f() { val x = 1; fun nested() { val hidden = 1 } }\n",
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
fn ordinary_generic_parameter_is_a_real_value_binding() {
    let _serial = functional::serial();
    let source = "fun <T> f(p: T) { val use = p }";
    let parameter = source.find("p:").unwrap();
    let read = source.rfind("= p").unwrap() + 2;
    accepted(
        source,
        2,
        parameter..parameter + 1,
        std::slice::from_ref(&(read..read + 1)),
    );
    let source = "fun <T> f(p: T) { val before = p; val p = p; val after = p }";
    let parameter = source.find("p:").unwrap();
    let local = source.find("val p =").unwrap() + 4;
    let uses: Vec<_> = source
        .match_indices("= p")
        .map(|(offset, _)| offset + 2..offset + 3)
        .collect();
    accepted(source, 2, parameter..parameter + 1, &uses[..2]);
    accepted(source, 4, local..local + 1, &uses[2..]);
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
    // Simple returns lower structurally but do not authorize value semantics.
    let result = parse("fun f(p: T): T { val use = p }");
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
        LocalBindingError::UnsupportedContext
    );
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
        "fun f(p: T): T? { val use = p }",
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
fn lexical_type_parameters_borrow_original_annotations_targets_and_owners() {
    use ktlsp_parser::local_value_binding::bind_lexical_type_parameter;
    let _serial = functional::serial();
    // Bind real annotations to original lexical declarations, without freezing
    // producer indices or confusing inherited regions with type visibility.
    for source in [
        "import elsewhere.T\nimport elsewhere.Other as T\nclass T {}\nfun <T> f(p: T) { val x: T = p; val use = x }",
        "fun <T, U> f(p: U) { val x: T = p; val use = x }\nfun <T> g(p: T) { val x: T = p }",
        "fun <`T`> f(T: T) { val x: `T` = T; val use = x }",
        "// 😀\r\nfun /* header */ < /* before */ `🦀`, // list\r\n U, /* trailing */ > f(p: `🦀`) {\r\n val x: U = p; val use = x\r\n}\r\n",
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        assert!(tree.diagnostics().is_empty(), "{:?}", tree.diagnostics());
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        for (index, reference) in tree.elements().iter().enumerate().filter(|(_, node)| {
            node.kind == "REFERENCE_EXPRESSION"
                && node
                    .parent
                    .is_some_and(|parent| tree.elements()[parent].kind == "USER_TYPE")
        }) {
            let binding = bind_lexical_type_parameter(&directory, &result, index, current).unwrap();
            let annotated = binding.annotated();
            let function = binding.function();
            let function_start = function.range().start;
            let header_end = source[function_start..].find('>').unwrap() + function_start;
            let spelling = &source[reference.start..reference.end];
            let unquoted = spelling.trim_matches('`');
            let target_start = source[function_start..header_end]
                .find(if unquoted == "🦀" {
                    "`🦀`"
                } else if source[function_start..header_end].contains("`T`") {
                    "`T`"
                } else {
                    unquoted
                })
                .unwrap()
                + function_start;
            let target = lower
                .declarations()
                .iter()
                .find(|d| d.name_range().start == target_start)
                .unwrap();
            assert!(std::ptr::eq(binding.target(), target));
            assert!(std::ptr::eq(binding.reference(), reference));
            assert!(std::ptr::eq(
                binding.name_token(),
                &tree.elements()[index + 1]
            ));
            assert_eq!(target.kind(), "TYPE_PARAMETER");
            assert_eq!(target.cst_parent_kind(), "TYPE_PARAMETER_LIST");
            assert_eq!(target.owner(), annotated.owner());
            assert!(std::ptr::eq(
                &lower.declarations()[target.owner().unwrap()],
                function,
            ));
            assert_eq!(
                tree.elements()[target.cst_parent_index().unwrap()].parent,
                Some(function.cst_index())
            );
            assert_eq!(binding.body().introducer(), annotated.owner());
            assert_eq!(
                target.raw_name().as_ptr(),
                tree.source()[target.name_range()].as_ptr()
            );
            assert_eq!(
                ktlsp_parser::lowering::reference_site(&lower, &result, current, index)
                    .unwrap_err(),
                ktlsp_parser::lowering::ReferenceSiteError::UnsupportedContext,
            );
            assert_eq!(
                local_value_occurrences(
                    &directory,
                    &result,
                    LocalValueAnchor::Declaration(target),
                    current
                )
                .unwrap_err(),
                LocalBindingError::UnsupportedContext,
            );
            let stale =
                SourceSnapshot::new(current.document(), 8, FileKind::Source, source).unwrap();
            assert_eq!(
                bind_lexical_type_parameter(&directory, &result, index, &stale).unwrap_err(),
                LocalBindingError::StaleSnapshot,
            );
        }
        for (index, _) in tree.elements().iter().enumerate().filter(|(_, node)| {
            node.kind == "REFERENCE_EXPRESSION"
                && node
                    .parent
                    .is_some_and(|parent| tree.elements()[parent].kind == "PROPERTY")
        }) {
            let site = reference_site(&lower, &result, current, index).unwrap();
            let binding = bind_local_value(&directory, &site, current).unwrap();
            assert!(matches!(
                binding.target().kind(),
                "PROPERTY" | "VALUE_PARAMETER"
            ));
            assert_eq!(
                ktlsp_parser::integer_initializer_types::infer_integer_initializer(&binding, current).unwrap_err(),
                ktlsp_parser::integer_initializer_types::IntegerInitializerError::UnsupportedContext,
            );
        }
    }
}

#[test]
fn lexical_type_parameter_duplicates_no_match_foreign_and_value_roles_refuse() {
    use ktlsp_parser::local_value_binding::{bind_lexical_type_parameter, visible_local_values};
    let _serial = functional::serial();
    for (source, error) in [
        (
            "fun <T, `T`> f(p: T) { val x: T = p }",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T, U, `T`> f(p: T) {}",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "class T {}\nfun <U> f(p: T) {}",
            LocalBindingError::NoEligibleTypeParameter,
        ),
        (
            "fun <T> f() {}\nfun <U> g(p: T) {}",
            LocalBindingError::NoEligibleTypeParameter,
        ),
        (
            "class T {}\nfun f(p: T) {}",
            LocalBindingError::NoEligibleTypeParameter,
        ),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        for (index, _) in tree
            .elements()
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == "REFERENCE_EXPRESSION")
        {
            assert_eq!(
                bind_lexical_type_parameter(&directory, &result, index, current).unwrap_err(),
                if tree.elements()[tree.elements()[index].parent.unwrap()].kind == "USER_TYPE" {
                    error
                } else {
                    LocalBindingError::UnsupportedContext
                },
                "{source}",
            );
        }
    }
    let source = "fun <T, U> f(T: T) { val x: U = T; val use = x }";
    let result = parse(source);
    let foreign = parse(source);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    let lower = lower_declarations(&result, current).unwrap();
    let directory = build_regions(&lower, &result, current).unwrap();
    let annotation = tree
        .elements()
        .iter()
        .position(|node| node.kind == "REFERENCE_EXPRESSION")
        .unwrap();
    assert_eq!(
        bind_lexical_type_parameter(&directory, &foreign, annotation, foreign.snapshot())
            .unwrap_err(),
        LocalBindingError::InvalidOrigin,
    );
    let value = tree
        .elements()
        .iter()
        .enumerate()
        .find(|(_, node)| {
            node.kind == "REFERENCE_EXPRESSION" && node.start == source.rfind("= x").unwrap() + 2
        })
        .unwrap()
        .0;
    let visible = visible_local_values(&directory, &result, value, current).unwrap();
    assert_eq!(
        visible
            .values()
            .iter()
            .map(|d| (d.kind(), d.raw_name()))
            .collect::<Vec<_>>(),
        [("VALUE_PARAMETER", "T"), ("PROPERTY", "x")]
    );
    assert_eq!(
        bind_lexical_type_parameter(&directory, &result, value, current).unwrap_err(),
        LocalBindingError::UnsupportedContext,
    );
    for index in [usize::MAX, tree.elements().len(), annotation + 1] {
        assert!(bind_lexical_type_parameter(&directory, &result, index, current).is_err());
    }
}

#[test]
fn generic_headers_require_complete_restricted_syntax_even_without_value_reads() {
    let _serial = functional::serial();
    for source in ["fun <T> f() {}", "fun <T, U,> f() { val x = 1 }"] {
        let result = parse(source);
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        assert_eq!(lower.declarations()[1].kind(), "TYPE_PARAMETER");
    }
    for source in [
        "fun outer() { fun <T> f() {} }",
        "fun outer() { fun <T> f(p:T) {} }",
    ] {
        use ktlsp_parser::local_value_binding::{TypeParameterAnchor, type_parameter_occurrences};
        let result = parse(source);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        assert!(tree.diagnostics().is_empty());
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        let local = &lower.declarations()[1];
        let target = &lower.declarations()[2];
        assert_eq!(
            (local.kind(), local.raw_name(), local.owner()),
            ("FUN", "f", Some(0))
        );
        assert_eq!(
            (target.kind(), target.raw_name(), target.owner()),
            ("TYPE_PARAMETER", "T", Some(1))
        );
        assert_eq!(
            directory.memberships()[2].region(),
            directory.memberships()[1].region()
        );
        let enclosing = &directory.regions()[directory.memberships()[1].region()];
        assert_eq!(enclosing.introducer(), Some(0));
        assert_eq!(local.cst_parent_index(), Some(enclosing.cst_index()));
        let found = type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(target),
            snapshot,
        )
        .unwrap();
        assert!(std::ptr::eq(found.target(), target));
        assert!(std::ptr::eq(found.function(), local));
        assert_eq!(found.body().introducer(), Some(1));
        assert_eq!(
            found.body().parent(),
            Some(directory.memberships()[1].region())
        );
        assert_eq!(
            found.occurrences().len(),
            usize::from(source.contains("p:T"))
        );
        for annotation in found.occurrences() {
            let by_use = type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(annotation.reference_index()),
                snapshot,
            )
            .unwrap();
            assert!(std::ptr::eq(by_use.target(), target));
        }
    }
    for source in [
        "fun <> f() {}",
        "fun <T",
        "fun <T> () {}",
        "fun <T, , U> f() {}",
        "fun <T U> f() {}",
        "fun <T,,> f() {}",
        "fun <T: Any> f() {}",
        "fun <out T> f() {}",
        "fun <in T> f() {}",
        "fun <reified T> f() {}",
        "fun <@A T> f() {}",
        "inline fun <T> f() {}",
        "fun <T> f() where T: Any {}",
        "fun f<T>() {}",
        "fun <T> f<U>() {}",
        "fun <T> T.f() {}",
        "context(T) fun <T> f() {}",
        "fun <T> f(): T? {}",
        "fun <T> f() = 1",
        "fun <T> f()",
        "fun <T> f() {",
        "fun <T> f() { val x: T }",
        "fun <T> f() { val x = }",
        "fun <T> f() { var x = 1 }",
        "class C { fun <T> f() {} }",
        "fun <T> f(p: T?) {}",
        "fun <T> f(p: pkg.T) {}",
        "fun <T> f() { val x: List<T> = 1 }",
        "fun <T> f() { val x: @A T = 1 }",
        "fun <T> f() { val x: (T) = 1 }",
        "fun <T> f() { val x: T & Any = 1 }",
    ] {
        let result = parse(source);
        assert!(
            lower_declarations(&result, result.snapshot()).is_err(),
            "{source}"
        );
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

#[test]
fn local_empty_function_callees_preserve_original_identity_and_mixed_values() {
    use ktlsp_parser::local_value_binding::{
        bind_lexical_type_parameter, bind_local_function_callee, guarded_local_value_rename,
        visible_local_values,
    };
    let _serial = functional::serial();
    for source in [
        "fun outer(){ fun target(){}; target() }",
        "// 🦀\r\nimport elsewhere.target\r\nfun target() {}\r\nfun <T> outer(p: T){ fun `target`(){}; val x=1; target(); val y=x; val z=p; val unused=2 }",
        "fun outer(){ fun `🦀`(){}; `🦀`() }",
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        assert!(tree.diagnostics().is_empty());
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let (callee_index, callee) = tree
            .elements()
            .iter()
            .enumerate()
            .find(|(_, e)| {
                e.kind == "REFERENCE_EXPRESSION"
                    && e.parent
                        .is_some_and(|p| tree.elements()[p].kind == "CALL_EXPRESSION")
            })
            .unwrap();
        let binding =
            bind_local_function_callee(&directory, &result, callee_index, current).unwrap();
        let target = lower
            .declarations()
            .iter()
            .find(|d| d.kind() == "FUN" && d.cst_parent_kind() == "BLOCK")
            .unwrap();
        assert!(std::ptr::eq(binding.target(), target));
        assert!(std::ptr::eq(binding.callee(), callee));
        assert_eq!(binding.target().owner(), binding.body().introducer());
        assert_eq!(
            binding.target().cst_parent_index(),
            Some(binding.body().cst_index())
        );
        assert_eq!(
            binding.function().cst_index(),
            tree.elements()[binding.body().cst_index()].parent.unwrap()
        );
        assert_eq!(
            tree.elements()[callee.parent.unwrap()].parent,
            Some(binding.body().cst_index())
        );
        assert_eq!(
            target.raw_name().as_ptr(),
            tree.source()[target.name_range()].as_ptr()
        );
        assert!(reference_site(&lower, &result, current, callee_index).is_err());
        let stale = SourceSnapshot::new(
            current.document(),
            current.revision() + 1,
            FileKind::Source,
            source,
        )
        .unwrap();
        assert_eq!(
            bind_local_function_callee(&directory, &result, callee_index, &stale).unwrap_err(),
            LocalBindingError::StaleSnapshot
        );
        for index in [
            usize::MAX,
            tree.elements().len(),
            callee_index + 1,
            target.cst_index(),
        ] {
            assert!(bind_local_function_callee(&directory, &result, index, current).is_err());
        }
        if !source.contains("val x") {
            continue;
        }
        let x = lower
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "x")
            .unwrap();
        let p = lower
            .declarations()
            .iter()
            .find(|d| d.kind() == "VALUE_PARAMETER")
            .unwrap();
        let y_use = tree
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == source.find("=x").unwrap() + 1
            })
            .unwrap();
        let site = reference_site(&lower, &result, current, y_use).unwrap();
        let value = bind_local_value(&directory, &site, current).unwrap();
        assert!(std::ptr::eq(value.target(), x));
        assert_eq!(
            ktlsp_parser::integer_initializer_types::infer_integer_initializer(&value, current)
                .unwrap()
                .default_type(),
            ktlsp_parser::integer_initializer_types::IntegerType::Int
        );
        let visible = visible_local_values(&directory, &result, y_use, current).unwrap();
        assert_eq!(
            visible
                .values()
                .iter()
                .map(|d| d.raw_name())
                .collect::<Vec<_>>(),
            ["p", "x"]
        );
        for (target, expected) in [
            (x, vec![source.find("=x").unwrap() + 1]),
            (p, vec![source.find("=p").unwrap() + 1]),
        ] {
            let uses = local_value_occurrences(
                &directory,
                &result,
                LocalValueAnchor::Declaration(target),
                current,
            )
            .unwrap();
            assert_eq!(
                uses.occurrences()
                    .iter()
                    .map(|s| s.range().start)
                    .collect::<Vec<_>>(),
                expected
            );
        }
        let unused = lower
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "unused")
            .unwrap();
        assert!(
            local_value_occurrences(
                &directory,
                &result,
                LocalValueAnchor::Declaration(unused),
                current
            )
            .unwrap()
            .occurrences()
            .is_empty()
        );
        let annotation = tree
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == source.find("p: T").unwrap() + 3
            })
            .unwrap();
        assert_eq!(
            bind_lexical_type_parameter(&directory, &result, annotation, current)
                .unwrap()
                .target()
                .raw_name(),
            "T"
        );
        assert_eq!(
            guarded_local_value_rename(
                &directory,
                &result,
                LocalValueAnchor::Declaration(x),
                current,
                Some("target")
            )
            .unwrap_err(),
            LocalBindingError::AmbiguousLocal
        );
        let rename = guarded_local_value_rename(
            &directory,
            &result,
            LocalValueAnchor::Declaration(x),
            current,
            Some("fresh"),
        )
        .unwrap();
        let mut edited = source.to_owned();
        for range in std::iter::once(x.name_range())
            .chain(rename.occurrences().iter().map(|s| s.range()))
            .rev()
        {
            edited.replace_range(range, "fresh");
        }
        let updated = parse(&edited);
        assert_eq!(
            bind_local_function_callee(&directory, &updated, callee_index, updated.snapshot())
                .unwrap_err(),
            LocalBindingError::InvalidOrigin
        );
        let lower2 = lower_declarations(&updated, updated.snapshot()).unwrap();
        let directory2 = build_regions(&lower2, &updated, updated.snapshot()).unwrap();
        let tree2 = updated.tree_for(updated.snapshot()).unwrap();
        let callee2 = tree2
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == edited.rfind("target()").unwrap()
            })
            .unwrap();
        let rebound =
            bind_local_function_callee(&directory2, &updated, callee2, updated.snapshot()).unwrap();
        assert_eq!(rebound.target().range(), target.range());
        assert_eq!(rebound.target().name_range(), target.name_range());
        assert_eq!(rebound.target().owner(), target.owner());
        let fresh_use = tree2
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == edited.find("=fresh").unwrap() + 1
            })
            .unwrap();
        let fresh_site = reference_site(&lower2, &updated, updated.snapshot(), fresh_use).unwrap();
        assert_eq!(
            bind_local_value(&directory2, &fresh_site, updated.snapshot())
                .unwrap()
                .target()
                .raw_name(),
            "fresh"
        );
    }
}

#[test]
fn local_callee_competitors_shapes_and_unproved_calls_refuse_atomically() {
    use ktlsp_parser::local_value_binding::bind_local_function_callee;
    let _serial = functional::serial();
    for source in [
        "fun outer(){ target(); fun target(){} }",
        "fun outer(){ fun target(){}; fun `target`(){}; target() }",
        "fun outer(){ fun target(){}; fun target(x: Int){}; target() }",
        "fun outer(){ fun target(){}; fun target(x: Int=1){}; target() }",
        "fun outer(){ fun target(){}; fun <T> target(){}; target() }",
        "fun outer(){ fun target(){}; class target; target() }",
        "fun outer(){ fun target(){}; val `target`=1; target() }",
        "fun outer(target: Int){ fun target(){}; target() }",
        "fun outer(){ fun target(){ target() }; target() }",
        "fun outer(){ fun target(){ val capture=1 }; target() }",
        "fun outer(){ fun target(){}; target(1) }",
        "fun outer(){ fun target(){}; target<Int>() }",
        "fun outer(){ fun target(){}; target(){} }",
        "fun outer(){ fun target(){}; target()() }",
        "fun outer(){ fun target(){}; (target)() }",
        "fun outer(){ fun target(){}; object.target() }",
        "fun outer(){ fun target(){}; target(); unknown() }",
        "fun outer(){ fun target(){}; val x=1; x(); val y=x }",
        "fun outer(){ fun `🦀`(){}; val `🦀`=1; `🦀`() }",
        "// 🦀\r\nfun outer(){ fun `target`(){}; fun target(){}; target() }\r\n",
        "fun outer(){ fun target(){}; val é=1; target() }",
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let Ok(lower) = lower_declarations(&result, current) else {
            continue;
        };
        let Ok(directory) = build_regions(&lower, &result, current) else {
            continue;
        };
        let tree = result.tree_for(current).unwrap();
        for (index, _) in tree
            .elements()
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
        {
            assert!(
                bind_local_function_callee(&directory, &result, index, current).is_err(),
                "{source}"
            );
        }
        for target in lower
            .declarations()
            .iter()
            .filter(|d| d.kind() == "PROPERTY")
        {
            assert!(
                local_value_occurrences(
                    &directory,
                    &result,
                    LocalValueAnchor::Declaration(target),
                    current
                )
                .is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn mixed_callees_keep_parameter_shadow_and_self_initializer_partitions() {
    let _serial = functional::serial();
    let source = "fun f(p: Int){ fun target(){}; val before=p; val p=p; target(); val after=p; val unused=1 }";
    let parameter = source.find("p:").unwrap();
    let local = source.find("val p").unwrap() + 4;
    let before = source.find("before=p").unwrap() + 7;
    let after = source.find("after=p").unwrap() + 6;
    accepted(
        source,
        1,
        parameter..parameter + 1,
        &[before..before + 1, local + 2..local + 3],
    );
    accepted(
        source,
        4,
        local..local + 1,
        std::slice::from_ref(&(after..after + 1)),
    );
}

#[test]
fn mixed_rename_refuses_future_function_names_and_unresolved_value_capture() {
    use ktlsp_parser::local_value_binding::guarded_local_value_rename;
    let _serial = functional::serial();
    for (source, name) in [
        ("fun f(){ val x=1; val use=x; fun later(){} }", "later"),
        (
            "fun f(){ fun target(){}; val x=1; target(); val use=external }",
            "external",
        ),
    ] {
        let result = parse(source);
        let current = result.snapshot();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let x = lower
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "x")
            .unwrap();
        assert!(
            guarded_local_value_rename(
                &directory,
                &result,
                LocalValueAnchor::Declaration(x),
                current,
                Some(name)
            )
            .is_err()
        );
    }
}

#[test]
fn type_parameter_occurrences_preserve_original_type_value_and_callee_partitions() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, bind_lexical_type_parameter, bind_local_function_callee,
        guarded_type_parameter_rename, type_parameter_occurrences,
    };
    let _serial = functional::serial();
    let original = "// 😀\r\nimport elsewhere.T\r\nclass T {}\r\nfun <T, Spare> f(T: T, U: Int) {\r\n fun Fresh(){}; Fresh()\r\n val before=T; val T:T=T; val after:T=T; val n:Int=U\r\n}\r\nfun <T> g(p:T) { val other:T=p }\r\n";
    for new_name in ["Fresh", "U", "out", "reified"] {
        let result = parse(original);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let type_start = original.find("<T").unwrap() + 1;
        let target = lower
            .declarations()
            .iter()
            .find(|d| d.name_range().start == type_start)
            .unwrap();
        let found = type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(target),
            current,
        )
        .unwrap();
        let expected = [
            original.find("T: T").unwrap() + 3,
            original.find("val T:T").unwrap() + 6,
            original.find("after:T").unwrap() + 6,
        ];
        assert!(std::ptr::eq(found.target(), target));
        assert!(std::ptr::eq(found.directory(), &directory));
        assert!(std::ptr::eq(
            found.function(),
            &lower.declarations()[target.owner().unwrap()]
        ));
        assert_eq!(found.body().introducer(), target.owner());
        assert_eq!(target.cst_parent_kind(), "TYPE_PARAMETER_LIST");
        assert_eq!(
            tree.elements()[target.cst_parent_index().unwrap()].parent,
            Some(found.function().cst_index())
        );
        assert_eq!(
            found
                .occurrences()
                .iter()
                .map(|a| a.range())
                .collect::<Vec<_>>(),
            expected.map(|start| start..start + 1)
        );
        for annotation in found.occurrences() {
            assert!(std::ptr::eq(
                annotation.reference(),
                &tree.elements()[annotation.reference_index()]
            ));
            assert!(std::ptr::eq(
                annotation.name_token(),
                &tree.elements()[annotation.reference_index() + 1]
            ));
            assert_eq!(annotation.annotated().owner(), target.owner());
            let by_use = type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(annotation.reference_index()),
                current,
            )
            .unwrap();
            assert!(std::ptr::eq(by_use.target(), target));
            assert_eq!(
                by_use
                    .occurrences()
                    .iter()
                    .map(|a| a.range())
                    .collect::<Vec<_>>(),
                expected.map(|start| start..start + 1)
            );
        }
        let spare = lower
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "Spare")
            .unwrap();
        let unused = type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(spare),
            current,
        )
        .unwrap();
        assert!(unused.occurrences().is_empty());
        for declaration in [target, spare] {
            assert_eq!(
                guarded_type_parameter_rename(
                    &directory,
                    &result,
                    TypeParameterAnchor::Declaration(declaration),
                    current,
                    Some("Int")
                )
                .unwrap_err(),
                LocalBindingError::AmbiguousTypeParameter
            );
        }
        let renamed = guarded_type_parameter_rename(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(target),
            current,
            Some(new_name),
        )
        .unwrap();
        let mut edited = original.to_owned();
        for range in std::iter::once(target.name_range())
            .chain(renamed.occurrences().iter().map(|a| a.range()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            edited.replace_range(range, new_name);
        }
        let edited_result = parse_revision(&edited, 8);
        for (phase, actual) in [("original", original), ("edited", edited.as_str())] {
            let rebound = if phase == "original" {
                &result
            } else {
                &edited_result
            };
            let snapshot = rebound.snapshot();
            let tree = rebound.tree_for(snapshot).unwrap();
            assert!(
                tree.diagnostics().is_empty(),
                "{actual}: {:?}",
                tree.diagnostics()
            );
            let lower = lower_declarations(rebound, snapshot).unwrap();
            let directory = build_regions(&lower, rebound, snapshot).unwrap();
            let ty = lower
                .declarations()
                .iter()
                .find(|d| d.name_range().start == type_start)
                .unwrap();
            let mut type_names = Vec::new();
            let mut values = Vec::new();
            let mut callees = Vec::new();
            for (index, node) in tree
                .elements()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
            {
                let parent = &tree.elements()[node.parent.unwrap()];
                if !matches!(
                    parent.kind.as_str(),
                    "USER_TYPE" | "CALL_EXPRESSION" | "PROPERTY"
                ) {
                    continue; // Imports are not same-function lexical occurrences.
                }
                if parent.kind == "USER_TYPE" {
                    let bound = bind_lexical_type_parameter(&directory, rebound, index, snapshot);
                    let spelling = &actual[node.start..node.end];
                    type_names.push(spelling);
                    if spelling == "Int" {
                        assert_eq!(
                            bound.unwrap_err(),
                            LocalBindingError::NoEligibleTypeParameter
                        );
                    } else {
                        let bound = bound.unwrap();
                        if bound.function().cst_index()
                            == ty
                                .owner()
                                .map(|i| lower.declarations()[i].cst_index())
                                .unwrap()
                        {
                            assert!(std::ptr::eq(bound.target(), ty));
                            assert_eq!(
                                bound.target().raw_name(),
                                if phase == "original" { "T" } else { new_name }
                            );
                        } else {
                            assert_eq!(bound.target().raw_name(), "T");
                            assert!(!std::ptr::eq(bound.target(), ty));
                        }
                    }
                } else if parent.kind == "CALL_EXPRESSION" {
                    let bound = bind_local_function_callee(&directory, rebound, index, snapshot);
                    let bound = bound.unwrap();
                    assert_eq!(bound.target().kind(), "FUN");
                    assert_eq!(bound.target().raw_name(), "Fresh");
                    callees.push((bound.function().raw_name(), bound.target().raw_name()));
                } else {
                    let site = reference_site(&lower, rebound, snapshot, index).unwrap();
                    let bound = bind_local_value(&directory, &site, snapshot);
                    let bound = bound.unwrap();
                    assert_eq!(
                        &actual[site.range()],
                        if site.containing_property().raw_name() == "n" {
                            "U"
                        } else if site.containing_property().raw_name() == "other" {
                            "p"
                        } else {
                            "T"
                        }
                    );
                    let ordinal = lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, bound.target()))
                        .unwrap();
                    let value_target = &lower.declarations()[ordinal];
                    values.push((
                        &actual[site.containing_property().name_range()],
                        value_target.kind(),
                        value_target.raw_name(),
                        ordinal,
                    ));
                }
            }
            let name = if phase == "original" { "T" } else { new_name };
            assert_eq!(type_names, [name, "Int", name, name, "Int", "T", "T"]);
            assert_eq!(callees, [("f", "Fresh")]);
            let parameter = lower
                .declarations()
                .iter()
                .position(|d| d.kind() == "VALUE_PARAMETER" && d.raw_name() == "T")
                .unwrap();
            let local = lower
                .declarations()
                .iter()
                .position(|d| d.kind() == "PROPERTY" && d.raw_name() == "T")
                .unwrap();
            assert_eq!(
                values.iter().map(|v| (v.0, v.1, v.2)).collect::<Vec<_>>(),
                [
                    ("before", "VALUE_PARAMETER", "T"),
                    ("T", "VALUE_PARAMETER", "T"),
                    ("after", "PROPERTY", "T"),
                    ("n", "VALUE_PARAMETER", "U"),
                    ("other", "VALUE_PARAMETER", "p")
                ]
            );
            assert_eq!(
                [values[0].3, values[1].3, values[2].3],
                [parameter, parameter, local]
            );
        }
    }
}

#[test]
fn type_parameter_rename_refuses_captures_duplicates_contextual_outputs_and_foreign_origins() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, guarded_type_parameter_rename, type_parameter_occurrences,
    };
    let _serial = functional::serial();
    for (source, name, error) in [
        (
            "fun <T, `T`> f() {}",
            "Fresh",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T, U, `T`> f(p:T) {}",
            "Fresh",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T, `U`> f(p:T) {}",
            "U",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T> f(p:U) {}",
            "U",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T> f() { val x:U=1 }",
            "U",
            LocalBindingError::AmbiguousTypeParameter,
        ),
        (
            "fun <T> f(p:T) {}",
            "where",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <T> f(p:T) {}",
            "dynamic",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <T> f(p:T) {}",
            "suspend",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <`where`> f(p:`where`) {}",
            "where",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <`dynamic`> f(p:`dynamic`) {}",
            "dynamic",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <`suspend`> f(p:`suspend`) {}",
            "suspend",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <T> f(p:T) {}",
            "___",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <T> f(p:T) {}",
            "`Fresh`",
            LocalBindingError::UnsupportedSpelling,
        ),
        (
            "fun <T> f(p:T) {}",
            "🦀",
            LocalBindingError::UnsupportedSpelling,
        ),
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        let target = lower
            .declarations()
            .iter()
            .find(|d| d.kind() == "TYPE_PARAMETER")
            .unwrap();
        assert_eq!(
            guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                snapshot,
                Some(name)
            )
            .unwrap_err(),
            error,
            "{source} -> {name}"
        );
    }
    let source = "// 😀\r\nfun <`🦀`, Unused> f(p:`🦀`) { val x:`🦀`=p }\r\n";
    let result = parse(source);
    let snapshot = result.snapshot();
    let lower = lower_declarations(&result, snapshot).unwrap();
    let directory = build_regions(&lower, &result, snapshot).unwrap();
    let target = &lower.declarations()[1];
    let found = guarded_type_parameter_rename(
        &directory,
        &result,
        TypeParameterAnchor::Declaration(target),
        snapshot,
        Some("Fresh"),
    )
    .unwrap();
    assert_eq!(found.target().semantic_spelling(), Some("🦀"));
    assert_eq!(found.occurrences().len(), 2);
    let foreign_result = parse(source);
    let foreign_lower = lower_declarations(&result, snapshot).unwrap();
    assert_eq!(
        type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(&foreign_lower.declarations()[1]),
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    assert_eq!(
        type_parameter_occurrences(
            &directory,
            &foreign_result,
            TypeParameterAnchor::Declaration(target),
            foreign_result.snapshot()
        )
        .unwrap_err(),
        LocalBindingError::InvalidOrigin
    );
    for stale in [
        SourceSnapshot::new(snapshot.document(), 8, FileKind::Source, source).unwrap(),
        SourceSnapshot::new("foreign", 7, FileKind::Source, source).unwrap(),
        SourceSnapshot::new(
            snapshot.document(),
            7,
            FileKind::Source,
            source.replace("Unused", "Spare"),
        )
        .unwrap(),
    ] {
        assert_eq!(
            type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                &stale
            )
            .unwrap_err(),
            LocalBindingError::StaleSnapshot
        );
    }
    let tree = result.tree_for(snapshot).unwrap();
    for (index, e) in tree
        .elements()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
    {
        if tree.elements()[e.parent.unwrap()].kind != "USER_TYPE" {
            assert_eq!(
                type_parameter_occurrences(
                    &directory,
                    &result,
                    TypeParameterAnchor::AnnotationReference(index),
                    snapshot
                )
                .unwrap_err(),
                LocalBindingError::UnsupportedContext
            );
        }
    }
    for index in [usize::MAX, tree.elements().len(), 0] {
        assert!(
            type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(index),
                snapshot
            )
            .is_err()
        );
    }
    assert_eq!(
        type_parameter_occurrences(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(&lower.declarations()[3]),
            snapshot
        )
        .unwrap_err(),
        LocalBindingError::UnsupportedContext
    );
}

#[test]
fn local_parameter_annotations_share_one_complete_original_outer_type_partition() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, bind_lexical_type_parameter, bind_local_function_callee,
        guarded_type_parameter_rename, type_parameter_occurrences,
    };
    let _serial = functional::serial();
    for original in [
        "fun <T> outer(){ fun target(p:T){} }",
        "fun <T, Spare> outer(){ fun target(p:T){} }",
        "// 😀\r\nfun <T, Spare> outer(T:T){ fun before(p:T,q:Int){}; val x:T=T; unknown(noSuch=x,noSuch=2L); fun after(p:`T`){}; before(); val y=x }\r\n",
        "fun <Int, Spare> outer(){ fun before(p:Int){}; unknown(label=1); fun after(p:`Int`){} }",
        "fun <`🦀`, Spare> outer(){ fun before(p:`🦀`){}; fun after(p:`🦀`){} }",
    ] {
        for new_name in ["Fresh", "out", "reified"] {
            let result = parse(original);
            let snapshot = result.snapshot();
            let tree = result.tree_for(snapshot).unwrap();
            assert!(tree.diagnostics().is_empty());
            let lower = lower_declarations(&result, snapshot).unwrap();
            let directory = build_regions(&lower, &result, snapshot).unwrap();
            let target = &lower.declarations()[1];
            let found = type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                snapshot,
            )
            .unwrap();
            let expected: Vec<_> = tree
                .elements()
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.kind == "REFERENCE_EXPRESSION"
                        && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
                        && original[e.start..e.end].trim_matches('`')
                            == target.semantic_spelling().unwrap()
                })
                .map(|(i, _)| i)
                .collect();
            assert_eq!(
                found
                    .occurrences()
                    .iter()
                    .map(|a| a.reference_index())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(std::ptr::eq(found.target(), target));
            let outer = &lower.declarations()[target.owner().unwrap()];
            assert!(std::ptr::eq(found.function(), outer));
            for annotation in found.occurrences() {
                let bound = bind_lexical_type_parameter(
                    &directory,
                    &result,
                    annotation.reference_index(),
                    snapshot,
                )
                .unwrap();
                assert!(std::ptr::eq(bound.target(), target));
                assert!(std::ptr::eq(bound.function(), outer));
                assert!(std::ptr::eq(bound.body(), found.body()));
                assert!(std::ptr::eq(
                    bound.reference(),
                    &tree.elements()[annotation.reference_index()]
                ));
                assert!(std::ptr::eq(
                    bound.name_token(),
                    &tree.elements()[annotation.reference_index() + 1]
                ));
                assert!(std::ptr::eq(bound.annotated(), annotation.annotated()));
                let owner_ordinal = annotation.annotated().owner().unwrap();
                let owner = &lower.declarations()[owner_ordinal];
                if !std::ptr::eq(owner, outer) {
                    assert_eq!(owner.owner(), target.owner());
                    assert_eq!(
                        tree.elements()[owner.cst_index()].parent,
                        Some(found.body().cst_index())
                    );
                    let parameter_ordinal = lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, annotation.annotated()))
                        .unwrap();
                    let local_ordinal = lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, owner))
                        .unwrap();
                    assert_eq!(
                        directory.memberships()[parameter_ordinal].region(),
                        directory.memberships()[local_ordinal].region()
                    );
                    assert_ne!(parameter_ordinal, local_ordinal);
                }
            }
            let spare = lower
                .declarations()
                .iter()
                .find(|d| d.raw_name() == "Spare");
            let mut anchors = vec![TypeParameterAnchor::Declaration(target)];
            anchors.extend(
                expected
                    .iter()
                    .copied()
                    .map(TypeParameterAnchor::AnnotationReference),
            );
            for anchor in anchors {
                for partition in [
                    type_parameter_occurrences(&directory, &result, anchor, snapshot).unwrap(),
                    guarded_type_parameter_rename(&directory, &result, anchor, snapshot, None)
                        .unwrap(),
                    guarded_type_parameter_rename(
                        &directory,
                        &result,
                        anchor,
                        snapshot,
                        Some(new_name),
                    )
                    .unwrap(),
                ] {
                    assert!(std::ptr::eq(partition.target(), target));
                    assert_eq!(
                        partition
                            .occurrences()
                            .iter()
                            .map(|a| a.reference_index())
                            .collect::<Vec<_>>(),
                        expected
                    );
                }
            }
            if let Some(spare) = spare {
                assert!(
                    type_parameter_occurrences(
                        &directory,
                        &result,
                        TypeParameterAnchor::Declaration(spare),
                        snapshot
                    )
                    .unwrap()
                    .occurrences()
                    .is_empty()
                );
            }
            if original.contains("q:Int") {
                for selected in std::iter::once(target).chain(spare) {
                    assert_eq!(
                        guarded_type_parameter_rename(
                            &directory,
                            &result,
                            TypeParameterAnchor::Declaration(selected),
                            snapshot,
                            Some("Int")
                        )
                        .unwrap_err(),
                        LocalBindingError::AmbiguousTypeParameter
                    );
                }
            }
            for (i, element) in tree
                .elements()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
            {
                let parent = &tree.elements()[element.parent.unwrap()];
                if parent.kind == "USER_TYPE"
                    && &original[element.start..element.end] == "Int"
                    && target.raw_name() != "Int"
                {
                    assert_eq!(
                        bind_lexical_type_parameter(&directory, &result, i, snapshot).unwrap_err(),
                        LocalBindingError::NoEligibleTypeParameter
                    );
                } else if parent.kind == "CALL_EXPRESSION" {
                    assert!(bind_local_function_callee(&directory, &result, i, snapshot).is_err());
                } else if parent.kind == "PROPERTY" {
                    let site = reference_site(&lower, &result, snapshot, i).unwrap();
                    assert!(bind_local_value(&directory, &site, snapshot).is_err());
                    assert!(
                        local_value_occurrences(
                            &directory,
                            &result,
                            LocalValueAnchor::ReferenceExpression(i),
                            snapshot
                        )
                        .is_err()
                    );
                }
            }
            let renamed = guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                snapshot,
                Some(new_name),
            )
            .unwrap();
            let mut edited = original.to_owned();
            for range in std::iter::once(target.name_range())
                .chain(renamed.occurrences().iter().map(|a| a.range()))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                edited.replace_range(range, new_name);
            }
            let edited_result = parse_revision(&edited, 8);
            let edited_snapshot = edited_result.snapshot();
            let edited_tree = edited_result.tree_for(edited_snapshot).unwrap();
            assert!(edited_tree.diagnostics().is_empty());
            let edited_lower = lower_declarations(&edited_result, edited_snapshot).unwrap();
            let edited_directory =
                build_regions(&edited_lower, &edited_result, edited_snapshot).unwrap();
            let edited_target = &edited_lower.declarations()[1];
            let rebound = type_parameter_occurrences(
                &edited_directory,
                &edited_result,
                TypeParameterAnchor::Declaration(edited_target),
                edited_snapshot,
            )
            .unwrap();
            assert_eq!(rebound.occurrences().len(), expected.len());
            assert_eq!(
                edited_lower.declarations().len(),
                lower.declarations().len()
            );
            for (old, new) in lower.declarations().iter().zip(edited_lower.declarations()) {
                assert_eq!((old.kind(), old.owner()), (new.kind(), new.owner()));
                if !std::ptr::eq(old, target) {
                    assert_eq!(old.raw_name(), new.raw_name());
                }
            }
            for annotation in rebound.occurrences() {
                let bound = bind_lexical_type_parameter(
                    &edited_directory,
                    &edited_result,
                    annotation.reference_index(),
                    edited_snapshot,
                )
                .unwrap();
                assert!(std::ptr::eq(bound.target(), edited_target));
            }
            let mut original_call_roles = Vec::new();
            for (phase_result, phase_lower) in [(&result, &lower), (&edited_result, &edited_lower)]
            {
                let phase_snapshot = phase_result.snapshot();
                let phase_tree = phase_result.tree_for(phase_snapshot).unwrap();
                let mut roles = Vec::new();
                for (i, e) in phase_tree
                    .elements()
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.kind == "CALL_EXPRESSION")
                {
                    let call = ktlsp_parser::call_syntax::call_site(
                        phase_lower,
                        phase_result,
                        phase_snapshot,
                        i,
                    )
                    .unwrap();
                    assert!(std::ptr::eq(call.call(), e));
                    roles.push((
                        phase_tree.source()[call.callee().start..call.callee().end].to_owned(),
                        call.arguments()
                            .iter()
                            .map(|a| {
                                (
                                    a.ordinal(),
                                    a.label().map(|l| {
                                        phase_tree.source()[l.name().start..l.name().end].to_owned()
                                    }),
                                    a.expression().kind.clone(),
                                    phase_tree.source()[a.value_token().start..a.value_token().end]
                                        .to_owned(),
                                    a.comma().is_some(),
                                )
                            })
                            .collect::<Vec<_>>(),
                    ));
                }
                if std::ptr::eq(phase_result, &result) {
                    original_call_roles = roles;
                } else {
                    assert_eq!(roles, original_call_roles);
                }
                for function in phase_lower
                    .declarations()
                    .iter()
                    .filter(|d| d.kind() == "FUN" && d.owner().is_some())
                {
                    let parameters = ktlsp_parser::call_syntax::callable_parameters(
                        phase_lower,
                        phase_result,
                        phase_snapshot,
                        function,
                    )
                    .unwrap();
                    assert!(std::ptr::eq(parameters.function(), function));
                    for (ordinal, p) in parameters.parameters().iter().enumerate() {
                        assert_eq!(p.ordinal(), ordinal);
                        assert_eq!(
                            p.declaration().owner(),
                            phase_lower
                                .declarations()
                                .iter()
                                .position(|d| std::ptr::eq(d, function))
                        );
                    }
                }
            }
            for (i, e) in tree.elements().iter().enumerate().filter(|(_, e)| {
                e.kind == "REFERENCE_EXPRESSION"
                    && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
            }) {
                let changed = &edited_tree.elements()[i];
                if !expected.contains(&i) {
                    assert_eq!(
                        &original[e.start..e.end],
                        &edited[changed.start..changed.end]
                    );
                    assert_eq!(
                        bind_lexical_type_parameter(
                            &edited_directory,
                            &edited_result,
                            i,
                            edited_snapshot
                        )
                        .unwrap_err(),
                        LocalBindingError::NoEligibleTypeParameter
                    );
                }
            }
            let foreign_result = parse(original);
            let foreign_lower = lower_declarations(&result, snapshot).unwrap();
            for (result, anchor, current, error) in [
                (
                    &foreign_result,
                    TypeParameterAnchor::Declaration(target),
                    foreign_result.snapshot(),
                    LocalBindingError::InvalidOrigin,
                ),
                (
                    &result,
                    TypeParameterAnchor::Declaration(&foreign_lower.declarations()[1]),
                    snapshot,
                    LocalBindingError::InvalidOrigin,
                ),
                (
                    &result,
                    TypeParameterAnchor::Declaration(target),
                    edited_snapshot,
                    LocalBindingError::StaleSnapshot,
                ),
            ] {
                assert_eq!(
                    type_parameter_occurrences(&directory, result, anchor, current).unwrap_err(),
                    error
                );
            }
        }
    }
}

#[test]
fn local_type_namespace_refuses_the_entire_unsupported_envelope_and_annotation_captures() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, bind_lexical_type_parameter, guarded_type_parameter_rename,
        type_parameter_occurrences,
    };
    let _serial = functional::serial();
    for source in [
        "fun <T> outer(){ class Other{}; fun target(p:T){} }",
        "fun <T> outer(){ fun target(p:T){}; class Other{} }",
        "fun <T> outer(){ fun middle(){ fun <U> unused(){ class Other{} } }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun unused(p:T?){ } }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun unused(){ if(true){} } }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun unused(){ val x={1} } }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun unused(){ var x=1 } }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun <U:Any> unused(){} }; fun target(p:T){} }",
        "fun <T> outer(){ fun middle(){ fun unused():T?{} }; fun target(p:T){} }",
        "fun <T> outer(){ fun target(p:T?){} }",
        "fun <T> outer(){ fun target(p:pkg.T){} }",
        "fun <T> outer(){ fun target(p:List<T>){} }",
        "fun <T> outer(){ fun target(p:()->T){} }",
        "fun <T> outer(){ fun target(p:@A T){} }",
        "fun <T> outer(){ fun target(p:){} }",
        "fun <T> outer(){ fun target(p:T=1){} }",
        "fun <T> outer(){ fun T.target(p:T){} }",
        "fun <T> outer(){ fun target(p:T):pkg.T{} }",
        "fun <T:Any> outer(){ fun target(p:T){} }",
        "fun <T> outer(){ context(T) fun target(p:T){} }",
        "class C { fun <T> outer(){ fun target(p:T){} } }",
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        if let Ok(lower) = lower_declarations(&result, snapshot)
            && let Ok(directory) = build_regions(&lower, &result, snapshot)
        {
            for target in lower
                .declarations()
                .iter()
                .filter(|d| d.kind() == "TYPE_PARAMETER")
            {
                assert!(
                    type_parameter_occurrences(
                        &directory,
                        &result,
                        TypeParameterAnchor::Declaration(target),
                        snapshot
                    )
                    .is_err(),
                    "{source}"
                );
                assert!(
                    guarded_type_parameter_rename(
                        &directory,
                        &result,
                        TypeParameterAnchor::Declaration(target),
                        snapshot,
                        Some("Fresh")
                    )
                    .is_err(),
                    "{source}"
                );
            }
            for (i, e) in result
                .tree_for(snapshot)
                .unwrap()
                .elements()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
            {
                let _ = e;
                assert!(
                    bind_lexical_type_parameter(&directory, &result, i, snapshot).is_err(),
                    "{source}"
                );
            }
        }
    }
    for source in [
        "fun <T, Spare> outer(){ fun before(p:T){}; fun unused(q:Captured){} }",
        "fun <T, Spare> outer(){ fun unused(q:`Captured`){}; fun after(p:T){} }",
        "fun <T, `T`, Spare> outer(){ fun target(p:T){} }",
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        let target = &lower.declarations()[1];
        assert_eq!(
            guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                snapshot,
                Some("Captured")
            )
            .unwrap_err(),
            LocalBindingError::AmbiguousTypeParameter
        );
        if let Some(spare) = lower
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "Spare")
            && !source.contains("`T`")
        {
            assert_eq!(
                guarded_type_parameter_rename(
                    &directory,
                    &result,
                    TypeParameterAnchor::Declaration(spare),
                    snapshot,
                    Some("Captured")
                )
                .unwrap_err(),
                LocalBindingError::AmbiguousTypeParameter
            );
        }
    }
}

#[test]
fn documentation_operation_cannot_authorize_lexical_type_consumers() {
    use ktlsp_parser::{
        documentation,
        local_value_binding::{
            TypeParameterAnchor, bind_lexical_type_parameter, guarded_type_parameter_rename,
            type_parameter_occurrences,
        },
    };
    let _serial = functional::serial();
    for (source, strict_control) in [
        ("fun <T> f(){ /** doc */ val x:T=1 }", false),
        ("fun <T> f(){ /** doc */ final val x:T=1 }", false),
        ("fun <T> f(){ val x:T=1 }", true),
        ("fun <T> f():T{ val x=1 }", true),
        ("fun <T> f():T{ /** doc */ val x=1 }", false),
        (
            "fun <T> f(){ fun middle(){ fun <U> deep(){ val x:T=1 } } }",
            true,
        ),
        (
            "fun <T> f(){ fun middle(){ fun <U> deep(){ /** doc */ val x:T=1 } } }",
            false,
        ),
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        let cancelled = || false;
        let mut budget = documentation::Budget::new(&cancelled);
        let lowered = documentation::lower(&result, snapshot, &mut budget).unwrap();
        let property = lowered
            .declarations()
            .iter()
            .find(|d| d.raw_name() == "x")
            .unwrap();
        let attached = documentation::select(
            &lowered,
            &result,
            snapshot,
            property.name_range().start,
            &mut budget,
        )
        .unwrap();
        assert!(std::ptr::eq(attached.declaration(), property));
        let directory = build_regions(&lowered, &result, snapshot).unwrap();
        let target = lowered
            .declarations()
            .iter()
            .find(|d| d.kind() == "TYPE_PARAMETER")
            .unwrap();
        let reference = tree
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION"
                    && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
            })
            .unwrap();
        assert_eq!(
            bind_lexical_type_parameter(&directory, &result, reference, snapshot).unwrap_err(),
            LocalBindingError::UnsupportedContext,
        );
        for anchor in [
            TypeParameterAnchor::Declaration(target),
            TypeParameterAnchor::AnnotationReference(reference),
        ] {
            assert_eq!(
                type_parameter_occurrences(&directory, &result, anchor, snapshot).unwrap_err(),
                LocalBindingError::UnsupportedContext,
            );
            for name in [None, Some("Fresh")] {
                assert_eq!(
                    guarded_type_parameter_rename(&directory, &result, anchor, snapshot, name)
                        .unwrap_err(),
                    LocalBindingError::UnsupportedContext,
                );
            }
        }
        if strict_control {
            let strict = lower_declarations(&result, snapshot).unwrap();
            let directory = build_regions(&strict, &result, snapshot).unwrap();
            let bound =
                bind_lexical_type_parameter(&directory, &result, reference, snapshot).unwrap();
            let target = strict
                .declarations()
                .iter()
                .find(|d| d.kind() == "TYPE_PARAMETER")
                .unwrap();
            assert!(std::ptr::eq(bound.target(), target));
            let found = guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(reference),
                snapshot,
                Some("Fresh"),
            )
            .unwrap();
            assert!(std::ptr::eq(found.target(), bound.target()));
            assert_eq!(found.occurrences().len(), 1);
            assert_eq!(found.occurrences()[0].reference_index(), reference);
        }
    }
}

#[test]
fn recursive_type_scopes_preserve_complete_original_identity_partitions_and_replay() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, bind_lexical_type_parameter, guarded_type_parameter_rename,
        type_parameter_occurrences,
    };
    let _serial = functional::serial();
    let ordinary = "// 😀\r\nfun <T, Spare> outer(root:T):T{ val before:T=root; fun <U> inner(a:`T`,b:U):T{ val inherited:T=a; fun middle(c:T):T{ fun <`T`> deep(d:T,e:U,f:Missing):T{ val own:T=d; unknown(noSuch=d); }; fun side(g:T):Missing{} }; fun <U> peer(h:U,i:T):U{} }; fun <T> shadow(j:T):T{}; fun after(k:T):T{}; val last:T=root }\r\nfun <T> other(z:T):T{}\r\nfun <Unused> returnOnly():Unused{}\r\nfun unmatched():Missing{}\r\n";
    for original in [
        ordinary.to_owned(),
        ordinary
            .replace("`T`", "`🦀`")
            .replace("<T", "<`🦀`")
            .replace(":T", ":`🦀`"),
    ] {
        let result = parse(&original);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        assert!(tree.diagnostics().is_empty());
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        for (ordinal, declaration) in lower.declarations().iter().enumerate() {
            let membership = directory.memberships()[ordinal].region();
            if declaration.kind() == "FUN" {
                let body = directory
                    .regions()
                    .iter()
                    .find(|r| r.introducer() == Some(ordinal))
                    .unwrap();
                assert_eq!(body.parent(), Some(membership));
                if let Some(owner) = declaration.owner() {
                    assert_eq!(directory.regions()[membership].introducer(), Some(owner));
                    assert_eq!(
                        declaration.cst_parent_index(),
                        Some(directory.regions()[membership].cst_index())
                    );
                }
            } else if matches!(declaration.kind(), "VALUE_PARAMETER" | "TYPE_PARAMETER") {
                assert_eq!(
                    membership,
                    directory.memberships()[declaration.owner().unwrap()].region()
                );
            } else if declaration.kind() == "PROPERTY" {
                assert_eq!(
                    directory.regions()[membership].introducer(),
                    declaration.owner()
                );
            }
        }
        let expected = [
            ("root", Some("outer")),
            ("outer", Some("outer")),
            ("before", Some("outer")),
            ("a", Some("outer")),
            ("b", Some("inner")),
            ("inner", Some("outer")),
            ("inherited", Some("outer")),
            ("c", Some("outer")),
            ("middle", Some("outer")),
            ("d", Some("deep")),
            ("e", Some("inner")),
            ("f", None),
            ("deep", Some("deep")),
            ("own", Some("deep")),
            ("g", Some("outer")),
            ("side", None),
            ("h", Some("peer")),
            ("i", Some("outer")),
            ("peer", Some("peer")),
            ("j", Some("shadow")),
            ("shadow", Some("shadow")),
            ("k", Some("outer")),
            ("after", Some("outer")),
            ("last", Some("outer")),
            ("z", Some("other")),
            ("other", Some("other")),
            ("returnOnly", Some("returnOnly")),
            ("unmatched", None),
        ];
        let annotations: Vec<_> = tree
            .elements()
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.kind == "REFERENCE_EXPRESSION"
                    && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(annotations.len(), expected.len());
        let mut original_targets = Vec::new();
        for (&index, &(annotated, owner)) in annotations.iter().zip(&expected) {
            let binding = bind_lexical_type_parameter(&directory, &result, index, snapshot);
            let target = if let Some(owner) = owner {
                let binding = binding.unwrap();
                assert_eq!(binding.annotated().raw_name(), annotated);
                assert_eq!(binding.function().raw_name(), owner);
                assert_eq!(
                    binding.target().owner(),
                    lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, binding.function()))
                );
                assert_eq!(binding.body().introducer(), binding.target().owner());
                if binding.annotated().kind() == "FUN" {
                    let annotated_ordinal = lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, binding.annotated()))
                        .unwrap();
                    let own_body = directory
                        .regions()
                        .iter()
                        .find(|r| r.introducer() == Some(annotated_ordinal))
                        .unwrap();
                    assert!(binding.reference().end <= own_body.range().start);
                    assert_eq!(
                        directory.memberships()[annotated_ordinal].region(),
                        own_body.parent().unwrap()
                    );
                }
                assert!(std::ptr::eq(binding.reference(), &tree.elements()[index]));
                Some(
                    lower
                        .declarations()
                        .iter()
                        .position(|d| std::ptr::eq(d, binding.target()))
                        .unwrap(),
                )
            } else {
                assert_eq!(
                    binding.unwrap_err(),
                    LocalBindingError::NoEligibleTypeParameter
                );
                None
            };
            original_targets.push(target);
        }
        let foreign = parse(&original);
        let foreign_lower = lower_declarations(&result, snapshot).unwrap();
        for (ordinal, target) in lower
            .declarations()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind() == "TYPE_PARAMETER")
        {
            let partition: Vec<_> = annotations
                .iter()
                .zip(&original_targets)
                .filter(|(_, t)| **t == Some(ordinal))
                .map(|(i, _)| *i)
                .collect();
            let anchors: Vec<_> = std::iter::once(TypeParameterAnchor::Declaration(target))
                .chain(
                    partition
                        .iter()
                        .copied()
                        .map(TypeParameterAnchor::AnnotationReference),
                )
                .collect();
            for anchor in anchors {
                for found in [
                    type_parameter_occurrences(&directory, &result, anchor, snapshot).unwrap(),
                    guarded_type_parameter_rename(&directory, &result, anchor, snapshot, None)
                        .unwrap(),
                    guarded_type_parameter_rename(
                        &directory,
                        &result,
                        anchor,
                        snapshot,
                        Some("Fresh"),
                    )
                    .unwrap(),
                ] {
                    assert!(std::ptr::eq(found.target(), target));
                    assert_eq!(
                        found
                            .occurrences()
                            .iter()
                            .map(|a| a.reference_index())
                            .collect::<Vec<_>>(),
                        partition
                    );
                }
                assert_eq!(
                    type_parameter_occurrences(&directory, &foreign, anchor, foreign.snapshot())
                        .unwrap_err(),
                    LocalBindingError::InvalidOrigin
                );
                let stale = SourceSnapshot::new(
                    "local-occurrences",
                    8,
                    FileKind::Source,
                    original.as_str(),
                )
                .unwrap();
                assert_eq!(
                    type_parameter_occurrences(&directory, &result, anchor, &stale).unwrap_err(),
                    LocalBindingError::StaleSnapshot
                );
            }
            assert_eq!(
                type_parameter_occurrences(
                    &directory,
                    &result,
                    TypeParameterAnchor::Declaration(&foreign_lower.declarations()[ordinal]),
                    snapshot
                )
                .unwrap_err(),
                LocalBindingError::InvalidOrigin
            );
            let renamed = guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(target),
                snapshot,
                Some("Fresh"),
            )
            .unwrap();
            let mut edited = original.clone();
            for range in std::iter::once(target.name_range())
                .chain(renamed.occurrences().iter().map(|a| a.range()))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                edited.replace_range(range, "Fresh");
            }
            let rebound = parse_revision(&edited, 8);
            let rebound_snapshot = rebound.snapshot();
            let rebound_tree = rebound.tree_for(rebound_snapshot).unwrap();
            assert!(rebound_tree.diagnostics().is_empty());
            let rebound_lower = lower_declarations(&rebound, rebound_snapshot).unwrap();
            let rebound_directory =
                build_regions(&rebound_lower, &rebound, rebound_snapshot).unwrap();
            assert_eq!(
                lower.declarations().len(),
                rebound_lower.declarations().len()
            );
            for (i, (old, new)) in lower
                .declarations()
                .iter()
                .zip(rebound_lower.declarations())
                .enumerate()
            {
                assert_eq!((old.kind(), old.owner()), (new.kind(), new.owner()));
                assert_eq!(
                    new.raw_name(),
                    if i == ordinal {
                        "Fresh"
                    } else {
                        old.raw_name()
                    }
                );
            }
            for (&index, &expected_target) in annotations.iter().zip(&original_targets) {
                let binding = bind_lexical_type_parameter(
                    &rebound_directory,
                    &rebound,
                    index,
                    rebound_snapshot,
                );
                if let Some(expected_target) = expected_target {
                    assert!(std::ptr::eq(
                        binding.unwrap().target(),
                        &rebound_lower.declarations()[expected_target]
                    ));
                } else {
                    assert_eq!(
                        binding.unwrap_err(),
                        LocalBindingError::NoEligibleTypeParameter
                    );
                }
                let old = &tree.elements()[index];
                let new = &rebound_tree.elements()[index];
                assert_eq!(
                    &edited[new.start..new.end],
                    if expected_target == Some(ordinal) {
                        "Fresh"
                    } else {
                        &original[old.start..old.end]
                    }
                );
            }
        }
        // Broader lowering is not authority for existing value/callee semantics.
        for (i, e) in tree
            .elements()
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == "REFERENCE_EXPRESSION")
        {
            if tree.elements()[e.parent.unwrap()].kind == "CALL_EXPRESSION" {
                assert!(
                    ktlsp_parser::local_value_binding::bind_local_function_callee(
                        &directory, &result, i, snapshot
                    )
                    .is_err()
                );
            }
        }
        for function in lower
            .declarations()
            .iter()
            .filter(|d| d.kind() == "FUN" && matches!(d.raw_name(), "inner" | "deep" | "middle"))
        {
            assert!(
                ktlsp_parser::call_syntax::callable_parameters(&lower, &result, snapshot, function)
                    .is_err()
            );
        }
    }
}

#[test]
fn recursive_type_rename_relooks_up_all_annotations_and_preserves_sibling_shadowing() {
    use ktlsp_parser::local_value_binding::{TypeParameterAnchor, guarded_type_parameter_rename};
    let _serial = functional::serial();
    for (source, function, name, accepted) in [
        (
            "fun <T> outer():T{ fun <U> inner():T{} }",
            "outer",
            "U",
            false,
        ),
        (
            "fun <T> outer():T{ fun <U> inner():T{} }",
            "inner",
            "T",
            false,
        ),
        (
            "fun <T> outer(){ fun middle(){ fun unused():Captured{} } }",
            "outer",
            "Captured",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(){ fun unused():Captured{} } }",
            "inner",
            "Captured",
            false,
        ),
        (
            "fun <T> outer():T{ fun <U> inner():U{}; fun sibling():T{} }",
            "outer",
            "U",
            true,
        ),
        (
            "fun <T> outer():T{ fun <U> inner():U{}; fun sibling():T{} }",
            "inner",
            "T",
            true,
        ),
        (
            "fun <T> outer(){ fun <U,V> inner():U{} }",
            "inner",
            "V",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(a:T){} }",
            "outer",
            "U",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(a:T){} }",
            "inner",
            "T",
            false,
        ),
        (
            "fun <T> outer(){ fun middle(){ fun unused(a:Captured){} } }",
            "outer",
            "Captured",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(){ fun unused(a:Captured){} } }",
            "inner",
            "Captured",
            false,
        ),
        (
            "fun <T> outer(){ fun <U> inner(a:U){}; fun sibling(b:T){} }",
            "outer",
            "U",
            true,
        ),
        (
            "fun <T> outer(){ fun <U> inner(a:U){}; fun sibling(b:T){} }",
            "inner",
            "T",
            true,
        ),
        (
            "fun <T> outer(){ fun middle(){ fun <U,`U`> unused(){} } }",
            "outer",
            "Fresh",
            false,
        ),
        (
            "fun <T> outer(){ fun <U,V> inner(){} }",
            "inner",
            "V",
            false,
        ),
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        let owner = lower
            .declarations()
            .iter()
            .position(|d| d.kind() == "FUN" && d.raw_name() == function)
            .unwrap();
        let target = lower
            .declarations()
            .iter()
            .find(|d| d.kind() == "TYPE_PARAMETER" && d.owner() == Some(owner))
            .unwrap();
        let renamed = guarded_type_parameter_rename(
            &directory,
            &result,
            TypeParameterAnchor::Declaration(target),
            snapshot,
            Some(name),
        );
        if accepted {
            renamed.unwrap();
        } else {
            assert_eq!(
                renamed.unwrap_err(),
                LocalBindingError::AmbiguousTypeParameter,
                "{source}"
            );
        }
    }
}

#[test]
fn recursive_type_scope_depth_is_structural_not_a_fixed_function_lane() {
    use ktlsp_parser::local_value_binding::{
        TypeParameterAnchor, bind_lexical_type_parameter, type_parameter_occurrences,
    };
    let _serial = functional::serial();
    for depth in [3usize, 7, 15, 31] {
        let mut source = String::new();
        for level in 0..depth {
            write!(
                &mut source,
                "fun {}f{level}(p{level}:T){{ ",
                if level.is_multiple_of(3) { "<T> " } else { "" }
            )
            .unwrap();
        }
        source.push_str("val leaf:T=p0");
        source.push_str(&"}".repeat(depth));
        let result = parse(&source);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        assert!(tree.diagnostics().is_empty());
        let lower = lower_declarations(&result, snapshot).unwrap();
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        for (index, _) in tree.elements().iter().enumerate().filter(|(_, e)| {
            e.kind == "REFERENCE_EXPRESSION"
                && tree.elements()[e.parent.unwrap()].kind == "USER_TYPE"
        }) {
            let binding =
                bind_lexical_type_parameter(&directory, &result, index, snapshot).unwrap();
            let level = if binding.annotated().raw_name() == "leaf" {
                depth - 1
            } else {
                binding.annotated().raw_name()[1..]
                    .parse::<usize>()
                    .unwrap()
            };
            assert_eq!(
                binding.function().raw_name(),
                format!("f{}", level - level % 3)
            );
            let by_use = type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(index),
                snapshot,
            )
            .unwrap();
            let by_declaration = type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::Declaration(binding.target()),
                snapshot,
            )
            .unwrap();
            assert!(std::ptr::eq(by_use.target(), by_declaration.target()));
            assert_eq!(
                by_use
                    .occurrences()
                    .iter()
                    .map(|a| a.reference_index())
                    .collect::<Vec<_>>(),
                by_declaration
                    .occurrences()
                    .iter()
                    .map(|a| a.reference_index())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn return_only_headers_require_the_complete_original_ordered_finite_shape() {
    use ktlsp_parser::local_value_binding::bind_lexical_type_parameter;
    let _serial = functional::serial();
    for source in [
        "fun f():Missing{}",
        "fun f() /* header */ : `Missing` /* body */ {}",
    ] {
        let result = parse(source);
        let snapshot = result.snapshot();
        let tree = result.tree_for(snapshot).unwrap();
        assert!(tree.diagnostics().is_empty());
        let lower = lower_declarations(&result, snapshot).unwrap();
        assert_eq!(lower.declarations().len(), 1);
        let directory = build_regions(&lower, &result, snapshot).unwrap();
        let index = tree
            .elements()
            .iter()
            .position(|e| e.kind == "REFERENCE_EXPRESSION")
            .unwrap();
        let annotation = &tree.elements()[tree.elements()[index].parent.unwrap()];
        let header = &tree.elements()[annotation.parent.unwrap()];
        assert_eq!(header.parent, Some(lower.declarations()[0].cst_index()));
        let body = directory
            .regions()
            .iter()
            .find(|r| r.introducer() == Some(0))
            .unwrap();
        assert!(header.end <= body.range().start);
        assert_eq!(body.parent(), Some(directory.memberships()[0].region()));
        assert_eq!(
            bind_lexical_type_parameter(&directory, &result, index, snapshot).unwrap_err(),
            LocalBindingError::NoEligibleTypeParameter
        );
    }
    for source in [
        "fun f():{}",
        "fun f():T:T{}",
        "fun f():T?{}",
        "fun f():pkg.T{}",
        "fun f():List<T>{}",
        "fun f():(T){}",
        "fun f():()->T{}",
        "fun f():@A T{}",
        "fun f():T = 1",
        "fun f():T",
        "fun f():T where T:Any{}",
        "fun f():T{ val x= }",
        "fun f():T;{}",
        "fun f():T(){}",
        "fun f():T{ fun broken():List<T>{} }",
    ] {
        let result = parse(source);
        assert!(
            lower_declarations(&result, result.snapshot()).is_err(),
            "{source}"
        );
    }
}
