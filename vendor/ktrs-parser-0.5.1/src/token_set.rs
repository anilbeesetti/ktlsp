//! IntelliJ `TokenSet`: an immutable, const-constructible bit set of [`SyntaxKind`]s.

use ktrs_syntax::SyntaxKind;

const WORDS: usize = 8;
const _: () = assert!((SyntaxKind::DUMMY_HOLDER as usize) < WORDS * 64, "grow TokenSet::WORDS");

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenSet([u64; WORDS]);

impl TokenSet {
    pub const EMPTY: TokenSet = TokenSet([0; WORDS]);

    /// `TokenSet.create(...)`.
    pub const fn create(kinds: &[SyntaxKind]) -> TokenSet {
        let mut bits = [0u64; WORDS];
        let mut i = 0;
        while i < kinds.len() {
            let k = kinds[i] as usize;
            bits[k / 64] |= 1 << (k % 64);
            i += 1;
        }
        TokenSet(bits)
    }

    /// `TokenSet.orSet(...)`.
    pub const fn or_set(sets: &[TokenSet]) -> TokenSet {
        let mut bits = [0u64; WORDS];
        let mut s = 0;
        while s < sets.len() {
            let mut w = 0;
            while w < WORDS {
                bits[w] |= sets[s].0[w];
                w += 1;
            }
            s += 1;
        }
        TokenSet(bits)
    }

    /// `TokenSet.andSet(a, b)`.
    pub const fn and_set(a: TokenSet, b: TokenSet) -> TokenSet {
        let mut bits = [0u64; WORDS];
        let mut w = 0;
        while w < WORDS {
            bits[w] = a.0[w] & b.0[w];
            w += 1;
        }
        TokenSet(bits)
    }

    /// `TokenSet.andNot(a, b)`.
    pub const fn and_not(a: TokenSet, b: TokenSet) -> TokenSet {
        let mut bits = [0u64; WORDS];
        let mut w = 0;
        while w < WORDS {
            bits[w] = a.0[w] & !b.0[w];
            w += 1;
        }
        TokenSet(bits)
    }

    /// Accepts `SyntaxKind` or `Option<SyntaxKind>`; like Java, `contains(null)` is false.
    pub fn contains(self, kind: impl Into<Option<SyntaxKind>>) -> bool {
        match kind.into() {
            Some(k) => self.0[k as usize / 64] & (1 << (k as usize % 64)) != 0,
            None => false,
        }
    }

    pub fn intersects(self, other: TokenSet) -> bool {
        self.0.iter().zip(other.0).any(|(a, b)| a & b != 0)
    }

    pub fn types(self) -> impl Iterator<Item = SyntaxKind> {
        (0..WORDS * 64)
            .filter(move |&i| self.0[i / 64] & (1 << (i % 64)) != 0)
            .map(|i| SyntaxKind::from_raw(i as u16))
    }
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.types()).finish()
    }
}
