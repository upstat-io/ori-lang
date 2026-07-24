//! Range summaries persist analysis evidence while representation selection remains downstream.

use ori_arc::ir::ArcVarId;
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::range::ValueRange;

use super::ReprPlan;

impl ReprPlan {
    /// Record per-variable range analysis results for a function.
    pub fn set_var_ranges(&mut self, func: Name, ranges: FxHashMap<ArcVarId, ValueRange>) {
        self.function_var_ranges.insert(func, ranges);
    }

    /// Get the range for a variable in a function.
    #[must_use]
    pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange {
        self.function_var_ranges
            .get(&func)
            .and_then(|ranges| ranges.get(&var))
            .copied()
            .unwrap_or_default()
    }

    /// Get mutable access to a function's per-variable range map.
    pub fn function_var_ranges_mut(
        &mut self,
        func: Name,
    ) -> Option<&mut FxHashMap<ArcVarId, ValueRange>> {
        self.function_var_ranges.get_mut(&func)
    }

    /// Join a field range into the persistent summary.
    pub fn join_field_range(&mut self, idx: Idx, field: u32, range: ValueRange) {
        self.field_range_summaries
            .entry((idx, field))
            .and_modify(|existing| *existing = existing.join(range))
            .or_insert(range);
    }

    /// Query the aggregated field range for a struct or tuple field.
    #[must_use]
    pub fn field_range(&self, idx: Idx, field: u32) -> ValueRange {
        self.field_range_summaries
            .get(&(idx, field))
            .copied()
            .unwrap_or_default()
    }

    /// Join an element range into the persistent summary for a collection type.
    pub fn join_element_range(&mut self, collection_idx: Idx, range: ValueRange) {
        self.element_range_summaries
            .entry(collection_idx)
            .and_modify(|existing| *existing = existing.join(range))
            .or_insert(range);
    }

    /// Query the aggregated element range for a collection type.
    #[must_use]
    pub fn element_range(&self, collection_idx: Idx) -> ValueRange {
        self.element_range_summaries
            .get(&collection_idx)
            .copied()
            .unwrap_or_default()
    }
}
