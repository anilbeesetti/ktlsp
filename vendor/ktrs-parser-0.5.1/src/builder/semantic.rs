//! Kotlin's builder wrappers as one struct: `SemanticWhitespaceAwarePsiBuilderImpl` (newline and
//! complex-token-joining stacks) here, the Truncated/ForByClause adapters in `layers.rs`.
//!
//! Java dispatch is reproduced exactly: the Impl's own methods (`advanceLexer`,
//! `newlineBeforeCurrentToken`, joined lookups) call the Impl's `getTokenType`/`eof`, never the
//! truncated overrides; only calls made by parser code go through the layers.

use ktrs_syntax::SyntaxKind::{self, *};

use super::layers::Layer;
use super::marker::{Marker, MarkerHost};
use super::psi_builder::PsiBuilder;

pub struct SemanticWhitespaceAwarePsiBuilder {
    pub psi: PsiBuilder,
    join_complex_tokens: BoolStack,
    newlines_enabled: BoolStack,
    pub(super) layers: Vec<Layer>,
    /// `impl_get_token_type` memo: its result at post-whitespace lexeme `tt_lexeme` (`usize::MAX`
    /// = none). It depends only on that lexeme's kind and the next one's (changed only by remaps of
    /// the current token) and the joining flag, so those clear it; any cursor move changes the key.
    tt_lexeme: usize,
    tt: Option<SyntaxKind>,
}

/// A `Stack<Boolean>` whose top, read on every token query, lives inline.
struct BoolStack {
    top: Option<bool>,
    below: Vec<bool>,
}

impl BoolStack {
    fn new(initial: bool) -> BoolStack {
        BoolStack { top: Some(initial), below: Vec::new() }
    }

    fn push(&mut self, value: bool) {
        if let Some(top) = self.top.replace(value) {
            self.below.push(top);
        }
    }

    fn pop(&mut self) {
        self.top = self.below.pop();
    }

    fn peek(&self) -> bool {
        self.top.expect("empty stack")
    }

    fn len(&self) -> usize {
        self.below.len() + usize::from(self.top.is_some())
    }
}

impl MarkerHost for SemanticWhitespaceAwarePsiBuilder {
    fn psi_builder(&mut self) -> &mut PsiBuilder {
        &mut self.psi
    }
}

/// `complexTokens`.
fn is_complex_token(kind: Option<SyntaxKind>) -> bool {
    matches!(kind, Some(SAFE_ACCESS | ELVIS | EXCLEXCL))
}

impl SemanticWhitespaceAwarePsiBuilder {
    pub fn new(psi: PsiBuilder) -> Self {
        SemanticWhitespaceAwarePsiBuilder {
            psi,
            join_complex_tokens: BoolStack::new(true),
            newlines_enabled: BoolStack::new(true),
            layers: Vec::new(),
            tt_lexeme: usize::MAX,
            tt: None,
        }
    }

    pub fn is_whitespace_or_comment(&self, kind: SyntaxKind) -> bool {
        self.psi.is_whitespace_or_comment(kind)
    }

    pub fn newline_before_current_token(&mut self) -> bool {
        if !self.newlines_enabled.peek() {
            return false;
        }

        if self.psi.eof() {
            return true;
        }

        // Upstream walks back `i` lexemes while `i <= offset`; trivia is never empty, so only the
        // start of the input can stop it first. `prev` is the lexeme `i` back.
        let psi = &self.psi;
        let mut prev = psi.current_lexeme;
        while prev > 0 {
            prev -= 1;
            match psi.lex_types[prev] {
                BLOCK_COMMENT | DOC_COMMENT | EOL_COMMENT | SHEBANG_COMMENT => continue,
                WHITE_SPACE => {
                    if psi.token_text(prev).as_bytes().contains(&b'\n') {
                        return true;
                    }
                }
                _ => break,
            }
        }

        false
    }

    pub fn disable_newlines(&mut self) {
        self.newlines_enabled.push(false);
        self.count_newlines_call(1);
    }

    pub fn enable_newlines(&mut self) {
        self.newlines_enabled.push(true);
        self.count_newlines_call(1);
    }

    pub fn restore_newlines_state(&mut self) {
        assert!(self.newlines_enabled.len() > 1);
        self.newlines_enabled.pop();
        self.count_newlines_call(-1);
    }

    fn join_complex_tokens(&self) -> bool {
        self.join_complex_tokens.peek()
    }

    pub fn restore_joining_complex_tokens_state(&mut self) {
        self.join_complex_tokens.pop();
        self.tt_lexeme = usize::MAX;
    }

    pub fn enable_joining_complex_tokens(&mut self) {
        self.join_complex_tokens.push(true);
        self.tt_lexeme = usize::MAX;
    }

    pub fn disable_joining_complex_tokens(&mut self) {
        self.join_complex_tokens.push(false);
        self.tt_lexeme = usize::MAX;
    }

    /// The Impl's `getTokenType` (no truncation), memoized (see `tt_lexeme`). Returning the memo
    /// without `eof()`'s skip is fine: the key is a lexeme that skip leaves in place.
    #[inline]
    pub(super) fn impl_get_token_type(&mut self) -> Option<SyntaxKind> {
        if self.tt_lexeme == self.psi.current_lexeme {
            return self.tt;
        }
        self.impl_get_token_type_uncached()
    }

    #[inline(never)]
    fn impl_get_token_type_uncached(&mut self) -> Option<SyntaxKind> {
        let raw = self.psi.get_token_type();
        let tt = if self.join_complex_tokens() { self.get_joined_token_type(raw, 1) } else { raw };
        (self.tt_lexeme, self.tt) = (self.psi.current_lexeme, tt);
        tt
    }

    #[inline]
    fn get_joined_token_type(&self, raw_token_type: Option<SyntaxKind>, raw_lookup_steps: i32) -> Option<SyntaxKind> {
        match raw_token_type {
            Some(QUEST) => match self.psi.raw_lookup(raw_lookup_steps) {
                Some(DOT) => return Some(SAFE_ACCESS),
                Some(COLON) => return Some(ELVIS),
                _ => {}
            },
            Some(EXCL) if self.psi.raw_lookup(raw_lookup_steps) == Some(EXCL) => return Some(EXCLEXCL),
            _ => {}
        }
        raw_token_type
    }

    pub fn advance_lexer(&mut self) {
        if !self.join_complex_tokens() {
            self.psi.advance_lexer();
            return;
        }
        let token_type = self.impl_get_token_type();
        if is_complex_token(token_type) {
            let mark = self.psi.mark();
            self.psi.advance_lexer();
            self.psi.advance_lexer();
            mark.collapse(&mut self.psi, token_type.unwrap());
        } else {
            self.psi.advance_lexer();
        }
    }

    /// The Impl's `getTokenText` (no truncation).
    pub(super) fn impl_get_token_text(&mut self) -> Option<&str> {
        if self.join_complex_tokens() {
            match self.impl_get_token_type() {
                Some(ELVIS) => return Some("?:"),
                Some(SAFE_ACCESS) => return Some("?."),
                _ => {}
            }
        }
        self.psi.get_token_text()
    }

    /// The Impl's `lookAhead` (no truncation).
    pub(super) fn impl_look_ahead(&mut self, steps: i32) -> Option<SyntaxKind> {
        if !self.join_complex_tokens() {
            return self.psi.look_ahead(steps);
        }

        if is_complex_token(self.impl_get_token_type()) {
            return self.psi.look_ahead(steps + 1);
        }
        let raw = self.psi.look_ahead(steps);
        // Upstream checks the raw token 2 after the *current* one, whatever `steps` is.
        self.get_joined_token_type(raw, 2)
    }

    // ---- Plain delegation to PsiBuilderImpl ----

    pub fn mark(&mut self) -> Marker {
        self.psi.mark()
    }

    pub fn error(&mut self, message: &str) {
        self.psi.error(message);
    }

    pub fn raw_lookup(&self, steps: i32) -> Option<SyntaxKind> {
        self.psi.raw_lookup(steps)
    }

    pub fn raw_token_type_start(&self, steps: i32) -> i32 {
        self.psi.raw_token_type_start(steps)
    }

    pub fn raw_token_index(&self) -> i32 {
        self.psi.raw_token_index()
    }

    pub fn get_current_offset(&mut self) -> i32 {
        self.psi.get_current_offset()
    }

    pub fn remap_current_token(&mut self, kind: SyntaxKind) {
        self.psi.remap_current_token(kind);
        self.tt_lexeme = usize::MAX;
    }

    pub fn get_original_text(&self) -> &str {
        self.psi.get_original_text()
    }
}
