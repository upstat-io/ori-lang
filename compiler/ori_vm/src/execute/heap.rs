//! Session-local reference-counted VM heap.

mod allocation;
mod ownership;

use crate::ExecutionError;
use ori_repr::executable::FunctionId;

use super::value::VmValue;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct HeapMetrics {
    pub(super) cumulative_allocations: u64,
    pub(super) cumulative_payload_bytes_lower_bound: u128,
    pub(super) live_objects: usize,
    pub(super) live_payload_bytes: usize,
    pub(super) peak_live_objects: usize,
    pub(super) peak_live_payload_bytes: usize,
    pub(super) peak_table_owned_bytes: usize,
}

pub(super) enum HeapObject {
    Vacant,
    List(Vec<VmValue>),
    Builder(Vec<VmValue>),
    String(String),
    Closure {
        callee: FunctionId,
        captures: Vec<VmValue>,
    },
}

impl HeapObject {
    pub(in crate::execute) fn owned_payload_bytes(&self) -> usize {
        match self {
            Self::Vacant => 0,
            Self::List(values) | Self::Builder(values) => {
                owned_capacity_bytes::<VmValue>(values.capacity())
            }
            Self::String(value) => owned_capacity_bytes::<u8>(value.capacity()),
            Self::Closure { captures, .. } => owned_capacity_bytes::<VmValue>(captures.capacity()),
        }
    }

    pub(in crate::execute) fn owned_children(&self) -> &[VmValue] {
        match self {
            Self::List(values) | Self::Builder(values) => values,
            Self::Closure { captures, .. } => captures,
            Self::Vacant | Self::String(_) => &[],
        }
    }

    pub(in crate::execute) fn into_owned_children(self) -> Option<Vec<VmValue>> {
        match self {
            Self::List(values) | Self::Builder(values) => Some(values),
            Self::Closure { captures, .. } => Some(captures),
            Self::Vacant | Self::String(_) => None,
        }
    }
}

pub(super) struct HeapSlot {
    pub(super) references: u32,
    pub(super) object: HeapObject,
}

#[derive(Default)]
pub(super) struct Heap {
    slots: Vec<HeapSlot>,
    free: Vec<u32>,
    live: usize,
    live_payload_bytes: usize,
    peak_live: usize,
    peak_live_payload_bytes: usize,
    peak_table_owned_bytes: usize,
    cumulative_allocations: u64,
    cumulative_payload_bytes_lower_bound: u128,
}

impl Heap {
    pub(super) fn get(&self, value: VmValue) -> Result<&HeapSlot, ExecutionError> {
        let index = value.heap_index()?;
        let slot = self.slot(index)?;
        if matches!(slot.object, HeapObject::Vacant) {
            Err(ExecutionError::ReleasedHeap { index })
        } else {
            Ok(slot)
        }
    }

    pub(in crate::execute) fn get_mut(
        &mut self,
        value: VmValue,
    ) -> Result<&mut HeapSlot, ExecutionError> {
        let index = value.heap_index()?;
        let slot = self.slot_mut(index)?;
        if matches!(slot.object, HeapObject::Vacant) {
            Err(ExecutionError::ReleasedHeap { index })
        } else {
            Ok(slot)
        }
    }

    pub(super) fn references(&self, value: VmValue) -> Result<u32, ExecutionError> {
        Ok(self.get(value)?.references)
    }

    pub(in crate::execute) fn replace_object(
        &mut self,
        value: VmValue,
        replacement: HeapObject,
    ) -> Result<(), ExecutionError> {
        let index = value.heap_index()?;
        let slot = self.slot(index)?;
        if matches!(slot.object, HeapObject::Vacant) {
            return Err(ExecutionError::ReleasedHeap { index });
        }
        let before = slot.object.owned_payload_bytes();
        let after = replacement.owned_payload_bytes();
        let metrics = self.prepare_payload_transition(before, after)?;
        self.slots[index].object = replacement;
        self.commit_payload_transition(metrics);
        Ok(())
    }

    pub(super) fn increment(&mut self, value: VmValue, count: u32) -> Result<(), ExecutionError> {
        if value.kind() != crate::ValueKind::Heap {
            return Ok(());
        }
        let slot = self.get_mut(value)?;
        slot.references = slot
            .references
            .checked_add(count)
            .ok_or(ExecutionError::ReferenceCountOverflow)?;
        Ok(())
    }

    pub(super) fn decrement(
        &mut self,
        value: VmValue,
    ) -> Result<Option<HeapObject>, ExecutionError> {
        if value.kind() != crate::ValueKind::Heap {
            return Ok(None);
        }
        let index = value.heap_index()?;
        let slot = self.get(value)?;
        let references = slot
            .references
            .checked_sub(1)
            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        if references == 0 {
            let payload_bytes = slot.object.owned_payload_bytes();
            let free_index = u32::try_from(index)
                .map_err(|_| ExecutionError::HeapHandleOverflow { length: index })?;
            let live = self
                .live
                .checked_sub(1)
                .ok_or(ExecutionError::ReferenceCountUnderflow)?;
            let live_payload_bytes = self.live_payload_bytes.checked_sub(payload_bytes).ok_or(
                ExecutionError::MetricOverflow {
                    metric: "live heap payload bytes",
                },
            )?;
            let released = std::mem::replace(&mut self.slots[index].object, HeapObject::Vacant);
            self.slots[index].references = 0;
            self.free.push(free_index);
            self.live = live;
            self.live_payload_bytes = live_payload_bytes;
            self.record_table_owned_bytes();
            return Ok(Some(released));
        }
        self.slots[index].references = references;
        Ok(None)
    }

    pub(super) fn is_shared(&self, value: VmValue) -> Result<bool, ExecutionError> {
        if value.kind() != crate::ValueKind::Heap {
            return Ok(false);
        }
        Ok(self.get(value)?.references > 1)
    }

    pub(super) const fn metrics(&self) -> HeapMetrics {
        HeapMetrics {
            cumulative_allocations: self.cumulative_allocations,
            cumulative_payload_bytes_lower_bound: self.cumulative_payload_bytes_lower_bound,
            live_objects: self.live,
            live_payload_bytes: self.live_payload_bytes,
            peak_live_objects: self.peak_live,
            peak_live_payload_bytes: self.peak_live_payload_bytes,
            peak_table_owned_bytes: self.peak_table_owned_bytes,
        }
    }

    pub(super) fn value_roots(&self) -> impl Iterator<Item = VmValue> + '_ {
        self.slots
            .iter()
            .flat_map(|slot| slot.object.owned_children())
            .copied()
    }

    pub(super) fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.live = 0;
        self.live_payload_bytes = 0;
    }

    fn slot(&self, index: usize) -> Result<&HeapSlot, ExecutionError> {
        self.slots
            .get(index)
            .ok_or(ExecutionError::HeapOutOfBounds {
                index,
                bound: self.slots.len(),
            })
    }

    fn slot_mut(&mut self, index: usize) -> Result<&mut HeapSlot, ExecutionError> {
        let bound = self.slots.len();
        self.slots
            .get_mut(index)
            .ok_or(ExecutionError::HeapOutOfBounds { index, bound })
    }

    fn prepare_payload_transition(
        &self,
        before: usize,
        after: usize,
    ) -> Result<(usize, u128), ExecutionError> {
        if after >= before {
            let growth = after - before;
            let live_payload_bytes = self.live_payload_bytes.checked_add(growth).ok_or(
                ExecutionError::MetricOverflow {
                    metric: "live heap payload bytes",
                },
            )?;
            let cumulative_payload_bytes_lower_bound = self
                .cumulative_payload_bytes_lower_bound
                .checked_add(growth as u128)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative heap payload bytes",
                })?;
            Ok((live_payload_bytes, cumulative_payload_bytes_lower_bound))
        } else {
            let live_payload_bytes = self.live_payload_bytes.checked_sub(before - after).ok_or(
                ExecutionError::MetricOverflow {
                    metric: "live heap payload bytes",
                },
            )?;
            Ok((
                live_payload_bytes,
                self.cumulative_payload_bytes_lower_bound,
            ))
        }
    }

    fn commit_payload_transition(&mut self, metrics: (usize, u128)) {
        self.live_payload_bytes = metrics.0;
        self.cumulative_payload_bytes_lower_bound = metrics.1;
        self.peak_live_payload_bytes = self.peak_live_payload_bytes.max(self.live_payload_bytes);
    }

    fn record_table_owned_bytes(&mut self) {
        let slots = owned_capacity_bytes::<HeapSlot>(self.slots.capacity());
        let free = owned_capacity_bytes::<u32>(self.free.capacity());
        self.peak_table_owned_bytes = self.peak_table_owned_bytes.max(slots.saturating_add(free));
    }
}

fn owned_capacity_bytes<T>(capacity: usize) -> usize {
    capacity.saturating_mul(std::mem::size_of::<T>())
}

#[cfg(test)]
mod tests;
