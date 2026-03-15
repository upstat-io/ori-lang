//! Post-rewrite verification for TRMC-rewritten functions.
//!
//! After the TRMC 4-equation rewrite transforms a self-recursive
//! constructor-context function into a loop, this pass verifies that
//! the rewrite produced structurally valid IR. It catches bugs in the
//! rewrite implementation (not semantic soundness — that's the
//! intraprocedural analysis + post-convergence detection).
//!
//! # Checks
//!
//! 1. **No residual self-calls**: All recursion must be converted to
//!    loop-back `Jump`s. Any remaining `Apply`/`Invoke` to `func.name`
//!    is a rewrite bug.
//! 2. **Linear context usage**: The context variables (last 3 block
//!    params on the loop header) must not leak into positions that
//!    would duplicate or alias them — `Apply` args, `Construct` args,
//!    `PartialApply` args, `Project` sources, etc.
//! 3. **Null sentinel containment**: `LitValue::Null` must only appear
//!    in the prologue block's `Let` instructions, not elsewhere.
//! 4. **Loop-header param consistency**: Jumps targeting the loop
//!    header must pass exactly `params.len()` arguments.
//! 5. **Context var dominance**: Every block that uses a context variable
//!    (`ctx_has`, `ctx_res`, `ctx_hole_obj`) must be dominated by the
//!    loop header where those variables are defined as block params.
//!
//! # References
//!
//! - Leijen & Lorenzen, JFP 2025 — context laws (appctx), (appcomp)
//! - Section 13.5 of the AIMS plan

use ori_ir::Name;

use crate::aims::contract::ContextRegion;
use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::lattice::Uniqueness;
use crate::graph::DominatorTree;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue};

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

    /// Function has effect-handler interactions that could break the unique
    /// linear chain via non-linear resumption. Currently Ori has no effect
    /// handlers, so this variant is never constructed. When effect handlers
    /// are implemented, `verify_trmc_soundness()` will construct this when
    /// the rewritten function's effects indicate non-linear resumption risk.
    #[expect(
        dead_code,
        reason = "deferred to effect-handler implementation — Ori v1 has no effect handlers, \
                  so non-linear resumption cannot occur"
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
        }
    }
}

/// Verify structural correctness of a TRMC-rewritten function.
///
/// Returns an empty vec if verification passes. Any errors indicate
/// bugs in the rewrite implementation ([`super::rewrite::rewrite_trmc`]).
///
/// This function only checks structural properties of the IR — it does
/// not require uniqueness analysis or interprocedural contracts. Checks
/// that depend on external state (uniqueness, effect purity) are deferred
/// to Section 13.6 pipeline integration.
pub(crate) fn verify_trmc_rewrite(
    func: &ArcFunction,
    regions: &[ContextRegion],
) -> Vec<TrmcVerificationError> {
    if regions.is_empty() {
        return Vec::new();
    }

    let mut errors = Vec::new();

    // Check 1: No residual self-calls.
    check_no_residual_self_calls(func, &mut errors);

    // Find the loop header and context variables for remaining checks.
    if let Some(ctx) = find_context_vars(func) {
        // Check 2: Linear context usage.
        check_linear_context_usage(func, &ctx, &mut errors);

        // Check 3: Null sentinel containment.
        check_null_containment(func, &ctx, &mut errors);

        // Check 4: Loop-header arg consistency.
        check_loop_header_args(func, &ctx, &mut errors);

        // Check 5: Context var dominance — every block using context vars
        // is dominated by the loop header.
        check_context_var_dominance(func, &ctx, &mut errors);
    }

    errors
}

/// Context variables extracted from the rewritten function structure.
struct RewriteContext {
    /// The prologue block (current function entry).
    prologue: ArcBlockId,
    /// The loop header block (original entry, now has context block params).
    loop_header: ArcBlockId,
    /// `ctx_has` block param (bool flag: "do we have an accumulated context?").
    ctx_has: ArcVarId,
    /// `ctx_res` block param (accumulated result root).
    ctx_res: ArcVarId,
    /// `ctx_hole_obj` block param (object containing the current hole).
    ctx_hole_obj: ArcVarId,
}

/// Locate the prologue, loop header, and context block params.
///
/// Returns `None` if the function doesn't have the expected TRMC
/// rewrite structure (prologue → Jump → loop header with 3+ params).
fn find_context_vars(func: &ArcFunction) -> Option<RewriteContext> {
    let prologue_id = func.entry;
    let prologue_idx = prologue_id.index();
    if prologue_idx >= func.blocks.len() {
        return None;
    }

    // Prologue terminates with Jump to loop header.
    let loop_header_id = match &func.blocks[prologue_idx].terminator {
        ArcTerminator::Jump { target, .. } => *target,
        _ => return None,
    };
    let loop_header_idx = loop_header_id.index();
    if loop_header_idx >= func.blocks.len() {
        return None;
    }

    // Loop header must have exactly func.params.len() fresh param block
    // params + 3 context block params (ctx_has, ctx_res, ctx_hole_obj).
    // The rewrite sets this exact shape; any other count is a rewrite bug.
    let params = &func.blocks[loop_header_idx].params;
    let expected = func.params.len() + 3;
    if params.len() != expected {
        return None;
    }

    let n = params.len();
    Some(RewriteContext {
        prologue: prologue_id,
        loop_header: loop_header_id,
        ctx_has: params[n - 3].0,
        ctx_res: params[n - 2].0,
        ctx_hole_obj: params[n - 1].0,
    })
}

/// Check 1: No `Apply`/`Invoke` to `func.name` should remain.
fn check_no_residual_self_calls(func: &ArcFunction, errors: &mut Vec<TrmcVerificationError>) {
    let self_name = func.name;

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply { func: callee, .. } = instr {
                if *callee == self_name {
                    errors.push(TrmcVerificationError::NonTailRecursiveCall {
                        function: self_name,
                        block: block.id,
                    });
                }
            }
        }
        if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
            if *callee == self_name {
                errors.push(TrmcVerificationError::NonTailRecursiveCall {
                    function: self_name,
                    block: block.id,
                });
            }
        }
    }
}

/// Check 2: Context variables must not appear in positions that
/// duplicate or alias them.
///
/// Allowed positions:
/// - `ctx_has`: `Branch.cond`, `Jump.args` (to loop header)
/// - `ctx_res`: `Return.value`, `Jump.args`, `Set.value`,
///   `Construct.args` (hole field placeholder — deliberate part of rewrite)
/// - `ctx_hole_obj`: `Set.base`, `Jump.args` (to loop header)
///
/// Disallowed positions (non-linear / aliasing):
/// - `Apply.args`, `ApplyIndirect.args`, `PartialApply.args`
/// - `Reuse.args`, `CollectionReuse.args`
/// - `Construct.args` for `ctx_has` and `ctx_hole_obj` (but NOT `ctx_res`)
/// - `Project.value` (projecting from context var aliases it)
fn check_linear_context_usage(
    func: &ArcFunction,
    ctx: &RewriteContext,
    errors: &mut Vec<TrmcVerificationError>,
) {
    // ctx_res is allowed in Construct.args (hole field placeholder).
    // ctx_has and ctx_hole_obj are not.
    let call_disallowed = [ctx.ctx_has, ctx.ctx_res, ctx.ctx_hole_obj];
    let construct_disallowed = [ctx.ctx_has, ctx.ctx_hole_obj];

    for block in &func.blocks {
        for instr in &block.body {
            let disallowed = match instr {
                ArcInstr::Apply { args, .. }
                | ArcInstr::PartialApply { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => {
                    find_any_in_args(args, &call_disallowed)
                }
                ArcInstr::Construct { args, .. } => find_any_in_args(args, &construct_disallowed),
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    if call_disallowed.contains(closure) {
                        Some(*closure)
                    } else {
                        find_any_in_args(args, &call_disallowed)
                    }
                }
                ArcInstr::Project { value, .. } => {
                    if call_disallowed.contains(value) {
                        Some(*value)
                    } else {
                        None
                    }
                }
                // Allowed: Let, Set, RcInc, RcDec, IsShared, SetTag, Reset, Select.
                _ => None,
            };

            if let Some(var) = disallowed {
                errors.push(TrmcVerificationError::NonLinearContext {
                    function: func.name,
                    var,
                });
            }
        }
    }
}

/// Find the first variable from `disallowed` that appears in `args`.
fn find_any_in_args(args: &[ArcVarId], disallowed: &[ArcVarId]) -> Option<ArcVarId> {
    args.iter().find(|arg| disallowed.contains(arg)).copied()
}

/// Check 3: `LitValue::Null` must only appear in the prologue block.
fn check_null_containment(
    func: &ArcFunction,
    ctx: &RewriteContext,
    errors: &mut Vec<TrmcVerificationError>,
) {
    for block in &func.blocks {
        if block.id == ctx.prologue {
            continue;
        }
        for instr in &block.body {
            if let ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Null),
                ..
            } = instr
            {
                errors.push(TrmcVerificationError::NullOutsidePrologue {
                    function: func.name,
                    block: block.id,
                });
            }
        }
    }
}

/// Check 4: Every `Jump` targeting the loop header must pass exactly
/// `loop_header.params.len()` arguments.
fn check_loop_header_args(
    func: &ArcFunction,
    ctx: &RewriteContext,
    errors: &mut Vec<TrmcVerificationError>,
) {
    let expected = func.blocks[ctx.loop_header.index()].params.len();

    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            if *target == ctx.loop_header && args.len() != expected {
                errors.push(TrmcVerificationError::LoopHeaderArgMismatch {
                    function: func.name,
                    block: block.id,
                    expected,
                    actual: args.len(),
                });
            }
        }
    }
}

/// Check 5: Every reachable block that uses a context variable must be
/// dominated by the loop header where those variables are defined.
///
/// Context variables (`ctx_has`, `ctx_res`, `ctx_hole_obj`) are block
/// params on the loop header. Helper blocks (compose, apply-ctx) use
/// them without re-threading via block params, relying on SSA dominance.
/// This check validates that the dominance relationship holds.
///
/// Unreachable blocks (those with `idom == None` from the dominator tree)
/// are skipped — dead CFG debris should not trigger false positives.
fn check_context_var_dominance(
    func: &ArcFunction,
    ctx: &RewriteContext,
    errors: &mut Vec<TrmcVerificationError>,
) {
    let dom_tree = DominatorTree::build(func);
    let ctx_vars = [ctx.ctx_has, ctx.ctx_res, ctx.ctx_hole_obj];

    for block in &func.blocks {
        // Skip the prologue and loop header — prologue doesn't use ctx
        // vars (only initializes them), and the header defines them.
        if block.id == ctx.prologue || block.id == ctx.loop_header {
            continue;
        }

        // Skip unreachable blocks (idom == None).
        if dom_tree.idom[block.id.index()].is_none() {
            continue;
        }

        // Use the IR's own used_vars() methods for completeness.
        let mut found_var = None;
        for instr in &block.body {
            for v in instr.used_vars() {
                if ctx_vars.contains(&v) {
                    found_var = Some(v);
                    break;
                }
            }
            if found_var.is_some() {
                break;
            }
        }
        if found_var.is_none() {
            for v in block.terminator.used_vars() {
                if ctx_vars.contains(&v) {
                    found_var = Some(v);
                    break;
                }
            }
        }

        if let Some(used_var) = found_var {
            if !dom_tree.dominates(ctx.loop_header, block.id) {
                errors.push(TrmcVerificationError::ContextVarDominanceViolation {
                    function: func.name,
                    block: block.id,
                    var: used_var,
                });
            }
        }
    }
}

/// Verify semantic soundness of a TRMC-rewritten function.
///
/// Unlike [`verify_trmc_rewrite`] which checks structural properties,
/// this function checks **semantic** properties that require the converged
/// intraprocedural analysis state:
///
/// - **Uniqueness**: Context variables (`ctx_res`, `ctx_hole_obj`) must
///   have `Uniqueness::Unique` at every block where they are used in a
///   `Set` instruction. This is the core soundness condition (Lemma 2,
///   Leijen & Lorenzen JFP 2025).
///
/// This function runs AFTER `analyze_function()` (step 4a in the pipeline),
/// NOT during the rewrite phase (step 3a). It uses block-entry state as a
/// conservative approximation — if the context var is `Unique` at block
/// entry, it remains unique for all instructions in the block (no aliasing
/// can occur, which is separately enforced by `check_linear_context_usage`).
///
/// Returns an empty vec if the rewrite is semantically sound.
pub(crate) fn verify_trmc_soundness(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> Vec<TrmcVerificationError> {
    let mut errors = Vec::new();

    let Some(ctx) = find_context_vars(func) else {
        // Can't find rewrite structure — nothing to verify.
        return errors;
    };

    // Context vars that require uniqueness: ctx_res and ctx_hole_obj.
    // ctx_has is a bool flag — it doesn't need uniqueness.
    let uniqueness_required = [ctx.ctx_res, ctx.ctx_hole_obj];

    for block in &func.blocks {
        // Skip prologue and loop header — prologue initializes with Null
        // sentinels, and loop header defines the vars.
        if block.id == ctx.prologue || block.id == ctx.loop_header {
            continue;
        }

        // Check if this block uses any uniqueness-required context var
        // in body instructions or the terminator.
        let uses_ctx = block.body.iter().any(|instr| {
            instr
                .used_vars()
                .iter()
                .any(|v| uniqueness_required.contains(v))
        }) || block
            .terminator
            .used_vars()
            .iter()
            .any(|v| uniqueness_required.contains(v));

        if !uses_ctx {
            continue;
        }

        // Check uniqueness at block entry for each context var used.
        // Conservative: if unique at block entry, it stays unique for all
        // instructions (linear context usage is enforced by check 2).
        for &var in &uniqueness_required {
            let used = block
                .body
                .iter()
                .any(|instr| instr.used_vars().contains(&var))
                || block.terminator.used_vars().contains(&var);
            if !used {
                continue;
            }

            let state = state_map.var_state_at_block_entry(block.id, var);
            if state.uniqueness != Uniqueness::Unique {
                errors.push(TrmcVerificationError::NonUniqueContext {
                    function: func.name,
                    var,
                    block: block.id,
                });
            }
        }
    }

    errors
}
