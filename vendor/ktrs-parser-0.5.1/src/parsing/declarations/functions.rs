//! Port of `KotlinParsing.java` lines 1803-1970 (functions, receiver types, function bodies).

use ktrs_syntax::SyntaxKind::{self, *};

use super::parameters::AnnotationParsingMode;
use super::{
    FUNCTION_NAME_FOLLOW_SET, FUNCTION_NAME_RECOVERY_SET, LBRACKET_LBRACE_RBRACE_LPAR_SET,
    LPAR_VALUE_PARAMETERS_FOLLOW_SET, RECEIVER_TYPE_TERMINATORS, VALUE_PARAMETERS_FOLLOW_SET,
    definitelyOutOfReceiverSet,
};
use crate::parsing::{AtSet, FirstBefore, LastBefore, Parser};
use crate::token_set::TokenSet;

impl Parser {
    pub(super) fn parse_function(&mut self) -> SyntaxKind {
        self.parse_function_1(false).expect("parseFunction(false) is never null")
    }

    /*
     * function
     *   : modifiers "fun" typeParameters?
     *       (type ".")?
     *       SimpleName
     *       typeParameters? functionParameters (":" type)?
     *       typeConstraints
     *       functionBody?
     *   ;
     */
    pub(crate) fn parse_function_1(&mut self, fail_if_identifier_exists: bool) -> Option<SyntaxKind> {
        debug_assert!(self._at(FUN_KEYWORD));

        self.advance(); // FUN_KEYWORD

        // Recovery for the case of class A { fun| }
        if self.at(RBRACE) {
            self.error("Function body expected");
            return Some(FUN);
        }

        let mut type_parameter_list_occurred = false;
        if self.at(LT) {
            self.parse_type_parameter_list(LBRACKET_LBRACE_RBRACE_LPAR_SET);
            type_parameter_list_occurred = true;
        }

        self.my_builder.disable_joining_complex_tokens();

        let receiver_found = self.parse_receiver_type("function", FUNCTION_NAME_FOLLOW_SET);

        if self.at(IDENTIFIER) && fail_if_identifier_exists {
            self.my_builder.restore_joining_complex_tokens_state();
            return None;
        }

        // function as expression has no name
        self.parse_function_or_property_name(
            receiver_found,
            "function",
            FUNCTION_NAME_FOLLOW_SET,
            FUNCTION_NAME_RECOVERY_SET,
            /*nameRequired = */ false,
        );

        self.my_builder.restore_joining_complex_tokens_state();

        if self.at(LT) {
            let mut error = self.mark();
            self.parse_type_parameter_list(LPAR_VALUE_PARAMETERS_FOLLOW_SET);
            if type_parameter_list_occurred {
                let finish_index = self.my_builder.raw_token_index();
                error.rollback_to(self);
                error = self.mark();
                let count = finish_index - self.my_builder.raw_token_index();
                self.advance_1(count);
                error.error(self, "Only one type parameter list is allowed for a function");
            } else {
                error.drop(self);
            }
            type_parameter_list_occurred = true;
        }

        if self.at(LPAR) {
            self.parse_value_parameter_list(false, /* typeRequired  = */ false, VALUE_PARAMETERS_FOLLOW_SET, true);
        } else {
            self.error("Expecting '('");
        }

        if self.at(COLON) {
            self.advance(); // COLON

            self.parse_type_ref();
        }

        let function_contract_occurred = self.parse_function_contract();

        self.parse_type_constraints_guarded(type_parameter_list_occurred);

        if !function_contract_occurred {
            self.parse_function_contract();
        }

        if self.at(SEMICOLON) {
            self.advance(); // SEMICOLON
        } else if self.at(EQ) || self.at(LBRACE) {
            self.parse_function_body();
        }

        Some(FUN)
    }

    /*
     *   (type "." | annotations)?
     */
    pub(super) fn parse_receiver_type(&mut self, title: &str, name_follow: TokenSet) -> bool {
        let annotations = self.mark();
        let annotations_present = self.parse_annotations(AnnotationParsingMode::Default);
        let last_dot = self.last_dot_after_receiver();
        let receiver_present = last_dot != -1;
        if annotations_present {
            if receiver_present {
                annotations.rollback_to(self);
            } else {
                annotations.error(self, "Annotations are not allowed in this position");
            }
        } else {
            annotations.drop(self);
        }

        if !receiver_present {
            return false;
        }

        self.create_truncated_builder(last_dot, |p| p.parse_type_ref_without_intersections());

        if self.at_set(RECEIVER_TYPE_TERMINATORS) {
            self.advance(); // expectation
        } else {
            self.error_with_recovery(&format!("Expecting '.' before a {title} name"), Some(name_follow));
        }
        true
    }

    fn last_dot_after_receiver(&mut self) -> i32 {
        if self.at(LPAR) {
            // lastDotAfterReceiverLParPattern
            let mut pattern = FirstBefore::new(
                AtSet::new(RECEIVER_TYPE_TERMINATORS),
                |p: &mut Parser, top_level: bool| -> bool {
                    if top_level && p.at_set(definitelyOutOfReceiverSet) {
                        return true;
                    }
                    top_level && !p.at(QUEST) && !p.at(LPAR) && !p.at(RPAR)
                },
            );
            pattern.reset();
            self.match_token_stream_predicate(&mut pattern)
        } else {
            // lastDotAfterReceiverNotLParPattern
            let mut pattern = LastBefore::new(
                AtSet::new(RECEIVER_TYPE_TERMINATORS),
                |p: &mut Parser, top_level: bool| -> bool {
                    if top_level && (p.at_set(definitelyOutOfReceiverSet) || p.at(LPAR)) {
                        return true;
                    }
                    if top_level && p.at(IDENTIFIER) {
                        let lookahead = p.lookahead(1);
                        return lookahead != Some(LT)
                            && lookahead != Some(DOT)
                            && lookahead != Some(SAFE_ACCESS)
                            && lookahead != Some(QUEST);
                    }
                    false
                },
            );
            pattern.reset();
            self.match_token_stream_predicate(&mut pattern)
        }
    }

    /*
     * IDENTIFIER
     */
    pub(super) fn parse_function_or_property_name(
        &mut self,
        receiver_found: bool,
        title: &str,
        name_follow: TokenSet,
        recovery_set: TokenSet,
        name_required: bool,
    ) {
        if !name_required && self.at_set(name_follow) {
            return; // no name
        }

        if self.expect(IDENTIFIER) {
            return;
        }

        self.error_with_recovery(
            &format!("Expecting {title} name{}", if !receiver_found { " or receiver type" } else { "" }),
            Some(recovery_set),
        );
    }

    /*
     * functionBody
     *   : block
     *   : "=" element
     *   ;
     */
    pub(super) fn parse_function_body(&mut self) {
        if self.at(LBRACE) {
            self.parse_block();
        } else if self.at(EQ) {
            self.advance(); // EQ
            self.parse_expression();
            self.consume_if(SEMICOLON);
        } else {
            self.error("Expecting function body");
        }
    }
}
