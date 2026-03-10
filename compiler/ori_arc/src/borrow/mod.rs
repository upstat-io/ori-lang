//! SCC-based borrow inference for ARC IR (Section 06.2).
//!
//! Determines which function parameters can be **borrowed** (no RC operations
//! at the call site) versus **owned** (caller must `rc_inc`, callee must
//! `rc_dec`).
//!
//! # Algorithm
//!
//! Follows Lean 4's approach (`src/Lean/Compiler/IR/Borrow.lean`):
//!
//! 1. **Decompose**: Build call graph, compute SCCs via Tarjan's algorithm.
//! 2. **Initialize**: All non-scalar parameters start as `Borrowed`.
//! 3. **Per-SCC analysis**: Process SCCs in topological order (callees first):
//!    - Non-recursive SCCs: single-pass analysis.
//!    - Recursive SCCs: fixed-point iteration within the SCC only.
//! 4. **Convergence**: Ownership is **monotonic** (Borrowed → Owned, never
//!    backwards). With N parameters per SCC, convergence is guaranteed in
//!    at most N iterations.
//!
//! # Entry Points
//!
//! - [`infer_borrows_scc`]: Full SCC-based pipeline (call graph → SCCs →
//!   per-SCC inference). Used by the JIT evaluator and standalone callers.
//! - [`infer_borrow_single`] / [`infer_borrow_fixed_point`]: Per-SCC building
//!   blocks used by both `infer_borrows_scc` and Salsa-tracked queries.
//!
//! # Projection Ownership Propagation
//!
//! When `Project { dst, value, .. }` extracts a field from a value and `dst`
//! becomes owned (returned or stored), the source `value` must also become
//! owned. Otherwise the caller might free the struct while the projected field
//! is still live. This propagation is transitive and handled naturally by the
//! fixed-point iteration.
//!
//! # Tail Call Preservation
//!
//! When a function tail-calls another function (or itself) and passes a
//! currently-borrowed parameter to an owned position, the parameter must be
//! promoted to owned. Without this, RC insertion would need to insert a `Dec`
//! after the tail call, which would break the tail call optimization (the
//! caller's stack frame must not exist after the tail call).

mod builtins;
mod callees;
mod derived;
mod per_scc;
mod update;

pub use builtins::{
    all_cow_method_names, borrowing_builtin_names, consuming_receiver_builtin_names,
    consuming_receiver_only_builtin_names, consuming_second_arg_builtin_names,
    sharing_builtin_names, BuiltinOwnershipSets,
};
pub use callees::extract_callees;
pub use derived::infer_derived_ownership;
pub use per_scc::{infer_borrow_fixed_point, infer_borrow_single, initialize_single_borrowed};

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::ArcFunction;
use crate::ownership::AnnotatedSig;
use crate::ArcClassification;

// Re-export for per_scc.rs (which uses `super::update_ownership_inner`).
use update::update_ownership_inner;

/// SCC-based borrow inference (replaces whole-program fixed-point).
///
/// Decomposes functions into SCCs via Tarjan's algorithm, then infers
/// borrow annotations per-SCC in topological order. Produces identical
/// results to the old whole-program fixed-point, but enables future
/// incrementality (each SCC can be cached independently).
///
/// # Arguments
///
/// * `functions` — ARC IR functions to analyze (typically one module's worth).
/// * `classifier` — type classifier for determining scalar vs ref types.
/// * `builtins` — pre-interned builtin ownership sets (borrowing methods,
///   protocol builtins, etc.). Used to determine per-argument ownership
///   for compiler-internal callees.
pub fn infer_borrows_scc(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
) -> FxHashMap<Name, AnnotatedSig> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();

    let mut all_sigs = FxHashMap::default();
    for scc in &sccs {
        if scc.is_recursive(&graph) {
            let scc_funcs: Vec<&ArcFunction> = scc
                .members
                .iter()
                .filter_map(|name| func_by_name.get(name).copied())
                .collect();
            let scc_sigs = infer_borrow_fixed_point(&scc_funcs, &all_sigs, classifier, builtins);
            all_sigs.extend(scc_sigs);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let sig = infer_borrow_single(func, &all_sigs, classifier, builtins);
            all_sigs.insert(func.name, sig);
        }
    }
    all_sigs
}

/// Apply borrow inference results back to `ArcFunction` parameters.
///
/// Updates each function's `ArcParam::ownership` in-place based on the
/// annotated signatures produced by [`infer_borrows_scc`]. This is the bridge
/// between analysis (Section 06.2) and downstream passes (Section 07).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn apply_borrows(functions: &mut [ArcFunction], sigs: &FxHashMap<Name, AnnotatedSig>) {
    for func in functions {
        if let Some(sig) = sigs.get(&func.name) {
            for (param, annotated) in func.params.iter_mut().zip(&sig.params) {
                param.ownership = annotated.ownership;
            }
        }
    }
}

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating the ARC pass's borrow inference.
// Compile-time exhaustiveness guard (Roc pattern). See plans/type_strategy_registry/section-14.
#[allow(
    dead_code,
    unreachable_code,
    reason = "compile-time exhaustiveness guard — never called"
)]
fn _enforce_type_tag_exhaustiveness(tag: ori_registry::TypeTag) {
    match tag {
        // All 23 TypeTag variants — Copy types (scalar), Arc types (borrow set),
        // Iterator (excluded), Function (capture analysis)
        ori_registry::TypeTag::Int
        | ori_registry::TypeTag::Float
        | ori_registry::TypeTag::Bool
        | ori_registry::TypeTag::Char
        | ori_registry::TypeTag::Byte
        | ori_registry::TypeTag::Duration
        | ori_registry::TypeTag::Size
        | ori_registry::TypeTag::Ordering
        | ori_registry::TypeTag::Range
        | ori_registry::TypeTag::Unit
        | ori_registry::TypeTag::Never
        | ori_registry::TypeTag::Str
        | ori_registry::TypeTag::Error
        | ori_registry::TypeTag::List
        | ori_registry::TypeTag::Map
        | ori_registry::TypeTag::Set
        | ori_registry::TypeTag::Tuple
        | ori_registry::TypeTag::Option
        | ori_registry::TypeTag::Result
        | ori_registry::TypeTag::Channel
        | ori_registry::TypeTag::Iterator
        | ori_registry::TypeTag::DoubleEndedIterator
        | ori_registry::TypeTag::Function => {}
    }
}

#[cfg(test)]
mod tests;
