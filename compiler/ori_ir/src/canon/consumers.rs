//! Canonical-IR consumer registry — the single pure-data list of crates that
//! depend on `ori_ir::canon` (`CanExpr` / `DecisionTreePool` / the resolved
//! type pool).
//!
//! # Design Invariants
//!
//! 1. **Single semantic authority** — every backend/consumer reads canonical
//!    IR rather than re-deriving program meaning; this module is the
//!    canonical home for which crates depend on `ori_ir::canon`, checked
//!    mechanically by `scripts/crate-dag-lint.py` (each registered crate
//!    must carry a dependency edge on `ori_ir`).
//! 2. **Pure data** — a `const` slice of `ConsumerEntry`, no logic beyond a
//!    linear-scan lookup (mirrors `ori_registry` / `ori_registry::burden`).
//! 3. **`crate_name` is the identity** — the real Cargo package name, the unit
//!    `cargo metadata` resolves. A module-qualified detail (e.g.
//!    `ori_llvm::evaluator`) lives in `description` only and is never checked.
//! 4. **One query** — `entry_for(crate_name)`; no parallel lookup functions.

/// A single `ori_ir::canon` consumer crate, keyed by its real Cargo package name.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConsumerEntry {
    /// Real Cargo package name — the `cargo metadata`-checkable identity.
    pub crate_name: &'static str,
    /// Human-facing detail (module-qualified consumption site, role); doc-only,
    /// never the checked identity.
    pub description: &'static str,
}

/// The canonical set of crates that depend on `ori_ir::canon`.
///
/// The five real `ori_ir::canon` dependents. Meaning-vs-ID classification —
/// tree-walking meaning-consumers (`ori_eval`, `ori_arc`, `ori_llvm`) vs
/// orchestration / ID-only handle-passers (`ori_compiler`, `oric`) vs
/// ID-only importers (`ori_types`, `ori_patterns`) — and per-pool-subset
/// flags are not tracked by this list; it registers dependency-edge
/// membership only.
pub const CANON_CONSUMERS: &[ConsumerEntry] = &[
    // ori_eval's runtime method-dispatch selects among candidates the frontend
    // already resolved and type-checked; it is a scoped-allowed dispatch surface,
    // never a re-derivation of program meaning.
    ConsumerEntry {
        crate_name: "ori_eval",
        description: "interpreter — tree-walks CanExpr; runtime dispatch scoped-allowed",
    },
    ConsumerEntry {
        crate_name: "ori_arc",
        description: "ARC lowering — CanExpr -> ARC IR",
    },
    ConsumerEntry {
        crate_name: "ori_llvm",
        description: "LLVM backend — ori_llvm::evaluator + codegen/monomorphize",
    },
    ConsumerEntry {
        crate_name: "ori_compiler",
        description: "pure compile facade — pipeline wiring",
    },
    ConsumerEntry {
        crate_name: "oric",
        description: "native driver + test harness",
    },
];

/// Look up a registered consumer by its Cargo package name.
#[must_use]
pub fn entry_for(crate_name: &str) -> Option<&'static ConsumerEntry> {
    CANON_CONSUMERS
        .iter()
        .find(|entry| entry.crate_name == crate_name)
}

#[cfg(test)]
mod tests;
