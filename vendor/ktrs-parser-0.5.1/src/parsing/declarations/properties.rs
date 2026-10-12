//! Port of `KotlinParsing.java` lines 1456-1605 (`DeclarationParsingMode`, properties, delegates).

use ktrs_syntax::SyntaxKind::{self, *};

use super::accessors::{MultiDeclarationMode, PropertyComponentKindCollector};
use super::{
    DECLARATION_FIRST, DESTRUCTURING_PROPERTY_NAME_FOLLOW_SET, EOL_OR_SEMICOLON_RBRACE_SET,
    IDENTIFIER_EQ_COLON_SEMICOLON_SET, PROPERTY_NAME_FOLLOW_FUNCTION_OR_PROPERTY_RECOVERY_SET,
    PROPERTY_NAME_FOLLOW_MULTI_DECLARATION_RECOVERY_SET, PROPERTY_NAME_FOLLOW_SET,
};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclarationParsingMode {
    MemberOrToplevel,
    Local,
    ScriptToplevel,
}

impl DeclarationParsingMode {
    pub(crate) fn destructuring_allowed(self) -> bool {
        matches!(self, DeclarationParsingMode::Local | DeclarationParsingMode::ScriptToplevel)
    }

    pub(crate) fn accessors_allowed(self) -> bool {
        matches!(self, DeclarationParsingMode::MemberOrToplevel | DeclarationParsingMode::ScriptToplevel)
    }

    pub(crate) fn can_be_enum_used_as_soft_keyword(self) -> bool {
        matches!(self, DeclarationParsingMode::MemberOrToplevel)
    }
}

impl Parser {
    /*
     * variableDeclarationEntry
     *   : SimpleName (":" type)?
     *   ;
     *
     * property
     *   : modifiers ("val" | "var")
     *       typeParameters?
     *       (type ".")?
     *       ("(" variableDeclarationEntry{","} ")" | variableDeclarationEntry)
     *       typeConstraints
     *       ("by" | "=" expression SEMI?)?
     *       (getter? setter? | setter? getter?) SEMI?
     *   ;
     */
    pub(crate) fn parse_property(&mut self, mode: DeclarationParsingMode) -> SyntaxKind {
        let is_short_form = self.at(VAL_KEYWORD) || self.at(VAR_KEYWORD);
        if is_short_form {
            self.advance();
        }

        let type_parameters_declared = self.at(LT) && self.parse_type_parameter_list(IDENTIFIER_EQ_COLON_SEMICOLON_SET);

        self.my_builder.disable_joining_complex_tokens();

        let receiver = self.mark();
        let receiver_type_declared = self.parse_receiver_type("property", PROPERTY_NAME_FOLLOW_SET);

        let multi_declaration = self.at(LPAR) || self.at(LBRACKET);

        self.error_if(
            receiver,
            multi_declaration && receiver_type_declared,
            "Receiver type is not allowed on a destructuring declaration",
        );

        let is_name_on_the_next_line = self.eol();
        let before_name = self.mark();

        if multi_declaration {
            let multi_decl = self.mark();
            self.parse_multi_declaration_entry(
                if is_short_form { PROPERTY_NAME_FOLLOW_SET } else { DESTRUCTURING_PROPERTY_NAME_FOLLOW_SET },
                PROPERTY_NAME_FOLLOW_MULTI_DECLARATION_RECOVERY_SET,
                if is_short_form { MultiDeclarationMode::Short } else { MultiDeclarationMode::Full },
            );
            self.error_if(
                multi_decl,
                !mode.destructuring_allowed(),
                "Destructuring declarations are only allowed for local variables/values",
            );
        } else {
            self.parse_function_or_property_name(
                receiver_type_declared,
                "property",
                PROPERTY_NAME_FOLLOW_SET,
                PROPERTY_NAME_FOLLOW_FUNCTION_OR_PROPERTY_RECOVERY_SET,
                /*nameRequired = */ true,
            );
        }

        self.my_builder.restore_joining_complex_tokens_state();

        let mut no_type_reference = true;
        if self.at(COLON) {
            no_type_reference = false;
            let type_ = self.mark();
            self.advance(); // COLON
            self.parse_type_ref();
            self.error_if(type_, multi_declaration, "Type annotations are not allowed on destructuring declarations");
        }

        self.parse_type_constraints_guarded(type_parameters_declared);

        if !self.parse_property_delegate_or_assignment()
            && is_name_on_the_next_line
            && no_type_reference
            && !receiver_type_declared
        {
            // Do not parse property identifier on the next line if declaration is invalid
            // In most cases this identifier relates to next statement/declaration
            if !multi_declaration || is_short_form {
                before_name.rollback_to(self);
                self.error("Expecting property name or receiver type");
            } else {
                before_name.drop(self);
            }

            return if multi_declaration { DESTRUCTURING_DECLARATION } else { PROPERTY };
        }

        before_name.drop(self);

        if mode.accessors_allowed() {
            // It's only needed for non-local properties, because in local ones:
            // "val a = 1; b" must not be an infix call of b on "val ...;"

            self.my_builder.enable_newlines();
            let has_new_line_with_semicolon = self.consume_if(SEMICOLON) && self.my_builder.newline_before_current_token();
            self.my_builder.restore_newlines_state();

            if !has_new_line_with_semicolon {
                let mut already_read = PropertyComponentKindCollector::default();
                let mut property_component_kind = self.parse_property_component(&already_read);

                while let Some(kind) = property_component_kind {
                    already_read.collect(kind);
                    property_component_kind = self.parse_property_component(&already_read);
                }

                if !self.at_set(EOL_OR_SEMICOLON_RBRACE_SET) {
                    if self.get_last_token() != Some(SEMICOLON) {
                        self.error_until(
                            "Property getter or setter expected",
                            TokenSet::or_set(&[DECLARATION_FIRST, TokenSet::create(&[EOL_OR_SEMICOLON, LBRACE, RBRACE])]),
                        );
                    }
                } else {
                    self.consume_if(SEMICOLON);
                }
            }
        }

        if multi_declaration { DESTRUCTURING_DECLARATION } else { PROPERTY }
    }

    fn parse_property_delegate_or_assignment(&mut self) -> bool {
        if self.at(BY_KEYWORD) {
            self.parse_property_delegate();
            return true;
        } else if self.at(EQ) {
            self.advance(); // EQ
            self.parse_expression();
            return true;
        }

        false
    }

    /*
     * propertyDelegate
     *   : "by" expression
     *   ;
     */
    fn parse_property_delegate(&mut self) {
        debug_assert!(self._at(BY_KEYWORD));
        let delegate = self.mark();
        self.advance(); // BY_KEYWORD
        self.parse_expression();
        delegate.done(self, PROPERTY_DELEGATE);
    }
}
