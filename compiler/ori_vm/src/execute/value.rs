//! Compact interpreter values and observable result snapshots.

use crate::{ExecutionError, ValueKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct VmValue {
    tag: ValueTag,
    payload: u64,
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

    pub(super) fn aggregate(frame: usize, slot: u32) -> Result<Self, ExecutionError> {
        let frame = u32::try_from(frame)
            .map_err(|_| ExecutionError::FrameHandleOverflow { depth: frame })?;
        Ok(Self {
            tag: ValueTag::Aggregate,
            payload: (u64::from(frame) << 32) | u64::from(slot),
        })
    }

    pub(super) fn iterator(frame: usize, slot: u32) -> Result<Self, ExecutionError> {
        let frame = u32::try_from(frame)
            .map_err(|_| ExecutionError::FrameHandleOverflow { depth: frame })?;
        Ok(Self {
            tag: ValueTag::Iterator,
            payload: (u64::from(frame) << 32) | u64::from(slot),
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

    pub(super) fn aggregate_parts(self) -> Result<(usize, usize), ExecutionError> {
        self.local_parts(ValueTag::Aggregate, ValueKind::Aggregate)
    }

    pub(super) fn iterator_parts(self) -> Result<(usize, usize), ExecutionError> {
        self.local_parts(ValueTag::Iterator, ValueKind::Iterator)
    }

    fn local_parts(
        self,
        tag: ValueTag,
        expected: ValueKind,
    ) -> Result<(usize, usize), ExecutionError> {
        self.require(tag, expected)?;
        let frame = u32::try_from(self.payload >> 32)
            .map_err(|_| ExecutionError::FrameHandleOverflow { depth: usize::MAX })?;
        let slot = u32::try_from(self.payload & u64::from(u32::MAX)).map_err(|_| {
            ExecutionError::LocalHandleOutOfBounds {
                kind: expected,
                frame: usize::MAX,
                slot: usize::MAX,
            }
        })?;
        let frame = usize::try_from(frame)
            .map_err(|_| ExecutionError::FrameHandleOverflow { depth: usize::MAX })?;
        let slot = usize::try_from(slot).map_err(|_| ExecutionError::LocalHandleOutOfBounds {
            kind: expected,
            frame,
            slot: usize::MAX,
        })?;
        Ok((frame, slot))
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
