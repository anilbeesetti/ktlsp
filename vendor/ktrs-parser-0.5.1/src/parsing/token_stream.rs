//! Ports of `TokenStreamPredicate`, `AbstractTokenStreamPredicate`, `TokenStreamPattern`,
//! `AbstractTokenStreamPattern`, `FirstBefore`, `LastBefore`, and `AbstractKotlinParsing`'s
//! `At`/`AtSet`/`matchTokenStreamPredicate`.
//! Predicates query the parser (`at`/`at_set` may remap tokens), so they take `&mut Parser`.

use ktrs_syntax::SyntaxKind::{self, *};

use super::Parser;
use crate::token_set::TokenSet;

impl Parser {
    /// `AbstractKotlinParsing.matchTokenStreamPredicate`.
    pub(crate) fn match_token_stream_predicate(&mut self, pattern: &mut dyn TokenStreamPattern) -> i32 {
        let current_position = self.mark();
        let mut opens: Vec<SyntaxKind> = Vec::new();
        let mut open_angle_brackets = 0;
        let mut open_braces = 0;
        let mut open_parentheses = 0;
        let mut open_brackets = 0;
        while !self.eof() {
            let offset = self.my_builder.get_current_offset();
            let top_level = pattern.is_top_level(open_angle_brackets, open_brackets, open_braces, open_parentheses);
            if pattern.process_token(self, offset, top_level) {
                break;
            }
            match self.get_token_id() {
                Some(LPAR) => {
                    open_parentheses += 1;
                    opens.push(LPAR);
                }
                Some(LT) => {
                    open_angle_brackets += 1;
                    opens.push(LT);
                }
                Some(LBRACE) => {
                    open_braces += 1;
                    opens.push(LBRACE);
                }
                Some(LBRACKET) => {
                    open_brackets += 1;
                    opens.push(LBRACKET);
                }
                Some(RPAR) => {
                    open_parentheses -= 1;
                    // Upstream's `break` here only leaves the switch, so the result is unused.
                    if opens.pop() != Some(LPAR) {
                        pattern.handle_unmatched_closing(RPAR);
                    }
                }
                Some(GT) => open_angle_brackets -= 1,
                Some(RBRACE) => open_braces -= 1,
                Some(RBRACKET) => open_brackets -= 1,
                _ => {}
            }

            self.advance(); // skip token
        }

        current_position.rollback_to(self);

        pattern.result()
    }
}

pub trait TokenStreamPredicate {
    fn matching(&self, p: &mut Parser, top_level: bool) -> bool;

    /// `AbstractTokenStreamPredicate.or`.
    fn or<O: TokenStreamPredicate>(self, other: O) -> Or<Self, O>
    where
        Self: Sized,
    {
        Or(self, other)
    }
}

/// Anonymous `AbstractTokenStreamPredicate` subclasses.
impl<F: Fn(&mut Parser, bool) -> bool> TokenStreamPredicate for F {
    fn matching(&self, p: &mut Parser, top_level: bool) -> bool {
        self(p, top_level)
    }
}

pub struct Or<A, B>(A, B);

impl<A: TokenStreamPredicate, B: TokenStreamPredicate> TokenStreamPredicate for Or<A, B> {
    fn matching(&self, p: &mut Parser, top_level: bool) -> bool {
        if self.0.matching(p, top_level) {
            return true;
        }
        self.1.matching(p, top_level)
    }
}

/// `TokenStreamPattern`, with `AbstractTokenStreamPattern`'s defaults.
pub trait TokenStreamPattern {
    /// Called on each token; `true` stops.
    fn process_token(&mut self, p: &mut Parser, offset: i32, top_level: bool) -> bool;

    /// The offset where the pattern matched, -1 if none.
    fn result(&self) -> i32;

    fn is_top_level(&self, open_angle_brackets: i32, open_brackets: i32, open_braces: i32, open_parentheses: i32) -> bool {
        open_braces == 0 && open_brackets == 0 && open_parentheses == 0 && open_angle_brackets == 0
    }

    /// Called on unmatched `)`; `true` asks to stop (ignored by the caller upstream).
    fn handle_unmatched_closing(&mut self, _token: SyntaxKind) -> bool {
        false
    }
}

/// The state of `AbstractTokenStreamPattern`.
#[derive(Debug)]
pub struct AbstractTokenStreamPattern {
    pub last_occurrence: i32,
}

impl Default for AbstractTokenStreamPattern {
    fn default() -> Self {
        AbstractTokenStreamPattern { last_occurrence: -1 }
    }
}

impl AbstractTokenStreamPattern {
    pub fn fail(&mut self) {
        self.last_occurrence = -1;
    }

    pub fn reset(&mut self) {
        self.last_occurrence = -1;
    }
}

pub struct FirstBefore<L, S> {
    base: AbstractTokenStreamPattern,
    look_for: L,
    stop_at: S,
}

impl<L: TokenStreamPredicate, S: TokenStreamPredicate> FirstBefore<L, S> {
    pub fn new(look_for: L, stop_at: S) -> Self {
        FirstBefore { base: AbstractTokenStreamPattern::default(), look_for, stop_at }
    }

    pub fn reset(&mut self) {
        self.base.reset();
    }
}

impl<L: TokenStreamPredicate, S: TokenStreamPredicate> TokenStreamPattern for FirstBefore<L, S> {
    fn process_token(&mut self, p: &mut Parser, offset: i32, top_level: bool) -> bool {
        if self.look_for.matching(p, top_level) {
            self.base.last_occurrence = offset;
            return true;
        }
        if self.stop_at.matching(p, top_level) {
            return true;
        }
        false
    }

    fn result(&self) -> i32 {
        self.base.last_occurrence
    }
}

pub struct LastBefore<L, S> {
    base: AbstractTokenStreamPattern,
    dont_stop_right_after_occurrence: bool,
    look_for: L,
    stop_at: S,
    previous_look_for_result: bool,
}

impl<L: TokenStreamPredicate, S: TokenStreamPredicate> LastBefore<L, S> {
    /// The private 3-arg constructor.
    fn new_3(look_for: L, stop_at: S, dont_stop_right_after_occurrence: bool) -> Self {
        LastBefore {
            base: AbstractTokenStreamPattern::default(),
            dont_stop_right_after_occurrence,
            look_for,
            stop_at,
            previous_look_for_result: false,
        }
    }

    pub fn new(look_for: L, stop_at: S) -> Self {
        Self::new_3(look_for, stop_at, false)
    }

    pub fn reset(&mut self) {
        self.base.reset();
        self.previous_look_for_result = false;
    }
}

impl<L: TokenStreamPredicate, S: TokenStreamPredicate> TokenStreamPattern for LastBefore<L, S> {
    fn process_token(&mut self, p: &mut Parser, offset: i32, top_level: bool) -> bool {
        let look_for_result = self.look_for.matching(p, top_level);
        if look_for_result {
            self.base.last_occurrence = offset;
        }
        if self.stop_at.matching(p, top_level)
            && top_level
            && (!self.dont_stop_right_after_occurrence || !self.previous_look_for_result)
        {
            return true;
        }
        self.previous_look_for_result = look_for_result;
        false
    }

    fn result(&self) -> i32 {
        self.base.last_occurrence
    }
}

/// `AbstractKotlinParsing.At`.
pub struct At {
    look_for: SyntaxKind,
    top_level_only: bool,
}

impl At {
    pub fn new(look_for: SyntaxKind) -> At {
        At::new_2(look_for, true)
    }

    pub fn new_2(look_for: SyntaxKind, top_level_only: bool) -> At {
        At { look_for, top_level_only }
    }
}

impl TokenStreamPredicate for At {
    fn matching(&self, p: &mut Parser, top_level: bool) -> bool {
        (top_level || !self.top_level_only) && p.at(self.look_for)
    }
}

/// `AbstractKotlinParsing.AtSet`.
pub struct AtSet {
    look_for: TokenSet,
    top_level_only: TokenSet,
}

impl AtSet {
    pub fn new(look_for: TokenSet) -> AtSet {
        AtSet::new_2(look_for, look_for)
    }

    pub fn new_2(look_for: TokenSet, top_level_only: TokenSet) -> AtSet {
        AtSet { look_for, top_level_only }
    }
}

impl TokenStreamPredicate for AtSet {
    fn matching(&self, p: &mut Parser, top_level: bool) -> bool {
        (top_level || !p.at_set(self.top_level_only)) && p.at_set(self.look_for)
    }
}
