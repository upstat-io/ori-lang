//! Fast-path, slow-path, and merge block construction for constructor reuse.
//!
//! After the orchestrator determines that a Reset/Reuse pair should be
//! expanded, this module builds the concrete basic blocks:
//!
//! - **Fast path** (unique, refcount == 1): in-place field `Set` mutations.
//! - **Slow path** (shared, refcount > 1): `RcDec` + fresh `Construct`.
//! - **Merge block**: receives the result from both paths via a block parameter.

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind, ValueRepr,
};
use crate::ArcClassification;

use super::{rc_strategy, ExpansionContext};

// Fast-path construction (§09.3 + §09.5)

/// Build the fast-path block: in-place field mutation via `Set`.
///
/// On the fast path, the value is uniquely owned (refcount == 1).
/// We mutate fields in-place and return the original object.
pub(super) fn build_fast_path(
    func: &mut ArcFunction,
    block_id: ArcBlockId,
    ctx: &ExpansionContext<'_>,
    classifier: &dyn ArcClassification,
    pool: Option<&ori_types::Pool>,
) -> ArcBlock {
    let mut body = Vec::new();

    // For each field being replaced:
    for (field_idx, arg) in ctx.pair.reuse_args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));

        // §09.5 Self-set elimination: if the arg was projected from the same
        // base at the same field index, the Set is a no-op.
        if is_self_set(ctx.pair.reset_var, field, *arg, ctx.proj_map) {
            continue;
        }

        // Dec the old field value if it's RC'd and not claimed.
        if !ctx.claimed.contains_key(&field) {
            if let Some(&old_val) = ctx.proj_map.get(&(ctx.pair.reset_var, field)) {
                let old_ty = func.var_type(old_val);
                if classifier.needs_rc(old_ty) {
                    body.push(ArcInstr::RcDec {
                        var: old_val,
                        strategy: rc_strategy(func, pool, old_val),
                    });
                }
            }
        }

        // Emit Set instruction.
        body.push(ArcInstr::Set {
            base: ctx.pair.reset_var,
            field,
            value: *arg,
        });
    }

    // SetTag for enum variants (in case the variant changed).
    if let CtorKind::EnumVariant { variant, .. } = ctx.pair.reuse_ctor {
        body.push(ArcInstr::SetTag {
            base: ctx.pair.reset_var,
            tag: u64::from(variant),
        });
    }

    // Terminator: merge or direct.
    let terminator = if let Some(mid) = ctx.merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.pair.reset_var], // result IS the original object
        }
    } else {
        let mut term = ctx.original_terminator.clone();
        term.substitute_var(ctx.pair.reuse_dst, ctx.pair.reset_var);
        term
    };

    ArcBlock {
        id: block_id,
        params: vec![],
        body,
        terminator,
    }
}

// Slow-path construction (§09.3)

/// Build the slow-path block: `RcDec` + fresh `Construct`.
///
/// On the slow path, the value is shared (refcount > 1). We decrement
/// the original (which won't reach zero) and allocate fresh memory.
pub(super) fn build_slow_path(
    block_id: ArcBlockId,
    ctx: &ExpansionContext<'_>,
    suffix: &[ArcInstr],
    pool: Option<&ori_types::Pool>,
    func: &ArcFunction,
) -> ArcBlock {
    let mut body = Vec::new();

    // Dec the shared original.
    body.push(ArcInstr::RcDec {
        var: ctx.pair.reset_var,
        strategy: rc_strategy(func, pool, ctx.pair.reset_var),
    });

    // Restore erased incs for claimed fields (§09.4).
    // Sort by field index for deterministic IR output — FxHashMap iteration
    // order is not stable across runs.
    let mut claimed_sorted: Vec<(u32, ArcVarId)> = ctx
        .claimed
        .iter()
        .map(|(&field, &var)| (field, var))
        .collect();
    claimed_sorted.sort_unstable_by_key(|(field, _)| *field);
    for (_, proj_var) in claimed_sorted {
        body.push(ArcInstr::RcInc {
            var: proj_var,
            count: 1,
            strategy: rc_strategy(func, pool, proj_var),
        });
    }

    // Fresh allocation via Construct.
    body.push(ArcInstr::Construct {
        dst: ctx.pair.reuse_dst,
        ty: ctx.pair.reuse_ty,
        ctor: ctx.pair.reuse_ctor,
        args: ctx.pair.reuse_args.clone(),
    });

    // Terminator: merge or direct.
    let terminator = if let Some(mid) = ctx.merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.pair.reuse_dst],
        }
    } else {
        ctx.original_terminator.clone()
    };

    // If no merge block, append suffix instructions.
    if ctx.merge_id.is_none() {
        body.extend_from_slice(suffix);
    }

    ArcBlock {
        id: block_id,
        params: vec![],
        body,
        terminator,
    }
}

// Merge block

/// Build the merge block that receives the result from fast/slow paths.
///
/// Creates a fresh parameter variable and substitutes `reuse_dst` with
/// it in the suffix instructions and terminator.
pub(super) fn build_merge_block(
    func: &mut ArcFunction,
    block_id: ArcBlockId,
    ctx: &ExpansionContext<'_>,
    suffix: &[ArcInstr],
    original_terminator: &ArcTerminator,
    classifier: &dyn ArcClassification,
    pool: Option<&ori_types::Pool>,
) -> ArcBlock {
    let reuse_dst = ctx.pair.reuse_dst;
    let reuse_ty = ctx.pair.reuse_ty;

    // The merge parameter receives either reset_var (fast) or reuse_dst (slow).
    // Compute its repr from the reused type so downstream passes see the correct
    // representation (not a Scalar placeholder).
    let repr = match pool {
        Some(p) => ValueRepr::from_arc_class(classifier.arc_class(reuse_ty), p, reuse_ty),
        None => ValueRepr::Scalar,
    };
    let merge_param = func.fresh_var_repr(reuse_ty, repr);

    // Clone suffix and substitute reuse_dst → merge_param.
    let mut body: Vec<ArcInstr> = suffix.to_vec();
    for instr in &mut body {
        instr.substitute_var(reuse_dst, merge_param);
    }

    let mut terminator = original_terminator.clone();
    terminator.substitute_var(reuse_dst, merge_param);

    ArcBlock {
        id: block_id,
        params: vec![(merge_param, reuse_ty)],
        body,
        terminator,
    }
}

// Self-set detection (§09.5)

/// Check whether writing `value` to `base.field` is a self-set (no-op).
///
/// A Set is a self-set if `value` was obtained via `Project { value: base, field }`.
fn is_self_set(base: ArcVarId, field: u32, value: ArcVarId, proj_map: &super::ProjMap) -> bool {
    proj_map.get(&(base, field)) == Some(&value)
}
