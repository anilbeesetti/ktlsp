//! Port of `kdoc/parser/KDocParser.java`.

use ktrs_syntax::SyntaxKind::{self, *};

use super::kdoc_known_tag::KDocKnownTag;
use crate::builder::{Marker, PsiBuilder};

/// `KDocParser.parse` minus `getTreeBuilt` (the caller builds the tree).
pub(crate) fn parse(root: SyntaxKind, builder: &mut PsiBuilder) {
    let root_marker = builder.mark();
    if builder.get_token_type() == Some(KDOC_START) {
        builder.advance_lexer();
    }
    let mut current_section_marker = Some(builder.mark());

    while !builder.eof() {
        if builder.get_token_type() == Some(KDOC_TAG_NAME) {
            current_section_marker = Some(parse_tag(builder, current_section_marker));
        } else if builder.get_token_type() == Some(KDOC_END) {
            if let Some(marker) = current_section_marker.take() {
                marker.done(builder, KDOC_SECTION);
            }
            builder.advance_lexer();
        } else {
            builder.advance_lexer();
        }
    }

    if let Some(marker) = current_section_marker {
        marker.done(builder, KDOC_SECTION);
    }
    root_marker.done(builder, root);
}

fn parse_tag(builder: &mut PsiBuilder, current_section_marker: Option<Marker>) -> Marker {
    // Upstream NPEs on a tag after `*/`; the KDoc lexer never produces one.
    let mut current_section_marker = current_section_marker.expect("KDoc tag after KDOC_END");
    let tag_name = builder.get_token_text().unwrap_or_default();
    let known_tag = KDocKnownTag::find_by_tag_name(tag_name);
    if known_tag.is_some_and(KDocKnownTag::is_section_start) {
        current_section_marker.done(builder, KDOC_SECTION);
        current_section_marker = builder.mark();
    }
    let tag_start = builder.mark();
    builder.advance_lexer();

    while !builder.eof() && !is_at_end_of_tag(builder) {
        builder.advance_lexer();
    }
    tag_start.done(builder, KDOC_TAG);
    current_section_marker
}

fn is_at_end_of_tag(builder: &mut PsiBuilder) -> bool {
    if builder.get_token_type() == Some(KDOC_END) {
        return true;
    }
    if builder.get_token_type() == Some(KDOC_LEADING_ASTERISK) {
        let mut look_ahead_count = 1;
        if builder.look_ahead(1) == Some(KDOC_TEXT) {
            look_ahead_count += 1;
        }
        if builder.look_ahead(look_ahead_count) == Some(KDOC_TAG_NAME) {
            return true;
        }
    }
    false
}
