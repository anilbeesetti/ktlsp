//! Bounded, lossless lexical classification of Kotlin source.
//!
//! String templates are deliberately opaque: a string (including interpolation
//! expressions) produces one token. These tokens are not a parser, semantic
//! highlighting, or compiler diagnostics. Offsets are UTF-8 bytes.

use std::fmt;
use unicode_general_category::{GeneralCategory, get_general_category};

pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TOKENS: usize = 1_000_000;
pub const MAX_DIAGNOSTICS: usize = 256;
pub const MAX_NESTING: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    Shebang,
    BlockComment,
    Identifier,
    BacktickIdentifier,
    Keyword,
    Number,
    String,
    RawString,
    Character,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Operator,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalDiagnostic {
    pub start: usize,
    pub end: usize,
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<LexicalDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxError {
    SourceTooLarge,
    TooManyTokens,
    TooManyDiagnostics,
    NestingTooDeep,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceTooLarge => "source exceeds 8 MiB",
            Self::TooManyTokens => "source exceeds 1,000,000 tokens",
            Self::TooManyDiagnostics => "source exceeds 256 lexical diagnostics",
            Self::NestingTooDeep => "source exceeds 128 nested scanner contexts",
        })
    }
}
impl std::error::Error for SyntaxError {}

/// Classify source, preserving every byte. Malformed syntax yields diagnostics;
/// resource exhaustion yields an error, never a truncated token partition.
pub fn lex(source: &str) -> Result<Lexed, SyntaxError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SyntaxError::SourceTooLarge);
    }
    Lexer {
        source,
        pos: 0,
        result: Lexed::default(),
    }
    .run()
}

struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    result: Lexed,
}

#[derive(Clone, Copy)]
enum Context {
    String { start: usize, raw: bool },
    Template { start: usize },
    BlockComment { start: usize },
    LineComment,
}

fn is_letter(c: char) -> bool {
    c == '_'
        || matches!(
            get_general_category(c),
            GeneralCategory::UppercaseLetter
                | GeneralCategory::LowercaseLetter
                | GeneralCategory::TitlecaseLetter
                | GeneralCategory::ModifierLetter
                | GeneralCategory::OtherLetter
        )
}
fn is_digit(c: char) -> bool {
    get_general_category(c) == GeneralCategory::DecimalNumber
}
fn is_identifier_part(c: char) -> bool {
    is_letter(c) || is_digit(c)
}
fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "break"
            | "class"
            | "continue"
            | "do"
            | "else"
            | "false"
            | "for"
            | "fun"
            | "if"
            | "in"
            | "interface"
            | "is"
            | "null"
            | "object"
            | "package"
            | "return"
            | "super"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typealias"
            | "typeof"
            | "val"
            | "var"
            | "when"
            | "while"
    )
}

impl Lexer<'_> {
    fn tail(&self) -> &str {
        &self.source[self.pos..]
    }
    fn peek(&self) -> Option<char> {
        self.tail().chars().next()
    }
    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }
    fn diagnostic(
        &mut self,
        start: usize,
        end: usize,
        message: &'static str,
    ) -> Result<(), SyntaxError> {
        if self.result.diagnostics.len() == MAX_DIAGNOSTICS {
            return Err(SyntaxError::TooManyDiagnostics);
        }
        self.result.diagnostics.push(LexicalDiagnostic {
            start,
            end,
            message,
        });
        Ok(())
    }
    fn run(mut self) -> Result<Lexed, SyntaxError> {
        while let Some(c) = self.peek() {
            if self.result.tokens.len() == MAX_TOKENS {
                return Err(SyntaxError::TooManyTokens);
            }
            let start = self.pos;
            let kind = if matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}') {
                self.bump();
                while self
                    .peek()
                    .is_some_and(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}'))
                {
                    self.bump();
                }
                TokenKind::Whitespace
            } else if start == 0 && self.tail().starts_with("#!") {
                while self.peek().is_some_and(|c| c != '\r' && c != '\n') {
                    self.bump();
                }
                TokenKind::Shebang
            } else if self.tail().starts_with("//") {
                while self.peek().is_some_and(|c| c != '\r' && c != '\n') {
                    self.bump();
                }
                TokenKind::LineComment
            } else if self.tail().starts_with("/*") {
                self.pos += 2;
                self.scan_contexts(vec![Context::BlockComment { start }])?;
                TokenKind::BlockComment
            } else if c == '"' {
                let raw = self.tail().starts_with("\"\"\"");
                self.pos += if raw { 3 } else { 1 };
                self.scan_contexts(vec![Context::String { start, raw }])?;
                if raw {
                    TokenKind::RawString
                } else {
                    TokenKind::String
                }
            } else if c == '\'' {
                self.character()?;
                TokenKind::Character
            } else if c == '`' {
                self.backtick()?;
                TokenKind::BacktickIdentifier
            } else if c.is_ascii_digit()
                || (c == '.'
                    && self
                        .tail()
                        .as_bytes()
                        .get(1)
                        .is_some_and(u8::is_ascii_digit))
            {
                self.number()?;
                TokenKind::Number
            } else if is_letter(c) {
                self.bump();
                while self.peek().is_some_and(is_identifier_part) {
                    self.bump();
                }
                if &self.source[start..self.pos] == "as" && self.peek() == Some('?') {
                    self.bump();
                    TokenKind::Operator
                } else if is_keyword(&self.source[start..self.pos]) {
                    TokenKind::Keyword
                } else {
                    TokenKind::Identifier
                }
            } else {
                self.punctuation()?
            };
            self.result.tokens.push(Token {
                kind,
                start,
                end: self.pos,
            });
        }
        Ok(self.result)
    }
    fn push(stack: &mut Vec<Context>, context: Context) -> Result<(), SyntaxError> {
        if stack.len() == MAX_NESTING {
            return Err(SyntaxError::NestingTooDeep);
        }
        stack.push(context);
        Ok(())
    }
    // A single iterative stack handles comments, strings and brace-balanced
    // templates. Inner quotes/comments/braces never escape into file tokens.
    fn scan_contexts(&mut self, mut stack: Vec<Context>) -> Result<(), SyntaxError> {
        while let Some(context) = stack.last().copied() {
            if self.peek().is_none() {
                for context in stack.into_iter().rev() {
                    let (start, message) = match context {
                        Context::String { start, .. } => (start, "unterminated string"),
                        Context::Template { start } => (start, "unterminated string template"),
                        Context::BlockComment { start } => (start, "unterminated block comment"),
                        Context::LineComment => continue,
                    };
                    self.diagnostic(start, self.pos, message)?;
                }
                return Ok(());
            }
            match context {
                Context::BlockComment { .. } => {
                    if self.tail().starts_with("/*") {
                        Self::push(&mut stack, Context::BlockComment { start: self.pos })?;
                        self.pos += 2;
                    } else if self.tail().starts_with("*/") {
                        self.pos += 2;
                        stack.pop();
                    } else {
                        self.bump();
                    }
                }
                Context::LineComment => {
                    if matches!(self.peek(), Some('\r' | '\n')) {
                        stack.pop();
                    } else {
                        self.bump();
                    }
                }
                Context::String { start, raw } => {
                    if (raw && self.tail().starts_with("\"\"\""))
                        || (!raw && self.peek() == Some('"'))
                    {
                        if raw {
                            while self.peek() == Some('"') {
                                self.bump();
                            }
                        } else {
                            self.bump();
                        }
                        stack.pop();
                    } else if !raw && matches!(self.peek(), Some('\r' | '\n')) {
                        self.diagnostic(start, self.pos, "unterminated string")?;
                        stack.pop();
                    } else if !raw && self.peek() == Some('\\') {
                        self.escape()?;
                    } else if self.tail().starts_with("${") {
                        Self::push(&mut stack, Context::Template { start: self.pos })?;
                        self.pos += 2;
                    } else {
                        self.bump();
                    }
                }
                Context::Template { .. } => {
                    if self.tail().starts_with("//") {
                        Self::push(&mut stack, Context::LineComment)?;
                        self.pos += 2;
                    } else if self.tail().starts_with("/*") {
                        Self::push(&mut stack, Context::BlockComment { start: self.pos })?;
                        self.pos += 2;
                    } else if self.peek() == Some('"') {
                        let raw = self.tail().starts_with("\"\"\"");
                        Self::push(
                            &mut stack,
                            Context::String {
                                start: self.pos,
                                raw,
                            },
                        )?;
                        self.pos += if raw { 3 } else { 1 };
                    } else if self.peek() == Some('\'') {
                        if stack.len() == MAX_NESTING {
                            return Err(SyntaxError::NestingTooDeep);
                        }
                        self.character()?;
                    } else if self.peek() == Some('`') {
                        if stack.len() == MAX_NESTING {
                            return Err(SyntaxError::NestingTooDeep);
                        }
                        self.backtick()?;
                    } else if self.peek() == Some('{') {
                        Self::push(&mut stack, Context::Template { start: self.pos })?;
                        self.bump();
                    } else if self.peek() == Some('}') {
                        self.bump();
                        stack.pop();
                    } else {
                        self.bump();
                    }
                }
            }
        }
        Ok(())
    }
    fn escape(&mut self) -> Result<(), SyntaxError> {
        let start = self.pos;
        self.bump();
        match self.peek() {
            Some('t' | 'b' | 'n' | 'r' | '\'' | '"' | '\\' | '$') => self.bump(),
            Some('u') => {
                self.bump();
                let digits_start = self.pos;
                for _ in 0..4 {
                    if self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.pos - digits_start != 4 {
                    self.diagnostic(start, self.pos, "invalid Unicode escape")?;
                }
            }
            // Leave the terminator for the enclosing string/character recovery.
            None | Some('\r' | '\n') => {
                self.diagnostic(start, self.pos, "incomplete escape")?;
            }
            Some(_) => {
                self.bump();
                self.diagnostic(start, self.pos, "invalid escape")?;
            }
        }
        Ok(())
    }
    fn character(&mut self) -> Result<(), SyntaxError> {
        let start = self.pos;
        self.bump();
        let mut units = 0;
        while let Some(c) = self.peek() {
            if c == '\'' {
                self.bump();
                if units != 1 {
                    self.diagnostic(
                        start,
                        self.pos,
                        "character literal must contain one UTF-16 code unit",
                    )?;
                }
                return Ok(());
            }
            if matches!(c, '\r' | '\n') {
                break;
            }
            if c == '\\' {
                self.escape()?;
                units += 1;
            } else {
                self.bump();
                units += c.len_utf16();
            }
        }
        self.diagnostic(start, self.pos, "unterminated character literal")
    }
    fn backtick(&mut self) -> Result<(), SyntaxError> {
        let start = self.pos;
        self.bump();
        let content = self.pos;
        while self
            .peek()
            .is_some_and(|c| c != '`' && c != '\r' && c != '\n')
        {
            self.bump();
        }
        if self.peek() == Some('`') {
            let empty = content == self.pos;
            self.bump();
            if empty {
                self.diagnostic(start, self.pos, "empty backtick identifier")?;
            }
            Ok(())
        } else {
            self.diagnostic(start, self.pos, "unterminated backtick identifier")
        }
    }
    fn digits(&mut self, radix: u32) -> bool {
        let start = self.pos;
        let mut last_underscore = false;
        let mut valid = true;
        while let Some(c) = self.peek() {
            if c == '_' {
                // Consecutive underscores are permitted between digits.
                if self.pos == start {
                    valid = false;
                }
                last_underscore = true;
                self.bump();
            } else if c.is_ascii() && c.is_digit(radix) {
                last_underscore = false;
                self.bump();
            } else {
                break;
            }
        }
        self.pos > start && valid && !last_underscore
    }
    fn number(&mut self) -> Result<(), SyntaxError> {
        let start = self.pos;
        let mut valid = true;
        let mut float = false;
        if self.tail().starts_with("0x") || self.tail().starts_with("0X") {
            self.pos += 2;
            valid &= self.digits(16);
        } else if self.tail().starts_with("0b") || self.tail().starts_with("0B") {
            self.pos += 2;
            valid &= self.digits(2);
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
                valid = false;
                self.bump();
            }
        } else {
            if self.peek() != Some('.') {
                valid &= self.digits(10);
            }
            if self.peek() == Some('.')
                && self
                    .tail()
                    .as_bytes()
                    .get(1)
                    .is_some_and(u8::is_ascii_digit)
            {
                float = true;
                self.bump();
                valid &= self.digits(10);
            }
            if matches!(self.peek(), Some('e' | 'E')) {
                float = true;
                self.bump();
                if matches!(self.peek(), Some('+' | '-')) {
                    self.bump();
                }
                valid &= self.digits(10);
            }
            if matches!(self.peek(), Some('f' | 'F')) {
                float = true;
                self.bump();
            }
            if !float {
                let digits = &self.source[start..self.pos];
                if digits.starts_with('0') && digits.bytes().filter(u8::is_ascii_digit).count() > 1
                {
                    valid = false;
                }
            }
        }
        if !float {
            if matches!(self.peek(), Some('u' | 'U')) {
                self.bump();
            }
            if matches!(self.peek(), Some('l' | 'L')) {
                self.bump();
            }
        }
        if !valid {
            self.diagnostic(start, self.pos, "malformed numeric literal")?;
        }
        Ok(())
    }
    fn punctuation(&mut self) -> Result<TokenKind, SyntaxError> {
        for operator in [
            "!==", "===", "...", "..<", "!in", "!is", "++", "=>", ";;", "--", "<=", ">=", "==",
            "!=", "&&", "||", "!!", "?.", "?:", "::", "->", "..", "+=", "-=", "*=", "/=", "%=",
        ] {
            if self.tail().starts_with(operator) {
                if matches!(operator, "!in" | "!is")
                    && self.tail()[operator.len()..]
                        .chars()
                        .next()
                        .is_some_and(is_identifier_part)
                {
                    continue;
                }
                self.pos += operator.len();
                return Ok(TokenKind::Operator);
            }
        }
        let c = self.peek().expect("punctuation called with a character");
        self.bump();
        Ok(match c {
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            '.' => TokenKind::Dot,
            '+' | '-' | '*' | '/' | '%' | '<' | '>' | '=' | '!' | '?' | '@' | '#' => {
                TokenKind::Operator
            }
            _ => {
                self.diagnostic(self.pos - c.len_utf8(), self.pos, "unrecognized character")?;
                TokenKind::Unknown
            }
        })
    }
}
