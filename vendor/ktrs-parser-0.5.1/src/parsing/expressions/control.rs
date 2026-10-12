//! `KotlinExpressionParsing.java` lines 1489-1658: `try`, `if`, conditions, jumps and `return`.

use ktrs_syntax::SyntaxKind::{self, *};

use super::TRY_CATCH_RECOVERY_TOKEN_SET;
use crate::kt_tokens::WHITE_SPACE_OR_COMMENT_BIT_SET;
use crate::parsing::Parser;
use crate::parsing::declarations::EXPRESSION_FIRST;

impl Parser {
    /*
     * try
     *   : "try" block catchBlock* finallyBlock?
     *   ;
     * catchBlock
     *   : "catch" "(" annotations SimpleName ":" userType ")" block
     *   ;
     *
     * finallyBlock
     *   : "finally" block
     *   ;
     */
    pub(crate) fn parse_try(&mut self) {
        debug_assert!(self._at(TRY_KEYWORD));

        let try_expression = self.mark();

        self.advance(); // TRY_KEYWORD

        self.parse_block();

        let mut catch_or_finally = false;
        while self.at(CATCH_KEYWORD) {
            catch_or_finally = true;
            let catch_block = self.mark();
            self.advance(); // CATCH_KEYWORD

            if self.at_set(TRY_CATCH_RECOVERY_TOKEN_SET) {
                self.error("Expecting exception variable declaration");
            } else {
                let parameters = self.mark();
                self.expect_3(LPAR, "Expecting '('", Some(TRY_CATCH_RECOVERY_TOKEN_SET));
                if !self.at_set(TRY_CATCH_RECOVERY_TOKEN_SET) {
                    self.parse_value_parameter(/*typeRequired = */ true);
                    if self.at(COMMA) {
                        self.advance(); // trailing comma
                    }
                    self.expect_3(RPAR, "Expecting ')'", Some(TRY_CATCH_RECOVERY_TOKEN_SET));
                } else {
                    self.error("Expecting exception variable declaration");
                }
                parameters.done(self, VALUE_PARAMETER_LIST);
            }

            if self.at(LBRACE) {
                self.parse_block();
            } else {
                self.error("Expecting a block: { ... }");
            }
            catch_block.done(self, CATCH);
        }

        if self.at(FINALLY_KEYWORD) {
            catch_or_finally = true;
            let finally_block = self.mark();

            self.advance(); // FINALLY_KEYWORD

            self.parse_block();

            finally_block.done(self, FINALLY);
        }

        if !catch_or_finally {
            self.error("Expecting 'catch' or 'finally'");
        }

        try_expression.done(self, TRY);
    }

    /*
     * if
     *   : "if" "(" element ")" element SEMI? ("else" element)?
     *   ;
     */
    pub(crate) fn parse_if(&mut self) {
        debug_assert!(self._at(IF_KEYWORD));

        let marker = self.mark();

        self.advance(); //IF_KEYWORD

        self.parse_condition();

        let then_branch = self.mark();
        if !self.at(ELSE_KEYWORD) && !self.at(SEMICOLON) {
            self.parse_control_structure_body();
        }
        if self.at(SEMICOLON) && self.lookahead(1) == Some(ELSE_KEYWORD) {
            self.advance(); // SEMICOLON
        }
        then_branch.done(self, THEN);

        // lookahead for arrow is needed to prevent capturing of whenEntry like "else -> "
        if self.at(ELSE_KEYWORD) && self.lookahead(1) != Some(ARROW) {
            self.advance(); // ELSE_KEYWORD

            let else_branch = self.mark();
            if !self.at(SEMICOLON) {
                self.parse_control_structure_body();
            }
            else_branch.done(self, ELSE);
        }

        marker.done(self, IF);
    }

    /*
     * "(" element ")"
     */
    pub(crate) fn parse_condition(&mut self) {
        self.my_builder.disable_newlines();

        if self.expect_3(LPAR, "Expecting a condition in parentheses '(...)'", Some(EXPRESSION_FIRST)) {
            let condition = self.mark();
            self.parse_expression();
            condition.done(self, CONDITION);
            self.expect_2(RPAR, "Expecting ')");
        }

        self.my_builder.restore_newlines_state();
    }

    /*
     * : "continue" getEntryPoint?
     * : "break" getEntryPoint?
     */
    pub(crate) fn parse_jump(&mut self, r#type: SyntaxKind) {
        debug_assert!(self._at(BREAK_KEYWORD) || self._at(CONTINUE_KEYWORD));

        let marker = self.mark();

        self.advance(); // BREAK_KEYWORD or CONTINUE_KEYWORD

        self.parse_label_reference_with_no_whitespace();

        marker.done(self, r#type);
    }

    /*
     * "return" getEntryPoint? element?
     */
    pub(crate) fn parse_return(&mut self) {
        debug_assert!(self._at(RETURN_KEYWORD));

        let return_expression = self.mark();

        self.advance(); // RETURN_KEYWORD

        self.parse_label_reference_with_no_whitespace();

        if self.at_set(EXPRESSION_FIRST) && !self.at(EOL_OR_SEMICOLON) {
            self.parse_expression();
        }

        return_expression.done(self, RETURN);
    }

    /*
     * labelReference?
     */
    pub(crate) fn parse_label_reference_with_no_whitespace(&mut self) {
        if self.at(AT) && !self.my_builder.newline_before_current_token() {
            if WHITE_SPACE_OR_COMMENT_BIT_SET.contains(self.my_builder.raw_lookup(-1)) {
                self.error("There should be no space or comments before '@' in label reference");
            }
            self.parse_label_reference();
        }
    }
}
