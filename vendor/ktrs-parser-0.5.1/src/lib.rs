//! Port of the Kotlin compiler's PSI parser (`KotlinParsing`, `KotlinExpressionParsing`,
//! `KDocParser`) on top of a re-implementation of IntelliJ's `PsiBuilder` semantics.
//! See `builder` for the builder contract and `parsing` for the porting conventions.

pub mod builder;
mod kdoc;
pub mod kt_tokens;
pub mod parsing;
pub mod token_set;

use ktrs_syntax::Parse;

pub use builder::ChameleonCache;

pub use parsing::kotlin_parser::{
    parse_block_code_fragment, parse_block_expression, parse_expression_code_fragment, parse_lambda_expression,
    parse_type_code_fragment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Source,
    Script,
}

impl FileKind {
    /// Like `KotlinParser.parse`: no extension or `.kt` is a source file, anything else a script.
    pub fn from_file_name(name: &str) -> FileKind {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        match base.rsplit_once('.') {
            None => FileKind::Source,
            Some((_, "kt")) | Some((_, "")) => FileKind::Source,
            Some(_) => FileKind::Script,
        }
    }
}

/// Parses a whole file. `text` must already have CRLF normalized to LF.
pub fn parse_file(text: &str, kind: FileKind) -> Parse {
    parsing::kotlin_parser::parse(text, kind)
}

/// [`parse_file`] for callers re-parsing similar text: the result is identical, but lazy blocks,
/// lambdas and KDoc whose text was already seen with `cache` are reused instead of re-parsed.
pub fn parse_file_cached(text: &str, kind: FileKind, cache: &mut ChameleonCache) -> Parse {
    parsing::kotlin_parser::parse_cached(text, kind, cache)
}
