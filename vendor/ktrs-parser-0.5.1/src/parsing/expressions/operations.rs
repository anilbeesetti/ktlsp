//! `KotlinExpressionParsing.java` lines 155-333: binary, labeled and prefix expressions, `::` suffixes.

use ktrs_syntax::SyntaxKind::*;

use super::{
    BinaryOperationPrecedence, MIN_BINARY_OPERATION_PRECEDENCE, PREFIX_OPERATIONS, TYPE_ARGUMENT_LIST_STOPPERS,
};
use crate::builder::Marker;
use crate::kt_tokens::SOFT_KEYWORDS;
use crate::parsing::Parser;
use crate::parsing::declarations::AnnotationParsingMode;

impl Parser {
    /// `element (operation element)*`; returns the minPrecedence to consider further (see `is`).
    pub(crate) fn parse_binary_expression(
        &mut self,
        max_precedence: Option<BinaryOperationPrecedence>,
    ) -> BinaryOperationPrecedence {
        let Some(max_precedence) = max_precedence else {
            panic!("Shouldn't be here");
        };

        let mut expression = self.mark();

        self.parse_prefix_expression();

        let mut min_precedence = MIN_BINARY_OPERATION_PRECEDENCE;

        while !self.interrupted_with_new_line() {
            let Some(operation) = self.tt() else { break };

            let Some(next_precedence) =
                BinaryOperationPrecedence::token_to_binary_precedence_map_with_soft_identifiers(operation)
            else {
                break;
            };
            if next_precedence.ordinal() > max_precedence.ordinal() || next_precedence.ordinal() < min_precedence.ordinal() {
                break;
            }

            if next_precedence == BinaryOperationPrecedence::Infix && SOFT_KEYWORDS.contains(operation) {
                // Remap soft keywords and modifiers that are treated as infix functions since there is no mutating `atSetWithRemap` call
                self.my_builder.remap_current_token(IDENTIFIER);
            }

            self.parse_operation_reference();

            let result_type = match operation {
                AS_KEYWORD | AS_SAFE => {
                    self.parse_type_ref_without_intersections();
                    min_precedence = BinaryOperationPrecedence::As;
                    BINARY_WITH_TYPE
                }
                IS_KEYWORD | NOT_IS => {
                    self.parse_type_ref_without_intersections();
                    // `is` doesn't parse its RHS recursively: `min_precedence` keeps more prioritized
                    // operations (INFIX, RANGE, ...) from attaching to it.
                    min_precedence = BinaryOperationPrecedence::InOrIs;
                    IS_EXPRESSION
                }
                _ => {
                    min_precedence = self.parse_binary_expression(next_precedence.get_higher_priority());
                    BINARY_EXPRESSION
                }
            };

            expression.done(self, result_type);
            expression = expression.precede(self);
        }

        expression.drop(self);

        min_precedence
    }

    /*
     * label prefixExpression
     */
    pub(crate) fn parse_labeled_expression(&mut self) {
        let expression = self.mark();
        self.parse_label_definition();
        self.parse_prefix_expression();
        expression.done(self, LABELED_EXPRESSION);
    }

    /*
     * operation? prefixExpression
     */
    pub(crate) fn parse_prefix_expression(&mut self) {
        if self.at(AT) {
            if !self.parse_local_declaration(/* rollbackIfDefinitelyNotExpression = */ false, false) {
                let expression = self.mark();
                self.parse_annotations(AnnotationParsingMode::Default);
                self.parse_prefix_expression();
                expression.done(self, ANNOTATED_EXPRESSION);
            }
        } else {
            self.my_builder.disable_joining_complex_tokens();
            if self.is_at_label_definition_or_missing_identifier() {
                self.my_builder.restore_joining_complex_tokens_state();
                self.parse_labeled_expression();
            } else if self.at_set(PREFIX_OPERATIONS) {
                let expression = self.mark();

                self.parse_operation_reference();

                self.my_builder.restore_joining_complex_tokens_state();

                self.parse_prefix_expression();
                expression.done(self, PREFIX_EXPRESSION);
            } else {
                self.my_builder.restore_joining_complex_tokens_state();
                self.parse_postfix_expression();
            }
        }
    }

    /*
     * doubleColonSuffix
     *   : "::" SimpleName typeArguments?
     *   ;
     */
    pub(crate) fn parse_double_colon_suffix(&mut self, expression: Marker) -> bool {
        if !self.at(COLONCOLON) {
            return false;
        }

        self.advance(); // COLONCOLON

        if self.at(CLASS_KEYWORD) {
            self.advance(); // CLASS_KEYWORD

            expression.done(self, CLASS_LITERAL_EXPRESSION);
            return true;
        }

        self.parse_simple_name_expression();

        if self.at(LT) {
            let type_argument_list = self.mark();
            if self.try_parse_type_argument_list(TYPE_ARGUMENT_LIST_STOPPERS) {
                type_argument_list.error(self, "Type arguments are not allowed");
            } else {
                type_argument_list.rollback_to(self);
            }
        }

        if self.at(LPAR) && !self.my_builder.newline_before_current_token() {
            let lpar = self.mark();
            self.parse_call_suffix();
            lpar.error(
                self,
                "This syntax is reserved for future use; to call a reference, enclose it in parentheses: (foo::bar)(args)",
            );
        }

        expression.done(self, CALLABLE_REFERENCE_EXPRESSION);
        true
    }

    pub(crate) fn skip_question_marks_before_double_colon(&mut self) {
        if self.at(QUEST) {
            let mut k = 1;
            while self.lookahead(k) == Some(QUEST) {
                k += 1;
            }
            if self.lookahead(k) == Some(COLONCOLON) {
                while k > 0 {
                    self.advance(); // QUEST
                    k -= 1;
                }
            }
        }
    }
}
