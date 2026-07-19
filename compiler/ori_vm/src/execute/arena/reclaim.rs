//! Transactional tracing and reclamation for session-arena values.

use crate::{ExecutionError, ValueKind};

use super::{adaptive_collection_threshold, IteratorSourceState, ValueArena, ValueArenaEntry};
use crate::execute::value::VmValue;

struct CollectionPlan {
    reclaimed: usize,
    cumulative_collections: u64,
    cumulative_reclaimed_entries: u64,
    next_collection_slots: usize,
}

impl ValueArena {
    pub(in crate::execute) fn collect(
        &mut self,
        roots: impl IntoIterator<Item = VmValue>,
        max_entries: usize,
    ) -> Result<(), ExecutionError> {
        let plan = self.plan_collection(roots, max_entries);
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => {
                self.clear_trace_state();
                self.record_peaks();
                return Err(error);
            }
        };

        self.mark_stack.clear();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.entry.is_some() && !slot.marked {
                slot.entry = None;
                self.free.push(index);
            }
            slot.marked = false;
        }
        self.live_entries -= plan.reclaimed;
        self.cumulative_collections = plan.cumulative_collections;
        self.cumulative_reclaimed_entries = plan.cumulative_reclaimed_entries;
        self.next_collection_slots = plan.next_collection_slots;
        self.record_peaks();
        Ok(())
    }

    fn plan_collection(
        &mut self,
        roots: impl IntoIterator<Item = VmValue>,
        max_entries: usize,
    ) -> Result<CollectionPlan, ExecutionError> {
        for root in roots {
            self.enqueue(root, max_entries)?;
            self.mark_reachable(max_entries)?;
        }

        for (index, slot) in self.slots.iter().enumerate() {
            if !slot.marked
                && matches!(
                    slot.entry,
                    Some(ValueArenaEntry::Iterator(super::IteratorState {
                        live: true,
                        ..
                    }))
                )
            {
                return Err(ExecutionError::LiveIteratorReclamation { index });
            }
        }

        let reclaimed = self
            .slots
            .iter()
            .filter(|slot| slot.entry.is_some() && !slot.marked)
            .count();
        let reclaimed_u64 =
            u64::try_from(reclaimed).map_err(|_| ExecutionError::MetricOverflow {
                metric: "cumulative reclaimed value arena entries",
            })?;
        let cumulative_collections =
            self.cumulative_collections
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "cumulative value arena collections",
                })?;
        let cumulative_reclaimed_entries = self
            .cumulative_reclaimed_entries
            .checked_add(reclaimed_u64)
            .ok_or(ExecutionError::MetricOverflow {
                metric: "cumulative reclaimed value arena entries",
            })?;
        let live_entries =
            self.live_entries
                .checked_sub(reclaimed)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live value arena entries",
                })?;
        if let Some(last) = self.slots.len().checked_sub(1) {
            u32::try_from(last)
                .map_err(|_| ExecutionError::ValueArenaHandleOverflow { length: last })?;
        }

        Ok(CollectionPlan {
            reclaimed,
            cumulative_collections,
            cumulative_reclaimed_entries,
            next_collection_slots: adaptive_collection_threshold(live_entries, max_entries),
        })
    }

    fn mark_reachable(&mut self, max_entries: usize) -> Result<(), ExecutionError> {
        while let Some(handle) = self.mark_stack.pop() {
            let entry = *self.entry(handle)?;
            match entry {
                ValueArenaEntry::Aggregate(aggregate) => {
                    for &field in &aggregate.fields[..usize::from(aggregate.length)] {
                        self.enqueue(field, max_entries)?;
                    }
                }
                ValueArenaEntry::Iterator(state) => {
                    if state.live {
                        if let IteratorSourceState::List { source, .. } = state.source {
                            self.enqueue(source, max_entries)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, value: VmValue, max_entries: usize) -> Result<(), ExecutionError> {
        let handle = match value.kind() {
            ValueKind::Aggregate => value.aggregate_handle()?,
            ValueKind::Iterator => value.iterator_handle()?,
            ValueKind::Unit
            | ValueKind::Int
            | ValueKind::Bool
            | ValueKind::Float
            | ValueKind::Char
            | ValueKind::Null
            | ValueKind::ConstantString
            | ValueKind::Heap => return Ok(()),
            ValueKind::IteratorStep => {
                if let Some(item) = value.as_iterator_step()? {
                    self.enqueue(item, max_entries)?;
                }
                return Ok(());
            }
        };
        let slot = self.validated_slot_mut(handle)?;
        if slot.marked {
            return Ok(());
        }
        slot.marked = true;
        if self.mark_stack.len() >= max_entries {
            return Err(ExecutionError::ResourceLimit {
                resource: "value arena mark work",
                limit: max_entries,
            });
        }
        self.mark_stack.push(handle);
        Ok(())
    }

    fn clear_trace_state(&mut self) {
        self.mark_stack.clear();
        for slot in &mut self.slots {
            slot.marked = false;
        }
    }
}
