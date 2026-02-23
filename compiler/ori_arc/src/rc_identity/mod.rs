//! RC identity propagation pass.
//!
//! Normalizes `RcInc`/`RcDec` operations to target canonical root variables
//! rather than projections. When `v1 = Project(v0, field)`, both `v1` and
//! `v0` share the same RC identity — incrementing `v1`'s ref count and
//! decrementing `v0`'s are operations on the same logical ownership chain.
//!
//! Without normalization, the RC eliminator (which matches pairs by exact
//! variable) can't see that `RcInc(v1)` and `RcDec(v0)` cancel each other.
//! After normalization, both target `v0`, enabling pair elimination.
//!
//! # Pipeline Position
//!
//! Runs after reset/reuse expansion and before RC elimination:
//!
//! ```text
//! expand_reset_reuse → propagate_rc_identity → eliminate_rc_ops_dataflow
//! ```
//!
//! # Safety
//!
//! Normalization is valid because `DerivedOwnership::BorrowedFrom(root)`
//! guarantees that the root is alive whenever the projection is alive.
//! When the strategy differs between projection and root, the strategy is
//! recomputed for the root variable using `RcStrategy::from_var()`.
//!
//! # References
//!
//! - Swift: `RCIdentityFunctionInfo` (`SILOptimizer/ARC/RCIdentityAnalysis.h`)
//! - Lean 4: `DerivedValMap` (`Compiler/IR/Borrow.lean`)

use ori_types::Pool;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, RcStrategy, ValueRepr};
use crate::ownership::DerivedOwnership;

/// Maps each variable to its canonical RC identity root.
///
/// Built from [`DerivedOwnership`] in a single pass. When `v1 = Project(v0, _)`,
/// `v1` borrows from `v0` — they share the same RC identity. Multi-level
/// projections (`v2 = Project(v1, _)` where `v1 = BorrowedFrom(v0)`) are
/// transitively resolved to the ultimate root (`v0`).
///
/// Variables that ARE their own root (Owned, Fresh, or not borrowed) map
/// to themselves.
pub struct RcIdentityMap {
    /// `identity[var.index()]` = canonical root for RC purposes.
    identity: Vec<ArcVarId>,
}

impl RcIdentityMap {
    /// Build the identity map from derived ownership information.
    ///
    /// Single-pass construction: for each `BorrowedFrom(source)`, resolve
    /// the chain to its ultimate root. The ownership vector is already
    /// transitively resolved by `infer_derived_ownership` (projections from
    /// borrowed sources inherit the root), but we still handle chains
    /// defensively in case of multi-level borrowing.
    pub fn build(func: &ArcFunction, ownership: &[DerivedOwnership]) -> Self {
        let n = func.var_types.len();

        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR var counts fit in u32"
        )]
        let mut identity: Vec<ArcVarId> = (0..n).map(|i| ArcVarId::new(i as u32)).collect();

        for (i, own) in ownership.iter().enumerate() {
            if let DerivedOwnership::BorrowedFrom(root) = own {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "ARC IR var counts fit in u32"
                )]
                let self_var = ArcVarId::new(i as u32);
                if *root != self_var {
                    identity[i] = Self::resolve_root(*root, &identity);
                }
            }
        }

        Self { identity }
    }

    /// Follow the identity chain to the ultimate root.
    ///
    /// Iterative resolution with a defensive loop bound to prevent infinite
    /// loops from cyclic ownership bugs. In well-formed IR, chains are short
    /// (typically 1-2 levels) because `infer_derived_ownership` already
    /// resolves transitively.
    fn resolve_root(var: ArcVarId, identity: &[ArcVarId]) -> ArcVarId {
        let mut current = var;
        for _ in 0..identity.len() {
            let parent = identity[current.index()];
            if parent == current {
                return current;
            }
            current = parent;
        }
        current
    }

    /// Get the canonical RC identity for a variable.
    pub fn root(&self, var: ArcVarId) -> ArcVarId {
        self.identity.get(var.index()).copied().unwrap_or(var)
    }

    /// True if this variable IS the root (not a projection).
    pub fn is_root(&self, var: ArcVarId) -> bool {
        self.root(var) == var
    }

    /// Number of variables that were normalized to a different root.
    pub fn normalized_count(&self) -> usize {
        self.identity
            .iter()
            .enumerate()
            .filter(|(i, root)| root.index() != *i)
            .count()
    }
}

/// Normalize RC operations to use canonical root identities.
///
/// After this pass:
/// - `RcInc(projected_field)` becomes `RcInc(root_owner)`
/// - `RcDec(projected_field)` becomes `RcDec(root_owner)`
///
/// Both `var` and `strategy` are updated — the strategy is recomputed for
/// the root variable via [`RcStrategy::from_var()`] because projections and
/// their roots can have different `ValueRepr` (e.g., a `str` field extracted
/// from a struct has `FatPointer` strategy while the struct has `AggregateFields`).
///
/// Variables whose root has `Scalar` repr are left unchanged (the root
/// doesn't need RC, so normalization would be semantically incorrect).
///
/// Returns the number of RC operations normalized.
pub fn propagate_rc_identity(
    func: &mut ArcFunction,
    identity_map: &RcIdentityMap,
    pool: &Pool,
) -> usize {
    // Pre-compute root strategies to avoid borrowing func immutably while
    // mutating func.blocks. Uses split borrows: var_reprs and var_types are
    // read-only, blocks is the mutation target.
    let strategies: Vec<Option<RcStrategy>> = (0..func.var_types.len())
        .map(|i| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            let var = ArcVarId::new(i as u32);
            let root = identity_map.root(var);
            if root == var {
                return None; // Self-rooted, no normalization needed
            }
            root_strategy(&func.var_reprs, &func.var_types, root, pool)
        })
        .collect();

    let mut normalized = 0;

    for block in &mut func.blocks {
        for instr in &mut block.body {
            match instr {
                ArcInstr::RcInc { var, strategy, .. } => {
                    if let Some(new_strategy) = strategies.get(var.index()).copied().flatten() {
                        let root = identity_map.root(*var);
                        tracing::trace!(
                            from = var.raw(),
                            to = root.raw(),
                            old_strategy = ?strategy,
                            new_strategy = ?new_strategy,
                            "normalizing RcInc to root identity"
                        );
                        *var = root;
                        *strategy = new_strategy;
                        normalized += 1;
                    }
                }
                ArcInstr::RcDec { var, strategy } => {
                    if let Some(new_strategy) = strategies.get(var.index()).copied().flatten() {
                        let root = identity_map.root(*var);
                        tracing::trace!(
                            from = var.raw(),
                            to = root.raw(),
                            old_strategy = ?strategy,
                            new_strategy = ?new_strategy,
                            "normalizing RcDec to root identity"
                        );
                        *var = root;
                        *strategy = new_strategy;
                        normalized += 1;
                    }
                }
                _ => {}
            }
        }
    }

    if normalized > 0 {
        tracing::debug!(
            function = func.name.raw(),
            count = normalized,
            "normalized RC operations to root identities"
        );
    }

    normalized
}

/// Compute the `RcStrategy` for a root variable from pre-borrowed fields.
///
/// Returns `None` if the root has `Scalar` repr (no RC needed — normalization
/// would be incorrect) or if `var_reprs` is empty (test-only path without
/// Pool-based repr computation).
fn root_strategy(
    var_reprs: &[ValueRepr],
    var_types: &[ori_types::Idx],
    root: ArcVarId,
    pool: &Pool,
) -> Option<RcStrategy> {
    let repr = var_reprs.get(root.index()).copied()?;
    if repr == ValueRepr::Scalar {
        return None;
    }
    let ty = var_types.get(root.index()).copied()?;
    Some(RcStrategy::from_var(repr, pool, ty))
}

#[cfg(test)]
mod tests;
