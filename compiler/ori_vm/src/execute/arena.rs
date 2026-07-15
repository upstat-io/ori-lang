//! Generation-safe session-owned aggregate and iterator storage.

mod metrics;
mod reclaim;

use crate::{ExecutionError, ValueKind};

use super::value::{ArenaHandle, VmValue};

const INITIAL_COLLECTION_THRESHOLD: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AggregateKind {
    /// Tuple, struct, and other tagless positional products.
    Product,
    /// Logical sum value whose tag projects before its payload fields.
    Variant,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Aggregate {
    pub(super) fields: [VmValue; 4],
    pub(super) length: u8,
    pub(super) variant: u64,
    pub(super) kind: AggregateKind,
}

impl Default for Aggregate {
    fn default() -> Self {
        Self {
            fields: [VmValue::UNIT; 4],
            length: 0,
            variant: 0,
            kind: AggregateKind::Product,
        }
    }
}

impl Aggregate {
    pub(super) fn projected_field(&self, field: usize) -> Result<Option<VmValue>, ExecutionError> {
        match (self.kind, field) {
            (AggregateKind::Variant, 0) => i64::try_from(self.variant)
                .map(VmValue::int)
                .map(Some)
                .map_err(|_| ExecutionError::IntegerOperation {
                    operation: "enum discriminant conversion",
                }),
            (AggregateKind::Variant, field) => Ok(self.payload_field(field - 1)),
            (AggregateKind::Product, field) => Ok(self.payload_field(field)),
        }
    }

    pub(super) fn projected_field_count(&self) -> usize {
        usize::from(self.length) + usize::from(self.kind == AggregateKind::Variant)
    }

    fn payload_field(&self, field: usize) -> Option<VmValue> {
        self.fields
            .get(field)
            .copied()
            .filter(|_| field < usize::from(self.length))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum IteratorSourceState {
    Range {
        current: i64,
        end: i64,
        step: i64,
        inclusive: bool,
    },
    List {
        source: VmValue,
        position: usize,
        owns_source: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct IteratorState {
    pub(super) source: IteratorSourceState,
    pub(super) live: bool,
}

impl Default for IteratorState {
    fn default() -> Self {
        Self {
            source: IteratorSourceState::Range {
                current: 0,
                end: 0,
                step: 1,
                inclusive: false,
            },
            live: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ValueArenaEntry {
    Aggregate(Aggregate),
    Iterator(IteratorState),
}

#[derive(Clone, Copy, Debug)]
struct ValueArenaSlot {
    generation: u32,
    marked: bool,
    entry: Option<ValueArenaEntry>,
}

pub(super) struct ValueArena {
    slots: Vec<ValueArenaSlot>,
    free: Vec<usize>,
    mark_stack: Vec<ArenaHandle>,
    live_entries: usize,
    next_collection_slots: usize,
    cumulative_allocations: u64,
    cumulative_aggregate_allocations: u64,
    cumulative_iterator_allocations: u64,
    cumulative_collections: u64,
    cumulative_reclaimed_entries: u64,
    cumulative_reused_entries: u64,
    peak_live_entries: usize,
    peak_slots: usize,
    peak_owned_bytes: usize,
}

impl Default for ValueArena {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            mark_stack: Vec::new(),
            live_entries: 0,
            next_collection_slots: INITIAL_COLLECTION_THRESHOLD,
            cumulative_allocations: 0,
            cumulative_aggregate_allocations: 0,
            cumulative_iterator_allocations: 0,
            cumulative_collections: 0,
            cumulative_reclaimed_entries: 0,
            cumulative_reused_entries: 0,
            peak_live_entries: 0,
            peak_slots: 0,
            peak_owned_bytes: 0,
        }
    }
}

impl ValueArena {
    pub(super) fn allocation_batch_requires_collection(
        &self,
        count: usize,
        max_entries: usize,
    ) -> bool {
        if count == 0 {
            return false;
        }
        let fresh = count.saturating_sub(self.free.len());
        let final_slots = self.slots.len().saturating_add(fresh);
        let threshold = self.next_collection_slots.min(max_entries);
        self.live_entries.saturating_add(count) > max_entries
            || final_slots > max_entries
            || (fresh > 0 && final_slots > threshold)
    }

    pub(super) fn validate_aggregate_allocation_batch(
        &self,
        count: usize,
        max_entries: usize,
    ) -> Result<(), ExecutionError> {
        if count == 0 {
            return Ok(());
        }
        let live_entries =
            self.live_entries
                .checked_add(count)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live value arena entries",
                })?;
        if live_entries > max_entries {
            return Err(ExecutionError::ResourceLimit {
                resource: "live value arena entries",
                limit: max_entries,
            });
        }

        let reused = count.min(self.free.len());
        let fresh = count - reused;
        let final_slots =
            self.slots
                .len()
                .checked_add(fresh)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "value arena slots",
                })?;
        if final_slots > max_entries {
            return Err(ExecutionError::ResourceLimit {
                resource: "value arena slots",
                limit: max_entries,
            });
        }
        if let Some(last) = final_slots.checked_sub(1) {
            u32::try_from(last)
                .map_err(|_| ExecutionError::ValueArenaHandleOverflow { length: last })?;
        }

        let count_u64 = u64::try_from(count).map_err(|_| ExecutionError::MetricOverflow {
            metric: "cumulative value arena allocations",
        })?;
        self.cumulative_allocations.checked_add(count_u64).ok_or(
            ExecutionError::MetricOverflow {
                metric: "cumulative value arena allocations",
            },
        )?;
        self.cumulative_aggregate_allocations
            .checked_add(count_u64)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "cumulative aggregate arena allocations",
            })?;
        let reused_u64 = u64::try_from(reused).map_err(|_| ExecutionError::MetricOverflow {
            metric: "cumulative reused value arena entries",
        })?;
        self.cumulative_reused_entries
            .checked_add(reused_u64)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "cumulative reused value arena entries",
            })?;

        for &index in self.free.iter().rev().take(reused) {
            self.slot(index)?
                .generation
                .checked_add(1)
                .ok_or(ExecutionError::ValueArenaGenerationOverflow { index })?;
        }
        Ok(())
    }

    pub(super) fn allocate_aggregate(
        &mut self,
        aggregate: Aggregate,
        max_entries: usize,
    ) -> Result<VmValue, ExecutionError> {
        let cumulative_aggregate_allocations = self
            .cumulative_aggregate_allocations
            .checked_add(1)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "cumulative aggregate arena allocations",
            })?;
        let handle = self.allocate(ValueArenaEntry::Aggregate(aggregate), max_entries)?;
        self.cumulative_aggregate_allocations = cumulative_aggregate_allocations;
        Ok(VmValue::aggregate(handle))
    }

    pub(super) fn allocate_iterator(
        &mut self,
        state: IteratorState,
        max_entries: usize,
    ) -> Result<VmValue, ExecutionError> {
        let cumulative_iterator_allocations = self
            .cumulative_iterator_allocations
            .checked_add(1)
            .ok_or(ExecutionError::MetricOverflow {
            metric: "cumulative iterator arena allocations",
        })?;
        let handle = self.allocate(ValueArenaEntry::Iterator(state), max_entries)?;
        self.cumulative_iterator_allocations = cumulative_iterator_allocations;
        Ok(VmValue::iterator(handle))
    }

    pub(super) fn aggregate(&self, value: VmValue) -> Result<&Aggregate, ExecutionError> {
        let handle = value.aggregate_handle()?;
        match self.entry(handle)? {
            ValueArenaEntry::Aggregate(aggregate) => Ok(aggregate),
            ValueArenaEntry::Iterator(_) => Err(ExecutionError::ValueArenaKindMismatch {
                index: handle.index(),
                expected: ValueKind::Aggregate,
                found: ValueKind::Iterator,
            }),
        }
    }

    pub(super) fn aggregate_mut(
        &mut self,
        value: VmValue,
    ) -> Result<&mut Aggregate, ExecutionError> {
        let handle = value.aggregate_handle()?;
        match self.entry_mut(handle)? {
            ValueArenaEntry::Aggregate(aggregate) => Ok(aggregate),
            ValueArenaEntry::Iterator(_) => Err(ExecutionError::ValueArenaKindMismatch {
                index: handle.index(),
                expected: ValueKind::Aggregate,
                found: ValueKind::Iterator,
            }),
        }
    }

    pub(super) fn iterator_mut(
        &mut self,
        value: VmValue,
    ) -> Result<&mut IteratorState, ExecutionError> {
        let handle = value.iterator_handle()?;
        match self.entry_mut(handle)? {
            ValueArenaEntry::Iterator(iterator) => Ok(iterator),
            ValueArenaEntry::Aggregate(_) => Err(ExecutionError::ValueArenaKindMismatch {
                index: handle.index(),
                expected: ValueKind::Iterator,
                found: ValueKind::Aggregate,
            }),
        }
    }

    pub(super) fn iterator(&self, value: VmValue) -> Result<&IteratorState, ExecutionError> {
        let handle = value.iterator_handle()?;
        match self.entry(handle)? {
            ValueArenaEntry::Iterator(iterator) => Ok(iterator),
            ValueArenaEntry::Aggregate(_) => Err(ExecutionError::ValueArenaKindMismatch {
                index: handle.index(),
                expected: ValueKind::Iterator,
                found: ValueKind::Aggregate,
            }),
        }
    }

    pub(super) fn clear(&mut self) {
        self.slots = Vec::new();
        self.free = Vec::new();
        self.mark_stack = Vec::new();
        self.live_entries = 0;
        self.next_collection_slots = INITIAL_COLLECTION_THRESHOLD;
    }

    fn allocate(
        &mut self,
        entry: ValueArenaEntry,
        max_entries: usize,
    ) -> Result<ArenaHandle, ExecutionError> {
        if self.live_entries >= max_entries {
            return Err(ExecutionError::ResourceLimit {
                resource: "live value arena entries",
                limit: max_entries,
            });
        }
        let cumulative_allocations =
            self.cumulative_allocations
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative value arena allocations",
                })?;
        let live_entries =
            self.live_entries
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live value arena entries",
                })?;
        let reused = !self.free.is_empty();
        let cumulative_reused_entries = if reused {
            self.cumulative_reused_entries
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative reused value arena entries",
                })?
        } else {
            self.cumulative_reused_entries
        };

        let handle = if let Some(&index) = self.free.last() {
            let generation = self
                .slot(index)?
                .generation
                .checked_add(1)
                .ok_or(ExecutionError::ValueArenaGenerationOverflow { index })?;
            let handle = ArenaHandle::new(index, generation)?;
            self.free.pop();
            self.slots[index] = ValueArenaSlot {
                generation,
                marked: false,
                entry: Some(entry),
            };
            handle
        } else {
            if self.slots.len() >= max_entries {
                return Err(ExecutionError::ResourceLimit {
                    resource: "value arena slots",
                    limit: max_entries,
                });
            }
            let handle = ArenaHandle::new(self.slots.len(), 1)?;
            self.slots.push(ValueArenaSlot {
                generation: handle.generation(),
                marked: false,
                entry: Some(entry),
            });
            handle
        };
        self.live_entries = live_entries;
        self.cumulative_allocations = cumulative_allocations;
        self.cumulative_reused_entries = cumulative_reused_entries;
        self.record_peaks();
        Ok(handle)
    }

    fn entry(&self, handle: ArenaHandle) -> Result<&ValueArenaEntry, ExecutionError> {
        let slot = self.validated_slot(handle)?;
        slot.entry
            .as_ref()
            .ok_or(ExecutionError::StaleValueArenaHandle {
                index: handle.index(),
                handle_generation: handle.generation(),
                slot_generation: slot.generation,
            })
    }

    fn entry_mut(&mut self, handle: ArenaHandle) -> Result<&mut ValueArenaEntry, ExecutionError> {
        let slot = self.validated_slot_mut(handle)?;
        let slot_generation = slot.generation;
        slot.entry
            .as_mut()
            .ok_or(ExecutionError::StaleValueArenaHandle {
                index: handle.index(),
                handle_generation: handle.generation(),
                slot_generation,
            })
    }

    fn validated_slot(&self, handle: ArenaHandle) -> Result<&ValueArenaSlot, ExecutionError> {
        let slot = self.slot(handle.index())?;
        if slot.generation != handle.generation() || slot.entry.is_none() {
            return Err(ExecutionError::StaleValueArenaHandle {
                index: handle.index(),
                handle_generation: handle.generation(),
                slot_generation: slot.generation,
            });
        }
        Ok(slot)
    }

    fn validated_slot_mut(
        &mut self,
        handle: ArenaHandle,
    ) -> Result<&mut ValueArenaSlot, ExecutionError> {
        let slot = self.slot_mut(handle.index())?;
        if slot.generation != handle.generation() || slot.entry.is_none() {
            return Err(ExecutionError::StaleValueArenaHandle {
                index: handle.index(),
                handle_generation: handle.generation(),
                slot_generation: slot.generation,
            });
        }
        Ok(slot)
    }

    fn slot(&self, index: usize) -> Result<&ValueArenaSlot, ExecutionError> {
        self.slots
            .get(index)
            .ok_or(ExecutionError::ValueArenaHandleOutOfBounds {
                index,
                bound: self.slots.len(),
            })
    }

    fn slot_mut(&mut self, index: usize) -> Result<&mut ValueArenaSlot, ExecutionError> {
        let bound = self.slots.len();
        self.slots
            .get_mut(index)
            .ok_or(ExecutionError::ValueArenaHandleOutOfBounds { index, bound })
    }

    fn record_peaks(&mut self) {
        self.peak_live_entries = self.peak_live_entries.max(self.live_entries);
        self.peak_slots = self.peak_slots.max(self.slots.len());
        self.peak_owned_bytes = self.peak_owned_bytes.max(self.metrics().owned_bytes);
    }
}

pub(super) fn adaptive_collection_threshold(live_entries: usize, max_entries: usize) -> usize {
    live_entries
        .saturating_add(live_entries.max(INITIAL_COLLECTION_THRESHOLD))
        .min(max_entries)
}

#[cfg(test)]
mod tests;
