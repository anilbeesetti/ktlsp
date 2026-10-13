//! Port of `KotlinParsing.java` lines 1003-1248 (classes, objects, enum bodies and entries).

use ktrs_syntax::SyntaxKind::{self, *};

use super::{
    CLASS_INTERFACE_SET, CLASS_NAME_RECOVERY_SET, COMMA_SEMICOLON_RBRACE_SET, LBRACE_RBRACE_SET,
    LPAR_LBRACE_COLON_CONSTRUCTOR_KEYWORD_SET, NameParsingMode, SOFT_KEYWORDS_AT_MEMBER_START,
    TYPE_PARAMETER_GT_RECOVERY_SET,
};
use crate::parsing::{OptionalMarker, Parser};
use crate::token_set::TokenSet;

enum ParseEnumEntryResult {
    Failed,
    NoDelimiter,
    CommaDelimiter,
    SemicolonDelimiter,
}

impl Parser {
    /*
     * class
     *   : modifiers ("class" | "interface") SimpleName
     *       typeParameters?
     *       primaryConstructor?
     *       (":" annotations delegationSpecifier{","})?
     *       typeConstraints
     *       (classBody? | enumClassBody)
     *   ;
     *
     * primaryConstructor
     *   : (modifiers "constructor")? ("(" functionParameter{","} ")")
     *   ;
     *
     * object
     *   : "object" SimpleName? primaryConstructor? ":" delegationSpecifier{","}? classBody?
     *   ;
     */
    fn parse_class_or_object(
        &mut self,
        object: bool,
        name_parsing_mode: NameParsingMode,
        optional_body: bool,
        enum_class: bool,
        expect_kind_keyword: bool,
    ) -> SyntaxKind {
        if expect_kind_keyword {
            if object {
                debug_assert!(self._at(OBJECT_KEYWORD));
            } else {
                debug_assert!(self._at_set(CLASS_INTERFACE_SET));
            }
            self.advance(); // CLASS_KEYWORD, INTERFACE_KEYWORD or OBJECT_KEYWORD
        } else {
            debug_assert!(
                enum_class,
                "Currently classifiers without class/interface/object are only allowed for enums"
            );
            self.error("'class' keyword is expected after 'enum'");
        }

        if name_parsing_mode == NameParsingMode::Required {
            self.expect_3(IDENTIFIER, "Name expected", Some(CLASS_NAME_RECOVERY_SET));
        } else {
            debug_assert!(object, "Must be an object to be nameless");
            if self.at(IDENTIFIER) {
                if name_parsing_mode == NameParsingMode::Prohibited {
                    self.error_and_advance("An object expression cannot bind a name");
                } else {
                    debug_assert!(name_parsing_mode == NameParsingMode::Allowed);
                    self.advance();
                }
            }
        }

        let (type_parameters_declared, recovered_function) =
            self.parse_classifier_type_parameters(TYPE_PARAMETER_GT_RECOVERY_SET, !object);
        if recovered_function {
            return CLASS;
        }

        let before_constructor_modifiers = self.mark();
        let primary_constructor_marker = self.mark();
        let has_constructor_modifiers = self.parse_modifier_list(TokenSet::EMPTY);

        // Some modifiers found, but no parentheses following: class has already ended, and we are looking at something else
        if has_constructor_modifiers && !self.at_set(LPAR_LBRACE_COLON_CONSTRUCTOR_KEYWORD_SET) {
            before_constructor_modifiers.rollback_to(self);
            return if object { OBJECT_DECLARATION } else { CLASS };
        }

        // We are still inside a class declaration
        before_constructor_modifiers.drop(self);

        let has_constructor_keyword = self.at(CONSTRUCTOR_KEYWORD);
        if has_constructor_keyword {
            self.advance(); // CONSTRUCTOR_KEYWORD
        }

        if self.at(LPAR) {
            self.parse_value_parameter_list(
                false,
                /* typeRequired  = */ true,
                LBRACE_RBRACE_SET,
                false,
            );
            primary_constructor_marker.done(self, PRIMARY_CONSTRUCTOR);
        } else if has_constructor_modifiers || has_constructor_keyword {
            // A comprehensive error message for cases like:
            //    class A private : Foo
            // or
            //    class A private {
            primary_constructor_marker.done(self, PRIMARY_CONSTRUCTOR);
            if has_constructor_keyword {
                self.error("Expecting primary constructor parameter list");
            } else {
                self.error("Expecting 'constructor' keyword");
            }
        } else {
            primary_constructor_marker.drop(self);
        }

        if self.at(COLON) {
            self.advance(); // COLON
            self.parse_delegation_specifier_list();
        }

        let where_marker = OptionalMarker::new(self, object);
        self.parse_type_constraints_guarded(type_parameters_declared);
        where_marker.error(self, "Where clause is not allowed for objects");

        if self.at(LBRACE) {
            if enum_class {
                self.parse_enum_class_body();
            } else {
                self.parse_class_body();
            }
        } else if !optional_body {
            let fake_body = self.mark();
            self.error("Expecting a class body");
            fake_body.done(self, CLASS_BODY);
        }

        if object { OBJECT_DECLARATION } else { CLASS }
    }

    pub(super) fn parse_class(
        &mut self,
        enum_class: bool,
        expect_kind_keyword: bool,
    ) -> SyntaxKind {
        self.parse_class_or_object(
            false,
            NameParsingMode::Required,
            true,
            enum_class,
            expect_kind_keyword,
        )
    }

    pub(crate) fn parse_object(&mut self, name_parsing_mode: NameParsingMode, optional_body: bool) {
        self.parse_class_or_object(true, name_parsing_mode, optional_body, false, true);
    }

    /*
     * enumClassBody
     *   : "{" enumEntries (";" members)? "}"
     *   ;
     */
    fn parse_enum_class_body(&mut self) {
        if !self.at(LBRACE) {
            return;
        }

        let body = self.mark();
        self.my_builder.enable_newlines();

        self.advance(); // LBRACE

        if !self.parse_enum_entries() && !self.at(RBRACE) {
            self.error("Expecting ';' after the last enum entry or '}' to close enum class body");
        }
        self.parse_members();
        self.expect_2(RBRACE, "Expecting '}' to close enum class body");

        self.my_builder.restore_newlines_state();
        body.done(self, CLASS_BODY);
    }

    /// enumEntries : enumEntry{","}? ;
    ///
    /// Returns true if enum regular members can follow.
    fn parse_enum_entries(&mut self) -> bool {
        while !self.eof() && !self.at(RBRACE) {
            match self.parse_enum_entry() {
                ParseEnumEntryResult::Failed => {
                    // Special case without any enum entries but with possible members after semicolon
                    if self.at(SEMICOLON) {
                        self.advance();
                        return true;
                    } else {
                        return false;
                    }
                }
                ParseEnumEntryResult::NoDelimiter => return false,
                ParseEnumEntryResult::CommaDelimiter => {}
                ParseEnumEntryResult::SemicolonDelimiter => return true,
            }
        }
        false
    }

    /*
     * enumEntry
     *   : modifiers SimpleName ("(" arguments ")")? classBody?
     *   ;
     */
    fn parse_enum_entry(&mut self) -> ParseEnumEntryResult {
        let entry = self.mark();

        self.parse_modifier_list(COMMA_SEMICOLON_RBRACE_SET);

        if !self.at_set(SOFT_KEYWORDS_AT_MEMBER_START) && self.at(IDENTIFIER) {
            self.advance(); // IDENTIFIER

            if self.at(LPAR) {
                // Arguments should be parsed here
                // Also, "fake" constructor call tree is created,
                // with empty type name inside
                let initializer_list = self.mark();
                let delegator_super_call = self.mark();

                let callee = self.mark();
                let type_reference = self.mark();
                let type_ = self.mark();
                let reference_expr = self.mark();
                reference_expr.done(self, ENUM_ENTRY_SUPERCLASS_REFERENCE_EXPRESSION);
                type_.done(self, USER_TYPE);
                type_reference.done(self, TYPE_REFERENCE);
                callee.done(self, CONSTRUCTOR_CALLEE);

                self.parse_value_argument_list();
                delegator_super_call.done(self, SUPER_TYPE_CALL_ENTRY);
                initializer_list.done(self, INITIALIZER_LIST);
            }
            if self.at(LBRACE) {
                self.parse_class_body();
            }
            let comma_found = self.at(COMMA);
            if comma_found {
                self.advance();
            }
            let semicolon_found = self.at(SEMICOLON);
            if semicolon_found {
                self.advance();
            }

            // Probably some helper function
            self.close_declaration_with_comment_binders(entry, ENUM_ENTRY, true);
            if semicolon_found {
                ParseEnumEntryResult::SemicolonDelimiter
            } else if comma_found {
                ParseEnumEntryResult::CommaDelimiter
            } else {
                ParseEnumEntryResult::NoDelimiter
            }
        } else {
            entry.rollback_to(self);
            ParseEnumEntryResult::Failed
        }
    }
}
