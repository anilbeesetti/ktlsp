//! Bounded lexical/structural providers; no declaration or compiler analysis.
use ktlsp_syntax::{TokenKind, lex};
use ktlsp_text::{Position, Range};
use serde_json::{Value, json};

pub const MAX_FOLDS: usize = 10_000;
pub const MAX_DIAGNOSTICS: usize = 256;
pub const MAX_DEPTH: usize = 1024;

#[derive(Debug)]
struct Fold {
    start: usize,
    end: usize,
    kind: &'static str,
}
#[derive(Debug)]
struct Diagnostic {
    start: usize,
    end: usize,
    code: &'static str,
    message: &'static str,
}
#[derive(Debug, Default)]
pub struct Analysis {
    folds: Vec<Fold>,
    diagnostics: Vec<Diagnostic>,
    last_non_whitespace_end: usize,
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
    pub fn render(&self, text: &str, line_only: bool) -> (Vec<Value>, Vec<Value>) {
        self.render_outputs(text, line_only, &self.folds)
    }
    pub fn diagnostics(&self, text: &str) -> Vec<Value> {
        self.render_outputs(text, false, &[]).1
    }
    pub fn trailing_closer(&self, text: &str) -> Option<(usize, usize, &'static str)> {
        let [diagnostic] = self.diagnostics.as_slice() else {
            return None;
        };
        if diagnostic.code != "structural"
            || diagnostic.message != "unexpected or mismatched closing delimiter"
            || diagnostic.end != self.last_non_whitespace_end
        {
            return None;
        }
        let title = match text.get(diagnostic.start..diagnostic.end)? {
            ")" => "Remove unmatched trailing ')'",
            "]" => "Remove unmatched trailing ']'",
            "}" => "Remove unmatched trailing '}'",
            _ => return None,
        };
        Some((diagnostic.start, diagnostic.end, title))
    }
    fn render_outputs(
        &self,
        text: &str,
        line_only: bool,
        folds: &[Fold],
    ) -> (Vec<Value>, Vec<Value>) {
        // Only retained output endpoints are indexed, then converted in one sweep.
        let mut offsets: Vec<_> = folds
            .iter()
            .flat_map(|f| [f.start, f.end])
            .chain(self.diagnostics.iter().flat_map(|d| [d.start, d.end]))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        let positions = positions(text, &offsets);
        let at = |offset| positions[offsets.binary_search(&offset).expect("retained endpoint")];
        let mut folds: Vec<_> = folds
            .iter()
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
        let diagnostics = self.diagnostics.iter().map(|d| {
            json!({"range":Range{start:at(d.start),end:at(d.end)},"severity":1,"source":"ktlsp-lexical-structural","code":d.code,"message":d.message})
        }).collect();
        (folds, diagnostics)
    }
}

// Sorted byte boundaries come only from lossless tokens and lexical diagnostics.
pub(crate) fn positions(text: &str, offsets: &[usize]) -> Vec<Position> {
    let mut result = Vec::with_capacity(offsets.len());
    let Ok(()) = visit_positions(
        text,
        offsets,
        |_| Ok::<_, std::convert::Infallible>(()),
        |position| result.push(position),
    );
    result
}

// The documentation consumer uses stack endpoints and observes its existing
// cancellation/work budget in this same UTF-16 sweep, without a second index.
pub(crate) fn visit_positions<E>(
    text: &str,
    offsets: &[usize],
    mut work: impl FnMut(usize) -> Result<(), E>,
    mut emit: impl FnMut(Position),
) -> Result<(), E> {
    let mut byte = 0;
    let mut position = Position {
        line: 0,
        character: 0,
    };
    for &offset in offsets {
        work(1)?;
        while byte < offset {
            work(1)?;
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
        emit(position);
    }
    Ok(())
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
            analysis.last_non_whitespace_end = token.end;
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
