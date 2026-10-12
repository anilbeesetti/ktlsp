//! Builder behaviour on hand-made token streams (no lexer needed).

mod binding;
mod semantic;

use ktrs_lexer::Token;
use ktrs_syntax::{SyntaxKind, psi_dump};

use super::{LazyLeaf, PsiBuilder, TreeSink};

fn builder(tokens: &[(SyntaxKind, &str)]) -> PsiBuilder {
    let text: String = tokens.iter().map(|(_, t)| *t).collect();
    let tokens: Vec<Token> = tokens.iter().map(|&(kind, t)| Token { kind, len: t.len() as u32 }).collect();
    PsiBuilder::new(&text, &tokens)
}

fn no_lazy(_: &LazyLeaf<'_>, _: &mut TreeSink) -> bool {
    false
}

fn dump(b: &mut PsiBuilder) -> String {
    psi_dump(&b.get_tree_built(&no_lazy), "test.kt")
}
