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
            TypeData::Error,    // 10 = first compound, but we use it for Error
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
    #[expect(
        clippy::cast_possible_truncation,
        reason = "shard_idx is bounded by NUM_SHARDS (16)"
    )]
    pub fn intern(&self, data: TypeData) -> TypeId {
        // Fast path for primitives: return fixed TypeId constants
        match &data {
            TypeData::Int => return TypeId::INT,
            TypeData::Float => return TypeId::FLOAT,
            TypeData::Bool => return TypeId::BOOL,
            TypeData::Str => return TypeId::STR,
            TypeData::Char => return TypeId::CHAR,
            TypeData::Byte => return TypeId::BYTE,
            TypeData::Unit => return TypeId::VOID,
            TypeData::Never => return TypeId::NEVER,
            TypeData::Duration => return TypeId::from_shard_local(0, 8),
            TypeData::Size => return TypeId::from_shard_local(0, 9),
            TypeData::Error => return TypeId::from_shard_local(0, 10),
            _ => {}
        }

        let shard_idx = Self::shard_for(&data);
        let shard = &self.shards[shard_idx];

        // Fast path: check if already interned
        {
            let guard = shard.read();
            if let Some(&local) = guard.map.get(&data) {
                return TypeId::from_shard_local(shard_idx as u32, local);
            }
        }

        // Slow path: need to insert
        let mut guard = shard.write();

        // Double-check after acquiring write lock
        if let Some(&local) = guard.map.get(&data) {
            return TypeId::from_shard_local(shard_idx as u32, local);
        }

        let local = u32::try_from(guard.types.len())
            .unwrap_or_else(|_| panic!("type interner shard {shard_idx} exceeded u32::MAX types"));

        guard.types.push(data.clone());
        guard.map.insert(data, local);

        TypeId::from_shard_local(shard_idx as u32, local)
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

    // Convenience methods for creating common types

    /// Create a List type.
    pub fn list(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::List(elem))
    }

    /// Create an Option type.
    pub fn option(&self, inner: TypeId) -> TypeId {
        self.intern(TypeData::Option(inner))
    }

    /// Create a Result type.
    pub fn result(&self, ok: TypeId, err: TypeId) -> TypeId {
        self.intern(TypeData::Result { ok, err })
    }

    /// Create a Map type.
    pub fn map(&self, key: TypeId, value: TypeId) -> TypeId {
        self.intern(TypeData::Map { key, value })
    }

    /// Create a Set type.
    pub fn set(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Set(elem))
    }

    /// Create a Range type.
    pub fn range(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Range(elem))
    }

    /// Create a Channel type.
    pub fn channel(&self, elem: TypeId) -> TypeId {
        self.intern(TypeData::Channel(elem))
    }

    /// Create a Tuple type.
    pub fn tuple(&self, types: impl Into<Box<[TypeId]>>) -> TypeId {
        self.intern(TypeData::Tuple(types.into()))
    }

    /// Create a Function type.
    pub fn function(&self, params: impl Into<Box<[TypeId]>>, ret: TypeId) -> TypeId {
        self.intern(TypeData::Function {
            params: params.into(),
            ret,
        })
    }

    /// Create a Named type.
    pub fn named(&self, name: Name) -> TypeId {
        self.intern(TypeData::Named(name))
    }

    /// Create an Applied generic type.
    pub fn applied(&self, name: Name, args: impl Into<Box<[TypeId]>>) -> TypeId {
        self.intern(TypeData::Applied {
            name,
            args: args.into(),
        })
    }

    /// Create a Projection type.
    pub fn projection(&self, base: TypeId, trait_name: Name, assoc_name: Name) -> TypeId {
        self.intern(TypeData::Projection {
            base,
            trait_name,
            assoc_name,
        })
    }

    /// Get the Error type.
    pub fn error(&self) -> TypeId {
        // Error is pre-interned at shard 0, index 10
        TypeId::from_shard_local(0, 10)
    }

    /// Create a ModuleNamespace type.
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
        // Shard 0 has 11 pre-interned types (primitives + error)
        self.len() <= 11
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

/// Trait for looking up interned type data.
///
/// This trait exists to avoid tight coupling: higher-level crates can define
/// methods that accept any `TypeLookup` implementor without depending directly
/// on `TypeInterner`.
pub trait TypeLookup {
    /// Look up the type data for a `TypeId`.
    fn lookup(&self, id: TypeId) -> TypeData;
}

impl TypeLookup for TypeInterner {
    fn lookup(&self, id: TypeId) -> TypeData {
        TypeInterner::lookup(self, id)
    }
}

impl TypeLookup for SharedTypeInterner {
    fn lookup(&self, id: TypeId) -> TypeData {
        TypeInterner::lookup(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_preinterned() {
        let interner = TypeInterner::new();

        // Primitives should return pre-interned IDs
        assert_eq!(interner.intern(TypeData::Int), TypeId::INT);
        assert_eq!(interner.intern(TypeData::Float), TypeId::FLOAT);
        assert_eq!(interner.intern(TypeData::Bool), TypeId::BOOL);
        assert_eq!(interner.intern(TypeData::Str), TypeId::STR);
        assert_eq!(interner.intern(TypeData::Char), TypeId::CHAR);
        assert_eq!(interner.intern(TypeData::Byte), TypeId::BYTE);
        assert_eq!(interner.intern(TypeData::Unit), TypeId::VOID);
        assert_eq!(interner.intern(TypeData::Never), TypeId::NEVER);
    }

    #[test]
    fn test_intern_and_lookup() {
        let interner = TypeInterner::new();

        let list_int = interner.list(TypeId::INT);
        let list_bool = interner.list(TypeId::BOOL);
        let list_int2 = interner.list(TypeId::INT);

        // Same type returns same ID
        assert_eq!(list_int, list_int2);
        // Different types return different IDs
        assert_ne!(list_int, list_bool);

        // Lookup returns correct data
        assert_eq!(interner.lookup(list_int), TypeData::List(TypeId::INT));
        assert_eq!(interner.lookup(list_bool), TypeData::List(TypeId::BOOL));
    }

    #[test]
    fn test_fresh_var() {
        let interner = TypeInterner::new();

        let var1 = interner.fresh_var();
        let var2 = interner.fresh_var();

        // Each fresh_var is different
        assert_ne!(var1, var2);

        // Lookup returns Var with incrementing IDs
        if let TypeData::Var(v1) = interner.lookup(var1) {
            if let TypeData::Var(v2) = interner.lookup(var2) {
                assert_eq!(v1.0, 0);
                assert_eq!(v2.0, 1);
            } else {
                panic!("Expected Var");
            }
        } else {
            panic!("Expected Var");
        }
    }

    #[test]
    fn test_complex_types() {
        let interner = TypeInterner::new();

        // Result<int, str>
        let result_type = interner.result(TypeId::INT, TypeId::STR);

        // Function(int, bool) -> str
        let fn_type = interner.function(vec![TypeId::INT, TypeId::BOOL], TypeId::STR);

        // Tuple(int, bool)
        let tuple_type = interner.tuple(vec![TypeId::INT, TypeId::BOOL]);

        // All different
        assert_ne!(result_type, fn_type);
        assert_ne!(fn_type, tuple_type);

        // Lookup verifies structure
        match interner.lookup(result_type) {
            TypeData::Result { ok, err } => {
                assert_eq!(ok, TypeId::INT);
                assert_eq!(err, TypeId::STR);
            }
            _ => panic!("Expected Result"),
        }

        match interner.lookup(fn_type) {
            TypeData::Function { params, ret } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], TypeId::INT);
                assert_eq!(params[1], TypeId::BOOL);
                assert_eq!(ret, TypeId::STR);
            }
            _ => panic!("Expected Function"),
        }
    }

    #[test]
    fn test_shared_interner() {
        let interner = SharedTypeInterner::new();
        let interner2 = interner.clone();

        let list1 = interner.list(TypeId::INT);
        let list2 = interner2.list(TypeId::INT);

        // Same type from shared interner
        assert_eq!(list1, list2);
    }

    #[test]
    fn test_error_type() {
        let interner = TypeInterner::new();

        let error = interner.error();
        let error2 = interner.intern(TypeData::Error);

        assert_eq!(error, error2);
        assert!(interner.lookup(error).is_error());
    }

    #[test]
    fn test_named_types() {
        let interner = TypeInterner::new();

        // Create two named types with different Names
        let name1 = Name::new(1, 100);
        let name2 = Name::new(1, 200);

        let named1 = interner.named(name1);
        let named2 = interner.named(name2);
        let named1_again = interner.named(name1);

        assert_eq!(named1, named1_again);
        assert_ne!(named1, named2);
    }

    #[test]
    fn test_applied_generic() {
        let interner = TypeInterner::new();

        let name = Name::new(0, 50); // e.g., "Vec"

        // Vec<int>
        let vec_int = interner.applied(name, vec![TypeId::INT]);
        // Vec<bool>
        let vec_bool = interner.applied(name, vec![TypeId::BOOL]);
        // Vec<int> again
        let vec_int2 = interner.applied(name, vec![TypeId::INT]);

        assert_eq!(vec_int, vec_int2);
        assert_ne!(vec_int, vec_bool);
    }

    #[test]
    fn test_type_to_type_id_roundtrip() {
        let interner = TypeInterner::new();

        // Test primitives
        assert_eq!(interner.to_type(TypeId::INT), Type::Int);
        assert_eq!(interner.to_type(TypeId::FLOAT), Type::Float);
        assert_eq!(interner.to_type(TypeId::BOOL), Type::Bool);
        assert_eq!(interner.to_type(TypeId::STR), Type::Str);

        // Test Type -> TypeId -> Type roundtrip for primitives
        assert_eq!(interner.to_type(Type::Int.to_type_id(&interner)), Type::Int);
        assert_eq!(
            interner.to_type(Type::Float.to_type_id(&interner)),
            Type::Float
        );
    }

    #[test]
    fn test_container_type_roundtrip() {
        let interner = TypeInterner::new();

        // List<int>
        let list_int = Type::List(Box::new(Type::Int));
        let list_id = list_int.to_type_id(&interner);
        let roundtrip = interner.to_type(list_id);
        assert_eq!(roundtrip, list_int);

        // Option<str>
        let opt_str = Type::Option(Box::new(Type::Str));
        let opt_id = opt_str.to_type_id(&interner);
        assert_eq!(interner.to_type(opt_id), opt_str);

        // Result<int, str>
        let result_ty = Type::Result {
            ok: Box::new(Type::Int),
            err: Box::new(Type::Str),
        };
        let result_id = result_ty.to_type_id(&interner);
        assert_eq!(interner.to_type(result_id), result_ty);

        // Map<str, int>
        let map_ty = Type::Map {
            key: Box::new(Type::Str),
            value: Box::new(Type::Int),
        };
        let map_id = map_ty.to_type_id(&interner);
        assert_eq!(interner.to_type(map_id), map_ty);
    }

    #[test]
    fn test_function_type_roundtrip() {
        let interner = TypeInterner::new();

        // (int, bool) -> str
        let fn_ty = Type::Function {
            params: vec![Type::Int, Type::Bool],
            ret: Box::new(Type::Str),
        };
        let fn_id = fn_ty.to_type_id(&interner);
        assert_eq!(interner.to_type(fn_id), fn_ty);
    }

    #[test]
    fn test_tuple_type_roundtrip() {
        let interner = TypeInterner::new();

        let tuple_ty = Type::Tuple(vec![Type::Int, Type::Bool, Type::Str]);
        let tuple_id = tuple_ty.to_type_id(&interner);
        assert_eq!(interner.to_type(tuple_id), tuple_ty);
    }

    #[test]
    fn test_nested_type_roundtrip() {
        let interner = TypeInterner::new();

        // [[int]] - nested list
        let nested = Type::List(Box::new(Type::List(Box::new(Type::Int))));
        let nested_id = nested.to_type_id(&interner);
        assert_eq!(interner.to_type(nested_id), nested);

        // Option<Result<int, str>>
        let complex = Type::Option(Box::new(Type::Result {
            ok: Box::new(Type::Int),
            err: Box::new(Type::Str),
        }));
        let complex_id = complex.to_type_id(&interner);
        assert_eq!(interner.to_type(complex_id), complex);
    }

    #[test]
    fn test_var_type_roundtrip() {
        let interner = TypeInterner::new();

        let var = Type::Var(TypeVar::new(42));
        let var_id = var.to_type_id(&interner);
        assert_eq!(interner.to_type(var_id), var);
    }

    #[test]
    fn test_named_type_roundtrip() {
        let interner = TypeInterner::new();

        let name = Name::new(0, 100);
        let named = Type::Named(name);
        let named_id = named.to_type_id(&interner);
        assert_eq!(interner.to_type(named_id), named);
    }

    #[test]
    fn test_applied_type_roundtrip() {
        let interner = TypeInterner::new();

        let name = Name::new(0, 50);
        let applied = Type::Applied {
            name,
            args: vec![Type::Int, Type::Bool],
        };
        let applied_id = applied.to_type_id(&interner);
        assert_eq!(interner.to_type(applied_id), applied);
    }

    #[test]
    fn test_projection_type_roundtrip() {
        let interner = TypeInterner::new();

        let trait_name = Name::new(0, 10);
        let assoc_name = Name::new(0, 20);
        let projection = Type::Projection {
            base: Box::new(Type::Var(TypeVar::new(5))),
            trait_name,
            assoc_name,
        };
        let proj_id = projection.to_type_id(&interner);
        assert_eq!(interner.to_type(proj_id), projection);
    }

    #[test]
    fn test_same_type_same_id() {
        let interner = TypeInterner::new();

        // Two structurally equal types should get the same TypeId
        let list1 = Type::List(Box::new(Type::Int));
        let list2 = Type::List(Box::new(Type::Int));

        let id1 = list1.to_type_id(&interner);
        let id2 = list2.to_type_id(&interner);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_module_namespace() {
        let interner = TypeInterner::new();

        let name1 = Name::new(0, 100); // e.g., "add"
        let name2 = Name::new(0, 200); // e.g., "subtract"

        // Create a module namespace with two function items
        let fn_type1 = interner.function(vec![TypeId::INT, TypeId::INT], TypeId::INT);
        let fn_type2 = interner.function(vec![TypeId::INT, TypeId::INT], TypeId::INT);

        let ns_id = interner.module_namespace(vec![(name1, fn_type1), (name2, fn_type2)]);

        // Verify lookup returns correct data
        match interner.lookup(ns_id) {
            TypeData::ModuleNamespace { items } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].0, name1);
                assert_eq!(items[0].1, fn_type1);
                assert_eq!(items[1].0, name2);
                assert_eq!(items[1].1, fn_type2);
            }
            _ => panic!("Expected ModuleNamespace"),
        }
    }

    #[test]
    fn test_module_namespace_roundtrip() {
        let interner = TypeInterner::new();

        let name1 = Name::new(0, 100);
        let name2 = Name::new(0, 200);

        let ns_ty = Type::ModuleNamespace {
            items: vec![
                (name1, Type::Function { params: vec![Type::Int], ret: Box::new(Type::Int) }),
                (name2, Type::Function { params: vec![Type::Str], ret: Box::new(Type::Bool) }),
            ],
        };
        let ns_id = ns_ty.to_type_id(&interner);
        assert_eq!(interner.to_type(ns_id), ns_ty);
    }
}
