//! Float narrowing — Phase A: struct field precision analysis.
//!
//! Determines whether `float` (semantic f64) fields can be narrowed to f32
//! when all observed values are exactly representable in f32.
//!
//! Unlike integer narrowing, float narrowing does NOT consume the
//! `ValueRange` lattice (which is integer-only). Instead, it defines its
//! own `FloatRange` lattice that tracks whether ALL observed literal values
//! at a struct field position are f32-exact.
//!
//! # Conservatism
//!
//! - Only storage-only narrowing: no arithmetic result is ever narrowed
//! - Only literal values contribute positive evidence (`F32Exact`)
//! - Any non-literal source (variables, function results) → `Top` (no narrowing)
//! - `#repr("c")` / `#repr("packed")` / `#repr("transparent")` types are
//!   never narrowed (ABI contract)
//! - Public types are never narrowed (ABI contract)

use ori_arc::ir::{ArcFunction, ArcInstr, ArcValue, CtorKind, LitValue};
use ori_arc::ArcVarId;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use crate::plan::ReprPlan;

#[cfg(test)]
mod tests;

/// Float precision lattice for field-level narrowing.
///
/// Unlike `ValueRange` (integer intervals), `FloatRange` tracks whether
/// ALL observed values at a field position are f32-exact, NOT an interval
/// of float values. This is because f32-exactness is a property of
/// individual values, not ranges — `[0.0, 1.0]` as a range contains
/// non-f32-exact values like `0.1`, even though the endpoints are f32-exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatRange {
    /// No observations yet (narrowable — no conflicting evidence).
    /// Joins with any other variant by taking the other variant.
    Bottom,
    /// All observed values are exactly representable in f32.
    F32Exact,
    /// At least one observed value is NOT f32-exact → keep f64.
    Top,
}

impl FloatRange {
    /// Lattice join: widen precision evidence.
    ///
    /// ```text
    /// Bottom ⊔ x      = x
    /// x      ⊔ Bottom = x
    /// Top    ⊔ _      = Top
    /// _      ⊔ Top    = Top
    /// F32Exact ⊔ F32Exact = F32Exact
    /// ```
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (FloatRange::Bottom, x) | (x, FloatRange::Bottom) => x,
            (FloatRange::Top, _) | (_, FloatRange::Top) => FloatRange::Top,
            (FloatRange::F32Exact, FloatRange::F32Exact) => FloatRange::F32Exact,
        }
    }

    /// Observe a float literal value and update the range.
    #[must_use]
    pub fn observe(self, value: f64) -> Self {
        if is_f32_exact(value) {
            self.join(FloatRange::F32Exact)
        } else {
            FloatRange::Top
        }
    }

    /// Observe an arithmetic result (conservatively non-narrowable).
    ///
    /// Any arithmetic on float values can produce results that are not
    /// f32-exact even if the inputs are, due to intermediate rounding
    /// differences between f32 and f64 paths.
    #[must_use]
    pub fn observe_arithmetic(self) -> Self {
        FloatRange::Top
    }

    /// Whether this range allows narrowing to f32.
    ///
    /// Only `F32Exact` allows narrowing. `Bottom` (no observations) is
    /// conservative — we don't narrow fields with no evidence.
    pub fn can_narrow_to_f32(self) -> bool {
        matches!(self, FloatRange::F32Exact)
    }
}

/// Check if an f64 value is exactly representable as f32.
///
/// The roundtrip `f64 → f32 → f64` is lossless iff the value has no
/// precision beyond f32's 24-bit significand. NaN is excluded because
/// NaN payload bits may not survive the roundtrip. Infinities ARE
/// f32-exact (positive and negative infinity have the same bit pattern
/// in f32 and f64). Negative zero IS f32-exact.
///
/// f64 values outside f32 range (magnitude > `f32::MAX` ~ 3.4e38) cast
/// to `f32::INFINITY`, so the roundtrip produces INFINITY != original.
/// These are correctly rejected by the equality check.
///
/// f64 subnormals below f32's smallest subnormal (~1.4e-45) flush to
/// zero in f32, so the roundtrip produces 0.0 != original. These are
/// correctly rejected.
pub(crate) fn is_f32_exact(value: f64) -> bool {
    if value.is_nan() {
        return false;
    }
    // Intentional truncation: this IS the precision test.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f64→f32 truncation is the point of this function"
    )]
    let as_f32 = value as f32;
    let roundtripped = f64::from(as_f32);
    // Intentional exact comparison: we need bit-identical roundtrip.
    #[expect(
        clippy::float_cmp,
        reason = "exact equality is the precision test — no epsilon"
    )]
    {
        roundtripped == value
    }
}

/// Apply float narrowing to struct fields.
///
/// Iterates all types with `Struct` representations, checks each
/// `Float { F64 }` field against the float field summary table, and
/// narrows to `Float { F32 }` when all observed values are f32-exact.
///
/// **Conservatism rules** (same as integer narrowing):
/// - `#repr("c")` / `#repr("packed")` / `#repr("transparent")` → skip
/// - Public types → skip (ABI contract)
/// - `NarrowingPolicy::Disabled` → skip (handled by caller)
/// - Tuples → skip (Phase A defers tuples)
///
/// **Integer narrowing / struct layout interface contract:** This pass
/// writes only `FieldRepr.repr`; `FieldRepr.offset` remains zero (struct
/// layout is the authority for offsets).
pub(crate) fn narrow_float_fields(
    plan: &mut ReprPlan,
    pool: &Pool,
    float_table: &FloatFieldSummaryTable,
) {
    use crate::plan::{DecisionReason, DecisionSource};
    use crate::repr::{FloatWidth, MachineRepr};

    let mut narrowed_count: u32 = 0;

    // Collect struct candidates (same pattern as int.rs:42-52).
    // We must collect first because we need mutable access to the plan.
    let candidates: Vec<Idx> = plan
        .decision_indices()
        .filter_map(|idx| {
            let repr = plan.get_repr(idx)?;
            match repr {
                MachineRepr::Struct(_) => Some(idx),
                // Phase A: skip tuples — same conservative rationale as integer narrowing.
                MachineRepr::Tuple(_) => {
                    tracing::trace!(?idx, "float narrowing: skipping tuple (Phase A)");
                    None
                }
                _ => None,
            }
        })
        .collect();

    for idx in candidates {
        // Skip types ineligible for narrowing (fixed layout / public ABI).
        if !super::is_narrowing_candidate(plan, idx) {
            tracing::trace!(?idx, "float narrowing: skipping — not a candidate");
            continue;
        }

        let Some(MachineRepr::Struct(mut struct_repr)) = plan.get_repr(idx).cloned() else {
            continue;
        };

        // Narrow Float { F64 } fields to Float { F32 } when f32-exact.
        let mut any_changed = false;
        for field in &mut struct_repr.fields {
            let is_canonical_f64 = matches!(
                field.repr,
                MachineRepr::Float {
                    width: FloatWidth::F64
                }
            );
            if !is_canonical_f64 {
                continue;
            }

            let range = float_table.get(idx, field.original_index);
            if range.can_narrow_to_f32() {
                tracing::debug!(
                    ?idx,
                    field_index = field.original_index,
                    "float narrowing: f64 → f32"
                );
                field.repr = MachineRepr::Float {
                    width: FloatWidth::F32,
                };
                any_changed = true;
            }
        }

        if any_changed {
            narrowed_count += 1;
            let range_info = float_field_summary_string(idx, &struct_repr.fields, float_table);
            super::commit_narrowing_decision(
                plan,
                pool,
                idx,
                DecisionSource::FloatNarrowing,
                MachineRepr::Struct(struct_repr),
                DecisionReason::Custom(range_info),
            );
        }
    }

    tracing::debug!(narrowed_count, "float narrowing complete (struct fields)");
}

/// Build a summary string of float field precision for the audit trail.
fn float_field_summary_string(
    idx: Idx,
    fields: &[crate::struct_repr::FieldRepr],
    float_table: &FloatFieldSummaryTable,
) -> String {
    use std::fmt::Write;
    let mut s = String::from("float narrowing: ");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let range = float_table.get(idx, field.original_index);
        let _ = write!(
            s,
            "f{}: {:?} → {:?}",
            field.original_index, range, field.repr
        );
    }
    s
}

// Float field summary table

/// Aggregates float precision evidence per (type, field) pair.
///
/// Scans `Construct` sites for float-typed fields and checks whether
/// ALL values stored at each position are f32-exact literals.
/// Mirrors `FieldSummaryTable` (`range/field_summary.rs`) but for
/// float precision instead of integer ranges.
#[derive(Debug, Default)]
pub struct FloatFieldSummaryTable {
    field_ranges: FxHashMap<(Idx, u32), FloatRange>,
}

impl FloatFieldSummaryTable {
    /// Create an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Join a float precision observation for a (type, field) pair.
    pub fn observe(&mut self, type_idx: Idx, field: u32, range: FloatRange) {
        self.field_ranges
            .entry((type_idx, field))
            .and_modify(|existing| *existing = existing.join(range))
            .or_insert(range);
    }

    /// Query the float precision for a (type, field) pair.
    /// Returns `Bottom` (no observations) if no evidence has been collected.
    #[must_use]
    pub fn get(&self, type_idx: Idx, field: u32) -> FloatRange {
        self.field_ranges
            .get(&(type_idx, field))
            .copied()
            .unwrap_or(FloatRange::Bottom)
    }
}

/// Collect float precision evidence from all `Construct` sites.
///
/// Scans every `ArcFunction` for `Construct` instructions that build
/// structs/tuples/enum variants. For each float-typed field argument,
/// checks whether the argument is a literal f32-exact value.
///
/// This is a single-pass scan (no fixpoint needed) because literal values
/// are statically known from `LitValue::Float`.
pub(crate) fn collect_float_field_summaries(
    _plan: &ReprPlan,
    pool: &Pool,
    arc_functions: &[ArcFunction],
) -> FloatFieldSummaryTable {
    let mut table = FloatFieldSummaryTable::new();

    for func in arc_functions {
        // Pre-scan: build a map of ArcVarId → float literal bits.
        // ARC IR is SSA, so each var has exactly one definition.
        let float_literals = build_float_literal_map(func);

        for block in &func.blocks {
            for instr in &block.body {
                let ArcInstr::Construct { ty, ctor, args, .. } = instr else {
                    continue;
                };

                // Only struct/tuple/enum variant constructors have field positions.
                match ctor {
                    CtorKind::Struct(_) | CtorKind::Tuple | CtorKind::EnumVariant { .. } => {}
                    _ => continue,
                }

                for (i, arg) in args.iter().enumerate() {
                    // Check if the argument is float-typed.
                    let arg_ty = func.var_types.get(arg.index()).copied();
                    let is_float =
                        arg_ty.is_some_and(|t| pool.tag(pool.resolve_fully(t)) == Tag::Float);
                    if !is_float {
                        continue;
                    }

                    // Check if it's a literal f32-exact value.
                    let range = match float_literals.get(arg) {
                        Some(&bits) => FloatRange::Bottom.observe(f64::from_bits(bits)),
                        None => FloatRange::Top, // variable/arithmetic → conservative
                    };
                    let field_idx = u32::try_from(i).unwrap_or(u32::MAX);
                    table.observe(*ty, field_idx, range);
                }
            }
        }
    }

    table
}

/// Build a map from `ArcVarId` to float literal bits (`u64`).
///
/// Scans all blocks for `Let { value: Literal(Float(bits)) }` instructions
/// and records the mapping. Since ARC IR is SSA, each variable is defined
/// exactly once.
fn build_float_literal_map(func: &ArcFunction) -> FxHashMap<ArcVarId, u64> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Literal(LitValue::Float(bits)),
                ..
            } = instr
            {
                map.insert(*dst, *bits);
            }
        }
    }
    map
}
