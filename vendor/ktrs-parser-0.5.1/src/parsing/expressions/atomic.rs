//! `KotlinExpressionParsing.java` lines 513-646: `parseAtomicExpression`.

use ktrs_syntax::SyntaxKind::*;

use crate::parsing::Parser;
use crate::parsing::declarations::EXPRESSION_FOLLOW;
use crate::token_set::TokenSet;

impl Parser {
    /*
     * atomicExpression
     *   : "this" label?
     *   : "super" ("<" type ">")? label?
     *   : objectLiteral
     *   : jump
     *   : if
     *   : when
     *   : try
     *   : loop
     *   : literalConstant
     *   : functionLiteral
     *   : declaration
     *   : SimpleName
     *   : collectionLiteral
     *   ;
     */
    pub(crate) fn parse_atomic_expression(&mut self) -> bool {
        let mut ok = true;

        match self.get_token_id() {
            Some(LPAR) => self.parse_parenthesized_expression(),
            Some(LBRACKET) => self.parse_collection_literal_expression(),
            Some(THIS_KEYWORD) => self.parse_this_expression(),
            Some(SUPER_KEYWORD) => self.parse_super_expression(),
            Some(OBJECT_KEYWORD) => self.parse_object_literal(),
            Some(THROW_KEYWORD) => self.parse_throw(),
            Some(RETURN_KEYWORD) => self.parse_return(),
            Some(CONTINUE_KEYWORD) => self.parse_jump(CONTINUE),
            Some(BREAK_KEYWORD) => self.parse_jump(BREAK),
            Some(IF_KEYWORD) => self.parse_if(),
            Some(WHEN_KEYWORD) => self.parse_when(),
            Some(TRY_KEYWORD) => self.parse_try(),
            Some(FOR_KEYWORD) => self.parse_for(),
            Some(WHILE_KEYWORD) => self.parse_while(),
            Some(DO_KEYWORD) => self.parse_do_while(),
            Some(IDENTIFIER) => 'identifier: {
                // Try to parse anonymous function with context parameters
                if self.at(CONTEXT_KEYWORD) && self.lookahead(1) == Some(LPAR) {
                    if self.parse_local_declaration(true, false) {
                        break 'identifier;
                    } else {
                        self.at(IDENTIFIER);
                    }
                }

                self.parse_simple_name_expression();
            }
            Some(LBRACE) => self.parse_function_literal(),
            Some(INTERPOLATION_PREFIX | OPEN_QUOTE) => self.parse_string_template(),
            /*
             * literalConstant
             *   : "true" | "false"
             *   : stringTemplate
             *   : NoEscapeString
             *   : IntegerLiteral
             *   : CharacterLiteral
             *   : FloatLiteral
             *   : "null"
             *   ;
             */
            Some(TRUE_KEYWORD | FALSE_KEYWORD) => self.parse_one_token_expression(BOOLEAN_CONSTANT),
            Some(INTEGER_LITERAL) => self.parse_one_token_expression(INTEGER_CONSTANT),
            Some(CHARACTER_LITERAL) => self.parse_one_token_expression(CHARACTER_CONSTANT),
            Some(FLOAT_LITERAL) => self.parse_one_token_expression(FLOAT_CONSTANT),
            Some(NULL_KEYWORD) => self.parse_one_token_expression(NULL),
            Some(CLASS_KEYWORD | INTERFACE_KEYWORD | FUN_KEYWORD | VAL_KEYWORD | VAR_KEYWORD | TYPE_ALIAS_KEYWORD) => {
                let rollback_if_definitely_not_expression = self.my_builder.newline_before_current_token();
                if !self.parse_local_declaration(rollback_if_definitely_not_expression, false) {
                    ok = false;
                }
                // declaration was parsed, do nothing
            }
            _ => ok = false,
        }

        if !ok {
            // TODO: better recovery if FIRST(element) did not match
            self.error_with_recovery(
                "Expecting an element",
                Some(TokenSet::or_set(&[EXPRESSION_FOLLOW, TokenSet::create(&[LONG_TEMPLATE_ENTRY_END])])),
            );
        }

        ok
    }
}
