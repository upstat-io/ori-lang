//! Scope management for ARC IR lowering.
//!
//! [`ArcScope`] tracks name→[`ArcVarId`] bindings with mutable variable
//! tracking. Unlike the LLVM codegen scope (which uses `alloca`/`load`/`store`
//! for mutable variables), ARC IR uses SSA rebinding: each assignment creates
//! a fresh `ArcVarId`, and at control-flow merge points, block parameters
//! serve as phi nodes.
//!
//! # SSA Merge
//!
//! [`merge_mutable_vars`] compares branch scopes against a pre-branch snapshot
//! to find mutable variables that were reassigned. For each changed variable,
//! it adds a block parameter to the merge block.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlockId, ArcVarId};

use super::ArcIrBuilder;

/// Lexical scope for ARC IR lowering.
///
/// Uses `FxHashMap` (fast, non-cryptographic hash) for name→variable lookup.
/// Child scopes are created via `clone()` — since ARC IR scoping is simpler
/// than LLVM codegen (no alloca/load/store, just name rebinding), the clone
/// cost is acceptable.
#[derive(Clone)]
pub struct ArcScope {
    bindings: FxHashMap<Name, ArcVarId>,
    mutable_names: FxHashSet<Name>,
}

impl ArcScope {
    /// Create an empty scope.
    pub fn new() -> Self {
        Self {
            bindings: FxHashMap::default(),
            mutable_names: FxHashSet::default(),
        }
    }

    /// Bind an immutable variable.
    pub fn bind(&mut self, name: Name, var: ArcVarId) {
        self.bindings.insert(name, var);
    }

    /// Bind a mutable variable and track it for SSA merge.
    pub fn bind_mutable(&mut self, name: Name, var: ArcVarId) {
        self.bindings.insert(name, var);
        self.mutable_names.insert(name);
    }

    /// Look up a variable by name.
    pub fn lookup(&self, name: Name) -> Option<ArcVarId> {
        self.bindings.get(&name).copied()
    }

    /// Check whether a name refers to a mutable variable.
    pub fn is_mutable(&self, name: Name) -> bool {
        self.mutable_names.contains(&name)
    }

    /// Iterate over all mutable bindings (name, current var).
    pub(crate) fn mutable_bindings(&self) -> impl Iterator<Item = (Name, ArcVarId)> + '_ {
        self.mutable_names
            .iter()
            .filter_map(|name| self.bindings.get(name).map(|var| (*name, *var)))
    }
}

impl Default for ArcScope {
    fn default() -> Self {
        Self::new()
    }
}

/// Merge mutable variable versions from divergent branches.
///
/// Compares each branch scope's mutable bindings against the `pre_scope`
/// (snapshot taken before the branch). For any mutable variable whose
/// `ArcVarId` changed in at least one branch, adds a block parameter to
/// `merge_block` and returns the `(Name, ArcVarId)` pairs for the caller
/// to rebind in the post-merge scope.
///
/// Returns the list of rebindings to apply to the post-merge scope.
pub(crate) fn merge_mutable_vars(
    builder: &mut ArcIrBuilder,
    merge_block: ArcBlockId,
    pre_scope: &ArcScope,
    branch_scopes: &[ArcScope],
    var_types: &FxHashMap<Name, Idx>,
) -> Vec<(Name, ArcVarId)> {
    let mut rebindings = Vec::new();

    for (name, pre_var) in pre_scope.mutable_bindings() {
        // Check if any branch changed this variable.
        let changed = branch_scopes
            .iter()
            .any(|scope| scope.lookup(name) != Some(pre_var));

        if changed {
            let ty = var_types.get(&name).copied().unwrap_or(Idx::UNIT);
            let merge_var = builder.add_block_param(merge_block, ty);
            rebindings.push((name, merge_var));
        }
    }

    rebindings
}

// Tests

#[cfg(test)]
mod tests;
