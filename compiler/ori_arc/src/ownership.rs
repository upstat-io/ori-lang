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
mod tests {
    use ori_ir::Name;
    use ori_types::Idx;

    use crate::ir::ArcVarId;

    use super::*;

    #[test]
    fn ownership_inequality() {
        assert_ne!(Ownership::Borrowed, Ownership::Owned);
    }

    #[test]
    fn ownership_is_copy() {
        let o = Ownership::Borrowed;
        let o2 = o;
        // Both are valid — Copy semantics.
        assert_eq!(o, o2);
    }

    // DerivedOwnership tests

    #[test]
    fn derived_ownership_variants() {
        let owned = DerivedOwnership::Owned;
        let borrowed = DerivedOwnership::BorrowedFrom(ArcVarId::new(3));
        let fresh = DerivedOwnership::Fresh;
        assert_ne!(owned, borrowed);
        assert_ne!(owned, fresh);
        assert_ne!(borrowed, fresh);
    }

    #[test]
    fn derived_ownership_is_copy() {
        let d = DerivedOwnership::BorrowedFrom(ArcVarId::new(7));
        let d2 = d;
        assert_eq!(d, d2);
    }

    #[test]
    fn derived_borrowed_from_carries_source() {
        let src = ArcVarId::new(42);
        let d = DerivedOwnership::BorrowedFrom(src);
        match d {
            DerivedOwnership::BorrowedFrom(v) => assert_eq!(v, src),
            _ => panic!("expected BorrowedFrom"),
        }
    }

    #[test]
    fn derived_ownership_equality_by_source() {
        let a = DerivedOwnership::BorrowedFrom(ArcVarId::new(1));
        let b = DerivedOwnership::BorrowedFrom(ArcVarId::new(1));
        let c = DerivedOwnership::BorrowedFrom(ArcVarId::new(2));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn annotated_param_construction() {
        let param = AnnotatedParam {
            name: Name::from_raw(1),
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        };
        assert_eq!(param.name, Name::from_raw(1));
        assert_eq!(param.ty, Idx::INT);
        assert_eq!(param.ownership, Ownership::Borrowed);
    }

    #[test]
    fn annotated_param_equality() {
        let a = AnnotatedParam {
            name: Name::from_raw(1),
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        };
        let b = AnnotatedParam {
            name: Name::from_raw(1),
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        };
        let c = AnnotatedParam {
            name: Name::from_raw(1),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn annotated_sig_construction() {
        let sig = AnnotatedSig {
            params: vec![
                AnnotatedParam {
                    name: Name::from_raw(1),
                    ty: Idx::STR,
                    ownership: Ownership::Borrowed,
                },
                AnnotatedParam {
                    name: Name::from_raw(2),
                    ty: Idx::INT,
                    ownership: Ownership::Owned,
                },
            ],
            return_type: Idx::BOOL,
        };
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
        assert_eq!(sig.params[1].ownership, Ownership::Owned);
        assert_eq!(sig.return_type, Idx::BOOL);
    }

    #[test]
    fn annotated_sig_empty_params() {
        let sig = AnnotatedSig {
            params: vec![],
            return_type: Idx::UNIT,
        };
        assert!(sig.params.is_empty());
        assert_eq!(sig.return_type, Idx::UNIT);
    }
}
