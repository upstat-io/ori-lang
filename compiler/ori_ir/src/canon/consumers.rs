//! Canonical-IR consumer registry — the single pure-data list of crates that
//! touch the frontend meaning surface (`CanExpr` / `DecisionTreePool` / the
//! resolved type pool).
//!
//! # Design Invariants
//!
//! 1. **Single semantic authority** — every backend/consumer reads frontend
//!    output rather than re-deriving program meaning; canonical home for
//!    `ori_ir::canon` consumers, checked by `scripts/crate-dag-lint.py`.
//! 2. **Closed consumption classification** — a crate touches the meaning
//!    surface in exactly ONE of [`PoolAccess`]'s three shapes (never as
//!    independent booleans, which would make overlapping combinations
//!    representable); [`PoolAccess::MeaningConsumer`]'s pool set is non-empty,
//!    pinned by a test.
//! 3. **Pure data** — a `const` slice of `ConsumerEntry`, no logic beyond a
//!    linear-scan lookup (mirrors `ori_registry` / `ori_registry::burden`).
//! 4. **`crate_name` is the identity** — the real Cargo package name, the unit
//!    `cargo metadata` resolves. A module-qualified detail (e.g.
//!    `ori_llvm::evaluator`) lives in `description` only and is never checked.
//! 5. **One query** — `entry_for(crate_name)`; no parallel lookup functions.

/// One of the three shared meaning pools the frontend produces and a consumer
/// tree-walks.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CanonPool {
    /// Canonical-expression nodes (`CanExpr`) in the canon arena.
    CanExpr,
    /// Pre-compiled pattern-match decision trees (`DecisionTreePool`).
    DecisionTreePool,
    /// The resolved type pool — every node's fully-resolved `Idx`.
    ResolvedTypePool,
}

/// How a registered crate touches the frontend meaning surface — the closed set
/// of the only three consumption shapes.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum PoolAccess {
    /// Tree-walks the listed pools for program meaning. The slice is the
    /// non-empty read-set of pools the crate actually reads.
    MeaningConsumer(&'static [CanonPool]),
    /// Wires the pipeline, passing pools and ids as opaque handles without
    /// tree-walking them for meaning.
    OrchestrationIdOnly,
    /// Imports a canon id/handle as a bare index or key, never a meaning
    /// re-derivation.
    IdOnlyImporter,
}

/// A single `ori_ir::canon` consumer crate, keyed by its real Cargo package name.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConsumerEntry {
    /// Real Cargo package name — the `cargo metadata`-checkable identity.
    pub crate_name: &'static str,
    /// Which meaning-surface pools this crate reads, as the closed
    /// classification.
    pub pool_access: PoolAccess,
    /// Human-facing detail (module-qualified consumption site, role); doc-only,
    /// never the checked identity.
    pub description: &'static str,
}

/// The canonical set of crates that depend on `ori_ir::canon`.
///
/// Each entry names its consumption shape via [`ConsumerEntry::pool_access`]:
/// meaning-consumers tree-walk pools, orchestration facades pass opaque
/// handles, id-only importers import a canon id/handle as a bare index/key —
/// registered deliberately, never silently omitted. Canon PRODUCERS
/// (`ori_canon`) hold an `ori_ir::canon` edge but consume no meaning, so they
/// are correctly absent from this curated consumer set.
pub const CANON_CONSUMERS: &[ConsumerEntry] = &[
    // Interpreter tree-walks CanExpr (via CanId) and the compiled decision
    // trees; no type-checker dependency means it reads no resolved type pool.
    // Runtime method dispatch is a scoped-allowed surface, never meaning re-derivation.
    ConsumerEntry {
        crate_name: "ori_eval",
        pool_access: PoolAccess::MeaningConsumer(&[
            CanonPool::CanExpr,
            CanonPool::DecisionTreePool,
        ]),
        description:
            "interpreter — tree-walks CanExpr + decision trees; runtime dispatch scoped-allowed",
    },
    // ARC lowering tree-walks CanExpr and the decision trees for Match, and
    // reads the resolved type pool for every lowered node's type.
    ConsumerEntry {
        crate_name: "ori_arc",
        pool_access: PoolAccess::MeaningConsumer(&[
            CanonPool::CanExpr,
            CanonPool::DecisionTreePool,
            CanonPool::ResolvedTypePool,
        ]),
        description:
            "ARC lowering — CanExpr + decision trees -> ARC IR, over the resolved type pool",
    },
    // LLVM backend delegates the CanExpr/decision-tree walk to ARC lowering
    // (it passes the CanId + CanonResult handle down); its own meaning
    // consumption is the resolved type pool it reads for layout and codegen.
    ConsumerEntry {
        crate_name: "ori_llvm",
        pool_access: PoolAccess::MeaningConsumer(&[CanonPool::ResolvedTypePool]),
        description:
            "LLVM backend — reads the resolved type pool for layout/codegen; ARC walk delegated",
    },
    // Pure compile facade — wires the pipeline and passes canon handles through;
    // does not tree-walk any pool for meaning.
    ConsumerEntry {
        crate_name: "ori_compiler",
        pool_access: PoolAccess::OrchestrationIdOnly,
        description: "pure compile facade — pipeline wiring",
    },
    // Native driver + Salsa orchestration — passes canon handles between
    // queries; does not tree-walk any pool for meaning.
    ConsumerEntry {
        crate_name: "oric",
        pool_access: PoolAccess::OrchestrationIdOnly,
        description: "native driver + Salsa orchestration",
    },
    // Imports MonoInstanceId as a sparse-side-table index/key for
    // monomorphization dispatch — an opaque handle, never a meaning
    // re-derivation. The type checker PRODUCES the resolved type pool.
    ConsumerEntry {
        crate_name: "ori_types",
        pool_access: PoolAccess::IdOnlyImporter,
        description: "type checker — imports MonoInstanceId as a dispatch index",
    },
    // Imports CanId + SharedCanonResult as opaque handles stored on a function
    // value for closure-body evaluation; the actual walk runs through the
    // evaluator callback, not here.
    ConsumerEntry {
        crate_name: "ori_patterns",
        pool_access: PoolAccess::IdOnlyImporter,
        description: "value model — stores CanId + canon handle for closure bodies",
    },
    // Holds a `&CanonResult` alongside a `CanId`/`MonoInstanceId` as opaque
    // monomorphization handles; layout computation reads the resolved type
    // inputs it is handed, never tree-walks a canon pool for meaning.
    ConsumerEntry {
        crate_name: "ori_repr",
        pool_access: PoolAccess::IdOnlyImporter,
        description: "repr planning — holds canon handles for monomorphization targets",
    },
    // Holds a `&CanonResult` inside `CodegenInput` purely as a pass-through
    // handle a codegen backend consumes; the contract crate itself performs no
    // traversal and derives no meaning.
    ConsumerEntry {
        crate_name: "ori_codegen",
        pool_access: PoolAccess::IdOnlyImporter,
        description: "codegen contract — passes the canon handle through CodegenInput to backends",
    },
];

/// Look up a registered consumer by its Cargo package name.
// test-only: consumed by `consumers::tests`.
#[cfg(test)]
#[must_use]
pub(crate) fn entry_for(crate_name: &str) -> Option<&'static ConsumerEntry> {
    CANON_CONSUMERS
        .iter()
        .find(|entry| entry.crate_name == crate_name)
}

#[cfg(test)]
mod tests;
