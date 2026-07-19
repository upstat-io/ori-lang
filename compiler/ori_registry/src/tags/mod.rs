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
/// The `all()` method and the `type_tag_all_returns_correct_count` test
/// are the single source of truth for the variant count.
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
/// This is a backend-neutral input to AIMS ownership realization and to each
/// executor's physical storage and copy/move projection.
///
/// For generic types (`List`, `Option`, etc.), the memory strategy describes
/// the container's OWN strategy, not the transitive strategy of its
/// contents. A `List` is always `Arc` even if it contains only `int`.
/// Transitive classification (does `Option<int>` need RC?) is computed
/// by `ori_arc::ArcClassifier` from this base fact plus the instantiated
/// type parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemoryStrategy {
    /// Value type: bitwise-copyable logical value with no counted ownership.
    ///
    /// Examples: `int`, `float`, `bool`, `byte`, `char`, `Unit`, `Never`,
    /// `Duration`, `Size`, `Ordering`.
    ///
    /// In AIMS: `ArcClass::Scalar` (for this type alone; compound types
    /// containing only `Copy` children are also Scalar transitively).
    /// Each executor chooses its own physical storage and calling convention.
    Copy,

    /// Logical value governed by counted ownership.
    ///
    /// Examples: `str`, `Error`, `List`, `Map`, `Set`, `Channel`,
    /// `Function`, `Iterator`.
    ///
    /// In AIMS: `ArcClass::DefiniteRef`; the realized ownership plan contains
    /// the required retain/release events. Pointer shape, headers, allocation,
    /// and event implementation belong to each executor's physical projection.
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
    /// Used for non-receiver parameters of value types (e.g., `int`
    /// params in factory functions). Receiver ownership for primitives
    /// uses `Ownership::Borrow`.
    Copy,
}

// Enforce that Ownership fits in a single byte.
const _: () = assert!(size_of::<Ownership>() == 1);

/// One operand's semantic ownership use at a primitive-operation boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveOperandUse {
    /// The operation reads the value and leaves its ownership obligation with
    /// the caller.
    Borrow,
    /// The operation takes one logical ownership obligation.
    Consume,
}

/// A compact set of primitive operand indices.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveOperandSet(u8);

impl PrimitiveOperandSet {
    /// No operands.
    pub const EMPTY: Self = Self(0);
    /// The first two operands.
    pub const FIRST_TWO: Self = Self(0b11);

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether an operand is in the set.
    #[must_use]
    pub const fn contains(self, operand: usize) -> bool {
        operand < u8::BITS as usize && self.0 & (1_u8 << operand) != 0
    }

    /// Whether the set names an operand outside `arity`.
    #[must_use]
    pub const fn fits_arity(self, arity: usize) -> bool {
        if arity >= u8::BITS as usize {
            true
        } else {
            self.0 < (1_u8 << arity)
        }
    }
}

/// Semantic ownership origin of a primitive result.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveResultOwnership {
    /// Non-reference-counted result outside the AIMS lattice carrier.
    Scalar,
    /// One independent owned result whose storage is not sourced from an
    /// input obligation.
    IndependentOwned,
    /// One independent owned result whose physical storage may come from one
    /// of the named consumed inputs or from an independent allocation.
    OwnedFromConsumedOrIndependent {
        /// Inputs eligible to fund physical storage takeover.
        eligible_inputs: PrimitiveOperandSet,
    },
    /// Result aliases one input and inherits its state.
    Alias {
        /// Aliased operand index.
        operand: u8,
    },
}

/// Physical allocation possibility, separate from result ownership.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveAllocationEffect {
    /// The primitive does not allocate.
    None,
    /// The primitive may allocate independent storage.
    MayAllocate,
    /// Allocation depends on the admitted physical strategy row.
    StrategyDependent,
}

/// Backend-neutral ownership descriptor for one primitive operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveDescriptor {
    /// Semantic result ownership.
    pub result: PrimitiveResultOwnership,
    /// One ownership use per operand.
    pub operand_uses: &'static [PrimitiveOperandUse],
    /// Allocation possibility, independent of logical result ownership.
    pub allocation: PrimitiveAllocationEffect,
}

impl PrimitiveDescriptor {
    /// Validate arity, index bounds, takeover consumption, and the permitted
    /// result/allocation combinations. Consumers must reject invalid metadata.
    #[must_use]
    pub fn is_valid_for(self, arity: usize) -> bool {
        if self.operand_uses.len() != arity {
            return false;
        }
        match (self.result, self.allocation) {
            (PrimitiveResultOwnership::Scalar, PrimitiveAllocationEffect::None)
            | (
                PrimitiveResultOwnership::IndependentOwned,
                PrimitiveAllocationEffect::MayAllocate,
            ) => true,
            (PrimitiveResultOwnership::Alias { operand }, PrimitiveAllocationEffect::None) => {
                usize::from(operand) < arity
            }
            (
                PrimitiveResultOwnership::OwnedFromConsumedOrIndependent { eligible_inputs },
                PrimitiveAllocationEffect::StrategyDependent,
            ) => {
                !eligible_inputs.is_empty()
                    && eligible_inputs.fits_arity(arity)
                    && self.operand_uses.iter().enumerate().all(|(index, use_)| {
                        !eligible_inputs.contains(index) || *use_ == PrimitiveOperandUse::Consume
                    })
            }
            _ => false,
        }
    }
}

const ONE_BORROWED_OPERAND: &[PrimitiveOperandUse] = &[PrimitiveOperandUse::Borrow];
const TWO_BORROWED_OPERANDS: &[PrimitiveOperandUse] =
    &[PrimitiveOperandUse::Borrow, PrimitiveOperandUse::Borrow];
const TWO_CONSUMED_OPERANDS: &[PrimitiveOperandUse] =
    &[PrimitiveOperandUse::Consume, PrimitiveOperandUse::Consume];

/// Typed identity for a runtime-backed primitive operator.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RuntimeOperator {
    /// UTF-8 string concatenation.
    StringConcat,
    /// UTF-8 string equality.
    StringEqual,
    /// UTF-8 string inequality.
    StringNotEqual,
    /// UTF-8 string ordering comparison.
    StringCompare,
    /// Persistent-list concatenation.
    ListConcat,
}

impl RuntimeOperator {
    /// Whether the source-level operator result is boolean.
    #[must_use]
    pub const fn returns_bool(self) -> bool {
        !matches!(self, Self::StringConcat | Self::ListConcat)
    }

    /// Canonical ownership descriptor for this runtime operation.
    #[must_use]
    pub const fn descriptor(self) -> PrimitiveDescriptor {
        match self {
            Self::StringConcat => PrimitiveDescriptor {
                result: PrimitiveResultOwnership::IndependentOwned,
                operand_uses: TWO_BORROWED_OPERANDS,
                allocation: PrimitiveAllocationEffect::MayAllocate,
            },
            Self::StringEqual | Self::StringNotEqual | Self::StringCompare => PrimitiveDescriptor {
                result: PrimitiveResultOwnership::Scalar,
                operand_uses: TWO_BORROWED_OPERANDS,
                allocation: PrimitiveAllocationEffect::None,
            },
            Self::ListConcat => PrimitiveDescriptor {
                result: PrimitiveResultOwnership::OwnedFromConsumedOrIndependent {
                    eligible_inputs: PrimitiveOperandSet::FIRST_TWO,
                },
                operand_uses: TWO_CONSUMED_OPERANDS,
                allocation: PrimitiveAllocationEffect::StrategyDependent,
            },
        }
    }
}

/// Backend-neutral executable strategy for an operator on a specific type.
///
/// Each builtin type declares one `OpStrategy` for every supported operator.
/// Evaluator, VM, and compiled projections consume that shared semantic
/// classification instead of rediscovering it from type spelling.
///
/// The strategy classifies source semantics. Each executor projects that
/// identity into its own instruction, bytecode, or runtime-call mechanism.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpStrategy {
    /// Signed integer arithmetic, equality, ordering, bitwise logic, shifts,
    /// and negation.
    SignedInteger,

    /// Floating-point arithmetic, equality, ordering, and negation.
    FloatingPoint,

    /// Unsigned equality and ordering for byte, character, and boolean values.
    UnsignedComparison,

    /// Boolean equality and eager logical operations.
    BooleanLogic,

    /// Recursive equality over a builtin compound value.
    StructuralEquality,

    /// Lexicographic ordering over a builtin compound value.
    StructuralOrdering,

    /// Delegate to one typed runtime operation.
    RuntimeCall(RuntimeOperator),

    /// This operator is not supported for this type.
    ///
    /// Attempting to use this operator is a type error caught by the type
    /// checker. No executor may encounter this variant in validated input;
    /// if one does, it is a compiler bug.
    Unsupported,
}

// Fieldless strategies and the niche-packed runtime identity fit in one byte.
const _: () = assert!(size_of::<OpStrategy>() == 1);

impl OpStrategy {
    /// Canonical primitive descriptor for this strategy and arity.
    /// Unsupported strategies or unsupported arities have no descriptor and
    /// must fail validation before AIMS or execution.
    #[must_use]
    pub const fn descriptor(self, arity: usize) -> Option<PrimitiveDescriptor> {
        match self {
            Self::RuntimeCall(runtime) => Some(runtime.descriptor()),
            Self::SignedInteger
            | Self::FloatingPoint
            | Self::UnsignedComparison
            | Self::BooleanLogic
            | Self::StructuralEquality
            | Self::StructuralOrdering => {
                let operand_uses = match arity {
                    1 => ONE_BORROWED_OPERAND,
                    2 => TWO_BORROWED_OPERANDS,
                    _ => return None,
                };
                Some(PrimitiveDescriptor {
                    result: PrimitiveResultOwnership::Scalar,
                    operand_uses,
                    allocation: PrimitiveAllocationEffect::None,
                })
            }
            Self::Unsupported => None,
        }
    }
}

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
