//! Port of `KotlinParsing.java` lines 2373-2541 (user types, platform-type recovery, type
//! arguments, function types).

use ktrs_syntax::SyntaxKind::*;

use super::TYPE_REF_FIRST;
use super::parameters::USER_TYPE_NAME_RECOVERY_SET;
use crate::builder::Marker;
use crate::parsing::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * userType
     *   : simpleUserType{"."}
     *   ;
     *
     *   recovers on platform types:
     *    - Foo!
     *    - (Mutable)List<Foo>!
     *    - Array<(out) Foo>!
     */
    pub(super) fn parse_user_type(&mut self) {
        let mut user_type = self.mark();

        if self.at(PACKAGE_KEYWORD) {
            let keyword = self.mark();
            self.advance(); // PACKAGE_KEYWORD
            keyword.error(self, "Expecting an element");
            self.expect_3(DOT, "Expecting '.'", Some(TokenSet::create(&[IDENTIFIER, LBRACE, RBRACE])));
        }

        let mut reference = self.mark();
        loop {
            self.recover_on_parenthesized_word_for_platform_types(0, "Mutable", true);

            if self.expect_3(IDENTIFIER, "Expecting type name", Some(USER_TYPE_NAME_RECOVERY_SET)) {
                reference.done(self, REFERENCE_EXPRESSION);
            } else {
                reference.drop(self);
                break;
            }

            self.parse_type_argument_list();

            self.recover_on_platform_type_suffix();

            if !self.at(DOT) {
                break;
            }
            if self.lookahead(1) == Some(LPAR) && !self.at_parenthesized_mutable_for_platform_types(1) {
                // This may be a receiver for a function type
                //   Int.(Int) -> Int
                break;
            }

            let precede = user_type.precede(self);
            user_type.done(self, USER_TYPE);
            user_type = precede;

            self.advance(); // DOT
            reference = self.mark();
        }

        user_type.done(self, USER_TYPE);
    }

    pub(super) fn at_parenthesized_mutable_for_platform_types(&mut self, offset: i32) -> bool {
        self.recover_on_parenthesized_word_for_platform_types(offset, "Mutable", false)
    }

    fn recover_on_parenthesized_word_for_platform_types(&mut self, offset: i32, word: &str, consume: bool) -> bool {
        // Array<(out) Foo>! or (Mutable)List<Bar>!
        if self.lookahead(offset) == Some(LPAR)
            && self.lookahead(offset + 1) == Some(IDENTIFIER)
            && self.lookahead(offset + 2) == Some(RPAR)
            && self.lookahead(offset + 3) == Some(IDENTIFIER)
        {
            let error = self.mark();

            self.advance_1(offset);

            self.advance(); // LPAR
            if self.my_builder.get_token_text() != Some(word) {
                // something other than "out" / "Mutable"
                error.rollback_to(self);
                return false;
            } else {
                self.advance(); // IDENTIFIER('out')
                self.advance(); // RPAR

                if consume {
                    error.error(self, "Unexpected tokens");
                } else {
                    error.rollback_to(self);
                }

                return true;
            }
        }
        false
    }

    fn recover_on_platform_type_suffix(&mut self) {
        // Recovery for platform types
        if self.at(EXCL) {
            let error = self.mark();
            self.advance(); // EXCL
            error.error(self, "Unexpected token");
        }
    }

    /*
     *  (optionalProjection type){","}
     */
    pub(super) fn parse_type_argument_list(&mut self) {
        if !self.at(LT) {
            return;
        }

        let list = self.mark();

        self.try_parse_type_argument_list(TokenSet::EMPTY);

        list.done(self, TYPE_ARGUMENT_LIST);
    }

    pub(crate) fn try_parse_type_argument_list(&mut self, extra_recovery_set: TokenSet) -> bool {
        self.my_builder.disable_newlines();
        self.advance(); // LT

        loop {
            let projection = self.mark();

            self.recover_on_parenthesized_word_for_platform_types(0, "out", true);

            // Currently we do not allow annotations on star projections and probably we should not
            // Annotations on other kinds of type arguments should be parsed as common type annotations (within parseTypeRef call)
            self.parse_type_argument_modifier_list();

            if self.at(MUL) {
                self.advance(); // MUL
            } else {
                self.parse_type_ref_1(extra_recovery_set);
            }
            projection.done(self, TYPE_PROJECTION);
            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
            if self.at(GT) {
                break;
            }
        }

        let at_gt = self.at(GT);
        if !at_gt {
            self.error("Expecting a '>'");
        } else {
            self.advance(); // GT
        }
        self.my_builder.restore_newlines_state();
        at_gt
    }

    /*
     * functionType
     *   : (type ".")? "(" parameter{","}? ")" "->" type?
     *   ;
     */
    pub(super) fn parse_function_type(&mut self, function_type: Marker) {
        let contents = self.parse_function_type_contents(function_type);
        contents.done(self, FUNCTION_TYPE);
    }

    fn parse_function_type_contents(&mut self, function_type: Marker) -> Marker {
        debug_assert!(self._at(LPAR), "{:?}", self.tt());

        self.parse_value_parameter_list(true, /* typeRequired  = */ true, TokenSet::EMPTY, false);

        self.expect_3(ARROW, "Expecting '->' to specify return type of a function type", Some(TYPE_REF_FIRST));
        self.parse_type_ref();

        function_type
    }
}
