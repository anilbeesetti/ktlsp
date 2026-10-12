//! `KotlinExpressionParsing.java` lines 335-511: postfix expressions, call suffixes, annotated lambdas.

use ktrs_syntax::SyntaxKind::{self, *};

use super::{POSTFIX_OPERATIONS, TYPE_ARGUMENT_LIST_STOPPERS};
use crate::builder::Marker;
use crate::parsing::Parser;
use crate::parsing::declarations::AnnotationParsingMode;

impl Parser {
    /*
     * postfixUnaryExpression
     *   : atomicExpression postfixUnaryOperation*
     *   ;
     *
     * postfixUnaryOperation
     *   : "++" : "--" : "!!"
     *   : typeArguments? valueArguments (getEntryPoint? functionLiteral)
     *   : typeArguments (getEntryPoint? functionLiteral)
     *   : arrayAccess
     *   : memberAccessOperation postfixUnaryExpression // TODO: Review
     *   ;
     */
    pub(crate) fn parse_postfix_expression(&mut self) {
        let mut expression = self.mark();

        let mut first_expression_parsed = if self.at(COLONCOLON) {
            let m = self.mark();
            self.parse_double_colon_suffix(m)
        } else {
            self.parse_atomic_expression()
        };

        loop {
            if self.interrupted_with_new_line() {
                break;
            } else if self.at(LBRACKET) {
                self.parse_array_access();
                expression.done(self, ARRAY_ACCESS_EXPRESSION);
            } else if self.parse_call_suffix() {
                expression.done(self, CALL_EXPRESSION);
            } else if self.at(DOT) || self.at(SAFE_ACCESS) {
                let expression_type = if self.at(DOT) { DOT_QUALIFIED_EXPRESSION } else { SAFE_ACCESS_EXPRESSION };
                self.advance(); // DOT or SAFE_ACCESS

                if !first_expression_parsed {
                    expression.drop(self);
                    expression = self.mark();
                    first_expression_parsed = self.parse_atomic_expression();
                    continue;
                }

                self.parse_selector_call_expression();

                expression.done(self, expression_type);
            } else if self.at_set(POSTFIX_OPERATIONS) {
                self.parse_operation_reference();
                expression.done(self, POSTFIX_EXPRESSION);
            } else {
                self.skip_question_marks_before_double_colon();
                if !self.parse_double_colon_suffix(expression) {
                    break;
                }
            }
            expression = expression.precede(self);
        }
        expression.drop(self);
    }

    /*
     * callSuffix
     *   : typeArguments? valueArguments annotatedLambda
     *   : typeArguments annotatedLambda
     *   ;
     */
    pub(crate) fn parse_call_suffix(&mut self) -> bool {
        if self.parse_call_with_closure() {
            // do nothing
        } else if self.at(LPAR) {
            self.parse_value_argument_list();
            self.parse_call_with_closure();
        } else if self.at(LT) {
            let type_argument_list = self.mark();
            if self.try_parse_type_argument_list(TYPE_ARGUMENT_LIST_STOPPERS) {
                type_argument_list.done(self, TYPE_ARGUMENT_LIST);
                if !self.my_builder.newline_before_current_token() && self.at(LPAR) {
                    self.parse_value_argument_list();
                }
                self.parse_call_with_closure();
            } else {
                type_argument_list.rollback_to(self);
                return false;
            }
        } else {
            return false;
        }

        true
    }

    /*
     * atomicExpression typeParameters? valueParameters? functionLiteral*
     */
    pub(crate) fn parse_selector_call_expression(&mut self) {
        let mark = self.mark();
        self.parse_atomic_expression();
        if !self.my_builder.newline_before_current_token() && self.parse_call_suffix() {
            mark.done(self, CALL_EXPRESSION);
        } else {
            mark.drop(self);
        }
    }

    pub(crate) fn parse_operation_reference(&mut self) {
        let operation_reference = self.mark();
        self.advance(); // operation
        operation_reference.done(self, OPERATION_REFERENCE);
    }

    /*
     * annotatedLambda*
     */
    pub(crate) fn parse_call_with_closure(&mut self) -> bool {
        // The by-clause parser's override (see parsing/mod.rs).
        if let Some(n) = self.by_clause_stack_size() {
            if n <= 0 {
                return false;
            }
        }

        let mut success = false;

        loop {
            let argument = self.mark();

            if !self.parse_annotated_lambda(/* preferBlock = */ false) {
                argument.drop(self);
                break;
            }

            argument.done(self, LAMBDA_ARGUMENT);
            success = true;
        }

        success
    }

    /*
     * annotatedLambda
     *  : ("@" annotationEntry)* labelDefinition? functionLiteral
     */
    pub(crate) fn parse_annotated_lambda(&mut self, prefer_block: bool) -> bool {
        let annotated = self.mark();

        let were_annotations = self.parse_annotations(AnnotationParsingMode::Default);
        let labeled = self.mark();

        let was_label = self.is_at_label_definition_or_missing_identifier();
        if was_label {
            self.parse_label_definition();
        }

        if !self.at(LBRACE) {
            annotated.rollback_to(self);
            return false;
        }

        self.parse_function_literal_2(prefer_block, /* collapse = */ true);

        self.done_or_drop(labeled, LABELED_EXPRESSION, was_label);
        self.done_or_drop(annotated, ANNOTATED_EXPRESSION, were_annotations);

        true
    }

    /// Static upstream; takes `self` as the marker host.
    pub(crate) fn done_or_drop(&mut self, marker: Marker, r#type: SyntaxKind, condition: bool) {
        if condition {
            marker.done(self, r#type);
        } else {
            marker.drop(self);
        }
    }

    pub(crate) fn is_at_label_definition_or_missing_identifier(&mut self) -> bool {
        (self.at(IDENTIFIER) && self.my_builder.raw_lookup(1) == Some(AT)) || self.at(AT)
    }
}
