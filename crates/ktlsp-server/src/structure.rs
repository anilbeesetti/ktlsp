//! Bounded lexical/structural providers; no declaration or compiler analysis.
use ktlsp_syntax::{TokenKind, lex};
use ktlsp_text::{Position, Range};
use serde_json::{Value, json};

pub const MAX_FOLDS: usize = 10_000;
pub const MAX_DIAGNOSTICS: usize = 256;
pub const MAX_DEPTH: usize = 1024;

struct Fold {
    start: usize,
    end: usize,
    kind: &'static str,
}
struct Diagnostic {
    start: usize,
    end: usize,
    code: &'static str,
    message: &'static str,
}
#[derive(Default)]
pub struct Analysis {
    folds: Vec<Fold>,
    diagnostics: Vec<Diagnostic>,
}
impl Analysis {
    fn diagnostic(&mut self, start: usize, end: usize, code: &'static str, message: &'static str) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic {
                start,
                end,
                code,
                message,
            });
        } else if let Some(last) = self.diagnostics.last_mut() {
            last.code = "structural-diagnostic-limit";
            last.message =
                "lexical/structural diagnostic limit reached; further diagnostics suppressed";
        }
    }
    fn fold(&mut self, start: usize, end: usize, kind: &'static str, limit: usize) {
        if self.folds.len() < limit {
            self.folds.push(Fold { start, end, kind });
        }
    }
    pub fn render(self, text: &str, line_only: bool) -> (Vec<Value>, Vec<Value>) {
        // Only retained output endpoints are indexed, then converted in one sweep.
        let mut offsets: Vec<_> = self
            .folds
            .iter()
            .flat_map(|f| [f.start, f.end])
            .chain(self.diagnostics.iter().flat_map(|d| [d.start, d.end]))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        let positions = positions(text, &offsets);
        let at = |offset| positions[offsets.binary_search(&offset).expect("retained endpoint")];
        let mut folds: Vec<_> = self
            .folds
            .into_iter()
            .filter_map(|f| {
                let start = at(f.start);
                let end = at(f.end);
                if start.line == end.line {
                    return None;
                }
                let mut value = json!({"startLine":start.line,"endLine":end.line,"kind":f.kind});
                if !line_only {
                    value["startCharacter"] = json!(start.character);
                    value["endCharacter"] = json!(end.character);
                }
                Some(value)
            })
            .collect();
        folds.sort_by_key(|f| {
            (
                f["startLine"].as_u64(),
                f["startCharacter"].as_u64(),
                f["endLine"].as_u64(),
            )
        });
        let diagnostics = self.diagnostics.into_iter().map(|d| {
            json!({"range":Range{start:at(d.start),end:at(d.end)},"severity":1,"source":"ktlsp-lexical-structural","code":d.code,"message":d.message})
        }).collect();
        (folds, diagnostics)
    }
}

// Sorted byte boundaries come only from lossless tokens and lexical diagnostics.
fn positions(text: &str, offsets: &[usize]) -> Vec<Position> {
    let mut result = Vec::with_capacity(offsets.len());
    let mut byte = 0;
    let mut position = Position {
        line: 0,
        character: 0,
    };
    for &offset in offsets {
        while byte < offset {
            let c = text[byte..]
                .chars()
                .next()
                .expect("in-bounds token endpoint");
            byte += c.len_utf8();
            if c == '\r' || c == '\n' {
                if c == '\r' && text.as_bytes().get(byte) == Some(&b'\n') {
                    byte += 1;
                }
                position.line += 1;
                position.character = 0;
            } else {
                position.character += c.len_utf16() as u32;
            }
        }
        result.push(position);
    }
    result
}

pub fn analyze(text: &str, fold_limit: usize) -> Result<Analysis, &'static str> {
    let lexed = lex(text).map_err(|_| "lexical analysis resource limit exceeded")?;
    let mut analysis = Analysis::default();
    for d in lexed.diagnostics {
        analysis.diagnostic(d.start, d.end, "lexical", d.message);
    }
    let mut stack = Vec::new();
    // Tracking line numbers here prevents single-line pairs consuming the fold budget.
    let mut line = 0;
    let mut byte = 0;
    let mut indentation_only = true;
    let mut comments: Option<(usize, usize, usize, usize)> = None;
    for token in lexed.tokens {
        let start_line = line;
        let whole_line_comment = token.kind == TokenKind::LineComment && indentation_only;
        while byte < token.end {
            match text.as_bytes()[byte] {
                b'\r' => {
                    line += 1;
                    indentation_only = true;
                    if text.as_bytes().get(byte + 1) == Some(&b'\n') {
                        byte += 1;
                    }
                }
                b'\n' => {
                    line += 1;
                    indentation_only = true;
                }
                b' ' | b'\t' | b'\x0c' => {}
                _ => indentation_only = false,
            }
            byte += 1;
        }
        if token.kind != TokenKind::Whitespace {
            // Complete a separated group before this token can complete another fold.
            match comments.take() {
                Some((start, _, first_line, last_line))
                    if whole_line_comment && start_line == last_line + 1 =>
                {
                    comments = Some((start, token.end, first_line, start_line));
                }
                previous => {
                    if let Some((start, end, first_line, last_line)) = previous
                        && first_line < last_line
                    {
                        analysis.fold(start, end, "comment", fold_limit);
                    }
                    if whole_line_comment {
                        comments = Some((token.start, token.end, start_line, start_line));
                    }
                }
            }
        }
        match token.kind {
            TokenKind::LeftBrace | TokenKind::LeftParen | TokenKind::LeftBracket => {
                if stack.len() == MAX_DEPTH {
                    return Err("structural delimiter depth limit exceeded");
                }
                stack.push((token.kind, token.start, token.end, start_line));
            }
            TokenKind::RightBrace | TokenKind::RightParen | TokenKind::RightBracket => {
                let expected = match token.kind {
                    TokenKind::RightBrace => TokenKind::LeftBrace,
                    TokenKind::RightParen => TokenKind::LeftParen,
                    _ => TokenKind::LeftBracket,
                };
                match stack.pop() {
                    Some((kind, start, _, opening_line)) if kind == expected => {
                        if opening_line < line {
                            analysis.fold(start, token.end, "region", fold_limit);
                        }
                    }
                    opening => {
                        analysis.diagnostic(
                            token.start,
                            token.end,
                            "structural",
                            "unexpected or mismatched closing delimiter",
                        );
                        // Discard pending opens after a mismatch; never invent crossing folds.
                        for (_, start, end, _) in opening.into_iter().chain(stack.drain(..)) {
                            analysis.diagnostic(start, end, "structural", "unclosed delimiter");
                        }
                    }
                }
            }
            TokenKind::BlockComment | TokenKind::RawString if start_line < line => {
                analysis.fold(
                    token.start,
                    token.end,
                    if token.kind == TokenKind::BlockComment {
                        "comment"
                    } else {
                        "region"
                    },
                    fold_limit,
                );
            }
            _ => {}
        }
    }
    if let Some((start, end, first_line, last_line)) = comments
        && first_line < last_line
    {
        analysis.fold(start, end, "comment", fold_limit);
    }
    for (_, start, end, _) in stack {
        analysis.diagnostic(start, end, "structural", "unclosed delimiter");
    }
    Ok(analysis)
}

pub fn limit_diagnostic(message: &str) -> Vec<Value> {
    vec![
        json!({"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"severity":2,"source":"ktlsp-lexical-structural","code":"analysis-limit","message":message}),
    ]
}
