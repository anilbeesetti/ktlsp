//! Real production parser/public type proof. No mock producer or semantic echo.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SnapshotResult, SourceSnapshot,
    integer_initializer_types::{IntegerInitializerError, IntegerType, infer_integer_initializer},
    local_value_binding::{
        LocalBindingError, LocalValueType, LocalValueTypeError, LocalValueTypeProof,
        LocalValueTypeTerminal, bind_local_value, infer_local_value_type,
    },
    lowering::{Declaration, lower_declarations, reference_site},
    structural_regions::{RegionDirectory, build_regions},
};
use std::sync::atomic::AtomicBool;

// TEMP428 actual-value writers stay until BOTH owner and ROOT read ORIGINAL
// hosted values. Assertions/fixtures remain permanent after writer retirement.
fn capture(label: &str, value: impl std::fmt::Debug) {
    std::io::Write::write_all(
        &mut std::io::stderr().lock(),
        format!("LOCAL_VALUE_TYPE_428 {label}\n{value:#?}\nEND_LOCAL_VALUE_TYPE_428\n").as_bytes(),
    )
    .unwrap();
}

fn parse(source: &str) -> SnapshotResult {
    let snapshot =
        SourceSnapshot::new("file:///local-value-types.kt", 7, FileKind::Source, source).unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    capture("original-snapshot-result-CST-diagnostics", &result);
    let tree = result.tree_for(&snapshot).unwrap();
    let mut cursor = 0;
    for (index, token) in tree.elements().iter().enumerate().filter(|(_, e)| e.token) {
        assert_eq!(token.start, cursor, "complete original token partition");
        assert!(token.start < token.end);
        let raw = &tree.source()[token.start..token.end];
        capture(
            "original-token-raw-UTF16",
            (
                index,
                token,
                raw,
                tree.position(token.start),
                tree.position(token.end),
            ),
        );
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
    result
}

fn declaration<'a>(directory: &'a RegionDirectory<'a>, name: &str) -> &'a Declaration<'a> {
    directory
        .lowered()
        .declarations()
        .iter()
        .find(|d| d.raw_name() == name)
        .unwrap()
}

fn query<'a>(
    directory: &'a RegionDirectory<'a>,
    result: &'a SnapshotResult,
    selected: &'a Declaration<'a>,
) -> Result<LocalValueTypeProof<'a>, LocalValueTypeError> {
    capture(
        "complete-original-declarations-owners-memberships",
        directory,
    );
    capture(
        "original-request-selector-snapshot-and-pointer",
        (
            selected,
            result.snapshot(),
            directory
                .lowered()
                .declarations()
                .iter()
                .position(|d| std::ptr::eq(d, selected)),
        ),
    );
    let outcome = infer_local_value_type(directory, result, selected, result.snapshot());
    capture("complete-public-proof-or-error", &outcome);
    if let Ok(proof) = &outcome {
        let tree = result.tree_for(result.snapshot()).unwrap();
        let selected_ordinal = directory
            .lowered()
            .declarations()
            .iter()
            .position(|d| std::ptr::eq(d, proof.selected()));
        let terminal_ordinal = directory
            .lowered()
            .declarations()
            .iter()
            .position(|d| std::ptr::eq(d, proof.terminal_declaration()));
        let function_ordinal = directory
            .lowered()
            .declarations()
            .iter()
            .position(|d| std::ptr::eq(d, proof.function()));
        let body_ordinal = directory
            .regions()
            .iter()
            .position(|b| std::ptr::eq(b, proof.body()));
        let comparisons = (
            std::ptr::eq(directory, proof.directory()),
            std::ptr::eq(selected, proof.selected()),
            selected_ordinal,
            terminal_ordinal,
            function_ordinal,
            body_ordinal,
            proof.dependency_edges(),
        );
        capture("original-proof-pointer-comparisons", comparisons);
        assert!(comparisons.0 && comparisons.1);
        assert!(selected_ordinal.is_some() && terminal_ordinal.is_some());
        assert!(function_ordinal.is_some() && body_ordinal.is_some());
        assert_eq!(proof.selected().owner(), function_ordinal);
        assert_eq!(proof.terminal_declaration().owner(), function_ordinal);
        assert_eq!(
            directory.memberships()[selected_ordinal.unwrap()].region(),
            body_ordinal.unwrap()
        );
        if let LocalValueType::LexicalTypeParameter(target) = proof.value_type() {
            let ordinal = directory
                .lowered()
                .declarations()
                .iter()
                .position(|d| std::ptr::eq(d, target));
            capture(
                "lexical-type-original-declaration-pointer-and-owner",
                (ordinal, target.owner(), target),
            );
            assert!(ordinal.is_some());
            assert_eq!(target.owner(), function_ordinal);
        }
        match proof.terminal() {
            LocalValueTypeTerminal::Integer(terminal) => {
                let index = tree
                    .elements()
                    .iter()
                    .position(|e| std::ptr::eq(e, terminal.literal()))
                    .unwrap();
                let integer = terminal.literal().parent.unwrap();
                let comparisons = (
                    index == integer + 1,
                    tree.elements()[integer].parent
                        == Some(proof.terminal_declaration().cst_index()),
                    terminal.raw_spelling().as_ptr() == tree.source()[terminal.range()].as_ptr(),
                );
                capture(
                    "integer-terminal-original-pointer-raw-range-value",
                    (
                        comparisons,
                        index,
                        terminal.range(),
                        terminal.raw_spelling(),
                        terminal.value(),
                    ),
                );
                assert!(comparisons.0 && comparisons.1 && comparisons.2);
                assert_eq!(&tree.source()[terminal.range()], terminal.raw_spelling());
            }
            LocalValueTypeTerminal::Parameter(annotation) => {
                assert!(std::ptr::eq(
                    annotation.annotated(),
                    proof.terminal_declaration()
                ));
                let index = annotation.reference_index();
                let comparisons = (
                    std::ptr::eq(annotation.reference(), &tree.elements()[index]),
                    std::ptr::eq(annotation.name_token(), &tree.elements()[index + 1]),
                );
                capture(
                    "parameter-terminal-original-annotation-pointers",
                    (comparisons, annotation),
                );
                assert!(comparisons.0 && comparisons.1);
                assert_eq!(proof.terminal_declaration().kind(), "VALUE_PARAMETER");
            }
        }
    }
    outcome
}

fn assert_integer(proof: &LocalValueTypeProof<'_>, ty: IntegerType, raw: &str, edges: usize) {
    match proof.value_type() {
        LocalValueType::DefaultInteger(actual) => assert_eq!(actual, ty),
        other => panic!("expected integer default, got {other:?}"),
    }
    let LocalValueTypeTerminal::Integer(terminal) = proof.terminal() else {
        panic!("not original literal")
    };
    assert_eq!(terminal.raw_spelling(), raw);
    assert_eq!(proof.dependency_edges(), edges);
}

#[test]
fn unused_direct_and_chained_original_integer_defaults() {
    let _serial = functional::serial();
    for (raw, ty) in [
        ("0", IntegerType::Int),
        ("2147483647", IntegerType::Int),
        ("2147483648", IntegerType::Long),
        ("9223372036854775807", IntegerType::Long),
        ("1L", IntegerType::Long),
        ("0x7fff_FFFF", IntegerType::Int),
        ("0B1__0", IntegerType::Int),
        ("0u", IntegerType::UInt),
        ("4294967295U", IntegerType::UInt),
        ("4294967296u", IntegerType::ULong),
        ("18446744073709551615u", IntegerType::ULong),
        ("0xFFFF_FFFF_FFFF_FFFFUL", IntegerType::ULong),
        ("1uL", IntegerType::ULong),
    ] {
        let result = parse(&format!(
            "fun f() {{ val a={raw}; val b=a; val c=b; val unused=2 }}"
        ));
        let current = result.snapshot();
        assert!(result.tree_for(current).unwrap().diagnostics().is_empty());
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        for (name, edges) in [("a", 0), ("b", 1), ("c", 2)] {
            let proof = query(&directory, &result, declaration(&directory, name)).unwrap();
            assert_integer(&proof, ty, raw, edges);
            assert!(std::ptr::eq(
                proof.terminal_declaration(),
                declaration(&directory, "a")
            ));
        }
        let unused = query(&directory, &result, declaration(&directory, "unused")).unwrap();
        assert_integer(&unused, IntegerType::Int, "2", 0);
        assert!(std::ptr::eq(
            unused.selected(),
            unused.terminal_declaration()
        ));
        // The old literal/value API stays strict: target b is not a literal.
        let tree = result.tree_for(current).unwrap();
        let index = tree
            .elements()
            .iter()
            .enumerate()
            .find(|(_, e)| {
                !e.token
                    && e.kind == "REFERENCE_EXPRESSION"
                    && e.parent == Some(declaration(&directory, "c").cst_index())
            })
            .unwrap()
            .0;
        let site = reference_site(&lower, &result, current, index).unwrap();
        let binding = bind_local_value(&directory, &site, current).unwrap();
        let old = infer_integer_initializer(&binding, current);
        capture("unchanged-direct-integer-reference-refusal", &old);
        assert_eq!(
            old.unwrap_err(),
            IntegerInitializerError::ReferenceInitializer
        );
    }
}

#[test]
fn lexical_identity_annotations_shadowing_and_original_use_positions() {
    let _serial = functional::serial();
    for (source, selected, terminal, edges, lexical) in [
        ("fun<T> f(p:T){val a=p;val b:T=a;val c=b}", "c", "p", 3, "T"),
        ("fun<Int> f(p:Int){val a:Int=p;val b=a}", "b", "p", 2, "Int"),
        (
            "fun<T> f(x:T){val before=x;val x=1;val after=x}",
            "before",
            "x",
            1,
            "T",
        ),
        ("fun<T> f(x:T){val x=x;val y=x}", "y", "x", 2, "T"),
        (
            "fun<`🦀`> f(`😀`:`🦀`){val a:`🦀`=`😀`;val b=a}",
            "b",
            "`😀`",
            2,
            "`🦀`",
        ),
        (
            "fun<T> f(p:T){val a=p;val unrelated:Long=1;val b:T=a}",
            "b",
            "p",
            2,
            "T",
        ),
    ] {
        let result = parse(source);
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let proof = query(&directory, &result, declaration(&directory, selected)).unwrap();
        assert_eq!(proof.dependency_edges(), edges);
        assert_eq!(proof.terminal_declaration().raw_name(), terminal);
        let LocalValueType::LexicalTypeParameter(target) = proof.value_type() else {
            panic!("lost lexical identity")
        };
        assert!(std::ptr::eq(target, declaration(&directory, lexical)));
        assert_eq!(target.kind(), "TYPE_PARAMETER");
        assert!(std::ptr::eq(
            proof.terminal_declaration(),
            directory
                .lowered()
                .declarations()
                .iter()
                .find(|d| d.kind() == "VALUE_PARAMETER" && d.raw_name() == terminal)
                .unwrap()
        ));
        if source.contains("before") {
            let after = query(&directory, &result, declaration(&directory, "after")).unwrap();
            assert_integer(&after, IntegerType::Int, "1", 1);
            assert_eq!(after.terminal_declaration().kind(), "PROPERTY");
        }
    }
}

#[test]
fn complete_competition_original_order_and_annotation_authority_refuse() {
    let _serial = functional::serial();
    use LocalBindingError::*;
    use LocalValueTypeError::{Binding, IncompatibleAnnotation};
    for (source, expected) in [
        ("fun f(){val a=b;val b=1;val c=a}", Binding(NoEligibleLocal)),
        ("fun f(){val a=b;val b=a;val c=b}", Binding(NoEligibleLocal)),
        ("fun f(){val a=a;val c=a}", Binding(NoEligibleLocal)),
        (
            "fun f(){val a=1;val b=a;val `a`=2;val c=b}",
            Binding(AmbiguousLocal),
        ),
        (
            "fun f(){val a=1;fun a(){};val c=a}",
            Binding(AmbiguousLocal),
        ),
        (
            "fun f(){val a=1;val c=a;fun a(){}}",
            Binding(AmbiguousLocal),
        ),
        ("fun<T,U> f(p:T){val a:U=p;val c=a}", IncompatibleAnnotation),
        ("fun<T> f(){val a:T=1;val c=a}", IncompatibleAnnotation),
        ("fun<T> f(){val a=1;val c:T=a}", IncompatibleAnnotation),
        (
            "fun f(){val a:Long=1;val c=a}",
            Binding(NoEligibleTypeParameter),
        ),
        (
            "fun f(){val a=1;val c:Int=a}",
            Binding(NoEligibleTypeParameter),
        ),
        (
            "fun f(){val a=1;val c:Byte=a}",
            Binding(NoEligibleTypeParameter),
        ),
        (
            "fun f(){val a=1;val c:Long=a}",
            Binding(NoEligibleTypeParameter),
        ),
        ("fun<Int> f(){val a=1;val c:Int=a}", IncompatibleAnnotation),
        (
            "fun<T,U> f(p:T){val a:T=p;val b:U=a;val c=b}",
            IncompatibleAnnotation,
        ),
        ("fun f(p:Int){val c=p}", Binding(NoEligibleTypeParameter)),
        (
            "import other.Int\nfun f(){val a=1;val c:Int=a}",
            Binding(NoEligibleTypeParameter),
        ),
        (
            "class Int {}\nfun f(p:Int){val c=p}",
            Binding(NoEligibleTypeParameter),
        ),
        (
            "fun<T,`T`> f(p:T){val c=p}",
            Binding(AmbiguousTypeParameter),
        ),
        ("fun<T,T> f(){val c=1}", Binding(AmbiguousTypeParameter)),
    ] {
        let result = parse(source);
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        assert_eq!(
            query(&directory, &result, declaration(&directory, "c")).unwrap_err(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn original_bytes_utf16_and_foreign_origin_snapshot_selectors() {
    let _serial = functional::serial();
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "// 😀{newline}fun f() {{{newline}val `🦀`=0B1__0;{newline}val b=`🦀`;val c=b{newline}}}"
        );
        let result = parse(&source);
        let current = result.snapshot();
        let lower = lower_declarations(&result, current).unwrap();
        let directory = build_regions(&lower, &result, current).unwrap();
        let selected = declaration(&directory, "c");
        let proof = query(&directory, &result, selected).unwrap();
        assert_integer(&proof, IntegerType::Int, "0B1__0", 2);
        let tree = result.tree_for(current).unwrap();
        let name = declaration(&directory, "`🦀`").name_range();
        let start = tree.position(name.start).unwrap();
        let end = tree.position(name.end).unwrap();
        capture("nonBMP-exact-LF-CRLF-name-range-UTF16", (&name, start, end));
        assert_eq!(start.line, 2);
        assert_eq!(start.character, 4);
        assert_eq!(end.character, 8);
        assert_eq!(&source[name], "`🦀`");
        for changed in [
            SourceSnapshot::new("other", 7, FileKind::Source, source.as_str()).unwrap(),
            SourceSnapshot::new(current.document(), 8, FileKind::Source, source.as_str()).unwrap(),
            SourceSnapshot::new(current.document(), 7, FileKind::Script, source.as_str()).unwrap(),
            SourceSnapshot::new(
                current.document(),
                7,
                FileKind::Source,
                format!("{source}\n"),
            )
            .unwrap(),
        ] {
            let outcome = infer_local_value_type(&directory, &result, selected, &changed);
            capture("changed-snapshot-proof-refusal", (&changed, &outcome));
            assert_eq!(
                outcome.unwrap_err(),
                LocalValueTypeError::Binding(LocalBindingError::StaleSnapshot)
            );
        }
        let independent = lower_declarations(&result, current).unwrap();
        let foreign = parse(&source);
        let foreign_lower = lower_declarations(&foreign, foreign.snapshot()).unwrap();
        for target in [
            &independent.declarations()[1],
            &foreign_lower.declarations()[1],
        ] {
            let outcome = infer_local_value_type(&directory, &result, target, current);
            capture("equal-byte-foreign-selector-refusal", &outcome);
            assert_eq!(
                outcome.unwrap_err(),
                LocalValueTypeError::Binding(LocalBindingError::InvalidOrigin)
            );
        }
        let outcome = infer_local_value_type(&directory, &foreign, selected, current);
        capture("equal-byte-foreign-result-refusal", &outcome);
        assert_eq!(
            outcome.unwrap_err(),
            LocalValueTypeError::Binding(LocalBindingError::InvalidOrigin)
        );
        let outcome = query(&directory, &result, &lower.declarations()[0]);
        assert_eq!(
            outcome.unwrap_err(),
            LocalValueTypeError::Binding(LocalBindingError::UnsupportedContext)
        );
    }
}

#[test]
fn integer_leaf_errors_are_reached_through_real_original_two_edge_chain() {
    let _serial = functional::serial();
    for (raw, expected) in [
        ("9223372036854775808", IntegerInitializerError::OutOfRange),
        ("18446744073709551616u", IntegerInitializerError::OutOfRange),
        ("0x8000000000000000", IntegerInitializerError::OutOfRange),
        ("1l", IntegerInitializerError::InvalidLiteral),
        ("0x_1", IntegerInitializerError::InvalidLiteral),
        ("1_", IntegerInitializerError::InvalidLiteral),
    ] {
        let result = parse(&format!("fun f(){{val a={raw};val b=a;val c=b}}"));
        let lower = lower_declarations(&result, result.snapshot())
            .unwrap_or_else(|error| panic!("{raw}: intended leaf NOT reached: {error:?}"));
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        let outcome = query(&directory, &result, declaration(&directory, "c"));
        assert_eq!(
            outcome.unwrap_err(),
            LocalValueTypeError::Integer(expected),
            "{raw}"
        );
    }
}

#[test]
fn strict_complete_body_is_not_widened_by_type_only_grammar() {
    let _serial = functional::serial();
    for source in [
        "fun<T> f(p:T){val a=1;val c=a;fun target(x:T){};target(x=p)}",
        "fun<T> f(p:T){val a=1;val c=a;fun target(){};target(1)}",
        "fun<T> f(p:T):T{val a=1;val c=a}",
        "fun f(){val a=1;val c=a;fun target(){val x=1}}",
    ] {
        let result = parse(source);
        let lower = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
        assert_eq!(
            query(&directory, &result, declaration(&directory, "c")).unwrap_err(),
            LocalValueTypeError::Binding(LocalBindingError::UnsupportedContext)
        );
    }
    // Shapes refused by lowering have NO selected declaration or public proof.
    // Capture their actual producer/lowering error, never count this as leaf proof.
    for tail in ["var z=1", "a=2", "if(true){val z=1}", "val z=", "val z=1+"] {
        let result = parse(&format!("fun f(){{val a=1;val c=a;{tail}}}"));
        let lowered = lower_declarations(&result, result.snapshot());
        capture("unsupported-tail-complete-lowering-outcome", &lowered);
        match lowered {
            Err(_) => {}
            Ok(lower) => {
                let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
                assert!(query(&directory, &result, declaration(&directory, "c")).is_err());
            }
        }
    }
    let result = parse("fun f(){val a=1;fun empty(){};empty();val c=a}");
    let lower = lower_declarations(&result, result.snapshot()).unwrap();
    let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
    assert_integer(
        &query(&directory, &result, declaration(&directory, "c")).unwrap(),
        IntegerType::Int,
        "1",
        1,
    );
}

#[test]
fn documentation_lowering_never_grants_value_type_authority() {
    let _serial = functional::serial();
    let result = parse("/** source docs */\nfun f(){val a=1;val c=a}");
    let mut budget = ktlsp_parser::documentation::Budget::new(&|| false);
    let lower =
        ktlsp_parser::documentation::lower(&result, result.snapshot(), &mut budget).unwrap();
    let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
    assert_eq!(
        query(&directory, &result, declaration(&directory, "c")).unwrap_err(),
        LocalValueTypeError::Binding(LocalBindingError::UnsupportedContext)
    );
}

#[test]
fn real_original_chain_consumes_one_cumulative_allowance_not_one_per_hop() {
    let _serial = functional::serial();
    let names: Vec<_> = (0..96).map(|i| format!("original_value_{i:04}")).collect();
    let mut source = String::from("fun f(){");
    for (i, name) in names.iter().enumerate() {
        source.push_str(&format!(
            "val {name}={};",
            if i == 0 { "1" } else { &names[i - 1] }
        ));
    }
    source.push('}');
    let result = parse(&source);
    let lower = lower_declarations(&result, result.snapshot()).unwrap();
    let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
    assert_integer(
        &query(&directory, &result, declaration(&directory, &names[0])).unwrap(),
        IntegerType::Int,
        "1",
        0,
    );
    let outcome = query(
        &directory,
        &result,
        declaration(&directory, names.last().unwrap()),
    );
    assert!(matches!(
        outcome.unwrap_err(),
        LocalValueTypeError::Binding(LocalBindingError::Resource)
            | LocalValueTypeError::Integer(IntegerInitializerError::Resource)
    ));
}

#[test]
fn parameter_selector_and_script_producer_do_not_manufacture_local_proofs() {
    let _serial = functional::serial();
    let result = parse("fun<T> f(p:T){val a=p}");
    let lower = lower_declarations(&result, result.snapshot()).unwrap();
    let directory = build_regions(&lower, &result, result.snapshot()).unwrap();
    assert_eq!(
        query(&directory, &result, declaration(&directory, "p")).unwrap_err(),
        LocalValueTypeError::Binding(LocalBindingError::UnsupportedContext)
    );
    let snapshot = SourceSnapshot::new(
        "file:///local-values.kts",
        7,
        FileKind::Script,
        "fun f(){val a=1;val c=a}",
    )
    .unwrap();
    let result = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker"))
        .unwrap()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    capture("original-script-snapshot-result", &result);
    let lowered = lower_declarations(&result, &snapshot);
    capture("script-lowering-no-declaration-authority", &lowered);
    assert!(lowered.is_err());
}
