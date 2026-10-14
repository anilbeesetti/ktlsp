//! Port of `KotlinParsing.java` lines 2078-2207 (type parameters, constraints, contracts).

use ktrs_syntax::SyntaxKind::*;

use super::parameters::AnnotationParsingMode;
use super::{
    COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET, GT_COMMA_COLON_SET,
    LBRACE_RBRACE_TYPE_REF_FIRST_SET, TYPE_PARAMETER_GT_RECOVERY_SET,
};
use crate::builder::{PsiBuilder, SemanticWhitespaceAwarePsiBuilder};
use crate::parsing::Parser;
use crate::token_set::TokenSet;

const fn generic_probe_token_fits(start: u32, end: u32, eof: Option<i32>) -> bool {
    end - start <= 4096
        && match eof {
            Some(eof) => end <= eof as u32,
            None => true,
        }
}

const _: () = {
    assert!(!generic_probe_token_fits(100, 103, Some(99)));
    assert!(!generic_probe_token_fits(100, 103, Some(100)));
    assert!(!generic_probe_token_fits(100, 103, Some(102)));
    assert!(generic_probe_token_fits(100, 103, Some(103)));
    assert!(generic_probe_token_fits(100, 103, None));
    assert!(generic_probe_token_fits(100, 4196, None));
    assert!(!generic_probe_token_fits(100, 4197, None));
    assert!(!generic_probe_token_fits(100, 4197, Some(5000)));
};

impl Parser {
    /*
     * typeParameters
     *   : ("<" typeParameter{","} ">"
     *   ;
     */
    pub(super) fn parse_type_parameter_list(&mut self, recovery_set: TokenSet) -> bool {
        self.parse_classifier_type_parameters(recovery_set, false).0
    }

    pub(super) fn parse_classifier_type_parameters(
        &mut self,
        recovery_set: TokenSet,
        recover_functions: bool,
    ) -> (bool, bool) {
        let mut result = false;
        let mut recovered = false;
        if self.at(LT) {
            let list = self.mark();

            self.my_builder.disable_newlines();
            self.advance(); // LT

            let mut probe_tokens = 4096;
            loop {
                if recover_functions && self.at_generic_sibling_function(&mut probe_tokens) {
                    self.error("Type parameter declaration expected");
                    recovered = true;
                    break;
                }
                if self.at(COMMA) {
                    self.error_and_advance("Expecting type parameter declaration");
                }
                let complete = self.parse_type_parameter(recover_functions);

                if recover_functions
                    && complete
                    && self.at_generic_sibling_function(&mut probe_tokens)
                {
                    recovered = true;
                    break;
                }

                if !self.at(COMMA) {
                    break;
                }
                self.advance(); // COMMA
                if self.at(GT) {
                    break;
                }
            }

            if recovered {
                self.error("Missing '>'");
            } else {
                self.expect_3(GT, "Missing '>'", Some(recovery_set));
            }
            self.my_builder.restore_newlines_state();
            result = true;

            list.done(self, TYPE_PARAMETER_LIST);
        }
        (result, recovered)
    }

    fn at_generic_sibling_function(&self, remaining: &mut usize) -> bool {
        if self.in_parameter_recovery_probe || *remaining == 0 {
            return false;
        }
        let psi = &self.my_builder.psi;
        let start = psi.shift_over_whitespace_forward(psi.current_lexeme);
        let Some(&kind) = psi.lex_types.get(start) else {
            return false;
        };
        let word = &psi.text[psi.lex_starts[start] as usize..psi.lex_starts[start + 1] as usize];
        if !matches!(kind, AT | FUN_KEYWORD | VAL_KEYWORD)
            && !crate::kt_tokens::MODIFIER_KEYWORDS.contains(kind)
            && !crate::kt_tokens::MODIFIER_KEYWORDS
                .contains(crate::kt_tokens::soft_keyword_by_text(Some(word)))
        {
            return false;
        }
        // shortcut: prefixes exceeding 256 raw tokens/4096 bytes decline recovery; expand only with bounded corpus evidence.
        let limit = (start + 256.min(*remaining)).min(psi.lex_types.len());
        let eof = self.my_builder.effective_truncated_eof_position();
        let mut end = start;
        while end < limit
            && generic_probe_token_fits(psi.lex_starts[start], psi.lex_starts[end + 1], eof)
        {
            end += 1;
        }
        *remaining -= (end - start).max(1).min(*remaining);
        if start == end {
            return false;
        }
        let builder = PsiBuilder::from_lexemes(
            &psi.text[psi.lex_starts[start] as usize..psi.lex_starts[end] as usize],
            &psi.lex_starts[start..=end],
            &psi.lex_types[start..end],
        );
        if let Some(observer) = crate::generic_copy_observer() {
            let (active_layers, independent_min) = self.my_builder.generic_copy_active_layers();
            let next_token = psi
                .lex_starts
                .get(end + 1)
                .map(|&next_end| (psi.lex_starts[end], next_end));
            observer(
                &psi.text,
                crate::GenericCopyBoundary {
                    active_layers,
                    independent_min,
                    production_bound: eof,
                    window_start: psi.lex_starts[start],
                    actual_copy_end: builder
                        .lex_starts
                        .last()
                        .and_then(|&last| psi.lex_starts[start].checked_add(last)),
                    next_token,
                    next_fits_other_caps: end < limit
                        && next_token
                            .is_some_and(|(_, next_end)| next_end - psi.lex_starts[start] <= 4096),
                },
            );
        }
        let mut probe =
            Parser::create_for_top_level_non_lazy(SemanticWhitespaceAwarePsiBuilder::new(builder));
        probe.in_parameter_recovery_probe = true;
        let marker = probe.mark();
        let found = if kind == VAL_KEYWORD {
            probe.expect(VAL_KEYWORD) && probe.expect(IDENTIFIER) && probe.at(EQ)
        } else {
            probe.parse_modifier_list(GT_COMMA_COLON_SET);
            !probe.my_builder.psi.has_errors_after(marker)
                && probe.expect(FUN_KEYWORD)
                && probe.expect(IDENTIFIER)
                && (if probe.consume_if(SAFE_ACCESS) {
                    probe.expect(IDENTIFIER)
                } else if probe.consume_if(QUEST) {
                    probe.expect(DOT) && probe.expect(IDENTIFIER)
                } else {
                    let mut plain = true;
                    let mut identifiers = 1;
                    while probe.consume_if(DOT) {
                        if !probe.expect(IDENTIFIER) {
                            plain = false;
                            break;
                        }
                        identifiers += 1;
                    }
                    plain
                        && (if identifiers == 2 && probe.consume_if(SAFE_ACCESS) {
                            probe.expect(IDENTIFIER)
                        } else if identifiers == 2 && probe.consume_if(QUEST) {
                            probe.expect(DOT) && probe.expect(IDENTIFIER)
                        } else {
                            true
                        })
                })
                && probe.at(LPAR)
        };
        marker.drop(&mut probe);
        found
    }

    fn completed_generic_bound(&self, first_event: usize, first_token: usize) -> bool {
        let psi = &self.my_builder.psi;
        let mut reference = false;
        let mut concrete = false;
        let mut parents = Vec::new();
        for &id in &psi.production.list[first_event..] {
            if id < 0 {
                parents.pop();
                continue;
            }
            let node = psi.production.marker(id);
            if node.is_error_item || node.kind == Some(ERROR_ELEMENT) || !node.is_done() {
                return false;
            }
            reference |= node.kind == Some(TYPE_REFERENCE)
                && node.lexeme as usize == first_token
                && node.done_lexeme > node.lexeme;
            concrete |= parents.len() == 1
                && matches!(
                    node.kind,
                    Some(
                        USER_TYPE
                            | FUNCTION_TYPE
                            | DYNAMIC_TYPE
                            | NULLABLE_TYPE
                            | INTERSECTION_TYPE
                    )
                );
            let closing = match node.kind {
                Some(TYPE_ARGUMENT_LIST) => Some(GT),
                Some(VALUE_PARAMETER_LIST | VALUE_ARGUMENT_LIST) => Some(RPAR),
                _ => None,
            };
            if let Some(closing) = closing {
                let last = (node.lexeme as usize..node.done_lexeme as usize)
                    .rev()
                    .find(|&i| !psi.lex_types[i].is_trivia());
                if last.is_none_or(|i| psi.lex_types[i] != closing) {
                    return false;
                }
            }
            parents.push(id);
        }
        reference && concrete
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
        if self.expect_3(
            IDENTIFIER,
            "Expecting type parameter name",
            Some(COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET),
        ) {
            reference.done(self, REFERENCE_EXPRESSION);
        } else {
            reference.drop(self);
        }

        self.expect_3(
            COLON,
            "Expecting ':' before the upper bound",
            Some(LBRACE_RBRACE_TYPE_REF_FIRST_SET),
        );

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
    fn parse_type_parameter(&mut self, check_completion: bool) -> bool {
        if self.at_set(TYPE_PARAMETER_GT_RECOVERY_SET) {
            self.error("Type parameter declaration expected");
            return false;
        }

        let mark = self.mark();

        self.parse_modifier_list(GT_COMMA_COLON_SET);

        let mut complete = self.expect_3(
            IDENTIFIER,
            "Type parameter name expected",
            Some(TokenSet::EMPTY),
        );

        if self.at(COLON) {
            self.advance(); // COLON
            let bound_start = if check_completion {
                self.tt();
                Some((
                    self.my_builder.psi.production.size(),
                    self.my_builder.psi.current_lexeme,
                ))
            } else {
                None
            };
            self.parse_type_ref();
            if let Some((first_event, first_token)) = bound_start {
                complete &= self.completed_generic_bound(first_event, first_token);
            }
        }

        if check_completion {
            complete &= !self.my_builder.psi.has_errors_after(mark);
        }
        mark.done(self, TYPE_PARAMETER);
        complete
    }
}
