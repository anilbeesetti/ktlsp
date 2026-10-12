use ktrs_parser::{FileKind, parse_file};
use ktrs_syntax::{
    Parse,
    SyntaxKind::{self, *},
};

fn declaration(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        FUN | CLASS | PROPERTY | OBJECT_DECLARATION | TYPEALIAS
    )
}

fn check(
    source: &str,
    expected: &[(SyntaxKind, &str, Option<&str>, Option<&str>)],
    errors: &[usize],
) {
    let parsed = parse_file(source, FileKind::Source);
    let tree = &parsed.tree;
    assert_eq!(tree.text(), source);
    assert!(parsed.missed_tokens.is_empty());
    let actual: Vec<_> = (0..tree.len() as u32)
        .filter(|&id| declaration(tree.kind(id)))
        .collect();
    assert_eq!(actual.len(), expected.len(), "{source:?}");
    for (&id, &(kind, text, name, parent)) in actual.iter().zip(expected) {
        let start = source.find(text).unwrap();
        let range = tree.text_range(id);
        assert_eq!(tree.kind(id), kind);
        assert_eq!(
            (
                u32::from(range.start()) as usize,
                u32::from(range.end()) as usize
            ),
            (start, start + text.len()),
            "{text:?}"
        );
        if let Some(name) = name {
            let name_start = start + text.find(name).unwrap();
            assert!(
                (id + 1..tree.subtree_end(id)).any(|child| tree.parent(child) == Some(id)
                    && tree.kind(child) == IDENTIFIER
                    && u32::from(tree.text_range(child).start()) as usize == name_start
                    && u32::from(tree.text_range(child).end()) as usize == name_start + name.len()),
                "name {name}"
            );
        }
        let mut ancestor = tree.parent(id);
        while ancestor.is_some_and(|p| !declaration(tree.kind(p))) {
            ancestor = tree.parent(ancestor.unwrap());
        }
        assert_eq!(
            ancestor.map(|p| {
                let r = tree.text_range(p);
                &source[u32::from(r.start()) as usize..u32::from(r.end()) as usize]
            }),
            parent
        );
    }
    assert_eq!(parsed.error_messages, vec!["Expecting ')'"; errors.len()]);
    let spans: Vec<_> = (0..tree.len() as u32)
        .filter(|&id| tree.kind(id) == ERROR_ELEMENT)
        .map(|id| {
            let r = tree.text_range(id);
            (u32::from(r.start()) as usize, u32::from(r.end()) as usize)
        })
        .collect();
    assert_eq!(spans, errors.iter().map(|&p| (p, p)).collect::<Vec<_>>());
    partition(&parsed, source);
}

fn partition(parsed: &Parse, source: &str) {
    let mut end = 0;
    for id in 0..parsed.tree.len() as u32 {
        let r = parsed.tree.text_range(id);
        let (start, next) = (u32::from(r.start()) as usize, u32::from(r.end()) as usize);
        assert!(source.is_char_boundary(start) && source.is_char_boundary(next));
        if parsed.tree.is_token(id) {
            assert_eq!(start, end);
            end = next;
        }
    }
    assert_eq!(end, source.len());
}

#[test]
fn top_member_local_original_boundaries() {
    for newline in ["\n"] {
        for (prefix, suffix, enclosing) in [
            ("", "", None),
            ("class C { ", "}", Some(CLASS)),
            ("fun outer() { ", "}", Some(FUN)),
        ] {
            let broken = "fun broken(";
            let later = "val `later 🦀` = 1";
            let healthy = "fun healthy() = 2";
            let source =
                format!("{prefix}{broken}{newline}{later}{newline}{healthy}{newline}{suffix}");
            let parent = enclosing.map(|_| source.as_str());
            let mut expected = vec![
                (FUN, broken, Some("broken"), parent),
                (PROPERTY, later, Some("`later 🦀`"), parent),
                (FUN, healthy, Some("healthy"), parent),
            ];
            if let Some(kind) = enclosing {
                expected.insert(
                    0,
                    (
                        kind,
                        &source,
                        Some(if kind == CLASS { "C" } else { "outer" }),
                        None,
                    ),
                );
            }
            check(&source, &expected, &[prefix.len() + broken.len()]);
        }
    }
}

#[test]
fn prefixes_and_repeated_recovery() {
    let source = "fun broken(x: Int,\n@A public fun healthy() = 2";
    check(
        source,
        &[
            (FUN, "fun broken(x: Int,", Some("broken"), None),
            (FUN, "@A public fun healthy() = 2", Some("healthy"), None),
        ],
        &[18],
    );
    let source = "fun broken(\nfun broken2(\nclass Later";
    check(
        source,
        &[
            (FUN, "fun broken(", Some("broken"), None),
            (FUN, "fun broken2(", Some("broken2"), None),
            (CLASS, "class Later", Some("Later"), None),
        ],
        &[11, 24],
    );
}

#[test]
fn valid_nested_grammar_controls() {
    for source in [
        "fun <T> T.ext(\n @A x: List<Map<String, T>>,\n crossinline action: (T) -> Unit = { value -> val local = value; action(local) },\n y: Int = nested(call(1, 2), 3)\n): T = this",
        "fun f(x: () -> Int = fun(): Int { val local = 1; return local }, y: Any = object { val member = 2 }) {}",
        "fun f(x: Boolean = 1 < 2, y: Int = if (x) nested(1) else nested(2)) {}",
        "class C(val first: Int, var second: String) { constructor(x: Int): this(x, \"\") }",
        "fun f(x: (@A Int, String) -> Unit) {}",
        "fun f(\nvararg x: String,\n@A y: Int = run { fun local() { }; local() },\n) {}",
    ] {
        let parsed = parse_file(source, FileKind::Source);
        assert!(
            parsed.error_messages.is_empty(),
            "{source:?}: {:?}",
            parsed.error_messages
        );
        assert!(parsed.missed_tokens.is_empty());
        assert_eq!(parsed.tree.text(), source);
        partition(&parsed, source);
    }
}

#[test]
fn member_initializers_constructors_and_soft_parameter_names() {
    let source =
        "class C { fun broken(\ninit { val local = 1 }\nconstructor() {}\nfun healthy() = 2\n}";
    let parsed = parse_file(source, FileKind::Source);
    let tree = &parsed.tree;
    for (kind, text) in [
        (CLASS_INITIALIZER, "init { val local = 1 }"),
        (SECONDARY_CONSTRUCTOR, "constructor() {}"),
        (FUN, "fun healthy() = 2"),
    ] {
        let start = source.find(text).unwrap();
        let id = (0..tree.len() as u32)
            .find(|&id| {
                tree.kind(id) == kind && u32::from(tree.text_range(id).start()) as usize == start
            })
            .unwrap();
        assert_eq!(
            u32::from(tree.text_range(id).end()) as usize,
            start + text.len()
        );
        assert_eq!(tree.kind(tree.parent(id).unwrap()), CLASS_BODY);
    }
    assert_eq!(parsed.error_messages, ["Expecting ')'"]);
    partition(&parsed, source);
    let source = "fun f(init: Int, constructor: String, public: Int, context: Int) {}";
    let parsed = parse_file(source, FileKind::Source);
    assert!(parsed.error_messages.is_empty());
    partition(&parsed, source);
}

#[test]
fn annotation_probes_do_not_recursively_replay_recovery() {
    let mut parameter = "x: Int".to_string();
    for _ in 0..20 {
        parameter = format!("@A(fun({parameter}) {{}}) x: Int");
    }
    let source = format!("fun f({parameter}) {{}}");
    let parsed = parse_file(&source, FileKind::Source);
    assert!(parsed.error_messages.is_empty());
    partition(&parsed, &source);
}

#[test]
fn malformed_eof_keeps_upstream_diagnostics() {
    let source = "fun broken(";
    let parsed = parse_file(source, FileKind::Source);
    assert_eq!(
        parsed.error_messages,
        [
            "Parameter name expected",
            "Expecting comma or ')'",
            "Expecting ')'"
        ]
    );
    for id in 0..parsed.tree.len() as u32 {
        if parsed.tree.kind(id) == ERROR_ELEMENT {
            let r = parsed.tree.text_range(id);
            assert_eq!(
                (u32::from(r.start()) as usize, u32::from(r.end()) as usize),
                (source.len(), source.len())
            );
        }
    }
    partition(&parsed, source);
}

#[test]
fn classifier_generic_recovery_retains_original_sibling_identity() {
    for (broken, expected_error) in [
        ("class Box<T", "Missing '>'"),
        ("interface Box<T", "Missing '>'"),
        ("class Box<", "Type parameter declaration expected"),
        ("class Box<T,", "Type parameter declaration expected"),
        ("class Box<T : Map<String, List<Int>>", "Missing '>'"),
    ] {
        let source = format!("{broken}\n@A public fun healthy() = 2");
        let parsed = parse_file(&source, FileKind::Source);
        assert_eq!(parsed.error_messages, [expected_error]);
        let tree = &parsed.tree;
        let declarations: Vec<_> = (0..tree.len() as u32)
            .filter(|&id| declaration(tree.kind(id)))
            .collect();
        assert_eq!(declarations.len(), 2);
        for (&id, (kind, start, end)) in declarations.iter().zip([
            (CLASS, 0, broken.len()),
            (FUN, broken.len() + 1, source.len()),
        ]) {
            assert_eq!(tree.kind(id), kind);
            let range = tree.text_range(id);
            assert_eq!(
                (
                    u32::from(range.start()) as usize,
                    u32::from(range.end()) as usize
                ),
                (start, end)
            );
            assert_eq!(tree.kind(tree.parent(id).unwrap()), KT_FILE);
        }
        let fun = declarations[1];
        let name_start = broken.len() + 15;
        assert!(
            (fun + 1..tree.subtree_end(fun)).any(|id| tree.parent(id) == Some(fun)
                && tree.kind(id) == IDENTIFIER
                && u32::from(tree.text_range(id).start()) as usize == name_start
                && u32::from(tree.text_range(id).end()) as usize == name_start + 7)
        );
        partition(&parsed, &source);
    }
}
