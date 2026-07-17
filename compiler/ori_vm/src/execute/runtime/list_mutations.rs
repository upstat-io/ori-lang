//! Persistent-list mutations with transactional copy-on-write ownership.

use ori_arc::ArgOwnership;
use ori_repr::executable::RuntimeCall;

use crate::ExecutionError;

use super::super::heap::HeapObject;
use super::super::operands::OperandAccess;
use super::super::value::VmValue;
use super::super::Interpreter;
use super::nonnegative_usize;

#[derive(Clone, Copy)]
struct ListCapacity {
    length: usize,
    capacity: usize,
    limit: usize,
}

#[derive(Clone, Copy)]
pub(super) enum ListMutationCall {
    Push,
    Insert,
    Remove,
    Prepend,
    Set,
}

impl ListMutationCall {
    const fn runtime_call(self) -> RuntimeCall {
        match self {
            Self::Push => RuntimeCall::ListPush,
            Self::Insert => RuntimeCall::ListInsert,
            Self::Remove => RuntimeCall::ListRemove,
            Self::Prepend => RuntimeCall::ListPrepend,
            Self::Set => RuntimeCall::ListSet,
        }
    }
}

impl Interpreter<'_> {
    pub(super) fn execute_list_mutation(
        &mut self,
        frame: usize,
        mutation: ListMutationCall,
        operands: usize,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let call = mutation.runtime_call();
        match mutation {
            ListMutationCall::Push => {
                let [list, value] =
                    self.runtime_values::<2>(frame, operands, call, operand_cursor)?;
                let list_ownership = self.runtime_argument_ownership(operands, 0, call)?;
                let value_ownership = self.runtime_argument_ownership(operands, 1, call)?;
                self.list_push_value(list, value, list_ownership, value_ownership)
            }
            ListMutationCall::Insert => {
                let [list, index, value] =
                    self.runtime_values::<3>(frame, operands, call, operand_cursor)?;
                let list_ownership = self.runtime_argument_ownership(operands, 0, call)?;
                let value_ownership = self.runtime_argument_ownership(operands, 2, call)?;
                self.list_insert_value(list, index, value, list_ownership, value_ownership)
            }
            ListMutationCall::Remove => {
                let [list, index] =
                    self.runtime_values::<2>(frame, operands, call, operand_cursor)?;
                let list_ownership = self.runtime_argument_ownership(operands, 0, call)?;
                self.list_remove_value(list, index, list_ownership)
            }
            ListMutationCall::Prepend => {
                let [list, value] =
                    self.runtime_values::<2>(frame, operands, call, operand_cursor)?;
                let list_ownership = self.runtime_argument_ownership(operands, 0, call)?;
                let value_ownership = self.runtime_argument_ownership(operands, 1, call)?;
                self.list_prepend_value(list, value, list_ownership, value_ownership)
            }
            ListMutationCall::Set => {
                let [list, index, value] =
                    self.runtime_values::<3>(frame, operands, call, operand_cursor)?;
                let list_ownership = self.runtime_argument_ownership(operands, 0, call)?;
                let value_ownership = self.runtime_argument_ownership(operands, 2, call)?;
                self.replace_list_element_value(list, index, value, list_ownership, value_ownership)
            }
        }
    }

    pub(super) fn list_push_value(
        &mut self,
        list: VmValue,
        value: VmValue,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let length = self.list_length_for_mutation(list, RuntimeCall::ListPush)?;
        self.insert_list_element(
            list,
            length,
            value,
            RuntimeCall::ListPush,
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_insert_value(
        &mut self,
        list: VmValue,
        index: VmValue,
        value: VmValue,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let index = nonnegative_usize(index.as_int()?, "list index")?;
        self.insert_list_element(
            list,
            index,
            value,
            RuntimeCall::ListInsert,
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_prepend_value(
        &mut self,
        list: VmValue,
        value: VmValue,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.insert_list_element(
            list,
            0,
            value,
            RuntimeCall::ListPrepend,
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_remove_value(
        &mut self,
        list: VmValue,
        index: VmValue,
        list_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let index = nonnegative_usize(index.as_int()?, "list index")?;
        let references = self.heap.references(list)?;
        let length = self.list_length_for_mutation(list, RuntimeCall::ListRemove)?;
        if index >= length {
            return Err(ExecutionError::CollectionIndexOutOfBounds { index, length });
        }

        if list_ownership == ArgOwnership::Owned && references == 1 {
            return self.remove_unique_list_element(list, index);
        }

        let mut values = self.clone_list_for_mutation(list, RuntimeCall::ListRemove)?;
        values.remove(index);
        self.allocate_list_result(list, values, None, list_ownership == ArgOwnership::Owned)
    }

    pub(super) fn replace_list_element_value(
        &mut self,
        list: VmValue,
        index: VmValue,
        value: VmValue,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let call = RuntimeCall::ListSet;
        let index = nonnegative_usize(index.as_int()?, "list index")?;
        let references = self.heap.references(list)?;
        let length = self.list_length_for_mutation(list, call)?;
        if index >= length {
            return Err(ExecutionError::CollectionIndexOutOfBounds { index, length });
        }

        if list_ownership == ArgOwnership::Owned && references == 1 {
            return self.replace_unique_list_element(list, index, value, call, value_ownership);
        }

        let mut values = self.clone_list_for_mutation(list, call)?;
        values[index] = value;
        let owned_value_index = (value_ownership == ArgOwnership::Owned).then_some(index);
        self.allocate_list_result(
            list,
            values,
            owned_value_index,
            list_ownership == ArgOwnership::Owned,
        )
    }

    fn insert_list_element(
        &mut self,
        list: VmValue,
        index: usize,
        value: VmValue,
        call: RuntimeCall,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let references = self.heap.references(list)?;
        let (length, capacity) = match &self.heap.get(list)?.object {
            HeapObject::List(values) => (values.len(), values.capacity()),
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject { call });
            }
        };
        if index > length {
            return Err(ExecutionError::CollectionIndexOutOfBounds { index, length });
        }
        let limit = self.config.max_collection_elements;
        if length >= limit {
            return Err(ExecutionError::ResourceLimit {
                resource: "collection elements",
                limit,
            });
        }

        if list_ownership == ArgOwnership::Owned && references == 1 {
            return self.insert_unique_list_element(
                list,
                index,
                value,
                call,
                value_ownership,
                ListCapacity {
                    length,
                    capacity,
                    limit,
                },
            );
        }

        let mut values = self.clone_list_for_mutation(list, call)?;
        values.insert(index, value);
        let owned_value_index = (value_ownership == ArgOwnership::Owned).then_some(index);
        self.allocate_list_result(
            list,
            values,
            owned_value_index,
            list_ownership == ArgOwnership::Owned,
        )
    }

    fn insert_unique_list_element(
        &mut self,
        list: VmValue,
        index: usize,
        value: VmValue,
        call: RuntimeCall,
        value_ownership: ArgOwnership,
        shape: ListCapacity,
    ) -> Result<VmValue, ExecutionError> {
        let replacement = if shape.length < shape.capacity {
            None
        } else {
            let requested = shape
                .length
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "list capacity",
                })?;
            let capacity = next_vector_capacity(shape.capacity, requested, shape.limit);
            let mut values = Vec::with_capacity(capacity);
            let HeapObject::List(current) = &self.heap.get(list)?.object else {
                return Err(ExecutionError::InvalidHeapObject { call });
            };
            values.extend_from_slice(current);
            values.insert(index, value);
            Some(values)
        };
        self.prepare_borrowed_value(value, value_ownership)?;
        let mutation = if let Some(values) = replacement {
            self.heap.replace_object(list, HeapObject::List(values))
        } else {
            self.heap.get_mut(list).and_then(|slot| {
                let HeapObject::List(values) = &mut slot.object else {
                    return Err(ExecutionError::InvalidHeapObject { call });
                };
                values.insert(index, value);
                Ok(())
            })
        };
        if let Err(error) = mutation {
            self.discard_borrowed_value(value_ownership);
            return Err(error);
        }
        self.commit_borrowed_value(value_ownership);
        Ok(list)
    }

    fn remove_unique_list_element(
        &mut self,
        list: VmValue,
        index: usize,
    ) -> Result<VmValue, ExecutionError> {
        let removed = match &self.heap.get(list)?.object {
            HeapObject::List(values) => values[index],
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListRemove,
                });
            }
        };
        self.prepare_release_owned_value(removed)?;
        let mutation = self.heap.get_mut(list).and_then(|slot| {
            let HeapObject::List(values) = &mut slot.object else {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListRemove,
                });
            };
            values.remove(index);
            Ok(())
        });
        if let Err(error) = mutation {
            self.discard_prepared_release();
            return Err(error);
        }
        self.commit_prepared_release(removed)?;
        Ok(list)
    }

    fn replace_unique_list_element(
        &mut self,
        list: VmValue,
        index: usize,
        value: VmValue,
        call: RuntimeCall,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let replaced = match &self.heap.get(list)?.object {
            HeapObject::List(values) => values[index],
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject { call });
            }
        };
        self.prepare_release_owned_value(replaced)?;
        if let Err(error) = self.prepare_borrowed_value(value, value_ownership) {
            self.discard_prepared_release();
            return Err(error);
        }
        let mutation = self.heap.get_mut(list).and_then(|slot| {
            let HeapObject::List(values) = &mut slot.object else {
                return Err(ExecutionError::InvalidHeapObject { call });
            };
            values[index] = value;
            Ok(())
        });
        if let Err(error) = mutation {
            self.discard_borrowed_value(value_ownership);
            self.discard_prepared_release();
            return Err(error);
        }
        self.commit_borrowed_value(value_ownership);
        self.commit_prepared_release(replaced)?;
        Ok(list)
    }

    fn allocate_list_result(
        &mut self,
        list: VmValue,
        values: Vec<VmValue>,
        owned_value_index: Option<usize>,
        consume_list: bool,
    ) -> Result<VmValue, ExecutionError> {
        self.prepare_retain_owned_values(
            values
                .iter()
                .enumerate()
                .filter_map(|(index, &value)| (Some(index) != owned_value_index).then_some(value)),
            1,
        )?;
        let prepared = match self
            .heap
            .prepare_allocation(HeapObject::List(values), self.config.max_heap_objects)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.discard_prepared_retains();
                return Err(error);
            }
        };
        if consume_list {
            if let Err(error) = self.heap.validate_shared_release(list) {
                self.discard_prepared_retains();
                return Err(error);
            }
        }
        self.commit_prepared_retains(1);
        let result = self.heap.commit_allocation(prepared);
        if consume_list {
            self.heap.commit_shared_release(list);
        }
        Ok(result)
    }

    fn list_length_for_mutation(
        &self,
        list: VmValue,
        call: RuntimeCall,
    ) -> Result<usize, ExecutionError> {
        match &self.heap.get(list)?.object {
            HeapObject::List(values) => Ok(values.len()),
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => Err(ExecutionError::InvalidHeapObject { call }),
        }
    }

    fn clone_list_for_mutation(
        &self,
        list: VmValue,
        call: RuntimeCall,
    ) -> Result<Vec<VmValue>, ExecutionError> {
        match &self.heap.get(list)?.object {
            HeapObject::List(values) => Ok(values.clone()),
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => Err(ExecutionError::InvalidHeapObject { call }),
        }
    }

    fn prepare_borrowed_value(
        &mut self,
        value: VmValue,
        ownership: ArgOwnership,
    ) -> Result<(), ExecutionError> {
        if ownership == ArgOwnership::Borrowed {
            self.prepare_retain_owned_values([value], 1)?;
        }
        Ok(())
    }

    fn commit_borrowed_value(&mut self, ownership: ArgOwnership) {
        if ownership == ArgOwnership::Borrowed {
            self.commit_prepared_retains(1);
        }
    }

    fn discard_borrowed_value(&mut self, ownership: ArgOwnership) {
        if ownership == ArgOwnership::Borrowed {
            self.discard_prepared_retains();
        }
    }
}

fn next_vector_capacity(current: usize, required: usize, limit: usize) -> usize {
    current
        .checked_mul(2)
        .unwrap_or(limit)
        .max(required)
        .min(limit)
}
