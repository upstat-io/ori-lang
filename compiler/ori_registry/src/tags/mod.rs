//! Type identity tags and classification enums for the Ori type registry.
//!
//! This module defines the core discriminant types used throughout the registry:
//! [`TypeTag`] identifies builtin types, while companion enums like
//! [`MemoryStrategy`], [`Ownership`], and [`OpStrategy`] describe their
//! behavioral properties.
//!
//! All types in this module are `Copy`, `const`-constructible, and have no
//! dependencies. They live in `.rodata` at zero runtime cost.

mod return_tag;

pub use return_tag::{ReturnTag, TypeProjection};

use core::mem::size_of;

/// Universal identity tag for all builtin types in the registry.
///
/// This is the registry's type discriminant. It identifies WHAT type
/// something is, independent of type parameters (`List` vs `List<int>`),
/// phase representation (`Idx` vs `TypeInfo`), or memory layout.
///
/// Exhaustive: adding a new builtin type requires a new variant here,
/// which produces compile errors in every consuming phase's match arms.
///
/// # Variant count
///
/// Currently 23 variants. The `all()` method and tests enforce this count.
/// `#[repr(u8)]` guarantees a single-byte discriminant, enabling use as
/// an array index for O(1) lookup tables.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TypeTag {
    // Primitive value types (Copy semantics)
    /// 64-bit signed integer (`int`).
    Int,
    /// 64-bit IEEE 754 floating point (`float`).
    Float,
    /// Boolean (`bool`).
    Bool,
    /// Unicode scalar value (`char`).
    Char,
    /// 8-bit unsigned integer (`byte`).
    Byte,

    // Special value types (Copy semantics)
    /// Unit type `()` — the empty tuple, used for void-returning functions.
    Unit,
    /// Bottom type (`Never`) — uninhabited, coerces to any type.
    Never,
    /// Time duration in nanoseconds (`Duration`).
    Duration,
    /// Memory/data size in bytes (`Size`).
    Size,
    /// Comparison result: `Less`, `Equal`, or `Greater` (`Ordering`).
    Ordering,

    // Reference types (Arc semantics)
    /// UTF-8 string (`str`).
    Str,
    /// Error with trace information (`Error`).
    Error,

    // Generic containers (Arc semantics)
    /// Dynamically-sized list (`[T]`).
    List,
    /// Key-value map (`{K: V}`).
    Map,
    /// Unique-element set (`Set<T>`).
    Set,
    /// Integer range (`Range<T>`).
    Range,
    /// Fixed-size heterogeneous tuple (`(T, U, ...)`).
    Tuple,
    /// Optional value (`Option<T>`).
    Option,
    /// Success-or-error (`Result<T, E>`).
    Result,
    /// Communication channel (`Channel<T>`).
    Channel,

    // Callable/iterator types (Arc semantics)
    /// Function or closure (`(T) -> U`).
    Function,
    /// Forward iterator (`Iterator<T>`).
    Iterator,
    /// Bidirectional iterator (`DoubleEndedIterator<T>`).
    DoubleEndedIterator,
}

// Enforce that TypeTag fits in a single byte.
const _: () = assert!(size_of::<TypeTag>() == 1);

/// All `TypeTag` variants in declaration order.
///
/// Used for exhaustive enumeration in tests and query APIs.
/// Updating the enum requires updating this array — the
/// `type_tag_all_returns_correct_count` test enforces consistency.
const ALL_TYPE_TAGS: &[TypeTag] = &[
    TypeTag::Int,
    TypeTag::Float,
    TypeTag::Bool,
    TypeTag::Char,
    TypeTag::Byte,
    TypeTag::Unit,
    TypeTag::Never,
    TypeTag::Duration,
    TypeTag::Size,
    TypeTag::Ordering,
    TypeTag::Str,
    TypeTag::Error,
    TypeTag::List,
    TypeTag::Map,
    TypeTag::Set,
    TypeTag::Range,
    TypeTag::Tuple,
    TypeTag::Option,
    TypeTag::Result,
    TypeTag::Channel,
    TypeTag::Function,
    TypeTag::Iterator,
    TypeTag::DoubleEndedIterator,
];

impl TypeTag {
    /// Returns the Ori-level name for this type.
    ///
    /// These are the names that appear in Ori source code and error messages.
    /// Primitive types use lowercase (`"int"`, `"str"`), composite types use
    /// `PascalCase` (`"Option"`, `"Iterator"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Char => "char",
            Self::Byte => "byte",
            Self::Unit => "void",
            Self::Never => "Never",
            Self::Duration => "Duration",
            Self::Size => "Size",
            Self::Ordering => "Ordering",
            Self::Str => "str",
            Self::Error => "Error",
            Self::List => "List",
            Self::Map => "Map",
            Self::Set => "Set",
            Self::Range => "Range",
            Self::Tuple => "Tuple",
            Self::Option => "Option",
            Self::Result => "Result",
            Self::Channel => "Channel",
            Self::Function => "Function",
            Self::Iterator => "Iterator",
            Self::DoubleEndedIterator => "DoubleEndedIterator",
        }
    }

    /// Returns a slice of all `TypeTag` variants in declaration order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        ALL_TYPE_TAGS
    }

    /// Returns `true` for primitive value types: `int`, `float`, `bool`,
    /// `char`, `byte`.
    ///
    /// These are the five types that have direct hardware representation
    /// and full operator support. Does NOT include special value types
    /// like `Unit`, `Duration`, or `Ordering`.
    #[must_use]
    pub const fn is_primitive(self) -> bool {
        matches!(
            self,
            Self::Int | Self::Float | Self::Bool | Self::Char | Self::Byte
        )
    }

    /// Returns `true` for types that carry type parameters.
    ///
    /// Generic types: `List<T>`, `Map<K, V>`, `Set<T>`, `Range<T>`,
    /// `Tuple<...>`, `Option<T>`, `Result<T, E>`, `Channel<T>`,
    /// `Function<..>`, `Iterator<T>`, `DoubleEndedIterator<T>`.
    #[must_use]
    pub const fn is_generic(self) -> bool {
        matches!(
            self,
            Self::List
                | Self::Map
                | Self::Set
                | Self::Range
                | Self::Tuple
                | Self::Option
                | Self::Result
                | Self::Channel
                | Self::Function
                | Self::Iterator
                | Self::DoubleEndedIterator
        )
    }

    /// Returns the base type tag for DEI aliasing.
    ///
    /// `DoubleEndedIterator` maps to `Iterator` because the registry stores
    /// all iterator methods on a single `TypeDef` keyed by `TypeTag::Iterator`.
    /// The query API uses `base_type()` to resolve the alias before lookup,
    /// then applies `dei_only` filtering.
    ///
    /// All other variants return `self`.
    #[must_use]
    pub const fn base_type(self) -> Self {
        match self {
            Self::DoubleEndedIterator => Self::Iterator,
            other => other,
        }
    }
}

/// How values of a type are managed in memory.
///
/// This determines whether the ARC pipeline inserts retain/release
/// operations, and how the LLVM backend copies/moves values.
///
/// For generic types (`List`, `Option`, etc.), the memory strategy describes
/// the container's OWN strategy, not the transitive strategy of its
/// contents. A `List` is always `Arc` even if it contains only `int`.
/// Transitive classification (does `Option<int>` need RC?) is computed
/// by `ori_arc::ArcClassifier` from this base fact plus the instantiated
/// type parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemoryStrategy {
    /// Value type: bitwise copy, no reference counting.
    ///
    /// Values live in registers or on the stack. Copying is a memcpy.
    /// No destructor needed.
    ///
    /// Examples: `int`, `float`, `bool`, `byte`, `char`, `Unit`, `Never`,
    /// `Duration`, `Size`, `Ordering`.
    ///
    /// In LLVM: passed by value, no RC calls emitted.
    /// In ARC: `ArcClass::Scalar` (for this type alone; compound types
    /// containing only `Copy` children are also Scalar transitively).
    Copy,

    /// Reference-counted heap allocation.
    ///
    /// Values contain a pointer to heap-allocated memory with a reference
    /// count header. Copying increments the count (`ori_rc_inc`), dropping
    /// decrements it (`ori_rc_dec`), and when the count reaches zero the
    /// memory is freed.
    ///
    /// Examples: `str`, `Error`, `List`, `Map`, `Set`, `Channel`,
    /// `Function`, `Iterator`.
    ///
    /// In LLVM: retain/release calls around copies and drops.
    /// In ARC: `ArcClass::DefiniteRef`.
    Arc,

    /// Structural: memory strategy depends on contents.
    ///
    /// The container itself has no inherent memory management — its
    /// strategy is determined by the types of its fields/elements.
    /// A `(int, bool)` tuple is Copy; a `(str, [int])` tuple contains
    /// Arc types and needs retain/release.
    ///
    /// The `ori_arc::ArcClassifier` computes the transitive strategy
    /// at instantiation time from the container's `Structural` base
    /// strategy plus its concrete type parameters.
    ///
    /// Examples: `Tuple`, `Option<T>`, `Result<T, E>`.
    Structural,
}

// Enforce that MemoryStrategy fits in a single byte.
const _: () = assert!(size_of::<MemoryStrategy>() == 1);

/// How a method parameter or receiver is passed with respect to reference counting.
///
/// This determines whether the ARC pipeline emits `rc_inc` at call sites
/// and whether the callee is responsible for `rc_dec` on the parameter.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ownership {
    /// Borrowed: the callee reads but does not consume.
    ///
    /// No `rc_inc` at the call site. The callee MUST NOT store, return,
    /// or pass this value to an `Owned` parameter. The caller retains
    /// ownership and handles the eventual `rc_dec`.
    ///
    /// Analogous to Lean 4's `@&` (borrow) annotation and Swift's
    /// `borrowing` parameter convention.
    ///
    /// Most builtin methods borrow their receiver: `str.len()`,
    /// `list.contains()`, `int.to_str()`, `Ordering.is_less()`.
    Borrow,

    /// Owned: the callee takes ownership.
    ///
    /// The caller emits `rc_inc` before the call. The callee is
    /// responsible for the value's lifecycle — it may store, return,
    /// or pass it onwards. If the callee doesn't use it, it must
    /// `rc_dec` on exit.
    ///
    /// Used when the method incorporates the value into its result:
    /// `list.push(elem)` takes ownership of `elem`,
    /// `map.insert(key, value)` takes ownership of both.
    Owned,

    /// Copy: trivially copied because it's a value type.
    ///
    /// No `rc_inc` or `rc_dec` needed. The value is bitwise-copied
    /// at call sites. Semantically similar to `Borrow` (the callee
    /// reads the value), but `Copy` captures the *reason*: the type
    /// is a value type (`MemoryStrategy::Copy`), not a reference type
    /// that happens to be borrowed.
    ///
    /// Reserved for future use. Current convention: all primitive type
    /// method receivers use `Ownership::Borrow` (frozen decision 18).
    Copy,
}

// Enforce that Ownership fits in a single byte.
const _: () = assert!(size_of::<Ownership>() == 1);

/// How an operator is lowered to machine code for a specific type.
///
/// Each builtin type declares an `OpStrategy` for every operator it supports.
/// The LLVM backend reads this strategy and emits the corresponding
/// instructions, eliminating the scattered `if is_float` / `if is_str`
/// guard chains that currently live in `emit_binary_op()`.
///
/// The strategy carries enough information for the backend to emit correct
/// code without further type inspection. For `RuntimeCall`, the function
/// name is included so the backend just calls it. For instruction-level
/// strategies, the backend knows the exact LLVM instruction family.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpStrategy {
    /// Signed integer instructions.
    ///
    /// Arithmetic: `add`, `sub`, `mul`, `sdiv`, `srem`.
    /// Comparison: `icmp slt`, `icmp sgt`, `icmp sle`, `icmp sge`.
    /// Equality: `icmp eq`, `icmp ne`.
    /// Negation: `sub 0, x`.
    /// Bitwise: `and`, `or`, `xor`, `shl`, `ashr`.
    ///
    /// Used by: `int`, `Duration`, `Size`.
    IntInstr,

    /// Floating-point instructions.
    ///
    /// Arithmetic: `fadd`, `fsub`, `fmul`, `fdiv`, `frem`.
    /// Comparison: `fcmp olt`, `fcmp ogt`, `fcmp ole`, `fcmp oge`.
    /// Equality: `fcmp oeq`, `fcmp one`.
    /// Negation: `fneg`.
    ///
    /// Used by: `float`.
    FloatInstr,

    /// Unsigned integer comparison.
    ///
    /// Comparison: `icmp ult`, `icmp ugt`, `icmp ule`, `icmp uge`.
    /// Equality: `icmp eq`, `icmp ne` (same as signed for equality).
    /// No arithmetic operators — `byte` and `char` don't support `+`, `-`, etc.
    ///
    /// Used by: `byte`, `char`, `bool` (for ordering — `false < true`
    /// uses unsigned comparison since `false=0, true=1`).
    UnsignedCmp,

    /// Boolean logic instructions.
    ///
    /// And: `and`. Or: `or`. Xor: `xor`.
    /// Equality: `icmp eq`, `icmp ne`.
    /// No arithmetic, no ordering (ordering uses `UnsignedCmp`).
    ///
    /// Used by: `bool` (for logical operators `&&`, `||`).
    BoolLogic,

    /// Delegate to an `ori_rt` runtime function.
    ///
    /// The function name is the symbol in the runtime library that
    /// implements this operation. The LLVM backend emits a `call`
    /// instruction to this function.
    ///
    /// `returns_bool` indicates whether the runtime function returns
    /// `i1` (for equality/comparison predicates like `ori_str_eq`)
    /// vs the type's own representation (for operations like
    /// `ori_str_concat` which returns a new `str`).
    ///
    /// Used by: `str` (all operators delegate to runtime).
    RuntimeCall {
        /// The runtime function symbol name (e.g., `"ori_str_concat"`).
        fn_name: &'static str,
        /// `true` if the function returns `i1` (bool), `false` if it
        /// returns the same type as the operands.
        returns_bool: bool,
    },

    /// This operator is not supported for this type.
    ///
    /// Attempting to use this operator is a type error caught by the
    /// type checker. The LLVM backend should never encounter this
    /// variant — if it does, it's a compiler bug.
    Unsupported,
}

// RuntimeCall has &'static str + bool + discriminant + padding.
// 64-bit: (16 + 1 + 1 + padding) = 24 bytes.
// 32-bit (WASM): (8 + 1 + 1 + padding) = 12 bytes.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<OpStrategy>() == 24);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(size_of::<OpStrategy>() == 12);

/// How many type parameters a builtin type expects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeParamArity {
    /// Fixed number of type parameters (0, 1, or 2).
    ///
    /// `Fixed(0)` — primitives, non-generic compounds.
    /// `Fixed(1)` — `List<T>`, `Set<T>`, `Option<T>`, `Iterator<T>`, etc.
    /// `Fixed(2)` — `Map<K, V>`, `Result<T, E>`.
    Fixed(u8),

    /// Variadic type parameters (`Tuple`: 0-N elements, `Function`: arbitrary params).
    Variadic,
}

/// Whether a method is called on an instance or on the type itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MethodKind {
    /// Instance method: called as `value.method(args)`.
    /// Has an implicit `self` receiver.
    Instance,

    /// Associated function: called as `Type.method(args)`.
    /// No `self` receiver. Examples: `Duration.from_seconds(ns:)`,
    /// `Size.from_bytes(b:)`, `str.from_utf8(bytes:)`.
    Associated,
}

/// How an iterator adapter method affects `DoubleEndedIterator` capability.
///
/// Only meaningful for iterator adapter methods (`map`, `filter`, `take`, etc.).
/// Consumer methods (`fold`, `count`, `collect`) and non-iterator methods
/// use `NotApplicable`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DeiPropagation {
    /// Adapter preserves DEI capability.
    ///
    /// If the input is DEI, the output is also DEI.
    /// Examples: `map`, `filter`, `enumerate`, `chain` (if both inputs are DEI).
    Propagate,

    /// Adapter downgrades DEI to plain `Iterator`.
    ///
    /// Even if the input is DEI, the output is only `Iterator`.
    /// Examples: `take`, `skip`, `flatten`, `flat_map`, `cycle`.
    Downgrade,

    /// Not an adapter — this is a consumer or non-iterator method.
    ///
    /// Used for terminal operations (`fold`, `count`, `collect`, `for_each`)
    /// and all non-iterator methods.
    NotApplicable,
}

#[cfg(test)]
mod tests;
