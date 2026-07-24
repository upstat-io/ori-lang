//! Preflight and commit primitives for failure-atomic ownership changes.

use crate::ExecutionError;

use super::{Heap, HeapObject};
use crate::execute::value::VmValue;

impl Heap {
    pub(in crate::execute) fn validate_increment_plan(
        &self,
        values: &[VmValue],
        count: u32,
    ) -> Result<(), ExecutionError> {
        let mut start = 0;
        while start < values.len() {
            let index = values[start].heap_index()?;
            let mut end = start + 1;
            while end < values.len() && values[end].heap_index()? == index {
                end += 1;
            }
            let occurrences =
                u32::try_from(end - start).map_err(|_| ExecutionError::ReferenceCountOverflow)?;
            let increment = occurrences
                .checked_mul(count)
                .ok_or(ExecutionError::ReferenceCountOverflow)?;
            self.get(values[start])?
                .references
                .checked_add(increment)
                .ok_or(ExecutionError::ReferenceCountOverflow)?;
            start = end;
        }
        Ok(())
    }

    pub(in crate::execute) fn commit_increment_plan(&mut self, values: &[VmValue], count: u32) {
        let mut start = 0;
        while start < values.len() {
            let index = values[start]
                .heap_index()
                .unwrap_or_else(|_| unreachable!("retain plan contains only validated heaps"));
            let mut end = start + 1;
            while end < values.len() && values[end].heap_index().unwrap_or(usize::MAX) == index {
                end += 1;
            }
            let occurrences = u32::try_from(end - start)
                .unwrap_or_else(|_| unreachable!("validated retain-plan multiplicity fits in u32"));
            let increment = occurrences * count;
            self.slots[index].references += increment;
            start = end;
        }
    }

    pub(in crate::execute) fn validate_release_metrics(
        &self,
        released_objects: usize,
        released_payload_bytes: usize,
    ) -> Result<(), ExecutionError> {
        self.live
            .checked_sub(released_objects)
            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        self.live_payload_bytes
            .checked_sub(released_payload_bytes)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "live heap payload bytes",
            })?;
        Ok(())
    }

    pub(in crate::execute) fn validate_shared_release(
        &self,
        value: VmValue,
    ) -> Result<(), ExecutionError> {
        if self.get(value)?.references > 1 {
            Ok(())
        } else {
            Err(ExecutionError::ReferenceCountUnderflow)
        }
    }

    pub(in crate::execute) fn commit_shared_release(&mut self, value: VmValue) {
        let index = value
            .heap_index()
            .unwrap_or_else(|_| unreachable!("shared release was prevalidated"));
        debug_assert!(!matches!(self.slots[index].object, HeapObject::Vacant));
        debug_assert!(self.slots[index].references > 1);
        self.slots[index].references -= 1;
    }
}
