//! `KotlinExpressionParsing.java` lines 1045-1235: simple names, local declarations, function literals.

use ktrs_syntax::SyntaxKind::{self, *};

use super::{
    ARROW_COMMA_SET, ARROW_SET, TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA,
    TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA_RECOVERY,
};
use crate::builder::{EdgeBinder, Marker};
use crate::parsing::Parser;
use crate::parsing::declarations::{ModifierDetector, MultiDeclarationMode};
use crate::token_set::TokenSet;

impl Parser {
    /*
     * SimpleName
     */
    pub(crate) fn parse_simple_name_expression(&mut self) {
        let simple_name = self.mark();
        self.expect_2(IDENTIFIER, "Expecting an identifier");
        simple_name.done(self, REFERENCE_EXPRESSION);
    }

    /*
     * modifiers declarationRest
     */
    pub(crate) fn parse_local_declaration(&mut self, rollback_if_definitely_not_expression: bool, is_script_top_level: bool) -> bool {
        let decl = self.mark();
        let mut detector = ModifierDetector::default();
        self.parse_modifier_list_3(Some(&mut detector), TokenSet::EMPTY, /* localDeclaration = */ true);

        let decl_type =
            self.parse_local_declaration_rest(&mut detector, rollback_if_definitely_not_expression, is_script_top_level);

        if let Some(decl_type) = decl_type {
            // we do not attach preceding comments (non-doc) to local variables because they are likely commenting a few statements below
            self.close_declaration_with_comment_binders(
                decl,
                decl_type,
                decl_type != PROPERTY && decl_type != DESTRUCTURING_DECLARATION,
            );
            true
        } else {
            decl.rollback_to(self);
            false
        }
    }

    /*
     * functionLiteral  // one can use "it" as a parameter name
     *   : "{" expressions "}"
     *   : "{" (modifiers SimpleName (":" type)?){","} "->" statements "}"
     *   ;
     */
    pub(crate) fn parse_function_literal(&mut self) {
        self.parse_function_literal_2(/* preferBlock = */ false, /* collapse = */ true);
    }

    /// If it has no `->`, it's a block, otherwise a function literal.
    pub(crate) fn parse_function_literal_2(&mut self, prefer_block: bool, collapse: bool) {
        debug_assert!(self._at(LBRACE));

        let literal_expression = self.mark();

        let literal = self.mark();

        self.my_builder.enable_newlines();
        self.advance(); // LBRACE

        let mut params_found = false;

        let token = self.tt();
        if token == Some(ARROW) {
            //   { -> ...}
            let m = self.mark();
            m.done(self, VALUE_PARAMETER_LIST);
            self.advance(); // ARROW
            params_found = true;
        } else if matches!(token, Some(IDENTIFIER | COLON | LPAR | LBRACKET)) {
            // Try to parse a simple name list followed by an ARROW
            //   {a -> ...}
            //   {a, b -> ...}
            //   {(a, b) -> ... }
            let rollback_marker = self.mark();
            let next_token = self.lookahead(1);
            let prefer_params_to_expressions = next_token == Some(COMMA) || next_token == Some(COLON);
            self.parse_function_literal_parameter_list();

            params_found = if prefer_params_to_expressions {
                self.rollback_or_drop(rollback_marker, ARROW, "An -> is expected", RBRACE)
            } else {
                self.rollback_or_drop_at(rollback_marker, ARROW)
            };
        }

        if !params_found && prefer_block {
            literal.drop(self);
            self.parse_statements();
            self.expect_2(RBRACE, "Expecting '}'");
            literal_expression.done(self, BLOCK);
            self.my_builder.restore_newlines_state();

            return;
        }

        if collapse && self.is_lazy {
            self.advance_balanced_block();
            literal.done(self, FUNCTION_LITERAL);
            literal_expression.collapse(self, LAMBDA_EXPRESSION);
        } else {
            let body = self.mark();
            self.parse_statements();

            body.done(self, BLOCK);
            body.set_custom_edge_token_binders(
                self,
                Some(EdgeBinder::PrecedingAllComments),
                Some(EdgeBinder::TrailingAllComments),
            );

            self.expect_2(RBRACE, "Expecting '}'");
            literal.done(self, FUNCTION_LITERAL);
            literal_expression.done(self, LAMBDA_EXPRESSION);
        }

        self.my_builder.restore_newlines_state();
    }

    pub(crate) fn rollback_or_drop_at(&mut self, rollback_marker: Marker, drop_at: SyntaxKind) -> bool {
        if self.at(drop_at) {
            self.advance(); // dropAt
            rollback_marker.drop(self);
            return true;
        }
        rollback_marker.rollback_to(self);
        false
    }

    pub(crate) fn rollback_or_drop(
        &mut self,
        rollback_marker: Marker,
        expected: SyntaxKind,
        expect_message: &str,
        valid_for_drop: SyntaxKind,
    ) -> bool {
        if self.at(expected) {
            self.advance(); // dropAt
            rollback_marker.drop(self);
            return true;
        } else if self.at(valid_for_drop) {
            rollback_marker.drop(self);
            self.expect_2(expected, expect_message);
            return true;
        }

        rollback_marker.rollback_to(self);
        false
    }

    /*
     * lambdaParameter{","}
     *
     * lambdaParameter
     *   : variableDeclarationEntry
     *   : multipleVariableDeclarations (":" type)?
     */
    pub(crate) fn parse_function_literal_parameter_list(&mut self) {
        let parameter_list = self.mark();

        while !self.eof() {
            if self.at(ARROW) {
                break;
            }
            let parameter = self.mark();

            if self.at(COLON) {
                self.error("Expecting parameter name");
            } else if self.at(LPAR) || self.at(LBRACKET) {
                let destructuring_declaration = self.mark();
                // No var in lambda parameter destructuring
                let mode = if self.lookahead(1) == Some(VAL_KEYWORD) {
                    MultiDeclarationMode::FullValOnly
                } else {
                    MultiDeclarationMode::Short
                };
                self.parse_multi_declaration_entry(
                    TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA,
                    TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA_RECOVERY,
                    mode,
                );
                destructuring_declaration.done(self, DESTRUCTURING_DECLARATION);
            } else {
                self.expect_3(IDENTIFIER, "Expecting parameter name", Some(ARROW_SET));
            }

            if self.at(COLON) {
                self.advance(); // COLON
                self.parse_type_ref_1(ARROW_COMMA_SET);
            }
            parameter.done(self, VALUE_PARAMETER);

            if self.at(ARROW) {
                break;
            } else if self.at(COMMA) {
                self.advance(); // COMMA
            } else {
                self.error("Expecting '->' or ','");
                break;
            }
        }

        parameter_list.done(self, VALUE_PARAMETER_LIST);
    }
}
