//! Port of `KotlinParsing.java` lines 546-740 (modifier lists, context parameter lists).

use ktrs_syntax::SyntaxKind::{self, *};

use super::parameters::{AnnotationParsingMode, NO_MODIFIER_BEFORE_FOR_VALUE_PARAMETER};
use super::{COMMA_COLON_GT_SET, CONTEXT_PARAMETERS_FOLLOW_SET};
use crate::kt_tokens::{
    MODIFIER_KEYWORDS, RESERVED_VALUE_PARAMETER_MODIFIER_KEYWORDS, TYPE_ARGUMENT_MODIFIER_KEYWORDS,
    TYPE_MODIFIER_KEYWORDS,
};
use crate::parsing::{Consumer, Parser};
use crate::token_set::TokenSet;

impl Parser {
    /*
     * (modifier | annotation)*
     */
    pub(crate) fn parse_modifier_list(&mut self, no_modifiers_before: TokenSet) -> bool {
        self.parse_modifier_list_3(
            None,
            no_modifiers_before,
            /* localDeclaration = */ false,
        )
    }

    pub(crate) fn parse_annotations_list(&mut self, no_modifiers_before: TokenSet) {
        self.do_parse_modifier_list(
            None,
            TokenSet::EMPTY,
            AnnotationParsingMode::Default,
            no_modifiers_before,
            /* localDeclaration = */ false,
        );
    }

    /// Feeds modifiers (not annotations) into `token_consumer`. `no_modifiers_before`: tokens
    /// after which the previous token is an identifier rather than a modifier.
    pub(crate) fn parse_modifier_list_3(
        &mut self,
        token_consumer: Option<&mut dyn Consumer<SyntaxKind>>,
        no_modifiers_before: TokenSet,
        local_declaration: bool,
    ) -> bool {
        self.do_parse_modifier_list(
            token_consumer,
            MODIFIER_KEYWORDS,
            AnnotationParsingMode::Default,
            no_modifiers_before,
            local_declaration,
        )
    }

    pub(super) fn parse_function_type_value_parameter_modifier_list(&mut self) {
        self.do_parse_modifier_list(
            None,
            RESERVED_VALUE_PARAMETER_MODIFIER_KEYWORDS,
            AnnotationParsingMode::NoAnnotationsNoContext,
            NO_MODIFIER_BEFORE_FOR_VALUE_PARAMETER,
            /* localDeclaration = */ false,
        );
    }

    pub(super) fn parse_type_modifier_list(&mut self) {
        self.do_parse_modifier_list(
            None,
            TYPE_MODIFIER_KEYWORDS,
            AnnotationParsingMode::TypeContext,
            TokenSet::EMPTY,
            /* localDeclaration = */ false,
        );
    }

    pub(super) fn parse_type_argument_modifier_list(&mut self) {
        self.do_parse_modifier_list(
            None,
            TYPE_ARGUMENT_MODIFIER_KEYWORDS,
            AnnotationParsingMode::NoAnnotationsNoContext,
            COMMA_COLON_GT_SET,
            /* localDeclaration = */ false,
        );
    }

    fn do_parse_modifier_list_body(
        &mut self,
        mut token_consumer: Option<&mut dyn Consumer<SyntaxKind>>,
        modifier_keywords: TokenSet,
        annotation_parsing_mode: AnnotationParsingMode,
        no_modifiers_before: TokenSet,
        local_declaration: bool,
    ) -> bool {
        let mut empty = true;
        while !self.eof() {
            if self.at(AT) && annotation_parsing_mode.allow_annotations() {
                let before_annotation_marker = self.mark();

                let is_annotation_parsed = self.parse_annotation_or_list(annotation_parsing_mode);

                if !is_annotation_parsed
                    && !annotation_parsing_mode.with_significant_whitespace_before_arguments()
                {
                    before_annotation_marker.rollback_to(self);
                    // try parse again, but with significant whitespace
                    let new_mode = if annotation_parsing_mode.allow_context_list() {
                        AnnotationParsingMode::WithSignificantWhitespaceBeforeArguments
                    } else {
                        AnnotationParsingMode::WithSignificantWhitespaceBeforeArgumentsNoContext
                    };
                    self.do_parse_modifier_list_body(
                        reborrow(&mut token_consumer),
                        modifier_keywords,
                        new_mode,
                        no_modifiers_before,
                        local_declaration,
                    );
                    empty = false;
                    break;
                } else {
                    before_annotation_marker.drop(self);
                }
            } else if self.at(CONTEXT_KEYWORD)
                && annotation_parsing_mode.allow_context_list()
                && self.lookahead(1) == Some(LPAR)
            {
                let context_marker = self.mark();
                if !self.parse_context_parameter_or_receiver_list(false) && local_declaration {
                    // Rollback the entire context declaration to make it possible to prevent parsing of potential local declarations
                    // that in fact are not declarations (we are trying to parse declarations at first and statements as second).
                    context_marker.rollback_to(self);
                    break;
                } else {
                    context_marker.drop(self);
                }
            } else if self.try_parse_modifier(
                reborrow(&mut token_consumer),
                no_modifiers_before,
                modifier_keywords,
            ) {
                // modifier advanced
            } else {
                break;
            }
            empty = false;
        }

        empty
    }

    fn do_parse_modifier_list(
        &mut self,
        token_consumer: Option<&mut dyn Consumer<SyntaxKind>>,
        modifier_keywords: TokenSet,
        annotation_parsing_mode: AnnotationParsingMode,
        no_modifiers_before: TokenSet,
        local_declaration: bool,
    ) -> bool {
        let list = self.mark();

        let empty = self.do_parse_modifier_list_body(
            token_consumer,
            modifier_keywords,
            annotation_parsing_mode,
            no_modifiers_before,
            local_declaration,
        );

        if empty {
            list.drop(self);
        } else {
            list.done(self, MODIFIER_LIST);
        }
        !empty
    }

    fn try_parse_modifier(
        &mut self,
        token_consumer: Option<&mut dyn Consumer<SyntaxKind>>,
        no_modifiers_before: TokenSet,
        modifier_keywords: TokenSet,
    ) -> bool {
        let marker = self.mark();

        if self.at_set(modifier_keywords) {
            let lookahead = self.lookahead(1);

            if self.at(FUN_KEYWORD) && lookahead != Some(INTERFACE_KEYWORD) {
                marker.rollback_to(self);
                return false;
            }

            if lookahead.is_some() && !no_modifiers_before.contains(lookahead) {
                let tt = self.tt().expect("at_set matched a token");
                if let Some(token_consumer) = token_consumer {
                    token_consumer.consume(tt);
                }
                self.advance(); // MODIFIER
                marker.collapse(self, tt);
                return true;
            }
        }

        marker.rollback_to(self);
        false
    }

    /// contextReceiverList : "context" "(" (contextReceiver{","})+ ")"
    ///
    /// Returns `true` if it parsed a context with value parameters; `false` for a context with
    /// type refs (that work as receivers) or on a syntax error.
    pub(super) fn parse_context_parameter_or_receiver_list(
        &mut self,
        in_function_type: bool,
    ) -> bool {
        debug_assert!(self._at(CONTEXT_KEYWORD));
        let value_parameter_or_type_ref_list = self.mark();
        self.advance(); // CONTEXT_KEYWORD

        debug_assert!(self._at(LPAR));

        let no_error;

        if self.lookahead(1) == Some(RPAR) {
            self.advance(); // LPAR
            self.error("Empty context parameter list");
            self.advance(); // RPAR
            no_error = false;
        } else {
            // Treat parsing of context receivers (deprecated syntax) as an error,
            // But an outer caller decides if the entire list should be dropped:
            // If we're trying to parse a local declaration, we should drop it to prevent unexpected parsing of ahead declarations
            no_error = self.value_parameter_loop(
                in_function_type,
                false,
                CONTEXT_PARAMETERS_FOLLOW_SET,
                &mut |p: &mut Parser| p.parse_value_parameter_or_type_ref(in_function_type),
            );
        }

        value_parameter_or_type_ref_list.done(self, CONTEXT_PARAMETER_LIST);
        no_error
    }

    /// contextReceiver : label? typeReference
    ///
    /// Returns `true` if it parsed a value parameter or type ref in the correct position (in
    /// function type).
    fn parse_value_parameter_or_type_ref(&mut self, in_function_type: bool) -> bool {
        if self.try_parse_value_parameter(true) {
            return true;
        }

        let context_receiver = self.mark();
        if !in_function_type && self.is_at_label_definition_or_missing_identifier() {
            self.parse_label_definition();
        }
        self.parse_type_ref();
        context_receiver.done(self, CONTEXT_RECEIVER);
        in_function_type
    }
}

/// `Option::as_deref_mut` can't shorten the trait-object lifetime behind `&mut`; this can.
fn reborrow<'a>(
    consumer: &'a mut Option<&mut dyn Consumer<SyntaxKind>>,
) -> Option<&'a mut dyn Consumer<SyntaxKind>> {
    match consumer {
        Some(consumer) => Some(&mut **consumer),
        None => None,
    }
}
