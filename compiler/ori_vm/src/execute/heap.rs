//! Session-local reference-counted VM heap.

use crate::ExecutionError;

use super::value::VmValue;

pub(super) enum HeapObject {
    Vacant,
    List(Vec<VmValue>),
    Builder(Vec<VmValue>),
    String(String),
}

pub(super) struct HeapSlot {
    pub(super) references: u32,
    pub(super) object: HeapObject,
}

#[derive(Default)]
pub(super) struct Heap {
    pub(super) slots: Vec<HeapSlot>,
    free: Vec<u32>,
    pub(super) peak_live: usize,
    live: usize,
}

impl Heap {
    pub(super) fn allocate(
        &mut self,
        object: HeapObject,
        max_objects: usize,
    ) -> Result<VmValue, ExecutionError> {
        if self.live >= max_objects {
            return Err(ExecutionError::ResourceLimit {
                resource: "live heap objects",
                limit: max_objects,
            });
        }
        let index = if let Some(index) = self.free.pop() {
            let slot = self.slot_mut(index as usize)?;
            *slot = HeapSlot {
                references: 1,
                object,
            };
            index
        } else {
            let index = u32::try_from(self.slots.len()).map_err(|_| {
                ExecutionError::HeapHandleOverflow {
                    length: self.slots.len(),
                }
            })?;
            self.slots.push(HeapSlot {
                references: 1,
                object,
            });
            index
        };
        self.live = self
            .live
            .checked_add(1)
            .ok_or(ExecutionError::ResourceLimit {
                resource: "live heap objects",
                limit: max_objects,
            })?;
        self.peak_live = self.peak_live.max(self.live);
        Ok(VmValue::heap(index))
    }

    pub(super) fn get(&self, value: VmValue) -> Result<&HeapSlot, ExecutionError> {
        let index = value.heap_index()?;
        let slot = self.slot(index)?;
        if matches!(slot.object, HeapObject::Vacant) {
            Err(ExecutionError::ReleasedHeap { index })
        } else {
            Ok(slot)
        }
    }

    pub(super) fn get_mut(&mut self, value: VmValue) -> Result<&mut HeapSlot, ExecutionError> {
        let index = value.heap_index()?;
        let slot = self.slot_mut(index)?;
        if matches!(slot.object, HeapObject::Vacant) {
            Err(ExecutionError::ReleasedHeap { index })
        } else {
            Ok(slot)
        }
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

    pub(super) fn decrement(&mut self, value: VmValue) -> Result<(), ExecutionError> {
        if value.kind() != crate::ValueKind::Heap {
            return Ok(());
        }
        let index = value.heap_index()?;
        let slot = self.get_mut(value)?;
        slot.references = slot
            .references
            .checked_sub(1)
            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        if slot.references == 0 {
            slot.object = HeapObject::Vacant;
            let free_index = u32::try_from(index)
                .map_err(|_| ExecutionError::HeapHandleOverflow { length: index })?;
            self.free.push(free_index);
            self.live = self
                .live
                .checked_sub(1)
                .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        }
        Ok(())
    }

    pub(super) fn is_shared(&self, value: VmValue) -> Result<bool, ExecutionError> {
        if value.kind() != crate::ValueKind::Heap {
            return Ok(false);
        }
        Ok(self.get(value)?.references > 1)
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
}
