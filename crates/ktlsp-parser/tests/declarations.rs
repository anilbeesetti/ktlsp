//! Independently authored Kotlin 2.3 declaration baseline v1; no generated snapshots.
use ktlsp_parser::{FileKind, Parsed, Parser, ParserError, SourceSnapshot};
use std::sync::atomic::AtomicBool;

fn parser() -> Parser {
    Parser::new(env!("CARGO_BIN_EXE_ktlsp-parser-worker")).unwrap()
}
fn parse(source: &str) -> Parsed {
    parser()
        .parse(source, FileKind::Source, &AtomicBool::new(false))
        .unwrap()
}
fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "PACKAGE_DIRECTIVE"
            | "IMPORT_DIRECTIVE"
            | "IMPORT_ALIAS"
            | "CLASS"
            | "OBJECT_DECLARATION"
            | "FUN"
            | "PROPERTY"
            | "TYPEALIAS"
    )
}
// Kind, exact source extent, immediate CST parent kind, nearest selected parent, direct name.
type Decl<'a> = (
    &'a str,
    &'a str,
    &'a str,
    Option<usize>,
    Option<(&'a str, usize)>,
);
fn check(source: &str, expected: &[Decl<'_>], diagnostics: &[(usize, usize, &str)]) -> Parsed {
    let parsed = parse(source);
    check_tree(&parsed, source, expected, diagnostics);
    parsed
}
fn check_tree(
    parsed: &Parsed,
    source: &str,
    expected: &[Decl<'_>],
    diagnostics: &[(usize, usize, &str)],
) {
    assert_eq!(parsed.source(), source);
    let elements = parsed.elements();
    let selected: Vec<_> = elements
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            !e.token && declaration(&e.kind) && !(e.kind == "PACKAGE_DIRECTIVE" && e.start == e.end)
        })
        .collect();
    assert_eq!(selected.len(), expected.len(), "{source:?}");
    for ((id, e), &(kind, text, parent_kind, owner, name)) in selected.iter().zip(expected) {
        let start = source
            .find(text)
            .expect("independent expected spelling must exist");
        assert_eq!(
            (&*e.kind, e.start, e.end),
            (kind, start, start + text.len()),
            "{source:?}"
        );
        assert_eq!(elements[e.parent.unwrap()].kind, parent_kind);
        let mut parent = e.parent;
        let actual_owner = loop {
            match parent {
                Some(p) => {
                    if let Some(index) = selected.iter().position(|(id, _)| *id == p) {
                        break Some(index);
                    }
                    parent = elements[p].parent;
                }
                None => break None,
            }
        };
        assert_eq!(actual_owner, owner);
        if matches!(
            kind,
            "CLASS" | "OBJECT_DECLARATION" | "FUN" | "PROPERTY" | "TYPEALIAS" | "IMPORT_ALIAS"
        ) {
            let names: Vec<_> = elements
                .iter()
                .filter(|n| n.parent == Some(*id) && n.token && n.kind == "IDENTIFIER")
                .collect();
            assert_eq!(names.len(), usize::from(name.is_some()));
            if let Some((name, relative_offset)) = name {
                let n = names[0];
                let offset = start + relative_offset;
                assert_eq!(
                    (n.start, n.end, &source[n.start..n.end]),
                    (offset, offset + name.len(), name)
                );
            }
        }
    }
    let actual: Vec<_> = parsed
        .diagnostics()
        .iter()
        .map(|d| (d.start, d.end, d.message.as_str()))
        .collect();
    assert_eq!(actual, diagnostics, "{source:?}");
    let mut cursor = 0;
    for token in elements.iter().filter(|e| e.token) {
        assert_eq!(token.start, cursor);
        assert!(source.is_char_boundary(token.start) && source.is_char_boundary(token.end));
        cursor = token.end;
    }
    assert_eq!(cursor, source.len());
}

#[test]
fn baseline_preamble_and_declaration_kinds() {
    let source = "package p\nimport a.b as c\nimport x.*\nclass C {}\ninterface I {}\nobject O {}\nfun f() = 1\nval x=1\ntypealias A=String\n";
    let parsed = check(
        source,
        &[
            ("PACKAGE_DIRECTIVE", "package p", "KT_FILE", None, None),
            (
                "IMPORT_DIRECTIVE",
                "import a.b as c",
                "IMPORT_LIST",
                None,
                None,
            ),
            (
                "IMPORT_ALIAS",
                "as c",
                "IMPORT_DIRECTIVE",
                Some(1),
                Some(("c", 3)),
            ),
            ("IMPORT_DIRECTIVE", "import x.*", "IMPORT_LIST", None, None),
            ("CLASS", "class C {}", "KT_FILE", None, Some(("C", 6))),
            ("CLASS", "interface I {}", "KT_FILE", None, Some(("I", 10))),
            (
                "OBJECT_DECLARATION",
                "object O {}",
                "KT_FILE",
                None,
                Some(("O", 7)),
            ),
            ("FUN", "fun f() = 1", "KT_FILE", None, Some(("f", 4))),
            ("PROPERTY", "val x=1", "KT_FILE", None, Some(("x", 4))),
            (
                "TYPEALIAS",
                "typealias A=String",
                "KT_FILE",
                None,
                Some(("A", 10)),
            ),
        ],
        &[],
    );
    for (kind, spelling) in [
        ("INTERFACE_KEYWORD", "interface"),
        ("MUL", "*"),
        ("AS_KEYWORD", "as"),
    ] {
        let offset = source.find(spelling).unwrap();
        assert!(parsed.elements().iter().any(|e| e.token
            && e.kind == kind
            && (e.start, e.end) == (offset, offset + spelling.len())));
    }
}

#[test]
fn baseline_modifiers_annotations_generics_constraints_and_nested_members() {
    let source = "@Deprecated(\"old\") public open class Box<T : Any> where T : Comparable<T> {\n private val value: T? = null\n fun T.echo(): T = this\n class Nested\n}\n";
    let parsed = check(
        source,
        &[
            (
                "CLASS",
                source.trim_end(),
                "KT_FILE",
                None,
                Some(("Box", 37)),
            ),
            (
                "PROPERTY",
                "private val value: T? = null",
                "CLASS_BODY",
                Some(0),
                Some(("value", 12)),
            ),
            (
                "FUN",
                "fun T.echo(): T = this",
                "CLASS_BODY",
                Some(0),
                Some(("echo", 6)),
            ),
            (
                "CLASS",
                "class Nested",
                "CLASS_BODY",
                Some(0),
                Some(("Nested", 6)),
            ),
        ],
        &[],
    );
    for (kind, spelling) in [
        ("MODIFIER_LIST", "@Deprecated(\"old\") public open"),
        ("ANNOTATION_ENTRY", "@Deprecated(\"old\")"),
        ("TYPE_PARAMETER_LIST", "<T : Any>"),
        ("TYPE_CONSTRAINT_LIST", "T : Comparable<T>"),
    ] {
        let start = source.find(spelling).unwrap();
        assert!(
            parsed
                .elements()
                .iter()
                .any(|e| e.kind == kind && (e.start, e.end) == (start, start + spelling.len())),
            "{kind}"
        );
    }
}

#[test]
fn baseline_receivers_and_locals() {
    check(
        "val String.sizeHint: Int get() = length\nfun <T> T.echo(value: T): T where T: Any = value\n",
        &[
            (
                "PROPERTY",
                "val String.sizeHint: Int get() = length",
                "KT_FILE",
                None,
                Some(("sizeHint", 11)),
            ),
            (
                "FUN",
                "fun <T> T.echo(value: T): T where T: Any = value",
                "KT_FILE",
                None,
                Some(("echo", 10)),
            ),
        ],
        &[],
    );
    let source = "fun outer() { class Local; fun inner() {}; val local=1 }";
    check(
        source,
        &[
            ("FUN", source, "KT_FILE", None, Some(("outer", 4))),
            ("CLASS", "class Local", "BLOCK", Some(0), Some(("Local", 6))),
            (
                "FUN",
                "fun inner() {}",
                "BLOCK",
                Some(0),
                Some(("inner", 4)),
            ),
            (
                "PROPERTY",
                "val local=1",
                "BLOCK",
                Some(0),
                Some(("local", 4)),
            ),
        ],
        &[],
    );
}

#[test]
fn baseline_incomplete_declarations_retain_later_class() {
    for (prefix, kind, message) in [
        ("class", "CLASS", "Name expected"),
        ("interface", "CLASS", "Name expected"),
        ("object", "OBJECT_DECLARATION", "Name expected"),
        ("fun", "FUN", "Expecting function name or receiver type"),
        (
            "val",
            "PROPERTY",
            "Expecting property name or receiver type",
        ),
        ("fun String.", "FUN", "Expecting function name"),
    ] {
        let source = format!("{prefix}\nclass Later {{}}");
        check(
            &source,
            &[
                (kind, prefix, "KT_FILE", None, None),
                (
                    "CLASS",
                    "class Later {}",
                    "KT_FILE",
                    None,
                    Some(("Later", 6)),
                ),
            ],
            &[(prefix.len(), prefix.len(), message)],
        );
    }
    check(
        "typealias\nclass Later {}",
        &[
            ("TYPEALIAS", "typealias\n", "KT_FILE", None, None),
            (
                "CLASS",
                "class Later {}",
                "KT_FILE",
                None,
                Some(("Later", 6)),
            ),
        ],
        &[(9, 9, "Type name expected"), (10, 10, "Type expected")],
    );
}

#[test]
fn baseline_incomplete_preamble_alias_star_and_annotation_recovery() {
    check(
        "package\nclass Later {}",
        &[
            ("PACKAGE_DIRECTIVE", "package", "KT_FILE", None, None),
            (
                "CLASS",
                "class Later {}",
                "KT_FILE",
                None,
                Some(("Later", 6)),
            ),
        ],
        &[(
            7,
            7,
            "Package name must be a '.'-separated identifier list placed on a single line",
        )],
    );
    check(
        "import a.b as\nclass Later {}",
        &[
            (
                "IMPORT_DIRECTIVE",
                "import a.b as",
                "IMPORT_LIST",
                None,
                None,
            ),
            ("IMPORT_ALIAS", "as", "IMPORT_DIRECTIVE", Some(0), None),
            (
                "CLASS",
                "class Later {}",
                "KT_FILE",
                None,
                Some(("Later", 6)),
            ),
        ],
        &[(13, 13, "Expecting identifier")],
    );
    check(
        "import a.* as Alias\nclass Later {}",
        &[
            (
                "IMPORT_DIRECTIVE",
                "import a.* as Alias",
                "IMPORT_LIST",
                None,
                None,
            ),
            (
                "IMPORT_ALIAS",
                "as Alias",
                "ERROR_ELEMENT",
                Some(0),
                Some(("Alias", 3)),
            ),
            (
                "CLASS",
                "class Later {}",
                "KT_FILE",
                None,
                Some(("Later", 6)),
            ),
        ],
        &[(11, 19, "Cannot rename all imported items to one identifier")],
    );
    check(
        "@\nfun later() = 1",
        &[(
            "FUN",
            "fun later() = 1",
            "KT_FILE",
            None,
            Some(("later", 4)),
        )],
        &[(0, 1, "Expected annotation identifier after '@'")],
    );
}

#[test]
fn baseline_original_unicode_newlines_backticks_and_eof() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = format!("// 😀e\u{301}{newline}{newline}val `δ 😀` = 1{newline}{newline}");
        let parsed = check(
            &source,
            &[(
                "PROPERTY",
                "val `δ 😀` = 1",
                "KT_FILE",
                None,
                Some(("`δ 😀`", 4)),
            )],
            &[],
        );
        let start = source.find("val").unwrap();
        assert_eq!(
            parsed.position(start).unwrap(),
            ktlsp_text::Position {
                line: 2,
                character: 0
            }
        );
        assert_eq!(
            parsed.position(source.len()).unwrap(),
            ktlsp_text::Position {
                line: 4,
                character: 0
            }
        );
        assert!(parsed.position(4).is_err()); // astral UTF-8 interior
        assert!(
            ktlsp_text::position_to_offset(
                &source,
                ktlsp_text::Position {
                    line: 0,
                    character: 4
                }
            )
            .is_err()
        ); // surrogate interior
        if newline == "\r\n" {
            assert!(parsed.position(source.find('\r').unwrap() + 1).is_err());
        }
    }
    check(
        "fun f(",
        &[("FUN", "fun f(", "KT_FILE", None, Some(("f", 4)))],
        &[
            (6, 6, "Parameter name expected"),
            (6, 6, "Expecting comma or ')'"),
            (6, 6, "Expecting ')'"),
        ],
    );
}

#[test]
fn snapshots_reject_changed_revision_document_kind_and_reused_identity() {
    let original =
        SourceSnapshot::new("document-incarnation-1", 4, FileKind::Source, "val x=1\r\n").unwrap();
    let accepted = parser()
        .parse_snapshot(&original, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(accepted.snapshot(), &original);
    assert_eq!(
        accepted.tree_for(&original.clone()).unwrap().source(),
        original.source()
    );
    for current in [
        SourceSnapshot::new(original.document(), 5, original.kind(), original.source()).unwrap(),
        SourceSnapshot::new(
            "document-incarnation-2",
            4,
            original.kind(),
            original.source(),
        )
        .unwrap(),
        SourceSnapshot::new(original.document(), 4, FileKind::Script, original.source()).unwrap(),
        SourceSnapshot::new(original.document(), 4, original.kind(), "val x=2\r\n").unwrap(),
    ] {
        assert_eq!(
            accepted.tree_for(&current).unwrap_err(),
            ParserError::StaleSnapshot
        );
    }
    assert_eq!(original.revision(), 4);
    assert_eq!(original.source(), "val x=1\r\n");
    assert_eq!(
        parser()
            .parse_snapshot(&original, &AtomicBool::new(true))
            .unwrap_err(),
        ParserError::Cancelled
    );
    assert!(accepted.tree_for(&original).is_ok());
}

#[test]
fn unresolved_generic_recovery_gate_is_recorded() {
    {
        let source = "class Box<T\nfun healthy() = 2\n";
        let parsed = parse(source);
        assert!(!parsed.diagnostics().is_empty());
        // This is a known defect assertion, not acceptance of ordinary recovery.
        assert!(
            !parsed
                .elements()
                .iter()
                .any(|e| e.kind == "FUN" && source[e.start..e.end].starts_with("fun healthy"))
        );
        assert_eq!(
            parsed
                .elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
    }
    assert_eq!(
        parser()
            .parse(
                "{fun<)]<T:@( {})",
                FileKind::Source,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        ParserError::Coverage
    );
    check(
        "fun healthy() = 2",
        &[(
            "FUN",
            "fun healthy() = 2",
            "KT_FILE",
            None,
            Some(("healthy", 4)),
        )],
        &[],
    );
}

#[test]
fn malformed_parameter_lists_restore_original_snapshot_declarations() {
    for newline in ["\n", "\r\n", "\r"] {
        for (prefix, suffix, outer_kind, immediate_parent) in [
            ("", "", None, "KT_FILE"),
            ("class C { ", "}", Some("CLASS"), "CLASS_BODY"),
            ("fun outer() { ", "}", Some("FUN"), "BLOCK"),
        ] {
            let broken = "fun broken(";
            let later = "val `later 🦀e\u{301}` = 1";
            let healthy = "fun healthy() = 2";
            let source =
                format!("{prefix}{broken}{newline}{later}{newline}{healthy}{newline}{suffix}");
            let snapshot =
                SourceSnapshot::new("parameter-recovery", 7, FileKind::Source, source.as_str())
                    .unwrap();
            let result = parser()
                .parse_snapshot(&snapshot, &AtomicBool::new(false))
                .unwrap();
            let parsed = result.tree_for(&snapshot).unwrap();
            let owner = outer_kind.map(|_| 0);
            let mut expected = vec![
                ("FUN", broken, immediate_parent, owner, Some(("broken", 4))),
                (
                    "PROPERTY",
                    later,
                    immediate_parent,
                    owner,
                    Some(("`later 🦀e\u{301}`", 4)),
                ),
                (
                    "FUN",
                    healthy,
                    immediate_parent,
                    owner,
                    Some(("healthy", 4)),
                ),
            ];
            if let Some(kind) = outer_kind {
                expected.insert(
                    0,
                    (
                        kind,
                        source.as_str(),
                        "KT_FILE",
                        None,
                        Some(if kind == "CLASS" {
                            ("C", 6)
                        } else {
                            ("outer", 4)
                        }),
                    ),
                );
            }
            check_tree(
                parsed,
                &source,
                &expected,
                &[(
                    prefix.len() + broken.len(),
                    prefix.len() + broken.len(),
                    "Expecting ')'",
                )],
            );
            assert_eq!(
                parsed.position(source.find(later).unwrap()).unwrap(),
                ktlsp_text::Position {
                    line: 1,
                    character: 0
                }
            );
            if newline == "\r\n" {
                assert!(parsed.position(source.find('\r').unwrap() + 1).is_err());
            }
            assert_eq!(snapshot.source(), source);
            assert_eq!(result.snapshot(), &snapshot);
        }
    }
}

#[test]
fn repeated_parameter_recovery_retains_annotation_prefix_and_eof() {
    let source = "fun broken(x: Int,\n@A public fun second(\ntypealias Later = String";
    let snapshot = SourceSnapshot::new("repeated-recovery", 3, FileKind::Source, source).unwrap();
    let result = parser()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        source,
        &[
            (
                "FUN",
                "fun broken(x: Int,",
                "KT_FILE",
                None,
                Some(("broken", 4)),
            ),
            (
                "FUN",
                "@A public fun second(",
                "KT_FILE",
                None,
                Some(("second", 14)),
            ),
            (
                "TYPEALIAS",
                "typealias Later = String",
                "KT_FILE",
                None,
                Some(("Later", 10)),
            ),
        ],
        &[(18, 18, "Expecting ')'"), (40, 40, "Expecting ')'")],
    );
    let source = "fun broken(";
    let snapshot = SourceSnapshot::new("eof-recovery", 4, FileKind::Source, source).unwrap();
    let result = parser()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        source,
        &[("FUN", source, "KT_FILE", None, Some(("broken", 4)))],
        &[
            (11, 11, "Parameter name expected"),
            (11, 11, "Expecting comma or ')'"),
            (11, 11, "Expecting ')'"),
        ],
    );
}

#[test]
fn valid_parameter_contexts_preserve_default_declarations() {
    let source = "fun f(x: () -> Int = fun(): Int { val local = 1; return local }, y: Any = object { val member = 2 }) {}";
    let snapshot = SourceSnapshot::new("nested-defaults", 1, FileKind::Source, source).unwrap();
    let result = parser()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    check_tree(
        result.tree_for(&snapshot).unwrap(),
        source,
        &[
            ("FUN", source, "KT_FILE", None, Some(("f", 4))),
            (
                "FUN",
                "fun(): Int { val local = 1; return local }",
                "VALUE_PARAMETER",
                Some(0),
                None,
            ),
            (
                "PROPERTY",
                "val local = 1",
                "BLOCK",
                Some(1),
                Some(("local", 4)),
            ),
            (
                "OBJECT_DECLARATION",
                "object { val member = 2 }",
                "OBJECT_LITERAL",
                Some(0),
                None,
            ),
            (
                "PROPERTY",
                "val member = 2",
                "CLASS_BODY",
                Some(3),
                Some(("member", 4)),
            ),
        ],
        &[],
    );
    for source in [
        "fun <T> T.ext(\n @A x: List<Map<String, T>>,\n crossinline action: (T) -> Unit = { value -> val local = value; action(local) },\n y: Int = nested(call(1, 2), 3)\n): T = this",
        "fun f(x: Boolean = 1 < 2, y: Int = if (x) nested(1) else nested(2)) {}",
        "class C(val first: Int, var second: String) { constructor(x: Int): this(x, \"\") }",
        "fun f(x: (@A Int, String) -> Unit) {}",
        "fun f(\nvararg x: String,\n@A y: Int = run { fun local() { }; local() },\n) {}",
        "fun f(init: Int, constructor: String, public: Int, context: Int) {}",
    ] {
        let snapshot =
            SourceSnapshot::new("valid-parameter-context", 2, FileKind::Source, source).unwrap();
        let result = parser()
            .parse_snapshot(&snapshot, &AtomicBool::new(false))
            .unwrap();
        let parsed = result.tree_for(&snapshot).unwrap();
        assert_eq!(parsed.source(), source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(
            parsed
                .elements()
                .iter()
                .filter(|e| e.token)
                .map(|e| &source[e.start..e.end])
                .collect::<String>(),
            source
        );
    }
}

#[test]
fn malformed_member_parameters_preserve_initializers_and_constructors() {
    let source =
        "class C { fun broken(\ninit { val local = 1 }\nconstructor() {}\nfun healthy() = 2\n}";
    let snapshot = SourceSnapshot::new("member-recovery", 1, FileKind::Source, source).unwrap();
    let result = parser()
        .parse_snapshot(&snapshot, &AtomicBool::new(false))
        .unwrap();
    let parsed = result.tree_for(&snapshot).unwrap();
    let elements = parsed.elements();
    for (kind, text) in [
        ("FUN", "fun broken("),
        ("CLASS_INITIALIZER", "init { val local = 1 }"),
        ("SECONDARY_CONSTRUCTOR", "constructor() {}"),
        ("FUN", "fun healthy() = 2"),
    ] {
        let start = source.find(text).unwrap();
        let e = elements
            .iter()
            .find(|e| !e.token && e.kind == kind && (e.start, e.end) == (start, start + text.len()))
            .unwrap();
        let body = &elements[e.parent.unwrap()];
        assert_eq!(body.kind, "CLASS_BODY");
        let class = &elements[body.parent.unwrap()];
        assert_eq!(
            (&*class.kind, class.start, class.end),
            ("CLASS", 0, source.len())
        );
    }
    let local = elements
        .iter()
        .find(|e| !e.token && e.kind == "PROPERTY")
        .unwrap();
    assert_eq!(&source[local.start..local.end], "val local = 1");
    let block = &elements[local.parent.unwrap()];
    assert_eq!(block.kind, "BLOCK");
    assert_eq!(elements[block.parent.unwrap()].kind, "CLASS_INITIALIZER");
    assert_eq!(parsed.diagnostics().len(), 1);
    let d = &parsed.diagnostics()[0];
    assert_eq!(
        (d.start, d.end, d.message.as_str()),
        (21, 21, "Expecting ')'")
    );
}
