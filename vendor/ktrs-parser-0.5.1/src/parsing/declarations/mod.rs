//! Port of `KotlinParsing.java` lines 28-264 (constants, entry points); the rest of the class lives
//! in the sibling modules, in upstream order.

mod accessors;
mod annotations;
mod blocks;
mod classes;
mod functions;
mod members;
mod modifiers;
mod parameters;
mod preamble;
mod properties;
mod type_parameters;
mod types;
mod user_types;

pub(crate) use accessors::MultiDeclarationMode;
pub(crate) use annotations::NameParsingMode;
pub(crate) use parameters::{AnnotationParsingMode, EXPRESSION_FIRST, EXPRESSION_FOLLOW, ModifierDetector};
pub(crate) use properties::DeclarationParsingMode;

use ktrs_syntax::SyntaxKind::*;

use super::Parser;
use crate::builder::{EdgeBinder, GREEDY_RIGHT_BINDER};
use crate::kt_tokens::{MODIFIER_KEYWORDS, VAL_VAR};
use crate::token_set::TokenSet;

const GT_COMMA_COLON_SET: TokenSet = TokenSet::create(&[GT, COMMA, COLON]);

const TOP_LEVEL_DECLARATION_FIRST: TokenSet = TokenSet::create(&[
    TYPE_ALIAS_KEYWORD, INTERFACE_KEYWORD, CLASS_KEYWORD, OBJECT_KEYWORD,
    FUN_KEYWORD, VAL_KEYWORD, VAR_KEYWORD, PACKAGE_KEYWORD,
]);
const TOP_LEVEL_DECLARATION_FIRST_SEMICOLON_SET: TokenSet =
    TokenSet::or_set(&[TOP_LEVEL_DECLARATION_FIRST, TokenSet::create(&[SEMICOLON])]);
const LT_EQ_SEMICOLON_TOP_LEVEL_DECLARATION_FIRST_SET: TokenSet =
    TokenSet::or_set(&[TokenSet::create(&[LT, EQ, SEMICOLON]), TOP_LEVEL_DECLARATION_FIRST]);
const DECLARATION_FIRST: TokenSet = TokenSet::or_set(&[
    TOP_LEVEL_DECLARATION_FIRST,
    TokenSet::create(&[INIT_KEYWORD, GET_KEYWORD, SET_KEYWORD, CONSTRUCTOR_KEYWORD]),
]);

const CLASS_NAME_RECOVERY_SET: TokenSet =
    TokenSet::or_set(&[TokenSet::create(&[LT, LPAR, COLON, LBRACE]), TOP_LEVEL_DECLARATION_FIRST]);
const TYPE_PARAMETER_GT_RECOVERY_SET: TokenSet = TokenSet::create(&[WHERE_KEYWORD, LPAR, COLON, LBRACE, GT]);
pub(crate) const PARAMETER_NAME_RECOVERY_SET: TokenSet =
    TokenSet::create(&[COLON, EQ, COMMA, RPAR, VAL_KEYWORD, VAR_KEYWORD]);
const PACKAGE_NAME_RECOVERY_SET: TokenSet = TokenSet::create(&[DOT, EOL_OR_SEMICOLON]);
const IMPORT_RECOVERY_SET: TokenSet = TokenSet::create(&[AS_KEYWORD, DOT, EOL_OR_SEMICOLON]);
const TYPE_REF_FIRST: TokenSet = TokenSet::create(&[LBRACKET, IDENTIFIER, LPAR, HASH, DYNAMIC_KEYWORD]);
const LBRACE_RBRACE_TYPE_REF_FIRST_SET: TokenSet = TokenSet::or_set(&[TokenSet::create(&[LBRACE, RBRACE]), TYPE_REF_FIRST]);
const COLON_COMMA_LBRACE_RBRACE_TYPE_REF_FIRST_SET: TokenSet =
    TokenSet::or_set(&[TokenSet::create(&[COLON, COMMA, LBRACE, RBRACE]), TYPE_REF_FIRST]);
const RECEIVER_TYPE_TERMINATORS: TokenSet = TokenSet::create(&[DOT, SAFE_ACCESS]);
const VALUE_PARAMETER_FIRST: TokenSet = TokenSet::or_set(&[
    TokenSet::create(&[IDENTIFIER, LBRACKET, VAL_KEYWORD, VAR_KEYWORD]),
    TokenSet::and_not(MODIFIER_KEYWORDS, TokenSet::create(&[FUN_KEYWORD])),
]);
const LAMBDA_VALUE_PARAMETER_FIRST: TokenSet = TokenSet::or_set(&[
    TokenSet::create(&[IDENTIFIER, LBRACKET]),
    TokenSet::and_not(MODIFIER_KEYWORDS, TokenSet::create(&[FUN_KEYWORD])),
]);
const SOFT_KEYWORDS_AT_MEMBER_START: TokenSet = TokenSet::create(&[CONSTRUCTOR_KEYWORD, INIT_KEYWORD]);
const ANNOTATION_TARGETS: TokenSet = TokenSet::create(&[
    ALL_KEYWORD, FILE_KEYWORD, FIELD_KEYWORD, GET_KEYWORD, SET_KEYWORD, PROPERTY_KEYWORD,
    RECEIVER_KEYWORD, PARAM_KEYWORD, SETPARAM_KEYWORD, DELEGATE_KEYWORD,
]);
const BLOCK_DOC_COMMENT_SET: TokenSet = TokenSet::create(&[BLOCK_COMMENT, DOC_COMMENT]);
const SEMICOLON_SET: TokenSet = TokenSet::create(&[SEMICOLON]);
const COMMA_COLON_GT_SET: TokenSet = TokenSet::create(&[COMMA, COLON, GT]);
const IDENTIFIER_RBRACKET_LBRACKET_SET: TokenSet = TokenSet::create(&[IDENTIFIER, RBRACKET, LBRACKET]);
const LBRACE_RBRACE_SET: TokenSet = TokenSet::create(&[LBRACE, RBRACE]);
const COMMA_SEMICOLON_RBRACE_SET: TokenSet = TokenSet::create(&[COMMA, SEMICOLON, RBRACE]);
const VALUE_ARGS_RECOVERY_SET: TokenSet = TokenSet::create(&[LBRACE, SEMICOLON, RPAR, EOL_OR_SEMICOLON, RBRACE]);
const PROPERTY_NAME_FOLLOW_SET: TokenSet =
    TokenSet::create(&[COLON, EQ, LBRACE, RBRACE, SEMICOLON, VAL_KEYWORD, VAR_KEYWORD, FUN_KEYWORD, CLASS_KEYWORD]);
const DESTRUCTURING_PROPERTY_NAME_FOLLOW_SET: TokenSet = TokenSet::and_not(PROPERTY_NAME_FOLLOW_SET, VAL_VAR);
const PROPERTY_NAME_FOLLOW_MULTI_DECLARATION_RECOVERY_SET: TokenSet =
    TokenSet::or_set(&[PROPERTY_NAME_FOLLOW_SET, PARAMETER_NAME_RECOVERY_SET]);
const PROPERTY_NAME_FOLLOW_FUNCTION_OR_PROPERTY_RECOVERY_SET: TokenSet =
    TokenSet::or_set(&[PROPERTY_NAME_FOLLOW_SET, LBRACE_RBRACE_SET, TOP_LEVEL_DECLARATION_FIRST]);
const IDENTIFIER_EQ_COLON_SEMICOLON_SET: TokenSet = TokenSet::create(&[IDENTIFIER, EQ, COLON, SEMICOLON]);
const COMMA_RPAR_RBRACKET_COLON_EQ_SET: TokenSet = TokenSet::create(&[COMMA, RPAR, RBRACKET, COLON, EQ]);
const ACCESSOR_FIRST_OR_PROPERTY_END: TokenSet = TokenSet::or_set(&[
    MODIFIER_KEYWORDS,
    TokenSet::create(&[AT, GET_KEYWORD, SET_KEYWORD, FIELD_KEYWORD, EOL_OR_SEMICOLON, RBRACE]),
]);
const RPAR_IDENTIFIER_COLON_LBRACE_EQ_SET: TokenSet = TokenSet::create(&[RPAR, IDENTIFIER, COLON, LBRACE, EQ]);
const COMMA_COLON_RPAR_SET: TokenSet = TokenSet::create(&[COMMA, COLON, RPAR]);
const RPAR_COLON_LBRACE_EQ_SET: TokenSet = TokenSet::create(&[RPAR, COLON, LBRACE, EQ]);
const LBRACKET_LBRACE_RBRACE_LPAR_SET: TokenSet = TokenSet::create(&[LBRACKET, LBRACE, RBRACE, LPAR]);
const FUNCTION_NAME_FOLLOW_SET: TokenSet = TokenSet::create(&[LT, LPAR, RPAR, COLON, EQ]);
const FUNCTION_NAME_RECOVERY_SET: TokenSet = TokenSet::or_set(&[
    TokenSet::create(&[LT, LPAR, RPAR, COLON, EQ]),
    LBRACE_RBRACE_SET,
    TOP_LEVEL_DECLARATION_FIRST,
]);
const VALUE_PARAMETERS_FOLLOW_SET: TokenSet = TokenSet::create(&[EQ, LBRACE, RBRACE, SEMICOLON, RPAR]);
const CONTEXT_PARAMETERS_FOLLOW_SET: TokenSet =
    TokenSet::create(&[CLASS_KEYWORD, OBJECT_KEYWORD, FUN_KEYWORD, VAL_KEYWORD, VAR_KEYWORD]);
const LPAR_VALUE_PARAMETERS_FOLLOW_SET: TokenSet =
    TokenSet::or_set(&[TokenSet::create(&[LPAR]), VALUE_PARAMETERS_FOLLOW_SET]);
const LPAR_LBRACE_COLON_CONSTRUCTOR_KEYWORD_SET: TokenSet = TokenSet::create(&[LPAR, LBRACE, COLON, CONSTRUCTOR_KEYWORD]);
#[allow(non_upper_case_globals)]
const definitelyOutOfReceiverSet: TokenSet =
    TokenSet::or_set(&[TokenSet::create(&[EQ, COLON, LBRACE, RBRACE, BY_KEYWORD]), TOP_LEVEL_DECLARATION_FIRST]);
const EOL_OR_SEMICOLON_RBRACE_SET: TokenSet = TokenSet::create(&[EOL_OR_SEMICOLON, RBRACE]);
const CLASS_INTERFACE_SET: TokenSet = TokenSet::create(&[CLASS_KEYWORD, INTERFACE_KEYWORD]);

// `lastDotAfterReceiver*Pattern` fields are built at their use site in `functions.rs`.

impl Parser {
    /*
     * [start] kotlinFile
     *   : preamble toplevelObject* [eof]
     *   ;
     */
    pub(crate) fn parse_file(&mut self) {
        let file_marker = self.mark();

        self.parse_preamble();

        while !self.eof() {
            self.parse_top_level_declaration();
        }

        self.check_unclosed_block_comment();
        file_marker.done(self, KT_FILE);
    }

    fn check_unclosed_block_comment(&mut self) {
        if BLOCK_DOC_COMMENT_SET.contains(self.my_builder.raw_lookup(-1)) {
            let start_offset = self.my_builder.raw_token_type_start(-1) as usize;
            let end_offset = self.my_builder.raw_token_type_start(0) as usize;
            let token_chars = &self.my_builder.get_original_text()[start_offset..end_offset];
            if !(token_chars.len() > 2 && token_chars.ends_with("*/")) {
                let marker = self.my_builder.mark();
                marker.error(self, "Unclosed comment");
                marker.set_custom_edge_token_binders(self, Some(GREEDY_RIGHT_BINDER), None);
            }
        }
    }

    pub(crate) fn parse_type_code_fragment(&mut self) {
        let marker = self.mark();
        self.parse_type_ref();

        self.check_for_unexpected_symbols();

        marker.done(self, TYPE_CODE_FRAGMENT);
    }

    pub(crate) fn parse_expression_code_fragment(&mut self) {
        let marker = self.mark();
        self.parse_expression();

        self.check_for_unexpected_symbols();

        marker.done(self, EXPRESSION_CODE_FRAGMENT);
    }

    pub(crate) fn parse_block_code_fragment(&mut self) {
        let marker = self.mark();
        let block_marker = self.mark();

        if self.at(PACKAGE_KEYWORD) || self.at(IMPORT_KEYWORD) {
            let err = self.mark();
            self.parse_preamble();
            err.error(self, "Package directive and imports are forbidden in code fragments");
        }

        self.parse_statements();

        self.check_for_unexpected_symbols();

        block_marker.done(self, BLOCK);
        marker.done(self, BLOCK_CODE_FRAGMENT);
    }

    pub(crate) fn parse_lambda_expression(&mut self) {
        self.parse_function_literal_2(/* preferBlock = */ false, /* collapse = */ false);
    }

    pub(crate) fn parse_block_expression(&mut self) {
        self.parse_block_1(/* collapse = */ false);
    }

    pub(crate) fn parse_script(&mut self) {
        let file_marker = self.mark();

        self.parse_preamble();

        let script_marker = self.mark();

        let block_marker = self.mark();

        self.parse_statements_1(/* isScriptTopLevel = */ true);

        self.check_for_unexpected_symbols();

        block_marker.done(self, BLOCK);
        block_marker.set_custom_edge_token_binders(self, Some(EdgeBinder::PrecedingAll), Some(EdgeBinder::TrailingAll));

        script_marker.done(self, SCRIPT);
        script_marker.set_custom_edge_token_binders(self, Some(EdgeBinder::PrecedingAll), Some(EdgeBinder::TrailingAll));

        file_marker.done(self, KT_FILE);
    }

    fn check_for_unexpected_symbols(&mut self) {
        while !self.eof() {
            self.error_and_advance("Unexpected symbol");
        }
    }
}
