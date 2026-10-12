//! `PsiBuilder.Marker` as a copyable handle. Every operation takes the owner of the builder
//! (`&mut Parser`, `&mut PsiBuilder`, ...) explicitly: Java `m.done(BLOCK)` is `m.done(self, BLOCK)`.

use ktrs_syntax::SyntaxKind;

use super::binders::EdgeBinder;
use super::psi_builder::PsiBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Marker(pub(crate) i32);

/// Anything that owns the underlying [`PsiBuilder`] (the builder itself, the Kotlin
/// semantic-whitespace wrapper, the `Parser`).
pub trait MarkerHost {
    fn psi_builder(&mut self) -> &mut PsiBuilder;
}

impl MarkerHost for PsiBuilder {
    fn psi_builder(&mut self) -> &mut PsiBuilder {
        self
    }
}

#[allow(clippy::should_implement_trait)]
impl Marker {
    /// A new marker starting at the same lexeme, placed before `self` in the production.
    pub fn precede<H: MarkerHost + ?Sized>(self, host: &mut H) -> Marker {
        host.psi_builder().precede(self)
    }

    pub fn drop<H: MarkerHost + ?Sized>(self, host: &mut H) {
        host.psi_builder().drop_marker(self);
    }

    /// Rewinds the lexer to this marker and discards it and everything produced after it.
    /// Token remaps done since are *not* undone (same as IntelliJ).
    pub fn rollback_to<H: MarkerHost + ?Sized>(self, host: &mut H) {
        host.psi_builder().rollback_to(self);
    }

    pub fn done<H: MarkerHost + ?Sized>(self, host: &mut H, kind: SyntaxKind) {
        host.psi_builder().process_done(self, kind, None, None);
    }

    /// Done, and the whole range becomes one leaf of `kind` (a lazy leaf is reparsed on build).
    pub fn collapse<H: MarkerHost + ?Sized>(self, host: &mut H, kind: SyntaxKind) {
        let builder = host.psi_builder();
        builder.process_done(self, kind, None, None);
        builder.mark_collapsed(self);
    }

    pub fn done_before<H: MarkerHost + ?Sized>(self, host: &mut H, kind: SyntaxKind, before: Marker) {
        host.psi_builder().process_done(self, kind, None, Some(before));
    }

    /// `doneBefore(type, before, errorMessage)`.
    pub fn done_before_3<H: MarkerHost + ?Sized>(self, host: &mut H, kind: SyntaxKind, before: Marker, error_message: &str) {
        host.psi_builder().done_before_with_error_item(self, kind, before, error_message);
    }

    pub fn error<H: MarkerHost + ?Sized>(self, host: &mut H, message: &str) {
        host.psi_builder().process_done(self, SyntaxKind::ERROR_ELEMENT, Some(message), None);
    }

    pub fn error_before<H: MarkerHost + ?Sized>(self, host: &mut H, message: &str, before: Marker) {
        host.psi_builder().process_done(self, SyntaxKind::ERROR_ELEMENT, Some(message), Some(before));
    }

    /// `None` keeps the current binder for that edge (Java passes `null`).
    pub fn set_custom_edge_token_binders<H: MarkerHost + ?Sized>(
        self,
        host: &mut H,
        left: Option<EdgeBinder>,
        right: Option<EdgeBinder>,
    ) {
        host.psi_builder().set_binders(self, left, right);
    }
}
