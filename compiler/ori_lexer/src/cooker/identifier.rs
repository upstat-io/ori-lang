//! Direct-mapped identifier cache for the token cooker.
//!
//! Caches `cook_ident()` results — keyword lookup + interning — for
//! repeated identifiers. Fixed 256-entry array indexed by hash; ~10x
//! cheaper than `FxHashMap` per lookup because it avoids probing,
//! bookkeeping, and dynamic resizing.

use ori_ir::TokenKind;

/// Number of slots in the direct-mapped identifier cache.
pub(super) const IDENT_CACHE_SIZE: usize = 256;
const IDENT_CACHE_MASK: usize = IDENT_CACHE_SIZE - 1;

/// Direct-mapped identifier cache: fixed 256-entry array indexed by hash.
///
/// On hit (text matches), returns the cached `TokenKind` — bypassing both
/// keyword lookup and the string interner. On miss or collision, the slot
/// is overwritten with the new entry (no probing, no chaining).
///
/// This is ~10x cheaper than `FxHashMap` per lookup because it avoids
/// `HashMap` bookkeeping, probing, and dynamic resizing. The tradeoff is
/// that collisions silently evict entries, but the hot identifiers (`int`,
/// `let`, `x`, etc.) that appear thousands of times naturally dominate
/// their slots.
pub(super) struct IdentCache<'src> {
    slots: [Option<(&'src str, TokenKind)>; IDENT_CACHE_SIZE],
}

impl<'src> IdentCache<'src> {
    pub(super) fn new() -> Self {
        Self {
            slots: [(); IDENT_CACHE_SIZE].map(|()| None),
        }
    }

    /// Look up a cached identifier result.
    #[expect(
        clippy::inline_always,
        reason = "hot inner loop: 3-instruction cache probe"
    )]
    #[inline(always)]
    pub(super) fn get(&self, text: &str) -> Option<&TokenKind> {
        let slot = Self::hash(text);
        if let Some((cached, kind)) = &self.slots[slot] {
            if *cached == text {
                return Some(kind);
            }
        }
        None
    }

    /// Insert or overwrite a cache entry.
    #[expect(
        clippy::inline_always,
        reason = "hot inner loop: 2-instruction cache store"
    )]
    #[inline(always)]
    pub(super) fn insert(&mut self, text: &'src str, kind: TokenKind) {
        let slot = Self::hash(text);
        self.slots[slot] = Some((text, kind));
    }

    /// Simple hash: first byte * 31 ^ last byte ^ length.
    /// Distributes identifiers well because last byte varies even for
    /// prefixed names (func0, func1, ...) and length separates keywords.
    #[expect(clippy::inline_always, reason = "hot inner loop: 4-instruction hash")]
    #[inline(always)]
    fn hash(text: &str) -> usize {
        let bytes = text.as_bytes();
        debug_assert!(!bytes.is_empty(), "identifiers are never empty");
        let len = bytes.len();
        let first = bytes[0] as usize;
        let last = bytes[len - 1] as usize;
        (first.wrapping_mul(31) ^ last ^ len) & IDENT_CACHE_MASK
    }
}
