//! Port of `KotlinParsing.java` lines 742-1001 (file annotations, annotations, `NameParsingMode`).

use ktrs_syntax::SyntaxKind::{self, *};

use super::parameters::AnnotationParsingMode;
use super::{ANNOTATION_TARGETS, IDENTIFIER_RBRACKET_LBRACKET_SET};
use crate::kt_tokens::{VAL_VAR, WHITE_SPACE_OR_COMMENT_BIT_SET};
use crate::parsing::Parser;

impl Parser {
    /*
     * fileAnnotationList
     *   : ("[" "file:" annotationEntry+ "]")*
     *   ;
     */
    pub(super) fn parse_file_annotation_list(&mut self, mode: AnnotationParsingMode) {
        debug_assert!(mode.is_file_annotation_parsing_mode(), "expected file annotation parsing mode, but:{mode:?}");

        let file_annotations_list = self.mark();

        if self.parse_annotations(mode) {
            file_annotations_list.done(self, FILE_ANNOTATION_LIST);
        } else {
            file_annotations_list.drop(self);
        }
    }

    /*
     * annotations
     *   : (annotation | annotationList)*
     *   ;
     */
    pub(crate) fn parse_annotations(&mut self, mode: AnnotationParsingMode) -> bool {
        if !self.parse_annotation_or_list(mode) {
            return false;
        }

        while self.parse_annotation_or_list(mode) {
            // do nothing
        }

        true
    }

    /*
     * annotation
     *   : "@" (annotationUseSiteTarget ":")? unescapedAnnotation
     *   ;
     *
     * annotationList
     *   : "@" (annotationUseSiteTarget ":")? "[" unescapedAnnotation+ "]"
     *   ;
     */
    pub(super) fn parse_annotation_or_list(&mut self, mode: AnnotationParsingMode) -> bool {
        if self.at(AT) {
            let next_raw_token = self.my_builder.raw_lookup(1);
            let mut token_to_match = next_raw_token;
            let mut is_targeted_annotation = false;

            if (next_raw_token == Some(IDENTIFIER) || ANNOTATION_TARGETS.contains(next_raw_token))
                && self.lookahead(2) == Some(COLON)
            {
                token_to_match = self.lookahead(3);
                is_targeted_annotation = true;
            } else if self.lookahead(1) == Some(COLON) {
                // recovery for "@:ann"
                is_targeted_annotation = true;
                token_to_match = self.lookahead(2);
            }

            if token_to_match == Some(IDENTIFIER) {
                return self.parse_annotation(mode);
            } else if token_to_match == Some(LBRACKET) {
                return self.parse_annotation_list(mode);
            } else if is_targeted_annotation {
                if self.lookahead(1) == Some(COLON) {
                    self.error_and_advance_2("Expected annotation identifier after ':'", 2); // AT, COLON
                } else {
                    self.error_and_advance_2("Expected annotation identifier after ':'", 3); // AT, (ANNOTATION TARGET KEYWORD), COLON
                }
            } else {
                self.error_and_advance_2("Expected annotation identifier after '@'", 1); // AT
            }
            return true;
        }

        false
    }

    fn parse_annotation_list(&mut self, mode: AnnotationParsingMode) -> bool {
        debug_assert!(self._at(AT));
        let annotation = self.mark();

        self.my_builder.disable_newlines();

        self.advance(); // AT

        if !self.parse_annotation_target_if_needed(mode) {
            annotation.rollback_to(self);
            self.my_builder.restore_newlines_state();
            return false;
        }

        debug_assert!(self._at(LBRACKET));
        self.advance(); // LBRACKET

        if !self.at(IDENTIFIER) && !self.at(AT) {
            self.error("Expecting a list of annotations");
        } else {
            while self.at(IDENTIFIER) || self.at(AT) {
                if self.at(AT) {
                    self.error_and_advance("No '@' needed in annotation list"); // AT
                    continue;
                }

                self.parse_annotation(AnnotationParsingMode::Default);
                while self.at(COMMA) {
                    self.error_and_advance("No commas needed to separate annotations");
                }
            }
        }

        self.expect_2(RBRACKET, "Expecting ']' to close the annotation list");
        self.my_builder.restore_newlines_state();

        annotation.done(self, ANNOTATION);
        true
    }

    // Returns true if we should continue parse annotation
    fn parse_annotation_target_if_needed(&mut self, mode: AnnotationParsingMode) -> bool {
        let expected_annotation_target_before_colon = "Expected annotation target before ':'";

        if self.at(COLON) {
            // recovery for "@:ann"
            self.error_and_advance(expected_annotation_target_before_colon); // COLON
            return true;
        }

        let target_keyword = if self.at_set(ANNOTATION_TARGETS) { self.my_builder.get_token_type() } else { None };
        if mode == AnnotationParsingMode::FileAnnotationsWhenPackageOmitted
            && !(target_keyword == Some(FILE_KEYWORD) && self.lookahead(1) == Some(COLON))
        {
            return false;
        }

        if self.lookahead(1) == Some(COLON) && target_keyword.is_none() && self.at(IDENTIFIER) {
            // recovery for "@fil:ann"
            self.error_and_advance(expected_annotation_target_before_colon); // IDENTIFIER
            self.advance(); // COLON
            return true;
        }

        if target_keyword.is_none() && mode.is_file_annotation_parsing_mode() {
            self.parse_annotation_target(FILE_KEYWORD);
        } else if let Some(target_keyword) = target_keyword {
            self.parse_annotation_target(target_keyword);
        }

        true
    }

    fn parse_annotation_target(&mut self, keyword: SyntaxKind) {
        let marker = self.mark();

        if !self.expect(keyword) {
            self.error(&generate_annotation_target_error_message(keyword));
            marker.drop(self);
        } else {
            marker.done(self, ANNOTATION_TARGET);
        }

        if !self.expect(COLON) {
            self.error_with_recovery(
                &generate_annotation_target_error_message(keyword),
                Some(IDENTIFIER_RBRACKET_LBRACKET_SET),
            );
        }
    }

    /*
     * annotation
     *   : "@" (annotationUseSiteTarget ":")? unescapedAnnotation
     *   ;
     *
     * unescapedAnnotation
     *   : SimpleName{"."} typeArguments? valueArguments?
     *   ;
     */
    fn parse_annotation(&mut self, mode: AnnotationParsingMode) -> bool {
        debug_assert!(
            self._at(IDENTIFIER)
                // We have "@ann" or "@:ann" or "@ :ann", but not "@ ann"
                // (it's guaranteed that call sites do not allow the latter case)
                || (self._at(AT) && (!self.is_next_raw_token_comment_or_whitespace() || self.lookahead(1) == Some(COLON))),
            "Invalid annotation prefix"
        );

        let annotation = self.mark();

        let at_at = self.at(AT);
        if at_at {
            self.advance(); // AT
        }

        if at_at && !self.parse_annotation_target_if_needed(mode) {
            annotation.rollback_to(self);
            return false;
        }

        let reference = self.mark();
        let type_reference = self.mark();
        self.parse_user_type();
        type_reference.done(self, TYPE_REFERENCE);
        reference.done(self, CONSTRUCTOR_CALLEE);

        self.parse_type_argument_list();

        if self.at(LPAR)
            && !VAL_VAR.contains(self.lookahead(1))
            && !(WHITE_SPACE_OR_COMMENT_BIT_SET.contains(self.my_builder.raw_lookup(-1))
                && mode.with_significant_whitespace_before_arguments())
        {
            self.parse_value_argument_list();

            // Annotations on a function type may have swallowed its parentheses (`@Anno () -> Unit`)
            // or failed on them (`@Anno (x: Any) -> Unit`): report failure so the caller reparses
            // with significant whitespace.
            if mode.type_context() && (self.get_last_token() != Some(RPAR) || self.at(ARROW)) {
                annotation.done(self, ANNOTATION_ENTRY);
                return false;
            }
        }
        annotation.done(self, ANNOTATION_ENTRY);

        true
    }

    fn is_next_raw_token_comment_or_whitespace(&mut self) -> bool {
        WHITE_SPACE_OR_COMMENT_BIT_SET.contains(self.my_builder.raw_lookup(1))
    }
}

fn generate_annotation_target_error_message(keyword: SyntaxKind) -> String {
    let keyword = keyword.keyword_text().unwrap_or_default();
    format!("Expecting \"{keyword}:\" prefix for {keyword} annotations")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NameParsingMode {
    Required,
    Allowed,
    Prohibited,
}
