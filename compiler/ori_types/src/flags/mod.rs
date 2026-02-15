//! Pre-computed type metadata flags.
//!
//! `TypeFlags` are computed once at type interning time and cached,
//! enabling O(1) queries about type properties without traversal.
//!
//! # Design (from Rust's `rustc_type_ir`)
//!
//! Flags are organized into categories:
//! - **Presence flags**: What elements does this type contain?
//! - **Category flags**: What kind of type is this?
//! - **Optimization flags**: Can we skip certain operations?
//! - **Capability flags**: What effects does this type involve?

use bitflags::bitflags;

bitflags! {
    /// Pre-computed type properties for O(1) queries.
    ///
    /// Computed once at interning time, never recomputed.
    /// Used to gate expensive operations (substitution, occurs check, etc.).
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct TypeFlags: u32 {
        // === Presence Flags (bits 0-7) ===
        // Track what elements a type contains.

        /// Contains unbound type variables (unification targets).
        const HAS_VAR = 1 << 0;
        /// Contains bound/quantified type variables.
        const HAS_BOUND_VAR = 1 << 1;
        /// Contains rigid type variables (from annotations).
        const HAS_RIGID_VAR = 1 << 2;
        /// Contains the Error type (error propagation).
        const HAS_ERROR = 1 << 3;
        /// Contains inference placeholders.
        const HAS_INFER = 1 << 4;
        /// Contains Self type.
        const HAS_SELF = 1 << 5;
        /// Contains type projections (associated types).
        const HAS_PROJECTION = 1 << 6;

        // === Category Flags (bits 8-15) ===
        // Classify types for fast dispatch.

        /// Built-in primitive type (int, bool, etc.).
        const IS_PRIMITIVE = 1 << 8;
        /// Generic container type (List, Option, etc.).
        const IS_CONTAINER = 1 << 9;
        /// Function type.
        const IS_FUNCTION = 1 << 10;
        /// User-defined composite type (Struct, Enum, Tuple).
        const IS_COMPOSITE = 1 << 11;
        /// Named type reference.
        const IS_NAMED = 1 << 12;
        /// Type scheme (quantified).
        const IS_SCHEME = 1 << 13;

        // === Optimization Flags (bits 16-23) ===
        // Enable optimization shortcuts.

        /// Has variables needing substitution.
        const NEEDS_SUBST = 1 << 16;
        /// Fully resolved, no holes.
        const IS_RESOLVED = 1 << 17;
        /// Monomorphic, no generics.
        const IS_MONO = 1 << 18;
        /// Known to be a Copy type.
        const IS_COPYABLE = 1 << 19;

        // === Capability Flags (bits 24-31) ===
        // Track Ori's capability/effect information.

        /// Uses capabilities.
        const HAS_CAPABILITY = 1 << 24;
        /// Guaranteed pure (no effects).
        const IS_PURE = 1 << 25;
        /// Has IO effects.
        const HAS_IO = 1 << 26;
        /// Has async effects.
        const HAS_ASYNC = 1 << 27;
    }
}

impl TypeFlags {
    /// Flags that should propagate from child types to parents.
    ///
    /// When building a compound type, these flags are inherited
    /// from all child types via bitwise OR.
    pub const PROPAGATE_MASK: Self = Self::from_bits_truncate(
        Self::HAS_VAR.bits()
            | Self::HAS_BOUND_VAR.bits()
            | Self::HAS_RIGID_VAR.bits()
            | Self::HAS_ERROR.bits()
            | Self::HAS_INFER.bits()
            | Self::HAS_SELF.bits()
            | Self::HAS_PROJECTION.bits()
            | Self::NEEDS_SUBST.bits()
            | Self::HAS_CAPABILITY.bits()
            | Self::HAS_IO.bits()
            | Self::HAS_ASYNC.bits(),
    );

    /// Check if the type contains any kind of type variable.
    #[inline]
    pub const fn has_any_var(self) -> bool {
        self.intersects(
            Self::HAS_VAR
                .union(Self::HAS_BOUND_VAR)
                .union(Self::HAS_RIGID_VAR),
        )
    }

    /// Check if the type contains unbound variables.
    #[inline]
    pub const fn has_vars(self) -> bool {
        self.contains(Self::HAS_VAR)
    }

    /// Check if the type contains errors.
    #[inline]
    pub const fn has_errors(self) -> bool {
        self.contains(Self::HAS_ERROR)
    }

    /// Check if the type needs substitution work.
    #[inline]
    pub const fn needs_work(self) -> bool {
        self.contains(Self::NEEDS_SUBST)
    }

    /// Check if the type is fully resolved (no holes).
    #[inline]
    pub const fn is_resolved(self) -> bool {
        self.contains(Self::IS_RESOLVED)
    }

    /// Check if the type is monomorphic.
    #[inline]
    pub const fn is_mono(self) -> bool {
        self.contains(Self::IS_MONO)
    }

    /// Combine flags from child types (for compound types).
    #[inline]
    pub const fn propagate_from(child: Self) -> Self {
        Self::from_bits_truncate(child.bits() & Self::PROPAGATE_MASK.bits())
    }

    /// Combine propagated flags from multiple children.
    #[inline]
    pub fn propagate_all(children: impl IntoIterator<Item = Self>) -> Self {
        let mut result = Self::empty();
        for child in children {
            result = result.union(Self::propagate_from(child));
        }
        result
    }

    /// Get the primary category of a type based on its flags.
    ///
    /// Returns the most specific category that applies.
    /// Note: A type may have multiple category flags, but this returns a single value.
    #[inline]
    pub const fn category(self) -> TypeCategory {
        // Check in order of specificity
        if self.contains(Self::IS_PRIMITIVE) {
            TypeCategory::Primitive
        } else if self.contains(Self::IS_FUNCTION) {
            TypeCategory::Function
        } else if self.contains(Self::IS_CONTAINER) {
            TypeCategory::Container
        } else if self.contains(Self::IS_COMPOSITE) {
            TypeCategory::Composite
        } else if self.contains(Self::IS_SCHEME) {
            TypeCategory::Scheme
        } else if self.contains(Self::IS_NAMED) {
            TypeCategory::Named
        } else if self.intersects(
            Self::HAS_VAR
                .union(Self::HAS_BOUND_VAR)
                .union(Self::HAS_RIGID_VAR),
        ) {
            TypeCategory::Variable
        } else {
            TypeCategory::Unknown
        }
    }
}

/// Category of a type for pattern matching and dispatch.
///
/// Derived from [`TypeFlags`] category bits.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeCategory {
    /// Built-in primitive type (int, bool, str, etc.)
    Primitive,
    /// Function type `(params) -> ret`
    Function,
    /// Generic container (List, Option, Map, Result, etc.)
    Container,
    /// User-defined composite (Struct, Enum, Tuple)
    Composite,
    /// Type scheme (quantified/polymorphic type)
    Scheme,
    /// Named type reference
    Named,
    /// Type variable (unbound, bound, or rigid)
    Variable,
    /// Unknown or special type
    Unknown,
}

impl Default for TypeFlags {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests;
