//! Port of `KotlinParsing.java` lines 1250-1454 (class bodies, members, secondary constructors,
//! type aliases).

use ktrs_syntax::SyntaxKind::{self, *};

use super::parameters::ModifierDetector;
use super::properties::DeclarationParsingMode;
use super::{
    LT_EQ_SEMICOLON_TOP_LEVEL_DECLARATION_FIRST_SET, NameParsingMode, TOP_LEVEL_DECLARATION_FIRST_SEMICOLON_SET,
    TYPE_PARAMETER_GT_RECOVERY_SET, VALUE_ARGS_RECOVERY_SET,
};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * classBody
     *   : ("{" members "}")?
     *   ;
     */
    pub(super) fn parse_class_body(&mut self) {
        let body = self.mark();

        self.my_builder.enable_newlines();

        if self.expect_2(LBRACE, "Expecting a class body") {
            self.parse_members();
            self.expect_2(RBRACE, "Missing '}");
        }

        self.my_builder.restore_newlines_state();

        body.done(self, CLASS_BODY);
    }

    /// members : memberDeclaration* ;
    pub(super) fn parse_members(&mut self) {
        while !self.eof() && !self.at(RBRACE) {
            self.parse_member_declaration();
        }
    }

    /*
     * memberDeclaration
     *   : modifiers memberDeclaration'
     *   ;
     *
     * memberDeclaration'
     *   : companionObject
     *   : secondaryConstructor
     *   : function
     *   : property
     *   : class
     *   : extension
     *   : typeAlias
     *   : anonymousInitializer
     *   : object
     *   ;
     */
    fn parse_member_declaration(&mut self) {
        if self.at(SEMICOLON) {
            self.advance(); // SEMICOLON
            return;
        }
        let decl = self.mark();

        let is_companion_block = self.at(COMPANION_KEYWORD) && self.lookahead(1) == Some(LBRACE);

        let mut detector = ModifierDetector::default();
        self.parse_modifier_list_3(Some(&mut detector), TokenSet::EMPTY, /* localDeclaration = */ false);

        let decl_type =
            if is_companion_block { Some(self.parse_companion_block()) } else { self.parse_member_declaration_rest(&detector) };

        match decl_type {
            None => {
                self.error_with_recovery("Expecting member declaration", Some(TokenSet::EMPTY));
                decl.drop(self);
            }
            Some(decl_type) => self.close_declaration_with_comment_binders(decl, decl_type, true),
        }
    }

    fn parse_member_declaration_rest(&mut self, modifier_detector: &ModifierDetector) -> Option<SyntaxKind> {
        let mut decl_type = self.parse_common_declaration(
            modifier_detector,
            if modifier_detector.is_companion_detected() { NameParsingMode::Allowed } else { NameParsingMode::Required },
            DeclarationParsingMode::MemberOrToplevel,
        );

        if decl_type.is_some() {
            return decl_type;
        }

        if self.at(INIT_KEYWORD) {
            self.advance(); // init
            if self.at(LBRACE) {
                self.parse_block();
            } else {
                let error = self.mark();
                error.error(self, "Expecting '{' after 'init'");
            }
            decl_type = Some(CLASS_INITIALIZER);
        } else if self.at(CONSTRUCTOR_KEYWORD) {
            self.parse_secondary_constructor();
            decl_type = Some(SECONDARY_CONSTRUCTOR);
        } else if self.at(LBRACE) {
            self.error("Expecting member declaration");
            self.parse_block();
            decl_type = Some(FUN);
        }
        decl_type
    }

    fn parse_companion_block(&mut self) -> SyntaxKind {
        self.parse_class_body();
        COMPANION_BLOCK
    }

    /*
     * secondaryConstructor
     *   : modifiers "constructor" valueParameters (":" constructorDelegationCall)? block
     * constructorDelegationCall
     *   : "this" valueArguments
     *   : "super" valueArguments
     */
    fn parse_secondary_constructor(&mut self) {
        debug_assert!(self._at(CONSTRUCTOR_KEYWORD));

        self.advance(); // CONSTRUCTOR_KEYWORD

        if self.at(LPAR) {
            self.parse_value_parameter_list(false, /*typeRequired = */ true, VALUE_ARGS_RECOVERY_SET, false);
        } else {
            self.error_with_recovery(
                "Expecting '('",
                Some(TokenSet::or_set(&[VALUE_ARGS_RECOVERY_SET, TokenSet::create(&[COLON])])),
            );
        }

        if self.at(COLON) {
            self.advance(); // COLON

            let delegation_call = self.mark();

            if self.at(THIS_KEYWORD) || self.at(SUPER_KEYWORD) {
                self.parse_this_or_super();
                self.parse_value_argument_list();
            } else {
                self.error("Expecting a 'this' or 'super' constructor call");
                let mut before_wrong_delegation_callee = None;
                if !self.at(LPAR) {
                    before_wrong_delegation_callee = Some(self.mark());
                    self.advance(); // wrong delegation callee
                }
                self.parse_value_argument_list();

                if let Some(before_wrong_delegation_callee) = before_wrong_delegation_callee {
                    if self.at(LBRACE) {
                        before_wrong_delegation_callee.drop(self);
                    } else {
                        before_wrong_delegation_callee.rollback_to(self);
                    }
                }
            }

            delegation_call.done(self, CONSTRUCTOR_DELEGATION_CALL);
        } else {
            // empty constructor delegation call
            let empty_delegation_call = self.mark();
            let reference = self.mark();
            reference.done(self, CONSTRUCTOR_DELEGATION_REFERENCE);
            empty_delegation_call.done(self, CONSTRUCTOR_DELEGATION_CALL);
        }

        if self.at(LBRACE) {
            self.parse_block();
        }
    }

    fn parse_this_or_super(&mut self) {
        debug_assert!(self._at(THIS_KEYWORD) || self._at(SUPER_KEYWORD));
        let mark = self.mark();

        self.advance(); // THIS_KEYWORD | SUPER_KEYWORD

        mark.done(self, CONSTRUCTOR_DELEGATION_REFERENCE);
    }

    /*
     * typeAlias
     *   : modifiers "typealias" SimpleName typeParameters? "=" type
     *   ;
     */
    pub(super) fn parse_type_alias(&mut self) -> SyntaxKind {
        debug_assert!(self._at(TYPE_ALIAS_KEYWORD));

        self.advance(); // TYPE_ALIAS_KEYWORD

        self.expect_3(IDENTIFIER, "Type name expected", Some(LT_EQ_SEMICOLON_TOP_LEVEL_DECLARATION_FIRST_SET));

        self.parse_type_parameter_list(TYPE_PARAMETER_GT_RECOVERY_SET);

        if self.at(WHERE_KEYWORD) {
            let error = self.mark();
            self.parse_type_constraints();
            error.error(self, "Type alias parameters can't have bounds");
        }

        self.expect_3(EQ, "Expecting '='", Some(TOP_LEVEL_DECLARATION_FIRST_SEMICOLON_SET));

        self.parse_type_ref();

        self.consume_if(SEMICOLON);

        TYPEALIAS
    }
}
