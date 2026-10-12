//! Port of `KotlinParsing.java` lines 2078-2207 (type parameters, constraints, contracts).

use ktrs_syntax::SyntaxKind::*;

use super::parameters::AnnotationParsingMode;
use super::{
    COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET, GT_COMMA_COLON_SET, LBRACE_RBRACE_TYPE_REF_FIRST_SET,
    TYPE_PARAMETER_GT_RECOVERY_SET,
};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * typeParameters
     *   : ("<" typeParameter{","} ">"
     *   ;
     */
    pub(super) fn parse_type_parameter_list(&mut self, recovery_set: TokenSet) -> bool {
        let mut result = false;
        if self.at(LT) {
            let list = self.mark();

            self.my_builder.disable_newlines();
            self.advance(); // LT

            loop {
                if self.at(COMMA) {
                    self.error_and_advance("Expecting type parameter declaration");
                }
                self.parse_type_parameter();

                if !self.at(COMMA) {
                    break;
                }
                self.advance(); // COMMA
                if self.at(GT) {
                    break;
                }
            }

            self.expect_3(GT, "Missing '>'", Some(recovery_set));
            self.my_builder.restore_newlines_state();
            result = true;

            list.done(self, TYPE_PARAMETER_LIST);
        }
        result
    }

    /*
     * typeConstraints
     *   : ("where" typeConstraint{","})?
     *   ;
     */
    pub(super) fn parse_type_constraints_guarded(&mut self, type_parameter_list_occurred: bool) {
        let error = self.mark();
        let constraints = self.parse_type_constraints();
        self.error_if(
            error,
            constraints && !type_parameter_list_occurred,
            "Type constraints are not allowed when no type parameters declared",
        );
    }

    pub(super) fn parse_type_constraints(&mut self) -> bool {
        if self.at(WHERE_KEYWORD) {
            self.parse_type_constraint_list();
            return true;
        }
        false
    }

    /*
     * typeConstraint{","}
     */
    fn parse_type_constraint_list(&mut self) {
        debug_assert!(self._at(WHERE_KEYWORD));

        self.advance(); // WHERE_KEYWORD

        let list = self.mark();

        loop {
            if self.at(COMMA) {
                self.error_and_advance("Type constraint expected");
            }
            self.parse_type_constraint();
            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
        }

        list.done(self, TYPE_CONSTRAINT_LIST);
    }

    /*
     * typeConstraint
     *   : annotations SimpleName ":" type
     *   ;
     */
    fn parse_type_constraint(&mut self) {
        let constraint = self.mark();

        self.parse_annotations(AnnotationParsingMode::Default);

        let reference = self.mark();
        if self.expect_3(IDENTIFIER, "Expecting type parameter name", Some(COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET)) {
            reference.done(self, REFERENCE_EXPRESSION);
        } else {
            reference.drop(self);
        }

        self.expect_3(COLON, "Expecting ':' before the upper bound", Some(LBRACE_RBRACE_TYPE_REF_FIRST_SET));

        self.parse_type_ref();

        constraint.done(self, TYPE_CONSTRAINT);
    }

    pub(super) fn parse_function_contract(&mut self) -> bool {
        if self.at(CONTRACT_KEYWORD) {
            self.parse_contract_description_block();
            return true;
        }
        false
    }

    /*
     * typeParameter
     *   : modifiers SimpleName (":" userType)?
     *   ;
     */
    fn parse_type_parameter(&mut self) {
        if self.at_set(TYPE_PARAMETER_GT_RECOVERY_SET) {
            self.error("Type parameter declaration expected");
            return;
        }

        let mark = self.mark();

        self.parse_modifier_list(GT_COMMA_COLON_SET);

        self.expect_3(IDENTIFIER, "Type parameter name expected", Some(TokenSet::EMPTY));

        if self.at(COLON) {
            self.advance(); // COLON
            self.parse_type_ref();
        }

        mark.done(self, TYPE_PARAMETER);
    }
}
