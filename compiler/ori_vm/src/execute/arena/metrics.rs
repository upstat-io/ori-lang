//! Deterministic value-arena resource accounting.

use super::{ValueArena, ValueArenaSlot};
use crate::execute::value::ArenaHandle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::execute) struct ValueArenaMetrics {
    pub(in crate::execute) cumulative_allocations: u64,
    pub(in crate::execute) cumulative_aggregate_allocations: u64,
    pub(in crate::execute) cumulative_iterator_allocations: u64,
    pub(in crate::execute) cumulative_collections: u64,
    pub(in crate::execute) cumulative_reclaimed_entries: u64,
    pub(in crate::execute) cumulative_reused_entries: u64,
    pub(in crate::execute) live_entries: usize,
    pub(in crate::execute) slots: usize,
    pub(in crate::execute) owned_bytes: usize,
    pub(in crate::execute) peak_live_entries: usize,
    pub(in crate::execute) peak_slots: usize,
    pub(in crate::execute) peak_owned_bytes: usize,
}

impl ValueArena {
    pub(in crate::execute) const fn metrics(&self) -> ValueArenaMetrics {
        ValueArenaMetrics {
            cumulative_allocations: self.cumulative_allocations,
            cumulative_aggregate_allocations: self.cumulative_aggregate_allocations,
            cumulative_iterator_allocations: self.cumulative_iterator_allocations,
            cumulative_collections: self.cumulative_collections,
            cumulative_reclaimed_entries: self.cumulative_reclaimed_entries,
            cumulative_reused_entries: self.cumulative_reused_entries,
            live_entries: self.live_entries,
            slots: self.slots.len(),
            owned_bytes: owned_capacity_bytes::<ValueArenaSlot>(self.slots.capacity())
                .saturating_add(owned_capacity_bytes::<usize>(self.free.capacity()))
                .saturating_add(owned_capacity_bytes::<ArenaHandle>(
                    self.mark_stack.capacity(),
                )),
            peak_live_entries: self.peak_live_entries,
            peak_slots: self.peak_slots,
            peak_owned_bytes: self.peak_owned_bytes,
        }
    }
}

const fn owned_capacity_bytes<T>(capacity: usize) -> usize {
    capacity.saturating_mul(std::mem::size_of::<T>())
}
