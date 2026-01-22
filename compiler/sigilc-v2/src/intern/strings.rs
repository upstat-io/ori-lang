//! Sharded string interner for efficient identifier storage.
//!
//! Uses 16 shards with per-shard RwLock for concurrent access.
//! Strings are stored contiguously and never deallocated.

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Interned string identifier.
///
/// Layout: 32-bit index split into shard (4 bits) + local index (28 bits)
/// - Bits 31-28: Shard index (0-15)
/// - Bits 27-0: Local index within shard
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Name(u32);

impl Name {
    /// Pre-interned empty string.
    pub const EMPTY: Name = Name(0);

    /// Maximum local index per shard.
    pub const MAX_LOCAL: u32 = 0x0FFF_FFFF;

    /// Number of shards.
    pub const NUM_SHARDS: usize = 16;

    /// Create from shard and local index.
    #[inline]
    pub const fn new(shard: u32, local: u32) -> Self {
        debug_assert!(shard < 16);
        debug_assert!(local <= Self::MAX_LOCAL);
        Name((shard << 28) | local)
    }

    /// Extract shard index.
    #[inline]
    pub const fn shard(self) -> usize {
        (self.0 >> 28) as usize
    }

    /// Extract local index.
    #[inline]
    pub const fn local(self) -> usize {
        (self.0 & Self::MAX_LOCAL) as usize
    }

    /// Get raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Create from raw u32 value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Name(raw)
    }
}

impl Hash for Name {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Name(shard={}, local={})", self.shard(), self.local())
    }
}

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
pub struct StringInterner {
    shards: [RwLock<InternShard>; Name::NUM_SHARDS],
}

impl StringInterner {
    /// Create a new interner with pre-interned keywords and common strings.
    pub fn new() -> Self {
        let shards = std::array::from_fn(|i| {
            if i == 0 {
                RwLock::new(InternShard::with_empty())
            } else {
                RwLock::new(InternShard::new())
            }
        });

        let interner = Self { shards };

        // Pre-intern keywords and common strings
        interner.pre_intern_keywords();

        interner
    }

    /// Compute shard for a string based on its hash.
    #[inline]
    fn shard_for(s: &str) -> usize {
        // Simple hash for shard selection
        let mut hash = 0u32;
        for byte in s.bytes().take(8) {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        (hash as usize) % Name::NUM_SHARDS
    }

    /// Intern a string, returning its Name.
    pub fn intern(&self, s: &str) -> Name {
        let shard_idx = Self::shard_for(s);
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(s) {
                return Name::new(shard_idx as u32, local);
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(s) {
            return Name::new(shard_idx as u32, local);
        }

        // Leak the string to get 'static lifetime
        let owned: String = s.to_owned();
        let leaked: &'static str = Box::leak(owned.into_boxed_str());

        let local = guard.strings.len() as u32;
        guard.strings.push(leaked);
        guard.map.insert(leaked, local);

        Name::new(shard_idx as u32, local)
    }

    /// Look up the string for a Name.
    pub fn lookup(&self, name: Name) -> &str {
        let shard = &self.shards[name.shard()];
        let guard = shard.read();
        guard.strings[name.local()]
    }

    /// Pre-intern all Sigil keywords and common identifiers.
    fn pre_intern_keywords(&self) {
        // Keywords
        const KEYWORDS: &[&str] = &[
            // Reserved keywords
            "async", "break", "continue", "do", "else", "false", "for", "if",
            "impl", "in", "let", "loop", "match", "mut", "pub", "self", "Self",
            "then", "trait", "true", "type", "use", "uses", "void", "where",
            "with", "yield",
            // Pattern keywords (context-sensitive)
            "cache", "collect", "filter", "find", "fold", "map", "parallel",
            "recurse", "retry", "run", "timeout", "try", "validate",
            // Primitive types
            "int", "float", "bool", "str", "char", "byte", "Never",
            // Common types
            "Option", "Result", "Some", "None", "Ok", "Err", "Error",
            "List", "Map", "Set", "Range", "Channel", "Duration", "Size",
            "Ordering", "Less", "Equal", "Greater",
            // Common traits
            "Eq", "Comparable", "Hashable", "Printable", "Clone", "Default",
            // Common functions
            "main", "print", "len", "compare", "panic", "assert", "assert_eq",
            // Common identifiers
            "x", "y", "z", "i", "j", "n", "s", "a", "b", "c",
            "value", "result", "error", "item", "items", "init", "acc",
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

// StringInterner is Send + Sync because:
// - All shards use RwLock which is Send + Sync
// - The strings are 'static and immutable after interning
// These traits are automatically derived since RwLock<T> is Send + Sync when T is Send

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

        // Keywords should be fast to look up (already interned)
        let if_name = interner.intern("if");
        let else_name = interner.intern("else");
        let for_name = interner.intern("for");

        assert_eq!(interner.lookup(if_name), "if");
        assert_eq!(interner.lookup(else_name), "else");
        assert_eq!(interner.lookup(for_name), "for");
    }

    #[test]
    fn test_name_shard_local() {
        let name = Name::new(5, 100);
        assert_eq!(name.shard(), 5);
        assert_eq!(name.local(), 100);
    }

    #[test]
    fn test_concurrent_interning() {
        use std::sync::Arc;
        use std::thread;

        let interner = Arc::new(StringInterner::new());
        let mut handles = vec![];

        for t in 0..4 {
            let interner = Arc::clone(&interner);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let s = format!("string_{}_{}", t, i);
                    let name = interner.intern(&s);
                    assert_eq!(interner.lookup(name), s);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
