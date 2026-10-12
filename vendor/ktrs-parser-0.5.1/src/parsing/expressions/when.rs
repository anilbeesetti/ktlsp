//! `KotlinExpressionParsing.java` lines 762-955: `when` expressions, entries, conditions and guards.

use ktrs_syntax::SyntaxKind::*;

use super::{EQ_RPAR_SET, WHEN_CONDITION_RECOVERY_SET, WHEN_CONDITION_RECOVERY_SET_WITH_ARROW};
use crate::builder::EdgeBinder;
use crate::parsing::Parser;
use crate::parsing::declarations::DeclarationParsingMode;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * when
     *   : "when" ("(" (modifiers "val" SimpleName "=")? element ")")? "{"
     *         whenEntry*
     *     "}"
     *   ;
     */
    pub(crate) fn parse_when(&mut self) {
        debug_assert!(self._at(WHEN_KEYWORD));

        let when = self.mark();

        self.advance(); // WHEN_KEYWORD

        // Parse condition
        self.my_builder.disable_newlines();
        if self.at(LPAR) {
            self.advance_at(LPAR);

            let at_when_start = self.mark();
            self.parse_annotations_list(EQ_RPAR_SET);
            if self.at(VAL_KEYWORD) || self.at(VAR_KEYWORD) {
                let decl_type = self.parse_property(DeclarationParsingMode::Local);

                at_when_start.done(self, decl_type);
                at_when_start.set_custom_edge_token_binders(
                    self,
                    Some(EdgeBinder::PrecedingDocComments),
                    Some(EdgeBinder::TrailingComments),
                );
            } else {
                at_when_start.rollback_to(self);
                self.parse_expression();
            }

            self.expect_2(RPAR, "Expecting ')'");
        }
        self.my_builder.restore_newlines_state();

        // Parse when block
        self.my_builder.enable_newlines();
        if self.expect_2(LBRACE, "Expecting '{'") {
            while !self.eof() && !self.at(RBRACE) {
                self.parse_when_entry();
            }

            self.expect_2(RBRACE, "Expecting '}'");
        }
        self.my_builder.restore_newlines_state();

        when.done(self, WHEN);
    }

    /*
     * whenEntry
     *   // TODO : consider empty after ->
     *   : whenCondition{","} whenEntryGuard? "->" element SEMI
     *   : "else" whenEntryGuard? "->" element SEMI
     *   ;
     */
    pub(crate) fn parse_when_entry(&mut self) {
        let entry = self.mark();

        if self.at(ELSE_KEYWORD) {
            self.advance(); // ELSE_KEYWORD

            self.parse_when_entry_guard_or_suggest();

            if !self.at(ARROW) {
                self.error_until("Expecting '->'", TokenSet::create(&[ARROW, LBRACE, RBRACE, EOL_OR_SEMICOLON]));
            }

            if self.at(ARROW) {
                self.advance(); // ARROW

                if self.at_set(WHEN_CONDITION_RECOVERY_SET) {
                    self.error("Expecting an element");
                } else {
                    self.parse_control_structure_body();
                }
            } else if self.at(LBRACE) {
                // no arrow, probably it's simply missing
                self.parse_control_structure_body();
            } else if !self.at_set(WHEN_CONDITION_RECOVERY_SET) {
                self.error_and_advance("Expecting '->'");
            }
        } else {
            self.parse_when_entry_not_else();
        }

        entry.done(self, WHEN_ENTRY);
        self.consume_if(SEMICOLON);
    }

    /*
     * : whenCondition{","} whenEntryGuard? "->" element SEMI
     */
    pub(crate) fn parse_when_entry_not_else(&mut self) {
        loop {
            while self.at(COMMA) {
                self.error_and_advance("Expecting a when-condition");
            }
            self.parse_when_condition();
            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
            if self.at(ARROW) {
                break;
            }
        }

        self.parse_when_entry_guard_or_suggest();

        self.expect_3(ARROW, "Expecting '->'", Some(WHEN_CONDITION_RECOVERY_SET));
        if self.at_set(WHEN_CONDITION_RECOVERY_SET) {
            self.error("Expecting an element");
        } else {
            self.parse_control_structure_body();
        }
        // SEMI is consumed in parseWhenEntry
    }

    /*
     * whenCondition
     *   : expression
     *   : ("in" | "!in") expression
     *   : ("is" | "!is") isRHS
     *   ;
     */
    pub(crate) fn parse_when_condition(&mut self) {
        let condition = self.mark();
        self.my_builder.disable_newlines();
        match self.get_token_id() {
            Some(IN_KEYWORD | NOT_IN) => {
                let mark = self.mark();
                self.advance(); // IN_KEYWORD or NOT_IN
                mark.done(self, OPERATION_REFERENCE);

                if self.at_set(WHEN_CONDITION_RECOVERY_SET_WITH_ARROW) {
                    self.error("Expecting an element");
                } else {
                    self.parse_expression();
                }
                condition.done(self, WHEN_CONDITION_IN_RANGE);
            }
            Some(IS_KEYWORD | NOT_IS) => {
                self.advance(); // IS_KEYWORD or NOT_IS

                if self.at_set(WHEN_CONDITION_RECOVERY_SET_WITH_ARROW) {
                    self.error("Expecting a type");
                } else {
                    self.parse_type_ref();
                }
                condition.done(self, WHEN_CONDITION_IS_PATTERN);
            }
            Some(RBRACE | ELSE_KEYWORD | ARROW | DOT) => {
                self.error("Expecting an expression, is-condition or in-condition");
                condition.done(self, WHEN_CONDITION_EXPRESSION);
            }
            _ => {
                self.parse_expression();
                condition.done(self, WHEN_CONDITION_EXPRESSION);
            }
        }
        self.my_builder.restore_newlines_state();
    }

    pub(crate) fn parse_when_entry_guard_or_suggest(&mut self) {
        if self.at(ANDAND) {
            self.error_until(
                "Unexpected '&&', use 'if' to introduce additional conditions; see https://kotl.in/guards-in-when",
                TokenSet::create(&[LBRACE, RBRACE, ARROW]),
            );
        } else if self.at(IF_KEYWORD) {
            self.parse_when_entry_guard();
        }
    }

    /*
     * whenEntryGuard
     *   : "if" expression
     *   ;
     */
    pub(crate) fn parse_when_entry_guard(&mut self) {
        debug_assert!(self._at(IF_KEYWORD));

        let guard = self.mark();
        self.advance(); // IF_KEYWORD
        self.parse_expression();
        guard.done(self, WHEN_ENTRY_GUARD);
    }
}
