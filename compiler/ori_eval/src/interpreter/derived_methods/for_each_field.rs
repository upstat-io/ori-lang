//! `ForEachField` derive strategy — Eq, Compare, Hash over struct/variant fields.

use ori_ir::{CombineOp, DerivedMethodInfo, FieldOp, Name};

use super::super::Interpreter;
use crate::errors::wrong_function_args;
use crate::{EvalResult, StructValue, Value};

impl Interpreter<'_> {
    /// Apply a per-field operation and combine results.
    ///
    /// Routes to struct or variant handling based on the receiver's shape.
    /// Binary operations (Eq, Compare) require one argument; unary (Hash)
    /// requires none.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    pub(super) fn eval_for_each_field(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
        field_op: FieldOp,
        combine: CombineOp,
    ) -> EvalResult {
        let other = if matches!(field_op, FieldOp::Equals | FieldOp::Compare) {
            if args.len() != 1 {
                return Err(wrong_function_args(1, args.len()).into());
            }
            Some(&args[0])
        } else {
            None
        };

        match (&receiver, other) {
            (Value::Struct(self_s), Some(Value::Struct(other_s))) => {
                self.for_each_struct(self_s, Some(other_s), info, field_op, combine)
            }
            (Value::Struct(self_s), None) => {
                self.for_each_struct(self_s, None, info, field_op, combine)
            }
            (Value::Newtype { inner, .. }, None)
                if field_op == FieldOp::Hash && combine == CombineOp::HashCombine =>
            {
                Ok(Value::int(self.eval_hashable_value(inner)?))
            }
            (
                Value::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                Some(Value::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                }),
            ) => self.for_each_variant_binary(*t1, *v1, f1, *t2, *v2, f2, info, combine),
            (
                Value::Variant {
                    variant_name,
                    fields,
                    ..
                },
                None,
            ) => self.for_each_variant_unary(*variant_name, fields, info, combine),
            _ => match combine {
                CombineOp::AllTrue => Ok(Value::Bool(false)),
                CombineOp::HashCombine => Err(crate::errors::no_such_method(
                    info.trait_kind.method_name(),
                    "incompatible value",
                )
                .into()),
                CombineOp::Lexicographic => Err(crate::errors::no_such_method(
                    info.trait_kind.method_name(),
                    "incompatible values",
                )
                .into()),
            },
        }
    }

    /// `ForEachField` on named struct fields.
    fn for_each_struct(
        &mut self,
        self_s: &StructValue,
        other_s: Option<&StructValue>,
        info: &DerivedMethodInfo,
        field_op: FieldOp,
        combine: CombineOp,
    ) -> EvalResult {
        match (field_op, combine) {
            (FieldOp::Equals, CombineOp::AllTrue) => {
                let Some(other) = other_s else {
                    debug_assert!(false, "Equals requires other");
                    return Ok(Value::Bool(false));
                };
                if self_s.type_name != other.type_name {
                    return Ok(Value::Bool(false));
                }
                for field_name in &info.field_names {
                    match (self_s.get_field(*field_name), other.get_field(*field_name)) {
                        (Some(sv), Some(ov)) if sv == ov => {}
                        _ => return Ok(Value::Bool(false)),
                    }
                }
                Ok(Value::Bool(true))
            }
            (FieldOp::Compare, CombineOp::Lexicographic) => {
                use crate::methods::compare::{compare_values, ordering_to_value};
                let Some(other) = other_s else {
                    debug_assert!(false, "Compare requires other");
                    return Err(
                        crate::errors::no_such_method("compare", "missing other argument").into(),
                    );
                };
                if self_s.type_name != other.type_name {
                    return Err(
                        crate::errors::no_such_method("compare", "different struct types").into(),
                    );
                }
                for field_name in &info.field_names {
                    match (self_s.get_field(*field_name), other.get_field(*field_name)) {
                        (Some(sv), Some(ov)) => {
                            let ord = compare_values(sv, ov, self.interner)?;
                            if ord != std::cmp::Ordering::Equal {
                                return Ok(ordering_to_value(ord));
                            }
                        }
                        _ => {
                            return Err(crate::errors::no_such_method(
                                "compare",
                                "struct with missing field",
                            )
                            .into());
                        }
                    }
                }
                Ok(ordering_to_value(std::cmp::Ordering::Equal))
            }
            (FieldOp::Hash, CombineOp::HashCombine) => {
                use crate::methods::compare::hash_combine;
                let mut hash = 0_i64;
                for field_name in &info.field_names {
                    let Some(value) = self_s.get_field(*field_name) else {
                        return Err(crate::errors::no_such_method(
                            "hash",
                            "struct with missing field",
                        )
                        .into());
                    };
                    if is_unit_value(value) {
                        continue;
                    }
                    hash = hash_combine(hash, self.eval_hashable_value(value)?);
                }
                Ok(Value::int(hash))
            }
            // SAFETY: DerivedTrait::strategy() only produces valid (FieldOp, CombineOp) pairings:
            // (Equals, AllTrue), (Compare, Lexicographic), (Hash, HashCombine).
            // INVARIANT: enforced by the all_foreach_field_strategies_have_eval_dispatch test.
            _ => unreachable!(
                "unsupported FieldOp+CombineOp: {:?}+{:?}",
                field_op, combine
            ),
        }
    }

    /// `ForEachField` on variant payloads — binary case (Eq, Compare).
    #[expect(
        clippy::too_many_arguments,
        reason = "variant fields from destructured match arms"
    )]
    fn for_each_variant_binary(
        &self,
        t1: Name,
        v1: Name,
        f1: &[Value],
        t2: Name,
        v2: Name,
        f2: &[Value],
        info: &DerivedMethodInfo,
        combine: CombineOp,
    ) -> EvalResult {
        match combine {
            CombineOp::AllTrue => Ok(Value::Bool(t1 == t2 && v1 == v2 && f1 == f2)),
            CombineOp::Lexicographic => {
                use crate::methods::compare::{compare_values, ordering_to_value};
                let pos1 = info.variant_names.iter().position(|n| *n == v1);
                let pos2 = info.variant_names.iter().position(|n| *n == v2);
                match (pos1, pos2) {
                    (Some(i1), Some(i2)) => {
                        let ord = i1.cmp(&i2);
                        if ord != std::cmp::Ordering::Equal {
                            return Ok(ordering_to_value(ord));
                        }
                        for (sv, ov) in f1.iter().zip(f2.iter()) {
                            let ord = compare_values(sv, ov, self.interner)?;
                            if ord != std::cmp::Ordering::Equal {
                                return Ok(ordering_to_value(ord));
                            }
                        }
                        Ok(ordering_to_value(std::cmp::Ordering::Equal))
                    }
                    _ => Err(
                        crate::errors::no_such_method("compare", "variant not found in type")
                            .into(),
                    ),
                }
            }
            // SAFETY: strategy() pairs Hash with HashCombine; Hash is unary (no `other` param).
            CombineOp::HashCombine => unreachable!("Hash is unary, not binary"),
        }
    }

    /// `ForEachField` on variant payloads — unary case (Hash).
    fn for_each_variant_unary(
        &mut self,
        variant_name: Name,
        fields: &[Value],
        info: &DerivedMethodInfo,
        combine: CombineOp,
    ) -> EvalResult {
        match combine {
            CombineOp::HashCombine => {
                use crate::methods::compare::hash_combine;
                let Some(ordinal) = info
                    .variant_names
                    .iter()
                    .position(|name| *name == variant_name)
                else {
                    return Err(
                        crate::errors::no_such_method("hash", "variant not found in type").into(),
                    );
                };
                let ordinal = i64::try_from(ordinal).map_err(|_| {
                    crate::EvalError::new("variant declaration ordinal does not fit in int")
                })?;
                let mut hash = hash_combine(0, ordinal);
                for field in fields {
                    if is_unit_value(field) {
                        continue;
                    }
                    hash = hash_combine(hash, self.eval_hashable_value(field)?);
                }
                Ok(Value::int(hash))
            }
            // SAFETY: only Hash routes to unary variant handling; Hash pairs with HashCombine.
            CombineOp::AllTrue | CombineOp::Lexicographic => {
                unreachable!("only HashCombine uses unary variant handling")
            }
        }
    }
}

fn is_unit_value(value: &Value) -> bool {
    matches!(value, Value::Void) || matches!(value, Value::Tuple(fields) if fields.is_empty())
}

#[cfg(test)]
mod tests;
