//! Salsa-tracked types and queries for ARC borrow inference.
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
//!
//! ## Per-SCC queries (Sections 12.4–12.6)
//!
//! [`arc_scc_decomposition`] computes the SCC structure from function IR.
//! [`infer_borrow_scc`] performs borrow inference for a single SCC, querying
//! callee SCCs via Salsa dependency edges. This enables incremental
//! recompilation: changing a function only re-analyzes its SCC and dependents.
//!
//! ## Early cutoff contract (Section 12.7)
//!
//! Salsa's early cutoff optimization: if a query re-executes but returns an
//! `Eq`-equal result, dependents are NOT invalidated. This is the key
//! incrementality win for borrow inference.
//!
//! **Example:** Function `foo` changes from `x + 1` to `x + 2`. Both versions
//! have the same borrow signature (param `x` is `Borrowed` in both). Salsa
//! re-runs `infer_borrow_scc` for foo's SCC, gets the same `BorrowSigResult`,
//! and skips re-evaluating all of foo's callers.
//!
//! **Equality chain** (all types derive `Eq + Hash`, no non-deterministic fields):
//! - `Name(u32)` — interned, O(1) comparison
//! - `Idx(u32)` — pool index, O(1)
//! - `Ownership` — 2-variant enum, O(1)
//! - `AnnotatedParam { name, ty, ownership }` — 3 Copy fields, O(1)
//! - `AnnotatedSig { params, return_type }` — O(params), typically ≤10
//! - `BorrowSigResult { sigs }` — sorted `Vec<(Name, AnnotatedSig)>`, deterministic
//!
//! **Determinism guarantee:** `BorrowSigResult::from_map()` sorts entries by
//! `Name` before storing. `SccDecomposition` sorts its index by `Name`.
//! No `FxHashMap` or other non-deterministic container appears in any
//! Salsa-tracked return type.
//!
//! ## Incremental invalidation strategy (Section 12.10)
//!
//! ### `ArcModuleInput` update
//!
//! When `typed()` produces new output, the codegen path re-lowers functions
//! to ARC IR and creates a **new** `ArcModuleInput` via `ArcModuleInput::new()`.
//! Salsa compares the new input's `functions` field to the previous revision:
//!
//! - **Functions unchanged** → `arc_scc_decomposition` returns memoized result
//!   → all `infer_borrow_scc` queries return memoized results → no work
//! - **Function body changed** → `arc_scc_decomposition` re-runs. If the call
//!   graph structure is unchanged (same callees), the `SccDecomposition` is
//!   `Eq`-equal → early cutoff → only the changed function's SCC re-infers.
//!   If the SCC's borrow sig is unchanged → early cutoff again → callers skip.
//!
//! ### SCC membership changes
//!
//! Adding/removing a call edge can restructure SCCs (merge or split). Since
//! `SccDecomposition` is a tracked query derived from the full function list,
//! any structural change produces a different `SccDecomposition` value. All
//! `infer_borrow_scc` queries re-execute with the new SCC indices, but most
//! produce the same `BorrowSigResult` (early cutoff). This is correct because
//! SCC index is a positional key, not a stable identity — Salsa treats any
//! change to the decomposition as full invalidation of per-SCC queries.
//!
//! ### Cross-file borrow inference
//!
//! Currently, borrow inference is per-file. Cross-file callees (imports) are
//! treated conservatively as external (all-Owned params). Each file gets its
//! own `ArcModuleInput`, and there are no Salsa dependency edges between
//! files' borrow queries. Cross-file Salsa dependencies are a future
//! optimization (would require `ArcModuleInput` per-file with inter-file
//! query edges).
//!
//! ### Granularity
//!
//! Current: one `ArcModuleInput` per file (all functions in the file).
//! Future optimization: one `ArcFunctionInput` per function would give
//! maximum Salsa granularity (changing one function only invalidates its
//! SCC's inputs). Deferred until profiling shows module-level is too coarse.

use ori_arc::{AnnotatedSig, ArcFunction};
use ori_ir::Name;
use rustc_hash::FxHashMap;
use std::path::PathBuf;

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
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
/// `FxHashMap<Name, AnnotatedSig>` return type of `infer_borrows_scc()`.
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

// ── SCC Decomposition (Section 12.4) ───────────────────────────────

/// Salsa-compatible SCC decomposition of the call graph.
///
/// Stores the SCCs in topological order (callees before callers), with
/// parallel arrays for recursion flags and a sorted index for O(log n)
/// function→SCC lookup.
///
/// Unlike [`CallGraph`](ori_arc::CallGraph) which contains `FxHashMap`/`FxHashSet`
/// (no `Eq`/`Hash`), this type is fully Salsa-compatible. The `CallGraph` is
/// computed transiently inside [`arc_scc_decomposition`] and discarded.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SccDecomposition {
    /// SCCs in topological order (callees before callers).
    pub sccs: Vec<ori_arc::Scc>,
    /// Parallel array: whether each SCC is recursive (self or mutual).
    recursive: Vec<bool>,
    /// Function name → SCC index, sorted by Name for binary search.
    scc_index: Vec<(Name, u32)>,
}

impl SccDecomposition {
    /// Look up which SCC a function belongs to.
    pub fn scc_of(&self, name: Name) -> Option<u32> {
        self.scc_index
            .binary_search_by_key(&name, |(n, _)| *n)
            .ok()
            .map(|idx| self.scc_index[idx].1)
    }

    /// Whether the SCC at `scc_index` is recursive.
    pub fn is_recursive(&self, scc_index: u32) -> bool {
        self.recursive
            .get(scc_index as usize)
            .copied()
            .unwrap_or(false)
    }

    /// Number of SCCs.
    pub fn len(&self) -> usize {
        self.sccs.len()
    }

    /// Whether the decomposition is empty.
    pub fn is_empty(&self) -> bool {
        self.sccs.is_empty()
    }
}

/// Compute the SCC decomposition of a module's call graph (Section 12.4).
///
/// Builds a transient [`CallGraph`](ori_arc::CallGraph) from the module's
/// functions, computes SCCs via Tarjan's algorithm, then extracts the
/// Salsa-compatible [`SccDecomposition`].
///
/// # Early Cutoff
///
/// If function bodies change but the call structure doesn't, the
/// `SccDecomposition` is unchanged → per-SCC queries are skipped.
#[salsa::tracked]
pub fn arc_scc_decomposition(db: &dyn crate::db::Db, module: ArcModuleInput) -> SccDecomposition {
    let functions = module.functions(db);
    tracing::debug!(
        function_count = functions.len(),
        "computing SCC decomposition"
    );

    // Build transient call graph (not stored in Salsa).
    // Uses build_from_pairs to avoid cloning all ArcFunctions from the Salsa-returned Vec.
    let call_graph = ori_arc::CallGraph::build_from_pairs(functions);
    let sccs = ori_arc::compute_sccs(&call_graph);

    // Extract recursion flags.
    let recursive: Vec<bool> = sccs
        .iter()
        .map(|scc| scc.is_recursive(&call_graph))
        .collect();

    // Build sorted name→SCC index.
    let mut scc_index = Vec::new();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "SCC count bounded by function count, fits in u32"
    )]
    for (idx, scc) in sccs.iter().enumerate() {
        for &member in &scc.members {
            scc_index.push((member, idx as u32));
        }
    }
    scc_index.sort_by_key(|(name, _)| *name);

    tracing::debug!(scc_count = sccs.len(), "SCC decomposition complete");

    SccDecomposition {
        sccs,
        recursive,
        scc_index,
    }
}

/// Infer borrow annotations for a single SCC (Section 12.4).
///
/// This is the core incremental query. For each SCC:
/// - Non-recursive: single-pass via [`infer_borrow_single`](ori_arc::infer_borrow_single)
/// - Recursive: fixed-point via [`infer_borrow_fixed_point`](ori_arc::infer_borrow_fixed_point)
///
/// External callee sigs are obtained by querying `infer_borrow_scc` for
/// the callee's SCC, creating Salsa dependency edges. This is the mechanism
/// for incremental invalidation: changing a function invalidates its SCC,
/// which invalidates any SCC that depends on it.
#[salsa::tracked]
pub fn infer_borrow_scc(
    db: &dyn crate::db::Db,
    module: ArcModuleInput,
    scc_index: u32,
) -> BorrowSigResult {
    let decomp = arc_scc_decomposition(db, module);
    let scc = &decomp.sccs[scc_index as usize];

    tracing::debug!(
        scc_index,
        member_count = scc.members.len(),
        is_recursive = decomp.is_recursive(scc_index),
        "inferring borrows for SCC"
    );

    // Collect external callee sigs from other SCCs.
    let external_sigs = collect_callee_sigs(db, module, &decomp, scc, scc_index);

    // Get classifier from Pool cache.
    debug_assert!(
        db.pool_cache().get(module.path(db)).is_some(),
        "Pool must be cached before borrow inference — typed() should run first"
    );
    let Some(pool_arc) = db.pool_cache().get(module.path(db)) else {
        tracing::warn!("Pool not cached for module — returning empty sigs");
        return BorrowSigResult::empty();
    };
    let classifier = ori_arc::ArcClassifier::new(&pool_arc);

    // Get borrowing builtins.
    let borrowing_builtins = ori_llvm::borrowing_builtin_names(db.interner());

    // Dispatch based on recursion.
    if decomp.is_recursive(scc_index) {
        // Collect ArcFunction refs for SCC members.
        let funcs: Vec<&ArcFunction> = scc
            .members
            .iter()
            .filter_map(|&name| module.get_function(db, name))
            .collect();

        let result = ori_arc::infer_borrow_fixed_point(
            &funcs,
            &external_sigs,
            &classifier,
            &borrowing_builtins,
        );

        tracing::debug!(sig_count = result.len(), "fixed-point complete");
        BorrowSigResult::from_map(result)
    } else {
        // Single function, single pass.
        let name = scc.members[0];
        let Some(func) = module.get_function(db, name) else {
            tracing::warn!(?name, "SCC member not found in module");
            return BorrowSigResult::empty();
        };

        let sig =
            ori_arc::infer_borrow_single(func, &external_sigs, &classifier, &borrowing_builtins);

        let mut map = FxHashMap::default();
        map.insert(name, sig);
        BorrowSigResult::from_map(map)
    }
}

/// Collect callee signatures from other SCCs.
///
/// For each member of the given SCC, extracts callees via instruction scanning.
/// Callees in different SCCs trigger `infer_borrow_scc` queries (creating
/// Salsa dependency edges). Same-SCC callees are skipped (handled by
/// internal fixed-point). External callees (not in any SCC) are skipped
/// (handled conservatively as unknown).
fn collect_callee_sigs(
    db: &dyn crate::db::Db,
    module: ArcModuleInput,
    decomp: &SccDecomposition,
    scc: &ori_arc::Scc,
    self_scc_index: u32,
) -> FxHashMap<Name, AnnotatedSig> {
    let mut external_sigs = FxHashMap::default();

    for &member_name in &scc.members {
        let Some(func) = module.get_function(db, member_name) else {
            continue;
        };

        for callee_name in ori_arc::extract_callees(func) {
            // Skip same-SCC callees (handled by fixed-point within the SCC).
            let Some(callee_scc_idx) = decomp.scc_of(callee_name) else {
                // External callee (not in any SCC) — skip.
                continue;
            };
            if callee_scc_idx == self_scc_index {
                continue;
            }

            // Query the callee's SCC (creates Salsa dependency edge).
            let callee_result = infer_borrow_scc(db, module, callee_scc_idx);
            if let Some(sig) = callee_result.get(callee_name) {
                external_sigs.insert(callee_name, sig.clone());
            }
        }
    }

    tracing::debug!(
        external_callee_count = external_sigs.len(),
        "collected callee sigs"
    );
    external_sigs
}
