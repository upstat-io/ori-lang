//! Sharded string interner for efficient identifier storage.
//!
//! Provides O(1) interning and lookup with thread-safe concurrent access
//! via per-shard locking.

// Arc is needed here for SharedInterner - the interner must be shared across
// threads for concurrent compilation and query execution.
#![expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedInterner thread-safety"
)]

use super::Name;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Per-shard storage for interned strings.
struct InternShard {
    /// Map from string content to local index.
    map: FxHashMap<&'static str, u32>,
    /// Storage for string contents.
    strings: Vec<&'static str>,
}

/// Error when interning a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternError {
    /// Shard exceeded capacity (over 4 billion strings).
    ShardOverflow { shard_idx: usize, count: usize },
}

impl std::fmt::Display for InternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternError::ShardOverflow { shard_idx, count } => write!(
                f,
                "interner shard {} exceeded capacity: {} strings (0x{:X}), max is {} (0x{:X})",
                shard_idx,
                count,
                count,
                u32::MAX,
                u32::MAX
            ),
        }
    }
}

impl std::error::Error for InternError {}

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
    /// Total count of interned strings across all shards (O(1) `len()`).
    total_count: AtomicUsize,
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

        // Start with 1 for the empty string pre-interned in shard 0
        let interner = Self {
            shards,
            total_count: AtomicUsize::new(1),
        };
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

    /// Try to intern a string, returning its Name or an error on overflow.
    ///
    /// This is the fallible version of `intern()`. Use this when you need to
    /// handle the overflow case gracefully instead of panicking.
    #[inline]
    pub fn try_intern(&self, s: &str) -> Result<Name, InternError> {
        let shard_idx = Self::shard_for(s);
        // shard_idx is always < NUM_SHARDS (16) due to modulo, guaranteed to fit in u32
        #[expect(
            clippy::cast_possible_truncation,
            reason = "shard_idx is bounded by NUM_SHARDS (16)"
        )]
        let shard_idx_u32 = shard_idx as u32;
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(s) {
                return Ok(Name::new(shard_idx_u32, local));
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(s) {
            return Ok(Name::new(shard_idx_u32, local));
        }

        // Leak the string to get 'static lifetime
        let owned: String = s.to_owned();
        let leaked: &'static str = Box::leak(owned.into_boxed_str());

        let local = u32::try_from(guard.strings.len()).map_err(|_| InternError::ShardOverflow {
            shard_idx,
            count: guard.strings.len(),
        })?;
        guard.strings.push(leaked);
        guard.map.insert(leaked, local);

        // Increment total count (Relaxed is fine - we don't need ordering guarantees)
        self.total_count.fetch_add(1, Ordering::Relaxed);

        Ok(Name::new(shard_idx_u32, local))
    }

    /// Intern a string, returning its Name.
    ///
    /// # Panics
    /// Panics if the interner exceeds capacity (over 4 billion strings per shard).
    /// Use `try_intern` for fallible interning.
    #[inline]
    pub fn intern(&self, s: &str) -> Name {
        self.try_intern(s).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Try to intern an owned String, returning its Name or an error on overflow.
    ///
    /// This is more efficient than `try_intern()` when you already have an owned String
    /// (e.g., from `unescape_string`), as it avoids the extra allocation that
    /// `try_intern(&s)` would perform.
    pub fn try_intern_owned(&self, s: String) -> Result<Name, InternError> {
        let shard_idx = Self::shard_for(&s);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "shard_idx is bounded by NUM_SHARDS (16)"
        )]
        let shard_idx_u32 = shard_idx as u32;
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(s.as_str()) {
                return Ok(Name::new(shard_idx_u32, local));
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(s.as_str()) {
            return Ok(Name::new(shard_idx_u32, local));
        }

        // Leak the owned string directly (no extra allocation)
        let leaked: &'static str = Box::leak(s.into_boxed_str());

        let local = u32::try_from(guard.strings.len()).map_err(|_| InternError::ShardOverflow {
            shard_idx,
            count: guard.strings.len(),
        })?;
        guard.strings.push(leaked);
        guard.map.insert(leaked, local);

        self.total_count.fetch_add(1, Ordering::Relaxed);

        Ok(Name::new(shard_idx_u32, local))
    }

    /// Intern an owned String, avoiding double allocation.
    ///
    /// This is more efficient than `intern()` when you already have an owned String
    /// (e.g., from `unescape_string`), as it avoids the extra allocation that
    /// `intern(&s)` would perform.
    ///
    /// # Panics
    /// Panics if the interner exceeds capacity (over 4 billion strings per shard).
    /// Use `try_intern_owned` for fallible interning.
    pub fn intern_owned(&self, s: String) -> Name {
        self.try_intern_owned(s).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Look up the string for a Name.
    pub fn lookup(&self, name: Name) -> &str {
        let shard = &self.shards[name.shard()];
        let guard = shard.read();
        guard.strings[name.local()]
    }

    /// Look up the string for a Name, returning a `'static` reference.
    ///
    /// This is safe because all interned strings are leaked (never deallocated).
    /// Use this when you need to store the string reference without lifetime concerns,
    /// such as in `Cow<'static, str>` for zero-copy string values.
    pub fn lookup_static(&self, name: Name) -> &'static str {
        let shard = &self.shards[name.shard()];
        let guard = shard.read();
        guard.strings[name.local()]
    }

    /// Pre-intern all Ori keywords and common identifiers.
    fn pre_intern_keywords(&self) {
        const KEYWORDS: &[&str] = &[
            // Reserved keywords
            "async",
            "break",
            "continue",
            "do",
            "else",
            "false",
            "for",
            "if",
            "impl",
            "in",
            "let",
            "loop",
            "match",
            "mut",
            "pub",
            "self",
            "Self",
            "then",
            "trait",
            "true",
            "type",
            "use",
            "uses",
            "void",
            "where",
            "with",
            "yield",
            // Pattern keywords
            "cache",
            "catch",
            "collect",
            "filter",
            "find",
            "fold",
            "map",
            "parallel",
            "recurse",
            "retry",
            "run",
            "timeout",
            "try",
            "validate",
            // Primitive types
            "int",
            "float",
            "bool",
            "str",
            "char",
            "byte",
            "Never",
            // Common types
            "Option",
            "Result",
            "Some",
            "None",
            "Ok",
            "Err",
            "Error",
            // Common functions
            "main",
            "print",
            "len",
            "compare",
            "panic",
            "assert",
            "assert_eq",
        ];

        for kw in KEYWORDS {
            self.intern(kw);
        }
    }

    /// Get the number of interned strings (O(1)).
    pub fn len(&self) -> usize {
        self.total_count.load(Ordering::Relaxed)
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

/// Trait for looking up interned string names.
///
/// This trait exists to avoid tight coupling: higher-level crates can define
/// methods that accept any `StringLookup` implementor without depending directly
/// on `StringInterner`.
///
/// # Example
///
/// ```text
/// fn display_type_name<I: StringLookup>(value: &Value, interner: &I) -> String {
///     value.type_name_with_interner(interner).into_owned()
/// }
/// ```
pub trait StringLookup {
    /// Look up the string for an interned name.
    fn lookup(&self, name: Name) -> &str;
}

impl StringLookup for StringInterner {
    fn lookup(&self, name: Name) -> &str {
        StringInterner::lookup(self, name)
    }
}

/// Shared interner for thread-safe string interning across compiler phases.
///
/// This newtype enforces that all thread-safe interner sharing goes through
/// this type, preventing accidental direct `Arc<StringInterner>` usage.
///
/// # When to Use This vs `&StringInterner`
///
/// **Use `SharedInterner` (Arc) when:**
/// - Creating the interner at a coordination point (e.g., Salsa database)
/// - Passing to phases that may run concurrently or outlive the caller
/// - The interner must be cloned into multiple owned handles
///
/// **Use `&'a StringInterner` (borrowed) when:**
/// - The caller owns the interner and callees just need read access
/// - Lifetime is well-defined (codegen borrows from earlier phase output)
/// - Zero runtime cost is required (no atomic ref counting)
///
/// **Example - Correct patterns:**
/// ```ignore
/// // Salsa database owns the interner
/// let db = CompilerDb::new(); // contains SharedInterner internally
///
/// // Codegen borrows - does NOT need Arc
/// fn compile(cx: &CodegenCx, interner: &StringInterner) { ... }
/// ```
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting. The underlying
/// `StringInterner` uses per-shard `RwLocks` for concurrent access.
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

    #[test]
    fn test_intern_owned() {
        let interner = StringInterner::new();

        // Intern an owned string
        let owned = String::from("owned_string");
        let name1 = interner.intern_owned(owned);

        // Should return same Name for equivalent string
        let name2 = interner.intern("owned_string");
        assert_eq!(name1, name2);

        assert_eq!(interner.lookup(name1), "owned_string");
    }

    #[test]
    fn test_intern_owned_already_interned() {
        let interner = StringInterner::new();

        // First intern via reference
        let name1 = interner.intern("test_string");

        // Then intern owned - should return same Name
        let owned = String::from("test_string");
        let name2 = interner.intern_owned(owned);

        assert_eq!(name1, name2);
    }
}
