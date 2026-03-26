//! Field-summary infrastructure for struct/tuple field range tracking.
//!
//! Aggregates value ranges across all `Construct` sites per (type, field)
//! pair. This is the evidence base for §04's struct-field narrowing —
//! without it, `Project` instructions return `Top` for all fields and
//! the `Pixel { r, g, b, a: int }` → 4 bytes optimization is impossible.
//!
//! # Flow
//!
//! 1. Fixpoint loop encounters `Construct { ty, ctor, args }` in a block body
//! 2. `update_field_summaries()` extracts int-typed argument ranges and joins
//!    them into the table via `observe_construct()`
//! 3. `Project` instructions query `table.as_map()` through `TransferContext`
//! 4. After fixpoint completes, `flush_to_repr_plan()` persists results

use ori_arc::ir::{ArcInstr, CtorKind};
use ori_arc::ArcVarId;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::{is_int_typed, ValueRange};

/// Aggregates field ranges across all `Construct` sites for struct/tuple types.
///
/// Each entry `(type_idx, field_index) → ValueRange` represents the join of
/// argument ranges at that position across ALL `Construct` instructions
/// building that type. This cross-site join is the evidence base for §04.
#[derive(Debug)]
pub struct FieldSummaryTable {
    field_ranges: FxHashMap<(Idx, u32), ValueRange>,
}

impl FieldSummaryTable {
    /// Create an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            field_ranges: FxHashMap::default(),
        }
    }

    /// Borrow the underlying map for read-only access (e.g., `TransferContext`).
    #[must_use]
    pub fn as_map(&self) -> &FxHashMap<(Idx, u32), ValueRange> {
        &self.field_ranges
    }

    /// Record one `Construct` site's argument ranges into the summary.
    ///
    /// Each `arg_ranges[i]` is joined with the existing range for `(type_idx, i)`.
    /// This means multiple construction sites widen the range — e.g.,
    /// `Pixel { r: 0 }` then `Pixel { r: 255 }` → field 0 = `[0, 255]`.
    pub fn observe_construct(&mut self, type_idx: Idx, arg_ranges: &[ValueRange]) {
        for (i, &range) in arg_ranges.iter().enumerate() {
            // Struct/tuple field count fits in u32 (max ~4 billion fields).
            let field_idx = u32::try_from(i).unwrap_or(u32::MAX);
            let key = (type_idx, field_idx);
            self.field_ranges
                .entry(key)
                .and_modify(|existing| *existing = existing.join(range))
                .or_insert(range);
        }
    }

    /// Query the aggregated range for a specific field.
    ///
    /// Returns `Top` if no `Construct` site for this `(type_idx, field)` was observed.
    #[must_use]
    pub fn field_range(&self, type_idx: Idx, field: u32) -> ValueRange {
        self.field_ranges
            .get(&(type_idx, field))
            .copied()
            .unwrap_or(ValueRange::Top)
    }

    /// Persist all field ranges into `ReprPlan::field_range_summaries`.
    ///
    /// Called once after the fixpoint completes for all functions. Each
    /// entry is joined (not overwritten) into the plan, so results from
    /// multiple functions accumulate correctly.
    pub fn flush_to_repr_plan(&self, repr_plan: &mut crate::plan::ReprPlan) {
        for (&(idx, field), &range) in &self.field_ranges {
            repr_plan.join_field_range(idx, field, range);
        }
    }
}

impl Default for FieldSummaryTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Update the field-summary table when a `Construct` instruction is encountered.
///
/// Only processes `Struct`, `Tuple`, and `EnumVariant` constructors.
/// `ListLiteral`, `MapLiteral`, `SetLiteral`, and `Closure` constructors
/// don't have meaningful field positions for narrowing.
pub fn update_field_summaries<S: std::hash::BuildHasher>(
    instr: &ArcInstr,
    ranges: &std::collections::HashMap<ArcVarId, ValueRange, S>,
    var_types: &[Idx],
    pool: &Pool,
    table: &mut FieldSummaryTable,
) {
    let ArcInstr::Construct { ty, ctor, args, .. } = instr else {
        return;
    };

    // Only struct, tuple, and enum variant constructors have meaningful fields.
    match ctor {
        CtorKind::Struct(_) | CtorKind::Tuple | CtorKind::EnumVariant { .. } => {}
        CtorKind::ListLiteral
        | CtorKind::MapLiteral
        | CtorKind::SetLiteral
        | CtorKind::Closure { .. } => return,
    }

    let arg_ranges: Vec<ValueRange> = args
        .iter()
        .map(|arg| {
            let arg_ty = var_types.get(arg.index()).copied();
            if arg_ty.is_some_and(|t| is_int_typed(t, pool)) {
                ranges.get(arg).copied().unwrap_or(ValueRange::Top)
            } else {
                ValueRange::Top // non-int fields get Top — §04 ignores them
            }
        })
        .collect();

    table.observe_construct(*ty, &arg_ranges);
}
