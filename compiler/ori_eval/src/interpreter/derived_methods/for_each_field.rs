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
        &self,
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
            ) => self.for_each_variant_unary(*variant_name, fields, combine),
            _ => match combine {
                CombineOp::AllTrue => Ok(Value::Bool(false)),
                CombineOp::HashCombine => {
                    use crate::methods::compare::FNV_OFFSET_BASIS;
                    Ok(Value::int(FNV_OFFSET_BASIS.cast_signed()))
                }
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
        &self,
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
                use crate::methods::compare::{hash_value, FNV_OFFSET_BASIS, FNV_PRIME};
                let mut hash = FNV_OFFSET_BASIS;
                for field_name in &info.field_names {
                    if let Some(val) = self_s.get_field(*field_name) {
                        let field_hash = hash_value(val, self.interner)?.cast_unsigned();
                        hash ^= field_hash;
                        hash = hash.wrapping_mul(FNV_PRIME);
                    }
                }
                Ok(Value::int(hash.cast_signed()))
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
        &self,
        variant_name: Name,
        fields: &[Value],
        combine: CombineOp,
    ) -> EvalResult {
        match combine {
            CombineOp::HashCombine => {
                use crate::methods::compare::{
                    fnv1a_hash, hash_value, FNV_OFFSET_BASIS, FNV_PRIME,
                };
                let mut hash = FNV_OFFSET_BASIS;
                let variant_str = self.interner.lookup(variant_name);
                let discriminant = fnv1a_hash(variant_str.as_bytes()).cast_unsigned();
                hash ^= discriminant;
                hash = hash.wrapping_mul(FNV_PRIME);
                for field in fields {
                    let field_hash = hash_value(field, self.interner)?.cast_unsigned();
                    hash ^= field_hash;
                    hash = hash.wrapping_mul(FNV_PRIME);
                }
                Ok(Value::int(hash.cast_signed()))
            }
            // SAFETY: only Hash routes to unary variant handling; Hash pairs with HashCombine.
            CombineOp::AllTrue | CombineOp::Lexicographic => {
                unreachable!("only HashCombine uses unary variant handling")
            }
        }
    }
}
