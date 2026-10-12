//! `KotlinExpressionParsing.java` lines 1366-1487: `do`/`while`/`for` loops and control-structure bodies.

use ktrs_syntax::SyntaxKind::*;

use super::{
    COLON_IN_KEYWORD_SET, IN_KEYWORD_L_BRACE_RECOVERY_SET, IN_KEYWORD_L_BRACE_SET, IN_KEYWORD_R_PAR_COLON_SET,
    IN_KEYWORD_SET, L_PAR_L_BRACE_R_PAR_SET,
};
use crate::parsing::Parser;
use crate::parsing::declarations::{EXPRESSION_FIRST, MultiDeclarationMode};

impl Parser {
    /*
     * doWhile
     *   : "do" element "while" "(" element ")"
     *   ;
     */
    pub(crate) fn parse_do_while(&mut self) {
        debug_assert!(self._at(DO_KEYWORD));

        let r#loop = self.mark();

        self.advance(); // DO_KEYWORD

        if !self.at(WHILE_KEYWORD) {
            self.parse_loop_body();
        }

        if self.expect_2(WHILE_KEYWORD, "Expecting 'while' followed by a post-condition") {
            self.parse_condition();
        }

        r#loop.done(self, DO_WHILE);
    }

    /*
     * while
     *   : "while" "(" element ")" element
     *   ;
     */
    pub(crate) fn parse_while(&mut self) {
        debug_assert!(self._at(WHILE_KEYWORD));

        let r#loop = self.mark();

        self.advance(); // WHILE_KEYWORD

        self.parse_condition();

        self.parse_loop_body();

        r#loop.done(self, WHILE);
    }

    /*
     * for
     *   : "for" "(" annotations ("val" | "var")? (multipleVariableDeclarations | variableDeclarationEntry) "in" expression ")" expression
     *   ;
     *
     *   TODO: empty loop body (at the end of the block)?
     */
    pub(crate) fn parse_for(&mut self) {
        debug_assert!(self._at(FOR_KEYWORD));

        let r#loop = self.mark();

        self.advance(); // FOR_KEYWORD

        if self.expect_3(LPAR, "Expecting '(' to open a loop range", Some(EXPRESSION_FIRST)) {
            self.my_builder.disable_newlines();

            if !self.at(RPAR) {
                let parameter = self.mark();

                if !self.at(IN_KEYWORD) {
                    self.parse_modifier_list(IN_KEYWORD_R_PAR_COLON_SET);
                }

                if self.at(VAL_KEYWORD) || self.at(VAR_KEYWORD) {
                    self.advance(); // VAL_KEYWORD or VAR_KEYWORD
                }

                if self.at(LPAR) || self.at(LBRACKET) {
                    let destructuring_declaration = self.mark();
                    // No var in destructured loop parameter
                    let mode = if self.lookahead(1) == Some(VAL_KEYWORD) {
                        MultiDeclarationMode::FullValOnly
                    } else {
                        MultiDeclarationMode::Short
                    };
                    self.parse_multi_declaration_entry(IN_KEYWORD_L_BRACE_SET, IN_KEYWORD_L_BRACE_RECOVERY_SET, mode);
                    destructuring_declaration.done(self, DESTRUCTURING_DECLARATION);
                } else {
                    self.expect_3(IDENTIFIER, "Expecting a variable name", Some(COLON_IN_KEYWORD_SET));

                    if self.at(COLON) {
                        self.advance(); // COLON
                        self.parse_type_ref_1(IN_KEYWORD_SET);
                    }
                }
                parameter.done(self, VALUE_PARAMETER);

                if self.expect_3(IN_KEYWORD, "Expecting 'in'", Some(L_PAR_L_BRACE_R_PAR_SET)) {
                    let range = self.mark();
                    self.parse_expression();
                    range.done(self, LOOP_RANGE);
                }
            } else {
                self.error("Expecting a variable name");
            }

            self.expect_no_advance(RPAR, "Expecting ')'");
            self.my_builder.restore_newlines_state();
        }

        self.parse_loop_body();

        r#loop.done(self, FOR);
    }

    pub(crate) fn parse_control_structure_body(&mut self) {
        if !self.parse_annotated_lambda(/* preferBlock = */ true) {
            self.parse_block_level_expression();
        }
    }

    /*
     * element
     */
    pub(crate) fn parse_loop_body(&mut self) {
        let body = self.mark();
        if !self.at(SEMICOLON) {
            self.parse_control_structure_body();
        }
        body.done(self, BODY);
    }
}
