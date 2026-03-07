//! Method and parameter definitions for the Ori type registry.
//!
//! [`MethodDef`] is the central method specification consumed by all
//! compiler phases. [`ParamDef`] describes individual parameters.
//! Both are `const`-constructible and stored in `.rodata`.

use crate::tags::{DeiPropagation, MethodKind, Ownership, ReturnTag};

/// Definition of a method parameter (excluding the receiver).
///
/// Parameters are `const`-constructible so they can be embedded in
/// static method definitions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParamDef {
    /// The parameter name as it appears in documentation and error messages.
    pub name: &'static str,

    /// The parameter's type.
    pub ty: ReturnTag,

    /// How the parameter is passed with respect to reference counting.
    pub ownership: Ownership,
}

/// Complete specification of a single builtin method.
///
/// This is the single source of truth for a method's signature, ownership
/// semantics, and cross-phase metadata. One `MethodDef` per method per type,
/// all phases read from it.
///
/// # Fields
///
/// All 10 fields are required. Sections 03-07 MUST include all fields in
/// every `MethodDef` literal.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MethodDef {
    /// The method name as it appears in Ori source code.
    pub name: &'static str,

    /// How the receiver (`self`) is passed with respect to reference counting.
    ///
    /// For associated functions (`kind: Associated`), this field is
    /// conventionally `Ownership::Borrow` (placeholder — no receiver exists).
    pub receiver: Ownership,

    /// Parameters (excluding the receiver).
    pub params: &'static [ParamDef],

    /// The method's return type.
    pub returns: ReturnTag,

    /// The trait this method belongs to, if any.
    ///
    /// `None` for inherent methods. `Some("Eq")` for trait implementations.
    pub trait_name: Option<&'static str>,

    /// Whether this method has no observable side effects.
    ///
    /// `true` means no IO, no mutation, no global state — but MAY panic
    /// on invalid input. The optimizer MAY reorder, CSE, and hoist pure
    /// calls, but MUST NOT eliminate them if reachable (panic must fire).
    pub pure: bool,

    /// Whether both backends (eval and LLVM) must implement this method.
    ///
    /// `true` means Section 14 enforcement tests will fail if any backend
    /// is missing a handler. `false` means the method is intentionally
    /// backend-specific.
    pub backend_required: bool,

    /// Whether this is an instance method or associated function.
    pub kind: MethodKind,

    /// Whether this method is only available on `DoubleEndedIterator`.
    ///
    /// `true` for `next_back`, `rev`, `last`, `rfind`, `rfold`.
    /// `false` for all other methods.
    pub dei_only: bool,

    /// How this method affects DEI capability when used as an adapter.
    ///
    /// Only meaningful for iterator adapter methods. Consumers and
    /// non-iterator methods use `NotApplicable`.
    pub dei_propagation: DeiPropagation,
}

impl MethodDef {
    /// Convenience constructor for primitive type methods.
    ///
    /// Fills in the 5 fields that are constant for all primitive methods:
    /// `pure: true`, `backend_required: true`, `kind: Instance`,
    /// `dei_only: false`, `dei_propagation: NotApplicable`.
    ///
    /// Without this helper, each method literal requires ~12 lines, and
    /// `float.rs` (43 methods) would exceed the 500-line file size limit.
    #[must_use]
    pub const fn primitive(
        name: &'static str,
        params: &'static [ParamDef],
        returns: ReturnTag,
        trait_name: Option<&'static str>,
        receiver: Ownership,
    ) -> Self {
        Self {
            name,
            receiver,
            params,
            returns,
            trait_name,
            pure: true,
            backend_required: true,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }

    /// Convenience constructor for compound type instance methods.
    ///
    /// Like [`primitive`](Self::primitive) but with configurable
    /// `backend_required`. Compound types (Duration, Size, Ordering, Error)
    /// have methods that exist only in typeck (`backend_required: false`)
    /// alongside methods implemented in both eval and LLVM (`true`).
    #[must_use]
    pub const fn compound(
        name: &'static str,
        params: &'static [ParamDef],
        returns: ReturnTag,
        trait_name: Option<&'static str>,
        receiver: Ownership,
        backend_required: bool,
    ) -> Self {
        Self {
            name,
            receiver,
            params,
            returns,
            trait_name,
            pure: true,
            backend_required,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }

    /// Convenience constructor for associated functions (factories).
    ///
    /// Fills in: `receiver: Borrow` (irrelevant — no receiver),
    /// `trait_name: None`, `pure: true`, `backend_required: false`,
    /// `kind: Associated`, `dei_only: false`, `dei_propagation: NotApplicable`.
    #[must_use]
    pub const fn associated(
        name: &'static str,
        params: &'static [ParamDef],
        returns: ReturnTag,
    ) -> Self {
        Self {
            name,
            receiver: Ownership::Borrow,
            params,
            returns,
            trait_name: None,
            pure: true,
            backend_required: false,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }

    /// Convenience constructor for associated functions requiring both backends.
    ///
    /// Like [`associated`](Self::associated) but with `backend_required: true`.
    /// Used for associated functions that have eval and LLVM implementations
    /// (e.g., `str.from_utf8`, `str.from_utf8_unchecked`).
    #[must_use]
    pub const fn associated_backend(
        name: &'static str,
        params: &'static [ParamDef],
        returns: ReturnTag,
    ) -> Self {
        Self {
            name,
            receiver: Ownership::Borrow,
            params,
            returns,
            trait_name: None,
            pure: true,
            backend_required: true,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }
}

impl ParamDef {
    /// Common parameter for binary operations on primitive value types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Copy`.
    /// Used by `equals`, `compare`, `add`, `sub`, etc.
    pub const SELF_TYPE: Self = Self {
        name: "other",
        ty: ReturnTag::SelfType,
        ownership: Ownership::Copy,
    };

    /// Common parameter for binary operations on reference types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Borrow`.
    /// Used by `equals`, `compare` on str, list, map, set.
    pub const SELF_BORROW: Self = Self {
        name: "other",
        ty: ReturnTag::SelfType,
        ownership: Ownership::Borrow,
    };

    /// Common parameter for binary operations on structural types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Owned`.
    /// Used by `equals`, `compare` on option, result, tuple.
    pub const SELF_OWNED: Self = Self {
        name: "other",
        ty: ReturnTag::SelfType,
        ownership: Ownership::Owned,
    };
}

/// One `Self`-typed parameter with `Copy` ownership.
pub static ONE_SELF_COPY: [ParamDef; 1] = [ParamDef::SELF_TYPE];

/// Two `Self`-typed parameters with `Copy` ownership.
pub static TWO_SELF_COPY: [ParamDef; 2] = [ParamDef::SELF_TYPE, ParamDef::SELF_TYPE];

/// One `Self`-typed parameter with `Borrow` ownership.
pub static ONE_SELF_BORROW: [ParamDef; 1] = [ParamDef::SELF_BORROW];

/// One `Self`-typed parameter with `Owned` ownership.
pub static ONE_SELF_OWNED: [ParamDef; 1] = [ParamDef::SELF_OWNED];

// MethodDef: two fat pointers (name + params) = 32, ReturnTag ~4,
// Option<&str> = 16, Ownership + MethodKind + DeiPropagation + 2 bools = ~5,
// padding. Verify it stays within a cache line.
const _: () = assert!(core::mem::size_of::<MethodDef>() <= 64);

#[cfg(test)]
mod tests;
