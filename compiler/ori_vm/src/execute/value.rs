//! Compact interpreter values and observable result snapshots.

use crate::{ExecutionError, ValueKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum ValueTag {
    Unit,
    Int,
    Bool,
    Float,
    Char,
    Null,
    ConstantString,
    Heap,
    Aggregate,
    Iterator,
    IteratorStepDone,
    IteratorStepUnit,
    IteratorStepInt,
    IteratorStepBool,
    IteratorStepFloat,
    IteratorStepChar,
    IteratorStepNull,
    IteratorStepConstantString,
    IteratorStepHeap,
    IteratorStepAggregate,
    IteratorStepIterator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct VmValue {
    tag: ValueTag,
    payload: u64,
}

const _: () = assert!(core::mem::size_of::<ValueTag>() == 1);
const _: () = assert!(core::mem::size_of::<VmValue>() == 16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ArenaHandle {
    index: u32,
    generation: u32,
}

impl ArenaHandle {
    pub(super) fn new(index: usize, generation: u32) -> Result<Self, ExecutionError> {
        let index = u32::try_from(index)
            .map_err(|_| ExecutionError::ValueArenaHandleOverflow { length: index })?;
        Ok(Self { index, generation })
    }

    pub(super) const fn index(self) -> usize {
        self.index as usize
    }

    pub(super) const fn generation(self) -> u32 {
        self.generation
    }

    const fn encode(self) -> u64 {
        (self.generation as u64) << 32 | self.index as u64
    }

    const fn decode(payload: u64) -> Self {
        let bytes = payload.to_le_bytes();
        Self {
            index: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            generation: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        }
    }
}

impl Default for VmValue {
    fn default() -> Self {
        Self::UNIT
    }
}

impl VmValue {
    pub(super) const UNIT: Self = Self {
        tag: ValueTag::Unit,
        payload: 0,
    };

    pub(super) const fn int(value: i64) -> Self {
        Self {
            tag: ValueTag::Int,
            payload: value.cast_unsigned(),
        }
    }

    pub(super) const fn bool(value: bool) -> Self {
        Self {
            tag: ValueTag::Bool,
            payload: value as u64,
        }
    }

    pub(super) const fn float(bits: u64) -> Self {
        Self {
            tag: ValueTag::Float,
            payload: bits,
        }
    }

    pub(super) const fn char(value: char) -> Self {
        Self {
            tag: ValueTag::Char,
            payload: value as u64,
        }
    }

    pub(super) const fn null() -> Self {
        Self {
            tag: ValueTag::Null,
            payload: 0,
        }
    }

    pub(super) fn constant_string(index: usize) -> Result<Self, ExecutionError> {
        let index = u32::try_from(index).map_err(|_| ExecutionError::InvalidVerifiedIndex {
            kind: crate::IndexKind::String,
            index,
            bound: u32::MAX as usize,
        })?;
        Ok(Self {
            tag: ValueTag::ConstantString,
            payload: u64::from(index),
        })
    }

    pub(super) const fn heap(index: u32) -> Self {
        Self {
            tag: ValueTag::Heap,
            payload: index as u64,
        }
    }

    pub(super) const fn aggregate(handle: ArenaHandle) -> Self {
        Self {
            tag: ValueTag::Aggregate,
            payload: handle.encode(),
        }
    }

    pub(super) const fn iterator(handle: ArenaHandle) -> Self {
        Self {
            tag: ValueTag::Iterator,
            payload: handle.encode(),
        }
    }

    pub(super) fn iterator_step(value: Option<Self>) -> Result<Self, ExecutionError> {
        let Some(value) = value else {
            return Ok(Self {
                tag: ValueTag::IteratorStepDone,
                payload: 0,
            });
        };
        let tag = match value.tag {
            ValueTag::Unit => ValueTag::IteratorStepUnit,
            ValueTag::Int => ValueTag::IteratorStepInt,
            ValueTag::Bool => ValueTag::IteratorStepBool,
            ValueTag::Float => ValueTag::IteratorStepFloat,
            ValueTag::Char => ValueTag::IteratorStepChar,
            ValueTag::Null => ValueTag::IteratorStepNull,
            ValueTag::ConstantString => ValueTag::IteratorStepConstantString,
            ValueTag::Heap => ValueTag::IteratorStepHeap,
            ValueTag::Aggregate => ValueTag::IteratorStepAggregate,
            ValueTag::Iterator => ValueTag::IteratorStepIterator,
            ValueTag::IteratorStepDone
            | ValueTag::IteratorStepUnit
            | ValueTag::IteratorStepInt
            | ValueTag::IteratorStepBool
            | ValueTag::IteratorStepFloat
            | ValueTag::IteratorStepChar
            | ValueTag::IteratorStepNull
            | ValueTag::IteratorStepConstantString
            | ValueTag::IteratorStepHeap
            | ValueTag::IteratorStepAggregate
            | ValueTag::IteratorStepIterator => {
                return Err(ExecutionError::UnsupportedPrimitive {
                    operation: "nested iterator-step construction",
                });
            }
        };
        Ok(Self {
            tag,
            payload: value.payload,
        })
    }

    pub(super) const fn kind(self) -> ValueKind {
        match self.tag {
            ValueTag::Unit => ValueKind::Unit,
            ValueTag::Int => ValueKind::Int,
            ValueTag::Bool => ValueKind::Bool,
            ValueTag::Float => ValueKind::Float,
            ValueTag::Char => ValueKind::Char,
            ValueTag::Null => ValueKind::Null,
            ValueTag::ConstantString => ValueKind::ConstantString,
            ValueTag::Heap => ValueKind::Heap,
            ValueTag::Aggregate => ValueKind::Aggregate,
            ValueTag::Iterator => ValueKind::Iterator,
            ValueTag::IteratorStepDone
            | ValueTag::IteratorStepUnit
            | ValueTag::IteratorStepInt
            | ValueTag::IteratorStepBool
            | ValueTag::IteratorStepFloat
            | ValueTag::IteratorStepChar
            | ValueTag::IteratorStepNull
            | ValueTag::IteratorStepConstantString
            | ValueTag::IteratorStepHeap
            | ValueTag::IteratorStepAggregate
            | ValueTag::IteratorStepIterator => ValueKind::IteratorStep,
        }
    }

    pub(super) fn as_int(self) -> Result<i64, ExecutionError> {
        self.require(ValueTag::Int, ValueKind::Int)?;
        Ok(self.payload.cast_signed())
    }

    pub(super) fn as_bool(self) -> Result<bool, ExecutionError> {
        self.require(ValueTag::Bool, ValueKind::Bool)?;
        Ok(self.payload != 0)
    }

    pub(super) fn as_float(self) -> Result<f64, ExecutionError> {
        self.require(ValueTag::Float, ValueKind::Float)?;
        Ok(f64::from_bits(self.payload))
    }

    pub(super) fn as_char(self) -> Result<char, ExecutionError> {
        self.require(ValueTag::Char, ValueKind::Char)?;
        let scalar = u32::try_from(self.payload).map_err(|_| ExecutionError::TypeMismatch {
            expected: ValueKind::Char,
            found: ValueKind::Char,
        })?;
        char::from_u32(scalar).ok_or(ExecutionError::TypeMismatch {
            expected: ValueKind::Char,
            found: ValueKind::Char,
        })
    }

    pub(super) fn heap_index(self) -> Result<usize, ExecutionError> {
        self.require(ValueTag::Heap, ValueKind::Heap)?;
        usize::try_from(self.payload).map_err(|_| ExecutionError::InvalidVerifiedIndex {
            kind: crate::IndexKind::Heap,
            index: usize::MAX,
            bound: usize::MAX,
        })
    }

    pub(super) fn string_index(self) -> Result<usize, ExecutionError> {
        self.require(ValueTag::ConstantString, ValueKind::ConstantString)?;
        usize::try_from(self.payload).map_err(|_| ExecutionError::InvalidVerifiedIndex {
            kind: crate::IndexKind::String,
            index: usize::MAX,
            bound: usize::MAX,
        })
    }

    pub(super) fn aggregate_handle(self) -> Result<ArenaHandle, ExecutionError> {
        self.arena_handle(ValueTag::Aggregate, ValueKind::Aggregate)
    }

    pub(super) fn iterator_handle(self) -> Result<ArenaHandle, ExecutionError> {
        self.arena_handle(ValueTag::Iterator, ValueKind::Iterator)
    }

    pub(super) fn as_iterator_step(self) -> Result<Option<Self>, ExecutionError> {
        let tag = match self.tag {
            ValueTag::IteratorStepDone => return Ok(None),
            ValueTag::IteratorStepUnit => ValueTag::Unit,
            ValueTag::IteratorStepInt => ValueTag::Int,
            ValueTag::IteratorStepBool => ValueTag::Bool,
            ValueTag::IteratorStepFloat => ValueTag::Float,
            ValueTag::IteratorStepChar => ValueTag::Char,
            ValueTag::IteratorStepNull => ValueTag::Null,
            ValueTag::IteratorStepConstantString => ValueTag::ConstantString,
            ValueTag::IteratorStepHeap => ValueTag::Heap,
            ValueTag::IteratorStepAggregate => ValueTag::Aggregate,
            ValueTag::IteratorStepIterator => ValueTag::Iterator,
            ValueTag::Unit
            | ValueTag::Int
            | ValueTag::Bool
            | ValueTag::Float
            | ValueTag::Char
            | ValueTag::Null
            | ValueTag::ConstantString
            | ValueTag::Heap
            | ValueTag::Aggregate
            | ValueTag::Iterator => {
                return Err(ExecutionError::TypeMismatch {
                    expected: ValueKind::IteratorStep,
                    found: self.kind(),
                });
            }
        };
        Ok(Some(Self {
            tag,
            payload: self.payload,
        }))
    }

    fn arena_handle(
        self,
        tag: ValueTag,
        expected: ValueKind,
    ) -> Result<ArenaHandle, ExecutionError> {
        self.require(tag, expected)?;
        Ok(ArenaHandle::decode(self.payload))
    }

    fn require(self, tag: ValueTag, expected: ValueKind) -> Result<(), ExecutionError> {
        if self.tag == tag {
            Ok(())
        } else {
            Err(ExecutionError::TypeMismatch {
                expected,
                found: self.kind(),
            })
        }
    }
}

/// Materialized value returned by the interpreted program.
#[derive(Clone, Debug, PartialEq)]
pub enum ExitValue {
    /// Unit.
    Unit,
    /// Signed integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
    /// IEEE-754 float.
    Float(f64),
    /// Unicode scalar value.
    Char(char),
    /// Null.
    Null,
    /// UTF-8 string.
    String(String),
    /// List elements.
    List(Vec<Self>),
    /// Aggregate discriminant and fields.
    Aggregate {
        /// Enum discriminant, or zero for tuples and structs.
        variant: u64,
        /// Materialized aggregate fields.
        fields: Vec<Self>,
    },
}
