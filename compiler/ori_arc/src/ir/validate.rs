//! Validation utilities for ARC IR correctness.
//!
//! Provides post-lowering checks that enforce the cross-phase invariant
//! contract:
//!
//! > Type Checker → Codegen | All type variables resolved |
//! > No `Idx` with `Tag::Var` in typed IR
//!
//! And:
//!
//! > All type indices SHALL be fully resolved via `pool.resolve_fully(idx)`
//! > before LLVM type construction. Unresolved type variables (`Tag::Var`)
//! > SHALL NOT reach codegen.
//!
//! The functions in this module make that invariant self-enforcing at the
//! single upstream codegen seam (`process_arc_function` and
//! `declare_and_process_lambda` in `ori_llvm::codegen::function_compiler`).
//!
//! # Exemption Set
//!
//! The producer-side validator (`ori_types::check::validators`) exempts
//! `VarState::Generalized` and `VarState::Rigid` per the documented pool
//! divergence: the current pool stores generalized vars as
//! `Tag::Var(VarState::Generalized)` rather than `Tag::BoundVar`. This
//! consumer-side validator mirrors the exemption via an `exempt_var_ids`
//! parameter so generic function bodies do not fire spuriously until the
//! pool converts generalized vars to `Tag::BoundVar`.

use std::collections::HashSet;
use std::hash::BuildHasher;

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

/// A single unresolved type variable encountered in an `ArcFunction` at any
/// of the six type-bearing positions the walker covers: `var_types`,
/// `params.ty`, `return_type`, block-param tuples, instruction-operand types
/// on `Idx`-bearing `ArcInstr` variants, and terminator-operand types on
/// `ArcTerminator::Invoke`/`InvokeIndirect`.
///
/// Constructed by [`assert_no_unresolved_type_vars`] on invariant violation.
/// Wrapped by `ori_arc::verify::VerifyError::UnresolvedTypeVar(_)` for
/// propagation up the verification pipeline alongside existing `VerifyError`
/// variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnresolvedTypeVar {
    /// The `ArcFunction.name` where the violation was detected.
    pub function: Name,
    /// The specific `ArcVarId` whose type is unresolved. For the
    /// `return_type` position, this is [`ArcVarId::INVALID`] (no owning SSA
    /// var); the `idx` field identifies the violation precisely.
    pub var_id: ArcVarId,
    /// The raw type-pool index that resolved to `Tag::Var` or `Tag::Projection`.
    pub idx: Idx,
    /// The tag at the violating index (`Tag::Var` or `Tag::Projection` at
    /// emission time).
    pub tag: Tag,
}

/// Check that no `Tag::Var` (outside `exempt_var_ids`) or `Tag::Projection`
/// appears in any type-bearing position of `func`. PC-2 enforcement covers
/// every `Idx` field on `ArcFunction`, `ArcParam`, `ArcInstr`, and
/// `ArcTerminator`:
///
/// - `func.var_types[*]`              — SSA-variable types (primary storage)
/// - `func.params[*].ty`              — entry-block parameter types
/// - `func.return_type`               — declared return-type `Idx`
/// - `func.blocks[*].params[*].1`     — CFG-block parameter types (tuple
///   `.1` = `Idx`; `ArcBlock.params` is `Vec<(ArcVarId, Idx)>`)
/// - `func.blocks[*].body[*].ty`      — instruction operand types for
///   `Let | Apply | ApplyIndirect | PartialApply | Project | Construct |
///   Reuse | CollectionReuse | Select`
/// - `func.blocks[*].terminator.ty`   — terminator operand types for
///   `Invoke { ty }` and `InvokeIndirect { ty }`
///
/// Walkers over instructions and terminators use exhaustive matches with
/// no `_ => ()` arm: a future `Idx`-bearing variant addition is a
/// compile-time error here, forcing the PC-2 contract to be re-evaluated.
/// `var_types`-only scope would let a `Tag::Var` in a parameter, return,
/// instruction, or terminator position bypass the check entirely.
///
/// # Parameters
///
/// - `pool`: the frozen type pool (post-typecheck).
/// - `func`: the ARC function to validate.
/// - `interner`: string interner for rendering function name in diagnostics.
/// - `exempt_var_ids`: var IDs that are legitimately `Tag::Var` because they
///   are `VarState::Generalized` or `VarState::Rigid` (mirrors the producer
///   side `build_exempt_var_ids` in `ori_types::check::validators`). For
///   monomorphized functions this set is EMPTY. For non-monomorphized
///   function bodies (e.g., pre-mono JIT path) the caller populates it from
///   the owning `FunctionSig.scheme_var_ids`.
///
/// # Returns
///
/// `Ok(())` when the invariant holds. `Err(UnresolvedTypeVar)` with the FIRST
/// offending variable (deterministic iteration order).
///
/// # When to Call
///
/// Call this from `process_arc_function` + `declare_and_process_lambda` in
/// `ori_llvm`, BEFORE `ori_arc::run_arc_pipeline(...)` is invoked. The AIMS
/// pipeline mutates `arc_func` in place; calling after would validate the
/// wrong IR.
pub fn assert_no_unresolved_type_vars<S: BuildHasher>(
    pool: &Pool,
    func: &ArcFunction,
    interner: &StringInterner,
    exempt_var_ids: &HashSet<u32, S>,
) -> Result<(), UnresolvedTypeVar> {
    // Gate order mirrors the producer-side validator
    // (ori_types/check/validators/mod.rs): resolve_fully → tag check →
    // exemption set. `resolve_fully` is load-bearing — a `Tag::Var` in any
    // position may be a Link to a concrete type that fully resolves.
    let check_idx = |ty: Idx, reporting_var_id: ArcVarId| -> Result<(), UnresolvedTypeVar> {
        let resolved = pool.resolve_fully(ty);
        let tag = pool.tag(resolved);
        if matches!(tag, Tag::Var) {
            let var_id = pool.data(resolved);
            if exempt_var_ids.contains(&var_id) {
                return Ok(());
            }
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: reporting_var_id,
                idx: resolved,
                tag,
            });
        }
        if matches!(tag, Tag::Projection) {
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: reporting_var_id,
                idx: resolved,
                tag,
            });
        }
        Ok(())
    };

    // 1. SSA-variable storage (primary position).
    //
    // SSA variable indices are allocated by ArcLowerer::new_var() which returns
    // `ArcVarId(u32)`; the count is therefore architecturally bounded by u32::MAX.
    // See `compiler/ori_arc/src/ir/function.rs` for the newtype definition.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "ArcVarId is u32; var_types.len() cannot exceed u32::MAX by construction"
    )]
    for (raw_idx, &ty) in func.var_types.iter().enumerate() {
        check_idx(ty, ArcVarId::new(raw_idx as u32))?;
    }
    // 2. Entry-block parameters.
    for param in &func.params {
        check_idx(param.ty, param.var)?;
    }
    // 3. Return type. `ArcVarId::INVALID` is a sentinel — no owning SSA var.
    check_idx(func.return_type, ArcVarId::INVALID)?;
    // 4. CFG-block parameters (skip blocks[0]; it mirrors func.params).
    for block in func.blocks.iter().skip(1) {
        for &(var, ty) in &block.params {
            check_idx(ty, var)?;
        }
    }
    // 5. Instruction operands carrying `Idx` payloads. Exhaustive match —
    //    new `Idx`-bearing variants are compile-time errors here, forcing
    //    the PC-2 contract to be re-evaluated when the IR grows.
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let { dst, ty, .. }
                | ArcInstr::Apply { dst, ty, .. }
                | ArcInstr::ApplyIndirect { dst, ty, .. }
                | ArcInstr::PartialApply { dst, ty, .. }
                | ArcInstr::Project { dst, ty, .. }
                | ArcInstr::Construct { dst, ty, .. }
                | ArcInstr::Reuse { dst, ty, .. }
                | ArcInstr::CollectionReuse { dst, ty, .. }
                | ArcInstr::Select { dst, ty, .. } => check_idx(*ty, *dst)?,
                ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::BurdenDecPartial { .. }
                | ArcInstr::BurdenDecField { .. }
                | ArcInstr::BurdenDecVariant { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. } => {}
            }
        }
        // 6. Terminator operands carrying `Idx` payloads. Exhaustive match
        //    — `Invoke` and `InvokeIndirect` are the only `Idx`-bearing
        //    terminator variants today.
        match &block.terminator {
            ArcTerminator::Invoke { dst, ty, .. }
            | ArcTerminator::InvokeIndirect { dst, ty, .. } => check_idx(*ty, *dst)?,
            ArcTerminator::Return { .. }
            | ArcTerminator::Jump { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. }
            | ArcTerminator::Resume
            | ArcTerminator::Unreachable => {}
        }
    }

    let _ = interner; // reserved for future Name rendering in Display impl
    Ok(())
}

impl UnresolvedTypeVar {
    /// Render a user-facing diagnostic message for this violation.
    pub fn render(&self, interner: &StringInterner) -> String {
        format!(
            "Tag::{:?} reached codegen: function `{}`, ArcVarId({}) has \
             unresolved type index {:?}. This is a typeck PC-2 contract \
             violation.",
            self.tag,
            interner.lookup(self.function),
            self.var_id.index(),
            self.idx,
        )
    }
}

/// Thin PC-2 guard for non-`ArcFunction` call sites that hold only a raw
/// type [`Idx`] and an owning function/site [`Name`].
///
/// Used at codegen surfaces that bypass the `ArcFunction` realization path
/// and therefore cannot rely on [`assert_no_unresolved_type_vars`] —
/// derive synthesis is the canonical bypass call site. Panic and iterator
/// trampolines have their own upstream coverage (`panic_info_idx` is
/// validated by `validate_body_types` over the user `@panic`'s
/// `FunctionSig`; iterator element types are extracted from the parent
/// `ArcFunction`'s already-walked positions) and do NOT invoke this
/// helper. The caller supplies a single `type_idx` to validate rather
/// than a full `ArcFunction` worth of positions.
///
/// # Behavior
///
/// `Ok(())` when `idx` resolves to a non-`Tag::Var`, non-`Tag::Projection`
/// tag. `Err(UnresolvedTypeVar)` otherwise. The reported `var_id` is
/// [`ArcVarId::INVALID`] because there is no owning SSA variable; the
/// `function` field carries the caller-supplied site name for diagnostic
/// attribution. There is no exempt-var-ids parameter — call sites of this
/// helper operate on monomorphized concrete types where the exempt set is
/// always empty (matches the producer-side
/// `build_exempt_var_ids({}) = HashSet::new()` invariant for non-generic
/// scopes per `ori_types::check::validators`).
pub fn assert_no_unresolved_idx(
    pool: &Pool,
    idx: Idx,
    function: Name,
) -> Result<(), UnresolvedTypeVar> {
    let resolved = pool.resolve_fully(idx);
    let tag = pool.tag(resolved);
    if matches!(tag, Tag::Var) {
        return Err(UnresolvedTypeVar {
            function,
            var_id: ArcVarId::INVALID,
            idx: resolved,
            tag,
        });
    }
    if matches!(tag, Tag::Projection) {
        return Err(UnresolvedTypeVar {
            function,
            var_id: ArcVarId::INVALID,
            idx: resolved,
            tag,
        });
    }
    Ok(())
}

/// A single unresolved `Tag::BoundVar` encountered in lambda parameters
/// after monomorphization-resolution should have run.
///
/// Constructed by [`assert_no_unresolved_bound_vars_in_params`] on invariant
/// violation. Wrapped by `ori_arc::verify::VerifyError::UnresolvedBoundVar(_)`.
///
/// Distinct from [`UnresolvedTypeVar`]: PC-2 forbids `Tag::Var` / `Tag::Projection`
/// at codegen (the monomorphization-resolution invariant for `Tag::BoundVar`
/// is a sibling, not a restatement). + —
/// scheme-body `BoundVar` leaves SHALL be substituted with fresh `Var`s at
/// instantiation; any surviving `BoundVar` at codegen means monomorphization
/// did not finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnresolvedBoundVar {
    /// The lambda `ArcFunction.name` where the violation was detected.
    pub function: Name,
    /// The parameter `ArcVarId` whose type is `Tag::BoundVar`.
    pub var_id: ArcVarId,
    /// The raw type-pool index that resolved to `Tag::BoundVar`.
    pub idx: Idx,
}

/// Check that no `Tag::BoundVar` appears in lambda-parameter types.
///
/// Scoped to `lambda.params` specifically — this mirrors the invariant
/// previously enforced by the `debug_assert!` at `define_phase.rs::compile_lambda_arc`
/// entry. The check runs AFTER `resolve_all_lambda_bound_vars` has substituted
/// every bound var in the lambda's captures + user params; any surviving
/// `BoundVar` at this point means monomorphization did not finish.
///
/// # When to Call
///
/// Call from `compile_lambda_arc` in `ori_llvm::codegen::function_compiler::define_phase`
/// BEFORE `declare_and_process_lambda` / `run_arc_pipeline` so failures short-circuit
/// the emission of a lambda whose IR is not safe to process further.
///
/// # Returns
///
/// `Ok(())` when the invariant holds. `Err(UnresolvedBoundVar)` with the FIRST
/// offending parameter (deterministic iteration order).
pub fn assert_no_unresolved_bound_vars_in_params(
    pool: &Pool,
    func: &ArcFunction,
) -> Result<(), UnresolvedBoundVar> {
    for param in &func.params {
        let resolved = pool.resolve_fully(param.ty);
        if matches!(pool.tag(resolved), Tag::BoundVar) {
            return Err(UnresolvedBoundVar {
                function: func.name,
                var_id: param.var,
                idx: resolved,
            });
        }
    }
    Ok(())
}

impl UnresolvedBoundVar {
    /// Render a user-facing diagnostic message for this violation.
    pub fn render(&self, interner: &StringInterner) -> String {
        format!(
            "Tag::BoundVar reached codegen: lambda `{}`, ArcVarId({}) param has \
             unresolved bound-var at type index {:?}. Monomorphization-resolution \
             did not finish.",
            interner.lookup(self.function),
            self.var_id.index(),
            self.idx,
        )
    }
}

#[cfg(test)]
mod tests;
