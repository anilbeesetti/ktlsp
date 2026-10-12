//! `KotlinExpressionParsing.java` lines 1660-1883: labels, `throw`, parenthesized/`this`/`super`
//! expressions, value arguments, object literals.

use ktrs_syntax::SyntaxKind::{self, *};

use super::ALLOW_NEWLINE_OPERATIONS;
use crate::parsing::Parser;
use crate::parsing::declarations::{EXPRESSION_FIRST, EXPRESSION_FOLLOW, NameParsingMode};

impl Parser {
    /*
     * IDENTIFIER "@"
     */
    pub(crate) fn parse_label_definition(&mut self) {
        debug_assert!(
            self.is_at_label_definition_or_missing_identifier(),
            "Callers must check that current token is IDENTIFIER followed with '@'"
        );

        let label_wrap = self.mark();
        let mark = self.mark();

        if self.at(AT) {
            self.error_and_advance("Expecting identifier before '@' in label definition");
            label_wrap.drop(self);
            mark.drop(self);
            return;
        }

        self.advance(); // IDENTIFIER
        self.advance(); // AT

        mark.done(self, LABEL);

        label_wrap.done(self, LABEL_QUALIFIER);
    }

    /*
     * "@" IDENTIFIER
     */
    pub(crate) fn parse_label_reference(&mut self) {
        debug_assert!(self._at(AT));

        let label_wrap = self.mark();

        let mark = self.mark();

        if self.my_builder.raw_lookup(1) != Some(IDENTIFIER) {
            self.error_and_advance("Label must be named"); // AT
            label_wrap.drop(self);
            mark.drop(self);
            return;
        }

        self.advance(); // AT
        self.advance(); // IDENTIFIER

        mark.done(self, LABEL);

        label_wrap.done(self, LABEL_QUALIFIER);
    }

    /*
     * : "throw" element
     */
    pub(crate) fn parse_throw(&mut self) {
        debug_assert!(self._at(THROW_KEYWORD));

        let marker = self.mark();

        self.advance(); // THROW_KEYWORD

        self.parse_expression();

        marker.done(self, THROW);
    }

    /*
     * "(" expression ")"
     */
    pub(crate) fn parse_parenthesized_expression(&mut self) {
        debug_assert!(self._at(LPAR));

        let mark = self.mark();

        self.my_builder.disable_newlines();
        self.advance(); // LPAR
        if self.at(RPAR) {
            self.error("Expecting an expression");
        } else {
            self.parse_expression();
        }

        self.expect_2(RPAR, "Expecting ')'");
        self.my_builder.restore_newlines_state();

        mark.done(self, PARENTHESIZED);
    }

    /*
     * "this" label?
     */
    pub(crate) fn parse_this_expression(&mut self) {
        debug_assert!(self._at(THIS_KEYWORD));
        let mark = self.mark();

        let this_reference = self.mark();
        self.advance(); // THIS_KEYWORD
        this_reference.done(self, REFERENCE_EXPRESSION);

        self.parse_label_reference_with_no_whitespace();

        mark.done(self, THIS_EXPRESSION);
    }

    /*
     * "this" ("<" type ">")? label?
     */
    pub(crate) fn parse_super_expression(&mut self) {
        debug_assert!(self._at(SUPER_KEYWORD));
        let mark = self.mark();

        let super_reference = self.mark();
        self.advance(); // SUPER_KEYWORD
        super_reference.done(self, REFERENCE_EXPRESSION);

        if self.at(LT) {
            // This may be "super < foo" or "super<foo>", thus the backtracking
            let supertype = self.mark();

            self.my_builder.disable_newlines();
            self.advance(); // LT

            self.parse_type_ref();

            if self.at(GT) {
                self.advance(); // GT
                supertype.drop(self);
            } else {
                supertype.rollback_to(self);
            }
            self.my_builder.restore_newlines_state();
        }
        self.parse_label_reference_with_no_whitespace();

        mark.done(self, SUPER_EXPRESSION);
    }

    /*
     * valueArguments
     *   : "(" (SimpleName "=")? "*"? element{","} ")"
     *   ;
     */
    pub(crate) fn parse_value_argument_list(&mut self) {
        let list = self.mark();

        self.my_builder.disable_newlines();

        if self.expect_3(LPAR, "Expecting an argument list", Some(EXPRESSION_FOLLOW)) {
            if !self.at(RPAR) {
                loop {
                    self.parse_value_argument();
                    if self.at(COLON) && self.lookahead(1) == Some(IDENTIFIER) {
                        self.error_and_advance_2("Unexpected type specification", 2);
                    }
                    if !self.at(COMMA) {
                        if self.at_set(EXPRESSION_FIRST) {
                            self.error("Expecting ','");
                            continue;
                        } else {
                            break;
                        }
                    }
                    self.advance(); // COMMA
                    if self.at(RPAR) {
                        break;
                    }
                }
            }

            self.expect_3(RPAR, "Expecting ')'", Some(EXPRESSION_FOLLOW));
        }

        self.my_builder.restore_newlines_state();

        list.done(self, VALUE_ARGUMENT_LIST);
    }

    /*
     * (SimpleName "=")? "*"? element
     */
    pub(crate) fn parse_value_argument(&mut self) {
        let argument = self.mark();
        if self.at(IDENTIFIER) && self.lookahead(1) == Some(EQ) {
            let arg_name = self.mark();
            let reference = self.mark();
            self.advance(); // IDENTIFIER
            reference.done(self, REFERENCE_EXPRESSION);
            arg_name.done(self, VALUE_ARGUMENT_NAME);
            self.advance(); // EQ
        }
        if self.at(MUL) {
            self.advance(); // MUL
        }
        self.parse_expression_1("Expecting an argument");
        argument.done(self, VALUE_ARGUMENT);
    }

    /*
     * "object" (":" delegationSpecifier{","})? classBody // Cannot make class body optional: foo(object : F, A)
     */
    pub(crate) fn parse_object_literal(&mut self) {
        let literal = self.mark();
        let declaration = self.mark();
        self.parse_object(NameParsingMode::Prohibited, false); // Body is not optional because of foo(object : A, B)
        declaration.done(self, OBJECT_DECLARATION);
        literal.done(self, OBJECT_LITERAL);
    }

    pub(crate) fn parse_one_token_expression(&mut self, r#type: SyntaxKind) {
        let mark = self.mark();
        self.advance();
        mark.done(self, r#type);
    }

    pub(crate) fn interrupted_with_new_line(&mut self) -> bool {
        let tt = self.tt();
        !ALLOW_NEWLINE_OPERATIONS.contains(tt) && self.my_builder.newline_before_current_token()
    }
}
