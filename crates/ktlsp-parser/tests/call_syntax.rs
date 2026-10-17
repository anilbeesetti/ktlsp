//! Hosted original-worker syntax/identity acceptance; never execute locally under #96.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    Element, FileKind, Parsed, Parser, SnapshotResult, SourceSnapshot,
    call_syntax::{call_site, callable_parameters},
    local_value_binding::{
        LocalBindingError, LocalValueAnchor, TypeParameterAnchor, bind_lexical_type_parameter,
        bind_local_function_callee, bind_local_value, guarded_local_value_rename,
        guarded_type_parameter_rename, local_value_occurrences, type_parameter_occurrences,
        visible_local_values,
    },
    lowering::{ReferenceSiteError, lower_declarations, reference_site},
    structural_regions::build_regions,
};
use std::sync::atomic::AtomicBool;

fn parse(source: &str, revision: i64) -> SnapshotResult {
    let snapshot =
        SourceSnapshot::new("original-call-syntax", revision, FileKind::Source, source).unwrap();
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap()
}
fn original(tree: &Parsed, node: &Element) -> usize {
    tree.elements()
        .iter()
        .position(|e| std::ptr::eq(e, node))
        .expect("original borrowed CST identity")
}
fn text<'a>(tree: &'a Parsed, node: &Element) -> &'a str {
    &tree.source()[node.start..node.end]
}
fn selected_call(tree: &Parsed) -> usize {
    tree.elements()
        .iter()
        .position(|e| e.kind == "CALL_EXPRESSION")
        .unwrap()
}
fn assert_tree(tree: &Parsed) {
    assert!(tree.diagnostics().is_empty());
    let mut offset = 0;
    for (i, e) in tree.elements().iter().enumerate() {
        assert!(tree.source().is_char_boundary(e.start));
        assert!(tree.source().is_char_boundary(e.end));
        assert!(e.subtree_end > i && e.subtree_end <= tree.elements().len());
        if e.token {
            assert_eq!(e.start, offset);
            assert!(e.end > e.start);
            assert_eq!(e.subtree_end, i + 1);
            offset = e.end;
        }
        if let Some(parent) = e.parent {
            assert!(parent < i && i < tree.elements()[parent].subtree_end);
            assert!(
                tree.elements()[parent].start <= e.start && e.end <= tree.elements()[parent].end
            );
        }
    }
    assert_eq!(offset, tree.source().len());
}

#[test]
fn original_call_roles_and_selected_parameter_order_are_distinct() {
    let _serial = functional::serial();
    for (source, values, labels, parameter_names) in [
        (
            "fun outer(){ target(1, 2L) }",
            vec!["1", "2L"],
            vec![None, None],
            vec![],
        ),
        (
            "fun outer(p:Int){ target(first=p, second=2L,) }",
            vec!["p", "2L"],
            vec![Some("first"), Some("second")],
            vec![],
        ),
        (
            "fun outer(){ fun target(first:Int, second:Long){}; target(second=2L, first=1) }",
            vec!["2L", "1"],
            vec![Some("second"), Some("first")],
            vec!["first", "second"],
        ),
        ("fun outer(){ missing() }", vec![], vec![], vec![]),
        ("fun outer(){ unknown(1,) }", vec!["1"], vec![None], vec![]),
        (
            "fun outer(){ fun target(x:Unknown, x:Int,){}; target(x=unresolved, x=2L, x=3) }",
            vec!["unresolved", "2L", "3"],
            vec![Some("x"), Some("x"), Some("x")],
            vec!["x", "x"],
        ),
        (
            "// 🦀\r\nfun outer(p:Int){\r\n fun `🦀`(`🦀`:T){};\r\n `🦀`(/*one*/ `🦀` /*two*/ = p, 2L, /*tail*/ )\r\n}",
            vec!["p", "2L"],
            vec![Some("`🦀`"), None],
            vec!["`🦀`"],
        ),
        (
            "fun outer(){\r fun target(){};\r target(1,2,3,)\r}",
            vec!["1", "2", "3"],
            vec![None, None, None],
            vec![],
        ),
    ] {
        let result = parse(source, 1);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        assert_tree(tree);
        assert_eq!(tree.source(), source);
        let lowered = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lowered, &result, current).unwrap();
        let index = selected_call(tree);
        let call = call_site(&lowered, &result, current, index).unwrap();
        assert!(std::ptr::eq(call.origin(), &lowered));
        assert_eq!(original(tree, call.call()), index);
        let callee_index = original(tree, call.callee());
        assert_eq!(call.callee_name().parent, Some(callee_index));
        assert_eq!(call.callee().parent, Some(index));
        assert_eq!(call.call().parent, Some(original(tree, call.body())));
        assert_eq!(call.body().parent, Some(call.function().cst_index()));
        assert!(
            lowered
                .declarations()
                .iter()
                .any(|d| std::ptr::eq(d, call.function()))
        );
        assert_eq!(call.argument_list().parent, Some(index));
        assert_eq!(
            call.open_paren().parent,
            Some(original(tree, call.argument_list()))
        );
        assert_eq!(
            call.close_paren().parent,
            Some(original(tree, call.argument_list()))
        );
        assert_eq!(text(tree, call.open_paren()), "(");
        assert_eq!(text(tree, call.close_paren()), ")");
        assert_eq!(call.arguments().len(), values.len());
        for (ordinal, ((argument, value), label)) in call
            .arguments()
            .iter()
            .zip(&values)
            .zip(&labels)
            .enumerate()
        {
            assert_eq!(argument.ordinal(), ordinal);
            assert_eq!(text(tree, argument.expression()), *value);
            assert_eq!(text(tree, argument.value_token()), *value);
            assert_eq!(
                original(tree, argument.value_token()),
                original(tree, argument.expression()) + 1
            );
            assert_eq!(
                argument.value_token().start..argument.value_token().end,
                argument.expression().start..argument.expression().end
            );
            assert_eq!(
                argument.expression().parent,
                Some(original(tree, argument.node()))
            );
            assert_eq!(
                argument.node().parent,
                Some(original(tree, call.argument_list()))
            );
            assert_eq!(argument.label().map(|l| text(tree, l.name())), *label);
            assert!(
                reference_site(
                    &lowered,
                    &result,
                    current,
                    original(tree, argument.expression())
                )
                .is_err()
            );
            if let Some(label) = argument.label() {
                assert_eq!(label.reference().parent, Some(original(tree, label.node())));
                assert_eq!(label.node().parent, Some(original(tree, argument.node())));
                assert_eq!(label.name().parent, Some(original(tree, label.reference())));
                assert_eq!(text(tree, label.equals()), "=");
                assert_eq!(label.equals().parent, Some(original(tree, argument.node())));
                assert_ne!(
                    original(tree, label.reference()),
                    original(tree, argument.expression())
                );
                assert!(
                    reference_site(
                        &lowered,
                        &result,
                        current,
                        original(tree, label.reference())
                    )
                    .is_err()
                );
            }
            if ordinal + 1 < call.arguments().len() {
                assert_eq!(text(tree, argument.comma().unwrap()), ",");
            }
            if let Some(comma) = argument.comma() {
                assert_eq!(text(tree, comma), ",");
                assert_eq!(comma.parent, Some(original(tree, call.argument_list())));
                assert!(argument.node().end <= comma.start);
            }
        }
        assert!(reference_site(&lowered, &result, current, callee_index).is_err());
        let selected = lowered
            .declarations()
            .iter()
            .find(|d| d.kind() == "FUN" && d.cst_parent_kind() == "BLOCK");
        let parameters = selected
            .map(|function| callable_parameters(&lowered, &result, current, function).unwrap());
        if let Some(parameters) = &parameters {
            assert!(std::ptr::eq(parameters.function(), selected.unwrap()));
            assert!(std::ptr::eq(parameters.origin(), &lowered));
            assert_eq!(
                parameters
                    .parameters()
                    .iter()
                    .map(|p| p.declaration().raw_name())
                    .collect::<Vec<_>>(),
                parameter_names
            );
            let function_ordinal = lowered
                .declarations()
                .iter()
                .position(|d| std::ptr::eq(d, parameters.function()))
                .unwrap();
            let outer_region = directory
                .regions()
                .iter()
                .position(|r| r.cst_index() == original(tree, call.body()))
                .unwrap();
            for (ordinal, parameter) in parameters.parameters().iter().enumerate() {
                let d = parameter.declaration();
                let declaration_ordinal = lowered
                    .declarations()
                    .iter()
                    .position(|original| std::ptr::eq(original, d))
                    .unwrap();
                assert_eq!(parameter.ordinal(), ordinal);
                assert_eq!(d.owner(), Some(function_ordinal));
                assert_eq!(
                    d.cst_parent_index(),
                    Some(original(tree, parameters.parameter_list()))
                );
                assert_eq!(d.cst_parent_kind(), "VALUE_PARAMETER_LIST");
                assert_eq!(
                    directory.memberships()[declaration_ordinal].region(),
                    outer_region
                );
                assert_eq!(parameter.annotation().parent, Some(d.cst_index()));
                assert!(d.range().end <= parameters.body().start);
                assert_eq!(
                    d.raw_name().as_ptr(),
                    tree.source()[d.name_range()].as_ptr()
                );
            }
        }
    }
}

#[test]
fn original_foreign_stale_and_wrong_selectors_refuse() {
    let _serial = functional::serial();
    let source = "fun outer(){ fun target(x:Int){}; target(x=1) }";
    let result = parse(source, 1);
    let duplicate = parse(source, 1);
    let current = result.snapshot();
    let tree = result.tree_for(current).unwrap();
    let lower = lower_declarations(&result, current).unwrap();
    let foreign = lower_declarations(&duplicate, duplicate.snapshot()).unwrap();
    let function = &lower.declarations()[1];
    let index = selected_call(tree);
    assert_eq!(
        call_site(&lower, &duplicate, duplicate.snapshot(), index).unwrap_err(),
        ReferenceSiteError::InvalidOrigin
    );
    assert_eq!(
        callable_parameters(&lower, &duplicate, duplicate.snapshot(), function).unwrap_err(),
        ReferenceSiteError::InvalidOrigin
    );
    assert_eq!(
        callable_parameters(&lower, &result, current, &foreign.declarations()[1]).unwrap_err(),
        ReferenceSiteError::InvalidOrigin
    );
    for (revision, changed) in [(2, source.to_owned()), (1, source.replace("x=1", "x=2"))] {
        let stale =
            SourceSnapshot::new(current.document(), revision, FileKind::Source, changed).unwrap();
        assert_eq!(
            call_site(&lower, &result, &stale, index).unwrap_err(),
            ReferenceSiteError::StaleSnapshot
        );
        assert_eq!(
            callable_parameters(&lower, &result, &stale, function).unwrap_err(),
            ReferenceSiteError::StaleSnapshot
        );
    }
    for selector in [
        usize::MAX,
        tree.elements().len(),
        index + 1,
        function.cst_index(),
        0,
    ] {
        assert_eq!(
            call_site(&lower, &result, current, selector).unwrap_err(),
            ReferenceSiteError::UnsupportedContext
        );
    }
    for d in lower
        .declarations()
        .iter()
        .filter(|d| !std::ptr::eq(*d, function))
    {
        assert_eq!(
            callable_parameters(&lower, &result, current, d).unwrap_err(),
            ReferenceSiteError::UnsupportedContext
        );
    }
}

#[test]
fn actual_edits_reparse_original_syntax_and_rebind_existing_empty_proof() {
    let _serial = functional::serial();
    let source = "fun outer(){ fun target(first:Int, second:Long){}; val x=1; target(second=2L, first=x); val y=x }";
    let result = parse(source, 1);
    let lower = lower_declarations(&result, result.snapshot()).unwrap();
    let tree = result.tree_for(result.snapshot()).unwrap();
    let old_call = call_site(&lower, &result, result.snapshot(), selected_call(tree)).unwrap();
    let old_parameters =
        callable_parameters(&lower, &result, result.snapshot(), &lower.declarations()[1]).unwrap();
    let mut edited = source.to_owned();
    // Actual original ranges, applied backwards: remove arguments/parameters to
    // enter the existing closed empty-call proof, then rename its callee+target.
    let edits = [
        (
            old_call.argument_list().start + 1..old_call.argument_list().end - 1,
            "",
        ),
        (
            old_call.callee_name().start..old_call.callee_name().end,
            "fresh",
        ),
        (
            old_parameters.parameter_list().start + 1..old_parameters.parameter_list().end - 1,
            "",
        ),
        (old_parameters.function().name_range(), "fresh"),
    ];
    for (range, replacement) in edits {
        edited.replace_range(range, replacement);
    }
    let updated = parse(&edited, 2);
    assert_eq!(
        call_site(&lower, &updated, updated.snapshot(), old_call.cst_index()).unwrap_err(),
        ReferenceSiteError::InvalidOrigin
    );
    let lower2 = lower_declarations(&updated, updated.snapshot()).unwrap();
    let directory = build_regions(&lower2, &updated, updated.snapshot()).unwrap();
    let tree2 = updated.tree_for(updated.snapshot()).unwrap();
    let call2 = call_site(&lower2, &updated, updated.snapshot(), selected_call(tree2)).unwrap();
    let binding = bind_local_function_callee(
        &directory,
        &updated,
        original(tree2, call2.callee()),
        updated.snapshot(),
    )
    .unwrap();
    assert_eq!(binding.target().raw_name(), "fresh");
    assert!(std::ptr::eq(binding.target(), &lower2.declarations()[1]));
    let use_index = tree2
        .elements()
        .iter()
        .position(|e| {
            e.kind == "REFERENCE_EXPRESSION" && e.start == edited.rfind("=x").unwrap() + 1
        })
        .unwrap();
    let site = reference_site(&lower2, &updated, updated.snapshot(), use_index).unwrap();
    assert_eq!(
        bind_local_value(&directory, &site, updated.snapshot())
            .unwrap()
            .target()
            .raw_name(),
        "x"
    );
    callable_parameters(&lower2, &updated, updated.snapshot(), binding.target()).unwrap();
}

#[test]
fn rich_syntax_admits_only_the_closed_type_namespace_not_value_or_callee_consumers() {
    let _serial = functional::serial();
    for extra in ["target(first=p, second=2L)", "target(1)", "target()"] {
        let source = format!(
            "fun <T> outer(p:T){{ fun target(first:Int, second:Long){{}}; val x=1; {extra}; val y=x; val z:T=p }}"
        );
        let result = parse(&source, 1);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let call = call_site(&lower, &result, current, selected_call(tree)).unwrap();
        callable_parameters(&lower, &result, current, &lower.declarations()[3]).unwrap();
        let use_index = tree
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == source.find("y=x").unwrap() + 2
            })
            .unwrap();
        let annotation = tree
            .elements()
            .iter()
            .position(|e| {
                e.kind == "REFERENCE_EXPRESSION" && e.start == source.find("p:T").unwrap() + 2
            })
            .unwrap();
        let site = reference_site(&lower, &result, current, use_index).unwrap();
        assert_eq!(
            bind_local_value(&directory, &site, current).unwrap_err(),
            LocalBindingError::UnsupportedContext
        );
        assert_eq!(
            bind_local_function_callee(&directory, &result, original(tree, call.callee()), current)
                .unwrap_err(),
            LocalBindingError::UnsupportedContext
        );
        assert_eq!(
            visible_local_values(&directory, &result, use_index, current).unwrap_err(),
            LocalBindingError::UnsupportedContext
        );
        assert_eq!(
            local_value_occurrences(
                &directory,
                &result,
                LocalValueAnchor::ReferenceExpression(use_index),
                current
            )
            .unwrap_err(),
            LocalBindingError::UnsupportedContext
        );
        assert_eq!(
            guarded_local_value_rename(
                &directory,
                &result,
                LocalValueAnchor::ReferenceExpression(use_index),
                current,
                Some("fresh")
            )
            .unwrap_err(),
            LocalBindingError::UnsupportedContext
        );
        assert_eq!(
            bind_lexical_type_parameter(&directory, &result, annotation, current)
                .unwrap()
                .target()
                .raw_name(),
            "T"
        );
        for found in [
            type_parameter_occurrences(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(annotation),
                current,
            )
            .unwrap(),
            guarded_type_parameter_rename(
                &directory,
                &result,
                TypeParameterAnchor::AnnotationReference(annotation),
                current,
                Some("Fresh"),
            )
            .unwrap(),
        ] {
            assert_eq!(found.target().raw_name(), "T");
            assert_eq!(found.occurrences().len(), 2);
        }
    }
}

#[test]
fn unsupported_calls_and_selected_function_headers_never_get_partial_views() {
    let _serial = functional::serial();
    for source in [
        "fun outer(){ obj.target(1) }",
        "fun outer(){ obj?.target(1) }",
        "fun outer(){ target(1)() }",
        "fun outer(){ (target)(1) }",
        "fun outer(){ target<Int>(1) }",
        "fun outer(){ target(*p) }",
        "fun outer(){ target(1){} }",
        "fun outer(){ target(nested()) }",
        "fun outer(){ target(1+2) }",
        "fun outer(){ target(-1) }",
        "fun outer(){ target(1, }",
        "fun outer(){ target(,1) }",
        "fun outer(){ target(x=) }",
        "fun outer(){ target(1 2) }",
        "fun outer(){ val x=target(1) }",
        "fun outer(){ fun inner(){ target(1) } }",
        "class C { fun outer(){ target(1) } }",
        "fun outer(){ fun target(x:Int=1){}; target() }",
        "fun outer(){ fun target(vararg x:Int){}; target() }",
        "fun outer(){ suspend fun target(x:Int){}; target() }",
        "fun outer(){ fun Int.target(x:Int){}; target() }",
        "fun outer(){ fun <T> target(x:T){}; target() }",
        "fun outer(){ fun target(x:List<Int>){}; target() }",
        "fun outer(){ fun target(x:Int?){}; target() }",
        "fun outer(){ fun target(x:pkg.Int){}; target() }",
        "fun outer(){ fun target(x:Int){ val y=1 }; target() }",
    ] {
        let result = parse(source, 1);
        let current = result.snapshot();
        let tree = result.tree_for(current).unwrap();
        let Ok(lower) = lower_declarations(&result, current) else {
            continue;
        };
        if let Some(function) = lower
            .declarations()
            .iter()
            .find(|d| d.kind() == "FUN" && d.cst_parent_kind() == "BLOCK")
        {
            assert!(
                callable_parameters(&lower, &result, current, function).is_err(),
                "{source}"
            );
        } else {
            for (index, _) in tree
                .elements()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == "CALL_EXPRESSION")
            {
                assert!(
                    call_site(&lower, &result, current, index).is_err(),
                    "{source}"
                );
            }
        }
    }
}
