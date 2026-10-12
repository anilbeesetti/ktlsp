//! Port of `AbstractKotlinParsing.java`.

use ktrs_syntax::SyntaxKind::{self, *};

use super::Parser;
use crate::builder::{EdgeBinder, Layer, Marker};
use crate::kt_tokens::{self, WHITE_SPACE_OR_COMMENT_BIT_SET};
use crate::token_set::TokenSet;

impl Parser {
    pub(crate) fn get_last_token(&mut self) -> Option<SyntaxKind> {
        let mut i = 1;
        let current_offset = self.my_builder.get_current_offset();
        while i <= current_offset && WHITE_SPACE_OR_COMMENT_BIT_SET.contains(self.my_builder.raw_lookup(-i)) {
            i += 1;
        }
        self.my_builder.raw_lookup(-i)
    }

    pub(crate) fn expect_2(&mut self, expectation: SyntaxKind, message: &str) -> bool {
        self.expect_3(expectation, message, None)
    }

    pub(crate) fn mark(&mut self) -> Marker {
        self.my_builder.mark()
    }

    pub(crate) fn error(&mut self, message: &str) {
        self.my_builder.error(message);
    }

    pub(crate) fn expect_3(&mut self, expectation: SyntaxKind, message: &str, recovery_set: Option<TokenSet>) -> bool {
        if self.expect(expectation) {
            return true;
        }

        self.error_with_recovery(message, recovery_set);

        false
    }

    pub(crate) fn expect(&mut self, expectation: SyntaxKind) -> bool {
        if self.at(expectation) {
            self.advance(); // expectation
            return true;
        }

        if expectation == IDENTIFIER && self.my_builder.get_token_text() == Some("`") {
            self.advance();
        }

        false
    }

    pub(crate) fn expect_no_advance(&mut self, expectation: SyntaxKind, message: &str) {
        if self.at(expectation) {
            self.advance(); // expectation
            return;
        }

        self.error(message);
    }

    pub(crate) fn error_with_recovery(&mut self, message: &str, recovery_set: Option<TokenSet>) {
        let tt = self.tt();
        let recover = match recovery_set {
            None => true,
            Some(set) => {
                set.contains(tt)
                    || tt == Some(LBRACE)
                    || tt == Some(RBRACE)
                    || (set.contains(EOL_OR_SEMICOLON)
                        && (self.eof() || tt == Some(SEMICOLON) || self.my_builder.newline_before_current_token()))
            }
        };
        if recover {
            self.error(message);
        } else {
            self.error_and_advance(message);
        }
    }

    pub(crate) fn error_and_advance(&mut self, message: &str) {
        self.error_and_advance_2(message, 1);
    }

    pub(crate) fn error_and_advance_2(&mut self, message: &str, advance_token_count: i32) {
        let err = self.mark();
        self.advance_1(advance_token_count);
        err.error(self, message);
    }

    pub(crate) fn eof(&mut self) -> bool {
        self.my_builder.eof()
    }

    pub(crate) fn advance(&mut self) {
        self.my_builder.advance_lexer();
    }

    pub(crate) fn advance_1(&mut self, advance_token_count: i32) {
        for _ in 0..advance_token_count {
            self.advance(); // erroneous token
        }
    }

    pub(crate) fn advance_at(&mut self, current: SyntaxKind) {
        debug_assert!(self._at(current));
        self.my_builder.advance_lexer();
    }

    /// `tokenId` is just an int alias of the `KtToken`; the kind itself serves (None = INVALID_Id).
    pub(crate) fn get_token_id(&mut self) -> Option<SyntaxKind> {
        self.tt()
    }

    #[inline]
    pub(crate) fn tt(&mut self) -> Option<SyntaxKind> {
        self.my_builder.get_token_type()
    }

    /// Side-effect-free version of `at()`.
    #[inline]
    pub(crate) fn _at(&mut self, expectation: SyntaxKind) -> bool {
        let token = self.tt();
        self.token_matches(token, expectation)
    }

    #[inline]
    fn token_matches(&mut self, token: Option<SyntaxKind>, expectation: SyntaxKind) -> bool {
        if token == Some(expectation) {
            return true;
        }
        if expectation == EOL_OR_SEMICOLON {
            if self.eof() {
                return true;
            }
            if token == Some(SEMICOLON) {
                return true;
            }
            if self.my_builder.newline_before_current_token() {
                return true;
            }
        }
        false
    }

    #[inline]
    pub(crate) fn at(&mut self, expectation: SyntaxKind) -> bool {
        let token = self.tt();
        if token == Some(expectation) {
            return true;
        }
        // Inlined so a constant `expectation` folds this away; the rest can only match in these cases.
        if expectation != EOL_OR_SEMICOLON && expectation != IDENTIFIER && !kt_tokens::is_soft_keyword(expectation) {
            return false;
        }
        self.at_rest(token, expectation)
    }

    /// The body of upstream's `at` after its `tt()`.
    #[inline(never)]
    fn at_rest(&mut self, token: Option<SyntaxKind>, expectation: SyntaxKind) -> bool {
        // `_at(expectation)`, keeping its `tt()` for the second lookup (which can't differ).
        if self.token_matches(token, expectation) {
            return true;
        }
        if token == Some(IDENTIFIER)
            && kt_tokens::is_soft_keyword(expectation)
            && expectation.keyword_text() == self.my_builder.get_token_text()
        {
            self.my_builder.remap_current_token(expectation);
            return true;
        }
        if expectation == IDENTIFIER && token.is_some_and(kt_tokens::is_soft_keyword) {
            self.my_builder.remap_current_token(IDENTIFIER);
            return true;
        }
        false
    }

    /// Side-effect-free version of `at_set()`.
    pub(crate) fn _at_set(&mut self, set: TokenSet) -> bool {
        let token = self.tt();
        self.token_matches_set(token, set)
    }

    fn token_matches_set(&mut self, token: Option<SyntaxKind>, set: TokenSet) -> bool {
        if set.contains(token) {
            return true;
        }
        if set.contains(EOL_OR_SEMICOLON) {
            if self.eof() {
                return true;
            }
            if token == Some(SEMICOLON) {
                return true;
            }
            if self.my_builder.newline_before_current_token() {
                return true;
            }
        }
        false
    }

    #[inline]
    pub(crate) fn at_set(&mut self, set: TokenSet) -> bool {
        let token = self.tt();
        if set.contains(token) {
            return true;
        }
        // As in `at`: folds away for a constant `set`.
        if !set.contains(EOL_OR_SEMICOLON) && !set.contains(IDENTIFIER) && !set.intersects(kt_tokens::SOFT_KEYWORDS) {
            return false;
        }
        self.at_set_rest(token, set)
    }

    /// The body of upstream's `atSet` after its `tt()`.
    #[inline(never)]
    fn at_set_rest(&mut self, token: Option<SyntaxKind>, set: TokenSet) -> bool {
        // `_at_set(set)`, keeping its `tt()` for the second lookup (which can't differ).
        if self.token_matches_set(token, set) {
            return true;
        }
        if token == Some(IDENTIFIER) {
            // Only a soft keyword in `set` can match by text, so most sets skip the text lookup.
            if !set.intersects(kt_tokens::SOFT_KEYWORDS) {
                return false;
            }
            let keyword_token = kt_tokens::soft_keyword_by_text(self.my_builder.get_token_text());
            if let Some(keyword_token) = keyword_token.filter(|&k| set.contains(k)) {
                self.my_builder.remap_current_token(keyword_token);
                return true;
            }
        } else {
            // We know at this point that `set` does not contain `token`
            if set.contains(IDENTIFIER) && token.is_some_and(kt_tokens::is_soft_keyword) {
                self.my_builder.remap_current_token(IDENTIFIER);
                return true;
            }
        }
        false
    }

    pub(crate) fn lookahead(&mut self, k: i32) -> Option<SyntaxKind> {
        self.my_builder.look_ahead(k)
    }

    pub(crate) fn consume_if(&mut self, token: SyntaxKind) -> bool {
        if self.at(token) {
            self.advance(); // token
            return true;
        }
        false
    }

    // TODO: Migrate to predicates
    pub(crate) fn skip_until(&mut self, token_set: TokenSet) {
        let stop_at_eol_or_semi = token_set.contains(EOL_OR_SEMICOLON);
        while !self.eof() {
            let tt = self.tt();
            if token_set.contains(tt) || (stop_at_eol_or_semi && self.at(EOL_OR_SEMICOLON)) {
                break;
            }
            self.advance();
        }
    }

    pub(crate) fn error_until(&mut self, message: &str, token_set: TokenSet) {
        debug_assert!(token_set.contains(LBRACE), "Cannot include LBRACE into error element!");
        debug_assert!(token_set.contains(RBRACE), "Cannot include RBRACE into error element!");
        let error = self.mark();
        self.skip_until(token_set);
        error.error(self, message);
    }

    pub(crate) fn error_if(&mut self, marker: Marker, condition: bool, message: &str) {
        if condition {
            marker.error(self, message);
        } else {
            marker.drop(self);
        }
    }

    // `matchTokenStreamPredicate` lives in token_stream.rs; `OptionalMarker` in optional_marker.rs.

    pub(crate) fn eol(&mut self) -> bool {
        self.my_builder.newline_before_current_token() || self.eof()
    }

    pub(crate) fn close_declaration_with_comment_binders(
        &mut self,
        marker: Marker,
        element_type: SyntaxKind,
        preceding_non_doc_comments: bool,
    ) {
        marker.done(self, element_type);
        let preceding =
            if preceding_non_doc_comments { EdgeBinder::PrecedingComments } else { EdgeBinder::PrecedingDocComments };
        marker.set_custom_edge_token_binders(self, Some(preceding), Some(EdgeBinder::TrailingComments));
    }

    /// `createTruncatedBuilder(eofPosition)` followed by a call on the created parser. Only
    /// `KotlinParsing` calls it, so `create` is `KotlinParsing.create` = `createForTopLevel`
    /// (lazy, top-level) even inside a by-clause.
    pub(crate) fn create_truncated_builder<R>(&mut self, eof_position: i32, f: impl FnOnce(&mut Parser) -> R) -> R {
        self.my_builder.push_layer(Layer::Truncated { eof_position });
        self.with_context(true, None, f)
    }

    /// Test-only upstream (`substringWithContext(text, offset, offset, 20)`).
    pub fn current_context(&mut self) -> String {
        let offset = self.my_builder.get_current_offset() as usize;
        let text = self.my_builder.get_original_text();
        let start = floor_char_boundary(text, offset.saturating_sub(20));
        let end = floor_char_boundary(text, (offset + 20).min(text.len()));
        format!("{}<caret>{}", &text[start..offset], &text[offset..end])
    }
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

