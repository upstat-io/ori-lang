//! Reset/Reuse detection for ARC IR (Section 07.6).
//!
//! After RC insertion (§07.2), identifies opportunities for in-place
//! constructor reuse: when an `RcDec` is immediately followed by a
//! `Construct` of the same type, the memory can be reused instead of
//! freed and reallocated.
//!
//! This pass replaces:
//! ```text
//! RcDec { var: x }
//! Construct { dst: y, ty: T, ctor, args }
//! ```
//! with:
//! ```text
//! Reset { var: x, token: t }
//! Reuse { token: t, dst: y, ty: T, ctor, args }
//! ```
//!
//! where `t` is a fresh reuse token. The `Reset`/`Reuse` pair is later
//! expanded by Section 09 into a conditional: if `x` is uniquely owned
//! (RC == 1), reuse the memory in-place; otherwise allocate fresh.
//!
//! # Constraints
//!
//! A `RcDec`/`Construct` pair is only valid for reset/reuse if:
//!
//! 1. The types match: `typeof(x) == ty` of the `Construct`.
//! 2. No use of `x` between the `RcDec` and `Construct` (no aliasing).
//! 3. The type needs RC (is heap-allocated).
//!
//! # Submodules
//!
//! - [`cross_block`] — cross-block detection using dominator tree + refined liveness.
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/ExpandResetReuse.lean`
//! - Lean 4: `src/Lean/Compiler/IR/ResetReuse.lean`
//! - Koka: Perceus paper §4 (reuse analysis)

mod cross_block;

pub use cross_block::detect_reset_reuse_cfg;

use ori_types::{Idx, Pool};
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, CtorKind, ValueRepr};
use crate::ArcClassification;

/// Detect and replace `RcDec`/`Construct` pairs with `Reset`/`Reuse`.
///
/// Scans each block forward for matching pairs. Only intra-block matches
/// are considered (cross-block reuse would require more complex analysis).
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `pool` — type pool for computing [`ValueRepr`] of token variables.
pub(crate) fn detect_reset_reuse(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    // Precondition: detection creates Reset/Reuse — none should exist yet.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. } | ArcInstr::Reuse { .. })),
        "detect_reset_reuse: IR already contains Reset/Reuse — pipeline ordering error"
    );

    tracing::debug!(
        function = func.name.raw(),
        "detecting reset/reuse opportunities"
    );

    let num_blocks = func.blocks.len();

    for block_idx in 0..num_blocks {
        detect_in_block(func, block_idx, classifier, pool);
    }
}

/// Detect reset/reuse pairs within a single block.
///
/// Uses a forward scan. When we find an `RcDec`, we look ahead for a
/// matching `Construct`. If found and constraints are satisfied, replace
/// both instructions.
fn detect_in_block(
    func: &mut ArcFunction,
    block_idx: usize,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    // Track which RcDec indices have been paired, so we don't pair twice.
    let mut paired_decs: FxHashSet<usize> = FxHashSet::default();
    // Track which Construct indices have been paired.
    let mut paired_constructs: FxHashSet<usize> = FxHashSet::default();

    // Phase 1: Scan — collect matches. Two categories:
    // - Struct matches → (dec_idx, construct_idx, dec_ty) for Reset/Reuse
    // - Collection matches → (dec_idx, construct_idx) for CollectionReuse
    let mut struct_matched: Vec<(usize, usize, Idx)> = Vec::new();
    let mut collection_matched: Vec<(usize, usize)> = Vec::new();

    let body = &func.blocks[block_idx].body;

    for i in 0..body.len() {
        if paired_decs.contains(&i) {
            continue;
        }

        // Look for RcDec instructions.
        let dec_var = match &body[i] {
            ArcInstr::RcDec { var, .. } => *var,
            _ => continue,
        };

        // Check that the type needs RC (skip scalars).
        let dec_ty = func.var_type(dec_var);
        if !classifier.needs_rc(dec_ty) {
            continue;
        }

        // Scan forward for a matching Construct.
        for (j, candidate) in body.iter().enumerate().skip(i + 1) {
            if paired_constructs.contains(&j) {
                continue;
            }

            // Check constraint: no use of dec_var between i and j.
            if candidate.uses_var(dec_var) && !matches!(candidate, ArcInstr::Construct { .. }) {
                // dec_var is used before we find a Construct → cannot reuse.
                break;
            }

            match candidate {
                ArcInstr::Construct { ty, ctor, .. } if *ty == dec_ty => {
                    // Check that dec_var is NOT used in the Construct's args.
                    // (If it is, there's an alias and reuse is unsafe.)
                    if candidate.uses_var(dec_var) {
                        continue;
                    }

                    if is_list_or_set_ctor(ctor) {
                        // List/Set → CollectionReuse (self-contained, no expansion).
                        collection_matched.push((i, j));
                    } else if !is_map_ctor(ctor) {
                        // Struct/Enum → Reset/Reuse (expanded by Section 09).
                        // Maps excluded (dual-region layout too complex).
                        struct_matched.push((i, j, dec_ty));
                    } else {
                        continue;
                    }

                    paired_decs.insert(i);
                    paired_constructs.insert(j);
                    break;
                }
                _ => {
                    // Check if this instruction uses dec_var → constraint violation.
                    if candidate.uses_var(dec_var) {
                        break;
                    }
                }
            }
        }
    }

    apply_struct_reuse(func, block_idx, struct_matched, classifier, pool);
    apply_collection_reuse(func, block_idx, collection_matched);
}

/// Apply struct Reset/Reuse replacements for matched pairs.
fn apply_struct_reuse(
    func: &mut ArcFunction,
    block_idx: usize,
    struct_matched: Vec<(usize, usize, Idx)>,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    let struct_pairs: Vec<(usize, usize, ArcVarId)> = struct_matched
        .into_iter()
        .map(|(dec_idx, construct_idx, dec_ty)| {
            let repr = repr_for_type(classifier, pool, dec_ty);
            let token = func.fresh_var_repr(dec_ty, repr);
            (dec_idx, construct_idx, token)
        })
        .collect();

    let body = &mut func.blocks[block_idx].body;
    for (dec_idx, construct_idx, token) in struct_pairs {
        let (dst, ty, ctor, args) = match &body[construct_idx] {
            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
                ..
            } => (*dst, *ty, *ctor, args.clone()),
            _ => unreachable!("paired construct index must be a Construct"),
        };

        let dec_var = match &body[dec_idx] {
            ArcInstr::RcDec { var, .. } => *var,
            _ => unreachable!("paired dec index must be an RcDec"),
        };

        body[dec_idx] = ArcInstr::Reset {
            var: dec_var,
            token,
        };
        body[construct_idx] = ArcInstr::Reuse {
            token,
            dst,
            ty,
            ctor,
            args,
        };
    }
}

/// Apply `CollectionReuse` replacements for matched list/set pairs.
///
/// Replaces `RcDec` with a noop `Let` (preserving instruction indices) and
/// `Construct` with `CollectionReuse`. The runtime function handles both
/// the uniqueness check and element cleanup.
fn apply_collection_reuse(
    func: &mut ArcFunction,
    block_idx: usize,
    collection_matched: Vec<(usize, usize)>,
) {
    // Allocate noop vars before body borrow (avoids mutable aliasing).
    let collection_pairs: Vec<(usize, usize, ArcVarId)> = collection_matched
        .into_iter()
        .map(|(dec_idx, construct_idx)| {
            let noop_var = func.fresh_var(Idx::UNIT);
            (dec_idx, construct_idx, noop_var)
        })
        .collect();

    let body = &mut func.blocks[block_idx].body;
    for (dec_idx, construct_idx, noop_var) in collection_pairs {
        let (dst, ty, ctor, args) = match &body[construct_idx] {
            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
                ..
            } => (*dst, *ty, *ctor, args.clone()),
            _ => unreachable!("paired construct index must be a Construct"),
        };

        let dec_var = match &body[dec_idx] {
            ArcInstr::RcDec { var, .. } => *var,
            _ => unreachable!("paired dec index must be an RcDec"),
        };

        body[dec_idx] = ArcInstr::Let {
            dst: noop_var,
            ty: Idx::UNIT,
            value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Unit),
        };
        body[construct_idx] = ArcInstr::CollectionReuse {
            old_var: dec_var,
            dst,
            ty,
            ctor,
            args,
        };
    }
}

/// List/Set constructors eligible for `CollectionReuse` (buffer-based reuse).
fn is_list_or_set_ctor(ctor: &CtorKind) -> bool {
    matches!(ctor, CtorKind::ListLiteral | CtorKind::SetLiteral)
}

/// Map constructors — excluded from all reuse (dual-region layout complexity).
fn is_map_ctor(ctor: &CtorKind) -> bool {
    matches!(ctor, CtorKind::MapLiteral)
}

/// Collection constructors use a separate RC'd buffer layout, not inline
/// struct fields. The `Set` fast-path in expansion assumes struct GEP, so
/// these constructors must be excluded from Reset/Reuse pairing.
///
/// Used by cross-block detection to exclude collections from Reset/Reuse.
pub(super) fn is_collection_ctor(ctor: &CtorKind) -> bool {
    matches!(
        ctor,
        CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
    )
}

/// Compute [`ValueRepr`] for a type from classifier + pool.
pub(super) fn repr_for_type(classifier: &dyn ArcClassification, pool: &Pool, ty: Idx) -> ValueRepr {
    let class = classifier.arc_class(ty);
    ValueRepr::from_arc_class(class, pool, ty)
}

#[cfg(test)]
mod tests;
