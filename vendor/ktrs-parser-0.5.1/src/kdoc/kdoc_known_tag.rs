//! Port of `kdoc/parser/KDocKnownTag.kt` (psi-api), only what `KDocParser` needs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KDocKnownTag {
    Author,
    Throws,
    Exception,
    Param,
    Receiver,
    Return,
    See,
    Since,
    Constructor,
    Property,
    Sample,
    Suppress,
}

impl KDocKnownTag {
    pub(crate) fn is_section_start(self) -> bool {
        matches!(self, KDocKnownTag::Constructor | KDocKnownTag::Property)
    }

    /// `valueOf(name.toUpperCaseAsciiOnly())` after stripping one leading `@`.
    pub(crate) fn find_by_tag_name(tag_name: &str) -> Option<KDocKnownTag> {
        let name = tag_name.strip_prefix('@').unwrap_or(tag_name).to_ascii_uppercase();
        Some(match name.as_str() {
            "AUTHOR" => KDocKnownTag::Author,
            "THROWS" => KDocKnownTag::Throws,
            "EXCEPTION" => KDocKnownTag::Exception,
            "PARAM" => KDocKnownTag::Param,
            "RECEIVER" => KDocKnownTag::Receiver,
            "RETURN" => KDocKnownTag::Return,
            "SEE" => KDocKnownTag::See,
            "SINCE" => KDocKnownTag::Since,
            "CONSTRUCTOR" => KDocKnownTag::Constructor,
            "PROPERTY" => KDocKnownTag::Property,
            "SAMPLE" => KDocKnownTag::Sample,
            "SUPPRESS" => KDocKnownTag::Suppress,
            _ => return None,
        })
    }
}
