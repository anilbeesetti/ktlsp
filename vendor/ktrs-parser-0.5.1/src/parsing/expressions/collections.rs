//! `KotlinExpressionParsing.java` lines 957-1043: array access, collection literals, contract effects.

use ktrs_syntax::SyntaxKind::{self, *};

use crate::parsing::Parser;

impl Parser {
    /*
     * arrayAccess
     *   : "[" element{","} "]"
     *   ;
     */
    pub(crate) fn parse_array_access(&mut self) {
        self.parse_as_collection_literal_expression(INDICES, false, "Expecting an index element");
    }

    /*
     * collectionLiteral
     *   : "[" element{","}? "]"
     *   ;
     */
    pub(crate) fn parse_collection_literal_expression(&mut self) {
        self.parse_as_collection_literal_expression(COLLECTION_LITERAL_EXPRESSION, true, "Expecting an element");
    }

    pub(crate) fn parse_as_collection_literal_expression(
        &mut self,
        node_type: SyntaxKind,
        can_be_empty: bool,
        missing_element_error_message: &str,
    ) {
        debug_assert!(self._at(LBRACKET));

        let inner_expressions = self.mark();

        self.my_builder.disable_newlines();
        self.advance(); // LBRACKET

        if !can_be_empty && self.at(RBRACKET) {
            self.error(missing_element_error_message);
        } else {
            self.parse_inner_expressions(missing_element_error_message);
        }

        self.expect_2(RBRACKET, "Expecting ']'");
        self.my_builder.restore_newlines_state();

        inner_expressions.done(self, node_type);
    }

    pub(crate) fn parse_inner_expressions(&mut self, missing_element_error_message: &str) {
        loop {
            if self.at(RBRACKET) {
                break;
            }
            self.parse_expression_1(missing_element_error_message);

            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
        }
    }

    pub(crate) fn parse_contract_description_block(&mut self) {
        debug_assert!(self._at(CONTRACT_KEYWORD));

        self.advance(); // CONTRACT_KEYWORD

        self.parse_contract_effect_list();
    }

    pub(crate) fn parse_contract_effect_list(&mut self) {
        let block = self.mark();

        self.expect_2(LBRACKET, "Expecting '['");
        self.my_builder.enable_newlines();

        self.parse_contract_effects();

        self.expect_2(RBRACKET, "Expecting ']'");
        self.my_builder.restore_newlines_state();

        block.done(self, CONTRACT_EFFECT_LIST);
    }

    pub(crate) fn parse_contract_effects(&mut self) {
        loop {
            if self.at(COMMA) {
                self.error_and_advance("Expecting a contract effect");
            }
            if self.at(RBRACKET) {
                break;
            }
            let effect = self.mark();
            self.parse_expression();
            effect.done(self, CONTRACT_EFFECT);

            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
        }
    }
}
