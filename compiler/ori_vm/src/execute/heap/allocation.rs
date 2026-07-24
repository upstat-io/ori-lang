//! Reservation and commit for failure-atomic heap allocation.

use crate::ExecutionError;

use super::{Heap, HeapObject, HeapSlot};
use crate::execute::value::VmValue;

#[derive(Clone, Copy)]
struct AllocationMetrics {
    live: usize,
    live_payload_bytes: usize,
    cumulative_allocations: u64,
    cumulative_payload_bytes_lower_bound: u128,
}

pub(in crate::execute) struct PreparedHeapAllocation {
    object: HeapObject,
    index: u32,
    reuses_slot: bool,
    metrics: AllocationMetrics,
}

pub(in crate::execute) struct PreparedHeapBatch {
    indices: Vec<u32>,
    reused_slots: usize,
    payload_bytes: Vec<usize>,
    metrics: AllocationMetrics,
}

impl PreparedHeapBatch {
    pub(in crate::execute) fn value_at(&self, index: usize) -> VmValue {
        VmValue::heap(self.indices[index])
    }
}

impl Heap {
    pub(in crate::execute) fn allocate(
        &mut self,
        object: HeapObject,
        max_objects: usize,
    ) -> Result<VmValue, ExecutionError> {
        let prepared = self.prepare_allocation(object, max_objects)?;
        Ok(self.commit_allocation(prepared))
    }

    pub(in crate::execute) fn prepare_allocation(
        &self,
        object: HeapObject,
        max_objects: usize,
    ) -> Result<PreparedHeapAllocation, ExecutionError> {
        let payload_bytes = object.owned_payload_bytes();
        let metrics = self.prepare_metrics(1, payload_bytes, max_objects)?;
        let (index, reuses_slot) = if let Some(&index) = self.free.last() {
            (index, true)
        } else {
            let index = u32::try_from(self.slots.len()).map_err(|_| {
                ExecutionError::HeapHandleOverflow {
                    length: self.slots.len(),
                }
            })?;
            (index, false)
        };
        Ok(PreparedHeapAllocation {
            object,
            index,
            reuses_slot,
            metrics,
        })
    }

    pub(in crate::execute) fn commit_allocation(
        &mut self,
        prepared: PreparedHeapAllocation,
    ) -> VmValue {
        if prepared.reuses_slot {
            let popped = self.free.pop();
            debug_assert_eq!(popped, Some(prepared.index));
            self.slots[prepared.index as usize] = HeapSlot {
                references: 1,
                object: prepared.object,
            };
        } else {
            debug_assert_eq!(prepared.index as usize, self.slots.len());
            self.slots.push(HeapSlot {
                references: 1,
                object: prepared.object,
            });
        }
        self.commit_metrics(prepared.metrics);
        VmValue::heap(prepared.index)
    }

    pub(in crate::execute) fn prepare_batch(
        &self,
        objects: &[HeapObject],
        max_objects: usize,
    ) -> Result<PreparedHeapBatch, ExecutionError> {
        let count = objects.len();
        self.validate_allocation_count(count, max_objects)?;
        let payload_bytes = objects
            .iter()
            .map(HeapObject::owned_payload_bytes)
            .collect::<Vec<_>>();
        let total_payload = payload_bytes.iter().try_fold(0_usize, |total, &payload| {
            total
                .checked_add(payload)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live heap payload bytes",
                })
        })?;
        let metrics = self.prepare_metrics(count, total_payload, max_objects)?;
        let reused_slots = count.min(self.free.len());
        let mut indices = Vec::with_capacity(count);
        indices.extend(self.free.iter().rev().take(reused_slots).copied());
        for offset in 0..count - reused_slots {
            let index =
                self.slots
                    .len()
                    .checked_add(offset)
                    .ok_or(ExecutionError::HeapHandleOverflow {
                        length: self.slots.len(),
                    })?;
            indices.push(
                u32::try_from(index)
                    .map_err(|_| ExecutionError::HeapHandleOverflow { length: index })?,
            );
        }
        Ok(PreparedHeapBatch {
            indices,
            reused_slots,
            payload_bytes,
            metrics,
        })
    }

    pub(in crate::execute) fn validate_allocation_count(
        &self,
        count: usize,
        max_objects: usize,
    ) -> Result<(), ExecutionError> {
        let requested_live =
            self.live
                .checked_add(count)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live heap objects",
                })?;
        if requested_live > max_objects {
            Err(ExecutionError::ResourceLimit {
                resource: "live heap objects",
                limit: max_objects,
            })
        } else {
            Ok(())
        }
    }

    pub(in crate::execute) fn commit_batch(
        &mut self,
        prepared: &PreparedHeapBatch,
        objects: Vec<HeapObject>,
    ) {
        debug_assert_eq!(objects.len(), prepared.indices.len());
        for (position, (index, object)) in prepared.indices.iter().copied().zip(objects).enumerate()
        {
            debug_assert_eq!(
                object.owned_payload_bytes(),
                prepared.payload_bytes[position]
            );
            if position < prepared.reused_slots {
                let popped = self.free.pop();
                debug_assert_eq!(popped, Some(index));
                self.slots[index as usize] = HeapSlot {
                    references: 1,
                    object,
                };
            } else {
                debug_assert_eq!(index as usize, self.slots.len());
                self.slots.push(HeapSlot {
                    references: 1,
                    object,
                });
            }
        }
        self.commit_metrics(prepared.metrics);
    }

    fn prepare_metrics(
        &self,
        count: usize,
        payload_bytes: usize,
        max_objects: usize,
    ) -> Result<AllocationMetrics, ExecutionError> {
        let live = self
            .live
            .checked_add(count)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "live heap objects",
            })?;
        if live > max_objects {
            return Err(ExecutionError::ResourceLimit {
                resource: "live heap objects",
                limit: max_objects,
            });
        }
        let allocation_count =
            u64::try_from(count).map_err(|_| ExecutionError::MetricOverflow {
                metric: "cumulative heap allocations",
            })?;
        Ok(AllocationMetrics {
            live,
            live_payload_bytes: self.live_payload_bytes.checked_add(payload_bytes).ok_or(
                ExecutionError::MetricOverflow {
                    metric: "live heap payload bytes",
                },
            )?,
            cumulative_allocations: self
                .cumulative_allocations
                .checked_add(allocation_count)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative heap allocations",
                })?,
            cumulative_payload_bytes_lower_bound: self
                .cumulative_payload_bytes_lower_bound
                .checked_add(payload_bytes as u128)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative heap payload bytes",
                })?,
        })
    }

    fn commit_metrics(&mut self, metrics: AllocationMetrics) {
        self.live = metrics.live;
        self.live_payload_bytes = metrics.live_payload_bytes;
        self.cumulative_allocations = metrics.cumulative_allocations;
        self.cumulative_payload_bytes_lower_bound = metrics.cumulative_payload_bytes_lower_bound;
        self.peak_live = self.peak_live.max(self.live);
        self.peak_live_payload_bytes = self.peak_live_payload_bytes.max(self.live_payload_bytes);
        self.record_table_owned_bytes();
    }
}
