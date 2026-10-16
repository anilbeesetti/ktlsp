use ktlsp_syntax::{
    Lexed, MAX_DIAGNOSTICS, MAX_NESTING, MAX_SOURCE_BYTES, MAX_TOKENS, SyntaxError, TokenKind, lex,
};

fn partition(source: &str, result: &Lexed) {
    let mut end = 0;
    for token in &result.tokens {
        assert_eq!(token.start, end, "{source:?}");
        assert!(token.end > token.start);
        assert!(source.is_char_boundary(token.start));
        assert!(source.is_char_boundary(token.end));
        end = token.end;
    }
    assert_eq!(end, source.len(), "{source:?}");
    for diagnostic in &result.diagnostics {
        assert!(diagnostic.start <= diagnostic.end);
        assert!(source.is_char_boundary(diagnostic.start));
        assert!(source.is_char_boundary(diagnostic.end));
    }
}

fn tokens(source: &str) -> Vec<(TokenKind, &str)> {
    let result = lex(source).unwrap();
    partition(source, &result);
    assert!(
        result.diagnostics.is_empty(),
        "{source:?}: {:?}",
        result.diagnostics
    );
    result
        .tokens
        .iter()
        .map(|t| (t.kind, &source[t.start..t.end]))
        .collect()
}

#[test]
fn lossless_source_with_unicode_comments_delimiters_and_crlf() {
    use TokenKind::*;
    assert_eq!(
        tokens("val 名٢ = f(1, [2]);\r\n/*outer /*inner*/ done*/ //tail"),
        vec![
            (Keyword, "val"),
            (Whitespace, " "),
            (Identifier, "名٢"),
            (Whitespace, " "),
            (Operator, "="),
            (Whitespace, " "),
            (Identifier, "f"),
            (LeftParen, "("),
            (Number, "1"),
            (Comma, ","),
            (Whitespace, " "),
            (LeftBracket, "["),
            (Number, "2"),
            (RightBracket, "]"),
            (RightParen, ")"),
            (Semicolon, ";"),
            (Whitespace, "\r\n"),
            (BlockComment, "/*outer /*inner*/ done*/"),
            (Whitespace, " "),
            (LineComment, "//tail")
        ]
    );
    assert!(lex("").unwrap().tokens.is_empty());
    assert_eq!(tokens("#! /usr/bin/env kotlin\n")[0].0, Shebang);
}

#[test]
fn unicode_categories_and_soft_keywords() {
    assert_eq!(
        tokens("_名१२ 𐐀 const suspend value")
            .iter()
            .filter(|t| t.0 != TokenKind::Whitespace)
            .count(),
        5
    );
    for source in ["e\u{301}", "a²", "\u{345}", "🙂", "\0", "\u{a0}"] {
        let result = lex(source).unwrap();
        partition(source, &result);
        assert!(
            result.tokens.iter().any(|t| t.kind == TokenKind::Unknown),
            "{source:?}"
        );
    }
}

#[test]
fn mixed_identifier_runs_preserve_categories_boundaries_and_byte_spans() {
    use TokenKind::*;
    let source = "aZ_09éǅː名𐐀٢z,e\u{301}x;a²b as? as名? class class9 _";
    let expected = [
        (Identifier, "aZ_09éǅː名𐐀٢z"),
        (Comma, ","),
        (Identifier, "e"),
        (Unknown, "\u{301}"),
        (Identifier, "x"),
        (Semicolon, ";"),
        (Identifier, "a"),
        (Unknown, "²"),
        (Identifier, "b"),
        (Whitespace, " "),
        (Operator, "as?"),
        (Whitespace, " "),
        (Identifier, "as名"),
        (Operator, "?"),
        (Whitespace, " "),
        (Keyword, "class"),
        (Whitespace, " "),
        (Identifier, "class9"),
        (Whitespace, " "),
        (Identifier, "_"),
    ];
    let result = lex(source).unwrap();
    partition(source, &result);
    assert_eq!(result.tokens.len(), expected.len());
    let mut offset = 0;
    let mut rejected_spans = Vec::new();
    for (token, (kind, text)) in result.tokens.iter().zip(expected) {
        let end = offset + text.len();
        assert_eq!((token.kind, token.start, token.end), (kind, offset, end));
        assert_eq!(&source[token.start..token.end], text);
        if kind == Unknown {
            rejected_spans.push((offset, end));
        }
        offset = end;
    }
    assert_eq!(offset, source.len());
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.start, diagnostic.end))
            .collect::<Vec<_>>(),
        rejected_spans
    );
}

#[test]
fn templates_are_opaque_including_nested_strings_comments_and_raw_strings() {
    let fixtures = [
        r#""${f("x", { 1 })}""#,
        r#""a${f("nested ${g("x")}")}z""#,
        r#""${ /* } " /* nested */ */ f('}', `a}b`) }""#,
        "\"${ // }\"\n f(1) }\"",
        "\"${\"\"\"raw } \" ${f(\"a\")}\"\"\"}\"",
        r#""""raw ${f("x", { "${1}" })} end""""#,
        r#""escaped \" quote \${ not_template }""#,
        "\"\"\"raw \\${f(\"x\")}\"\"\"",
        "\"\"\"trailing quote\"\"\"\"\"",
    ];
    for source in fixtures {
        let result = tokens(source);
        assert_eq!(result.len(), 1, "{source:?}: {result:?}");
        assert!(matches!(
            result[0].0,
            TokenKind::String | TokenKind::RawString
        ));
        assert_eq!(result[0].1, source);
        let surrounded = format!("{{{source}}}");
        let result = tokens(&surrounded);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, TokenKind::LeftBrace);
        assert_eq!(result[2].0, TokenKind::RightBrace);
    }
}

#[test]
fn backticks_and_characters() {
    assert_eq!(
        tokens("`when` `spaces {} quotes \"` 'x' '\\u0041' '\\n'")
            .iter()
            .filter(|t| t.0 != TokenKind::Whitespace)
            .count(),
        5
    );
    for source in [
        "``",
        "`unterminated",
        "`line\nbreak",
        "''",
        "'ab'",
        "'🙂'",
        "'x",
        "'\\q'",
        "'\\u12'",
        "'\\\n",
    ] {
        let result = lex(source).unwrap();
        partition(source, &result);
        assert!(!result.diagnostics.is_empty(), "{source:?}");
    }
}

#[test]
fn numeric_forms_and_ranges() {
    for source in [
        "0", "123", "1__2", "0xA__f", "0b10", "123uL", "1l", "0XFFUL", "1.5", ".5", "0.1e-2F",
        "01.0", "12e+2", "3f",
    ] {
        let result = tokens(source);
        assert_eq!(result, vec![(TokenKind::Number, source)], "{source}");
    }
    assert_eq!(
        tokens("1..2 0..<9")
            .iter()
            .filter(|t| t.0 == TokenKind::Operator)
            .map(|t| t.1)
            .collect::<Vec<_>>(),
        vec!["..", "..<"]
    );
    for source in ["0x", "0x_1", "0b102", "12_", "01", "1e", "1e+", ".5e_2"] {
        let result = lex(source).unwrap();
        partition(source, &result);
        assert!(!result.diagnostics.is_empty(), "{source}");
    }
}

#[test]
fn longest_operators_and_keyword_boundaries() {
    for source in [
        "!==", "===", "...", "..<", "!in", "!is", "as?", "++", "=>", ";;", "?.", "?:", "::", "->",
        "%=",
    ] {
        assert_eq!(tokens(source), vec![(TokenKind::Operator, source)]);
    }
    assert_eq!(
        tokens("!inside"),
        vec![
            (TokenKind::Operator, "!"),
            (TokenKind::Identifier, "inside")
        ]
    );
    assert_eq!(tokens("classy"), vec![(TokenKind::Identifier, "classy")]);
}

#[test]
fn malformed_eof_and_line_recovery_are_lossless() {
    for source in [
        "/*",
        "/* /*",
        "\"",
        "\"\\",
        "\"\\uZZZZ\"",
        "\"${f(\"x",
        "\"\"\"raw",
        "\"line\r\nval next = 1",
        "\"${ /*",
        "\"${ //tail",
    ] {
        let result = lex(source).unwrap();
        partition(source, &result);
        assert!(!result.diagnostics.is_empty(), "{source:?}");
    }
    let source = "\"bad\n{}";
    assert!(
        lex(source)
            .unwrap()
            .tokens
            .iter()
            .any(|t| t.kind == TokenKind::LeftBrace)
    );
}

#[test]
fn budgets_fail_explicitly_at_boundaries() {
    assert!(lex(&" ".repeat(MAX_SOURCE_BYTES)).is_ok());
    assert_eq!(
        lex(&" ".repeat(MAX_SOURCE_BYTES + 1)),
        Err(SyntaxError::SourceTooLarge)
    );
    assert_eq!(
        lex(&".".repeat(MAX_TOKENS * 3 + 1)),
        Err(SyntaxError::TooManyTokens)
    );
    let result = lex(&"@ ".repeat(MAX_TOKENS / 2)).unwrap();
    assert_eq!(result.tokens.len(), MAX_TOKENS);
    assert_eq!(
        lex(&format!("{}@", "@ ".repeat(MAX_TOKENS / 2))),
        Err(SyntaxError::TooManyTokens)
    );
    assert_eq!(
        lex(&"\0".repeat(MAX_DIAGNOSTICS))
            .unwrap()
            .diagnostics
            .len(),
        MAX_DIAGNOSTICS
    );
    assert_eq!(
        lex(&"\0".repeat(MAX_DIAGNOSTICS + 1)),
        Err(SyntaxError::TooManyDiagnostics)
    );
    let comment = format!("{}{}", "/*".repeat(MAX_NESTING), "*/".repeat(MAX_NESTING));
    assert!(lex(&comment).unwrap().diagnostics.is_empty());
    assert_eq!(
        lex(&"/*".repeat(MAX_NESTING + 1)),
        Err(SyntaxError::NestingTooDeep)
    );
    let template = format!(
        "\"${{{}{}}}\"",
        "{".repeat(MAX_NESTING - 2),
        "}".repeat(MAX_NESTING - 2)
    );
    assert!(lex(&template).unwrap().diagnostics.is_empty());
    assert_eq!(
        lex(&format!("\"${{{}", "{".repeat(MAX_NESTING - 1))),
        Err(SyntaxError::NestingTooDeep)
    );
}

#[test]
fn deterministic_generated_utf8_crash_and_partition_corpus() {
    let alphabet = [
        '\0', '\r', '\n', '\t', ' ', 'a', '名', '𐐀', '🙂', '\u{301}', '١', '_', '0', '1', '9', 'x',
        'e', 'u', '$', '\\', '\'', '"', '`', '/', '*', '{', '}', '(', ')', '[', ']', '+', '-', '.',
        '#', '?',
    ];
    let mut state = 0x5a17_2e39_u64;
    for case in 0..4096 {
        let mut source = String::new();
        for _ in 0..case % 511 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push(alphabet[(state >> 32) as usize % alphabet.len()]);
        }
        match lex(&source) {
            Ok(result) => partition(&source, &result),
            Err(error) => assert!(matches!(
                error,
                SyntaxError::TooManyDiagnostics | SyntaxError::NestingTooDeep
            )),
        }
    }
}

#[test]
fn multi_dollar_strings_preserve_prefix_and_activation_count() {
    for source in [
        "$$\"literal ${unbalanced and $name}\"",
        "$$\"literal ${ never closes\"",
        "$$\"active $${f(\"nested\", { 1 })} literal ${ not active\"",
        "$$\"\"\"literal ${not active} $${f(\"nested\")} end\"\"\"",
        "$$$\"\"\"literal $${ never closes $$$${f(\"nested\")} end\"\"\"",
        "\"${f($$\"literal ${not active\", $$\"$${g(\"inner\")}\")}\"",
        "$$\"escaped \\$${not active} and $${1}\"",
    ] {
        let result = tokens(source);
        assert_eq!(result.len(), 1, "{source:?}: {result:?}");
        assert!(matches!(
            result[0].0,
            TokenKind::String | TokenKind::RawString
        ));
        assert_eq!(result[0].1, source);
        let surrounded = format!("{{{source}}}");
        let result = tokens(&surrounded);
        assert_eq!(result.len(), 3, "{source:?}: {result:?}");
    }
    let source = "$$\"$${f(\"inner\")\"";
    let result = lex(source).unwrap();
    partition(source, &result);
    assert_eq!(result.tokens.len(), 1);
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn long_dollar_runs_have_linear_progress() {
    let dollars = "$".repeat(256 * 1024);
    let result = lex(&dollars).unwrap();
    partition(&dollars, &result);
    assert_eq!(result.tokens.len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    let template = format!("\"${{{dollars}}}\"");
    assert_eq!(tokens(&template).len(), 1);
    let prefixed = format!("{dollars}\"literal ${{ brace\"");
    assert_eq!(tokens(&prefixed).len(), 1);
}
