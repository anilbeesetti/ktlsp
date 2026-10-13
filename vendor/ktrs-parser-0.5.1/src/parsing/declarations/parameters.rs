//! Port of `KotlinParsing.java` lines 2543-2818 (value parameters, `ModifierDetector`,
//! `AnnotationParsingMode`).

use ktrs_syntax::SyntaxKind::{self, *};

use super::{
    DECLARATION_FIRST, LAMBDA_VALUE_PARAMETER_FIRST, PARAMETER_NAME_RECOVERY_SET,
    VALUE_PARAMETER_FIRST,
};
use crate::builder::{PsiBuilder, SemanticWhitespaceAwarePsiBuilder};
use crate::parsing::{Consumer, Parser};
use crate::token_set::TokenSet;

pub(super) const NO_MODIFIER_BEFORE_FOR_VALUE_PARAMETER: TokenSet =
    TokenSet::create(&[COMMA, COLON, EQ, RPAR]);

pub(crate) const EXPRESSION_FIRST: TokenSet = TokenSet::create(&[
    // Prefix
    MINUS,
    PLUS,
    MINUSMINUS,
    PLUSPLUS,
    EXCL,
    EXCLEXCL, // Joining complex tokens makes it necessary to put EXCLEXCL here
    // Atomic
    COLONCOLON, // callable reference
    LPAR,       // parenthesized
    // literal constant
    TRUE_KEYWORD,
    FALSE_KEYWORD,
    INTERPOLATION_PREFIX,
    OPEN_QUOTE,
    INTEGER_LITERAL,
    CHARACTER_LITERAL,
    FLOAT_LITERAL,
    NULL_KEYWORD,
    LBRACE,         // functionLiteral
    FUN_KEYWORD,    // expression function
    THIS_KEYWORD,   // this
    SUPER_KEYWORD,  // super
    IF_KEYWORD,     // if
    WHEN_KEYWORD,   // when
    TRY_KEYWORD,    // try
    OBJECT_KEYWORD, // object
    // jump
    THROW_KEYWORD,
    RETURN_KEYWORD,
    CONTINUE_KEYWORD,
    BREAK_KEYWORD,
    // loop
    FOR_KEYWORD,
    WHILE_KEYWORD,
    DO_KEYWORD,
    IDENTIFIER, // SimpleName
    AT,         // Just for better recovery and maybe for annotations
    LBRACKET,   // Collection literal expression
]);

pub(crate) const EXPRESSION_FOLLOW: TokenSet =
    TokenSet::create(&[EOL_OR_SEMICOLON, ARROW, COMMA, RBRACE, RPAR, RBRACKET]);

pub(super) const USER_TYPE_NAME_RECOVERY_SET: TokenSet =
    TokenSet::or_set(&[EXPRESSION_FIRST, EXPRESSION_FOLLOW, DECLARATION_FIRST]);

impl Parser {
    /*
     * functionParameters
     *   : "(" functionParameter{","}? ")"
     *   ;
     *
     * functionParameter
     *   : modifiers functionParameterRest
     *   ;
     *
     * functionParameterRest
     *   : parameter ("=" element)?
     *   ;
     */
    pub(super) fn parse_value_parameter_list(
        &mut self,
        is_function_type_contents: bool,
        type_required: bool,
        recovery_set: TokenSet,
        recover_declarations: bool,
    ) {
        debug_assert!(self._at(LPAR));
        let parameters = self.mark();

        self.my_builder.disable_newlines();

        self.value_parameter_loop(
            is_function_type_contents,
            recover_declarations,
            recovery_set,
            &mut |p: &mut Parser| {
                if is_function_type_contents {
                    if !p.try_parse_value_parameter(type_required) {
                        let value_parameter = p.mark();
                        p.parse_function_type_value_parameter_modifier_list();
                        p.parse_type_ref();
                        p.close_declaration_with_comment_binders(
                            value_parameter,
                            VALUE_PARAMETER,
                            false,
                        );
                    }
                } else {
                    p.parse_value_parameter_2(false, type_required, recover_declarations);
                }
                true
            },
        );

        self.my_builder.restore_newlines_state();

        parameters.done(self, VALUE_PARAMETER_LIST);
    }

    /// `parse_parameter` returns `true` if internal parsing is correct; the result is `true` if the
    /// parsing of the entire parameter loop is correct.
    pub(super) fn value_parameter_loop(
        &mut self,
        in_function_type_context: bool,
        recover_declarations: bool,
        recovery_set: TokenSet,
        parse_parameter: &mut dyn FnMut(&mut Parser) -> bool,
    ) -> bool {
        self.advance(); // LPAR

        let mut no_error = true;

        if !self.at(RPAR) && !self.at_set(recovery_set) {
            loop {
                let offset_before = self.my_builder.get_current_offset();
                if self.at(RPAR) || (recover_declarations && self.at_set(recovery_set)) {
                    break;
                }
                if recover_declarations && self.at_parameter_list_declaration() {
                    self.error("Expecting ')'");
                    return false;
                }

                no_error = parse_parameter(self) && no_error;

                if recover_declarations && self.at_parameter_list_declaration() {
                    self.error("Expecting ')'");
                    return false;
                }
                if self.at(COMMA) {
                    self.advance(); // COMMA
                } else if self.at(COLON) {
                    // recovery for the case "fun bar(x: Array<Int> : Int)" when we've just parsed "x: Array<Int>"
                    // error should be reported in the `parseValueParameter` call
                    continue;
                } else {
                    if !self.at(RPAR) {
                        self.error("Expecting comma or ')'");
                        no_error = false;
                    }
                    if !self.at_set(if in_function_type_context {
                        LAMBDA_VALUE_PARAMETER_FIRST
                    } else {
                        VALUE_PARAMETER_FIRST
                    }) {
                        break;
                    }
                    if offset_before == self.my_builder.get_current_offset() {
                        break;
                    }
                }
            }
        }

        self.expect_3(RPAR, "Expecting ')'", Some(recovery_set)) && no_error
    }

    fn at_parameter_list_declaration(&mut self) -> bool {
        // shortcut: malformed defaults/nested types need their own recovery corpus beyond parameter boundaries.
        // Nested annotation functions must not recursively replay speculative probes.
        if self.in_parameter_recovery_probe {
            return false;
        }
        self.in_parameter_recovery_probe = true;
        let probe = self.mark();
        self.parse_modifier_list(NO_MODIFIER_BEFORE_FOR_VALUE_PARAMETER);
        let declaration = self.at_set(super::TOP_LEVEL_DECLARATION_FIRST)
            || (self.at(INIT_KEYWORD) && self.lookahead(1) == Some(LBRACE))
            || (self.at(CONSTRUCTOR_KEYWORD) && self.lookahead(1) == Some(LPAR));
        probe.rollback_to(self);
        self.in_parameter_recovery_probe = false;
        declaration
    }

    /*
     * functionParameter
     *   : modifiers ("val" | "var")? parameter ("=" element)?
     *   ;
     */
    pub(super) fn try_parse_value_parameter(&mut self, type_required: bool) -> bool {
        self.parse_value_parameter_2(true, type_required, false)
    }

    pub(crate) fn parse_value_parameter(&mut self, type_required: bool) {
        self.parse_value_parameter_2(false, type_required, false);
    }

    fn parse_value_parameter_2(
        &mut self,
        rollback_on_failure: bool,
        type_required: bool,
        recover_declarations: bool,
    ) -> bool {
        let parameter = self.mark();

        self.parse_modifier_list(NO_MODIFIER_BEFORE_FOR_VALUE_PARAMETER);

        if self.at(VAR_KEYWORD) || self.at(VAL_KEYWORD) {
            self.advance(); // VAR_KEYWORD | VAL_KEYWORD
        }

        if !self.parse_function_parameter_rest(type_required, recover_declarations)
            && rollback_on_failure
        {
            parameter.rollback_to(self);
            return false;
        }

        self.close_declaration_with_comment_binders(parameter, VALUE_PARAMETER, false);
        true
    }

    pub(super) fn at_simple_function_header(&self) -> bool {
        if self.in_parameter_recovery_probe {
            return false;
        }
        let psi = &self.my_builder.psi;
        let start = psi.shift_over_whitespace_forward(psi.current_lexeme);
        if psi.lex_types.get(start) != Some(&FUN_KEYWORD) {
            return false;
        }
        // shortcut: only complete simple zero-parameter headers recover; widen with a bounded corpus.
        let limit = (start + 256).min(psi.lex_types.len());
        let eof = self.my_builder.effective_truncated_eof_position();
        let mut end = start;
        let mut terminals = 0;
        while end < limit
            && psi.lex_starts[end + 1] - psi.lex_starts[start] <= 4096
            && eof.is_none_or(|eof| psi.lex_starts[end + 1] <= eof as u32)
        {
            terminals += usize::from(!psi.lex_types[end].is_trivia());
            end += 1;
            if terminals == 4 {
                break;
            }
        }
        if start == end {
            return false;
        }
        let builder = PsiBuilder::from_lexemes(
            &psi.text[psi.lex_starts[start] as usize..psi.lex_starts[end] as usize],
            &psi.lex_starts[start..=end],
            &psi.lex_types[start..end],
        );
        let mut probe =
            Parser::create_for_top_level_non_lazy(SemanticWhitespaceAwarePsiBuilder::new(builder));
        probe.in_parameter_recovery_probe = true;
        // Four grammar terminals, no nested parsing; all remaps and cursor changes stay in scratch.
        probe.expect(FUN_KEYWORD)
            && probe.expect(IDENTIFIER)
            && probe.expect(LPAR)
            && probe.expect(RPAR)
    }

    /*
     * functionParameterRest
     *   : parameter ("=" element)?
     *   ;
     */
    fn parse_function_parameter_rest(
        &mut self,
        type_required: bool,
        recover_declarations: bool,
    ) -> bool {
        let mut no_errors = true;

        if self.at(COMMA) || self.at(RPAR) {
            self.error("Expecting a parameter declaration");
            no_errors = false;
        } else if self.at(IDENTIFIER) && self.lookahead(1) == Some(LT) {
            // Recovery for the case 'fun foo(Array<String>) {}'
            self.error("Parameter name expected");
            no_errors = false;
            self.parse_parameter_type_ref(recover_declarations);
        } else if self.at(COLON) {
            // Recovery for the case 'fun foo(: Int) {}'
            self.error("Parameter name expected");
            // We keep noErrors == true so that unnamed parameters starting with ":" are not rolled back during parsing of functional types
            self.advance(); // COLON
            self.parse_parameter_type_ref(recover_declarations);
        } else {
            self.expect_3(
                IDENTIFIER,
                "Parameter name expected",
                Some(PARAMETER_NAME_RECOVERY_SET),
            );

            if self.at(COLON) {
                self.advance(); // COLON

                if self.at(IDENTIFIER) && self.lookahead(1) == Some(COLON) {
                    // recovery for the case "fun foo(x: y: Int)" when we're at "y: " it's likely that this is a name of the next parameter,
                    // not a type reference of the current one
                    self.error("Type reference expected");
                    return false;
                }

                self.parse_parameter_type_ref(recover_declarations);
            } else if type_required {
                self.error_with_recovery(
                    "Parameters must have type annotation",
                    Some(PARAMETER_NAME_RECOVERY_SET),
                );
                no_errors = false;
            }
        }

        if self.at(EQ) {
            self.advance(); // EQ
            if recover_declarations && self.at_simple_function_header() {
                self.error("Expecting an expression");
                return false;
            }
            self.parse_expression();
        }

        no_errors
    }
}

#[derive(Default)]
pub(crate) struct ModifierDetector {
    enum_detected: bool,
    companion_detected: bool,
}

impl Consumer<SyntaxKind> for ModifierDetector {
    fn consume(&mut self, item: SyntaxKind) {
        if item == ENUM_KEYWORD {
            self.enum_detected = true;
        } else if item == COMPANION_KEYWORD {
            self.companion_detected = true;
        }
    }
}

impl ModifierDetector {
    pub(crate) fn is_enum_detected(&self) -> bool {
        self.enum_detected
    }

    pub(crate) fn is_companion_detected(&self) -> bool {
        self.companion_detected
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnnotationParsingMode {
    Default,
    FileAnnotationsBeforePackage,
    FileAnnotationsWhenPackageOmitted,
    TypeContext,
    WithSignificantWhitespaceBeforeArguments,
    WithSignificantWhitespaceBeforeArgumentsNoContext,
    NoAnnotationsNoContext,
}

impl AnnotationParsingMode {
    /// `(isFileAnnotationParsingMode, allowAnnotations, allowContextList, typeContext,
    /// withSignificantWhitespaceBeforeArguments)`, the Java constructor arguments.
    const fn flags(self) -> (bool, bool, bool, bool, bool) {
        use AnnotationParsingMode::*;
        match self {
            Default => (false, true, true, false, false),
            FileAnnotationsBeforePackage => (true, true, false, false, false),
            FileAnnotationsWhenPackageOmitted => (true, true, false, false, false),
            TypeContext => (false, true, false, true, false),
            WithSignificantWhitespaceBeforeArguments => (false, true, true, true, true),
            WithSignificantWhitespaceBeforeArgumentsNoContext => (false, true, false, true, true),
            NoAnnotationsNoContext => (false, false, false, false, false),
        }
    }

    pub(crate) fn is_file_annotation_parsing_mode(self) -> bool {
        self.flags().0
    }

    pub(crate) fn allow_annotations(self) -> bool {
        self.flags().1
    }

    pub(crate) fn allow_context_list(self) -> bool {
        self.flags().2
    }

    pub(crate) fn type_context(self) -> bool {
        self.flags().3
    }

    pub(crate) fn with_significant_whitespace_before_arguments(self) -> bool {
        self.flags().4
    }
}
