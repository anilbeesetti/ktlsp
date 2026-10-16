//! Relative semantic-token transport for positively known lexer categories only.
use ktlsp_syntax::{TokenKind, lex};
use ktlsp_text::{Position, Range};
use serde_json::{Value, json};

const TYPES: [&str; 5] = ["keyword", "comment", "string", "number", "operator"];
const MAX_FRAGMENTS: usize = 10_000;
const MAX_ENCODED_BYTES: usize = 1024 * 1024;
// Five u32 decimal values plus separators; includes the enclosing {"data":[]}.
const FRAGMENT_BYTES: usize = 5 * 11;
const LIMIT: (i32, &str) = (-32803, "semantic token resource limit exceeded");
const INVALID_RANGE: (i32, &str) = (-32602, "invalid UTF-16 semantic token range");

#[derive(Debug)]
pub(crate) struct Client {
    types: Vec<&'static str>,
    indices: [Option<u32>; 5],
    full: bool,
    range: bool,
}
impl Client {
    pub(crate) fn negotiate(capabilities: &Value) -> Option<Self> {
        let tokens = capabilities.get("textDocument")?.get("semanticTokens")?.as_object()?;
        for key in ["dynamicRegistration", "overlappingTokenSupport", "multilineTokenSupport", "serverCancelSupport", "augmentsSyntaxTokens"] {
            if tokens.get(key).is_some_and(|value| !value.is_boolean()) {
                return None;
            }
        }
        let strings = |key| {
            let values = tokens.get(key)?.as_array()?;
            values.iter().all(Value::is_string).then_some(values)
        };
        let supported = strings("tokenTypes")?;
        strings("tokenModifiers")?;
        if !strings("formats")?.iter().any(|format| format == "relative") {
            return None;
        }
        let requests = tokens.get("requests")?.as_object()?;
        let request = |key: &str| -> Option<bool> {
            match requests.get(key) {
                None => Some(false),
                Some(Value::Bool(value)) => Some(*value),
                Some(Value::Object(options)) => {
                    if key == "full" && options.get("delta").is_some_and(|value| !value.is_boolean()) {
                        return None;
                    }
                    Some(true)
                }
                _ => None,
            }
        };
        let full = request("full")?;
        let range = request("range")?;
        if !full && !range {
            return None;
        }
        let mut client = Self { types: Vec::new(), indices: [None; 5], full, range };
        for (category, name) in TYPES.into_iter().enumerate() {
            if supported.iter().any(|value| value == name) {
                client.indices[category] = Some(client.types.len() as u32);
                client.types.push(name);
            }
        }
        (!client.types.is_empty()).then_some(client)
    }

    pub(crate) fn options(&self) -> Value {
        let mut options = json!({"legend":{"tokenTypes":self.types,"tokenModifiers":[]}});
        if self.full {
            options["full"] = json!(true);
        }
        if self.range {
            options["range"] = json!(true);
        }
        options
    }

    pub(crate) fn supports(&self, range: bool) -> bool {
        if range { self.range } else { self.full }
    }

    pub(crate) fn render(&self, text: &str, range: Option<Range>) -> Result<Value, (i32, &'static str)> {
        if range.is_some_and(|range| range.start > range.end) {
            return Err(INVALID_RANGE);
        }
        let lexed = lex(text);
        let lexical_error = lexed.is_err();
        let mut output = Output::default();
        let mut output_error = None;
        let mut position = Position { line: 0, character: 0 };
        let mut start_seen = range.is_none();
        let mut end_seen = range.is_none();
        // One source/position sweep also validates strict range endpoints: neither
        // surrogate interiors nor the byte inside CRLF is ever a cursor position.
        let spans = lexed.into_iter().flat_map(|lexed| lexed.tokens).map(|token| {
            // ponytail: identifiers and opaque template interiors have no semantic
            // identity here; upgrade with parser-backed resolution, not name guesses.
            let category = match token.kind {
                TokenKind::Keyword => Some(0),
                TokenKind::LineComment | TokenKind::Shebang | TokenKind::BlockComment => Some(1),
                TokenKind::String | TokenKind::RawString | TokenKind::Character => Some(2),
                TokenKind::Number => Some(3),
                TokenKind::Operator => Some(4),
                _ => None,
            };
            let index = category.and_then(|category| self.indices[category]);
            (index, &text[token.start..token.end])
        });
        // On lexical exhaustion, validate coordinates over one unclassified span
        // before returning failure; an invalid range must still be InvalidParams.
        for (index, text) in spans.chain((lexical_error && range.is_some()).then_some((None, text))) {
            let mut fragment = position;
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                if let Some(range) = range {
                    start_seen |= position == range.start;
                    end_seen |= position == range.end;
                }
                if matches!(c, '\r' | '\n') {
                    if output_error.is_none() {
                        output_error = output.fragment(fragment, position, index, range).err();
                    }
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    position.line = position.line.checked_add(1).ok_or(LIMIT)?;
                    position.character = 0;
                    fragment = position;
                } else {
                    position.character = position.character.checked_add(c.len_utf16() as u32).ok_or(LIMIT)?;
                }
            }
            if output_error.is_none() {
                output_error = output.fragment(fragment, position, index, range).err();
            }
        }
        if let Some(range) = range {
            start_seen |= position == range.start;
            end_seen |= position == range.end;
        }
        if !start_seen || !end_seen {
            return Err(INVALID_RANGE);
        }
        if lexical_error {
            return Err(LIMIT);
        }
        if let Some(error) = output_error {
            return Err(error);
        }
        Ok(json!({"data":output.data}))
    }
}

#[derive(Default)]
struct Output {
    data: Vec<u32>,
    previous: Option<Position>,
}
impl Output {
    fn fragment(&mut self, mut start: Position, mut end: Position, index: Option<u32>, range: Option<Range>) -> Result<(), (i32, &'static str)> {
        let Some(index) = index else { return Ok(()); };
        if let Some(range) = range {
            start = start.max(range.start);
            end = end.min(range.end);
        }
        if start >= end {
            return Ok(());
        }
        let fragments = self.data.len() / 5;
        let next_bytes = fragments.checked_add(1).and_then(|count| count.checked_mul(FRAGMENT_BYTES)).and_then(|bytes| bytes.checked_add(11)).ok_or(LIMIT)?;
        if fragments == MAX_FRAGMENTS || next_bytes > MAX_ENCODED_BYTES {
            return Err(LIMIT);
        }
        let previous = self.previous.unwrap_or(Position { line: 0, character: 0 });
        let line = start.line.checked_sub(previous.line).ok_or(LIMIT)?;
        let column = if line == 0 { start.character.checked_sub(previous.character).ok_or(LIMIT)? } else { start.character };
        let length = end.character.checked_sub(start.character).ok_or(LIMIT)?;
        self.data.extend_from_slice(&[line, column, length, index, 0]);
        self.previous = Some(start);
        Ok(())
    }
}
