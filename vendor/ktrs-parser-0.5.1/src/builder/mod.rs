//! Re-implementation of IntelliJ's `PsiBuilderImpl` (platform 251.27812.49, the one bundled with
//! Kotlin 2.4.20) and the Kotlin builder wrappers on top of it.
//!
//! Semantics kept exactly:
//! - Whitespace/comments (`WHITE_SPACE`, `EOL/BLOCK/SHEBANG_COMMENT`, `DOC_COMMENT`) are skipped
//!   lazily: by `eof()`/`get_token_type()` and by `mark()` (except for the very first marker),
//!   never by `advance_lexer()`, `error()` or `done()`. Positions are fixed up afterwards by
//!   `balanceWhiteSpaces` using each edge's [`EdgeBinder`] (defaults: start markers skip leading
//!   trivia, done markers and error items exclude trailing trivia; an *empty* marker of a
//!   left-bound type — `ERROR_ELEMENT`, `CONSTRUCTOR_DELEGATION_*` — binds to the left).
//! - `builder.error()` adds an empty error item at the current raw lexeme, ignored if the last
//!   production entry is an error item at the same lexeme; when building, only the first error item
//!   per lexeme (in production order) survives.
//! - Collapsed markers become one leaf; leaves of lazy-parseable kinds are reparsed with a fresh
//!   builder into the same [`TreeSink`] (see `tree.rs`), so the tree equals the compiler's fully
//!   expanded PSI.
//! - Token remaps survive `rollback_to`.
//!
//! Deviations: offsets are UTF-8 byte offsets (IntelliJ: UTF-16 units) — only compared with each
//! other, so behaviour is identical. No debug-mode checks, no `ITokenTypeRemapper`,
//! `WhitespaceSkippedCallback`, incremental reparse or recursive binders (Kotlin uses none).
//! Tokens outside the root marker are dropped (upstream logs an error).
//!
//! Performance (none of it observable): builders take their vectors from a per-thread pool
//! (`pool.rs`), chameleons reuse the outer builder's lexemes instead of re-lexing (`LazyLeaf`), and
//! a caller re-parsing similar text can reuse expanded chameleons (`ChameleonCache`). Measure with
//! `cargo run -p ktrs-parser --release --example bench`.

mod binders;
mod chameleon_cache;
mod layers;
mod marker;
mod pool;
mod production;
mod psi_builder;
mod semantic;
mod sink;
mod tree;

#[cfg(test)]
mod tests;

pub use binders::{EdgeBinder, GREEDY_LEFT_BINDER, GREEDY_RIGHT_BINDER};
pub use chameleon_cache::ChameleonCache;
pub use marker::{Marker, MarkerHost};
pub use psi_builder::PsiBuilder;
pub use layers::Layer;
pub use semantic::SemanticWhitespaceAwarePsiBuilder;
pub use sink::TreeSink;
pub use tree::{LazyLeaf, LazyReparse};
