//! [`ChameleonCache`]: expanded lazy nodes (`BLOCK`, `LAMBDA_EXPRESSION`, `DOC_COMMENT`, ...) keyed
//! by kind and text, for re-parsing text that is mostly unchanged (the formatter parses each file
//! about three times, and most bodies survive import sorting and trailing-comma edits verbatim).
//!
//! Exact: a chameleon's subtree depends only on its kind and text (see `LazyLeaf`), and each entry
//! is a standalone [`Tree`] spliced in with rebased indices. Only error-free subtrees are kept, so a
//! hit never has to replay error messages.

use std::collections::HashMap;

use ktrs_syntax::{SyntaxKind, Tree};

#[derive(Default)]
pub struct ChameleonCache {
    /// The entry's own text is its key; the hash only picks the bucket.
    subtrees: HashMap<u64, Vec<Tree>>,
}

impl ChameleonCache {
    pub fn new() -> ChameleonCache {
        ChameleonCache::default()
    }

    pub(super) fn get(&self, kind: SyntaxKind, text: &str) -> Option<&Tree> {
        let bucket = self.subtrees.get(&hash(kind, text))?;
        bucket.iter().find(|t| t.kind(Tree::ROOT) == kind && t.text() == text)
    }

    pub(super) fn insert(&mut self, subtree: Tree) {
        let key = hash(subtree.kind(Tree::ROOT), subtree.text());
        self.subtrees.entry(key).or_default().push(subtree);
    }
}

/// FxHash-style multiply-rotate over 8-byte words.
fn hash(kind: SyntaxKind, text: &str) -> u64 {
    let mut h = kind as u64;
    let mut mix = |word: u64| h = (h.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    let (words, rest) = text.as_bytes().as_chunks::<8>();
    for &word in words {
        mix(u64::from_le_bytes(word));
    }
    let mut tail = [0u8; 8];
    tail[..rest.len()].copy_from_slice(rest);
    mix(u64::from_le_bytes(tail) ^ (text.len() as u64) << 56);
    h ^ h >> 29
}
