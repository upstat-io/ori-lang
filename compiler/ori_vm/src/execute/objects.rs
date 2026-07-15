//! Aggregate, collection, and result-materialization operations.

use ori_arc::CtorKind;

use crate::{ExecutionError, IndexKind, ValueKind};

use super::arena::{Aggregate, AggregateKind};
use super::heap::HeapObject;
use super::operands::{FrameSlot, OperandAccess};
use super::value::{ExitValue, VmValue};
use super::{ArenaAllocationRoots, Interpreter};

impl Interpreter<'_> {
    pub(super) fn execute_moves(
        &mut self,
        frame_index: usize,
        moves_index: usize,
        operands: &mut impl OperandAccess,
    ) {
        let moves = &self.program.moves[moves_index];
        let frame = &mut self.frames[frame_index];
        let registers = &mut frame.registers;
        let move_scratch = &mut frame.move_scratch;
        move_scratch.clear();
        for &(_, source) in moves {
            move_scratch.push(operands.read(registers, source));
        }
        for (&(destination, _), &value) in moves.iter().zip(move_scratch.iter()) {
            registers[operands.write(destination).index()] = value;
        }
        move_scratch.clear();
    }

    pub(super) fn construct(
        &mut self,
        frame: usize,
        destination: FrameSlot,
        constructor: CtorKind,
        operand_list: usize,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let arguments = &self.program.operands[operand_list];
        match constructor {
            CtorKind::Tuple | CtorKind::Struct(_) | CtorKind::EnumVariant { .. } => {
                if arguments.len() > 4 {
                    return Err(ExecutionError::ResourceLimit {
                        resource: "aggregate fields",
                        limit: 4,
                    });
                }
                let mut aggregate = Aggregate {
                    length: u8::try_from(arguments.len()).map_err(|_| {
                        ExecutionError::ResourceLimit {
                            resource: "aggregate fields",
                            limit: 4,
                        }
                    })?,
                    variant: match constructor {
                        CtorKind::EnumVariant { variant, .. } => u64::from(variant),
                        _ => 0,
                    },
                    kind: if matches!(constructor, CtorKind::EnumVariant { .. }) {
                        AggregateKind::Variant
                    } else {
                        AggregateKind::Product
                    },
                    ..Aggregate::default()
                };
                for (slot, &argument) in aggregate.fields.iter_mut().zip(arguments) {
                    *slot = operand_cursor.read(&self.frames[frame].registers, argument);
                }
                self.prepare_value_arena_allocation(ArenaAllocationRoots::overwriting(
                    frame,
                    destination,
                    &aggregate.fields[..usize::from(aggregate.length)],
                ))?;
                self.value_arena
                    .allocate_aggregate(aggregate, self.config.max_value_arena_entries)
            }
            CtorKind::ListLiteral => {
                if arguments.len() > self.config.max_collection_elements {
                    return Err(ExecutionError::ResourceLimit {
                        resource: "collection elements",
                        limit: self.config.max_collection_elements,
                    });
                }
                let values = arguments
                    .iter()
                    .map(|&argument| operand_cursor.read(&self.frames[frame].registers, argument))
                    .collect::<Vec<_>>();
                self.heap
                    .allocate(HeapObject::List(values), self.config.max_heap_objects)
            }
            CtorKind::MapLiteral | CtorKind::SetLiteral | CtorKind::Closure { .. } => {
                Err(ExecutionError::UnsupportedConstructor {
                    constructor: constructor_name(constructor),
                })
            }
        }
    }

    pub(super) fn project(&self, value: VmValue, field: u32) -> Result<VmValue, ExecutionError> {
        if value.kind() == ValueKind::IteratorStep {
            let item = value.as_iterator_step()?;
            return match field {
                0 => Ok(VmValue::int(i64::from(item.is_some()))),
                1 => Ok(item.unwrap_or(VmValue::int(0))),
                _ => Err(ExecutionError::AggregateFieldOutOfBounds {
                    field: field as usize,
                    length: 2,
                }),
            };
        }
        if value.kind() == ValueKind::Aggregate {
            let aggregate = self.aggregate(value)?;
            let index = field as usize;
            return aggregate.projected_field(index)?.ok_or(
                ExecutionError::AggregateFieldOutOfBounds {
                    field: index,
                    length: aggregate.projected_field_count(),
                },
            );
        }
        if value.kind() == ValueKind::Heap && field == 0 {
            return match &self.heap.get(value)?.object {
                HeapObject::List(values) | HeapObject::Builder(values) => {
                    let length = i64::try_from(values.len()).map_err(|_| {
                        ExecutionError::IntegerOperation {
                            operation: "collection length conversion",
                        }
                    })?;
                    Ok(VmValue::int(length))
                }
                HeapObject::String(_) | HeapObject::Closure { .. } | HeapObject::Vacant => {
                    Err(ExecutionError::UnsupportedPrimitive {
                        operation: "heap field projection",
                    })
                }
            };
        }
        Err(ExecutionError::UnsupportedPrimitive {
            operation: "field projection",
        })
    }

    pub(super) fn set_field(
        &mut self,
        base: VmValue,
        field: u32,
        value: VmValue,
    ) -> Result<(), ExecutionError> {
        let aggregate = self.value_arena.aggregate_mut(base)?;
        let index = field as usize;
        if index >= usize::from(aggregate.length) {
            return Err(ExecutionError::AggregateFieldOutOfBounds {
                field: index,
                length: usize::from(aggregate.length),
            });
        }
        aggregate.fields[index] = value;
        Ok(())
    }

    pub(super) fn set_tag(&mut self, base: VmValue, tag: u64) -> Result<(), ExecutionError> {
        self.value_arena.aggregate_mut(base)?.variant = tag;
        Ok(())
    }

    pub(super) fn discriminant(&self, value: VmValue) -> Result<u64, ExecutionError> {
        match value.kind() {
            ValueKind::Int => Ok(value.as_int()?.cast_unsigned()),
            ValueKind::Bool => Ok(u64::from(value.as_bool()?)),
            ValueKind::Char => Ok(u64::from(value.as_char()? as u32)),
            ValueKind::Aggregate => Ok(self.aggregate(value)?.variant),
            ValueKind::IteratorStep => Ok(0),
            _ => Err(ExecutionError::UnsupportedPrimitive {
                operation: "switch discriminant",
            }),
        }
    }

    pub(super) fn materialize(&self, value: VmValue) -> Result<ExitValue, ExecutionError> {
        let mut remaining = self.config.max_collection_elements;
        self.materialize_with_budget(value, &mut remaining)
    }

    fn materialize_with_budget(
        &self,
        value: VmValue,
        remaining: &mut usize,
    ) -> Result<ExitValue, ExecutionError> {
        match value.kind() {
            ValueKind::Unit => Ok(ExitValue::Unit),
            ValueKind::Int => value.as_int().map(ExitValue::Int),
            ValueKind::Bool => value.as_bool().map(ExitValue::Bool),
            ValueKind::Float => value.as_float().map(ExitValue::Float),
            ValueKind::Char => value.as_char().map(ExitValue::Char),
            ValueKind::Null => Ok(ExitValue::Null),
            ValueKind::ConstantString => self
                .program
                .strings
                .get(value.string_index()?)
                .cloned()
                .map(ExitValue::String)
                .ok_or_else(|| {
                    super::invalid_verified_index(
                        IndexKind::String,
                        value.string_index().unwrap_or(usize::MAX),
                        self.program.strings.len(),
                    )
                }),
            ValueKind::Heap => match &self.heap.get(value)?.object {
                HeapObject::String(value) => Ok(ExitValue::String(value.clone())),
                HeapObject::List(values) | HeapObject::Builder(values) => {
                    self.consume_materialization_budget(values.len(), remaining)?;
                    let values = values
                        .iter()
                        .map(|&element| self.materialize_with_budget(element, remaining))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(ExitValue::List(values))
                }
                HeapObject::Closure { .. } => Err(ExecutionError::EscapingClosure),
                HeapObject::Vacant => Err(ExecutionError::ReleasedHeap {
                    index: value.heap_index()?,
                }),
            },
            ValueKind::Aggregate => {
                let aggregate = self.aggregate(value)?;
                let length = usize::from(aggregate.length);
                self.consume_materialization_budget(length, remaining)?;
                let fields = aggregate.fields[..length]
                    .iter()
                    .map(|&field| self.materialize_with_budget(field, remaining))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExitValue::Aggregate {
                    variant: aggregate.variant,
                    fields,
                })
            }
            ValueKind::IteratorStep => {
                let item = value.as_iterator_step()?;
                self.consume_materialization_budget(2, remaining)?;
                Ok(ExitValue::Aggregate {
                    variant: 0,
                    fields: vec![
                        ExitValue::Int(i64::from(item.is_some())),
                        self.materialize_with_budget(item.unwrap_or(VmValue::int(0)), remaining)?,
                    ],
                })
            }
            ValueKind::Iterator => Err(ExecutionError::EscapingIterator),
        }
    }

    fn consume_materialization_budget(
        &self,
        amount: usize,
        remaining: &mut usize,
    ) -> Result<(), ExecutionError> {
        *remaining = remaining
            .checked_sub(amount)
            .ok_or(ExecutionError::ResourceLimit {
                resource: "materialized values",
                limit: self.config.max_collection_elements,
            })?;
        Ok(())
    }

    pub(super) fn aggregate(&self, value: VmValue) -> Result<&Aggregate, ExecutionError> {
        self.value_arena.aggregate(value)
    }
}

const fn constructor_name(constructor: CtorKind) -> &'static str {
    match constructor {
        CtorKind::MapLiteral => "map literal",
        CtorKind::SetLiteral => "set literal",
        CtorKind::Closure { .. } => "closure",
        CtorKind::Tuple => "tuple",
        CtorKind::Struct(_) => "struct",
        CtorKind::EnumVariant { .. } => "enum variant",
        CtorKind::ListLiteral => "list literal",
    }
}
