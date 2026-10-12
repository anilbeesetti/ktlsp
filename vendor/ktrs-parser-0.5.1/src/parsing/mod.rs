//! Port of `org.jetbrains.kotlin.parsing` (third_party/kotlin/compiler/psi/parser/src/.../parsing).
//!
//! # Mapping Java onto one `Parser`
//! `AbstractKotlinParsing`, `KotlinParsing` and `KotlinExpressionParsing` are all `impl Parser`
//! blocks: `abstract_kotlin_parsing.rs`, `declarations/` (KotlinParsing.java) and
//! `expressions/` (KotlinExpressionParsing.java), split into files by upstream line range, each
//! keeping upstream method order. Porting rules:
//! - One Rust fn per Java method, `snake_case` of the Java name, same order as the Java file.
//!   `private static final` fields become module `const`s with the Java name; nested enums/classes
//!   become Rust types in the same file.
//! - Name collisions between the two classes get an `expr_` prefix on the expression side (today
//!   only `create`, whose expression-side variant is unreachable and needn't be ported).
//! - Overloads: the one with the fewest parameters keeps the name; each other overload appends
//!   `_{param count}` (`expect(t)`, `expect_2(t, msg)`, `expect_3(t, msg, set)`,
//!   `parse_type_ref()`, `parse_type_ref_1(set)`, `parse_type_ref_2(set, b)`).
//! - `myExpressionParsing.x()` / `myKotlinParsing.x()` → `self.x()`; `myBuilder.x()` →
//!   `self.my_builder.x()`; `isLazy` → `self.is_lazy`.
//! - Markers: `Marker m = mark();` → `let m = self.mark();`; `m.done(T)` → `m.done(self, T)`,
//!   likewise `drop/rollback_to/precede/collapse/error/done_before/set_custom_edge_token_binders`.
//! - Nullable `IElementType` results (`tt()`, `lookahead(k)`, `rawLookup`) are
//!   `Option<SyntaxKind>`: `tt() == LBRACE` → `self.tt() == Some(LBRACE)`. Parameters that are
//!   never null take `SyntaxKind`. `KtTokens.X`/`KtNodeTypes.X` → `SyntaxKind::X`; `KtTokens` sets
//!   → `crate::kt_tokens::*`; `TokenSet.create(a, b)` → `TokenSet::create(&[a, b])`,
//!   `orSet(a, b)` → `TokenSet::or_set(&[a, b])` (all `const fn`). `x instanceof KtKeywordToken` /
//!   `isSoft()` / `getValue()` → `kt_tokens::is_keyword_token` / `is_soft_keyword` /
//!   `kind.keyword_text()`.
//! - Java `assert` → `debug_assert!`. `Consumer<IElementType>` → `&mut dyn Consumer<SyntaxKind>`.
//! - Token-stream pattern fields (`lastDotAfterReceiver*Pattern`) are `reset()` before every use,
//!   so build them as locals at the use site (see `token_stream.rs`); anonymous predicates are
//!   closures `|p: &mut Parser, top_level: bool| -> bool`.
//!
//! # Wrapped builders and sub-parsers
//! Java creates new parser objects over wrapped builders; here they are scoped context switches
//! that push a builder [`Layer`] and restore everything afterwards:
//! - `createForByClause(myBuilder, isLazy).myExpressionParsing.parseExpression()` →
//!   `self.create_for_by_clause(self.is_lazy, |p| p.parse_expression())`.
//! - `createTruncatedBuilder(eof).parseTypeRefWithoutIntersections()` →
//!   `self.create_truncated_builder(eof, |p| p.parse_type_ref_without_intersections())`.
//! - The by-clause parser's anonymous `parseCallWithClosure` override → start
//!   `parse_call_with_closure` with
//!   `if let Some(n) = self.by_clause_stack_size() { if n <= 0 { return false; } }`.
//!
//! # Entry points and lazy nodes
//! `kotlin_parser.rs` ports `KotlinParser.java`. Collapsed `BLOCK`/`LAMBDA_EXPRESSION` leaves and
//! `DOC_COMMENT` tokens are reparsed when the tree is built, over their own text with a fresh
//! builder (`reparse_lazy`), exactly like the compiler's chameleons.

mod abstract_kotlin_parsing;
mod declarations;
mod expressions;
pub(crate) mod kotlin_parser;
mod optional_marker;
mod token_stream;

pub use optional_marker::OptionalMarker;
pub use token_stream::{
    AbstractTokenStreamPattern, At, AtSet, FirstBefore, LastBefore, Or, TokenStreamPattern, TokenStreamPredicate,
};

use crate::builder::{Layer, MarkerHost, PsiBuilder, SemanticWhitespaceAwarePsiBuilder};

/// `Consumer.java`.
pub trait Consumer<T> {
    fn consume(&mut self, item: T);
}

/// The state shared by one `KotlinParsing` + `KotlinExpressionParsing` pair.
pub struct Parser {
    pub(crate) my_builder: SemanticWhitespaceAwarePsiBuilder,
    pub(crate) is_lazy: bool,
    /// Index of this context's `ForByClause` builder layer; `None` for top-level parsers.
    by_clause_layer: Option<usize>,
    in_parameter_recovery_probe: bool,
}

impl MarkerHost for Parser {
    fn psi_builder(&mut self) -> &mut PsiBuilder {
        &mut self.my_builder.psi
    }
}

impl Parser {
    /// `KotlinParsing.createForTopLevel`.
    pub(crate) fn create_for_top_level(builder: SemanticWhitespaceAwarePsiBuilder) -> Parser {
        Parser { my_builder: builder, is_lazy: true, by_clause_layer: None, in_parameter_recovery_probe: false }
    }

    /// `KotlinParsing.createForTopLevelNonLazy`.
    #[allow(dead_code)] // TODO: only `KotlinLightParser` uses it upstream; not ported yet.
    pub(crate) fn create_for_top_level_non_lazy(builder: SemanticWhitespaceAwarePsiBuilder) -> Parser {
        Parser { my_builder: builder, is_lazy: false, by_clause_layer: None, in_parameter_recovery_probe: false }
    }

    /// `KotlinParsing.createForByClause(myBuilder, isLazy)`: runs `f` on a non-top-level parser
    /// over a `SemanticWhitespaceAwarePsiBuilderForByClause` wrapping the current builder.
    pub(crate) fn create_for_by_clause<R>(&mut self, is_lazy: bool, f: impl FnOnce(&mut Parser) -> R) -> R {
        let layer = self.my_builder.push_layer(Layer::ForByClause { stack_size: 0 });
        self.with_context(is_lazy, Some(layer), f)
    }

    /// `getStackSize()` of the by-clause builder if this is a by-clause parser, else `None`.
    pub(crate) fn by_clause_stack_size(&self) -> Option<i32> {
        self.by_clause_layer.map(|layer| self.my_builder.for_by_clause_stack_size(layer))
    }

    /// Runs `f` with a new parser context, then pops the layer pushed by the caller and restores.
    fn with_context<R>(&mut self, is_lazy: bool, by_clause_layer: Option<usize>, f: impl FnOnce(&mut Parser) -> R) -> R {
        let saved = (self.is_lazy, self.by_clause_layer);
        self.is_lazy = is_lazy;
        self.by_clause_layer = by_clause_layer;
        let result = f(self);
        (self.is_lazy, self.by_clause_layer) = saved;
        self.my_builder.pop_layer();
        result
    }
}
