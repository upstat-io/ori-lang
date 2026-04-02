//! Field-summary infrastructure for struct/tuple field range tracking.
//!
//! Aggregates value ranges across all `Construct` sites per (type, field)
//! pair. This is the evidence base for struct-field narrowing —
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

use ori_arc::ir::{ArcInstr, ArcTerminator, CtorKind};
use ori_arc::ArcVarId;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use super::{is_int_typed, ValueRange};

/// Aggregates field ranges across all `Construct` sites for struct/tuple types.
///
/// Each entry `(type_idx, field_index) → ValueRange` represents the join of
/// argument ranges at that position across ALL `Construct` instructions
/// building that type. This cross-site join is the evidence base for integer narrowing.
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

    /// Clear all accumulated field ranges. Used to recompute field summaries
    /// from final (post-narrowing) variable ranges — see TPR-03-016.
    pub fn clear(&mut self) {
        self.field_ranges.clear();
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

/// Aggregates element value ranges across all collection construction and
/// reuse sites for a collection type (e.g., `[int]`).
///
/// Each entry `collection_type_idx → ValueRange` represents the join of
/// all observed element values across `Construct(ListLiteral|SetLiteral)`
/// and `CollectionReuse` instructions building that collection type.
/// This is the evidence base for collection element narrowing.
#[derive(Debug)]
pub struct ElementSummaryTable {
    element_ranges: FxHashMap<Idx, ValueRange>,
}

impl ElementSummaryTable {
    /// Create an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            element_ranges: FxHashMap::default(),
        }
    }

    /// Clear all accumulated element ranges.
    pub fn clear(&mut self) {
        self.element_ranges.clear();
    }

    /// Record element value ranges from a collection construction site.
    ///
    /// Each `arg_ranges[i]` is joined with the existing range for
    /// `collection_type_idx`. Only int-typed elements contribute;
    /// non-int elements cause the range to become `Top` (no narrowing).
    pub fn observe_elements(&mut self, collection_type_idx: Idx, arg_ranges: &[ValueRange]) {
        // Join all element values into a single range for this collection type.
        // An empty literal `[]` contributes nothing (no evidence either way).
        for &range in arg_ranges {
            self.element_ranges
                .entry(collection_type_idx)
                .and_modify(|existing| *existing = existing.join(range))
                .or_insert(range);
        }
    }

    /// Returns the number of collection types that have observed element ranges.
    #[must_use]
    pub fn observation_count(&self) -> usize {
        self.element_ranges.len()
    }

    /// Query the aggregated element range for a collection type.
    ///
    /// Returns `Top` if no construction sites were observed.
    #[must_use]
    pub fn element_range(&self, collection_type_idx: Idx) -> ValueRange {
        self.element_ranges
            .get(&collection_type_idx)
            .copied()
            .unwrap_or(ValueRange::Top)
    }

    /// Persist all element ranges into `ReprPlan::element_range_summaries`.
    ///
    /// Called once after the fixpoint completes for all functions. Each
    /// entry is joined (not overwritten) into the plan, so results from
    /// multiple functions accumulate correctly.
    pub fn flush_to_repr_plan(&self, repr_plan: &mut crate::plan::ReprPlan) {
        for (&idx, &range) in &self.element_ranges {
            repr_plan.join_element_range(idx, range);
        }
    }
}

impl Default for ElementSummaryTable {
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
                ValueRange::Top // non-int fields get Top — narrowing ignores them
            }
        })
        .collect();

    table.observe_construct(*ty, &arg_ranges);
}

/// Update the element-summary table when a collection construction, reuse,
/// or mutation instruction is encountered.
///
/// Tracks element value ranges from `Construct(ListLiteral|SetLiteral)` and
/// `CollectionReuse` instructions. Only int-typed elements contribute bounded
/// ranges; non-int elements cause `Top` (no narrowing).
///
/// Any `Apply` or `ApplyIndirect` returning a collection with int elements
/// widens the element range to `Top`. This prevents unsound narrowing when
/// push, map, or user functions produce elements outside the literal-only
/// range (BUG-05-001).
pub fn update_element_summaries<S: std::hash::BuildHasher>(
    instr: &ArcInstr,
    ranges: &std::collections::HashMap<ArcVarId, ValueRange, S>,
    var_types: &[Idx],
    pool: &Pool,
    table: &mut ElementSummaryTable,
) {
    // BUG-05-001: function calls returning collection types with int elements
    // widen the element range to Top. Push, map, user functions, and closures
    // can produce elements outside the literal-only observed range.
    let call_ty = match instr {
        ArcInstr::Apply { ty, .. } | ArcInstr::ApplyIndirect { ty, .. } => Some(*ty),
        _ => None,
    };
    if let Some(ty) = call_ty {
        let inner_ty = match pool.tag(ty) {
            Tag::List => Some(pool.list_elem(ty)),
            Tag::Set => Some(pool.set_elem(ty)),
            _ => None,
        };
        if inner_ty.is_some_and(|t| is_int_typed(t, pool)) {
            table.observe_elements(ty, &[ValueRange::Top]);
        }
        return;
    }

    let (ty, args) = match instr {
        ArcInstr::Construct {
            ty,
            ctor: CtorKind::ListLiteral | CtorKind::SetLiteral,
            args,
            ..
        }
        | ArcInstr::CollectionReuse { ty, args, .. } => (*ty, args.as_slice()),
        _ => return,
    };

    // Check if the collection's inner element type is int.
    // For `[int]`, `ty` is the list type idx; we need the inner element type.
    let inner_ty = match pool.tag(ty) {
        Tag::List => Some(pool.list_elem(ty)),
        Tag::Set => Some(pool.set_elem(ty)),
        _ => None,
    };
    let is_int_collection = inner_ty.is_some_and(|t| is_int_typed(t, pool));

    if !is_int_collection {
        return;
    }

    let arg_ranges: Vec<ValueRange> = args
        .iter()
        .map(|arg| {
            let arg_ty = var_types.get(arg.index()).copied();
            if arg_ty.is_some_and(|t| is_int_typed(t, pool)) {
                ranges.get(arg).copied().unwrap_or(ValueRange::Top)
            } else {
                ValueRange::Top
            }
        })
        .collect();

    table.observe_elements(ty, &arg_ranges);
}

/// Update the element-summary table for block terminators.
///
/// Handles `Invoke` terminators (function calls that may unwind) the same
/// way `update_element_summaries` handles `Apply` — any Invoke returning a
/// collection with int elements widens the element range to `Top`.
///
/// This is critical because collection-mutating methods like `push` are
/// lowered as `Invoke` (they can panic), not `Apply` (BUG-05-001).
pub fn update_element_summaries_from_terminator(
    terminator: &ArcTerminator,
    pool: &Pool,
    table: &mut ElementSummaryTable,
) {
    let ty = match terminator {
        ArcTerminator::Invoke { ty, .. } => *ty,
        _ => return,
    };
    let inner_ty = match pool.tag(ty) {
        Tag::List => Some(pool.list_elem(ty)),
        Tag::Set => Some(pool.set_elem(ty)),
        _ => None,
    };
    if inner_ty.is_some_and(|t| is_int_typed(t, pool)) {
        table.observe_elements(ty, &[ValueRange::Top]);
    }
}
