//! `KotlinExpressionParsing.java` lines 648-760: string templates.

use ktrs_syntax::SyntaxKind::{self, *};

use crate::kt_tokens::KEYWORDS;
use crate::parsing::Parser;

/// `KEYWORD_TEXTS.get(text)`: `KEYWORDS` keyed by their text.
fn keyword_texts_get(text: Option<&str>) -> Option<SyntaxKind> {
    text.and_then(SyntaxKind::hard_keyword).filter(|&k| KEYWORDS.contains(k))
}

impl Parser {
    /*
     * stringTemplate
     *   : INTERPOLATION_PREFIX OPEN_QUOTE stringTemplateElement* CLOSING_QUOTE
     *   ;
     */
    pub(crate) fn parse_string_template(&mut self) {
        debug_assert!(self._at(INTERPOLATION_PREFIX) || self._at(OPEN_QUOTE));

        let template = self.mark();

        if self.at(INTERPOLATION_PREFIX) {
            let mark = self.mark();
            self.advance(); // INTERPOLATION_PREFIX
            mark.done(self, STRING_INTERPOLATION_PREFIX);
        }

        debug_assert!(self._at(OPEN_QUOTE));
        self.advance(); // OPEN_QUOTE

        while !self.eof() {
            if self.at(CLOSING_QUOTE) || self.at(DANGLING_NEWLINE) {
                break;
            }
            self.parse_string_template_element();
        }

        if self.at(DANGLING_NEWLINE) {
            self.error_and_advance("Expecting '\"'");
        } else {
            self.expect_2(CLOSING_QUOTE, "Expecting '\"'");
        }
        template.done(self, STRING_TEMPLATE);
    }

    /*
     * stringTemplateElement
     *   : RegularStringPart
     *   : ShortTemplateEntrySTART (SimpleName | "this")
     *   : EscapeSequence
     *   : longTemplate
     *   ;
     *
     * longTemplate
     *   : "${" expression "}"
     *   ;
     */
    pub(crate) fn parse_string_template_element(&mut self) {
        if self.at(REGULAR_STRING_PART) {
            let mark = self.mark();
            self.advance(); // REGULAR_STRING_PART
            mark.done(self, LITERAL_STRING_TEMPLATE_ENTRY);
        } else if self.at(ESCAPE_SEQUENCE) {
            let mark = self.mark();
            self.advance(); // ESCAPE_SEQUENCE
            mark.done(self, ESCAPE_STRING_TEMPLATE_ENTRY);
        } else if self.at(SHORT_TEMPLATE_ENTRY_START) {
            let entry = self.mark();
            self.advance(); // SHORT_TEMPLATE_ENTRY_START

            if self.at(THIS_KEYWORD) {
                let this_expression = self.mark();
                let reference = self.mark();
                self.advance(); // THIS_KEYWORD
                reference.done(self, REFERENCE_EXPRESSION);
                this_expression.done(self, THIS_EXPRESSION);
            } else {
                let keyword = keyword_texts_get(self.my_builder.get_token_text());
                if let Some(keyword) = keyword {
                    self.my_builder.remap_current_token(keyword);
                    self.error_and_advance("Keyword cannot be used as a reference");
                } else {
                    let reference = self.mark();
                    self.expect_2(IDENTIFIER, "Expecting a name");
                    reference.done(self, REFERENCE_EXPRESSION);
                }
            }

            entry.done(self, SHORT_STRING_TEMPLATE_ENTRY);
        } else if self.at(LONG_TEMPLATE_ENTRY_START) {
            let long_template_entry = self.mark();

            self.advance(); // LONG_TEMPLATE_ENTRY_START

            while !self.eof() {
                let offset = self.my_builder.get_current_offset();

                self.parse_expression();

                if self._at(LONG_TEMPLATE_ENTRY_END) {
                    self.advance();
                    break;
                } else {
                    self.error("Expecting '}'");
                    if offset == self.my_builder.get_current_offset() {
                        // Prevent hang if can't advance with parseExpression()
                        self.advance();
                    }
                }
            }

            long_template_entry.done(self, LONG_STRING_TEMPLATE_ENTRY);
        } else {
            self.error_and_advance("Unexpected token in a string template");
        }
    }
}
