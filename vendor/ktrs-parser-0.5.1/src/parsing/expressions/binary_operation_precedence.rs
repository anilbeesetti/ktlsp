//! Port of psi-api `org/jetbrains/kotlin/lang/BinaryOperationPrecedence.kt` (the parser's half).

use ktrs_syntax::SyntaxKind::{self, *};

use crate::kt_tokens::SOFT_KEYWORDS;

/// Declaration order = ordinal; lower ordinal = higher priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BinaryOperationPrecedence {
    As,
    Multiplicative,
    Additive,
    Range,
    Infix,
    Elvis,
    InOrIs,
    Comparison,
    Equality,
    Conjunction,
    Disjunction,
    Assignment,
}

use BinaryOperationPrecedence as P;

impl BinaryOperationPrecedence {
    pub(crate) const ENTRIES: [BinaryOperationPrecedence; 12] = [
        P::As, P::Multiplicative, P::Additive, P::Range, P::Infix, P::Elvis, P::InOrIs, P::Comparison, P::Equality,
        P::Conjunction, P::Disjunction, P::Assignment,
    ];

    pub(crate) fn ordinal(self) -> usize {
        self as usize
    }

    pub(crate) fn get_higher_priority(self) -> Option<BinaryOperationPrecedence> {
        self.ordinal().checked_sub(1).map(|i| Self::ENTRIES[i])
    }

    pub(crate) fn tokens(self) -> &'static [SyntaxKind] {
        match self {
            P::As => &[AS_KEYWORD, AS_SAFE],
            P::Multiplicative => &[MUL, DIV, PERC],
            P::Additive => &[PLUS, MINUS],
            P::Range => &[RANGE, RANGE_UNTIL],
            P::Infix => &[IDENTIFIER],
            P::Elvis => &[ELVIS],
            P::InOrIs => &[IN_KEYWORD, NOT_IN, IS_KEYWORD, NOT_IS],
            P::Comparison => &[LT, GT, LTEQ, GTEQ],
            P::Equality => &[EQEQ, EXCLEQ, EQEQEQ, EXCLEQEQEQ],
            P::Conjunction => &[ANDAND],
            P::Disjunction => &[OROR],
            P::Assignment => &[EQ, PLUSEQ, MINUSEQ, MULTEQ, DIVEQ, PERCEQ],
        }
    }

    /// `TOKEN_TO_BINARY_PRECEDENCE_MAP_WITH_SOFT_IDENTIFIERS.get(token)`: soft keywords map like
    /// `IDENTIFIER` (INFIX).
    pub(crate) fn token_to_binary_precedence_map_with_soft_identifiers(token: SyntaxKind) -> Option<BinaryOperationPrecedence> {
        if SOFT_KEYWORDS.contains(token) {
            return Some(P::Infix);
        }
        Self::ENTRIES.into_iter().find(|p| p.tokens().contains(&token))
    }
}
