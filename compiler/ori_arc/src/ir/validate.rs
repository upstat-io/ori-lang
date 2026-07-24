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
//! closed ARC-program ingress before [`crate::realize_closed_program`]. Any
//! remaining compiler-driver or LLVM calls are transitional redundant guards,
//! not backend-owned validation seams.
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

/// Gate order mirrors producer-side validator (`ori_types/check/validators/mod.rs`):
/// `resolve_fully` → tag check → exemption set. `resolve_fully` load-bearing —
/// `Tag::Var` in any position may be a Link to a concrete type that fully resolves.
fn check_unresolved_idx<S: BuildHasher>(
    pool: &Pool,
    ty: Idx,
    func_name: Name,
    reporting_var_id: ArcVarId,
    exempt_var_ids: &HashSet<u32, S>,
) -> Result<(), UnresolvedTypeVar> {
    let resolved = pool.resolve_fully(ty);
    let tag = pool.tag(resolved);
    if matches!(tag, Tag::Var) {
        let var_id = pool.data(resolved);
        if exempt_var_ids.contains(&var_id) {
            return Ok(());
        }
        return Err(UnresolvedTypeVar {
            function: func_name,
            var_id: reporting_var_id,
            idx: resolved,
            tag,
        });
    }
    if matches!(tag, Tag::Projection) {
        return Err(UnresolvedTypeVar {
            function: func_name,
            var_id: reporting_var_id,
            idx: resolved,
            tag,
        });
    }
    Ok(())
}

/// Return one instruction's type-bearing destination, when present.
/// Exhaustive matching makes new ARC variants re-evaluate this shared seam.
fn instruction_type_site(instr: &ArcInstr) -> Option<(Idx, ArcVarId)> {
    match instr {
        ArcInstr::Let { dst, ty, .. }
        | ArcInstr::Apply { dst, ty, .. }
        | ArcInstr::ApplyIndirect { dst, ty, .. }
        | ArcInstr::PartialApply { dst, ty, .. }
        | ArcInstr::Project { dst, ty, .. }
        | ArcInstr::Construct { dst, ty, .. }
        | ArcInstr::Reuse { dst, ty, .. }
        | ArcInstr::CollectionReuse { dst, ty, .. }
        | ArcInstr::Select { dst, ty, .. } => Some((*ty, *dst)),
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::RcDecPartial { .. }
        | ArcInstr::RcDecField { .. }
        | ArcInstr::RcDecVariant { .. }
        | ArcInstr::BurdenInc { .. }
        | ArcInstr::BurdenDec { .. }
        | ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. }
        | ArcInstr::IsShared { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. } => None,
    }
}

/// Return one terminator's type-bearing destination, when present.
fn terminator_type_site(terminator: &ArcTerminator) -> Option<(Idx, ArcVarId)> {
    match terminator {
        ArcTerminator::Invoke { dst, ty, .. } | ArcTerminator::InvokeIndirect { dst, ty, .. } => {
            Some((*ty, *dst))
        }
        ArcTerminator::Return { .. }
        | ArcTerminator::Jump { .. }
        | ArcTerminator::Branch { .. }
        | ArcTerminator::Switch { .. }
        | ArcTerminator::Resume
        | ArcTerminator::Unreachable => None,
    }
}

/// Return one instruction's mutable type-bearing destination, when present.
///
/// This is the mutation counterpart to [`instruction_type_site`]. Keeping the
/// exhaustive variant list beside the validation walker makes type rewrites
/// use the same ARC surface that the closure gates validate.
fn instruction_type_site_mut(instr: &mut ArcInstr) -> Option<(&mut Idx, ArcVarId)> {
    match instr {
        ArcInstr::Let { dst, ty, .. }
        | ArcInstr::Apply { dst, ty, .. }
        | ArcInstr::ApplyIndirect { dst, ty, .. }
        | ArcInstr::PartialApply { dst, ty, .. }
        | ArcInstr::Project { dst, ty, .. }
        | ArcInstr::Construct { dst, ty, .. }
        | ArcInstr::Reuse { dst, ty, .. }
        | ArcInstr::CollectionReuse { dst, ty, .. }
        | ArcInstr::Select { dst, ty, .. } => Some((ty, *dst)),
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::RcDecPartial { .. }
        | ArcInstr::RcDecField { .. }
        | ArcInstr::RcDecVariant { .. }
        | ArcInstr::BurdenInc { .. }
        | ArcInstr::BurdenDec { .. }
        | ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. }
        | ArcInstr::IsShared { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. } => None,
    }
}

/// Return one terminator's mutable type-bearing destination, when present.
fn terminator_type_site_mut(terminator: &mut ArcTerminator) -> Option<(&mut Idx, ArcVarId)> {
    match terminator {
        ArcTerminator::Invoke { dst, ty, .. } | ArcTerminator::InvokeIndirect { dst, ty, .. } => {
            Some((ty, *dst))
        }
        ArcTerminator::Return { .. }
        | ArcTerminator::Jump { .. }
        | ArcTerminator::Branch { .. }
        | ArcTerminator::Switch { .. }
        | ArcTerminator::Resume
        | ArcTerminator::Unreachable => None,
    }
}

/// Rewrite every type-bearing ARC position in deterministic order.
///
/// Shared pre-AIMS transformations use this rather than maintaining
/// instruction-specific fixup lists. The reporting variable is supplied so a
/// rewrite may remain provenance-aware when the same type index appears at
/// several SSA sites.
#[expect(
    clippy::cast_possible_truncation,
    reason = "ArcFunction variable tables are indexed by u32-backed ArcVarId"
)]
pub(crate) fn rewrite_type_sites(
    func: &mut ArcFunction,
    mut rewrite: impl FnMut(Idx, ArcVarId) -> Idx,
) {
    for (raw_index, ty) in func.var_types.iter_mut().enumerate() {
        *ty = rewrite(*ty, ArcVarId::new(raw_index as u32));
    }
    for parameter in &mut func.params {
        parameter.ty = rewrite(parameter.ty, parameter.var);
    }
    func.return_type = rewrite(func.return_type, ArcVarId::INVALID);
    for fact in &mut func.method_call_facts {
        fact.receiver_type = rewrite(fact.receiver_type, fact.destination);
    }
    for block in &mut func.blocks {
        for (var, ty) in &mut block.params {
            *ty = rewrite(*ty, *var);
        }
        for instruction in &mut block.body {
            if let Some((ty, var)) = instruction_type_site_mut(instruction) {
                *ty = rewrite(*ty, var);
            }
        }
        if let Some((ty, var)) = terminator_type_site_mut(&mut block.terminator) {
            *ty = rewrite(*ty, var);
        }
    }
}

/// Visit every type-bearing ARC position in deterministic order.
#[expect(
    clippy::cast_possible_truncation,
    reason = "ArcFunction variable tables are indexed by u32-backed ArcVarId"
)]
fn validate_type_sites<E>(
    func: &ArcFunction,
    mut validate: impl FnMut(Idx, ArcVarId) -> Result<(), E>,
) -> Result<(), E> {
    for (raw_index, &ty) in func.var_types.iter().enumerate() {
        validate(ty, ArcVarId::new(raw_index as u32))?;
    }
    for parameter in &func.params {
        validate(parameter.ty, parameter.var)?;
    }
    validate(func.return_type, ArcVarId::INVALID)?;
    for fact in &func.method_call_facts {
        validate(fact.receiver_type, fact.destination)?;
    }
    for block in func.blocks.iter().skip(1) {
        for &(var, ty) in &block.params {
            validate(ty, var)?;
        }
    }
    for block in &func.blocks {
        for instruction in &block.body {
            if let Some((ty, var)) = instruction_type_site(instruction) {
                validate(ty, var)?;
            }
        }
        if let Some((ty, var)) = terminator_type_site(&block.terminator) {
            validate(ty, var)?;
        }
    }
    Ok(())
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
/// Call this at the shared ARC ingress, BEFORE
/// [`crate::realize_closed_program`] is invoked. Every physical executor must
/// consume that same validated artifact rather than define a backend-local
/// validation seam. AIMS realization mutates `arc_func` in place, so calling
/// after it would validate the wrong IR.
pub fn assert_no_unresolved_type_vars<S: BuildHasher>(
    pool: &Pool,
    func: &ArcFunction,
    interner: &StringInterner,
    exempt_var_ids: &HashSet<u32, S>,
) -> Result<(), UnresolvedTypeVar> {
    let name = func.name;
    validate_type_sites(func, |ty, var| {
        check_unresolved_idx(pool, ty, name, var, exempt_var_ids)
    })?;

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

/// A single unresolved `Tag::BoundVar` encountered after shared lambda
/// specialization should have run.
///
/// Constructed by [`assert_no_unresolved_bound_vars`] on invariant
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
    /// The ARC variable whose type contains `Tag::BoundVar`, or
    /// [`ArcVarId::INVALID`] for the function return type.
    pub var_id: ArcVarId,
    /// The raw type-pool index that resolved to `Tag::BoundVar`.
    pub idx: Idx,
}

/// Check every shared ARC type position for a surviving `Tag::BoundVar`.
///
/// This is the backend-neutral closure gate for specialized ARC: LLVM, the VM,
/// and future executable projections consume the same validated result.
pub fn assert_no_unresolved_bound_vars(
    pool: &Pool,
    func: &ArcFunction,
) -> Result<(), UnresolvedBoundVar> {
    validate_type_sites(func, |ty, var_id| {
        if let Some(idx) = crate::first_unresolved_bound_var(pool, ty) {
            Err(UnresolvedBoundVar {
                function: func.name,
                var_id,
                idx,
            })
        } else {
            Ok(())
        }
    })
}

impl UnresolvedBoundVar {
    /// Render a user-facing diagnostic message for this violation.
    pub fn render(&self, interner: &StringInterner) -> String {
        format!(
            "Tag::BoundVar reached shared ARC closure: function `{}`, ArcVarId({}) has \
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
