//! Ownership annotations for ARC borrow inference.
//!
//! After borrow inference, every parameter in every function
//! gets an [`Ownership`] annotation: either [`Borrowed`](Ownership::Borrowed)
//! (callee receives no owner credit) or [`Owned`](Ownership::Owned) (one
//! logical owner transfers to the callee).
//!
//! These annotations drive backend-neutral ownership-event realization. A
//! counter-based physical plan may spell a required additional credit as a
//! retain; that mechanism is not part of [`Ownership`].

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::ArcVarId;

/// Per-variable ownership derived from SSA data flow.
///
/// Unlike [`Ownership`] which annotates only function parameters,
/// `DerivedOwnership` classifies **every** variable in a function body.
/// This enables realization to avoid redundant owner-credit events for values
/// borrowed from an already-live owner or freshly constructed with exactly one
/// logical owner and no prior aliases.
///
/// Computed by [`infer_derived_ownership()`](crate::borrow::infer_derived_ownership)
/// in a single forward pass over SSA blocks (no fixed-point needed since
/// each variable is defined exactly once in SSA form).
///
/// Historical influence: Lean 4's per-variable borrow tracking (`Lean.Compiler.IR.Borrow`)
/// and Swift's ownership SSA (`OwnershipKind`) SHAPE.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DerivedOwnership {
    /// The variable holds an owned value: function call results, literals,
    /// block params (which receive values via jump arguments).
    Owned,

    /// The variable is a projection or alias of another variable.
    /// No additional owner credit is needed while the source remains alive.
    BorrowedFrom(ArcVarId),

    /// The variable was freshly constructed (`Construct` / `PartialApply`),
    /// has exactly one logical owner, and has no prior aliases. This permits
    /// stronger reset/reuse reasoning without prescribing a counter.
    Fresh,
}

/// Ownership classification for a function parameter.
///
/// Historical influence: Lean 4's borrow-inference SHAPE: parameters are either borrowed
/// (callee receives no transferable owner credit) or owned (one logical owner
/// transfers to the callee).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum Ownership {
    /// The callee borrows the value — it will not store or return it.
    /// No owner-credit transfer occurs at the call site.
    Borrowed,

    /// The callee takes ownership — it may store, return, or pass the value
    /// to another owned parameter. The call transfers one logical owner credit.
    Owned,
}

/// A function parameter annotated with its ownership.
///
/// Produced by borrow inference and consumed by
/// AIMS realization to decide where logical ownership events belong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct AnnotatedParam {
    /// The parameter name (interned).
    pub name: Name,
    /// The parameter's type in the type pool.
    pub ty: Idx,
    /// Whether the parameter is borrowed or owned.
    pub ownership: Ownership,
}

/// A function signature annotated with ownership on all parameters.
///
/// This is the output of borrow inference for a single function.
/// AIMS realization reads these to decide call-site ownership events.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct AnnotatedSig {
    /// Annotated parameters (order matches the function definition).
    pub params: Vec<AnnotatedParam>,
    /// The function's return type.
    pub return_type: Idx,
}

#[cfg(test)]
mod tests;
