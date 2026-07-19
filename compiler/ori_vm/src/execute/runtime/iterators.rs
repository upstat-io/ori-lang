//! Iterator creation, advancement, and destruction.

use ori_arc::ArgOwnership;
use ori_repr::executable::{IteratorSource, RuntimeCall};

use crate::ExecutionError;

use super::super::arena::{IteratorSourceState, IteratorState};
use super::super::heap::HeapObject;
use super::super::operands::FrameSlot;
use super::super::value::VmValue;
use super::super::{ArenaAllocationRoots, Interpreter};

impl Interpreter<'_> {
    pub(super) fn create_iterator_value(
        &mut self,
        frame: usize,
        destination: FrameSlot,
        source_kind: IteratorSource,
        source_value: VmValue,
        ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let source_state = match source_kind {
            IteratorSource::Range => {
                let aggregate = *self.aggregate(source_value)?;
                if aggregate.length != 4 {
                    return Err(ExecutionError::AggregateFieldOutOfBounds {
                        field: 3,
                        length: usize::from(aggregate.length),
                    });
                }
                let step = aggregate.fields[2].as_int()?;
                if step == 0 {
                    return Err(ExecutionError::ZeroRangeStep);
                }
                IteratorSourceState::Range {
                    current: aggregate.fields[0].as_int()?,
                    end: aggregate.fields[1].as_int()?,
                    step,
                    inclusive: aggregate.fields[3].as_int()? != 0,
                }
            }
            IteratorSource::List => {
                if !matches!(self.heap.get(source_value)?.object, HeapObject::List(_)) {
                    return Err(ExecutionError::InvalidHeapObject {
                        call: RuntimeCall::Iter(IteratorSource::List),
                    });
                }
                IteratorSourceState::List {
                    source: source_value,
                    position: 0,
                    owns_source: ownership == ArgOwnership::Owned,
                }
            }
            IteratorSource::Str
            | IteratorSource::Map
            | IteratorSource::Set
            | IteratorSource::Option => {
                return Err(ExecutionError::UnsupportedIteratorSource {
                    iterator_source: source_kind,
                });
            }
        };
        let state = IteratorState {
            source: source_state,
            live: true,
        };
        self.prepare_value_arena_allocation(ArenaAllocationRoots::overwriting(
            frame,
            destination,
            &[source_value],
        ))?;
        self.value_arena
            .allocate_iterator(state, self.config.max_value_arena_entries)
    }

    pub(super) fn iter_next_value(&mut self, iterator: VmValue) -> Result<VmValue, ExecutionError> {
        let state = *self.value_arena.iterator(iterator)?;
        if !state.live {
            return Err(ExecutionError::EscapingIterator);
        }
        let value = match state.source {
            IteratorSourceState::Range {
                current,
                end,
                step,
                inclusive,
            } => {
                if range_has_value(current, end, step, inclusive) {
                    let value = current;
                    let next =
                        current
                            .checked_add(step)
                            .ok_or(ExecutionError::IntegerOperation {
                                operation: "range iteration",
                            })?;
                    let state = self.iterator_mut(iterator)?;
                    let IteratorSourceState::Range { current, .. } = &mut state.source else {
                        return Err(ExecutionError::UnsupportedPrimitive {
                            operation: "iterator source changed during range iteration",
                        });
                    };
                    *current = next;
                    Some(VmValue::int(value))
                } else {
                    None
                }
            }
            IteratorSourceState::List {
                source, position, ..
            } => {
                let value = match &self.heap.get(source)?.object {
                    HeapObject::List(values) => values.get(position).copied(),
                    HeapObject::Builder(_)
                    | HeapObject::String(_)
                    | HeapObject::Closure { .. }
                    | HeapObject::Vacant => {
                        return Err(ExecutionError::InvalidHeapObject {
                            call: RuntimeCall::IterNext,
                        });
                    }
                };
                if value.is_some() {
                    let next = position
                        .checked_add(1)
                        .ok_or(ExecutionError::MetricOverflow {
                            metric: "iterator position",
                        })?;
                    let state = self.iterator_mut(iterator)?;
                    let IteratorSourceState::List { position, .. } = &mut state.source else {
                        return Err(ExecutionError::UnsupportedPrimitive {
                            operation: "iterator source changed during list iteration",
                        });
                    };
                    *position = next;
                }
                value
            }
        };
        VmValue::iterator_step(value)
    }

    pub(super) fn iter_drop(&mut self, iterator: VmValue) -> Result<VmValue, ExecutionError> {
        self.drop_iterator(iterator)?;
        Ok(VmValue::UNIT)
    }

    pub(in crate::execute) fn iterator_mut(
        &mut self,
        value: VmValue,
    ) -> Result<&mut IteratorState, ExecutionError> {
        self.value_arena.iterator_mut(value)
    }
}

fn range_has_value(current: i64, end: i64, step: i64, inclusive: bool) -> bool {
    if step > 0 {
        current < end || (inclusive && current == end)
    } else {
        current > end || (inclusive && current == end)
    }
}
