//! Port of the parse-time half of `PsiBuilderImpl` (token cursor, markers, errors).
//! Tree construction (`getTreeBuilt`) lives in `tree.rs`.

use ktrs_syntax::SyntaxKind;

use super::marker::Marker;
use super::production::Production;

/// Constructed (from recycled buffers) in `pool.rs`.
pub struct PsiBuilder {
    pub(crate) text: String,
    /// `myLexStarts`: `lexeme_count + 1` entries, the last one is `text.len()`.
    pub(crate) lex_starts: Vec<u32>,
    /// `myLexTypes`; mutable because of `remapCurrentToken`.
    pub(crate) lex_types: Vec<SyntaxKind>,
    /// The lexer's kinds before any remap, for chameleons to reuse (see `LazyLeaf`).
    pub(crate) orig_types: Vec<SyntaxKind>,
    pub(crate) current_lexeme: usize,
    pub(super) token_type_checked: bool,
    pub(crate) production: Production,
    /// Scratch for `prepareLightTree`'s duplicate-error pass (see `tree.rs`).
    pub(crate) skipped_errors: Vec<bool>,
}

impl PsiBuilder {
    pub(crate) fn lexeme_count(&self) -> usize {
        self.lex_types.len()
    }

    pub fn get_original_text(&self) -> &str {
        &self.text
    }

    /// No remapper is ever installed for Kotlin, so the cached-type machinery reduces to this.
    #[inline]
    pub fn get_token_type(&mut self) -> Option<SyntaxKind> {
        if self.eof() { None } else { Some(self.lex_types[self.current_lexeme]) }
    }

    /// Whitespace = `KtTokens.WHITESPACES`, comments = `KtTokens.COMMENTS` (KotlinParserDefinition);
    /// the KDoc sub-builder uses the same Kotlin language definition.
    pub fn is_whitespace_or_comment(&self, kind: SyntaxKind) -> bool {
        kind.is_trivia()
    }

    pub fn remap_current_token(&mut self, kind: SyntaxKind) {
        self.lex_types[self.current_lexeme] = kind;
    }

    pub fn look_ahead(&mut self, steps: i32) -> Option<SyntaxKind> {
        let mut cur = self.shift_over_whitespace_forward(self.current_lexeme);
        let mut steps = steps;
        while steps > 0 {
            cur = self.shift_over_whitespace_forward(cur + 1);
            steps -= 1;
        }
        self.lex_types.get(cur).copied()
    }

    pub(crate) fn shift_over_whitespace_forward(&self, lex_index: usize) -> usize {
        let mut lex_index = lex_index;
        while lex_index < self.lexeme_count() && self.is_whitespace_or_comment(self.lex_types[lex_index]) {
            lex_index += 1;
        }
        lex_index
    }

    pub fn raw_lookup(&self, steps: i32) -> Option<SyntaxKind> {
        let cur = self.current_lexeme as i64 + steps as i64;
        if cur >= 0 && (cur as usize) < self.lexeme_count() { Some(self.lex_types[cur as usize]) } else { None }
    }

    pub fn raw_token_type_start(&self, steps: i32) -> i32 {
        let cur = self.current_lexeme as i64 + steps as i64;
        if cur < 0 {
            return -1;
        }
        if cur as usize >= self.lexeme_count() {
            return self.text.len() as i32;
        }
        self.lex_starts[cur as usize] as i32
    }

    pub fn raw_token_index(&self) -> i32 {
        self.current_lexeme as i32
    }

    pub fn raw_advance_lexer(&mut self, steps: i32) {
        assert!(steps >= 0, "Steps must be a positive integer - lexer can only be advanced.");
        if steps == 0 {
            return;
        }
        self.current_lexeme = (self.current_lexeme + steps as usize).min(self.lexeme_count());
        self.token_type_checked = false;
    }

    pub fn advance_lexer(&mut self) {
        if self.eof() {
            return;
        }
        self.token_type_checked = false;
        self.current_lexeme += 1;
    }

    fn skip_whitespace(&mut self) {
        while self.current_lexeme < self.lexeme_count()
            && self.is_whitespace_or_comment(self.lex_types[self.current_lexeme])
        {
            self.current_lexeme += 1;
        }
    }

    pub fn get_current_offset(&mut self) -> i32 {
        if self.eof() {
            return self.text.len() as i32;
        }
        self.lex_starts[self.current_lexeme] as i32
    }

    pub fn get_token_text(&mut self) -> Option<&str> {
        if self.eof() {
            return None;
        }
        let (start, end) = (self.lex_starts[self.current_lexeme], self.lex_starts[self.current_lexeme + 1]);
        Some(&self.text[start as usize..end as usize])
    }

    /// Skips whitespace first unless this is the very first (root) marker.
    pub fn mark(&mut self) -> Marker {
        if !self.production.is_empty() {
            self.skip_whitespace();
        }
        let id = self.production.allocate(false, self.current_lexeme as i32);
        self.production.add_marker(id);
        Marker(id)
    }

    #[inline]
    pub fn eof(&mut self) -> bool {
        if !self.token_type_checked {
            self.token_type_checked = true;
            self.skip_whitespace();
        }
        self.current_lexeme >= self.lexeme_count()
    }

    /// Adds an empty error element at the current (possibly pre-whitespace) lexeme; a second
    /// error at the same lexeme right after the first is ignored.
    pub fn error(&mut self, message: &str) {
        let size = self.production.size();
        if let Some(last) = size.checked_sub(1).and_then(|i| self.production.get_start_marker_at(i)) {
            let last = self.production.marker(last);
            if last.is_error_item && last.lexeme == self.current_lexeme as i32 {
                return;
            }
        }
        let id = self.production.allocate(true, self.current_lexeme as i32);
        self.production.set_message(id, message);
        self.production.add_marker(id);
    }

    /// `PsiBuilderImpl.hasErrorsAfter`.
    pub fn has_errors_after(&self, marker: Marker) -> bool {
        self.production.has_errors_after(marker.0)
    }

    pub(crate) fn precede(&mut self, marker: Marker) -> Marker {
        let lexeme = self.production.marker(marker.0).lexeme;
        assert!(lexeme >= 0, "Preceding disposed marker");
        let id = self.production.allocate(false, lexeme);
        self.production.add_before(id, marker.0);
        Marker(id)
    }

    pub(crate) fn rollback_to(&mut self, marker: Marker) {
        let lexeme = self.production.marker(marker.0).lexeme;
        assert!(lexeme >= 0, "The marker is already disposed");
        self.current_lexeme = lexeme as usize;
        self.token_type_checked = true;
        self.production.rollback_to(marker.0);
    }

    pub(crate) fn drop_marker(&mut self, marker: Marker) {
        self.production.drop_marker(marker.0);
    }

    /// `processDone`: `kind` is `myType`, already chosen by the caller (`done`/`error`/...).
    pub(crate) fn process_done(
        &mut self,
        marker: Marker,
        kind: SyntaxKind,
        error_message: Option<&str>,
        before: Option<Marker>,
    ) {
        let id = marker.0;
        assert!(!self.production.marker(id).is_done(), "Marker already done.");
        let done_lexeme = match before {
            None => self.current_lexeme as i32,
            Some(before) => self.production.marker(before.0).lexeme,
        };
        let start = self.production.marker(id).lexeme;
        let left_bound_empty = is_left_bound(kind) && self.is_empty(start, done_lexeme);
        if let Some(message) = error_message {
            self.production.set_message(id, message);
        }
        let data = self.production.marker_mut(id);
        data.kind = Some(kind);
        if left_bound_empty {
            data.left_binder = Some(super::EdgeBinder::DefaultRight);
        }
        data.done_lexeme = done_lexeme;
        self.production.add_done(id, before.map(|b| b.0));
    }

    fn is_empty(&self, start_idx: i32, end_idx: i32) -> bool {
        (start_idx.max(0)..end_idx.max(0)).all(|i| self.is_whitespace_or_comment(self.lex_types[i as usize]))
    }

    /// `doneBefore(type, before, errorMessage)`: an error item at `before`, then the done.
    pub(crate) fn done_before_with_error_item(
        &mut self,
        marker: Marker,
        kind: SyntaxKind,
        before: Marker,
        error_message: &str,
    ) {
        let lexeme = self.production.marker(before.0).lexeme;
        let error_id = self.production.allocate(true, lexeme);
        self.production.set_message(error_id, error_message);
        self.production.add_before(error_id, before.0);
        self.process_done(marker, kind, None, Some(before));
    }

    pub(crate) fn mark_collapsed(&mut self, marker: Marker) {
        self.production.marker_mut(marker.0).collapsed = true;
    }

    pub(crate) fn set_binders(
        &mut self,
        marker: Marker,
        left: Option<super::EdgeBinder>,
        right: Option<super::EdgeBinder>,
    ) {
        let data = self.production.marker_mut(marker.0);
        if left.is_some() {
            data.left_binder = left;
        }
        if right.is_some() {
            data.right_binder = right;
        }
    }
}

/// `IElementType.isLeftBound()`: true for `TokenType.ERROR_ELEMENT` and `KtLeftBoundNodeType`s.
fn is_left_bound(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ERROR_ELEMENT | SyntaxKind::CONSTRUCTOR_DELEGATION_CALL | SyntaxKind::CONSTRUCTOR_DELEGATION_REFERENCE
    )
}
