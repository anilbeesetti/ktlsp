//! `WhitespacesAndCommentsBinder` implementations: IntelliJ's `WhitespacesBinders` plus Kotlin's
//! `KotlinWhitespaceAndCommentsBinders.kt`, as one closed enum. None of them is recursive
//! (`isRecursive()`), so `MarkerProduction.confineMarkersToMaxLexeme` is never needed.

use ktrs_syntax::SyntaxKind::{self, *};

use crate::kt_tokens::COMMENTS;

/// Decides where an element edge lands inside a run of whitespace/comment tokens:
/// the result is an index into `tokens` (0 = before all of them, `tokens.len()` = after all).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeBinder {
    /// `WhitespacesBinders.DEFAULT_LEFT_BINDER` (= `GREEDY_RIGHT_BINDER`).
    DefaultLeft,
    /// `WhitespacesBinders.DEFAULT_RIGHT_BINDER` (= `GREEDY_LEFT_BINDER`).
    DefaultRight,
    PrecedingComments,
    PrecedingDocComments,
    TrailingComments,
    /// `PRECEDING_ALL_COMMENTS_BINDER` (`AllCommentsBinder(isTrailing = false)`).
    PrecedingAllComments,
    /// `TRAILING_ALL_COMMENTS_BINDER` (`AllCommentsBinder(isTrailing = true)`).
    TrailingAllComments,
    DoNotBindAnything,
    BindFirstShebangWithWhitespaceOnly,
    /// `PRECEDING_ALL_BINDER` (`BindAll(isTrailing = false)`).
    PrecedingAll,
    /// `TRAILING_ALL_BINDER` (`BindAll(isTrailing = true)`).
    TrailingAll,
}

pub const GREEDY_LEFT_BINDER: EdgeBinder = EdgeBinder::DefaultRight;
pub const GREEDY_RIGHT_BINDER: EdgeBinder = EdgeBinder::DefaultLeft;

impl EdgeBinder {
    pub fn get_edge_position<'t>(
        self,
        tokens: &[SyntaxKind],
        _at_stream_edge: bool,
        getter: &dyn Fn(usize) -> &'t str,
    ) -> usize {
        match self {
            EdgeBinder::DefaultLeft => tokens.len(),
            EdgeBinder::DefaultRight => 0,
            EdgeBinder::PrecedingComments => preceding_comments(tokens, getter),
            EdgeBinder::PrecedingDocComments => {
                if tokens.is_empty() {
                    return 0;
                }
                tokens.iter().rposition(|&t| t == DOC_COMMENT).unwrap_or(tokens.len())
            }
            EdgeBinder::TrailingComments => trailing_comments(tokens, getter),
            EdgeBinder::PrecedingAllComments => all_comments(tokens, false),
            EdgeBinder::TrailingAllComments => all_comments(tokens, true),
            EdgeBinder::DoNotBindAnything => 0,
            EdgeBinder::BindFirstShebangWithWhitespaceOnly => {
                if tokens.first() == Some(&SHEBANG_COMMENT) {
                    return if tokens.get(1) == Some(&WHITE_SPACE) { 2 } else { 1 };
                }
                0
            }
            EdgeBinder::PrecedingAll => 0,
            EdgeBinder::TrailingAll => tokens.len(),
        }
    }
}

fn preceding_comments<'t>(tokens: &[SyntaxKind], getter: &dyn Fn(usize) -> &'t str) -> usize {
    if tokens.is_empty() {
        return 0;
    }

    // 1. bind doc comment
    if let Some(idx) = tokens.iter().rposition(|&t| t == DOC_COMMENT) {
        return idx;
    }

    // 2. bind plain comments
    let mut result = tokens.len();
    for idx in (0..tokens.len()).rev() {
        let token_type = tokens[idx];
        if token_type == WHITE_SPACE {
            if get_line_break_count(getter(idx)) > 1 {
                break;
            }
        } else if COMMENTS.contains(token_type) {
            if idx == 0 || tokens[idx - 1] == WHITE_SPACE && contains_line_break(getter(idx - 1)) {
                result = idx;
            }
        } else {
            break;
        }
    }
    result
}

fn trailing_comments<'t>(tokens: &[SyntaxKind], getter: &dyn Fn(usize) -> &'t str) -> usize {
    if tokens.is_empty() {
        return 0;
    }

    let mut result = 0;
    for (idx, &token_type) in tokens.iter().enumerate() {
        match token_type {
            WHITE_SPACE => {
                if contains_line_break(getter(idx)) {
                    break;
                }
            }
            EOL_COMMENT | BLOCK_COMMENT => result = idx + 1,
            _ => break,
        }
    }
    result
}

fn all_comments(tokens: &[SyntaxKind], is_trailing: bool) -> usize {
    if tokens.is_empty() {
        return 0;
    }
    let size = tokens.len();
    // Skip one whitespace if needed. Expect that there can't be several consecutive whitespaces
    let end_token = tokens[if is_trailing { size - 1 } else { 0 }];
    let shift = usize::from(end_token == WHITE_SPACE);
    if is_trailing { size - shift } else { shift }
}

/// `StringUtil.getLineBreakCount`: `\r\n`, `\r` and `\n` each count once.
fn get_line_break_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => count += 1,
            b'\r' => {
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                count += 1;
            }
            _ => {}
        }
        i += 1;
    }
    count
}

/// `StringUtil.containsLineBreak`.
fn contains_line_break(text: &str) -> bool {
    text.bytes().any(|b| b == b'\n' || b == b'\r')
}
