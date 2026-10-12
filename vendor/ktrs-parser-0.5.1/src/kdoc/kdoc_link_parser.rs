//! Port of `kdoc/parser/KDocLinkParser.kt`: parses a `[...]` Markdown link with the Kotlin lexer.

use ktrs_syntax::SyntaxKind::{self, *};

use crate::builder::PsiBuilder;
use crate::kt_tokens::KEYWORDS;

/// `KDocLinkParser.parse` minus `getTreeBuilt` (the caller builds the tree).
pub(crate) fn parse(root: SyntaxKind, builder: &mut PsiBuilder) {
    let root_marker = builder.mark();
    let has_l_bracket = builder.get_token_type() == Some(LBRACKET);
    if has_l_bracket {
        builder.advance_lexer();
    }
    parse_qualified_name(builder);
    if has_l_bracket {
        if !builder.eof() && builder.get_token_type() != Some(RBRACKET) {
            builder.error("Closing bracket expected");
            while !builder.eof() && builder.get_token_type() != Some(RBRACKET) {
                builder.advance_lexer();
            }
        }
        if builder.get_token_type() == Some(RBRACKET) {
            builder.advance_lexer();
        }
    } else if !builder.eof() {
        builder.error("Expression expected");
        while !builder.eof() {
            builder.advance_lexer();
        }
    }
    root_marker.done(builder, root);
}

fn parse_qualified_name(builder: &mut PsiBuilder) {
    let mut marker = builder.mark();
    loop {
        if !is_name(builder.get_token_type()) {
            marker.drop(builder);
            builder.error("Identifier expected");
            break;
        }
        builder.advance_lexer();
        marker.done(builder, KDOC_NAME);
        if builder.get_token_type() != Some(DOT) {
            break;
        }
        marker = marker.precede(builder);
        builder.advance_lexer();
    }
}

fn is_name(token_type: Option<SyntaxKind>) -> bool {
    token_type == Some(IDENTIFIER) || KEYWORDS.contains(token_type)
}
