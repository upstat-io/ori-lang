//! Representation optimization IR for the Ori compiler.
//!
//! This crate provides the `MachineRepr` type and `ReprPlan` data structure
//! that records all narrowing decisions between type checking and codegen.
//! The type checker never sees machine representations; codegen never makes
//! narrowing decisions. This separation mirrors Lean 4's LCNF phase.
//!
//! # Architecture
//!
//! ```text
//! ori_types (Pool, Tag, Idx) → ori_arc (ArcFunction) → ori_repr (ReprPlan) → ori_llvm
//! ```
//!
//! `ori_repr` reads from `ori_types` and `ori_arc` but neither depends on it.

#![deny(unsafe_code)]

mod canonical;
mod enum_repr;
pub mod escape;
mod layout;
mod plan;
pub mod range;
mod repr;
mod struct_repr;

#[cfg(test)]
mod tests;

pub use canonical::canonical;
pub use enum_repr::{EnumRepr, EnumTag, VariantRepr};
pub use plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, RcStrategy, ReprAttribute, ReprDecision,
    ReprPlan,
};
pub use repr::{FloatWidth, IntWidth, MachineRepr};
pub use struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};

use ori_arc::ir::ArcFunction;
use ori_types::Pool;

/// Compute the representation plan for all types reachable from the program.
///
/// Called after type checking and ARC borrow inference, before LLVM codegen.
/// The `arc_functions` parameter is unused in §01 but the signature is
/// established now so later sections (§03 range analysis, §08 escape analysis)
/// can add their passes without changing the call sites in `oric`.
///
/// When `policy` is `NarrowingPolicy::Disabled` (`--no-repr-opt`), returns
/// after `populate_canonical()` — canonical representations only, zero
/// behavioral change versus the pre-`ori_repr` pipeline.
pub fn compute_repr_plan(
    pool: &Pool,
    arc_functions: &[ArcFunction],
    policy: NarrowingPolicy,
) -> ReprPlan {
    let mut plan = ReprPlan::new(policy);

    // Phase 1: Set canonical representations for all types (§01).
    canonical::populate_canonical(&mut plan, pool);

    if policy == NarrowingPolicy::Disabled {
        tracing::debug!("repr-opt disabled — returning canonical-only plan");
        return plan;
    }

    // Phase 2: Triviality analysis (§02)
    analyze_triviality(&mut plan, pool);

    // Phase 3: Range analysis (§03) → Integer narrowing (§04) → Float narrowing (§05)
    analyze_ranges(&mut plan, pool, arc_functions);
    apply_integer_narrowing(&mut plan, pool);
    apply_float_narrowing(&mut plan, pool);

    // Phase 4: Struct layout (§06), Enum repr (§07)
    compute_struct_layouts(&mut plan, pool);
    compute_enum_reprs(&mut plan, pool);

    // Phase 5: Escape analysis (§08) → ARC header (§09) → Thread-local (§10)
    analyze_escape(&mut plan, pool, arc_functions);
    compress_arc_headers(&mut plan, pool);
    apply_thread_local_arc(&mut plan, pool, arc_functions);

    // Phase 6: Collection specialization (§11)
    specialize_collections(&mut plan, pool);

    plan
}

// Stubs — replaced by real implementations in §02–§11.
// Each stub is a no-op today; the corresponding section fills it in.

/// §02: Transitive triviality analysis and ARC elision.
fn analyze_triviality(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §03: Value range analysis (interval propagation per function).
fn analyze_ranges(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}

/// §04: Integer narrowing (i64 → i32/i16/i8 when range fits).
fn apply_integer_narrowing(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §05: Float narrowing (f64 → f32 when precision is exact).
fn apply_float_narrowing(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §06: Struct and tuple field reordering for padding minimization.
fn compute_struct_layouts(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §07: Enum niche optimization and discriminant narrowing.
fn compute_enum_reprs(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §08: Escape analysis for stack promotion.
fn analyze_escape(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}

/// §09: ARC header compression (refcount width narrowing).
fn compress_arc_headers(_plan: &mut ReprPlan, _pool: &Pool) {}

/// §10: Thread-local non-atomic ARC (Rc vs Arc selection).
fn apply_thread_local_arc(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}

/// §11: Collection specialization (SSO, SVO, packed bool, element narrowing).
fn specialize_collections(_plan: &mut ReprPlan, _pool: &Pool) {}
