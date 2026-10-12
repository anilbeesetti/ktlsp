//! Port of `KotlinParsing.java` lines 2209-2371 (type references, nullable and intersection types).

use ktrs_syntax::SyntaxKind::*;

use super::TOP_LEVEL_DECLARATION_FIRST;
use crate::builder::Marker;
use crate::parsing::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * type
     *   : typeModifiers typeReference
     *   ;
     *
     * typeReference
     *   : functionType
     *   : userType
     *   : nullableType
     *   : "dynamic"
     *   ;
     *
     * nullableType
     *   : typeReference "?"
     *   ;
     */
    pub(crate) fn parse_type_ref(&mut self) {
        self.parse_type_ref_1(TokenSet::EMPTY);
    }

    pub(crate) fn parse_type_ref_without_intersections(&mut self) {
        self.parse_type_ref_2(TokenSet::EMPTY, /* allowSimpleIntersectionTypes */ false);
    }

    pub(crate) fn parse_type_ref_1(&mut self, extra_recovery_set: TokenSet) {
        self.parse_type_ref_2(extra_recovery_set, /* allowSimpleIntersectionTypes */ true);
    }

    fn parse_type_ref_2(&mut self, extra_recovery_set: TokenSet, allow_simple_intersection_types: bool) {
        let type_ref_marker = self.parse_type_ref_contents(extra_recovery_set, allow_simple_intersection_types);
        type_ref_marker.done(self, TYPE_REFERENCE);
    }

    // The extraRecoverySet is needed for the foo(bar<x, 1, y>(z)) case, to tell whether we should stop
    // on expression-indicating symbols or not
    fn parse_type_ref_contents(&mut self, extra_recovery_set: TokenSet, allow_simple_intersection_types: bool) -> Marker {
        let type_ref_marker = self.mark();

        self.parse_type_modifier_list();

        let lookahead = self.lookahead(1);
        let lookahead2 = self.lookahead(2);
        let mut type_before_dot = true;
        let with_context_receiver = self.at(CONTEXT_KEYWORD) && lookahead == Some(LPAR);
        let mut was_function_type_parsed = false;

        let context_receivers_start = self.mark();

        if with_context_receiver {
            self.parse_context_parameter_or_receiver_list(true);
        }

        let mut type_element_marker = self.mark();

        if self.at(IDENTIFIER)
            && !(lookahead == Some(DOT) && lookahead2 == Some(IDENTIFIER))
            && lookahead != Some(LT)
            && self.at(DYNAMIC_KEYWORD)
        {
            let dynamic_type = self.mark();
            self.advance(); // DYNAMIC_KEYWORD
            dynamic_type.done(self, DYNAMIC_TYPE);
        } else if self.at(IDENTIFIER) || self.at(PACKAGE_KEYWORD) || self.at_parenthesized_mutable_for_platform_types(0) {
            self.parse_user_type();
        } else if self.at(LPAR) {
            let function_or_parenthesized_type = self.mark();

            // This may be a function parameter list or just a parenthesized type
            self.advance(); // LPAR
            // parenthesized types, no reference element around it is needed
            let contents = self.parse_type_ref_contents(TokenSet::EMPTY, /* allowSimpleIntersectionTypes */ true);
            contents.drop(self);

            if self.at(RPAR) && self.lookahead(1) != Some(ARROW) {
                // It's a parenthesized type
                //    (A)
                self.advance();
                function_or_parenthesized_type.drop(self);
            } else {
                // This must be a function type
                //   (A, B) -> C
                // or
                //   (a : A) -> C
                function_or_parenthesized_type.rollback_to(self);
                let function_type = context_receivers_start.precede(self);
                self.parse_function_type(function_type);
                was_function_type_parsed = true;
            }
        } else {
            self.error_with_recovery(
                "Type expected",
                Some(TokenSet::or_set(&[
                    TOP_LEVEL_DECLARATION_FIRST,
                    TokenSet::create(&[EQ, COMMA, GT, RBRACKET, DOT, RPAR, RBRACE, LBRACE, SEMICOLON]),
                    extra_recovery_set,
                ])),
            );
            type_before_dot = false;
        }

        // Disabling token merge is required for cases like
        //    Int?.(Foo) -> Bar
        self.my_builder.disable_joining_complex_tokens();
        type_element_marker = self.parse_nullable_type_suffix(type_element_marker);
        self.my_builder.restore_joining_complex_tokens_state();

        let mut was_intersection = false;
        if allow_simple_intersection_types && self.at(AND) {
            let left_type_ref = type_element_marker;

            type_element_marker = type_element_marker.precede(self);
            let intersection_type = left_type_ref.precede(self);

            left_type_ref.done(self, TYPE_REFERENCE);

            self.advance(); // &
            self.parse_type_ref_2(extra_recovery_set, /* allowSimpleIntersectionTypes */ true);

            intersection_type.done(self, INTERSECTION_TYPE);
            was_intersection = true;
        }

        if type_before_dot && self.at(DOT) && !was_intersection && !was_function_type_parsed {
            // This is a receiver for a function type
            //  A.(B) -> C
            //   ^

            let function_type = context_receivers_start.precede(self);

            let receiver_type_ref = type_element_marker.precede(self);
            let receiver_type = receiver_type_ref.precede(self);
            receiver_type_ref.done(self, TYPE_REFERENCE);
            receiver_type.done(self, FUNCTION_TYPE_RECEIVER);

            self.advance(); // DOT

            if self.at(LPAR) {
                self.parse_function_type(function_type);
            } else {
                function_type.drop(self);
                self.error("Expecting function type");
            }

            was_function_type_parsed = true;
        }

        if with_context_receiver && !was_function_type_parsed {
            self.error_with_recovery(
                "Function type expected expected",
                Some(TokenSet::or_set(&[
                    TOP_LEVEL_DECLARATION_FIRST,
                    TokenSet::create(&[EQ, COMMA, GT, RBRACKET, DOT, RPAR, RBRACE, LBRACE, SEMICOLON]),
                    extra_recovery_set,
                ])),
            );
        }

        type_element_marker.drop(self);
        context_receivers_start.drop(self);
        type_ref_marker
    }

    fn parse_nullable_type_suffix(&mut self, type_element_marker: Marker) -> Marker {
        let mut type_element_marker = type_element_marker;
        // ?: is joined regardless of joining state
        while self.at(QUEST) && self.my_builder.raw_lookup(1) != Some(COLON) {
            let precede = type_element_marker.precede(self);
            self.advance(); // QUEST
            type_element_marker.done(self, NULLABLE_TYPE);
            type_element_marker = precede;
        }
        type_element_marker
    }
}
