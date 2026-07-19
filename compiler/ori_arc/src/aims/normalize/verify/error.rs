//! `TrmcVerificationError` — structural-invariant violation types for TRMC verification.

use ori_ir::Name;

use crate::ir::{ArcBlockId, ArcVarId};

/// Errors found during post-rewrite TRMC verification.
///
/// Each variant represents a specific structural invariant violation
/// in the rewritten IR. An empty error list means verification passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrmcVerificationError {
    /// Context variable used in a position that duplicates or aliases
    /// the reference (e.g., passed as an `Apply` argument, used in a
    /// `Construct`, or projected from).
    NonLinearContext { function: Name, var: ArcVarId },

    /// Context variable not provably unique at a use point. Constructed
    /// by [`verify_trmc_soundness`] after intraprocedural analysis
    /// converges on the rewritten function.
    NonUniqueContext {
        function: Name,
        var: ArcVarId,
        block: ArcBlockId,
    },

    /// Function has effect-handler interactions that break the unique linear
    /// chain via non-linear resumption — a rewritten function whose effects
    /// indicate non-linear resumption risk.
    #[expect(
        dead_code,
        reason = "out of scope: Ori has no effect handlers, so non-linear resumption \
                  cannot occur — this variant activates when effect handlers are added"
    )]
    EffectPurityViolation { function: Name },

    /// Residual self-recursive call found after rewrite. All self-recursion
    /// should have been converted to loop-back `Jump`s.
    NonTailRecursiveCall { function: Name, block: ArcBlockId },

    /// `LitValue::Null` found outside the prologue block. Null sentinels
    /// are only valid in the prologue's identity-context initialization.
    NullOutsidePrologue { function: Name, block: ArcBlockId },

    /// A `Jump` targeting the loop header passes the wrong number of
    /// arguments (expected `loop_header.params.len()`).
    LoopHeaderArgMismatch {
        function: Name,
        block: ArcBlockId,
        expected: usize,
        actual: usize,
    },

    /// A block uses a context variable (`ctx_has`, `ctx_res`, or
    /// `ctx_hole_obj`) but is not dominated by the loop header where
    /// those variables are defined.
    ContextVarDominanceViolation {
        function: Name,
        block: ArcBlockId,
        var: ArcVarId,
    },

    /// `BurdenInc(v)` and `BurdenDec(v)` counts diverge along a CFG
    /// path through the TRMC context region, for a `ShapeClass::ContextHole`
    /// variable. Structural verification per PL-10 (well-formedness) +
    /// VF-7 tier (a). On failure, the AIMS pipeline rolls back the TRMC
    /// rewrite per PL-10.
    ///
    /// `region` is the loop header that anchors the TRMC context region
    /// (where `ContextHole`-shaped block params are defined). `path` is
    /// the sequence of block ids that surfaces the divergent CFG edge.
    BurdenImbalance {
        function: Name,
        var: ArcVarId,
        region: ArcBlockId,
        path: Vec<ArcBlockId>,
    },
}

impl std::fmt::Display for TrmcVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonLinearContext { function, var } => {
                write!(
                    f,
                    "TRMC verify: non-linear context use of v{} in function {:?}",
                    var.raw(),
                    function.raw()
                )
            }
            Self::NonUniqueContext {
                function,
                var,
                block,
            } => {
                write!(
                    f,
                    "TRMC verify: context var v{} not unique at block {} in function {:?}",
                    var.raw(),
                    block.raw(),
                    function.raw()
                )
            }
            Self::EffectPurityViolation { function } => {
                write!(
                    f,
                    "TRMC verify: effect purity violation (non-linear resumption risk) in function {:?}",
                    function.raw()
                )
            }
            Self::NonTailRecursiveCall { function, block } => {
                write!(
                    f,
                    "TRMC verify: residual self-call in block {} of function {:?}",
                    block.raw(),
                    function.raw()
                )
            }
            Self::NullOutsidePrologue { function, block } => {
                write!(
                    f,
                    "TRMC verify: LitValue::Null in non-prologue block {} of function {:?}",
                    block.raw(),
                    function.raw()
                )
            }
            Self::LoopHeaderArgMismatch {
                function,
                block,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "TRMC verify: Jump to loop header in block {} passes {actual} args \
                     (expected {expected}) in function {:?}",
                    block.raw(),
                    function.raw()
                )
            }
            Self::ContextVarDominanceViolation {
                function,
                block,
                var,
            } => {
                write!(
                    f,
                    "TRMC verify: context var v{} used in block {} which is not dominated \
                     by the loop header in function {:?}",
                    var.raw(),
                    block.raw(),
                    function.raw()
                )
            }
            Self::BurdenImbalance {
                function,
                var,
                region,
                path,
            } => {
                write!(
                    f,
                    "TRMC verify: BurdenInc/BurdenDec imbalance on context-hole var v{} \
                     in region anchored at block {} along path {:?} of function {:?}",
                    var.raw(),
                    region.raw(),
                    path.iter().map(|b| b.raw()).collect::<Vec<_>>(),
                    function.raw()
                )
            }
        }
    }
}
