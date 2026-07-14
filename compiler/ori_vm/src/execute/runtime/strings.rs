//! String operations and string-backed runtime services.

use ori_repr::executable::RuntimeCall;

use crate::bytecode::{Register, StringBinaryOp};
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

    pub(in crate::execute) fn string_binary_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        operation: StringBinaryOp,
        left: Register,
        right: Register,
    ) -> Result<(), ExecutionError> {
        let value = self.string_binary(
            operation,
            self.register(frame, left),
            self.register(frame, right),
        )?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn convert_to_string(
        &mut self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.register(frame, value_register);
        let string = match value.kind() {
            ValueKind::Int => value.as_int()?.to_string(),
            ValueKind::Bool => value.as_bool()?.to_string(),
            ValueKind::Float => value.as_float()?.to_string(),
            ValueKind::Char => value.as_char()?.to_string(),
            _ => {
                return Err(ExecutionError::UnsupportedPrimitive {
                    operation: "to_str",
                });
            }
        };
        self.allocate_string(string)
    }

    pub(super) fn concat(
        &mut self,
        frame: usize,
        left_register: Register,
        right_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        self.string_binary(
            StringBinaryOp::Concat,
            self.register(frame, left_register),
            self.register(frame, right_register),
        )
    }

    fn string_binary(
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
        frame: usize,
        value_register: Register,
        needle_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(
            self.register(frame, value_register),
            RuntimeCall::StringContains,
        )?;
        let needle = self.string_ref(
            self.register(frame, needle_register),
            RuntimeCall::StringContains,
        )?;
        Ok(VmValue::bool(value.contains(needle)))
    }

    pub(super) fn string_starts_with(
        &self,
        frame: usize,
        value_register: Register,
        prefix_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(
            self.register(frame, value_register),
            RuntimeCall::StringStartsWith,
        )?;
        let prefix = self.string_ref(
            self.register(frame, prefix_register),
            RuntimeCall::StringStartsWith,
        )?;
        Ok(VmValue::bool(value.starts_with(prefix)))
    }

    pub(super) fn string_ends_with(
        &self,
        frame: usize,
        value_register: Register,
        suffix_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(
            self.register(frame, value_register),
            RuntimeCall::StringEndsWith,
        )?;
        let suffix = self.string_ref(
            self.register(frame, suffix_register),
            RuntimeCall::StringEndsWith,
        )?;
        Ok(VmValue::bool(value.ends_with(suffix)))
    }

    pub(super) fn string_is_empty(
        &self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_ref(
            self.register(frame, value_register),
            RuntimeCall::StringIsEmpty,
        )?;
        Ok(VmValue::bool(value.is_empty()))
    }

    pub(super) fn string_trim(
        &mut self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(self.register(frame, value_register))?;
        self.allocate_string(value.trim().to_owned())
    }

    pub(super) fn string_uppercase(
        &mut self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(self.register(frame, value_register))?;
        self.allocate_string(value.to_uppercase())
    }

    pub(super) fn string_lowercase(
        &mut self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(self.register(frame, value_register))?;
        self.allocate_string(value.to_lowercase())
    }

    pub(super) fn string_split(
        &mut self,
        frame: usize,
        value_register: Register,
        separator_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.string_owned(self.register(frame, value_register))?;
        let separator = self.string_owned(self.register(frame, separator_register))?;
        let pieces: Vec<String> = if separator.is_empty() {
            if value.is_empty() {
                vec![String::new()]
            } else {
                value
                    .chars()
                    .map(|character| character.to_string())
                    .collect()
            }
        } else {
            value.split(&separator).map(str::to_owned).collect()
        };
        if pieces.len() > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "collection elements",
                limit: self.config.max_collection_elements,
            });
        }
        let mut values = Vec::with_capacity(pieces.len());
        for piece in pieces {
            values.push(self.allocate_string(piece)?);
        }
        self.heap
            .allocate(HeapObject::List(values), self.config.max_heap_objects)
    }

    pub(super) fn print(
        &mut self,
        frame: usize,
        value_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        let string = self.string_owned(self.register(frame, value_register))?;
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

    pub(super) fn panic(
        &self,
        frame: usize,
        message_register: Register,
    ) -> Result<VmValue, ExecutionError> {
        Err(ExecutionError::Panic {
            message: self.string_owned(self.register(frame, message_register))?,
        })
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
                HeapObject::List(_) | HeapObject::Builder(_) | HeapObject::Vacant => {
                    Err(ExecutionError::InvalidHeapObject { call })
                }
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
