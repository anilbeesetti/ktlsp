//! Port of `KotlinParser.java` plus the chameleon (`ILazyParseableElementType.parseContents`)
//! reparse used when building trees.

use ktrs_syntax::{Parse, SyntaxKind};

use super::Parser;
use crate::FileKind;
use crate::builder::{ChameleonCache, LazyLeaf, PsiBuilder, SemanticWhitespaceAwarePsiBuilder, TreeSink};

/// `KotlinParser.parse(psiBuilder, psiFile)`: `parseFile` or `parseScript` by file kind.
pub fn parse(text: &str, kind: FileKind) -> Parse {
    run_top_level(text, |kt_parsing| parse_by_kind(kt_parsing, kind))
}

/// [`parse`], reusing and extending `cache`'s expanded chameleons.
pub fn parse_cached(text: &str, kind: FileKind, cache: &mut ChameleonCache) -> Parse {
    let psi = PsiBuilder::lex_kotlin(text);
    let mut sink = TreeSink::for_file(&psi, Some(std::mem::take(cache)));
    run_into(psi, |kt_parsing| parse_by_kind(kt_parsing, kind), None, &mut sink);
    *cache = sink.take_cache().expect("sink keeps its cache");
    sink.finish()
}

fn parse_by_kind(kt_parsing: &mut Parser, kind: FileKind) {
    match kind {
        FileKind::Source => kt_parsing.parse_file(),
        FileKind::Script => kt_parsing.parse_script(),
    }
}

pub fn parse_type_code_fragment(text: &str) -> Parse {
    run_top_level(text, Parser::parse_type_code_fragment)
}

pub fn parse_expression_code_fragment(text: &str) -> Parse {
    run_top_level(text, Parser::parse_expression_code_fragment)
}

pub fn parse_block_code_fragment(text: &str) -> Parse {
    run_top_level(text, Parser::parse_block_code_fragment)
}

/// `LambdaExpressionElementType.parseContents`: the reparse of a collapsed `LAMBDA_EXPRESSION`.
pub fn parse_lambda_expression(text: &str) -> Parse {
    run_top_level(text, Parser::parse_lambda_expression)
}

/// `BlockExpressionElementType.parseContents`: the reparse of a collapsed `BLOCK`.
pub fn parse_block_expression(text: &str) -> Parse {
    run_top_level(text, Parser::parse_block_expression)
}

/// `KotlinParsing.createForTopLevel(new SemanticWhitespaceAwarePsiBuilderImpl(psiBuilder))`, run,
/// then `psiBuilder.getTreeBuilt()`.
fn run_top_level(text: &str, parse: impl FnOnce(&mut Parser)) -> Parse {
    let psi = PsiBuilder::lex_kotlin(text);
    let mut sink = TreeSink::for_file(&psi, None);
    run_into(psi, parse, None, &mut sink);
    sink.finish()
}

fn run_into(psi: PsiBuilder, parse: impl FnOnce(&mut Parser), root_kind: Option<SyntaxKind>, sink: &mut TreeSink) {
    let mut kt_parsing = Parser::create_for_top_level(SemanticWhitespaceAwarePsiBuilder::new(psi));
    parse(&mut kt_parsing);
    kt_parsing.my_builder.psi.build_tree_into(root_kind, sink, &reparse_lazy);
}

/// Expands a lazy-parseable leaf: its kind's `parseContents` over its own text. The reparsed
/// root's children become the leaf node's children (`getFirstChildNode()` upstream).
/// Kotlin's lazy types don't `reuseCollapsedTokens`, so remaps made by the outer parse are
/// forgotten; the leaf's unremapped lexemes stand in for re-lexing its text (see `LazyLeaf`).
pub(crate) fn reparse_lazy(leaf: &LazyLeaf<'_>, sink: &mut TreeSink) -> bool {
    let (kind, text) = (leaf.kind, leaf.text);
    match kind {
        SyntaxKind::BLOCK => sink.chameleon(kind, text, |sink| {
            run_into(leaf.relexed_builder(), Parser::parse_block_expression, Some(kind), sink)
        }),
        SyntaxKind::LAMBDA_EXPRESSION => sink.chameleon(kind, text, |sink| {
            run_into(leaf.relexed_builder(), Parser::parse_lambda_expression, Some(kind), sink)
        }),
        SyntaxKind::DOC_COMMENT => sink.chameleon(kind, text, |sink| crate::kdoc::parse_kdoc_into(text, sink)),
        SyntaxKind::KDOC_MARKDOWN_LINK => {
            sink.chameleon(kind, text, |sink| crate::kdoc::parse_markdown_link_into(text, sink))
        }
        _ => return false,
    }
    true
}
