//! Binary, unary, and cast operations for canonical expression evaluation.

use ori_ir::canon::{CanExpr, CanId};
use ori_ir::{BinaryOp, Name, UnaryOp};
use ori_patterns::{ControlAction, EvalError, EvalResult, Value};

use super::super::Interpreter;
use crate::{evaluate_binary, evaluate_unary};

impl Interpreter<'_> {
    /// Evaluate a canonical binary operation with short-circuit support.
    pub(super) fn eval_can_binary(
        &mut self,
        binary_id: CanId,
        left: CanId,
        op: BinaryOp,
        right: CanId,
    ) -> EvalResult {
        let left_val = self.eval_can(left)?;
        let span = self.can_span(binary_id);

        // Short-circuit for &&, ||, ??
        match op {
            BinaryOp::And => {
                if !left_val.is_truthy() {
                    return Ok(Value::Bool(false));
                }
                let right_val = self.eval_can(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            BinaryOp::Or => {
                if left_val.is_truthy() {
                    return Ok(Value::Bool(true));
                }
                let right_val = self.eval_can(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            BinaryOp::Coalesce => {
                // In canonical mode, we compare TypeIds directly (always available).
                let canon = self.canon_ref();
                let is_chaining = canon.arena.ty(left) == canon.arena.ty(binary_id);

                match left_val {
                    Value::Some(inner) => {
                        if is_chaining {
                            return Ok(Value::Some(inner));
                        }
                        return Ok((*inner).clone());
                    }
                    Value::Ok(inner) => {
                        if is_chaining {
                            return Ok(Value::Ok(inner));
                        }
                        return Ok((*inner).clone());
                    }
                    Value::None | Value::Err(_) => {
                        return self.eval_can(right);
                    }
                    _ => {
                        let err: ControlAction = EvalError::new(format!(
                            "operator '??' requires Option or Result, got {}",
                            left_val.type_name()
                        ))
                        .into();
                        return Err(Self::attach_span(err, span));
                    }
                }
            }
            _ => {}
        }

        let right_val = self.eval_can(right)?;

        // Primitive types use direct evaluation
        if super::super::is_primitive_value(&left_val)
            && super::super::is_primitive_value(&right_val)
        {
            return evaluate_binary(left_val, right_val, op)
                .map_err(|e| Self::attach_span(e, span));
        }

        // User-defined types: dispatch through method system
        if let Some(method) = super::super::binary_op_to_method(op, self.op_names) {
            return self.eval_method_call(left_val, method, vec![right_val]);
        }

        evaluate_binary(left_val, right_val, op).map_err(|e| Self::attach_span(e, span))
    }

    /// Evaluate a canonical unary operation.
    pub(super) fn eval_can_unary(
        &mut self,
        expr_id: CanId,
        op: UnaryOp,
        operand: CanId,
    ) -> EvalResult {
        let value = self.eval_can(operand)?;
        let span = self.can_span(expr_id);

        if super::super::is_primitive_value(&value) {
            return evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span));
        }

        if let Some(method) = super::super::unary_op_to_method(op, self.op_names) {
            return self.eval_method_call(value, method, vec![]);
        }

        evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span))
    }

    /// Evaluate a canonical type cast using the target type name.
    ///
    /// Uses pre-interned `TypeNames` for O(1) `Name` comparison instead of
    /// deinterning to `&str`. Only falls back to `interner.lookup()` on the
    /// cold error path for diagnostic messages.
    pub(super) fn eval_can_cast(&self, value: Value, target: Name, fallible: bool) -> EvalResult {
        let tn = &self.type_names;
        let result = match &value {
            // int conversions
            #[expect(
                clippy::cast_precision_loss,
                reason = "intentional int-to-float conversion"
            )]
            Value::Int(n) if target == tn.float => Ok(Value::Float(n.raw() as f64)),
            Value::Int(n) if target == tn.byte => {
                let raw = n.raw();
                if !(0..=255).contains(&raw) {
                    if fallible {
                        return Ok(Value::None);
                    }
                    return Err(EvalError::new(format!(
                        "value {raw} out of range for byte (0-255)"
                    ))
                    .into());
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "range checked on line above"
                )]
                Ok(Value::Byte(raw as u8))
            }
            Value::Int(n) if target == tn.char_ => {
                let raw = n.raw();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "char::from_u32 validates the value"
                )]
                if let Some(c) = char::from_u32(raw as u32) {
                    Ok(Value::Char(c))
                } else if fallible {
                    return Ok(Value::None);
                } else {
                    return Err(EvalError::new(format!(
                        "value {raw} is not a valid Unicode codepoint"
                    ))
                    .into());
                }
            }
            Value::Byte(b) if target == tn.int => Ok(Value::int(i64::from(*b))),
            Value::Char(c) if target == tn.int => Ok(Value::int(i64::from(*c as u32))),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "intentional float-to-int truncation"
            )]
            Value::Float(f) if target == tn.int => Ok(Value::int(*f as i64)),
            Value::Str(s) if target == tn.int => match s.parse::<i64>() {
                Ok(n) => Ok(Value::int(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as int")).into());
                }
            },
            Value::Str(s) if target == tn.float => match s.parse::<f64>() {
                Ok(n) => Ok(Value::Float(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as float")).into());
                }
            },
            // Identity conversions
            Value::Int(_) if target == tn.int => Ok(value),
            Value::Float(_) if target == tn.float => Ok(value),
            Value::Str(_) if target == tn.str_ => Ok(value),
            Value::Bool(_) if target == tn.bool_ => Ok(value),
            Value::Byte(_) if target == tn.byte => Ok(value),
            Value::Char(_) if target == tn.char_ => Ok(value),
            // str conversion - anything can become a string
            _ if target == tn.str_ => Ok(Value::string(value.to_string())),
            _ => {
                if fallible {
                    return Ok(Value::None);
                }
                let target_name = self.interner.lookup(target);
                Err(EvalError::new(format!(
                    "cannot convert {} to {target_name}",
                    value.type_name()
                ))
                .into())
            }
        };
        if fallible {
            result.map(Value::some)
        } else {
            result
        }
    }

    /// Evaluate a canonical expression with `#` resolved to a collection length.
    pub(super) fn eval_can_with_hash_length(&mut self, can_id: CanId, length: i64) -> EvalResult {
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(can_id);
        match kind {
            CanExpr::HashLength => Ok(Value::int(length)),
            CanExpr::Binary { op, left, right } => {
                let l = self.eval_can_with_hash_length(left, length)?;
                let r = self.eval_can_with_hash_length(right, length)?;
                evaluate_binary(l, r, op).map_err(|e| Self::attach_span(e, self.can_span(can_id)))
            }
            CanExpr::Unary {
                op: UnaryOp::Neg,
                operand,
            } => {
                let v = self.eval_can_with_hash_length(operand, length)?;
                evaluate_unary(v, UnaryOp::Neg)
                    .map_err(|e| Self::attach_span(e, self.can_span(can_id)))
            }
            _ => self.eval_can(can_id),
        }
    }
}
