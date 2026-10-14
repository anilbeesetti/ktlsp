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
    parse_block_code_fragment, parse_block_expression, parse_expression_code_fragment,
    parse_lambda_expression, parse_type_code_fragment,
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

std::thread_local! {
    static EOF_OBSERVER: std::cell::Cell<Option<fn()>> = const { std::cell::Cell::new(None) };
}

/// Dedicated grammar-checkpoint observation; does not establish EOF or CPU consumption.
#[doc(hidden)]
pub fn parse_file_observing_eof(text: &str, kind: FileKind, observer: fn()) -> Parse {
    struct Restore(Option<fn()>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = EOF_OBSERVER.try_with(|slot| slot.set(self.0));
        }
    }
    let Ok(previous) = EOF_OBSERVER.try_with(|slot| slot.replace(Some(observer))) else {
        return parse_file(text, kind);
    };
    let _restore = Restore(previous);
    parse_file(text, kind)
}

pub(crate) fn observe_eof() {
    if let Ok(Some(observer)) = EOF_OBSERVER.try_with(std::cell::Cell::get) {
        observer();
    }
}

/// Actual constructed generic-recovery scratch boundary; offsets are in the original builder.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct GenericCopyBoundary {
    pub active_layers: usize,
    pub independent_min: Option<i32>,
    pub production_bound: Option<i32>,
    pub window_start: u32,
    pub actual_copy_end: Option<u32>,
    pub next_token: Option<(u32, u32)>,
    pub next_fits_other_caps: bool,
}

type GenericCopyCallback = fn(&str, GenericCopyBoundary);
std::thread_local! {
    static GENERIC_COPY_OBSERVER: std::cell::Cell<Option<GenericCopyCallback>> =
        const { std::cell::Cell::new(None) };
}

/// Finite copy observation; false means no new callback scope was installed.
#[doc(hidden)]
pub fn parse_file_observing_generic_copy(
    text: &str,
    kind: FileKind,
    observer: GenericCopyCallback,
) -> (Parse, bool) {
    struct Restore(Option<GenericCopyCallback>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = GENERIC_COPY_OBSERVER.try_with(|slot| slot.set(self.0));
        }
    }
    let Ok(previous) = GENERIC_COPY_OBSERVER.try_with(|slot| {
        let previous = slot.replace(Some(observer));
        if previous.is_some() {
            slot.set(None);
        }
        previous
    }) else {
        return (parse_file(text, kind), false);
    };
    let installed = previous.is_none();
    let parse = {
        let _restore = Restore(previous);
        parse_file(text, kind)
    };
    (parse, installed)
}

pub(crate) fn generic_copy_observer() -> Option<GenericCopyCallback> {
    GENERIC_COPY_OBSERVER
        .try_with(std::cell::Cell::get)
        .ok()
        .flatten()
}

/// [`parse_file`] for callers re-parsing similar text: the result is identical, but lazy blocks,
/// lambdas and KDoc whose text was already seen with `cache` are reused instead of re-parsed.
pub fn parse_file_cached(text: &str, kind: FileKind, cache: &mut ChameleonCache) -> Parse {
    parsing::kotlin_parser::parse_cached(text, kind, cache)
}
