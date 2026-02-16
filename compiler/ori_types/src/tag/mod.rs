//! Type kind tag for tag-driven dispatch.
//!
//! Each type in the pool has a `Tag` that identifies its kind.
//! The tag determines how to interpret the associated `data` field.
//!
//! # Tag Categories
//!
//! Tags are organized into semantic ranges:
//! - 0-15: Primitives (data unused)
//! - 16-31: Simple containers (data = child Idx)
//! - 32-47: Two-child containers (data = extra index)
//! - 48-79: Complex types (data = extra index with length)
//! - 80-95: Named types (data = extra index)
//! - 96-111: Type variables (data = var id)
//! - 112-127: Type schemes (data = extra index)
//! - 240-255: Special types

use std::fmt;

/// Type kind discriminant (u8 = 256 possible kinds).
///
/// Determines how to interpret the `data` field in an `Item`.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Tag {
    // === Primitives (0-15) ===
    // data: unused (0)
    /// 64-bit signed integer.
    Int = 0,
    /// 64-bit floating point.
    Float = 1,
    /// Boolean.
    Bool = 2,
    /// UTF-8 string.
    Str = 3,
    /// Unicode scalar value.
    Char = 4,
    /// 8-bit unsigned integer.
    Byte = 5,
    /// Unit type `()`.
    Unit = 6,
    /// Never type (bottom, no values).
    Never = 7,
    /// Error placeholder (propagates silently).
    Error = 8,
    /// Time duration.
    Duration = 9,
    /// Memory size/count.
    Size = 10,
    /// Comparison ordering (Less, Equal, Greater).
    Ordering = 11,

    // Reserved: 12-15 for future primitives

    // === Simple Containers (16-31) ===
    // data: child Idx.raw()
    /// List type `[T]`.
    List = 16,
    /// Option type `T?`.
    Option = 17,
    /// Set type `{T}`.
    Set = 18,
    /// Channel type `chan<T>`.
    Channel = 19,
    /// Range type `range<T>`.
    Range = 20,
    /// Iterator type `Iterator<T>`.
    Iterator = 21,

    // Reserved: 22-31 for future simple containers

    // === Two-Child Containers (32-47) ===
    // data: index into extra[] with two consecutive Idx values
    /// Map type `{K: V}`.
    Map = 32,
    /// Result type `result<T, E>`.
    Result = 33,

    /// Borrowed reference type with lifetime (future: `&T`, `Slice<T>`).
    ///
    /// Reserved for future use — not yet constructed. Extra layout: `[inner_idx, lifetime_id]`.
    /// See `proposals/approved/low-level-future-proofing-proposal.md`.
    Borrowed = 34,

    // Reserved: 35-47 for future two-child types

    // === Complex Types (48-79) ===
    // data: index into extra[] with length prefix
    /// Function type `(P1, P2, ...) -> R`.
    Function = 48,
    /// Tuple type `(T1, T2, ...)`.
    Tuple = 49,
    /// Struct type with named fields.
    Struct = 50,
    /// Enum type with variants.
    Enum = 51,

    // Reserved: 52-79 for future complex types

    // === Named Types (80-95) ===
    // data: index into extra[]
    /// Named type reference (not yet resolved).
    Named = 80,
    /// Applied generic type `T<A, B>`.
    Applied = 81,
    /// Type alias.
    Alias = 82,

    // Reserved: 83-95 for future named types

    // === Type Variables (96-111) ===
    // data: variable id (into var_states)
    /// Unbound type variable (unification target).
    Var = 96,
    /// Bound/quantified type variable (in a scheme).
    BoundVar = 97,
    /// Rigid type variable (from annotation, cannot unify with concrete).
    RigidVar = 98,

    // Reserved: 99-111 for future variable types

    // === Type Schemes (112-127) ===
    // data: index into extra[]
    /// Quantified type scheme (forall vars. body).
    Scheme = 112,

    // Reserved: 113-127 for future scheme types

    // === Special (240-255) ===
    /// Type projection (associated type).
    Projection = 240,
    /// Module namespace type.
    ModuleNs = 241,
    /// Inference placeholder (to be filled).
    Infer = 254,
    /// Self type in trait context.
    SelfType = 255,
}

impl Tag {
    /// Check if this tag uses the extra array for data.
    #[inline]
    pub const fn uses_extra(self) -> bool {
        matches!(
            self,
            // Two-child containers
            Self::Map | Self::Result | Self::Borrowed |
            // Complex types
            Self::Function | Self::Tuple | Self::Struct | Self::Enum |
            // Named types
            Self::Named | Self::Applied | Self::Alias |
            // Schemes
            Self::Scheme |
            // Special
            Self::Projection
        )
    }

    /// Check if this tag represents a primitive type.
    #[inline]
    pub const fn is_primitive(self) -> bool {
        (self as u8) < 16
    }

    /// Check if this tag represents a container type.
    #[inline]
    pub const fn is_container(self) -> bool {
        let v = self as u8;
        v >= 16 && v < 48
    }

    /// Check if this tag represents a type variable.
    #[inline]
    pub const fn is_type_variable(self) -> bool {
        matches!(self, Self::Var | Self::BoundVar | Self::RigidVar)
    }

    /// Get the name of this tag as a static string.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Char => "char",
            Self::Byte => "byte",
            Self::Unit => "()",
            Self::Never => "never",
            Self::Error => "<error>",
            Self::Duration => "duration",
            Self::Size => "size",
            Self::Ordering => "ordering",
            Self::List => "list",
            Self::Option => "option",
            Self::Set => "set",
            Self::Channel => "chan",
            Self::Range => "range",
            Self::Iterator => "Iterator",
            Self::Map => "map",
            Self::Result => "result",
            Self::Borrowed => "borrowed",
            Self::Function => "function",
            Self::Tuple => "tuple",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Named => "named",
            Self::Applied => "applied",
            Self::Alias => "alias",
            Self::Var => "var",
            Self::BoundVar => "bound_var",
            Self::RigidVar => "rigid_var",
            Self::Scheme => "scheme",
            Self::Projection => "projection",
            Self::ModuleNs => "module",
            Self::Infer => "infer",
            Self::SelfType => "Self",
        }
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag::{}", self.name())
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// Compile-time size assertion: Tag must be exactly 1 byte
const _: () = assert!(std::mem::size_of::<Tag>() == 1);

#[cfg(test)]
mod tests;
