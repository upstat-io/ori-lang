//! String operations and string-backed runtime services.

use ori_repr::executable::RuntimeCall;

use crate::bytecode::StringBinaryOp;
use crate::{ExecutionError, IndexKind, ValueKind};

use super::super::heap::HeapObject;
use super::super::value::VmValue;
use super::super::{primitives, Interpreter};

impl Interpreter<'_> {
    pub(in crate::execute) fn execute_binary(
        &mut self,
        operation: ori_ir::BinaryOp,
        left: VmValue,
        right: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let left_is_string = self.value_is_string(left)?;
        let right_is_string = self.value_is_string(right)?;
        if left_is_string || right_is_string {
            let operation = StringBinaryOp::from_binary(operation).ok_or(
                ExecutionError::UnsupportedPrimitive {
                    operation: "string binary operation",
                },
            )?;
            self.string_binary(operation, left, right)
        } else {
            primitives::binary(operation, left, right)
        }
    }

    pub(super) fn convert_to_string(&mut self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let string = Self::primitive_string(value, "to_str")?;
        self.allocate_string(string)
    }

    fn primitive_string(value: VmValue, operation: &'static str) -> Result<String, ExecutionError> {
        Ok(match value.kind() {
            ValueKind::Int => value.as_int()?.to_string(),
            ValueKind::Bool => value.as_bool()?.to_string(),
            ValueKind::Float => value.as_float()?.to_string(),
            ValueKind::Char => value.as_char()?.to_string(),
            _ => {
                return Err(ExecutionError::UnsupportedPrimitive { operation });
            }
        })
    }

    pub(super) fn concat(
        &mut self,
        left: VmValue,
        right: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        self.string_binary(StringBinaryOp::Concat, left, right)
    }

    pub(in crate::execute) fn string_binary(
        &mut self,
        operation: StringBinaryOp,
        left: VmValue,
        right: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        if !matches!(operation, StringBinaryOp::Concat) {
            let left = self.string_ref(left, RuntimeCall::ToString)?;
            let right = self.string_ref(right, RuntimeCall::ToString)?;
            let result = match operation {
                StringBinaryOp::Eq => left == right,
                StringBinaryOp::NotEq => left != right,
                StringBinaryOp::Lt => left < right,
                StringBinaryOp::LtEq => left <= right,
                StringBinaryOp::Gt => left > right,
                StringBinaryOp::GtEq => left >= right,
                StringBinaryOp::Concat => {
                    return Err(ExecutionError::UnsupportedPrimitive {
                        operation: "string comparison",
                    });
                }
            };
            return Ok(VmValue::bool(result));
        }
        let left = self.string_owned(left)?;
        let right = self.string_owned(right)?;
        let length = left
            .len()
            .checked_add(right.len())
            .ok_or(ExecutionError::ResourceLimit {
                resource: "string bytes",
                limit: self.config.max_collection_elements,
            })?;
        if length > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "string bytes",
                limit: self.config.max_collection_elements,
            });
        }
        let mut result = String::with_capacity(length);
        result.push_str(&left);
        result.push_str(&right);
        self.allocate_string(result)
    }

    pub(super) fn string_contains(
        &self,
        value: VmValue,
        needle: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(value, RuntimeCall::StringContains)?;
        let needle = self.string_ref(needle, RuntimeCall::StringContains)?;
        Ok(VmValue::bool(value.contains(needle)))
    }

    pub(super) fn string_starts_with(
        &self,
        value: VmValue,
        prefix: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(value, RuntimeCall::StringStartsWith)?;
        let prefix = self.string_ref(prefix, RuntimeCall::StringStartsWith)?;
        Ok(VmValue::bool(value.starts_with(prefix)))
    }

    pub(super) fn string_ends_with(
        &self,
        value: VmValue,
        suffix: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(value, RuntimeCall::StringEndsWith)?;
        let suffix = self.string_ref(suffix, RuntimeCall::StringEndsWith)?;
        Ok(VmValue::bool(value.ends_with(suffix)))
    }

    pub(super) fn string_is_empty(&self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(value, RuntimeCall::StringIsEmpty)?;
        Ok(VmValue::bool(value.is_empty()))
    }

    pub(super) fn string_trim(&mut self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(value)?;
        self.allocate_string(value.trim().to_owned())
    }

    pub(super) fn string_uppercase(&mut self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(value)?;
        self.allocate_string(value.to_uppercase())
    }

    pub(super) fn string_lowercase(&mut self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(value)?;
        self.allocate_string(value.to_lowercase())
    }

    pub(super) fn string_split_value(
        &mut self,
        value: VmValue,
        separator: VmValue,
    ) -> Result<VmValue, ExecutionError> {
        let piece_count = {
            let value = self.string_ref(value, RuntimeCall::StringSplit)?;
            let separator = self.string_ref(separator, RuntimeCall::StringSplit)?;
            validate_split(value, separator, self.config.max_collection_elements)?
        };
        let allocation_count =
            piece_count
                .checked_add(1)
                .ok_or(ExecutionError::MetricOverflow {
                    metric: "live heap objects",
                })?;
        self.heap
            .validate_allocation_count(allocation_count, self.config.max_heap_objects)?;
        let mut objects = {
            let value = self.string_ref(value, RuntimeCall::StringSplit)?;
            let separator = self.string_ref(separator, RuntimeCall::StringSplit)?;
            collect_split_objects(value, separator, allocation_count)
        };
        objects.push(HeapObject::List(vec![VmValue::UNIT; piece_count]));
        let prepared = self
            .heap
            .prepare_batch(&objects, self.config.max_heap_objects)?;
        let result = prepared.value_at(piece_count);
        let Some(HeapObject::List(values)) = objects.get_mut(piece_count) else {
            return Err(ExecutionError::InvalidHeapObject {
                call: RuntimeCall::StringSplit,
            });
        };
        for (index, value) in values.iter_mut().enumerate() {
            *value = prepared.value_at(index);
        }
        self.heap.commit_batch(&prepared, objects);
        Ok(result)
    }

    pub(super) fn print(&mut self, value: VmValue) -> Result<VmValue, ExecutionError> {
        let string = match value.kind() {
            ValueKind::ConstantString | ValueKind::Heap => self
                .string_ref(value, RuntimeCall::Print)
                .map(str::to_owned)?,
            _ => Self::primitive_string(value, "print")?,
        };
        let requested = self
            .output
            .len()
            .checked_add(string.len())
            .and_then(|length| length.checked_add(1))
            .ok_or(ExecutionError::ResourceLimit {
                resource: "output bytes",
                limit: self.config.max_output_bytes,
            })?;
        if requested > self.config.max_output_bytes {
            return Err(ExecutionError::ResourceLimit {
                resource: "output bytes",
                limit: self.config.max_output_bytes,
            });
        }
        self.output.extend_from_slice(string.as_bytes());
        self.output.push(b'\n');
        Ok(VmValue::UNIT)
    }

    pub(super) fn panic(&self, message: VmValue) -> Result<VmValue, ExecutionError> {
        Err(ExecutionError::Panic {
            message: self.string_owned(message)?,
        })
    }

    pub(super) fn catch_recover(&mut self) -> Result<VmValue, ExecutionError> {
        match self
            .pending_panic
            .take()
            .ok_or(ExecutionError::CatchRecoverWithoutPanic)?
        {
            ExecutionError::Panic { message } => self.allocate_string(message),
            error => Err(error),
        }
    }

    fn allocate_string(&mut self, value: String) -> Result<VmValue, ExecutionError> {
        if value.len() > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "string bytes",
                limit: self.config.max_collection_elements,
            });
        }
        self.heap
            .allocate(HeapObject::String(value), self.config.max_heap_objects)
    }

    fn string_owned(&self, value: VmValue) -> Result<String, ExecutionError> {
        self.string_ref(value, RuntimeCall::ToString)
            .map(str::to_owned)
    }

    fn string_ref(&self, value: VmValue, call: RuntimeCall) -> Result<&str, ExecutionError> {
        match value.kind() {
            ValueKind::ConstantString => {
                let index = value.string_index()?;
                self.program
                    .strings
                    .get(index)
                    .map(String::as_str)
                    .ok_or_else(|| {
                        super::super::invalid_verified_index(
                            IndexKind::String,
                            index,
                            self.program.strings.len(),
                        )
                    })
            }
            ValueKind::Heap => match &self.heap.get(value)?.object {
                HeapObject::String(value) => Ok(value),
                HeapObject::List(_)
                | HeapObject::Builder(_)
                | HeapObject::Closure { .. }
                | HeapObject::Vacant => Err(ExecutionError::InvalidHeapObject { call }),
            },
            _ => Err(ExecutionError::TypeMismatch {
                expected: ValueKind::ConstantString,
                found: value.kind(),
            }),
        }
    }

    fn value_is_string(&self, value: VmValue) -> Result<bool, ExecutionError> {
        match value.kind() {
            ValueKind::ConstantString => Ok(true),
            ValueKind::Heap => Ok(matches!(
                self.heap.get(value)?.object,
                HeapObject::String(_)
            )),
            _ => Ok(false),
        }
    }
}

fn validate_split(value: &str, separator: &str, limit: usize) -> Result<usize, ExecutionError> {
    let mut piece_count = 0_usize;
    if separator.is_empty() {
        if value.is_empty() {
            validate_split_piece("", &mut piece_count, limit)?;
        } else {
            for character in value.chars() {
                piece_count = piece_count
                    .checked_add(1)
                    .ok_or(ExecutionError::MetricOverflow {
                        metric: "string split pieces",
                    })?;
                if piece_count > limit {
                    return Err(ExecutionError::ResourceLimit {
                        resource: "collection elements",
                        limit,
                    });
                }
                if character.len_utf8() > limit {
                    return Err(ExecutionError::ResourceLimit {
                        resource: "string bytes",
                        limit,
                    });
                }
            }
        }
    } else {
        for piece in value.split(separator) {
            validate_split_piece(piece, &mut piece_count, limit)?;
        }
    }
    Ok(piece_count)
}

fn validate_split_piece(
    piece: &str,
    piece_count: &mut usize,
    limit: usize,
) -> Result<(), ExecutionError> {
    *piece_count = piece_count
        .checked_add(1)
        .ok_or(ExecutionError::MetricOverflow {
            metric: "string split pieces",
        })?;
    if *piece_count > limit {
        return Err(ExecutionError::ResourceLimit {
            resource: "collection elements",
            limit,
        });
    }
    if piece.len() > limit {
        return Err(ExecutionError::ResourceLimit {
            resource: "string bytes",
            limit,
        });
    }
    Ok(())
}

fn collect_split_objects(value: &str, separator: &str, capacity: usize) -> Vec<HeapObject> {
    let mut objects = Vec::with_capacity(capacity);
    if separator.is_empty() {
        if value.is_empty() {
            objects.push(HeapObject::String(String::new()));
        } else {
            objects.extend(
                value
                    .chars()
                    .map(|character| HeapObject::String(character.to_string())),
            );
        }
    } else {
        objects.extend(
            value
                .split(separator)
                .map(|piece| HeapObject::String(piece.to_owned())),
        );
    }
    objects
}
