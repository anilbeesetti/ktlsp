//! Ports of `KDocParser.java` / `KDocLinkParser.kt` and the `parseContents` of their lazy element
//! types (`KotlinElementTypeProviderImpl.kdocType` / `kdocMarkdownLinkType`). Both sub-builders use
//! the Kotlin language definition, so whitespace/comment sets are Kotlin's. Nested lazy leaves
//! (`KDOC_MARKDOWN_LINK` inside a doc, `DOC_COMMENT` inside a link) are expanded by `reparse_lazy`.

mod kdoc_known_tag;
mod kdoc_link_parser;
mod kdoc_parser;

#[cfg(test)]
mod tests;

use ktrs_syntax::SyntaxKind;

use crate::builder::{PsiBuilder, TreeSink};
use crate::parsing::kotlin_parser::reparse_lazy;

/// The KDoc lexer + `KDocParser` over a `DOC_COMMENT`'s text, unbuilt.
fn kdoc_builder(text: &str) -> PsiBuilder {
    let tokens = ktrs_lexer::tokenize_kdoc(text);
    let mut builder = PsiBuilder::new(text, &tokens);
    kdoc_parser::parse(SyntaxKind::DOC_COMMENT, &mut builder);
    builder
}

#[cfg(test)]
fn parse_kdoc(text: &str) -> ktrs_syntax::Parse {
    kdoc_builder(text).get_tree_built(&reparse_lazy)
}

/// `kdocType.parseContents`.
pub(crate) fn parse_kdoc_into(text: &str, sink: &mut TreeSink) {
    kdoc_builder(text).build_tree_into(Some(SyntaxKind::DOC_COMMENT), sink, &reparse_lazy);
}

/// `KDocLinkParser.parseMarkdownLink`: the Kotlin lexer + `KDocLinkParser` over a link's text.
pub(crate) fn parse_markdown_link_into(text: &str, sink: &mut TreeSink) {
    let mut builder = PsiBuilder::lex_kotlin(text);
    kdoc_link_parser::parse(SyntaxKind::KDOC_MARKDOWN_LINK, &mut builder);
    builder.build_tree_into(Some(SyntaxKind::KDOC_MARKDOWN_LINK), sink, &reparse_lazy);
}
