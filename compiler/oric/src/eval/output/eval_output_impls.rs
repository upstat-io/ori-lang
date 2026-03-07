//! Manual `PartialEq` and `Hash` implementations for `EvalOutput`.
//!
//! Extracted from `output/mod.rs` to keep the file within the 500-line limit.
//! These impls are inherent to the type defined in `mod.rs`.

use std::hash::{Hash, Hasher};

use super::EvalOutput;

impl PartialEq for EvalOutput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // i64 types (Int, Duration in nanoseconds)
            (EvalOutput::Int(a), EvalOutput::Int(b))
            | (EvalOutput::Duration(a), EvalOutput::Duration(b)) => a == b,
            (EvalOutput::Bool(a), EvalOutput::Bool(b)) => a == b,
            (EvalOutput::Byte(a), EvalOutput::Byte(b)) => a == b,
            // i8 types (Ordering tag)
            (EvalOutput::Ordering(a), EvalOutput::Ordering(b)) => a == b,
            (EvalOutput::Char(a), EvalOutput::Char(b)) => a == b,
            // u64 types (Float stored as bits, Size in bytes)
            (EvalOutput::Float(a), EvalOutput::Float(b))
            | (EvalOutput::Size(a), EvalOutput::Size(b)) => a == b,
            // String types can be merged
            (EvalOutput::Str(a), EvalOutput::Str(b))
            | (EvalOutput::Error(a), EvalOutput::Error(b)) => a == b,
            (
                EvalOutput::Function {
                    description: a,
                    arity: ar1,
                },
                EvalOutput::Function {
                    description: b,
                    arity: ar2,
                },
            ) => a == b && ar1 == ar2,
            (
                EvalOutput::Struct {
                    description: a,
                    field_count: fc1,
                },
                EvalOutput::Struct {
                    description: b,
                    field_count: fc2,
                },
            ) => a == b && fc1 == fc2,
            // Vec<EvalOutput> types can be merged
            (EvalOutput::List(a), EvalOutput::List(b))
            | (EvalOutput::Tuple(a), EvalOutput::Tuple(b))
            | (EvalOutput::Set(a), EvalOutput::Set(b)) => a == b,
            // Box<EvalOutput> types can be merged
            (EvalOutput::Some(a), EvalOutput::Some(b))
            | (EvalOutput::Ok(a), EvalOutput::Ok(b))
            | (EvalOutput::Err(a), EvalOutput::Err(b)) => a == b,
            (EvalOutput::Map(a), EvalOutput::Map(b)) => a == b,
            // Unit types
            (EvalOutput::Void, EvalOutput::Void) | (EvalOutput::None, EvalOutput::None) => true,
            // Range with multiple fields
            (
                EvalOutput::Range {
                    start: s1,
                    end: e1,
                    step: st1,
                    inclusive: i1,
                },
                EvalOutput::Range {
                    start: s2,
                    end: e2,
                    step: st2,
                    inclusive: i2,
                },
            ) => s1 == s2 && e1 == e2 && st1 == st2 && i1 == i2,
            // Variant with type, variant name, and fields
            (
                EvalOutput::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                EvalOutput::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                },
            ) => t1 == t2 && v1 == v2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for EvalOutput {}

impl Hash for EvalOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            EvalOutput::Int(n) => n.hash(state),
            EvalOutput::Bool(b) => b.hash(state),
            EvalOutput::Char(c) => c.hash(state),
            EvalOutput::Byte(b) => b.hash(state),
            EvalOutput::Ordering(tag) => tag.hash(state),
            // u64 types
            EvalOutput::Float(bits) | EvalOutput::Size(bits) => {
                bits.hash(state);
            }
            // i64 types
            EvalOutput::Duration(ns) => ns.hash(state),
            // String types
            EvalOutput::Str(s) | EvalOutput::Error(s) => s.hash(state),
            EvalOutput::Function { description, arity } => {
                description.hash(state);
                arity.hash(state);
            }
            EvalOutput::Struct {
                description,
                field_count,
            } => {
                description.hash(state);
                field_count.hash(state);
            }
            // Vec<EvalOutput> types
            EvalOutput::List(items) | EvalOutput::Tuple(items) | EvalOutput::Set(items) => {
                items.hash(state);
            }
            // Box<EvalOutput> types
            EvalOutput::Some(v) | EvalOutput::Ok(v) | EvalOutput::Err(v) => v.hash(state),
            EvalOutput::Map(entries) => entries.hash(state),
            // Unit types
            EvalOutput::Void | EvalOutput::None => {}
            EvalOutput::Range {
                start,
                end,
                step,
                inclusive,
            } => {
                start.hash(state);
                end.hash(state);
                step.hash(state);
                inclusive.hash(state);
            }
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                type_name.hash(state);
                variant_name.hash(state);
                fields.hash(state);
            }
        }
    }
}
