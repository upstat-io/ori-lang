//! Backend-neutral runtime-call implementations for VM values.

mod strings;

use ori_repr::executable::RuntimeCall;

use crate::{ExecutionError, ValueKind};

use super::frame::{Aggregate, IteratorState};
use super::heap::HeapObject;
use super::value::VmValue;
use super::Interpreter;

impl Interpreter<'_> {
    pub(super) fn execute_runtime(
        &mut self,
        frame: usize,
        destination: crate::bytecode::Register,
        call: RuntimeCall,
        operands: usize,
    ) -> Result<VmValue, ExecutionError> {
        match call {
            RuntimeCall::Iter => {
                let [range] = self.runtime_arguments::<1>(operands, call)?;
                self.create_iterator(frame, destination, range)
            }
            RuntimeCall::ListNew => {
                let [capacity, _element_type] = self.runtime_arguments::<2>(operands, call)?;
                self.list_new(frame, capacity)
            }
            RuntimeCall::IterNext => {
                let [iterator, _item_type] = self.runtime_arguments::<2>(operands, call)?;
                self.iter_next(frame, destination, iterator)
            }
            RuntimeCall::ListBuilderPush => {
                let [builder, value, _element_type] =
                    self.runtime_arguments::<3>(operands, call)?;
                self.list_builder_push(frame, builder, value)
            }
            RuntimeCall::ListPush => {
                let [list, value] = self.runtime_arguments::<2>(operands, call)?;
                self.list_push(frame, list, value)
            }
            RuntimeCall::IterDrop => {
                let [iterator] = self.runtime_arguments::<1>(operands, call)?;
                self.iter_drop(frame, iterator)
            }
            RuntimeCall::ListTake => {
                let [builder] = self.runtime_arguments::<1>(operands, call)?;
                self.list_take(frame, builder)
            }
            RuntimeCall::Index => {
                let [collection, index] = self.runtime_arguments::<2>(operands, call)?;
                self.index(frame, collection, index)
            }
            RuntimeCall::Updated | RuntimeCall::ListSet => {
                let [list, index, value] = self.runtime_arguments::<3>(operands, call)?;
                self.updated(frame, list, index, value, call)
            }
            RuntimeCall::Length => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.length(frame, value)
            }
            RuntimeCall::ToString => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.convert_to_string(frame, value)
            }
            RuntimeCall::Concat => {
                let [left, right] = self.runtime_arguments::<2>(operands, call)?;
                self.concat(frame, left, right)
            }
            RuntimeCall::StringContains => {
                let [value, needle] = self.runtime_arguments::<2>(operands, call)?;
                self.string_contains(frame, value, needle)
            }
            RuntimeCall::StringStartsWith => {
                let [value, prefix] = self.runtime_arguments::<2>(operands, call)?;
                self.string_starts_with(frame, value, prefix)
            }
            RuntimeCall::StringEndsWith => {
                let [value, suffix] = self.runtime_arguments::<2>(operands, call)?;
                self.string_ends_with(frame, value, suffix)
            }
            RuntimeCall::StringIsEmpty => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.string_is_empty(frame, value)
            }
            RuntimeCall::StringTrim => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.string_trim(frame, value)
            }
            RuntimeCall::StringUppercase => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.string_uppercase(frame, value)
            }
            RuntimeCall::StringLowercase => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.string_lowercase(frame, value)
            }
            RuntimeCall::StringSplit => {
                let [value, separator] = self.runtime_arguments::<2>(operands, call)?;
                self.string_split(frame, value, separator)
            }
            RuntimeCall::Print => {
                let [value] = self.runtime_arguments::<1>(operands, call)?;
                self.print(frame, value)
            }
            RuntimeCall::Panic => {
                let [message] = self.runtime_arguments::<1>(operands, call)?;
                self.panic(frame, message)
            }
        }
    }

    fn runtime_arguments<const N: usize>(
        &self,
        operands: usize,
        call: RuntimeCall,
    ) -> Result<[crate::bytecode::Register; N], ExecutionError> {
        let arguments = self.operands(operands);
        let actual = arguments.len();
        let arguments: &[crate::bytecode::Register; N] =
            arguments
                .try_into()
                .map_err(|_| ExecutionError::RuntimeArity {
                    call,
                    expected: call.arity(),
                    actual,
                })?;
        Ok(*arguments)
    }

    fn create_iterator(
        &mut self,
        frame: usize,
        destination: crate::bytecode::Register,
        range_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let range = self.register(frame, range_register);
        let aggregate = *self.aggregate(range)?;
        if aggregate.length != 4 {
            return Err(ExecutionError::AggregateFieldOutOfBounds {
                field: 3,
                length: usize::from(aggregate.length),
            });
        }
        let state = IteratorState {
            current: aggregate.fields[0].as_int()?,
            end: aggregate.fields[1].as_int()?,
            step: aggregate.fields[2].as_int()?,
            inclusive: aggregate.fields[3].as_int()? != 0,
            live: true,
        };
        if state.step == 0 {
            return Err(ExecutionError::ZeroRangeStep);
        }
        let slot = destination.index();
        let iterator = self.frames[frame].iterators.get_mut(slot).ok_or(
            ExecutionError::LocalHandleOutOfBounds {
                kind: ValueKind::Iterator,
                frame,
                slot,
            },
        )?;
        *iterator = state;
        VmValue::iterator(frame, destination.raw())
    }

    fn list_new(
        &mut self,
        frame: usize,
        capacity_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let capacity = nonnegative_usize(
            self.register(frame, capacity_register).as_int()?,
            "list capacity",
        )?;
        if capacity > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "collection elements",
                limit: self.config.max_collection_elements,
            });
        }
        self.heap.allocate(
            HeapObject::Builder(Vec::with_capacity(capacity)),
            self.config.max_heap_objects,
        )
    }

    fn iter_next(
        &mut self,
        frame: usize,
        destination: crate::bytecode::Register,
        iterator_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let iterator = self.register(frame, iterator_register);
        let (owner, slot) = iterator.iterator_parts()?;
        let state = self.iterator_mut(owner, slot)?;
        if !state.live {
            return Err(ExecutionError::EscapingIterator);
        }
        let has_value = iterator_has_value(*state);
        let value = state.current;
        if has_value {
            state.current =
                state
                    .current
                    .checked_add(state.step)
                    .ok_or(ExecutionError::IntegerOperation {
                        operation: "range iteration",
                    })?;
        }
        let aggregate = Aggregate {
            fields: [
                VmValue::int(i64::from(has_value)),
                VmValue::int(value),
                VmValue::UNIT,
                VmValue::UNIT,
            ],
            length: 2,
            variant: 0,
        };
        let target_slot = destination.index();
        let target = self.frames[frame].aggregates.get_mut(target_slot).ok_or(
            ExecutionError::LocalHandleOutOfBounds {
                kind: ValueKind::Aggregate,
                frame,
                slot: target_slot,
            },
        )?;
        *target = aggregate;
        VmValue::aggregate(frame, destination.raw())
    }

    fn list_builder_push(
        &mut self,
        frame: usize,
        builder_register: crate::bytecode::Register,
        value_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let builder = self.register(frame, builder_register);
        let value = self.register(frame, value_register);
        let limit = self.config.max_collection_elements;
        match &mut self.heap.get_mut(builder)?.object {
            HeapObject::Builder(values) if values.len() < limit => values.push(value),
            HeapObject::Builder(_) => {
                return Err(ExecutionError::ResourceLimit {
                    resource: "collection elements",
                    limit,
                });
            }
            HeapObject::List(_) | HeapObject::String(_) | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListBuilderPush,
                });
            }
        }
        Ok(VmValue::UNIT)
    }

    fn list_push(
        &mut self,
        frame: usize,
        list_register: crate::bytecode::Register,
        value_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let list = self.register(frame, list_register);
        let value = self.register(frame, value_register);
        let heap_index = list.heap_index()?;
        let bound = self.heap.slots.len();
        let slot = self
            .heap
            .slots
            .get(heap_index)
            .ok_or(ExecutionError::HeapOutOfBounds {
                index: heap_index,
                bound,
            })?;
        let HeapObject::List(current) = &slot.object else {
            return Err(ExecutionError::InvalidHeapObject {
                call: RuntimeCall::ListPush,
            });
        };
        if current.len() >= self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "collection elements",
                limit: self.config.max_collection_elements,
            });
        }
        if slot.references == 1 {
            let values = match &mut self.heap.slots[heap_index].object {
                HeapObject::List(values) => values,
                HeapObject::Builder(_) | HeapObject::String(_) | HeapObject::Vacant => {
                    return Err(ExecutionError::InvalidHeapObject {
                        call: RuntimeCall::ListPush,
                    });
                }
            };
            values.push(value);
            return Ok(list);
        }
        let mut values = current.clone();
        values.push(value);
        self.heap.slots[heap_index].references = self.heap.slots[heap_index]
            .references
            .checked_sub(1)
            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        self.heap
            .allocate(HeapObject::List(values), self.config.max_heap_objects)
    }

    fn iter_drop(
        &mut self,
        frame: usize,
        iterator_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let iterator = self.register(frame, iterator_register);
        let (owner, slot) = iterator.iterator_parts()?;
        self.iterator_mut(owner, slot)?.live = false;
        Ok(VmValue::UNIT)
    }

    fn list_take(
        &mut self,
        frame: usize,
        builder_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let builder = self.register(frame, builder_register);
        let slot = self.heap.get_mut(builder)?;
        let object = std::mem::replace(&mut slot.object, HeapObject::Vacant);
        match object {
            HeapObject::Builder(values) => {
                slot.object = HeapObject::List(values);
                Ok(builder)
            }
            other => {
                slot.object = other;
                Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListTake,
                })
            }
        }
    }

    fn index(
        &mut self,
        frame: usize,
        list_register: crate::bytecode::Register,
        index_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let list = self.register(frame, list_register);
        let index =
            nonnegative_usize(self.register(frame, index_register).as_int()?, "list index")?;
        let value =
            match &self.heap.get(list)?.object {
                HeapObject::List(values) => values.get(index).copied().ok_or(
                    ExecutionError::CollectionIndexOutOfBounds {
                        index,
                        length: values.len(),
                    },
                )?,
                HeapObject::Builder(_) | HeapObject::String(_) | HeapObject::Vacant => {
                    return Err(ExecutionError::InvalidHeapObject {
                        call: RuntimeCall::Index,
                    });
                }
            };
        self.heap.increment(value, 1)?;
        Ok(value)
    }

    fn updated(
        &mut self,
        frame: usize,
        list_register: crate::bytecode::Register,
        index_register: crate::bytecode::Register,
        value_register: crate::bytecode::Register,
        call: RuntimeCall,
    ) -> Result<VmValue, ExecutionError> {
        let list = self.register(frame, list_register);
        let index =
            nonnegative_usize(self.register(frame, index_register).as_int()?, "list index")?;
        let value = self.register(frame, value_register);
        let heap_index = list.heap_index()?;
        let bound = self.heap.slots.len();
        let slot = self
            .heap
            .slots
            .get(heap_index)
            .ok_or(ExecutionError::HeapOutOfBounds {
                index: heap_index,
                bound,
            })?;
        if slot.references == 1 {
            let slot = &mut self.heap.slots[heap_index];
            let HeapObject::List(values) = &mut slot.object else {
                return Err(ExecutionError::InvalidHeapObject { call });
            };
            let length = values.len();
            let element = values
                .get_mut(index)
                .ok_or(ExecutionError::CollectionIndexOutOfBounds { index, length })?;
            *element = value;
            return Ok(list);
        }
        let mut values = match &slot.object {
            HeapObject::List(values) => values.clone(),
            HeapObject::Builder(_) | HeapObject::String(_) | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject { call });
            }
        };
        let length = values.len();
        let element = values
            .get_mut(index)
            .ok_or(ExecutionError::CollectionIndexOutOfBounds { index, length })?;
        *element = value;
        self.heap.slots[heap_index].references = self.heap.slots[heap_index]
            .references
            .checked_sub(1)
            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
        self.heap
            .allocate(HeapObject::List(values), self.config.max_heap_objects)
    }

    fn length(
        &self,
        frame: usize,
        value_register: crate::bytecode::Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.register(frame, value_register);
        let length = match &self.heap.get(value)?.object {
            HeapObject::List(values) | HeapObject::Builder(values) => values.len(),
            HeapObject::String(value) => value.chars().count(),
            HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::Length,
                });
            }
        };
        i64::try_from(length)
            .map(VmValue::int)
            .map_err(|_| ExecutionError::IntegerOperation {
                operation: "collection length conversion",
            })
    }

    fn iterator_mut(
        &mut self,
        frame: usize,
        slot: usize,
    ) -> Result<&mut IteratorState, ExecutionError> {
        self.frames
            .get_mut(frame)
            .and_then(|frame| frame.iterators.get_mut(slot))
            .ok_or(ExecutionError::LocalHandleOutOfBounds {
                kind: ValueKind::Iterator,
                frame,
                slot,
            })
    }
}

fn nonnegative_usize(value: i64, purpose: &'static str) -> Result<usize, ExecutionError> {
    usize::try_from(value).map_err(|_| ExecutionError::NegativeInteger { purpose, value })
}

fn iterator_has_value(state: IteratorState) -> bool {
    if state.step > 0 {
        state.current < state.end || (state.inclusive && state.current == state.end)
    } else {
        state.current > state.end || (state.inclusive && state.current == state.end)
    }
}
