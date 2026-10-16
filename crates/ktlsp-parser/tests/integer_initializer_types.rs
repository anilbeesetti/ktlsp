//! Real native parser -> lowering -> authenticated site -> binding -> integer query.
#[path = "support/functional.rs"]
mod functional;

use ktlsp_parser::{
    FileKind, Parser, SourceSnapshot,
    integer_initializer_types::{IntegerInitializerError, IntegerType, infer_integer_initializer},
    local_value_binding::bind_local_value,
    lowering::{ReferenceSiteError, lower_declarations, reference_site},
    structural_regions::build_regions,
};
use std::sync::atomic::AtomicBool;

#[test]
fn native_integer_targets_preserve_literal_identity_and_default_types() {
    let _serial = functional::serial();
    use IntegerType::*;
    let cases = [
        ("1", 1, Int),
        ("2147483647", 2147483647, Int),
        ("2147483648", 2147483648, Long),
        ("9223372036854775807", i64::MAX as u64, Long),
        ("1L", 1, Long),
        ("0x7fff_FFFF", i32::MAX as u64, Int),
        ("0B1__0", 2, Int),
        ("0u", 0, UInt),
        ("4294967295U", u32::MAX as u64, UInt),
        ("4294967296u", 4294967296, ULong),
        ("18446744073709551615u", u64::MAX, ULong),
        ("0xFFFF_FFFF_FFFF_FFFFUL", u64::MAX, ULong),
        ("1uL", 1, ULong),
    ];
    let mut source = String::new();
    for (i, (literal, _, _)) in cases.iter().enumerate() {
        source.push_str(&format!("fun f{i}() {{ val x = {literal}; val y = x }}\n"));
    }
    let snapshot =
        SourceSnapshot::new("integer-initializers", 1, FileKind::Source, source.as_str()).unwrap();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    assert!(
        tree.diagnostics().is_empty(),
        "real producer diagnostics: {:?}",
        tree.diagnostics()
    );
    let lowered = lower_declarations(&result, &snapshot).unwrap();
    let regions = build_regions(&lowered, &result, &snapshot).unwrap();
    let references: Vec<_> = tree
        .elements()
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.token && e.kind == "REFERENCE_EXPRESSION")
        .collect();
    assert_eq!(references.len(), cases.len());
    for ((index, expression), (raw, value, ty)) in references.into_iter().zip(cases) {
        let site = reference_site(&lowered, &result, &snapshot, index).unwrap();
        let binding = bind_local_value(&regions, &site, &snapshot).unwrap();
        let inferred = infer_integer_initializer(&binding, &snapshot).unwrap();
        assert!(std::ptr::eq(inferred.binding(), &binding));
        assert_eq!(binding.target().raw_name(), "x");
        assert_eq!(site.containing_property().raw_name(), "y");
        assert_eq!(&source[expression.start..expression.end], "x");
        assert_eq!(inferred.raw_spelling(), raw);
        assert_eq!(inferred.value(), value);
        assert_eq!(inferred.default_type(), ty);
        assert_eq!(&source[inferred.range()], raw);
        assert!(binding.target().range().start <= inferred.range().start);
        assert!(inferred.range().end <= binding.target().range().end);
        let token_index = tree
            .elements()
            .iter()
            .position(|e| std::ptr::eq(e, inferred.literal()))
            .unwrap();
        let integer_index = inferred.literal().parent.unwrap();
        assert_eq!(tree.elements()[integer_index].kind, "INTEGER_CONSTANT");
        assert_eq!(
            tree.elements()[integer_index].parent,
            Some(binding.target().cst_index())
        );
        assert_eq!(token_index, integer_index + 1);
        assert_eq!(
            inferred.raw_spelling().as_ptr(),
            tree.source()[inferred.range()].as_ptr()
        );
        for changed in [
            SourceSnapshot::new("other", 1, FileKind::Source, source.as_str()).unwrap(),
            SourceSnapshot::new("integer-initializers", 2, FileKind::Source, source.as_str())
                .unwrap(),
            SourceSnapshot::new("integer-initializers", 1, FileKind::Script, source.as_str())
                .unwrap(),
            SourceSnapshot::new(
                "integer-initializers",
                1,
                FileKind::Source,
                format!("{source}\n"),
            )
            .unwrap(),
        ] {
            assert_eq!(
                infer_integer_initializer(&binding, &changed).unwrap_err(),
                IntegerInitializerError::StaleSnapshot
            );
        }
        let independent = lower_declarations(&result, &snapshot).unwrap();
        let independent_regions = build_regions(&independent, &result, &snapshot).unwrap();
        assert_eq!(
            bind_local_value(&independent_regions, &site, &snapshot).unwrap_err(),
            ktlsp_parser::local_value_binding::LocalBindingError::InvalidOrigin
        );
        // Temporary direct-Write observation for ROOT's original hosted proof.
        std::io::Write::write_all(
            &mut std::io::stderr().lock(),
            format!(
                "INTEGER_INITIALIZER_PROOF raw={raw:?} value={} type={:?} target={}..{} literal={}..{} site={}..{} original_binding=true original_literal=true original_source=true\n",
                inferred.value(), inferred.default_type(), binding.target().range().start,
                binding.target().range().end, inferred.range().start, inferred.range().end,
                site.range().start, site.range().end
            )
            .as_bytes(),
        )
        .unwrap();
    }
}

#[test]
fn native_reference_target_is_not_chased_and_foreign_result_is_refused() {
    let _serial = functional::serial();
    let source = "fun f() { val z = 1; val x = z; val y = x }\n";
    let snapshot = SourceSnapshot::new("integer-reference", 1, FileKind::Source, source).unwrap();
    let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
    let result = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let foreign = parser
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let tree = result.tree_for(&snapshot).unwrap();
    let lowered = lower_declarations(&result, &snapshot).unwrap();
    let regions = build_regions(&lowered, &result, &snapshot).unwrap();
    let index = tree
        .elements()
        .iter()
        .rposition(|e| !e.token && e.kind == "REFERENCE_EXPRESSION")
        .unwrap();
    let site = reference_site(&lowered, &result, &snapshot, index).unwrap();
    assert_eq!(
        reference_site(&lowered, &foreign, &snapshot, index).unwrap_err(),
        ReferenceSiteError::InvalidOrigin
    );
    let binding = bind_local_value(&regions, &site, &snapshot).unwrap();
    assert_eq!(binding.target().raw_name(), "x");
    assert_eq!(
        infer_integer_initializer(&binding, &snapshot).unwrap_err(),
        IntegerInitializerError::ReferenceInitializer
    );
}

#[test]
fn native_overflow_and_invalid_suffix_are_not_types() {
    let _serial = functional::serial();
    for (literal, expected) in [
        ("9223372036854775808", IntegerInitializerError::OutOfRange),
        ("18446744073709551616u", IntegerInitializerError::OutOfRange),
        ("0x8000000000000000", IntegerInitializerError::OutOfRange),
        ("1l", IntegerInitializerError::InvalidLiteral),
        ("0x_1", IntegerInitializerError::InvalidLiteral),
        ("1_", IntegerInitializerError::InvalidLiteral),
    ] {
        let source = format!("fun f() {{ val x = {literal}; val y = x }}\n");
        let snapshot =
            SourceSnapshot::new("integer-invalid", 1, FileKind::Source, source.as_str()).unwrap();
        let parser = Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap();
        let result = parser
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let tree = result.tree_for(&snapshot).unwrap();
        let lowered = lower_declarations(&result, &snapshot)
            .unwrap_or_else(|error| panic!("{literal}: real producer lowering refused: {error:?}"));
        let regions = build_regions(&lowered, &result, &snapshot).unwrap();
        let index = tree
            .elements()
            .iter()
            .rposition(|e| !e.token && e.kind == "REFERENCE_EXPRESSION")
            .unwrap();
        let site = reference_site(&lowered, &result, &snapshot, index)
            .unwrap_or_else(|error| panic!("{literal}: real producer site refused: {error:?}"));
        let binding = bind_local_value(&regions, &site, &snapshot)
            .unwrap_or_else(|error| panic!("{literal}: real producer binding refused: {error:?}"));
        assert_eq!(
            infer_integer_initializer(&binding, &snapshot).unwrap_err(),
            expected,
            "{literal}"
        );
        // Temporary direct-Write observation for ROOT's original hosted proof.
        std::io::Write::write_all(
            &mut std::io::stderr().lock(),
            format!("INTEGER_INITIALIZER_REFUSAL raw={literal:?} error={expected:?} native_binding=true\n")
                .as_bytes(),
        )
        .unwrap();
    }
}
