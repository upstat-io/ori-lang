//! Aggregate, collection, and result-materialization operations.

use ori_arc::CtorKind;

use crate::bytecode::Register;
use crate::{ExecutionError, IndexKind, ValueKind};

use super::frame::Aggregate;
use super::heap::HeapObject;
use super::value::{ExitValue, VmValue};
use super::Interpreter;

impl Interpreter<'_> {
    pub(super) fn execute_moves(&mut self, frame_index: usize, moves_index: usize) {
        let moves = &self.program.moves[moves_index];
        let frame = &mut self.frames[frame_index];
        let registers = &mut frame.registers;
        let move_scratch = &mut frame.move_scratch;
        move_scratch.clear();
        for &(_, source) in moves {
            move_scratch.push(registers[source.index()]);
        }
        for (&(destination, _), &value) in moves.iter().zip(move_scratch.iter()) {
            registers[destination.index()] = value;
        }
    }

    pub(super) fn construct(
        &mut self,
        frame: usize,
        destination: Register,
        constructor: CtorKind,
        operands: usize,
    ) -> Result<VmValue, ExecutionError> {
        let arguments = self.operands(operands).to_vec();
        match constructor {
            CtorKind::Tuple | CtorKind::Struct(_) | CtorKind::EnumVariant { .. } => {
                let values = arguments
                    .iter()
                    .map(|&argument| self.register(frame, argument))
                    .collect::<Vec<_>>();
                let mut aggregate = Aggregate {
                    length: u8::try_from(values.len()).map_err(|_| {
                        ExecutionError::ResourceLimit {
                            resource: "aggregate fields",
                            limit: 4,
                        }
                    })?,
                    variant: match constructor {
                        CtorKind::EnumVariant { variant, .. } => u64::from(variant),
                        _ => 0,
                    },
                    ..Aggregate::default()
                };
                for (slot, value) in aggregate.fields.iter_mut().zip(values) {
                    *slot = value;
                }
                let slot = destination.index();
                let bound = self.frames[frame].aggregates.len();
                let target = self.frames[frame].aggregates.get_mut(slot).ok_or(
                    ExecutionError::LocalHandleOutOfBounds {
                        kind: ValueKind::Aggregate,
                        frame,
                        slot,
                    },
                )?;
                *target = aggregate;
                debug_assert!(slot < bound);
                VmValue::aggregate(frame, destination.raw())
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
                    .map(|&argument| self.register(frame, argument))
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
        if value.kind() == ValueKind::Aggregate {
            let aggregate = self.aggregate(value)?;
            let index = field as usize;
            return aggregate
                .fields
                .get(index)
                .copied()
                .filter(|_| index < usize::from(aggregate.length))
                .ok_or(ExecutionError::AggregateFieldOutOfBounds {
                    field: index,
                    length: usize::from(aggregate.length),
                });
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
                HeapObject::String(_) | HeapObject::Vacant => {
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
        let (frame, slot) = base.aggregate_parts()?;
        let aggregate = self.aggregate_mut(frame, slot)?;
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
        let (frame, slot) = base.aggregate_parts()?;
        self.aggregate_mut(frame, slot)?.variant = tag;
        Ok(())
    }

    pub(super) fn discriminant(&self, value: VmValue) -> Result<u64, ExecutionError> {
        match value.kind() {
            ValueKind::Int => Ok(value.as_int()?.cast_unsigned()),
            ValueKind::Bool => Ok(u64::from(value.as_bool()?)),
            ValueKind::Char => Ok(u64::from(value.as_char()? as u32)),
            ValueKind::Aggregate => Ok(self.aggregate(value)?.variant),
            _ => Err(ExecutionError::UnsupportedPrimitive {
                operation: "switch discriminant",
            }),
        }
    }

    pub(super) fn promote_escaping(
        &mut self,
        value: VmValue,
        source_frame: usize,
        target_frame: usize,
        destination: Option<Register>,
    ) -> Result<VmValue, ExecutionError> {
        match value.kind() {
            ValueKind::Aggregate => {
                self.promote_aggregate(value, source_frame, target_frame, destination)
            }
            ValueKind::Iterator => Err(ExecutionError::EscapingIterator),
            ValueKind::Heap => {
                self.promote_heap_values(value, source_frame, target_frame)?;
                Ok(value)
            }
            _ => Ok(value),
        }
    }

    fn promote_aggregate(
        &mut self,
        value: VmValue,
        source_frame: usize,
        target_frame: usize,
        destination: Option<Register>,
    ) -> Result<VmValue, ExecutionError> {
        let (owner, _) = value.aggregate_parts()?;
        if owner != source_frame {
            return Ok(value);
        }
        let mut aggregate = *self.aggregate(value)?;
        for index in 0..usize::from(aggregate.length) {
            aggregate.fields[index] =
                self.promote_escaping(aggregate.fields[index], source_frame, target_frame, None)?;
        }
        let target_slot = if let Some(destination) = destination {
            let slot = destination.index();
            let target = self.frames[target_frame].aggregates.get_mut(slot).ok_or(
                ExecutionError::LocalHandleOutOfBounds {
                    kind: ValueKind::Aggregate,
                    frame: target_frame,
                    slot,
                },
            )?;
            *target = aggregate;
            slot
        } else {
            if self.frames[target_frame].aggregates.len() >= self.config.max_frame_values {
                return Err(ExecutionError::ResourceLimit {
                    resource: "frame aggregate values",
                    limit: self.config.max_frame_values,
                });
            }
            let slot = self.frames[target_frame].aggregates.len();
            self.frames[target_frame].aggregates.push(aggregate);
            slot
        };
        let slot = u32::try_from(target_slot).map_err(|_| ExecutionError::ResourceLimit {
            resource: "frame aggregate values",
            limit: self.config.max_frame_values,
        })?;
        VmValue::aggregate(target_frame, slot)
    }

    fn promote_heap_values(
        &mut self,
        value: VmValue,
        source_frame: usize,
        target_frame: usize,
    ) -> Result<(), ExecutionError> {
        let values = match &self.heap.get(value)?.object {
            HeapObject::List(values) | HeapObject::Builder(values) => Some(values.clone()),
            HeapObject::String(_) | HeapObject::Vacant => None,
        };
        let Some(mut values) = values else {
            return Ok(());
        };
        for element in &mut values {
            *element = self.promote_escaping(*element, source_frame, target_frame, None)?;
        }
        match &mut self.heap.get_mut(value)?.object {
            HeapObject::List(target) | HeapObject::Builder(target) => *target = values,
            HeapObject::String(_) | HeapObject::Vacant => {}
        }
        Ok(())
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
        let (frame, slot) = value.aggregate_parts()?;
        self.frames
            .get(frame)
            .and_then(|frame| frame.aggregates.get(slot))
            .ok_or(ExecutionError::LocalHandleOutOfBounds {
                kind: ValueKind::Aggregate,
                frame,
                slot,
            })
    }

    fn aggregate_mut(
        &mut self,
        frame: usize,
        slot: usize,
    ) -> Result<&mut Aggregate, ExecutionError> {
        self.frames
            .get_mut(frame)
            .and_then(|frame| frame.aggregates.get_mut(slot))
            .ok_or(ExecutionError::LocalHandleOutOfBounds {
                kind: ValueKind::Aggregate,
                frame,
                slot,
            })
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
