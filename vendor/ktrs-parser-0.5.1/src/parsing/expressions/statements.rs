//! `KotlinExpressionParsing.java` lines 1237-1364: statements, block-level expressions, local declarations.

use ktrs_syntax::SyntaxKind::{self, *};

use super::{STATEMENT_FIRST, STATEMENT_NEW_LINE_QUICK_RECOVERY_SET};
use crate::parsing::Parser;
use crate::parsing::declarations::{
    AnnotationParsingMode, DeclarationParsingMode, EXPRESSION_FIRST, ModifierDetector, NameParsingMode,
};
use crate::token_set::TokenSet;

impl Parser {
    /*
     * expressions
     *   : SEMI* statement{SEMI+} SEMI*
     */
    pub(crate) fn parse_statements(&mut self) {
        self.parse_statements_1(false);
    }

    /*
     * expressions
     *   : SEMI* statement{SEMI+} SEMI*
     */
    pub(crate) fn parse_statements_1(&mut self, is_script_top_level: bool) {
        while self.at(SEMICOLON) {
            self.advance(); // SEMICOLON
        }
        while !self.eof() && !self.at(RBRACE) {
            if !self.at_set(STATEMENT_FIRST) {
                self.error_and_advance("Expecting an element");
            }
            if self.at_set(STATEMENT_FIRST) {
                self.parse_statement(is_script_top_level);
            }
            if self.at(SEMICOLON) {
                while self.at(SEMICOLON) {
                    self.advance(); // SEMICOLON
                }
            } else if self.at(RBRACE) {
                break;
            } else if !is_script_top_level && !self.my_builder.newline_before_current_token() {
                let several_statements_error = "Unexpected tokens (use ';' to separate expressions on the same line)";

                if self.at_set(STATEMENT_NEW_LINE_QUICK_RECOVERY_SET) {
                    self.error(several_statements_error);
                } else {
                    self.error_until(several_statements_error, TokenSet::create(&[EOL_OR_SEMICOLON, LBRACE, RBRACE]));
                }
            }
        }
    }

    /*
     * statement
     *  : declaration
     *  : blockLevelExpression
     *  ;
     */
    pub(crate) fn parse_statement(&mut self, is_script_top_level: bool) {
        if !self.parse_local_declaration(/* rollbackIfDefinitelyNotExpression = */ false, is_script_top_level) {
            if !self.at_set(EXPRESSION_FIRST) {
                self.error_and_advance("Expecting a statement");
            } else if is_script_top_level {
                let script_initializer = self.mark();
                self.parse_block_level_expression();
                script_initializer.done(self, SCRIPT_INITIALIZER);
            } else {
                self.parse_block_level_expression();
            }
        }
    }

    /*
     * blockLevelExpression
     *  : annotations + ("\n")+ expression
     *  ;
     */
    pub(crate) fn parse_block_level_expression(&mut self) {
        if self.at(AT) {
            let expression = self.mark();
            self.parse_annotations(AnnotationParsingMode::Default);

            if !self.my_builder.newline_before_current_token() {
                expression.rollback_to(self);
                self.parse_expression();
                return;
            }

            self.parse_block_level_expression();
            expression.done(self, ANNOTATED_EXPRESSION);
            return;
        }

        self.parse_expression();
    }

    /*
     * declaration
     *   : function
     *   : property
     *   : extension
     *   : class
     *   : typeAlias
     *   : object
     *   ;
     */
    pub(crate) fn parse_local_declaration_rest(
        &mut self,
        modifier_detector: &mut ModifierDetector,
        fail_if_definitely_not_expression: bool,
        is_script_top_level: bool,
    ) -> Option<SyntaxKind> {
        let keyword_token = self.tt();
        if fail_if_definitely_not_expression {
            if keyword_token != Some(FUN_KEYWORD) {
                return None;
            }

            return self.parse_function_1(/* failIfIdentifierExists = */ true);
        }

        if keyword_token == Some(OBJECT_KEYWORD) {
            // Object expression may appear at the statement position: should parse it
            // as expression instead of object declaration
            // sample:
            // {
            //   object : Thread() {
            //   }
            // }
            let lookahead = self.lookahead(1);
            if lookahead == Some(COLON) || lookahead == Some(LBRACE) {
                return None;
            }
        }

        self.parse_common_declaration(
            modifier_detector,
            NameParsingMode::Required,
            if is_script_top_level { DeclarationParsingMode::ScriptToplevel } else { DeclarationParsingMode::Local },
        )
    }
}
