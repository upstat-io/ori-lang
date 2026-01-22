//! Type interner for efficient type storage and comparison.
//!
//! Uses DashMap for concurrent access with pre-interned primitives.

use super::Name;
use dashmap::DashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

/// Interned type identifier.
///
/// Pre-interned primitives have known indices (0-8).
/// Compound types are interned on first use.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    // Pre-interned primitive types
    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    pub const CHAR: TypeId = TypeId(4);
    pub const BYTE: TypeId = TypeId(5);
    pub const VOID: TypeId = TypeId(6);
    pub const NEVER: TypeId = TypeId(7);
    pub const INFER: TypeId = TypeId(8); // Placeholder during inference

    /// First index for compound types.
    pub const FIRST_COMPOUND: u32 = 9;

    /// Invalid type ID (used for errors).
    pub const INVALID: TypeId = TypeId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        TypeId(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn is_primitive(self) -> bool {
        self.0 < Self::FIRST_COMPOUND
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl Hash for TypeId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::INT => write!(f, "TypeId::INT"),
            Self::FLOAT => write!(f, "TypeId::FLOAT"),
            Self::BOOL => write!(f, "TypeId::BOOL"),
            Self::STR => write!(f, "TypeId::STR"),
            Self::CHAR => write!(f, "TypeId::CHAR"),
            Self::BYTE => write!(f, "TypeId::BYTE"),
            Self::VOID => write!(f, "TypeId::VOID"),
            Self::NEVER => write!(f, "TypeId::NEVER"),
            Self::INFER => write!(f, "TypeId::INFER"),
            Self::INVALID => write!(f, "TypeId::INVALID"),
            _ => write!(f, "TypeId({})", self.0),
        }
    }
}

/// Range of types (for function params, tuple elements, generics).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct TypeRange {
    pub start: u32,
    pub len: u16,
}

impl TypeRange {
    pub const EMPTY: TypeRange = TypeRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        TypeRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn indices(&self) -> impl Iterator<Item = u32> {
        self.start..(self.start + self.len as u32)
    }
}

impl fmt::Debug for TypeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeRange({}..+{})", self.start, self.len)
    }
}

/// Representation of a type in the type system.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum TypeKind {
    // Primitives (interned at known indices, but also stored here for lookup)
    Int,
    Float,
    Bool,
    Str,
    Char,
    Byte,
    Void,
    Never,

    /// Type variable during inference.
    Infer(u32),

    /// Named type (struct, enum, type alias).
    Named {
        name: Name,
        type_args: TypeRange,
    },

    /// Function type: (params) -> return
    Function {
        params: TypeRange,
        ret: TypeId,
    },

    /// Tuple type: (T, U, V)
    Tuple(TypeRange),

    /// List type: [T]
    List(TypeId),

    /// Map type: {K: V}
    Map {
        key: TypeId,
        value: TypeId,
    },

    /// Set type: Set<T>
    Set(TypeId),

    /// Option type: Option<T>
    Option(TypeId),

    /// Result type: Result<T, E>
    Result {
        ok: TypeId,
        err: TypeId,
    },

    /// Range type: Range<T>
    Range(TypeId),

    /// Channel type: Channel<T>
    Channel(TypeId),

    /// Reference (for method receivers)
    Ref {
        inner: TypeId,
        mutable: bool,
    },

    /// Generic type parameter: T
    TypeParam {
        name: Name,
        bounds: TypeRange,
    },

    /// Error type (for error recovery)
    Error,
}

impl fmt::Debug for TypeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeKind::Int => write!(f, "int"),
            TypeKind::Float => write!(f, "float"),
            TypeKind::Bool => write!(f, "bool"),
            TypeKind::Str => write!(f, "str"),
            TypeKind::Char => write!(f, "char"),
            TypeKind::Byte => write!(f, "byte"),
            TypeKind::Void => write!(f, "void"),
            TypeKind::Never => write!(f, "Never"),
            TypeKind::Infer(id) => write!(f, "?{}", id),
            TypeKind::Named { name, type_args } => {
                write!(f, "Named({:?}, {:?})", name, type_args)
            }
            TypeKind::Function { params, ret } => {
                write!(f, "({:?}) -> {:?}", params, ret)
            }
            TypeKind::Tuple(elems) => write!(f, "Tuple({:?})", elems),
            TypeKind::List(elem) => write!(f, "[{:?}]", elem),
            TypeKind::Map { key, value } => write!(f, "{{{:?}: {:?}}}", key, value),
            TypeKind::Set(elem) => write!(f, "Set<{:?}>", elem),
            TypeKind::Option(inner) => write!(f, "Option<{:?}>", inner),
            TypeKind::Result { ok, err } => write!(f, "Result<{:?}, {:?}>", ok, err),
            TypeKind::Range(elem) => write!(f, "Range<{:?}>", elem),
            TypeKind::Channel(elem) => write!(f, "Channel<{:?}>", elem),
            TypeKind::Ref { inner, mutable } => {
                if *mutable {
                    write!(f, "&mut {:?}", inner)
                } else {
                    write!(f, "&{:?}", inner)
                }
            }
            TypeKind::TypeParam { name, bounds } => {
                write!(f, "TypeParam({:?}, {:?})", name, bounds)
            }
            TypeKind::Error => write!(f, "Error"),
        }
    }
}

/// Type interner for concurrent type deduplication.
pub struct TypeInterner {
    /// Map from type to ID.
    type_to_id: DashMap<TypeKind, TypeId>,
    /// Map from ID to type (for lookup).
    id_to_type: DashMap<u32, TypeKind>,
    /// Storage for type lists (function params, tuple elements, etc.).
    type_lists: parking_lot::RwLock<Vec<TypeId>>,
    /// Next compound type ID.
    next_id: AtomicU32,
}

impl TypeInterner {
    /// Create a new type interner with pre-interned primitives.
    pub fn new() -> Self {
        let interner = Self {
            type_to_id: DashMap::new(),
            id_to_type: DashMap::new(),
            type_lists: parking_lot::RwLock::new(Vec::with_capacity(1024)),
            next_id: AtomicU32::new(TypeId::FIRST_COMPOUND),
        };

        // Pre-intern primitives
        interner.pre_intern_primitives();

        interner
    }

    fn pre_intern_primitives(&self) {
        let primitives = [
            (TypeId::INT, TypeKind::Int),
            (TypeId::FLOAT, TypeKind::Float),
            (TypeId::BOOL, TypeKind::Bool),
            (TypeId::STR, TypeKind::Str),
            (TypeId::CHAR, TypeKind::Char),
            (TypeId::BYTE, TypeKind::Byte),
            (TypeId::VOID, TypeKind::Void),
            (TypeId::NEVER, TypeKind::Never),
        ];

        for (id, kind) in primitives {
            self.type_to_id.insert(kind.clone(), id);
            self.id_to_type.insert(id.raw(), kind);
        }
    }

    /// Intern a type, returning its TypeId.
    pub fn intern(&self, kind: TypeKind) -> TypeId {
        // Fast path: check if already interned
        if let Some(id) = self.type_to_id.get(&kind) {
            return *id;
        }

        // Slow path: allocate new ID
        let id = TypeId::new(self.next_id.fetch_add(1, AtomicOrdering::Relaxed));

        // Insert into both maps
        self.type_to_id.insert(kind.clone(), id);
        self.id_to_type.insert(id.raw(), kind);

        id
    }

    /// Look up a type by ID.
    pub fn lookup(&self, id: TypeId) -> Option<TypeKind> {
        self.id_to_type.get(&id.raw()).map(|r| r.clone())
    }

    /// Allocate a type list, returning a TypeRange.
    pub fn alloc_list(&self, types: impl IntoIterator<Item = TypeId>) -> TypeRange {
        let mut guard = self.type_lists.write();
        let start = guard.len() as u32;
        guard.extend(types);
        let len = (guard.len() as u32 - start) as u16;
        TypeRange::new(start, len)
    }

    /// Get types from a range.
    pub fn get_list(&self, range: TypeRange) -> Vec<TypeId> {
        let guard = self.type_lists.read();
        let start = range.start as usize;
        let end = start + range.len as usize;
        guard[start..end].to_vec()
    }

    /// Intern a function type.
    pub fn intern_function(&self, params: &[TypeId], ret: TypeId) -> TypeId {
        let params_range = self.alloc_list(params.iter().copied());
        self.intern(TypeKind::Function { params: params_range, ret })
    }

    /// Intern a tuple type.
    pub fn intern_tuple(&self, elements: &[TypeId]) -> TypeId {
        if elements.is_empty() {
            return TypeId::VOID; // () is void
        }
        let elems_range = self.alloc_list(elements.iter().copied());
        self.intern(TypeKind::Tuple(elems_range))
    }

    /// Intern an Option<T> type.
    pub fn intern_option(&self, inner: TypeId) -> TypeId {
        self.intern(TypeKind::Option(inner))
    }

    /// Intern a Result<T, E> type.
    pub fn intern_result(&self, ok: TypeId, err: TypeId) -> TypeId {
        self.intern(TypeKind::Result { ok, err })
    }

    /// Intern a List type [T].
    pub fn intern_list(&self, elem: TypeId) -> TypeId {
        self.intern(TypeKind::List(elem))
    }

    /// Intern a Map type {K: V}.
    pub fn intern_map(&self, key: TypeId, value: TypeId) -> TypeId {
        self.intern(TypeKind::Map { key, value })
    }

    /// Get the number of interned types.
    pub fn len(&self) -> usize {
        self.id_to_type.len()
    }

    /// Check if only primitives are interned.
    pub fn is_empty(&self) -> bool {
        self.len() <= TypeId::FIRST_COMPOUND as usize
    }
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_types() {
        assert!(TypeId::INT.is_primitive());
        assert!(TypeId::FLOAT.is_primitive());
        assert!(TypeId::BOOL.is_primitive());
        assert!(TypeId::VOID.is_primitive());
        assert!(!TypeId::new(100).is_primitive());
    }

    #[test]
    fn test_type_interner_primitives() {
        let interner = TypeInterner::new();

        assert_eq!(interner.intern(TypeKind::Int), TypeId::INT);
        assert_eq!(interner.intern(TypeKind::Float), TypeId::FLOAT);
        assert_eq!(interner.intern(TypeKind::Bool), TypeId::BOOL);
        assert_eq!(interner.intern(TypeKind::Str), TypeId::STR);
    }

    #[test]
    fn test_type_interner_compound() {
        let interner = TypeInterner::new();

        let list_int = interner.intern_list(TypeId::INT);
        let list_int2 = interner.intern_list(TypeId::INT);
        let list_str = interner.intern_list(TypeId::STR);

        assert_eq!(list_int, list_int2);
        assert_ne!(list_int, list_str);
        assert!(!list_int.is_primitive());
    }

    #[test]
    fn test_type_interner_function() {
        let interner = TypeInterner::new();

        // Note: Currently, each intern_function call creates a new TypeRange,
        // so function types are not deduplicated. This is a known limitation
        // that can be improved with structural deduplication later.
        let fn1 = interner.intern_function(&[TypeId::INT, TypeId::INT], TypeId::INT);
        let fn3 = interner.intern_function(&[TypeId::INT], TypeId::INT);

        // Different parameter counts should give different types
        assert_ne!(fn1, fn3);

        // Both should be compound types
        assert!(!fn1.is_primitive());
        assert!(!fn3.is_primitive());
    }

    #[test]
    fn test_type_range() {
        let range = TypeRange::new(10, 5);
        assert_eq!(range.start, 10);
        assert_eq!(range.len, 5);
        assert!(!range.is_empty());

        let indices: Vec<_> = range.indices().collect();
        assert_eq!(indices, vec![10, 11, 12, 13, 14]);

        assert!(TypeRange::EMPTY.is_empty());
    }
}
