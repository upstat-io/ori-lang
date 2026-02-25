//! Inter-function call graph extracted from ARC IR.
//!
//! The call graph captures static callees from [`ArcInstr::Apply`],
//! [`ArcInstr::PartialApply`], and [`ArcTerminator::Invoke`] instructions.
//! Indirect calls ([`ArcInstr::ApplyIndirect`]) are excluded — their callees
//! are unknown at compile time and handled conservatively (all-Owned) by
//! borrow inference.
//!
//! Used by SCC-based borrow inference (Section 12) to decompose functions
//! into strongly connected components for incremental Salsa-tracked analysis.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};

#[cfg(test)]
mod tests;

/// Inter-function call graph.
///
/// Nodes are function names ([`Name`]). Edges are direct calls extracted
/// from `Apply`, `PartialApply`, and `Invoke` instructions in the ARC IR.
///
/// Both forward (callees) and reverse (callers) indexes are maintained
/// for O(1) lookups in either direction.
#[derive(Clone, Debug)]
pub struct CallGraph {
    /// Function → set of functions it directly calls.
    callees: FxHashMap<Name, FxHashSet<Name>>,
    /// Reverse index: function → set of functions that call it.
    callers: FxHashMap<Name, FxHashSet<Name>>,
    /// All function names in the graph (including leaves with no calls).
    functions: FxHashSet<Name>,
}

/// Sentinel value for empty callee/caller sets (avoids Option).
static EMPTY_SET: std::sync::LazyLock<FxHashSet<Name>> =
    std::sync::LazyLock::new(FxHashSet::default);

impl CallGraph {
    /// Build a call graph from a slice of ARC functions.
    ///
    /// Walks each function's blocks, extracts callees from direct call
    /// instructions (`Apply`, `PartialApply`, `Invoke`), and builds both
    /// forward and reverse indexes in a single pass.
    ///
    /// External callees (e.g., `ori_*` runtime functions) appear in callee
    /// sets but not in [`functions()`] — they are not graph nodes.
    pub fn build(functions: &[ArcFunction]) -> Self {
        Self::build_inner(functions.iter())
    }

    /// Build a call graph from `(Name, ArcFunction)` pairs.
    ///
    /// Zero-copy variant of [`build`](Self::build) for Salsa-returned
    /// `Vec<(Name, ArcFunction)>` — avoids cloning all functions into a
    /// separate `Vec<ArcFunction>` just to strip the names.
    pub fn build_from_pairs(functions: &[(Name, ArcFunction)]) -> Self {
        Self::build_inner(functions.iter().map(|(_, f)| f))
    }

    /// Shared implementation for [`build`] and [`build_from_pairs`].
    fn build_inner<'a>(functions: impl Iterator<Item = &'a ArcFunction> + Clone) -> Self {
        let function_names: FxHashSet<Name> = functions.clone().map(|f| f.name).collect();

        let mut callees: FxHashMap<Name, FxHashSet<Name>> = FxHashMap::default();
        let mut callers: FxHashMap<Name, FxHashSet<Name>> = FxHashMap::default();

        for func in functions {
            let caller = func.name;
            let callee_set = callees.entry(caller).or_default();

            for block in &func.blocks {
                // Extract callees from instructions.
                for instr in &block.body {
                    match instr {
                        ArcInstr::Apply { func: callee, .. }
                        | ArcInstr::PartialApply { func: callee, .. } => {
                            if callee_set.insert(*callee) {
                                callers.entry(*callee).or_default().insert(caller);
                            }
                        }
                        // ApplyIndirect: runtime-determined callee, skip.
                        _ => {}
                    }
                }

                // Extract callees from terminators.
                if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
                    if callee_set.insert(*callee) {
                        callers.entry(*callee).or_default().insert(caller);
                    }
                }
            }
        }

        Self {
            callees,
            callers,
            functions: function_names,
        }
    }

    /// Functions directly called by `name`.
    ///
    /// Returns an empty set if `name` has no callees or is not in the graph.
    /// Note: returned names may include external functions not in
    /// [`functions()`].
    pub fn callees_of(&self, name: Name) -> &FxHashSet<Name> {
        self.callees.get(&name).unwrap_or(&EMPTY_SET)
    }

    /// Functions that directly call `name`.
    ///
    /// Returns an empty set if `name` has no callers or is not in the graph.
    pub fn callers_of(&self, name: Name) -> &FxHashSet<Name> {
        self.callers.get(&name).unwrap_or(&EMPTY_SET)
    }

    /// All function names in the graph (excluding external callees).
    pub fn functions(&self) -> impl Iterator<Item = Name> + '_ {
        self.functions.iter().copied()
    }

    /// Number of functions in the graph.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Whether the graph has no functions.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Whether `name` calls itself (directly).
    pub fn is_recursive(&self, name: Name) -> bool {
        self.callees
            .get(&name)
            .is_some_and(|set| set.contains(&name))
    }

    /// Whether `name` is in the graph's function set.
    pub fn contains(&self, name: Name) -> bool {
        self.functions.contains(&name)
    }

    /// Whether `name` has no callees that are in the graph.
    ///
    /// A function is a leaf if all its callees (if any) are external
    /// (e.g., runtime functions not in the analyzed function set).
    pub fn is_leaf(&self, name: Name) -> bool {
        match self.callees.get(&name) {
            None => true,
            Some(set) => set.iter().all(|callee| !self.functions.contains(callee)),
        }
    }
}
