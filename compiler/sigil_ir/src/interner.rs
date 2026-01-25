//! Sharded string interner for efficient identifier storage.
//!
//! Ported from V2 with all Salsa-required traits.

// Arc is needed here for SharedInterner - the interner must be shared across
// threads for concurrent compilation and query execution.
#![expect(clippy::disallowed_types, reason = "Arc required for SharedInterner thread-safety")]

use super::Name;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Per-shard storage for interned strings.
struct InternShard {
    /// Map from string content to local index.
    map: FxHashMap<&'static str, u32>,
    /// Storage for string contents.
    strings: Vec<&'static str>,
}

impl InternShard {
    fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            strings: Vec::with_capacity(256),
        }
    }

    fn with_empty() -> Self {
        let mut shard = Self::new();
        // Pre-intern empty string at index 0
        let empty: &'static str = "";
        shard.map.insert(empty, 0);
        shard.strings.push(empty);
        shard
    }
}

/// Sharded string interner for concurrent access.
///
/// Provides O(1) lookup and equality comparison for interned strings.
///
/// # Thread Safety
/// Uses `RwLock` per shard for concurrent read/write access.
/// Can be wrapped in Arc for sharing across threads.
pub struct StringInterner {
    shards: [RwLock<InternShard>; Name::NUM_SHARDS],
}

impl StringInterner {
    /// Create a new interner with pre-interned keywords.
    pub fn new() -> Self {
        let shards = std::array::from_fn(|i| {
            if i == 0 {
                RwLock::new(InternShard::with_empty())
            } else {
                RwLock::new(InternShard::new())
            }
        });

        let interner = Self { shards };
        interner.pre_intern_keywords();
        interner
    }

    /// Compute shard for a string based on its hash.
    #[inline]
    fn shard_for(s: &str) -> usize {
        let mut hash = 0u32;
        for byte in s.bytes().take(8) {
            hash = hash.wrapping_mul(31).wrapping_add(u32::from(byte));
        }
        (hash as usize) % Name::NUM_SHARDS
    }

    /// Intern a string, returning its Name.
    ///
    /// # Panics
    /// Panics if the interner exceeds capacity (over 4 billion strings per shard).
    pub fn intern(&self, s: &str) -> Name {
        let shard_idx = Self::shard_for(s);
        // shard_idx is always < NUM_SHARDS (16) due to modulo, guaranteed to fit in u32
        let shard_idx_u32 = u32::try_from(shard_idx).unwrap_or_else(|_| {
            unreachable!("shard_idx {} from modulo {} cannot exceed u32", shard_idx, Name::NUM_SHARDS)
        });
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(s) {
                return Name::new(shard_idx_u32, local);
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(s) {
            return Name::new(shard_idx_u32, local);
        }

        // Leak the string to get 'static lifetime
        let owned: String = s.to_owned();
        let leaked: &'static str = Box::leak(owned.into_boxed_str());

        let local = u32::try_from(guard.strings.len()).unwrap_or_else(|_| {
            panic!("interner shard {shard_idx} exceeded u32::MAX strings")
        });
        guard.strings.push(leaked);
        guard.map.insert(leaked, local);

        Name::new(shard_idx_u32, local)
    }

    /// Look up the string for a Name.
    pub fn lookup(&self, name: Name) -> &str {
        let shard = &self.shards[name.shard()];
        let guard = shard.read();
        guard.strings[name.local()]
    }

    /// Pre-intern all Sigil keywords and common identifiers.
    fn pre_intern_keywords(&self) {
        const KEYWORDS: &[&str] = &[
            // Reserved keywords
            "async", "break", "continue", "do", "else", "false", "for", "if",
            "impl", "in", "let", "loop", "match", "mut", "pub", "self", "Self",
            "then", "trait", "true", "type", "use", "uses", "void", "where",
            "with", "yield",
            // Pattern keywords
            "cache", "collect", "filter", "find", "fold", "map", "parallel",
            "recurse", "retry", "run", "timeout", "try", "validate",
            // Primitive types
            "int", "float", "bool", "str", "char", "byte", "Never",
            // Common types
            "Option", "Result", "Some", "None", "Ok", "Err", "Error",
            // Common functions
            "main", "print", "len", "compare", "panic", "assert", "assert_eq",
        ];

        for kw in KEYWORDS {
            self.intern(kw);
        }
    }

    /// Get the number of interned strings.
    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.read().strings.len()).sum()
    }

    /// Check if the interner is empty (only has the empty string).
    pub fn is_empty(&self) -> bool {
        self.len() <= 1
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared interner for thread-safe string interning across compiler phases.
///
/// This newtype enforces that all thread-safe interner sharing goes through
/// this type, preventing accidental direct `Arc<StringInterner>` usage.
///
/// # Purpose
/// The string interner must be shared across lexer, parser, type checker,
/// and evaluator. `SharedInterner` provides a clonable handle that can be
/// passed to each compiler phase while ensuring all phases share the same
/// interned string storage.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting. The underlying
/// `StringInterner` uses per-shard `RwLocks` for concurrent access.
///
/// # Usage
/// ```ignore
/// let interner = SharedInterner::new();
/// let name = interner.intern("my_function");
/// let lookup = interner.resolve(name);
/// ```
#[derive(Clone)]
pub struct SharedInterner(Arc<StringInterner>);

impl SharedInterner {
    /// Create a new shared interner.
    pub fn new() -> Self {
        SharedInterner(Arc::new(StringInterner::new()))
    }
}

impl Default for SharedInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for SharedInterner {
    type Target = StringInterner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_and_lookup() {
        let interner = StringInterner::new();

        let hello = interner.intern("hello");
        let world = interner.intern("world");
        let hello2 = interner.intern("hello");

        assert_eq!(hello, hello2);
        assert_ne!(hello, world);

        assert_eq!(interner.lookup(hello), "hello");
        assert_eq!(interner.lookup(world), "world");
    }

    #[test]
    fn test_empty_string() {
        let interner = StringInterner::new();
        let empty = interner.intern("");
        assert_eq!(empty, Name::EMPTY);
        assert_eq!(interner.lookup(Name::EMPTY), "");
    }

    #[test]
    fn test_keywords_pre_interned() {
        let interner = StringInterner::new();

        let if_name = interner.intern("if");
        let else_name = interner.intern("else");

        assert_eq!(interner.lookup(if_name), "if");
        assert_eq!(interner.lookup(else_name), "else");
    }

    #[test]
    fn test_shared_interner() {
        let interner = SharedInterner::new();
        let interner2 = interner.clone();

        let name1 = interner.intern("shared");
        let name2 = interner2.intern("shared");

        assert_eq!(name1, name2);
    }
}
