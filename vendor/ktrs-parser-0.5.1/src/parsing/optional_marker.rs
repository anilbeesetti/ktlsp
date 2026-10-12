//! `AbstractKotlinParsing.OptionalMarker`.

use ktrs_syntax::SyntaxKind;

use super::Parser;
use crate::builder::Marker;

pub struct OptionalMarker {
    marker: Option<Marker>,
    offset: i32,
}

impl OptionalMarker {
    pub fn new(p: &mut Parser, actually_mark: bool) -> OptionalMarker {
        let marker = actually_mark.then(|| p.mark());
        OptionalMarker { marker, offset: p.my_builder.get_current_offset() }
    }

    pub fn done(&self, p: &mut Parser, element_type: SyntaxKind) {
        let Some(marker) = self.marker else { return };
        marker.done(p, element_type);
    }

    pub fn error(&self, p: &mut Parser, message: &str) {
        let Some(marker) = self.marker else { return };
        if self.offset == p.my_builder.get_current_offset() {
            marker.drop(p); // no empty errors
        } else {
            marker.error(p, message);
        }
    }

    pub fn drop(&self, p: &mut Parser) {
        let Some(marker) = self.marker else { return };
        marker.drop(p);
    }
}
