//! Port of `KotlinExpressionParsing.java` (fields and `parseExpression`: lines 1-153).
//! The remaining methods follow in upstream order across the submodules.

mod atomic;
mod binary_operation_precedence;
mod calls;
mod collections;
mod control;
mod lambdas;
mod loops;
mod operations;
mod primary;
mod statements;
mod strings;
mod when;

use ktrs_syntax::SyntaxKind::*;

use super::Parser;
use super::declarations::{EXPRESSION_FIRST, PARAMETER_NAME_RECOVERY_SET};
use crate::kt_tokens::{KEYWORDS, MODIFIER_KEYWORDS};
use crate::token_set::TokenSet;
pub(crate) use binary_operation_precedence::BinaryOperationPrecedence;

const WHEN_CONDITION_RECOVERY_SET: TokenSet =
    TokenSet::create(&[RBRACE, IN_KEYWORD, NOT_IN, IS_KEYWORD, NOT_IS, ELSE_KEYWORD]);
const WHEN_CONDITION_RECOVERY_SET_WITH_ARROW: TokenSet =
    TokenSet::create(&[RBRACE, IN_KEYWORD, NOT_IN, IS_KEYWORD, NOT_IS, ELSE_KEYWORD, ARROW, DOT]);
// `KEYWORD_TEXTS.get(text)` is `keyword_texts_get` in strings.rs.

const TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA: TokenSet = TokenSet::create(&[ARROW, COMMA, COLON]);
const TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA_RECOVERY: TokenSet =
    TokenSet::or_set(&[TOKEN_SET_TO_FOLLOW_AFTER_DESTRUCTURING_DECLARATION_IN_LAMBDA, PARAMETER_NAME_RECOVERY_SET]);
const EQ_RPAR_SET: TokenSet = TokenSet::create(&[EQ, RPAR]);
const ARROW_SET: TokenSet = TokenSet::create(&[ARROW]);
const ARROW_COMMA_SET: TokenSet = TokenSet::create(&[ARROW, COMMA]);
const IN_KEYWORD_R_PAR_COLON_SET: TokenSet = TokenSet::create(&[IN_KEYWORD, RPAR, COLON]);
const IN_KEYWORD_L_BRACE_SET: TokenSet = TokenSet::create(&[IN_KEYWORD, LBRACE]);
const IN_KEYWORD_L_BRACE_RECOVERY_SET: TokenSet = TokenSet::or_set(&[IN_KEYWORD_L_BRACE_SET, PARAMETER_NAME_RECOVERY_SET]);
const COLON_IN_KEYWORD_SET: TokenSet = TokenSet::create(&[COLON, IN_KEYWORD]);
const L_PAR_L_BRACE_R_PAR_SET: TokenSet = TokenSet::create(&[LPAR, LBRACE, RPAR]);
const IN_KEYWORD_SET: TokenSet = TokenSet::create(&[IN_KEYWORD]);
const TRY_CATCH_RECOVERY_TOKEN_SET: TokenSet = TokenSet::create(&[LBRACE, RBRACE, FINALLY_KEYWORD, CATCH_KEYWORD]);

const TYPE_ARGUMENT_LIST_STOPPERS: TokenSet = TokenSet::create(&[
    INTEGER_LITERAL, FLOAT_LITERAL, CHARACTER_LITERAL, INTERPOLATION_PREFIX, OPEN_QUOTE,
    PACKAGE_KEYWORD, AS_KEYWORD, TYPE_ALIAS_KEYWORD, INTERFACE_KEYWORD, CLASS_KEYWORD, THIS_KEYWORD, VAL_KEYWORD, VAR_KEYWORD,
    FUN_KEYWORD, FOR_KEYWORD, NULL_KEYWORD,
    TRUE_KEYWORD, FALSE_KEYWORD, IS_KEYWORD, THROW_KEYWORD, RETURN_KEYWORD, BREAK_KEYWORD,
    CONTINUE_KEYWORD, OBJECT_KEYWORD, IF_KEYWORD, TRY_KEYWORD, ELSE_KEYWORD, WHILE_KEYWORD, DO_KEYWORD,
    WHEN_KEYWORD, RBRACKET, RBRACE, RPAR, PLUSPLUS, MINUSMINUS, EXCLEXCL,
    PLUS, MINUS, EXCL, DIV, PERC, LTEQ,
    EQEQEQ, EXCLEQEQEQ, EQEQ, EXCLEQ, ANDAND, OROR, SAFE_ACCESS, ELVIS,
    SEMICOLON, RANGE, RANGE_UNTIL, EQ, MULTEQ, DIVEQ, PERCEQ, PLUSEQ, MINUSEQ, NOT_IN, NOT_IS,
    COLONCOLON,
    COLON,
]);

pub(crate) const STATEMENT_FIRST: TokenSet = TokenSet::or_set(&[
    EXPRESSION_FIRST,
    TokenSet::create(&[
        // declaration
        FUN_KEYWORD,
        VAL_KEYWORD, VAR_KEYWORD,
        INTERFACE_KEYWORD,
        CLASS_KEYWORD,
        TYPE_ALIAS_KEYWORD,
    ]),
    MODIFIER_KEYWORDS,
]);

const STATEMENT_NEW_LINE_QUICK_RECOVERY_SET: TokenSet = TokenSet::or_set(&[
    TokenSet::and_set(STATEMENT_FIRST, TokenSet::and_not(KEYWORDS, TokenSet::create(&[IN_KEYWORD]))),
    TokenSet::create(&[EOL_OR_SEMICOLON]),
]);

const MIN_BINARY_OPERATION_PRECEDENCE: BinaryOperationPrecedence = BinaryOperationPrecedence::ENTRIES[0];

const MAX_BINARY_OPERATION_PRECEDENCE: BinaryOperationPrecedence =
    BinaryOperationPrecedence::ENTRIES[BinaryOperationPrecedence::ENTRIES.len() - 1];

const ALLOW_NEWLINE_OPERATIONS: TokenSet = TokenSet::create(&[
    DOT, SAFE_ACCESS,
    COLON, AS_KEYWORD, AS_SAFE,
    ELVIS,
    // Can't allow `is` and `!is` because of when entry conditions: IS_KEYWORD, NOT_IS,
    ANDAND,
    OROR,
]);

/// `KtTokenSets.PREFIX_OPERATIONS` (psi-api).
const PREFIX_OPERATIONS: TokenSet = TokenSet::create(&[MINUS, PLUS, MINUSMINUS, PLUSPLUS, EXCL]);
/// `KtTokenSets.POSTFIX_OPERATIONS` (psi-api).
const POSTFIX_OPERATIONS: TokenSet = TokenSet::create(&[PLUSPLUS, MINUSMINUS, EXCLEXCL, DOT, SAFE_ACCESS]);

impl Parser {
    /*
     * element
     *   : annotations element
     *   : "(" element ")" // see tupleLiteral
     *   : literalConstant
     *   : functionLiteral
     *   : tupleLiteral
     *   : "null"
     *   : "this" ("<" type ">")?
     *   : expressionWithPrecedences
     *   : if
     *   : try
     *   : "typeof" "(" element ")"
     *   : "new" constructorInvocation
     *   : objectLiteral
     *   : declaration
     *   : jump
     *   : loop
     *   // block is syntactically equivalent to a functionLiteral with no parameters
     *   ;
     */
    pub(crate) fn parse_expression(&mut self) {
        self.parse_expression_1("Expecting an expression");
    }

    pub(crate) fn parse_expression_1(&mut self, message_if_not_expression_first: &str) {
        if !self.at_set(EXPRESSION_FIRST) {
            self.error(message_if_not_expression_first);
            return;
        }

        self.parse_binary_expression(Some(MAX_BINARY_OPERATION_PRECEDENCE));
    }
}
