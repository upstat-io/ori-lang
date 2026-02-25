//! Salsa-tracked types for ARC borrow inference.
//!
//! Bridges `ori_arc`'s ARC IR into `oric`'s Salsa database for
//! incremental borrow inference (Section 12).
//!
//! # Design
//!
//! [`ArcFunction`] and [`AnnotatedSig`] already derive all Salsa-required
//! traits (`Clone, Eq, Hash, Debug`), but they're defined in `ori_arc`
//! (which doesn't depend on Salsa). This module defines Salsa input/result
//! types in `oric` that wrap ARC IR.
//!
//! ## Why call graph and SCCs are NOT in the input
//!
//! The call graph and SCCs are derived from the functions — they're computed
//! data, not independent inputs. Storing them as derived queries (Section 12.4)
//! rather than input fields gives an extra layer of Salsa early cutoff: if a
//! function body changes but the call graph structure doesn't, SCC-dependent
//! borrow inference queries are skipped entirely.

use ori_arc::{AnnotatedSig, ArcFunction};
use ori_ir::Name;
use rustc_hash::FxHashMap;
use std::path::PathBuf;

#[cfg(test)]
mod tests;

/// Salsa input: lowered ARC IR for one source file.
///
/// Set once during the lowering phase. Salsa tracks whether the
/// content changes between compilations — if it doesn't, all
/// dependent queries are skipped.
///
/// # Fields
///
/// - `path`: Source file path for cache keying and diagnostics.
/// - `functions`: Lowered ARC functions, sorted by name. Uses `Vec` instead
///   of `FxHashMap` because `Vec` satisfies Salsa's `Eq + Hash` requirements
///   via element-wise comparison.
///
/// # Intentionally Absent
///
/// Call graph and SCCs are NOT stored here. They are derived from `functions`
/// by tracked queries in Section 12.4, enabling Salsa early cutoff at the
/// SCC level.
#[salsa::input]
pub struct ArcModuleInput {
    /// Source file path (for cache keying and diagnostics).
    #[return_ref]
    pub path: PathBuf,

    /// Lowered ARC functions, sorted by function name.
    ///
    /// Sorted by [`Name`] for deterministic `Eq`/`Hash` comparison, which
    /// Salsa uses for change detection and early cutoff.
    #[return_ref]
    pub functions: Vec<(Name, ArcFunction)>,
}

impl ArcModuleInput {
    /// Look up a function by name via binary search.
    ///
    /// Returns `None` if the function is not in this module.
    pub fn get_function(self, db: &dyn crate::db::Db, name: Name) -> Option<&ArcFunction> {
        let funcs = self.functions(db);
        funcs
            .binary_search_by_key(&name, |(n, _)| *n)
            .ok()
            .map(|idx| &funcs[idx].1)
    }

    /// Get only the `ArcFunction` values (without names).
    ///
    /// Useful for passing to APIs that expect `&[ArcFunction]`.
    pub fn function_list(self, db: &dyn crate::db::Db) -> Vec<ArcFunction> {
        self.functions(db).iter().map(|(_, f)| f.clone()).collect()
    }

    /// Create a sorted functions vec from a map, suitable for constructing
    /// this input type.
    ///
    /// Ensures deterministic ordering by sorting on [`Name`].
    pub fn sorted_functions(map: FxHashMap<Name, ArcFunction>) -> Vec<(Name, ArcFunction)> {
        let mut funcs: Vec<_> = map.into_iter().collect();
        funcs.sort_by_key(|(name, _)| *name);
        funcs
    }
}

/// Per-SCC borrow inference result.
///
/// Stores annotated signatures sorted by [`Name`] for deterministic
/// Salsa comparison (enables early cutoff — if the result is unchanged,
/// callers' queries are not re-executed).
///
/// This is the Salsa-compatible replacement for the current
/// `FxHashMap<Name, AnnotatedSig>` return type of `infer_borrows()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BorrowSigResult {
    /// Annotated signatures, sorted by Name for deterministic Eq/Hash.
    sigs: Vec<(Name, AnnotatedSig)>,
}

impl BorrowSigResult {
    /// Create an empty result.
    pub fn empty() -> Self {
        Self { sigs: Vec::new() }
    }

    /// Look up a signature by name via binary search.
    ///
    /// Returns `None` if the name is not in this result.
    pub fn get(&self, name: Name) -> Option<&AnnotatedSig> {
        self.sigs
            .binary_search_by_key(&name, |(n, _)| *n)
            .ok()
            .map(|idx| &self.sigs[idx].1)
    }

    /// Convert to a hash map for downstream consumers that need O(1) lookup
    /// by name without maintaining sort order.
    pub fn into_map(self) -> FxHashMap<Name, AnnotatedSig> {
        self.sigs.into_iter().collect()
    }

    /// Create from a hash map, sorting entries by [`Name`] for deterministic
    /// `Eq`/`Hash` comparison.
    pub fn from_map(map: FxHashMap<Name, AnnotatedSig>) -> Self {
        let mut sigs: Vec<_> = map.into_iter().collect();
        sigs.sort_by_key(|(name, _)| *name);
        Self { sigs }
    }

    /// Number of signatures in this result.
    pub fn len(&self) -> usize {
        self.sigs.len()
    }

    /// Whether this result is empty.
    pub fn is_empty(&self) -> bool {
        self.sigs.is_empty()
    }

    /// Iterate over (name, sig) pairs in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &(Name, AnnotatedSig)> {
        self.sigs.iter()
    }
}
