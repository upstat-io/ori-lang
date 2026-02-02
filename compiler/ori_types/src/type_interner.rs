//! Sharded type interner for efficient type storage.
//!
//! Provides O(1) type interning, lookup, and equality comparison via `TypeId`.
//! Follows the same pattern as `StringInterner` in `ori_ir`.

// Arc is needed here for SharedTypeInterner - the interner must be shared across
// threads for concurrent compilation and query execution.
#![expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedTypeInterner thread-safety"
)]

use ori_ir::{Name, TypeId};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::core::Type;
use crate::data::{TypeData, TypeVar};

/// Error when interning a type fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeInternError {
    /// Shard exceeded capacity (over 268 million types per shard).
    ShardOverflow { shard_idx: usize },
}

impl std::fmt::Display for TypeInternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeInternError::ShardOverflow { shard_idx } => {
                write!(f, "type interner shard {shard_idx} exceeded u32::MAX types")
            }
        }
    }
}

impl std::error::Error for TypeInternError {}

/// Per-shard storage for interned types.
struct TypeShard {
    /// Map from type data to local index for deduplication.
    map: FxHashMap<TypeData, u32>,
    /// Storage for type data, indexed by local index.
    types: Vec<TypeData>,
}

impl TypeShard {
    fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            types: Vec::with_capacity(256),
        }
    }

    /// Create shard 0 with pre-interned primitives.
    fn with_primitives() -> Self {
        let mut shard = Self::new();

        // Pre-intern primitives at fixed indices matching TypeId constants
        let primitives = [
            TypeData::Int,      // 0 = TypeId::INT
            TypeData::Float,    // 1 = TypeId::FLOAT
            TypeData::Bool,     // 2 = TypeId::BOOL
            TypeData::Str,      // 3 = TypeId::STR
            TypeData::Char,     // 4 = TypeId::CHAR
            TypeData::Byte,     // 5 = TypeId::BYTE
            TypeData::Unit,     // 6 = TypeId::VOID
            TypeData::Never,    // 7 = TypeId::NEVER
            TypeData::Duration, // 8 - we skip INFER (it's special)
            TypeData::Size,     // 9 - we skip SELF_TYPE (it's special)
            TypeData::Error,    // 10 = TypeId::ERROR
            TypeData::Ordering, // 11 = TypeId::ORDERING
        ];

        for (idx, data) in primitives.into_iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "primitives count is fixed and small"
            )]
            let idx_u32 = idx as u32;
            shard.map.insert(data.clone(), idx_u32);
            shard.types.push(data);
        }

        shard
    }
}

/// Number of shards for type interning.
const NUM_SHARDS: usize = 16;

/// Sharded type interner for concurrent access.
///
/// Provides O(1) lookup and equality comparison for interned types.
///
/// # Thread Safety
/// Uses `RwLock` per shard for concurrent read/write access.
/// Can be wrapped in Arc for sharing across threads via `SharedTypeInterner`.
///
/// # Pre-interned Types
/// Primitive types are pre-interned with fixed `TypeId` values matching
/// the constants in `TypeId` (INT, FLOAT, BOOL, etc.).
pub struct TypeInterner {
    /// Sharded storage for concurrent access.
    shards: [RwLock<TypeShard>; NUM_SHARDS],
    /// Atomic counter for generating fresh type variables.
    next_var: AtomicU32,
}

impl TypeInterner {
    /// Create a new interner with pre-interned primitives.
    pub fn new() -> Self {
        let shards = std::array::from_fn(|i| {
            if i == 0 {
                RwLock::new(TypeShard::with_primitives())
            } else {
                RwLock::new(TypeShard::new())
            }
        });

        Self {
            shards,
            next_var: AtomicU32::new(0),
        }
    }

    /// Compute shard index for a type based on its hash.
    #[inline]
    fn shard_for(data: &TypeData) -> usize {
        let mut hasher = rustc_hash::FxHasher::default();
        data.hash(&mut hasher);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "truncation is fine for hash-based shard selection"
        )]
        let hash_usize = hasher.finish() as usize;
        hash_usize % NUM_SHARDS
    }

    /// Try to intern a type, returning its `TypeId` or an error on overflow.
    ///
    /// If the type is already interned, returns the existing `TypeId`.
    /// Otherwise, creates a new entry and returns a fresh `TypeId`.
    ///
    /// # Pre-interned Primitives
    /// Primitive types return fixed `TypeId` constants (INT, FLOAT, etc.)
    /// for compatibility with code that depends on these constants.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "shard_idx is bounded by NUM_SHARDS (16)"
    )]
    pub fn try_intern(&self, data: TypeData) -> Result<TypeId, TypeInternError> {
        // Fast path for primitives: return fixed TypeId constants
        match &data {
            TypeData::Int => return Ok(TypeId::INT),
            TypeData::Float => return Ok(TypeId::FLOAT),
            TypeData::Bool => return Ok(TypeId::BOOL),
            TypeData::Str => return Ok(TypeId::STR),
            TypeData::Char => return Ok(TypeId::CHAR),
            TypeData::Byte => return Ok(TypeId::BYTE),
            TypeData::Unit => return Ok(TypeId::VOID),
            TypeData::Never => return Ok(TypeId::NEVER),
            TypeData::Duration => return Ok(TypeId::from_shard_local(0, 8)),
            TypeData::Size => return Ok(TypeId::from_shard_local(0, 9)),
            TypeData::Error => return Ok(TypeId::from_shard_local(0, 10)),
            TypeData::Ordering => return Ok(TypeId::from_shard_local(0, 11)),
            _ => {}
        }

        let shard_idx = Self::shard_for(&data);
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(&data) {
                return Ok(TypeId::from_shard_local(shard_idx as u32, local));
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(&data) {
            return Ok(TypeId::from_shard_local(shard_idx as u32, local));
        }

        let local = u32::try_from(guard.types.len())
            .map_err(|_| TypeInternError::ShardOverflow { shard_idx })?;

        guard.types.push(data.clone());
        guard.map.insert(data, local);

        Ok(TypeId::from_shard_local(shard_idx as u32, local))
    }

    /// Intern a type, returning its `TypeId`.
    ///
    /// If the type is already interned, returns the existing `TypeId`.
    /// Otherwise, creates a new entry and returns a fresh `TypeId`.
    ///
    /// # Pre-interned Primitives
    /// Primitive types return fixed `TypeId` constants (INT, FLOAT, etc.)
    /// for compatibility with code that depends on these constants.
    ///
    /// # Panics
    /// Panics if a shard exceeds capacity (over 268 million types per shard).
    /// Use `try_intern` for fallible interning.
    pub fn intern(&self, data: TypeData) -> TypeId {
        self.try_intern(data).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Look up the type data for a `TypeId`.
    ///
    /// # Panics
    /// Panics if the `TypeId` is invalid (was not created by this interner).
    pub fn lookup(&self, id: TypeId) -> TypeData {
        let shard_idx = id.shard();
        let local = id.local();
        let shard = &self.shards[shard_idx];
        let guard = shard.read();
        guard.types[local].clone()
    }

    /// Convert a `TypeId` back to a boxed Type.
    ///
    /// This is the reverse direction of the bidirectional `Type`<->`TypeId` conversion.
    /// The forward operation is [`Type::to_type_id`], which interns a `Type`
    /// into a compact `TypeId`. Together they satisfy:
    ///
    /// ```text
    /// interner.to_type(ty.to_type_id(&interner)) == ty   // roundtrip
    /// ```
    ///
    /// Each match arm here reconstructs the boxed representation from interned
    /// `TypeData`. This is intentionally symmetric with `Type::to_type_id` —
    /// both traverse the same set of type variants but in opposite directions
    /// (interning vs reconstructing).
    ///
    /// See also: `Type::to_type_id` for the forward direction, and
    /// `test_type_to_type_id_roundtrip` and related tests for roundtrip verification.
    pub fn to_type(&self, id: TypeId) -> Type {
        match self.lookup(id) {
            // Primitives
            TypeData::Int => Type::Int,
            TypeData::Float => Type::Float,
            TypeData::Bool => Type::Bool,
            TypeData::Str => Type::Str,
            TypeData::Char => Type::Char,
            TypeData::Byte => Type::Byte,
            TypeData::Unit => Type::Unit,
            TypeData::Never => Type::Never,
            TypeData::Duration => Type::Duration,
            TypeData::Size => Type::Size,
            TypeData::Ordering => Type::Ordering,
            TypeData::Error => Type::Error,

            // Container types with single inner type
            TypeData::List(elem) => Type::List(Box::new(self.to_type(elem))),
            TypeData::Option(inner) => Type::Option(Box::new(self.to_type(inner))),
            TypeData::Set(elem) => Type::Set(Box::new(self.to_type(elem))),
            TypeData::Range(elem) => Type::Range(Box::new(self.to_type(elem))),
            TypeData::Channel(elem) => Type::Channel(Box::new(self.to_type(elem))),

            // Container types with multiple inner types
            TypeData::Map { key, value } => Type::Map {
                key: Box::new(self.to_type(key)),
                value: Box::new(self.to_type(value)),
            },
            TypeData::Result { ok, err } => Type::Result {
                ok: Box::new(self.to_type(ok)),
                err: Box::new(self.to_type(err)),
            },

            // Compound types
            TypeData::Tuple(types) => Type::Tuple(types.iter().map(|&t| self.to_type(t)).collect()),
            TypeData::Function { params, ret } => Type::Function {
                params: params.iter().map(|&p| self.to_type(p)).collect(),
                ret: Box::new(self.to_type(ret)),
            },

            // Named and generic types
            TypeData::Named(name) => Type::Named(name),
            TypeData::Applied { name, args } => Type::Applied {
                name,
                args: args.iter().map(|&a| self.to_type(a)).collect(),
            },

            // Type variables
            TypeData::Var(var) => Type::Var(var),

            // Projections
            TypeData::Projection {
                base,
                trait_name,
                assoc_name,
            } => Type::Projection {
                base: Box::new(self.to_type(base)),
                trait_name,
                assoc_name,
            },

            // Module namespaces
            TypeData::ModuleNamespace { items } => Type::ModuleNamespace {
                items: items
                    .iter()
                    .map(|(name, type_id)| (*name, self.to_type(*type_id)))
                    .collect(),
            },
        }
    }

    /// Generate a fresh type variable.
    pub fn fresh_var(&self) -> TypeId {
        let var_id = self.next_var.fetch_add(1, Ordering::Relaxed);
        self.intern(TypeData::Var(TypeVar::new(var_id)))
    }

    /// Get the current type variable counter value.
    pub fn var_count(&self) -> u32 {
        self.next_var.load(Ordering::Relaxed)
    }

    // Convenience methods for creating common types.
    //
    // All methods below use `intern()` internally, which guarantees deduplication:
    // calling the same method with the same arguments returns the same `TypeId`.
    // This enables O(1) type equality via `TypeId` comparison.

    /// Create a List type.
    ///
    /// Returns the same `TypeId` for identical element types (deduplication).
    pub fn list(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::List(elem))
    }

    /// Create an Option type.
    ///
    /// Returns the same `TypeId` for identical inner types (deduplication).
    pub fn option(&self, inner: TypeId) -> TypeId {
        self.intern(TypeData::Option(inner))
    }

    /// Create a Result type.
    ///
    /// Returns the same `TypeId` for identical ok/err types (deduplication).
    pub fn result(&self, ok: TypeId, err: TypeId) -> TypeId {
        self.intern(TypeData::Result { ok, err })
    }

    /// Create a Map type.
    ///
    /// Returns the same `TypeId` for identical key/value types (deduplication).
    pub fn map(&self, key: TypeId, value: TypeId) -> TypeId {
        self.intern(TypeData::Map { key, value })
    }

    /// Create a Set type.
    ///
    /// Returns the same `TypeId` for identical element types (deduplication).
    pub fn set(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Set(elem))
    }

    /// Create a Range type.
    ///
    /// Returns the same `TypeId` for identical element types (deduplication).
    pub fn range(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Range(elem))
    }

    /// Create a Channel type.
    ///
    /// Returns the same `TypeId` for identical element types (deduplication).
    pub fn channel(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Channel(elem))
    }

    /// Create a Tuple type.
    ///
    /// Returns the same `TypeId` for identical element type sequences (deduplication).
    pub fn tuple(&self, types: impl Into<Box<[TypeId]>>) -> TypeId {
        self.intern(TypeData::Tuple(types.into()))
    }

    /// Create a Function type.
    ///
    /// Returns the same `TypeId` for identical param/return types (deduplication).
    pub fn function(&self, params: impl Into<Box<[TypeId]>>, ret: TypeId) -> TypeId {
        self.intern(TypeData::Function {
            params: params.into(),
            ret,
        })
    }

    /// Create a Named type.
    ///
    /// Returns the same `TypeId` for identical names (deduplication).
    pub fn named(&self, name: Name) -> TypeId {
        self.intern(TypeData::Named(name))
    }

    /// Create an Applied generic type.
    ///
    /// Returns the same `TypeId` for identical name/args (deduplication).
    pub fn applied(&self, name: Name, args: impl Into<Box<[TypeId]>>) -> TypeId {
        self.intern(TypeData::Applied {
            name,
            args: args.into(),
        })
    }

    /// Create a Projection type.
    ///
    /// Returns the same `TypeId` for identical base/trait/assoc (deduplication).
    pub fn projection(&self, base: TypeId, trait_name: Name, assoc_name: Name) -> TypeId {
        self.intern(TypeData::Projection {
            base,
            trait_name,
            assoc_name,
        })
    }

    /// Get the Error type.
    ///
    /// Always returns the same pre-interned `TypeId`.
    pub fn error(&self) -> TypeId {
        // Error is pre-interned at shard 0, index 10
        TypeId::from_shard_local(0, 10)
    }

    /// Create a `ModuleNamespace` type.
    ///
    /// Returns the same `TypeId` for identical item lists (deduplication).
    pub fn module_namespace(&self, items: impl Into<Box<[(Name, TypeId)]>>) -> TypeId {
        self.intern(TypeData::ModuleNamespace {
            items: items.into(),
        })
    }

    /// Get the number of interned types.
    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.read().types.len()).sum()
    }

    /// Check if the interner has only pre-interned primitives.
    pub fn is_empty(&self) -> bool {
        // Shard 0 has 12 pre-interned types (primitives + error + ordering)
        self.len() <= 12
    }
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared type interner for thread-safe type interning across compiler phases.
///
/// This newtype enforces that all thread-safe interner sharing goes through
/// this type, preventing accidental direct `Arc<TypeInterner>` usage.
///
/// # Purpose
/// The type interner must be shared across type checking, pattern checking,
/// and evaluation phases. `SharedTypeInterner` provides a clonable handle
/// that can be passed to each compiler phase while ensuring all phases share
/// the same interned type storage.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting. The underlying
/// `TypeInterner` uses per-shard `RwLocks` for concurrent access.
#[derive(Clone)]
pub struct SharedTypeInterner(Arc<TypeInterner>);

impl std::fmt::Debug for SharedTypeInterner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedTypeInterner")
            .field("len", &self.0.len())
            .finish()
    }
}

impl SharedTypeInterner {
    /// Create a new shared type interner.
    pub fn new() -> Self {
        SharedTypeInterner(Arc::new(TypeInterner::new()))
    }
}

impl Default for SharedTypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for SharedTypeInterner {
    type Target = TypeInterner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
