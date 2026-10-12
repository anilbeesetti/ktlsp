//! `PsiBuilderImpl.getTreeBuilt`: `prepareLightTree` (+ `balanceWhiteSpaces`) and `bind`,
//! producing a [`ktrs_syntax::Tree`] plus preorder error messages.
//!
//! Leaves whose type is lazy-parseable (collapsed `BLOCK`/`LAMBDA_EXPRESSION`, `DOC_COMMENT`, ...)
//! are handed to the `lazy` callback, which reparses their text with a fresh builder exactly as
//! the compiler's chameleons do and emits that builder's tree into the same [`TreeSink`], its root
//! renamed to the leaf's kind. `false` from the callback means "plain leaf".

use ktrs_syntax::{MissedTokens, Parse, SyntaxKind};

use super::binders::EdgeBinder;
use super::psi_builder::PsiBuilder;
use super::sink::TreeSink;

/// Reparses a lazy-parseable leaf into `sink`; `false` if its kind isn't lazy (nothing emitted).
/// Generic rather than `dyn` so the per-leaf kind check inlines into `create_leaf`.
pub trait LazyReparse: Fn(&LazyLeaf<'_>, &mut TreeSink) -> bool {}
impl<F: Fn(&LazyLeaf<'_>, &mut TreeSink) -> bool> LazyReparse for F {}

/// A leaf about to be built, with the outer builder's (unremapped) lexemes covering it.
pub struct LazyLeaf<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
    /// Lexemes `start..end` of `outer`; sliced only when the leaf is actually reparsed.
    outer: &'a PsiBuilder,
    start: usize,
    end: usize,
}

impl LazyLeaf<'_> {
    /// A builder over the leaf's lexemes. Equal to re-lexing `text` with the outer builder's
    /// lexer when the leaf is a balanced `{...}` range or runs to the end of the input: the
    /// Kotlin lexer's state at a `{` only differs inside a `${...}` template, whose brace
    /// counting agrees with `advanceBalancedBlock`.
    pub(crate) fn relexed_builder(&self) -> PsiBuilder {
        let (start, end) = (self.start, self.end);
        PsiBuilder::from_lexemes(self.text, &self.outer.lex_starts[start..=end], &self.outer.orig_types[start..end])
    }
}

impl PsiBuilder {
    pub fn get_tree_built(&mut self, lazy: &impl LazyReparse) -> Parse {
        let mut sink = TreeSink::new();
        self.build_tree_into(None, &mut sink, lazy);
        sink.finish()
    }

    /// `getTreeBuilt` into a shared sink; the root node gets `root_kind` if given.
    pub fn build_tree_into(&mut self, root_kind: Option<SyntaxKind>, sink: &mut TreeSink, lazy: &impl LazyReparse) {
        assert!(!self.production.is_empty(), "Parser produced no markers");
        self.balance_white_spaces();
        let mut skipped_errors = std::mem::take(&mut self.skipped_errors);
        self.duplicate_error_items(&mut skipped_errors);
        let root = sink.len();
        self.bind(root_kind, &skipped_errors, sink, lazy);
        self.skipped_errors = skipped_errors;
        if self.current_lexeme < self.lexeme_count() {
            let tokens = self.lex_types[self.current_lexeme..].iter().map(|kind| kind.debug_name()).collect();
            sink.missed_tokens.push(MissedTokens { element: root, tokens, text: self.text.to_string() });
        }
    }

    fn balance_white_spaces(&mut self) {
        let mut last_index: i32 = 0;
        // `getLexemeIndexAt(i - 1)`, carried along: item i - 1's index as this loop left it.
        let mut prev_index = self.production.get_lexeme_index_at(0);
        let size = self.production.size().saturating_sub(1);
        for i in 1..size {
            let id = self.production.list[i];
            let done = id < 0;
            let item = self.production.marker(id);
            assert!(done || item.is_error_item || item.is_done(), "Unbalanced tree: marker not done");

            let binder = if item.is_error_item { EdgeBinder::DefaultRight } else { item.get_binder(done) };
            let mut lexeme_index = item.get_lexeme_index(done);

            // The default binders always land on the run's end (left) or start (right), so they
            // skip scanning the other side; the result equals the general path's.
            if binder == EdgeBinder::DefaultLeft {
                lexeme_index = self.shift_over_whitespace_forward(lexeme_index as usize) as i32;
                self.production.marker_mut(id).set_lexeme_index(lexeme_index, done);
                (last_index, prev_index) = (lexeme_index, lexeme_index);
                continue;
            }
            let prev_production_lex_index = prev_index;
            let mut ws_start_index = lexeme_index.max(last_index);
            while ws_start_index > prev_production_lex_index
                && self.is_whitespace_or_comment(self.lex_types[ws_start_index as usize - 1])
            {
                ws_start_index -= 1;
            }
            if binder == EdgeBinder::DefaultRight {
                self.production.marker_mut(id).set_lexeme_index(ws_start_index, done);
                (last_index, prev_index) = (ws_start_index, ws_start_index);
                continue;
            }
            let ws_end_index = self.shift_over_whitespace_forward(lexeme_index as usize) as i32;

            if ws_start_index != ws_end_index {
                // Production lexemes are monotonic, so the run is never inverted.
                debug_assert!(ws_start_index < ws_end_index);
                let (start, end) = (ws_start_index as usize, ws_end_index as usize);
                let at_end = ws_start_index == 0 || end == self.lexeme_count();
                let getter = |i: usize| self.token_text(start + i);
                let edge = binder.get_edge_position(&self.lex_types[start..end], at_end, &getter);
                lexeme_index = ws_start_index + edge as i32;
                self.production.marker_mut(id).set_lexeme_index(lexeme_index, done);
            } else if lexeme_index < ws_start_index {
                lexeme_index = ws_start_index;
                self.production.marker_mut(id).set_lexeme_index(ws_start_index, done);
            }

            last_index = lexeme_index;
            prev_index = lexeme_index;
        }
    }

    /// `prepareLightTree` keeps only the first (deepest) error item per lexeme, in production order.
    fn duplicate_error_items(&self, skipped: &mut Vec<bool>) {
        skipped.clear();
        // `bind` only reads the flags of error items.
        if !self.production.has_error_items() {
            return;
        }
        skipped.resize(self.production.size(), false);
        let mut last_error_index = -1;
        for (i, &id) in self.production.list.iter().enumerate().skip(1) {
            if id > 0 && self.production.marker(id).is_error_item {
                let cur_token = self.production.marker(id).lexeme;
                if cur_token != last_error_index {
                    last_error_index = cur_token;
                } else {
                    skipped[i] = true;
                }
            }
        }
    }

    /// Walks the production in order; equivalent to `bind` over the light tree.
    fn bind(&self, root_kind: Option<SyntaxKind>, skipped_errors: &[bool], out: &mut TreeSink, lazy: &impl LazyReparse) {
        let list = &self.production.list;
        let root = self.production.marker(list[0]);
        out.start_node(root_kind.unwrap_or_else(|| kind_of(root.kind)));
        let mut depth = 1;
        let mut lex_index = root.lexeme.max(0) as usize;

        let mut i = 1;
        while i < list.len() {
            let id = list[i];
            let item = self.production.marker(id);
            if id < 0 {
                lex_index = self.insert_leaves(lex_index, item.done_lexeme, out, lazy);
                // Tokens after the root's done are dropped, as upstream (which LOG.errors: `MissedTokens`).
                if id == -list[0] {
                    break;
                }
                out.finish_node();
                depth -= 1;
            } else if item.is_error_item {
                if !skipped_errors[i] {
                    lex_index = self.insert_leaves(lex_index, item.lexeme, out, lazy);
                    out.errors.push(self.production.message(id).unwrap_or_default().to_owned());
                    out.start_node(SyntaxKind::ERROR_ELEMENT);
                    out.finish_node();
                }
            } else {
                lex_index = self.insert_leaves(lex_index, item.lexeme, out, lazy);
                if item.collapsed {
                    lex_index = self.collapse_leaves(item.lexeme, item.done_lexeme, kind_of(item.kind), out, lazy);
                    i = list[i..].iter().position(|&x| x == -id).map_or(list.len(), |p| i + p);
                } else {
                    let kind = kind_of(item.kind);
                    if kind == SyntaxKind::ERROR_ELEMENT {
                        out.errors.push(self.production.message(id).expect("error marker without message").to_owned());
                    }
                    out.start_node(kind);
                    depth += 1;
                }
            }
            i += 1;
        }
        // Unbalanced markers are a parser bug upstream (LOG.error); close whatever is still open.
        for _ in 0..depth {
            out.finish_node();
        }
    }

    fn insert_leaves(&self, cur_token: usize, last_idx: i32, out: &mut TreeSink, lazy: &impl LazyReparse) -> usize {
        let last_idx = (last_idx.max(0) as usize).min(self.lexeme_count());
        if cur_token >= last_idx {
            return cur_token;
        }
        let kinds = &self.lex_types[cur_token..last_idx];
        let starts = &self.lex_starts[cur_token..=last_idx];
        for (i, (&kind, bounds)) in kinds.iter().zip(starts.windows(2)).enumerate() {
            // Empty tokens are skipped (no Kotlin token type is an ILeafElementType).
            if bounds[0] < bounds[1] {
                let text = &self.text[bounds[0] as usize..bounds[1] as usize];
                self.create_leaf(kind, cur_token + i, cur_token + i + 1, text, out, lazy);
            }
        }
        last_idx
    }

    fn collapse_leaves(&self, start: i32, end: i32, kind: SyntaxKind, out: &mut TreeSink, lazy: &impl LazyReparse) -> usize {
        let (start, end) = (start as usize, end as usize);
        let text = &self.text[self.lex_starts[start] as usize..self.lex_starts[end] as usize];
        self.create_leaf(kind, start, end, text, out, lazy);
        end
    }

    /// A leaf of `kind` over lexemes `start..end`, whose text is `text`.
    fn create_leaf(
        &self,
        kind: SyntaxKind,
        start: usize,
        end: usize,
        text: &str,
        out: &mut TreeSink,
        lazy: &impl LazyReparse,
    ) {
        let leaf = LazyLeaf { kind, text, outer: self, start, end };
        if !lazy(&leaf, out) {
            out.token(kind, leaf.text);
        }
    }

    pub(crate) fn token_text(&self, lexeme: usize) -> &str {
        &self.text[self.lex_starts[lexeme] as usize..self.lex_starts[lexeme + 1] as usize]
    }
}

fn kind_of(kind: Option<SyntaxKind>) -> SyntaxKind {
    kind.expect("Unbalanced tree. Most probably caused by unbalanced markers.")
}
