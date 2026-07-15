//! List-builder, indexing, and length runtime operations.

use ori_registry::RuntimeOperator;
use ori_repr::executable::RuntimeCall;

use crate::ExecutionError;

use super::super::heap::HeapObject;
use super::super::value::VmValue;
use super::super::Interpreter;

impl Interpreter<'_> {
    pub(in crate::execute) fn runtime_binary(
        &mut self,
        operator: RuntimeOperator,
        left: VmValue,
        right: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        match operator {
            RuntimeOperator::ListConcat => self.list_concat_value(left, right),
            RuntimeOperator::StringConcat
            | RuntimeOperator::StringEqual
            | RuntimeOperator::StringNotEqual
            | RuntimeOperator::StringCompare => Err(ExecutionError::UnsupportedPrimitive {
                operation: "non-list runtime binary bytecode",
            }),
        }
    }

    /// Execute the dual-consume list-concat ownership contract.
    ///
    /// The prototype deliberately chooses the independent-allocation strategy
    /// row. It retains each result element, commits the result allocation, then
    /// releases both input ownership obligations. All fallible validation runs
    /// before the first mutation, including the aliased-input two-release case.
    pub(super) fn list_concat_value(
        &mut self,
        left: VmValue,
        right: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let left_values = self.list_values_for_concat(left)?;
        let right_values = self.list_values_for_concat(right)?;
        let length = left_values.len().checked_add(right_values.len()).ok_or(
            ExecutionError::MetricOverflow {
                metric: "list concat length",
            },
        )?;
        if length > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "collection elements",
                limit: self.config.max_collection_elements,
            });
        }

        let mut values = Vec::with_capacity(length);
        values.extend_from_slice(&left_values);
        values.extend_from_slice(&right_values);

        self.prepare_release_owned_values(&[left, right])?;
        if let Err(error) = self.prepare_retain_owned_values(values.iter().copied(), 1) {
            self.discard_prepared_release();
            return Err(error);
        }
        let prepared = match self
            .heap
            .prepare_allocation(HeapObject::List(values), self.config.max_heap_objects)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.discard_prepared_retains();
                self.discard_prepared_release();
                return Err(error);
            }
        };

        self.commit_prepared_retains(1);
        let result = self.heap.commit_allocation(prepared);
        self.commit_prepared_releases(&[left, right])?;
        Ok(result)
    }

    fn list_values_for_concat(&self, list: VmValue) -> Result<Vec<VmValue>, ExecutionError> {
        match &self.heap.get(list)?.object {
            HeapObject::List(values) => Ok(values.clone()),
            HeapObject::Builder(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => Err(ExecutionError::InvalidPrimitiveObject {
                operator: RuntimeOperator::ListConcat,
            }),
        }
    }

    pub(super) fn list_new(&mut self, capacity: VmValue) -> Result<VmValue, ExecutionError> {
        let capacity = nonnegative_usize(capacity.as_int()?, "list capacity")?;
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

    pub(super) fn list_builder_push(
        &mut self,
        builder: VmValue,
        value: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let limit = self.config.max_collection_elements;
        let (length, capacity) = match &self.heap.get(builder)?.object {
            HeapObject::Builder(values) if values.len() < limit => {
                (values.len(), values.capacity())
            }
            HeapObject::Builder(_) => {
                return Err(ExecutionError::ResourceLimit {
                    resource: "collection elements",
                    limit,
                });
            }
            HeapObject::List(_)
            | HeapObject::String(_)
            | HeapObject::Closure { .. }
            | HeapObject::Vacant => {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListBuilderPush,
                });
            }
        };
        if length < capacity {
            let HeapObject::Builder(values) = &mut self.heap.get_mut(builder)?.object else {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListBuilderPush,
                });
            };
            values.push(value);
        } else {
            let requested = length
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "builder capacity",
                })?;
            let capacity = next_vector_capacity(capacity, requested, limit);
            let mut values = Vec::with_capacity(capacity);
            let HeapObject::Builder(current) = &self.heap.get(builder)?.object else {
                return Err(ExecutionError::InvalidHeapObject {
                    call: RuntimeCall::ListBuilderPush,
                });
            };
            values.extend_from_slice(current);
            values.push(value);
            self.heap
                .replace_object(builder, HeapObject::Builder(values))?;
        }
        Ok(VmValue::UNIT)
    }

    pub(super) fn list_take(&mut self, builder: VmValue) -> Result<VmValue, ExecutionError> {
        if !matches!(self.heap.get(builder)?.object, HeapObject::Builder(_)) {
            return Err(ExecutionError::InvalidHeapObject {
                call: RuntimeCall::ListTake,
            });
        }
        let slot = self.heap.get_mut(builder)?;
        let HeapObject::Builder(values) = &mut slot.object else {
            return Err(ExecutionError::InvalidHeapObject {
                call: RuntimeCall::ListTake,
            });
        };
        let values = std::mem::take(values);
        slot.object = HeapObject::List(values);
        Ok(builder)
    }

    pub(super) fn index(
        &mut self,
        list: VmValue,
        index: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let index = nonnegative_usize(index.as_int()?, "list index")?;
        let value =
            match &self.heap.get(list)?.object {
                HeapObject::List(values) => values.get(index).copied().ok_or(
                    ExecutionError::CollectionIndexOutOfBounds {
                        index,
                        length: values.len(),
                    },
                )?,
                HeapObject::Builder(_)
                | HeapObject::String(_)
                | HeapObject::Closure { .. }
                | HeapObject::Vacant => {
                    return Err(ExecutionError::InvalidHeapObject {
                        call: RuntimeCall::Index,
                    });
                }
            };
        self.retain_owned_value(value, 1)?;
        Ok(value)
    }

    pub(super) fn length_value(&self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let length = match value.kind() {
            crate::ValueKind::ConstantString => {
                let index = value.string_index()?;
                self.program
                    .strings
                    .get(index)
                    .ok_or_else(|| {
                        super::super::invalid_verified_index(
                            crate::IndexKind::String,
                            index,
                            self.program.strings.len(),
                        )
                    })?
                    .len()
            }
            crate::ValueKind::Heap => match &self.heap.get(value)?.object {
                HeapObject::List(values) | HeapObject::Builder(values) => values.len(),
                HeapObject::String(value) => value.len(),
                HeapObject::Closure { .. } | HeapObject::Vacant => {
                    return Err(ExecutionError::InvalidHeapObject {
                        call: RuntimeCall::Length,
                    });
                }
            },
            found => {
                return Err(ExecutionError::TypeMismatch {
                    expected: crate::ValueKind::Heap,
                    found,
                });
            }
        };
        i64::try_from(length)
            .map(VmValue::int)
            .map_err(|_| ExecutionError::IntegerOperation {
                operation: "collection length conversion",
            })
    }
}

fn nonnegative_usize(value: i64, purpose: &'static str) -> Result<usize, ExecutionError> {
    usize::try_from(value).map_err(|_| ExecutionError::NegativeInteger { purpose, value })
}

fn next_vector_capacity(current: usize, required: usize, limit: usize) -> usize {
    current
        .checked_mul(2)
        .unwrap_or(limit)
        .max(required)
        .min(limit)
}
