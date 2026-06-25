//! Supporting value and diagnostic types for canonical IR.
//!
//! [`CanMapEntry`] / [`CanField`] are the entry/field element types referenced by
//! `CanExpr` collection variants. [`ConstValue`] is the compile-time-folded value
//! stored in a constant pool. [`PatternProblem`] is the exhaustiveness/usefulness
//! diagnostic produced after decision-tree compilation. None is the `CanExpr` enum
//! itself; they are co-located by reference from canonicalization output.

use std::hash::{Hash, Hasher};

use crate::{DurationUnit, Name, SizeUnit, Span};

use super::ids::CanId;

/// A map entry in canonical form: key-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanMapEntry {
    pub key: CanId,
    pub value: CanId,
}

/// A struct field initializer in canonical form: name-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanField {
    pub name: Name,
    pub value: CanId,
}

/// A compile-time constant value stored in a [`ConstantPool`](super::pools::ConstantPool).
///
/// These are produced by constant folding during canonicalization.
/// Only values that can be fully determined at compile time are
/// represented here.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(u64),
    Bool(bool),
    Str(Name),
    Char(char),
    Unit,
    Duration { value: u64, unit: DurationUnit },
    Size { value: u64, unit: SizeUnit },
}

impl Eq for ConstValue {}

impl Hash for ConstValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ConstValue::Int(v) => v.hash(state),
            ConstValue::Float(v) => v.hash(state),
            ConstValue::Bool(v) => v.hash(state),
            ConstValue::Str(v) => v.hash(state),
            ConstValue::Char(v) => v.hash(state),
            ConstValue::Unit => {}
            ConstValue::Duration { value, unit } => {
                value.hash(state);
                unit.hash(state);
            }
            ConstValue::Size { value, unit } => {
                value.hash(state);
                unit.hash(state);
            }
        }
    }
}

/// A pattern-related problem detected during canonicalization.
///
/// These are produced by the exhaustiveness checker after decision tree
/// compilation. Both variants carry spans for rich diagnostic rendering.
///
/// # Salsa Compatibility
///
/// Derives `Clone, Eq, PartialEq, Hash, Debug` for Salsa query return types.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum PatternProblem {
    /// A match expression does not cover all possible values.
    NonExhaustive {
        /// Span of the `match` keyword / expression.
        match_span: Span,
        /// Human-readable descriptions of missing patterns (e.g. `"false"`, `"_"`).
        missing: Vec<String>,
    },
    /// A match arm can never be reached because earlier arms cover all its cases.
    RedundantArm {
        /// Span of the unreachable arm.
        arm_span: Span,
        /// Span of the enclosing match expression.
        match_span: Span,
        /// Zero-based index of the redundant arm.
        arm_index: usize,
    },
}
