//! [`PsiBuilder`] construction from per-thread recycled vectors. A file plus its chameleons
//! create one builder per lazy node; fresh (often multi-megabyte) vectors per builder cost more
//! in allocation and page faults than parsing a short block does, while recycled ones are warm.

use std::cell::RefCell;

use ktrs_lexer::Token;
use ktrs_syntax::SyntaxKind;

use super::production::{Production, ProductionVecs};
use super::psi_builder::PsiBuilder;

/// Enough for any realistic chameleon nesting depth; deeper builders just allocate.
const MAX_POOLED: usize = 16;
/// Don't keep buffers of huge files (~650 KB of source and up) alive for the rest of the thread.
const MAX_POOLED_LEXEMES: usize = 1 << 18;

thread_local! {
    static POOL: RefCell<Vec<Buffers>> = const { RefCell::new(Vec::new()) };
}

#[derive(Default)]
struct Buffers {
    text: String,
    lex_starts: Vec<u32>,
    lex_types: Vec<SyntaxKind>,
    orig_types: Vec<SyntaxKind>,
    production: ProductionVecs,
    skipped_errors: Vec<bool>,
}

impl Buffers {
    fn take() -> Buffers {
        POOL.with(|pool| pool.borrow_mut().pop()).unwrap_or_default()
    }

    fn give_back(mut self) {
        if self.lex_types.capacity() > MAX_POOLED_LEXEMES || self.production.markers.capacity() > 4 * MAX_POOLED_LEXEMES {
            return;
        }
        self.text.clear();
        self.lex_starts.clear();
        self.lex_types.clear();
        self.orig_types.clear();
        self.production.clear();
        self.skipped_errors.clear();
        POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < MAX_POOLED {
                pool.push(self);
            }
        });
    }
}

impl PsiBuilder {
    /// `tokens` must tile `text` exactly (lengths sum to `text.len()`).
    pub fn new(text: &str, tokens: &[Token]) -> PsiBuilder {
        PsiBuilder::from_tokens(text, tokens.iter().copied())
    }

    /// A builder over `text` lexed by the Kotlin lexer.
    pub(crate) fn lex_kotlin(text: &str) -> PsiBuilder {
        PsiBuilder::from_tokens(text, ktrs_lexer::tokens_of(text))
    }

    fn from_tokens(text: &str, tokens: impl Iterator<Item = Token>) -> PsiBuilder {
        let mut buffers = Buffers::take();
        let mut offset = 0u32;
        for token in tokens {
            buffers.lex_starts.push(offset);
            buffers.lex_types.push(token.kind);
            offset += token.len;
        }
        buffers.lex_starts.push(offset);
        PsiBuilder::from_buffers(text, buffers)
    }

    /// A builder over lexemes cut from another builder: `starts` has one more entry than
    /// `kinds` and is rebased so that `starts[0]` becomes offset 0 of `text`.
    pub(crate) fn from_lexemes(text: &str, starts: &[u32], kinds: &[SyntaxKind]) -> PsiBuilder {
        let mut buffers = Buffers::take();
        let base = starts[0];
        buffers.lex_starts.extend(starts.iter().map(|s| s - base));
        buffers.lex_types.extend_from_slice(kinds);
        PsiBuilder::from_buffers(text, buffers)
    }

    fn from_buffers(text: &str, buffers: Buffers) -> PsiBuilder {
        let Buffers { text: mut own_text, lex_starts, lex_types, mut orig_types, production, skipped_errors } = buffers;
        assert_eq!(lex_starts.last().copied(), Some(text.len() as u32), "token lengths must sum to the text length");
        own_text.push_str(text);
        orig_types.extend_from_slice(&lex_types);
        PsiBuilder {
            text: own_text,
            lex_starts,
            lex_types,
            orig_types,
            current_lexeme: 0,
            token_type_checked: false,
            production: Production::from_vecs(production),
            skipped_errors,
        }
    }
}

impl Drop for PsiBuilder {
    fn drop(&mut self) {
        let production = self.production.take_vecs();
        Buffers {
            text: std::mem::take(&mut self.text),
            lex_starts: std::mem::take(&mut self.lex_starts),
            lex_types: std::mem::take(&mut self.lex_types),
            orig_types: std::mem::take(&mut self.orig_types),
            production,
            skipped_errors: std::mem::take(&mut self.skipped_errors),
        }
        .give_back();
    }
}
