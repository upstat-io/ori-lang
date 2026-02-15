//! Ownership annotations for ARC borrow inference.
//!
//! After borrow inference (Section 06.2), every parameter in every function
//! gets an [`Ownership`] annotation: either [`Borrowed`](Ownership::Borrowed)
//! (callee will not retain) or [`Owned`](Ownership::Owned) (callee may retain,
//! caller must increment RC).
//!
//! These annotations drive RC insertion (Section 07) — borrowed parameters
//! skip `rc_inc` at call sites, reducing runtime overhead.

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::ArcVarId;

/// Per-variable ownership derived from SSA data flow.
///
/// Unlike [`Ownership`] which annotates only function parameters,
/// `DerivedOwnership` classifies **every** variable in a function body.
/// This enables RC insertion to skip `RcInc`/`RcDec` for variables that
/// are provably borrowed from an already-live variable or freshly
/// constructed with refcount = 1.
///
/// Computed by [`infer_derived_ownership()`](crate::borrow::infer_derived_ownership)
/// in a single forward pass over SSA blocks (no fixed-point needed since
/// each variable is defined exactly once in SSA form).
///
/// Inspired by Lean 4's per-variable borrow tracking (`Lean.Compiler.IR.Borrow`)
/// and Swift's ownership SSA (`OwnershipKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DerivedOwnership {
    /// The variable holds an owned value: function call results, literals,
    /// block params (which receive values via jump arguments).
    Owned,

    /// The variable is a projection or alias of another variable.
    /// No `RcInc` is needed as long as the source variable is alive.
    BorrowedFrom(ArcVarId),

    /// The variable was freshly constructed (`Construct` / `PartialApply`)
    /// and has refcount = 1. This means the first `RcDec` is guaranteed
    /// to deallocate, enabling more aggressive reset/reuse pairing.
    Fresh,
}

/// Ownership classification for a function parameter.
///
/// Inspired by Lean 4's borrow inference: parameters are either borrowed
/// (callee promises not to store the reference) or owned (callee may retain,
/// requiring the caller to increment the reference count).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum Ownership {
    /// The callee borrows the value — it will not store or return it.
    /// No `rc_inc` needed at the call site.
    Borrowed,

    /// The callee takes ownership — it may store, return, or pass the value
    /// to another owned parameter. The caller must `rc_inc` before the call.
    Owned,
}

/// A function parameter annotated with its ownership.
///
/// Produced by borrow inference (Section 06.2) and consumed by
/// RC insertion (Section 07) to decide where to place `rc_inc`/`rc_dec`.
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
/// RC insertion reads these to decide call-site RC operations.
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
