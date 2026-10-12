//! Port of `KotlinParsing.java` lines 1972-2076 (blocks, delegation specifiers).

use ktrs_syntax::SyntaxKind::*;

use crate::parsing::Parser;

impl Parser {
    /*
     * block
     *   : "{" (expressions)* "}"
     *   ;
     */
    pub(crate) fn parse_block(&mut self) {
        self.parse_block_1(/*collapse*/ true);
    }

    pub(super) fn parse_block_1(&mut self, collapse: bool) {
        let lazy_block = self.mark();

        self.my_builder.enable_newlines();

        let has_opening_brace = self.expect_2(LBRACE, "Expecting '{' to open a block");
        let can_collapse = collapse && has_opening_brace && self.is_lazy;

        if can_collapse {
            self.advance_balanced_block();
        } else {
            self.parse_statements();
            self.expect_2(RBRACE, "Expecting '}'");
        }

        self.my_builder.restore_newlines_state();

        if can_collapse {
            lazy_block.collapse(self, BLOCK);
        } else {
            lazy_block.done(self, BLOCK);
        }
    }

    pub(crate) fn advance_balanced_block(&mut self) {
        let mut brace_count = 1;
        while !self.eof() {
            // `_at(LBRACE)` / `_at(RBRACE)` on one `tt()`: this loop runs once per nesting level
            // over every token of a lazy block.
            let token = self.tt();
            if token == Some(LBRACE) {
                brace_count += 1;
            } else if token == Some(RBRACE) {
                brace_count -= 1;
            }

            self.advance();

            if brace_count == 0 {
                break;
            }
        }
    }

    /*
     * delegationSpecifier{","}
     */
    pub(super) fn parse_delegation_specifier_list(&mut self) {
        let list = self.mark();

        loop {
            if self.at(COMMA) {
                self.error_and_advance("Expecting a delegation specifier");
                continue;
            }
            self.parse_delegation_specifier();
            if !self.at(COMMA) {
                break;
            }
            self.advance(); // COMMA
        }

        list.done(self, SUPER_TYPE_LIST);
    }

    /*
     * delegationSpecifier
     *   : constructorInvocation // type and constructor arguments
     *   : userType
     *   : explicitDelegation
     *   ;
     *
     * explicitDelegation
     *   : userType "by" element
     *   ;
     */
    fn parse_delegation_specifier(&mut self) {
        let delegator = self.mark();
        let reference = self.mark();
        self.parse_type_ref();

        if self.at(BY_KEYWORD) {
            reference.drop(self);
            self.advance(); // BY_KEYWORD
            self.create_for_by_clause(self.is_lazy, |p| p.parse_expression());
            delegator.done(self, DELEGATED_SUPER_TYPE_ENTRY);
        } else if self.at(LPAR) {
            reference.done(self, CONSTRUCTOR_CALLEE);
            self.parse_value_argument_list();
            delegator.done(self, SUPER_TYPE_CALL_ENTRY);
        } else {
            reference.drop(self);
            delegator.done(self, SUPER_TYPE_ENTRY);
        }
    }
}
