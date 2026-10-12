use expect_test::expect;
use ktrs_syntax::SyntaxKind::{self, *};
use ktrs_syntax::psi_dump;

use super::{builder, dump};
use crate::builder::{EdgeBinder, LazyLeaf, TreeSink};

#[test]
fn default_edges_exclude_surrounding_trivia() {
    let mut b = builder(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (BLOCK_COMMENT, "/*c*/"), (WHITE_SPACE, " "), (IDENTIFIER, "b"), (WHITE_SPACE, " ")]);
    let root = b.mark();
    b.advance_lexer();
    let m = b.mark();
    b.advance_lexer();
    m.done(&mut b, CLASS);
    assert!(b.eof());
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('a')
          PsiWhiteSpace(' ')
          PsiComment(BLOCK_COMMENT)('/*c*/')
          PsiWhiteSpace(' ')
          CLASS
            PsiElement(IDENTIFIER)('b')
          PsiWhiteSpace(' ')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn empty_marker_lands_after_whitespace() {
    let mut b = builder(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b")]);
    let root = b.mark();
    b.advance_lexer();
    let m = b.mark();
    m.done(&mut b, VALUE_ARGUMENT_LIST);
    b.advance_lexer();
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('a')
          PsiWhiteSpace(' ')
          VALUE_ARGUMENT_LIST
            <empty list>
          PsiElement(IDENTIFIER)('b')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn empty_left_bound_marker_binds_left() {
    let mut b = builder(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b")]);
    let root = b.mark();
    b.advance_lexer();
    let m = b.mark();
    m.error(&mut b, "empty");
    b.advance_lexer();
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('a')
          PsiErrorElement:empty
            <empty list>
          PsiWhiteSpace(' ')
          PsiElement(IDENTIFIER)('b')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn error_items_bind_left_and_dedup_per_lexeme() {
    let mut b = builder(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b")]);
    let root = b.mark();
    b.advance_lexer();
    b.error("first");
    b.error("second"); // same lexeme, right after: ignored at once
    assert!(!b.eof()); // skips the whitespace
    b.error("third"); // balanced back onto the lexeme of "first": dropped when building
    b.advance_lexer();
    root.done(&mut b, KT_FILE);
    let parse = b.get_tree_built(&super::no_lazy);
    assert_eq!(parse.error_messages, ["first"]);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('a')
          PsiErrorElement:first
            <empty list>
          PsiWhiteSpace(' ')
          PsiElement(IDENTIFIER)('b')"#]]
    .assert_eq(&psi_dump(&parse, "test.kt"));
}

#[test]
fn declaration_comment_binders() {
    let mut b = builder(&[
        (IDENTIFIER, "x"),
        (WHITE_SPACE, "\n\n"),
        (EOL_COMMENT, "// a"),
        (WHITE_SPACE, "\n"),
        (EOL_COMMENT, "// b"),
        (WHITE_SPACE, "\n"),
        (FUN_KEYWORD, "fun"),
        (WHITE_SPACE, " "),
        (EOL_COMMENT, "// t"),
        (WHITE_SPACE, "\n"),
        (IDENTIFIER, "y"),
    ]);
    let root = b.mark();
    b.advance_lexer();
    let m = b.mark();
    b.advance_lexer();
    m.done(&mut b, FUN);
    m.set_custom_edge_token_binders(&mut b, Some(EdgeBinder::PrecedingComments), Some(EdgeBinder::TrailingComments));
    b.advance_lexer();
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          PsiElement(IDENTIFIER)('x')
          PsiWhiteSpace('\n\n')
          FUN
            PsiComment(EOL_COMMENT)('// a')
            PsiWhiteSpace('\n')
            PsiComment(EOL_COMMENT)('// b')
            PsiWhiteSpace('\n')
            PsiElement(fun)('fun')
            PsiWhiteSpace(' ')
            PsiComment(EOL_COMMENT)('// t')
          PsiWhiteSpace('\n')
          PsiElement(IDENTIFIER)('y')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn preceding_doc_comments_binder_takes_last_doc_and_after() {
    let mut b = builder(&[
        (DOC_COMMENT, "/** d */"),
        (WHITE_SPACE, "\n"),
        (EOL_COMMENT, "// e"),
        (WHITE_SPACE, "\n"),
        (CLASS_KEYWORD, "class"),
    ]);
    let root = b.mark();
    let m = b.mark();
    b.advance_lexer();
    m.done(&mut b, CLASS);
    m.set_custom_edge_token_binders(&mut b, Some(EdgeBinder::PrecedingDocComments), None);
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          CLASS
            PsiElement(KDoc)('/** d */')
            PsiWhiteSpace('\n')
            PsiComment(EOL_COMMENT)('// e')
            PsiWhiteSpace('\n')
            PsiElement(class)('class')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn precede_rollback_drop_collapse() {
    let mut b = builder(&[(IDENTIFIER, "a"), (DOT, "."), (IDENTIFIER, "b"), (WHITE_SPACE, " "), (IDENTIFIER, "c")]);
    let root = b.mark();
    let m1 = b.mark();
    b.advance_lexer();
    m1.done(&mut b, REFERENCE_EXPRESSION);
    let m2 = m1.precede(&mut b);
    b.advance_lexer();
    let m3 = b.mark();
    b.advance_lexer();
    m3.done(&mut b, REFERENCE_EXPRESSION);
    m2.done(&mut b, DOT_QUALIFIED_EXPRESSION);
    let r = b.mark();
    b.advance_lexer();
    b.error("rolled back");
    r.rollback_to(&mut b);
    let d = b.mark();
    d.drop(&mut b);
    let x = b.mark();
    b.advance_lexer();
    x.collapse(&mut b, FIELD_IDENTIFIER);
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          DOT_QUALIFIED_EXPRESSION
            REFERENCE_EXPRESSION
              PsiElement(IDENTIFIER)('a')
            PsiElement(DOT)('.')
            REFERENCE_EXPRESSION
              PsiElement(IDENTIFIER)('b')
          PsiWhiteSpace(' ')
          PsiElement(FIELD_IDENTIFIER)('c')"#]]
    .assert_eq(&dump(&mut b));
}

#[test]
fn done_before_variants() {
    let mut b = builder(&[(IDENTIFIER, "a"), (WHITE_SPACE, " "), (IDENTIFIER, "b"), (WHITE_SPACE, " "), (IDENTIFIER, "c")]);
    let root = b.mark();
    let m = b.mark();
    b.advance_lexer();
    let n = b.mark();
    m.done_before_3(&mut b, VALUE_ARGUMENT, n, "before n");
    b.advance_lexer();
    let o = b.mark();
    n.error_before(&mut b, "n", o);
    b.advance_lexer();
    o.done(&mut b, VALUE_ARGUMENT);
    root.done(&mut b, KT_FILE);
    expect![[r#"
        KtFile: test.kt
          VALUE_ARGUMENT
            PsiElement(IDENTIFIER)('a')
            PsiErrorElement:before n
              <empty list>
          PsiWhiteSpace(' ')
          PsiErrorElement:n
            PsiElement(IDENTIFIER)('b')
          PsiWhiteSpace(' ')
          VALUE_ARGUMENT
            PsiElement(IDENTIFIER)('c')"#]]
    .assert_eq(&dump(&mut b));
}

/// A collapsed lazy node is replaced by its reparse; error messages stay in tree preorder.
#[test]
fn lazy_leaf_is_reparsed_and_spliced() {
    const BLOCK_TOKENS: &[(SyntaxKind, &str)] =
        &[(LBRACE, "{"), (WHITE_SPACE, " "), (IDENTIFIER, "x"), (WHITE_SPACE, " "), (RBRACE, "}")];
    fn reparse(leaf: &LazyLeaf<'_>, sink: &mut TreeSink) -> bool {
        if leaf.kind != BLOCK {
            return false;
        }
        assert_eq!(leaf.text, "{ x }");
        let mut b = builder(BLOCK_TOKENS);
        let root = b.mark();
        b.advance_lexer();
        b.error("inner");
        while !b.eof() {
            b.advance_lexer();
        }
        root.done(&mut b, BLOCK);
        b.build_tree_into(Some(BLOCK), sink, &reparse);
        true
    }

    let mut tokens = vec![(IDENTIFIER, "f"), (WHITE_SPACE, " ")];
    tokens.extend_from_slice(BLOCK_TOKENS);
    tokens.extend([(WHITE_SPACE, " "), (IDENTIFIER, "g")]);
    let mut b = builder(&tokens);
    let root = b.mark();
    b.error("before");
    b.advance_lexer();
    let block = b.mark();
    for _ in 0..3 {
        b.advance_lexer(); // { x }
    }
    block.collapse(&mut b, BLOCK);
    b.error("after");
    b.advance_lexer();
    root.done(&mut b, KT_FILE);
    let parse = b.get_tree_built(&reparse);
    assert_eq!(parse.error_messages, ["before", "inner", "after"]);
    expect![[r#"
        KtFile: test.kt
          PsiErrorElement:before
            <empty list>
          PsiElement(IDENTIFIER)('f')
          PsiWhiteSpace(' ')
          BLOCK
            PsiElement(LBRACE)('{')
            PsiErrorElement:inner
              <empty list>
            PsiWhiteSpace(' ')
            PsiElement(IDENTIFIER)('x')
            PsiWhiteSpace(' ')
            PsiElement(RBRACE)('}')
          PsiErrorElement:after
            <empty list>
          PsiWhiteSpace(' ')
          PsiElement(IDENTIFIER)('g')"#]]
    .assert_eq(&psi_dump(&parse, "test.kt"));
}
