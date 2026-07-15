//! Method and parameter definitions for the Ori type registry.
//!
//! [`MethodDef`] is the central method specification consumed by all
//! compiler phases. [`ParamDef`] describes individual parameters.
//! Both are `const`-constructible and stored in `.rodata`.

use crate::tags::{DeiPropagation, MethodKind, Ownership, ReturnTag, TypeTag};

/// Exact identity of one method entry in the versioned builtin registry.
///
/// Construction is registry-owned, so downstream artifacts can carry a
/// compact semantic identity without retaining or switching on method text.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RegisteredMethodId {
    receiver: TypeTag,
    index: u16,
    arity: u8,
}

impl RegisteredMethodId {
    pub(crate) fn new(receiver: TypeTag, index: usize, arity: usize) -> Option<Self> {
        Some(Self {
            receiver,
            index: u16::try_from(index).ok()?,
            arity: u8::try_from(arity).ok()?,
        })
    }

    /// Receiver type whose registry method table owns this identity.
    #[must_use]
    pub const fn receiver(self) -> TypeTag {
        self.receiver
    }

    /// Version-local method-table position.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index as usize
    }

    /// Number of source operands, including the receiver.
    #[must_use]
    pub const fn arity(self) -> usize {
        self.arity as usize
    }
}

/// Backend-neutral semantic identity of an `Option` method.
///
/// These variants name source behavior only. They do not prescribe enum tags,
/// niche encodings, ownership mechanics, or a concrete backend operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OptionRuntime {
    /// Apply a closure to a present payload, preserving absence.
    AndThen,
    /// Produce the same logical value with independent result ownership.
    Clone,
    /// Compare variants and then present payloads.
    Compare,
    /// Render the structural debug form.
    Debug,
    /// Compare variants and present payloads for equality.
    Equals,
    /// Return a present payload or panic with a supplied message.
    Expect,
    /// Keep a present payload only when a predicate accepts it.
    Filter,
    /// Hash the logical variant and any present payload.
    Hash,
    /// Test whether the value is absent.
    IsNone,
    /// Test whether the value is present.
    IsSome,
    /// Transform a present payload, preserving absence.
    Map,
    /// Convert presence to success and absence to a supplied error.
    OkOr,
    /// Keep a present receiver or select another optional value.
    Or,
    /// Keep a present receiver or invoke a fallback closure.
    OrElse,
    /// Return a present payload or panic.
    Unwrap,
    /// Return a present payload or a supplied default.
    UnwrapOr,
}

impl OptionRuntime {
    /// Number of source operands, including the receiver.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Clone | Self::Debug | Self::Hash | Self::IsNone | Self::IsSome | Self::Unwrap => {
                1
            }
            Self::AndThen
            | Self::Compare
            | Self::Equals
            | Self::Expect
            | Self::Filter
            | Self::Map
            | Self::OkOr
            | Self::Or
            | Self::OrElse
            | Self::UnwrapOr => 2,
        }
    }
}

/// Backend-neutral semantic identity of a `Result` method.
///
/// The successful and error variants are logical names here; their physical
/// tags and payload layouts remain backend-plan decisions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResultRuntime {
    /// Apply a closure to a successful payload, preserving failure.
    AndThen,
    /// Produce the same logical value with independent result ownership.
    Clone,
    /// Compare variants and then their typed payloads.
    Compare,
    /// Render the structural debug form.
    Debug,
    /// Compare variants and their typed payloads for equality.
    Equals,
    /// Project an error into an optional value.
    Err,
    /// Return a successful payload or panic with a supplied message.
    Expect,
    /// Return an error payload or panic with a supplied message.
    ExpectErr,
    /// Hash the logical variant and its typed payload.
    Hash,
    /// Test whether the delegated error carries trace entries.
    HasTrace,
    /// Test whether the value is an error.
    IsErr,
    /// Test whether the value is successful.
    IsOk,
    /// Transform a successful payload, preserving failure.
    Map,
    /// Transform an error payload, preserving success.
    MapErr,
    /// Project a successful payload into an optional value.
    Ok,
    /// Preserve success or invoke a closure on the error payload.
    OrElse,
    /// Render the delegated error trace.
    Trace,
    /// Project delegated error trace entries.
    TraceEntries,
    /// Return a successful payload or panic.
    Unwrap,
    /// Return an error payload or panic.
    UnwrapErr,
    /// Return a successful payload or a supplied default.
    UnwrapOr,
}

impl ResultRuntime {
    /// Number of source operands, including the receiver.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Clone
            | Self::Debug
            | Self::Err
            | Self::Hash
            | Self::HasTrace
            | Self::IsErr
            | Self::IsOk
            | Self::Ok
            | Self::Trace
            | Self::TraceEntries
            | Self::Unwrap
            | Self::UnwrapErr => 1,
            Self::AndThen
            | Self::Compare
            | Self::Equals
            | Self::Expect
            | Self::ExpectErr
            | Self::Map
            | Self::MapErr
            | Self::OrElse
            | Self::UnwrapOr => 2,
        }
    }
}

/// Backend-neutral semantic identity of a string runtime method.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StrRuntime {
    Concat,
    Contains,
    StartsWith,
    EndsWith,
    IsEmpty,
    Trim,
    Uppercase,
    Lowercase,
    Split,
}

impl StrRuntime {
    /// Number of source operands, including the receiver.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::IsEmpty | Self::Trim | Self::Uppercase | Self::Lowercase => 1,
            Self::Concat | Self::Contains | Self::StartsWith | Self::EndsWith | Self::Split => 2,
        }
    }
}

/// Backend-neutral identity of a builtin method implemented by a runtime.
///
/// These are semantic identities, not backend entry-point names. Keeping them
/// in the zero-dependency registry lets realization select a runtime operation
/// without spelling-dependent switches in any backend.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MethodRuntime {
    /// Read the length of a value implementing `Len`.
    Length,
    /// Append one element to a persistent list.
    ListPush,
    /// Replace one element in a persistent list.
    ListSet,
    /// Insert one element at an index in a persistent list.
    ListInsert,
    /// Remove one element at an index from a persistent list.
    ListRemove,
    /// Insert one element at the front of a persistent list.
    ListPrepend,
    /// Create the receiver's canonical iterator.
    Iter,
    /// Render a value through its printable conversion.
    ToString,
    /// Logical `Option` behavior implemented by an admitted executor.
    Option(OptionRuntime),
    /// Logical `Result` behavior implemented by an admitted executor.
    Result(ResultRuntime),
    /// Logical string behavior implemented by an admitted executor.
    Str(StrRuntime),
}

impl MethodRuntime {
    /// Number of source operands, including an instance receiver.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Length | Self::Iter | Self::ToString => 1,
            Self::ListPush | Self::ListRemove | Self::ListPrepend => 2,
            Self::ListSet | Self::ListInsert => 3,
            Self::Option(operation) => operation.arity(),
            Self::Result(operation) => operation.arity(),
            Self::Str(operation) => operation.arity(),
        }
    }
}

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
/// All 11 fields are required. Every `MethodDef` literal must include all
/// fields.
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

    /// Runtime identity selected before a concrete backend, when one exists.
    pub runtime: Option<MethodRuntime>,

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

    /// Whether every admitted semantic and physical executor must implement
    /// this method.
    ///
    /// Current enforcement covers the evaluator and LLVM handlers. Admitting
    /// another executor requires extending that exhaustive check before the
    /// executor may accept this method. `false` means no physical handler is
    /// required by the current language surface.
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
            runtime: None,
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
    /// alongside methods that require executable behavior (`true`).
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
            runtime: None,
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
            runtime: None,
            trait_name: None,
            pure: true,
            backend_required: false,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }

    /// Convenience constructor for associated functions requiring executable behavior.
    ///
    /// Like [`associated`](Self::associated) but with `backend_required: true`.
    /// Used for associated functions that every admitted executor must support
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
            runtime: None,
            trait_name: None,
            pure: true,
            backend_required: true,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        }
    }

    /// Attach a backend-neutral runtime identity to this method definition.
    #[must_use]
    pub const fn with_runtime(mut self, runtime: MethodRuntime) -> Self {
        self.runtime = Some(runtime);
        self
    }
}

impl ParamDef {
    /// Common parameter for binary operations on primitive value types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Copy`.
    pub const SELF_TYPE: Self = Self {
        name: "other",
        ty: ReturnTag::SelfType,
        ownership: Ownership::Copy,
    };

    /// Common parameter for binary operations on reference types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Borrow`.
    pub const SELF_BORROW: Self = Self {
        name: "other",
        ty: ReturnTag::SelfType,
        ownership: Ownership::Borrow,
    };

    /// Common parameter for binary operations on structural types.
    ///
    /// Name `"other"`, type `ReturnTag::SelfType`, ownership `Owned`.
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
// MethodRuntime, padding. Verify it stays within a cache line.
const _: () = assert!(core::mem::size_of::<MethodDef>() <= 64);

#[cfg(test)]
mod tests;
