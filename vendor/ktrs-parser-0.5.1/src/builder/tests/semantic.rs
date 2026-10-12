use expect_test::expect;
use ktrs_syntax::SyntaxKind::{self, *};
use ktrs_syntax::psi_dump;

use super::builder;
use crate::builder::{Layer, SemanticWhitespaceAwarePsiBuilder};

fn semantic(tokens: &[(SyntaxKind, &str)]) -> SemanticWhitespaceAwarePsiBuilder {
    SemanticWhitespaceAwarePsiBuilder::new(builder(tokens))
}

/// Walks to eof, recording what the parser would see at each step.
fn walk(b: &mut SemanticWhitespaceAwarePsiBuilder) -> Vec<(SyntaxKind, String)> {
    let mut seen = Vec::new();
    while !b.eof() {
        seen.push((b.get_token_type().unwrap(), b.get_token_text().unwrap().to_owned()));
        b.advance_lexer();
    }
    seen
}

#[test]
fn complex_tokens_are_joined_and_collapsed() {
    let mut b = semantic(&[
        (IDENTIFIER, "a"),
        (QUEST, "?"),
        (DOT, "."),
        (IDENTIFIER, "b"),
        (WHITE_SPACE, " "),
        (QUEST, "?"),
        (COLON, ":"),
        (IDENTIFIER, "c"),
        (EXCL, "!"),
        (EXCL, "!"),
    ]);
    let root = b.mark();
    assert_eq!(b.look_ahead(1), Some(SAFE_ACCESS));
    let seen = walk(&mut b);
    root.done(&mut b, KT_FILE);
    expect![[r#"
        [
            (
                IDENTIFIER,
                "a",
            ),
            (
                SAFE_ACCESS,
                "?.",
            ),
            (
                IDENTIFIER,
                "b",
            ),
            (
                ELVIS,
                "?:",
            ),
            (
                IDENTIFIER,
                "c",
            ),
            (
                EXCLEXCL,
                "!",
            ),
        ]
    "#]]
    .assert_debug_eq(&seen);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('a')
          PsiElement(SAFE_ACCESS)('?.')
          PsiElement(IDENTIFIER)('b')
          PsiWhiteSpace(' ')
          PsiElement(ELVIS)('?:')
          PsiElement(IDENTIFIER)('c')
          PsiElement(EXCLEXCL)('!!')"#]]
    .assert_eq(&psi_dump(&b.psi.get_tree_built(&super::no_lazy), "test.kt"));
}

#[test]
fn joining_can_be_disabled() {
    let mut b = semantic(&[(QUEST, "?"), (DOT, ".")]);
    b.disable_joining_complex_tokens();
    assert_eq!(b.get_token_type(), Some(QUEST));
    b.restore_joining_complex_tokens_state();
    assert_eq!(b.get_token_type(), Some(SAFE_ACCESS));
}

#[test]
fn newline_before_current_token_skips_comments() {
    let mut b = semantic(&[(IDENTIFIER, "a"), (WHITE_SPACE, "\n"), (BLOCK_COMMENT, "/**/"), (IDENTIFIER, "b")]);
    assert!(!b.newline_before_current_token());
    b.advance_lexer();
    assert!(b.newline_before_current_token());
    b.disable_newlines();
    assert!(!b.newline_before_current_token());
    b.restore_newlines_state();
    b.advance_lexer();
    assert!(b.newline_before_current_token(), "eof counts as a newline");

    let mut b = semantic(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b")]);
    b.advance_lexer();
    assert!(!b.newline_before_current_token());
}

#[test]
fn truncated_layer_hides_tokens_from_eof_position() {
    let mut b = semantic(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b"), (WHITE_SPACE, " "), (IDENTIFIER, "c")]);
    b.push_layer(Layer::Truncated { eof_position: 4 });
    assert_eq!(b.look_ahead(1), Some(IDENTIFIER));
    assert_eq!(b.look_ahead(2), None);
    let seen: Vec<_> = walk(&mut b).into_iter().map(|(_, text)| text).collect();
    assert_eq!(seen, ["a", "b"]);
    assert_eq!(b.get_token_type(), None);
    b.pop_layer();
    assert_eq!(b.get_token_text(), Some("c"));
}

#[test]
fn by_clause_layers_count_newline_state_changes() {
    let mut b = semantic(&[(IDENTIFIER, "a")]);
    let outer = b.push_layer(Layer::ForByClause { stack_size: 0 });
    b.enable_newlines();
    let inner = b.push_layer(Layer::ForByClause { stack_size: 0 });
    b.disable_newlines();
    assert_eq!((b.for_by_clause_stack_size(outer), b.for_by_clause_stack_size(inner)), (2, 1));
    b.restore_newlines_state();
    b.pop_layer();
    b.restore_newlines_state();
    assert_eq!(b.for_by_clause_stack_size(outer), 0);
}
