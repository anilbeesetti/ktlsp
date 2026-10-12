//! The adapter chain above `SemanticWhitespaceAwarePsiBuilderImpl`:
//! `TruncatedSemanticWhitespaceAwarePsiBuilder` and `SemanticWhitespaceAwarePsiBuilderForByClause`
//! as a stack of [`Layer`]s. The methods here are what parser code sees (the top of the chain).

use ktrs_syntax::SyntaxKind;

use super::semantic::SemanticWhitespaceAwarePsiBuilder;

#[derive(Clone, Copy, Debug)]
pub enum Layer {
    /// `TruncatedSemanticWhitespaceAwarePsiBuilder(builder, eofPosition)`.
    Truncated { eof_position: i32 },
    /// `SemanticWhitespaceAwarePsiBuilderForByClause`; `stack_size` is its `getStackSize()`.
    ForByClause { stack_size: i32 },
}

/// `isOffsetBeyondEof`.
fn is_offset_beyond_eof(eof_position: i32, offset_from_current: i32) -> bool {
    eof_position >= 0 && offset_from_current >= eof_position
}

impl SemanticWhitespaceAwarePsiBuilder {
    pub fn push_layer(&mut self, layer: Layer) -> usize {
        self.layers.push(layer);
        self.layers.len() - 1
    }

    pub fn pop_layer(&mut self) {
        self.layers.pop();
    }

    /// `getStackSize()` of the `ForByClause` layer at `index`.
    pub fn for_by_clause_stack_size(&self, index: usize) -> i32 {
        match self.layers[index] {
            Layer::ForByClause { stack_size } => stack_size,
            Layer::Truncated { .. } => unreachable!("layer {index} is not a by-clause layer"),
        }
    }

    /// Every by-clause adapter on the delegation chain sees newline-state calls and counts them.
    pub(super) fn count_newlines_call(&mut self, delta: i32) {
        for layer in &mut self.layers {
            if let Layer::ForByClause { stack_size } = layer {
                *stack_size += delta;
            }
        }
    }

    fn top_truncated_eof_position(&self) -> Option<i32> {
        self.layers.iter().rev().find_map(|l| match *l {
            Layer::Truncated { eof_position } => Some(eof_position),
            Layer::ForByClause { .. } => None,
        })
    }

    pub fn eof(&mut self) -> bool {
        let base = self.psi.eof();
        if base || self.layers.is_empty() {
            return base;
        }
        let offset = self.psi.get_current_offset();
        self.layers.iter().any(|l| match *l {
            Layer::Truncated { eof_position } => is_offset_beyond_eof(eof_position, offset),
            Layer::ForByClause { .. } => false,
        })
    }

    /// Inlined into every `at`: with no layers (the common case) it is the memo check.
    #[inline]
    pub fn get_token_type(&mut self) -> Option<SyntaxKind> {
        if !self.layers.is_empty() {
            return self.layered_get_token_type();
        }
        self.impl_get_token_type()
    }

    #[inline(never)]
    fn layered_get_token_type(&mut self) -> Option<SyntaxKind> {
        if self.top_truncated_eof_position().is_some() && self.eof() {
            return None;
        }
        self.impl_get_token_type()
    }

    pub fn get_token_text(&mut self) -> Option<&str> {
        if self.top_truncated_eof_position().is_some() && self.eof() {
            return None;
        }
        self.impl_get_token_text()
    }

    pub fn look_ahead(&mut self, steps: i32) -> Option<SyntaxKind> {
        let Some(eof_position) = self.top_truncated_eof_position() else {
            return self.impl_look_ahead(steps);
        };
        if self.eof() {
            return None;
        }

        let raw_look_ahead_steps = self.raw_look_ahead(steps);
        if is_offset_beyond_eof(eof_position, self.psi.raw_token_type_start(raw_look_ahead_steps)) {
            return None;
        }

        // Truncated lookahead is raw: complex tokens are not joined.
        self.psi.raw_lookup(raw_look_ahead_steps)
    }

    /// `TruncatedSemanticWhitespaceAwarePsiBuilder.rawLookAhead`.
    fn raw_look_ahead(&self, steps: i32) -> i32 {
        let mut cur = 0;
        let mut steps = steps;
        while steps > 0 {
            cur += 1;

            let mut raw_token_type = self.psi.raw_lookup(cur);
            while raw_token_type.is_some_and(|t| self.is_whitespace_or_comment(t)) {
                cur += 1;
                raw_token_type = self.psi.raw_lookup(cur);
            }

            steps -= 1;
        }
        cur
    }
}
