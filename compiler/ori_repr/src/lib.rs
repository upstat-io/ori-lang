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
//!
//! # Salsa Integration (§01.6)
//!
//! [`compute_repr_plan()`] is **not** a Salsa query. It is a pure function
//! that runs imperatively after type checking and ARC borrow inference:
//!
//! - **AOT path** (`codegen_pipeline.rs`): called once, result passed as
//!   `&ReprPlan` to `TypeLayoutResolver` and then to codegen.
//! - **JIT path** (`evaluator/compile.rs`): called per compilation unit,
//!   same ownership model.
//!
//! The `ReprPlan` is recomputed on every compilation. It has no interior
//! mutability (`Send + Sync` by construction), unlike `TypeInfoStore`
//! which uses `RefCell` for lazy population.

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

pub use enum_repr::{EnumRepr, EnumTag, VariantRepr};
pub use plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, RcStrategy, ReprAttribute, ReprDecision,
    ReprPlan,
};
pub use repr::{FloatWidth, IntWidth, MachineRepr};
pub use struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};

use ori_arc::ir::ArcFunction;
use ori_ir::ReprAttrKind;
use ori_types::{Idx, Pool};

/// Compute the representation plan for all types reachable from the program.
///
/// Called after type checking and ARC borrow inference, before LLVM codegen.
/// The `arc_functions` parameter is unused in §01 but the signature is
/// established now so later sections (§03 range analysis, §08 escape analysis)
/// can add their passes without changing the call sites in `oric`.
///
/// `repr_attrs` carries user-specified `#repr` attributes from the type
/// registry, keyed by pool `Idx`. Each entry is converted to
/// `ReprAttribute` and stored in the plan for §06/§07 to query.
///
/// When `policy` is `NarrowingPolicy::Disabled` (`--no-repr-opt`), returns
/// after `populate_canonical()` — canonical representations only, zero
/// behavioral change versus the pre-`ori_repr` pipeline.
pub fn compute_repr_plan(
    pool: &Pool,
    arc_functions: &[ArcFunction],
    policy: NarrowingPolicy,
    repr_attrs: &[(Idx, ReprAttrKind)],
) -> ReprPlan {
    let mut plan = ReprPlan::new(policy);

    // Phase 0: Store user-specified #repr attributes (§01.7).
    for &(idx, ref attr) in repr_attrs {
        plan.set_repr_attr(idx, convert_repr_attr_kind(attr));
    }

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

/// §02: Transitive triviality validation pass.
///
/// Validates that `classify_triviality()` (the canonical source of truth in
/// `ori_types`) agrees with `is_trivial_repr()` for every type that has a
/// stored canonical representation in the `ReprPlan`. Types without a stored
/// repr are skipped (they weren't canonicalized — typically unresolved or
/// error types that don't reach codegen).
///
/// This is a **validation-only** pass — it does not modify the `ReprPlan`.
/// The canonical pass (`populate_canonical()`) already embeds the correct
/// triviality into `StructRepr.trivial`, `TupleRepr.trivial`, and enum
/// variant walks. Any mismatch is a bug in either `classify_triviality()`
/// or `populate_canonical()`.
fn analyze_triviality(plan: &mut ReprPlan, pool: &Pool) {
    use ori_types::triviality::{classify_triviality, Triviality};

    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    let mut validated: u32 = 0;
    let mut mismatches: u32 = 0;

    for raw in 0..pool_len {
        let idx = ori_types::Idx::from_raw(raw);

        // Skip types without a stored repr — they weren't canonicalized.
        let Some(repr) = plan.get_repr(idx) else {
            continue;
        };

        let pool_triviality = classify_triviality(idx, pool);
        let repr_trivial = crate::layout::is_trivial_repr(repr);

        match pool_triviality {
            Triviality::Trivial if !repr_trivial => {
                tracing::warn!(
                    ?idx,
                    "triviality mismatch: classify_triviality says Trivial, ReprPlan says non-trivial"
                );
                mismatches += 1;
            }
            Triviality::NonTrivial if repr_trivial => {
                tracing::warn!(
                    ?idx,
                    "triviality mismatch: classify_triviality says NonTrivial, ReprPlan says trivial"
                );
                mismatches += 1;
            }
            _ => {} // Agrees, or Unknown (can't validate — conservative OK)
        }
        validated += 1;
    }

    tracing::debug!(validated, mismatches, "triviality validation complete");
    // NOTE: Known mismatch exists for Iterator/DoubleEndedIterator:
    // classify_triviality() correctly classifies them as Trivial (Box-allocated,
    // no RC header), but populate_canonical() maps them to MachineRepr::OpaquePtr
    // which is_trivial_repr() classifies as non-trivial. This is a representation
    // limitation — OpaquePtr doesn't distinguish managed (RC) vs unmanaged (Box)
    // pointers. Tracked for resolution in a future MachineRepr refinement.
    if mismatches > 0 {
        tracing::warn!(
            mismatches,
            "triviality classification disagrees with canonical repr — \
             likely Iterator/OpaquePtr coarseness (see §02.2b note)"
        );
    }
}

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

/// Convert IR-level `ReprAttrKind` to repr-opt `ReprAttribute`.
fn convert_repr_attr_kind(kind: &ReprAttrKind) -> ReprAttribute {
    match *kind {
        ReprAttrKind::C => ReprAttribute::C,
        ReprAttrKind::Packed => ReprAttribute::Packed,
        ReprAttrKind::Transparent => ReprAttribute::Transparent,
        ReprAttrKind::Aligned(n) => ReprAttribute::Aligned(u32::try_from(n).unwrap_or(u32::MAX)),
        ReprAttrKind::CAligned(n) => ReprAttribute::CAligned(u32::try_from(n).unwrap_or(u32::MAX)),
    }
}
