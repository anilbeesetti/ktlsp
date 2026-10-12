//! Port of `KotlinParsing.java` lines 1607-1801 (destructuring entries, property accessors and
//! backing fields).

use ktrs_syntax::SyntaxKind::*;

use super::{
    ACCESSOR_FIRST_OR_PROPERTY_END, COMMA_COLON_RPAR_SET, COMMA_RPAR_RBRACKET_COLON_EQ_SET, RPAR_COLON_LBRACE_EQ_SET,
    RPAR_IDENTIFIER_COLON_LBRACE_EQ_SET,
};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MultiDeclarationMode {
    Short,
    Full,
    FullValOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropertyComponentKind {
    Get,
    Set,
    Field,
}

/// `PropertyComponentKind.Collector`.
#[derive(Default)]
pub(super) struct PropertyComponentKindCollector {
    collected: [bool; 3],
}

impl PropertyComponentKindCollector {
    pub(super) fn collect(&mut self, kind: PropertyComponentKind) {
        self.collected[kind as usize] = true;
    }

    fn contains(&self, kind: PropertyComponentKind) -> bool {
        self.collected[kind as usize]
    }
}

impl Parser {
    /*
     * (SimpleName (":" type){","})
     */
    pub(crate) fn parse_multi_declaration_entry(
        &mut self,
        follow: TokenSet,
        recovery_set: TokenSet,
        mode: MultiDeclarationMode,
    ) {
        // Parsing multi-name, e.g.
        //   val (a, b) = foo()
        //   (val a: X = aa, var b) = foo()
        //   val [a, b] = foo()
        //   [val a: X, var b] = foo()
        self.my_builder.disable_newlines();

        let is_parentheses = self.at(LPAR);
        let closing_brace = if is_parentheses { RPAR } else { RBRACKET };

        self.advance(); // LPAR | LBRACKET

        if !self.at_set(follow) {
            loop {
                if self.at(COMMA) {
                    self.error_and_advance("Expecting a name");
                } else if self.at(closing_brace) {
                    // For declaration similar to `val () = somethingCall()`
                    self.error("Expecting a name");
                    break;
                }
                let property = self.mark();

                if mode == MultiDeclarationMode::Full {
                    if self.at(VAL_KEYWORD) || self.at(VAR_KEYWORD) {
                        self.advance();
                    } else {
                        self.error_with_recovery("Expecting val or var keyword", Some(recovery_set));
                    }
                } else if mode == MultiDeclarationMode::FullValOnly {
                    if self.at(VAL_KEYWORD) {
                        self.advance();
                    } else {
                        self.error_with_recovery("Expecting val keyword", Some(recovery_set));
                    }
                } else {
                    self.parse_modifier_list(COMMA_RPAR_RBRACKET_COLON_EQ_SET);
                }

                self.expect_3(IDENTIFIER, "Expecting a name", Some(recovery_set));

                if self.at(COLON) {
                    self.advance(); // COLON
                    self.parse_type_ref_1(follow);
                }

                // Renaming is only allowed in name-based destructuring
                if self.at(EQ) && closing_brace == RPAR {
                    self.advance();
                    self.parse_simple_name_expression();
                }

                property.done(self, DESTRUCTURING_DECLARATION_ENTRY);

                if !self.at(COMMA) {
                    break;
                }
                self.advance(); // COMMA
                if self.at(closing_brace) {
                    break;
                }
            }
        }

        self.expect_3(closing_brace, if is_parentheses { "Expecting ')'" } else { "Expecting ']'" }, Some(follow));
        self.my_builder.restore_newlines_state();
    }

    /*
     * propertyComponent
     *   : modifiers ("get" | "set")
     *   :
     *        (     "get" "(" ")"
     *           |
     *              "set" "(" modifiers parameter ")"
     *           |
     *              "field"
     *        ) functionBody
     *   ;
     */
    pub(super) fn parse_property_component(
        &mut self,
        not_allowed_kind: &PropertyComponentKindCollector,
    ) -> Option<PropertyComponentKind> {
        let property_component = self.mark();

        self.parse_modifier_list(TokenSet::EMPTY);

        let property_component_kind = if self.at(GET_KEYWORD) {
            PropertyComponentKind::Get
        } else if self.at(SET_KEYWORD) {
            PropertyComponentKind::Set
        } else if self.at(FIELD_KEYWORD) {
            PropertyComponentKind::Field
        } else {
            property_component.rollback_to(self);
            return None;
        };

        if not_allowed_kind.contains(property_component_kind) {
            property_component.rollback_to(self);
            return None;
        }

        self.advance(); // GET_KEYWORD, SET_KEYWORD or FIELD_KEYWORD

        if !self.at(LPAR) && property_component_kind != PropertyComponentKind::Field {
            // Account for Jet-114 (val a : int get {...})
            if !self.at_set(ACCESSOR_FIRST_OR_PROPERTY_END) {
                self.error_until(
                    "Accessor body expected",
                    TokenSet::or_set(&[ACCESSOR_FIRST_OR_PROPERTY_END, TokenSet::create(&[LBRACE, LPAR, EQ])]),
                );
            } else {
                self.close_declaration_with_comment_binders(property_component, PROPERTY_ACCESSOR, true);
                return Some(property_component_kind);
            }
        }

        self.my_builder.disable_newlines();

        if property_component_kind != PropertyComponentKind::Field {
            let parameter_list = self.mark();
            self.expect_3(LPAR, "Expecting '('", Some(RPAR_IDENTIFIER_COLON_LBRACE_EQ_SET));
            if property_component_kind == PropertyComponentKind::Set {
                let setter_parameter = self.mark();
                self.parse_modifier_list(COMMA_COLON_RPAR_SET);
                self.expect_3(IDENTIFIER, "Expecting parameter name", Some(RPAR_COLON_LBRACE_EQ_SET));

                if self.at(COLON) {
                    self.advance(); // COLON
                    self.parse_type_ref();
                }
                setter_parameter.done(self, VALUE_PARAMETER);
                if self.at(COMMA) {
                    self.advance(); // COMMA
                }
            }
            if !self.at(RPAR) {
                self.error_until("Expecting ')'", TokenSet::create(&[RPAR, COLON, LBRACE, RBRACE, EQ, EOL_OR_SEMICOLON]));
            }
            if self.at(RPAR) {
                self.advance();
            }
            parameter_list.done(self, VALUE_PARAMETER_LIST);
        }
        self.my_builder.restore_newlines_state();

        if self.at(COLON) {
            self.advance();

            self.parse_type_ref();
        }

        if property_component_kind != PropertyComponentKind::Field {
            self.parse_function_contract();
            self.parse_function_body();
        } else if self.at(EQ) {
            self.advance();
            self.parse_expression();
            self.consume_if(SEMICOLON);
        }

        if property_component_kind == PropertyComponentKind::Field {
            self.close_declaration_with_comment_binders(property_component, BACKING_FIELD, true);
        } else {
            self.close_declaration_with_comment_binders(property_component, PROPERTY_ACCESSOR, true);
        }

        Some(property_component_kind)
    }
}
