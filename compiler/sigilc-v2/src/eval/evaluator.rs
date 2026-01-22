//! Expression evaluator for the Sigil interpreter.

use std::rc::Rc;
use std::cell::RefCell;
use crate::intern::{Name, StringInterner};
use crate::syntax::{ExprId, ExprKind, ExprArena, BinaryOp, UnaryOp, StmtKind, BindingPattern, PatternKind};
use super::value::{Value, FunctionValue, RangeValue};
use super::environment::Environment;

/// Result of evaluation.
pub type EvalResult = Result<Value, EvalError>;

/// Evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Error message.
    pub message: String,
    /// If this error is from `?` propagation, holds the original Err/None value.
    pub propagated_value: Option<Value>,
}

impl EvalError {
    pub fn new(message: impl Into<String>) -> Self {
        EvalError { message: message.into(), propagated_value: None }
    }

    /// Create an error for propagating an Err or None value.
    pub fn propagate(value: Value, message: impl Into<String>) -> Self {
        EvalError { message: message.into(), propagated_value: Some(value) }
    }
}

/// Tree-walking evaluator for Sigil expressions.
pub struct Evaluator<'a> {
    /// String interner for name lookup.
    interner: &'a StringInterner,
    /// Expression arena.
    arena: &'a ExprArena,
    /// Current environment.
    env: Environment,
}

impl<'a> Evaluator<'a> {
    /// Create a new evaluator.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Evaluator {
            interner,
            arena,
            env: Environment::new(),
        }
    }

    /// Create an evaluator with an existing environment.
    pub fn with_env(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment) -> Self {
        Evaluator { interner, arena, env }
    }

    /// Evaluate an expression.
    pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        let expr = self.arena.get(expr_id);
        match &expr.kind {
            // Literals
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(f) => Ok(Value::Float(*f)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::String(s) => {
                let string = self.interner.lookup(*s).to_string();
                Ok(Value::Str(Rc::new(string)))
            }
            ExprKind::Char(c) => Ok(Value::Char(*c)),
            ExprKind::Unit => Ok(Value::Void),

            // Duration and Size
            ExprKind::Duration { value, unit } => {
                Ok(Value::Duration(unit.to_millis(*value)))
            }
            ExprKind::Size { value, unit } => {
                Ok(Value::Size(unit.to_bytes(*value)))
            }

            // Identifiers
            ExprKind::Ident(name) => {
                self.env.lookup(*name).ok_or_else(|| {
                    let name_str = self.interner.lookup(*name);
                    EvalError::new(format!("undefined variable: {}", name_str))
                })
            }

            // Binary and unary
            ExprKind::Binary { left, op, right } => {
                self.eval_binary(*left, *op, *right)
            }

            ExprKind::Unary { op, operand } => {
                self.eval_unary(*op, *operand)
            }

            // Control flow
            ExprKind::If { cond, then_branch, else_branch } => {
                let cond_val = self.eval(*cond)?;
                if cond_val.is_truthy() {
                    self.eval(*then_branch)
                } else if let Some(else_expr) = else_branch {
                    self.eval(*else_expr)
                } else {
                    Ok(Value::Void)
                }
            }

            // Collections
            ExprKind::List(range) => {
                let items = self.arena.get_expr_list(*range);
                let values: Result<Vec<_>, _> = items.iter()
                    .map(|id| self.eval(*id))
                    .collect();
                Ok(Value::List(Rc::new(values?)))
            }

            ExprKind::Tuple(range) => {
                let items = self.arena.get_expr_list(*range);
                let values: Result<Vec<_>, _> = items.iter()
                    .map(|id| self.eval(*id))
                    .collect();
                Ok(Value::Tuple(Rc::new(values?)))
            }

            // Range
            ExprKind::Range { start, end, inclusive } => {
                let start_val = if let Some(s) = start {
                    self.eval(*s)?.as_int()
                        .ok_or_else(|| EvalError::new("range start must be an integer"))?
                } else {
                    0
                };
                let end_val = if let Some(e) = end {
                    self.eval(*e)?.as_int()
                        .ok_or_else(|| EvalError::new("range end must be an integer"))?
                } else {
                    return Err(EvalError::new("unbounded range end"));
                };

                if *inclusive {
                    Ok(Value::Range(RangeValue::inclusive(start_val, end_val)))
                } else {
                    Ok(Value::Range(RangeValue::exclusive(start_val, end_val)))
                }
            }

            // Access
            ExprKind::Index { receiver, index } => {
                let value = self.eval(*receiver)?;
                // Get the length for HashLength resolution
                let length = self.get_collection_length(&value)?;
                let idx = self.eval_with_hash_length(*index, length)?;
                self.eval_index(value, idx)
            }

            ExprKind::Field { receiver, field } => {
                let value = self.eval(*receiver)?;
                self.eval_field_access(value, *field)
            }

            // Lambda
            ExprKind::Lambda { params, body, .. } => {
                let param_names: Vec<Name> = self.arena.get_params(*params)
                    .iter()
                    .map(|p| p.name)
                    .collect();

                // Capture the current environment
                let captures = self.env.capture();

                Ok(Value::Function(FunctionValue {
                    params: param_names,
                    body: *body,
                    captures: Rc::new(RefCell::new(captures.into_iter().collect())),
                }))
            }

            // Block
            ExprKind::Block { stmts, result } => {
                self.eval_block(*stmts, *result)
            }

            // Call
            ExprKind::Call { func, args } => {
                let func_val = self.eval(*func)?;
                let arg_list = self.arena.get_expr_list(*args);
                let arg_values: Result<Vec<_>, _> = arg_list.iter()
                    .map(|id| self.eval(*id))
                    .collect();
                self.eval_call(func_val, arg_values?)
            }

            // Variant constructors
            ExprKind::Some(inner) => {
                let value = self.eval(*inner)?;
                Ok(Value::Some(Box::new(value)))
            }
            ExprKind::None => Ok(Value::None),
            ExprKind::Ok(inner) => {
                let value = if let Some(e) = inner {
                    self.eval(*e)?
                } else {
                    Value::Void
                };
                Ok(Value::Ok(Box::new(value)))
            }
            ExprKind::Err(inner) => {
                let value = if let Some(e) = inner {
                    self.eval(*e)?
                } else {
                    Value::Void
                };
                Ok(Value::Err(Box::new(value)))
            }

            // Let binding (in expression context)
            ExprKind::Let { pattern, init, mutable, .. } => {
                let value = self.eval(*init)?;
                self.bind_pattern(pattern, value, *mutable)?;
                Ok(Value::Void)
            }

            // Patterns
            ExprKind::Pattern { kind, args } => {
                self.eval_pattern(*kind, *args)
            }

            // Function reference
            ExprKind::FunctionRef(name) => {
                self.env.lookup(*name).ok_or_else(|| {
                    let name_str = self.interner.lookup(*name);
                    EvalError::new(format!("undefined function: @{}", name_str))
                })
            }

            // Method call
            ExprKind::MethodCall { receiver, method, args } => {
                let recv_val = self.eval(*receiver)?;
                let arg_list = self.arena.get_expr_list(*args);
                let arg_values: Result<Vec<_>, _> = arg_list.iter()
                    .map(|id| self.eval(*id))
                    .collect();
                self.eval_method_call(recv_val, *method, arg_values?)
            }

            // Match expression
            ExprKind::Match { scrutinee, arms } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(value, *arms)
            }

            // For loop
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                let iter_val = self.eval(*iter)?;
                self.eval_for(*binding, iter_val, *guard, *body, *is_yield)
            }

            // Loop
            ExprKind::Loop { body } => {
                self.eval_loop(*body)
            }

            // Map literal
            ExprKind::Map(entries) => {
                let entry_list = self.arena.get_map_entries(*entries);
                let mut map = std::collections::HashMap::new();
                for entry in entry_list {
                    let key = self.eval(entry.key)?;
                    let value = self.eval(entry.value)?;
                    if let Value::Str(k) = key {
                        map.insert(k.to_string(), value);
                    } else {
                        return Err(EvalError::new("map keys must be strings"));
                    }
                }
                Ok(Value::Map(Rc::new(map)))
            }

            // Struct literal
            ExprKind::Struct { name, fields } => {
                let field_list = self.arena.get_field_inits(*fields);
                let mut field_values = std::collections::HashMap::new();
                for field in field_list {
                    let value = if let Some(v) = field.value {
                        self.eval(v)?
                    } else {
                        // Shorthand: { x } means { x: x }
                        self.env.lookup(field.name).ok_or_else(|| {
                            let name_str = self.interner.lookup(field.name);
                            EvalError::new(format!("undefined variable: {}", name_str))
                        })?
                    };
                    field_values.insert(field.name, value);
                }
                Ok(Value::Struct(super::value::StructValue::new(*name, field_values)))
            }

            // Return
            ExprKind::Return(val) => {
                let value = if let Some(v) = val {
                    self.eval(*v)?
                } else {
                    Value::Void
                };
                Err(EvalError::new(format!("return:{}", self.value_to_string(&value))))
            }

            // Break
            ExprKind::Break(val) => {
                let value = if let Some(v) = val {
                    self.eval(*v)?
                } else {
                    Value::Void
                };
                Err(EvalError::new(format!("break:{}", self.value_to_string(&value))))
            }

            // Continue
            ExprKind::Continue => {
                Err(EvalError::new("continue"))
            }

            // Assignment
            ExprKind::Assign { target, value } => {
                let val = self.eval(*value)?;
                self.eval_assign(*target, val)
            }

            // Try expression (error propagation)
            ExprKind::Try(inner) => {
                let result = self.eval(*inner)?;
                match result {
                    Value::Ok(v) => Ok(*v),
                    Value::Err(e) => Err(EvalError::propagate(
                        Value::Err(e.clone()),
                        format!("propagated error: {}", self.value_to_string(&e))
                    )),
                    Value::Some(v) => Ok(*v),
                    Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
                    other => Ok(other),
                }
            }

            // Config reference
            ExprKind::Config(name) => {
                self.env.lookup(*name).ok_or_else(|| {
                    let name_str = self.interner.lookup(*name);
                    EvalError::new(format!("undefined config: ${}", name_str))
                })
            }

            // Error placeholder
            ExprKind::Error => Err(EvalError::new("parse error")),

            // Hash length (in index context)
            ExprKind::HashLength => Err(EvalError::new("# can only be used inside index brackets")),

            // Self reference
            ExprKind::SelfRef => {
                // Look for "self" in the environment
                let self_name = self.interner.intern("self");
                self.env.lookup(self_name).ok_or_else(|| {
                    EvalError::new("'self' used outside of method context")
                })
            }

            // Await (not supported in interpreter)
            ExprKind::Await(_) => Err(EvalError::new("await not supported in interpreter")),
        }
    }

    /// Evaluate a binary operation.
    fn eval_binary(&mut self, left: ExprId, op: BinaryOp, right: ExprId) -> EvalResult {
        let left_val = self.eval(left)?;

        // Short-circuit for && and ||
        match op {
            BinaryOp::And => {
                if !left_val.is_truthy() {
                    return Ok(Value::Bool(false));
                }
                let right_val = self.eval(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            BinaryOp::Or => {
                if left_val.is_truthy() {
                    return Ok(Value::Bool(true));
                }
                let right_val = self.eval(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            _ => {}
        }

        let right_val = self.eval(right)?;

        match (left_val, right_val) {
            // Integer operations
            (Value::Int(a), Value::Int(b)) => match op {
                BinaryOp::Add => Ok(Value::Int(a + b)),
                BinaryOp::Sub => Ok(Value::Int(a - b)),
                BinaryOp::Mul => Ok(Value::Int(a * b)),
                BinaryOp::Div => {
                    if b == 0 {
                        Err(EvalError::new("division by zero"))
                    } else {
                        Ok(Value::Int(a / b))
                    }
                }
                BinaryOp::Mod => {
                    if b == 0 {
                        Err(EvalError::new("modulo by zero"))
                    } else {
                        Ok(Value::Int(a % b))
                    }
                }
                BinaryOp::FloorDiv => {
                    if b == 0 {
                        Err(EvalError::new("division by zero"))
                    } else {
                        // Floor division: truncate toward negative infinity
                        let result = a / b;
                        let remainder = a % b;
                        // Adjust if signs differ and there's a remainder
                        if remainder != 0 && (a < 0) != (b < 0) {
                            Ok(Value::Int(result - 1))
                        } else {
                            Ok(Value::Int(result))
                        }
                    }
                }
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::Lt => Ok(Value::Bool(a < b)),
                BinaryOp::Le => Ok(Value::Bool(a <= b)),
                BinaryOp::Gt => Ok(Value::Bool(a > b)),
                BinaryOp::Ge => Ok(Value::Bool(a >= b)),
                BinaryOp::BitAnd => Ok(Value::Int(a & b)),
                BinaryOp::BitOr => Ok(Value::Int(a | b)),
                BinaryOp::BitXor => Ok(Value::Int(a ^ b)),
                BinaryOp::Shl => Ok(Value::Int(a << (b as u32))),
                BinaryOp::Shr => Ok(Value::Int(a >> (b as u32))),
                BinaryOp::Range => Ok(Value::Range(RangeValue::exclusive(a, b))),
                BinaryOp::RangeInc => Ok(Value::Range(RangeValue::inclusive(a, b))),
                _ => Err(EvalError::new("invalid operator for integers")),
            },

            // Float operations
            (Value::Float(a), Value::Float(b)) => match op {
                BinaryOp::Add => Ok(Value::Float(a + b)),
                BinaryOp::Sub => Ok(Value::Float(a - b)),
                BinaryOp::Mul => Ok(Value::Float(a * b)),
                BinaryOp::Div => Ok(Value::Float(a / b)),
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::Lt => Ok(Value::Bool(a < b)),
                BinaryOp::Le => Ok(Value::Bool(a <= b)),
                BinaryOp::Gt => Ok(Value::Bool(a > b)),
                BinaryOp::Ge => Ok(Value::Bool(a >= b)),
                _ => Err(EvalError::new("invalid operator for floats")),
            },

            // Mixed int/float
            (Value::Int(a), Value::Float(b)) => {
                self.eval_binary_float(a as f64, op, b)
            }
            (Value::Float(a), Value::Int(b)) => {
                self.eval_binary_float(a, op, b as f64)
            }

            // Boolean operations
            (Value::Bool(a), Value::Bool(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                _ => Err(EvalError::new("invalid operator for booleans")),
            },

            // String operations
            (Value::Str(a), Value::Str(b)) => match op {
                BinaryOp::Add => {
                    let result = format!("{}{}", a, b);
                    Ok(Value::Str(Rc::new(result)))
                }
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for strings")),
            },

            // List operations
            (Value::List(a), Value::List(b)) => match op {
                BinaryOp::Add => {
                    let mut result = (*a).clone();
                    result.extend((*b).iter().cloned());
                    Ok(Value::List(Rc::new(result)))
                }
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for lists")),
            },

            // Option comparisons
            (Value::Some(a), Value::Some(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for Option")),
            },
            (Value::None, Value::None) => match op {
                BinaryOp::Eq => Ok(Value::Bool(true)),
                BinaryOp::Ne => Ok(Value::Bool(false)),
                _ => Err(EvalError::new("invalid operator for Option")),
            },
            (Value::Some(_), Value::None) | (Value::None, Value::Some(_)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(false)),
                BinaryOp::Ne => Ok(Value::Bool(true)),
                _ => Err(EvalError::new("invalid operator for Option")),
            },

            // Result comparisons
            (Value::Ok(a), Value::Ok(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for Result")),
            },
            (Value::Err(a), Value::Err(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for Result")),
            },
            (Value::Ok(_), Value::Err(_)) | (Value::Err(_), Value::Ok(_)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(false)),
                BinaryOp::Ne => Ok(Value::Bool(true)),
                _ => Err(EvalError::new("invalid operator for Result")),
            },

            // Tuple comparisons
            (Value::Tuple(a), Value::Tuple(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::Ne => Ok(Value::Bool(*a != *b)),
                _ => Err(EvalError::new("invalid operator for tuples")),
            },

            // Char comparisons
            (Value::Char(a), Value::Char(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::Lt => Ok(Value::Bool(a < b)),
                BinaryOp::Le => Ok(Value::Bool(a <= b)),
                BinaryOp::Gt => Ok(Value::Bool(a > b)),
                BinaryOp::Ge => Ok(Value::Bool(a >= b)),
                _ => Err(EvalError::new("invalid operator for char")),
            },

            (left, right) => {
                Err(EvalError::new(format!(
                    "type mismatch in binary operation: {} and {}",
                    left.type_name(), right.type_name()
                )))
            }
        }
    }

    fn eval_binary_float(&self, a: f64, op: BinaryOp, b: f64) -> EvalResult {
        match op {
            BinaryOp::Add => Ok(Value::Float(a + b)),
            BinaryOp::Sub => Ok(Value::Float(a - b)),
            BinaryOp::Mul => Ok(Value::Float(a * b)),
            BinaryOp::Div => Ok(Value::Float(a / b)),
            BinaryOp::Lt => Ok(Value::Bool(a < b)),
            BinaryOp::Le => Ok(Value::Bool(a <= b)),
            BinaryOp::Gt => Ok(Value::Bool(a > b)),
            BinaryOp::Ge => Ok(Value::Bool(a >= b)),
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::Ne => Ok(Value::Bool(a != b)),
            _ => Err(EvalError::new("invalid operator for floats")),
        }
    }

    /// Evaluate a unary operation.
    fn eval_unary(&mut self, op: UnaryOp, operand: ExprId) -> EvalResult {
        let value = self.eval(operand)?;
        match (op, value) {
            (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOp::BitNot, Value::Int(n)) => Ok(Value::Int(!n)),
            (op, value) => Err(EvalError::new(format!(
                "invalid unary {:?} on {}", op, value.type_name()
            ))),
        }
    }

    /// Evaluate an index operation.
    /// Get the length of a collection for HashLength resolution.
    fn get_collection_length(&self, value: &Value) -> Result<i64, EvalError> {
        match value {
            Value::List(items) => Ok(items.len() as i64),
            Value::Str(s) => Ok(s.chars().count() as i64),
            Value::Map(map) => Ok(map.len() as i64),
            Value::Tuple(items) => Ok(items.len() as i64),
            _ => Err(EvalError::new(format!(
                "cannot get length of {}", value.type_name()
            ))),
        }
    }

    /// Evaluate an expression with # (HashLength) resolved to a specific length.
    fn eval_with_hash_length(&mut self, expr_id: ExprId, length: i64) -> EvalResult {
        let expr = self.arena.get(expr_id);
        match &expr.kind {
            ExprKind::HashLength => Ok(Value::Int(length)),
            // For binary operations, recursively evaluate both sides with hash length available
            ExprKind::Binary { left, op, right } => {
                let left_val = self.eval_with_hash_length(*left, length)?;
                let right_val = self.eval_with_hash_length(*right, length)?;
                self.eval_binary_values(left_val, *op, right_val)
            }
            // For other expressions, evaluate normally
            _ => self.eval(expr_id),
        }
    }

    /// Evaluate binary operation on already-evaluated values.
    fn eval_binary_values(&mut self, left_val: Value, op: BinaryOp, right_val: Value) -> EvalResult {
        match (left_val, right_val) {
            (Value::Int(a), Value::Int(b)) => match op {
                BinaryOp::Add => Ok(Value::Int(a + b)),
                BinaryOp::Sub => Ok(Value::Int(a - b)),
                BinaryOp::Mul => Ok(Value::Int(a * b)),
                BinaryOp::Div => {
                    if b == 0 {
                        Err(EvalError::new("division by zero"))
                    } else {
                        Ok(Value::Int(a / b))
                    }
                }
                _ => Err(EvalError::new("operator not supported in index context")),
            },
            _ => Err(EvalError::new("non-integer in index context")),
        }
    }

    fn eval_index(&self, value: Value, index: Value) -> EvalResult {
        match (value, index) {
            (Value::List(items), Value::Int(i)) => {
                let idx = if i < 0 {
                    (items.len() as i64 + i) as usize
                } else {
                    i as usize
                };
                items.get(idx).cloned().ok_or_else(|| {
                    EvalError::new(format!("index {} out of bounds", i))
                })
            }
            (Value::Str(s), Value::Int(i)) => {
                let idx = if i < 0 {
                    (s.len() as i64 + i) as usize
                } else {
                    i as usize
                };
                s.chars().nth(idx)
                    .map(Value::Char)
                    .ok_or_else(|| EvalError::new(format!("index {} out of bounds", i)))
            }
            (Value::Map(map), Value::Str(key)) => {
                map.get(key.as_str()).cloned()
                    .ok_or_else(|| EvalError::new(format!("key not found: {}", key)))
            }
            (value, index) => Err(EvalError::new(format!(
                "cannot index {} with {}", value.type_name(), index.type_name()
            ))),
        }
    }

    /// Evaluate field access.
    fn eval_field_access(&self, value: Value, field: Name) -> EvalResult {
        match value {
            Value::Struct(s) => {
                s.get_field(field).cloned().ok_or_else(|| {
                    let field_name = self.interner.lookup(field);
                    EvalError::new(format!("no field {} on struct", field_name))
                })
            }
            Value::Tuple(items) => {
                // Tuple field access like t.0, t.1
                let field_name = self.interner.lookup(field);
                if let Ok(idx) = field_name.parse::<usize>() {
                    items.get(idx).cloned().ok_or_else(|| {
                        EvalError::new(format!("tuple index {} out of bounds", idx))
                    })
                } else {
                    Err(EvalError::new(format!("invalid tuple field: {}", field_name)))
                }
            }
            value => Err(EvalError::new(format!(
                "cannot access field on {}", value.type_name()
            ))),
        }
    }

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: crate::syntax::StmtRange, result: Option<ExprId>) -> EvalResult {
        self.env.push_scope();

        let stmt_list = self.arena.get_stmts(stmts);

        for stmt in stmt_list {
            match &stmt.kind {
                StmtKind::Expr(expr) => {
                    self.eval(*expr)?;
                }
                StmtKind::Let { pattern, init, mutable, .. } => {
                    let value = self.eval(*init)?;
                    self.bind_pattern(pattern, value, *mutable)?;
                }
            }
        }

        let result_val = if let Some(r) = result {
            self.eval(r)?
        } else {
            Value::Void
        };

        self.env.pop_scope();
        Ok(result_val)
    }

    /// Bind a pattern to a value.
    fn bind_pattern(&mut self, pattern: &BindingPattern, value: Value, mutable: bool) -> EvalResult {
        match pattern {
            BindingPattern::Name(name) => {
                self.env.define(*name, value, mutable);
                Ok(Value::Void)
            }
            BindingPattern::Wildcard => Ok(Value::Void),
            BindingPattern::Tuple(patterns) => {
                if let Value::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return Err(EvalError::new("tuple pattern length mismatch"));
                    }
                    for (pat, val) in patterns.iter().zip(values.iter()) {
                        self.bind_pattern(pat, val.clone(), mutable)?;
                    }
                    Ok(Value::Void)
                } else {
                    Err(EvalError::new("expected tuple value"))
                }
            }
            BindingPattern::Struct { fields } => {
                if let Value::Struct(s) = value {
                    for (field_name, binding) in fields {
                        if let Some(val) = s.get_field(*field_name) {
                            if let Some(nested_pattern) = binding {
                                self.bind_pattern(nested_pattern, val.clone(), mutable)?;
                            } else {
                                // Shorthand: let { x } = s -> binds x to s.x
                                self.env.define(*field_name, val.clone(), mutable);
                            }
                        } else {
                            return Err(EvalError::new("missing struct field"));
                        }
                    }
                    Ok(Value::Void)
                } else {
                    Err(EvalError::new("expected struct value"))
                }
            }
            BindingPattern::List { elements, rest } => {
                if let Value::List(values) = value {
                    if values.len() < elements.len() {
                        return Err(EvalError::new("list pattern too long for value"));
                    }
                    for (pat, val) in elements.iter().zip(values.iter()) {
                        self.bind_pattern(pat, val.clone(), mutable)?;
                    }
                    if let Some(rest_name) = rest {
                        let rest_values: Vec<_> = values[elements.len()..].to_vec();
                        self.env.define(*rest_name, Value::List(Rc::new(rest_values)), mutable);
                    }
                    Ok(Value::Void)
                } else {
                    Err(EvalError::new("expected list value"))
                }
            }
        }
    }

    /// Evaluate a function call.
    fn eval_call(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        match func.clone() {
            Value::Function(f) => {
                if args.len() != f.params.len() {
                    return Err(EvalError::new(format!(
                        "expected {} arguments, got {}",
                        f.params.len(), args.len()
                    )));
                }

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();  // Push a new scope for this call's locals

                // Bind captured variables
                for (name, value) in f.captures.borrow().iter() {
                    call_env.define(*name, value.clone(), false);
                }

                // Bind parameters
                for (param, arg) in f.params.iter().zip(args.iter()) {
                    call_env.define(*param, arg.clone(), false);
                }

                // Bind 'self' to the current function for recursive patterns
                let self_name = self.interner.intern("self");
                call_env.define(self_name, func, false);

                // Evaluate body in new environment
                let mut call_evaluator = Evaluator::with_env(self.interner, self.arena, call_env);
                let result = call_evaluator.eval(f.body);
                call_evaluator.env.pop_scope();  // Pop the local scope
                result
            }
            Value::Builtin(func, _name) => {
                func(&args).map_err(EvalError::new)
            }
            _ => Err(EvalError::new(format!("{} is not callable", func.type_name()))),
        }
    }

    /// Evaluate a pattern invocation.
    fn eval_pattern(&mut self, kind: PatternKind, args_id: crate::syntax::PatternArgsId) -> EvalResult {
        let args = self.arena.get_pattern_args(args_id);

        match kind {
            PatternKind::Run => {
                // run(stmt1, stmt2, ..., result)
                // Evaluate positional arguments in sequence
                let positional = self.arena.get_expr_list(args.positional);
                let mut result = Value::Void;
                for expr_id in positional {
                    result = self.eval(*expr_id)?;
                }
                Ok(result)
            }
            PatternKind::Try => {
                // try(let x = expr?, let y = expr2?, result) - evaluate in sequence
                // If any expr? propagates an error, return that error as a Value
                let positional = self.arena.get_expr_list(args.positional);
                let mut result = Value::Void;
                for expr_id in positional {
                    match self.eval(*expr_id) {
                        Ok(v) => result = v,
                        Err(e) => {
                            // If this is a propagated error, return the value
                            if let Some(propagated) = e.propagated_value {
                                return Ok(propagated);
                            }
                            // Otherwise it's a real error, propagate it
                            return Err(e);
                        }
                    }
                }
                Ok(result)
            }
            PatternKind::Map => {
                // map(.over: items, .transform: fn)
                let over = self.get_named_arg(&args.named, "over")?;
                let transform = self.get_named_arg(&args.named, "transform")?;

                let items = self.eval(over)?;
                let func = self.eval(transform)?;

                if let Value::List(list) = items {
                    let results: Result<Vec<_>, _> = list.iter()
                        .map(|item| self.eval_call(func.clone(), vec![item.clone()]))
                        .collect();
                    Ok(Value::List(Rc::new(results?)))
                } else if let Value::Range(range) = items {
                    let results: Result<Vec<_>, _> = range.iter()
                        .map(|i| self.eval_call(func.clone(), vec![Value::Int(i)]))
                        .collect();
                    Ok(Value::List(Rc::new(results?)))
                } else {
                    Err(EvalError::new("map requires a list or range"))
                }
            }
            PatternKind::Filter => {
                // filter(.over: items, .predicate: fn)
                let over = self.get_named_arg(&args.named, "over")?;
                let predicate = self.get_named_arg(&args.named, "predicate")?;

                let items = self.eval(over)?;
                let func = self.eval(predicate)?;

                if let Value::List(list) = items {
                    let mut results = Vec::new();
                    for item in list.iter() {
                        let keep = self.eval_call(func.clone(), vec![item.clone()])?;
                        if keep.is_truthy() {
                            results.push(item.clone());
                        }
                    }
                    Ok(Value::List(Rc::new(results)))
                } else {
                    Err(EvalError::new("filter requires a list"))
                }
            }
            PatternKind::Fold => {
                // fold(.over: items, .init: val, .op: fn)
                let over = self.get_named_arg(&args.named, "over")?;
                let init = self.get_named_arg(&args.named, "init")?;
                let op = self.get_named_arg(&args.named, "op")?;

                let items = self.eval(over)?;
                let mut acc = self.eval(init)?;
                let func = self.eval(op)?;

                if let Value::List(list) = items {
                    for item in list.iter() {
                        acc = self.eval_call(func.clone(), vec![acc, item.clone()])?;
                    }
                    Ok(acc)
                } else if let Value::Range(range) = items {
                    for i in range.iter() {
                        acc = self.eval_call(func.clone(), vec![acc, Value::Int(i)])?;
                    }
                    Ok(acc)
                } else {
                    Err(EvalError::new("fold requires a list or range"))
                }
            }
            PatternKind::Find => {
                // find(.over: items, .where: fn, .default?: value)
                let over = self.get_named_arg(&args.named, "over")?;
                let where_fn = self.get_named_arg(&args.named, "where")?;
                let default_arg = self.get_named_arg_opt(&args.named, "default");

                let items = self.eval(over)?;
                let func = self.eval(where_fn)?;

                if let Value::List(list) = items {
                    for item in list.iter() {
                        let matches = self.eval_call(func.clone(), vec![item.clone()])?;
                        if matches.is_truthy() {
                            // If default is provided, return T; otherwise return Some(T)
                            if default_arg.is_some() {
                                return Ok(item.clone());
                            } else {
                                return Ok(Value::Some(Box::new(item.clone())));
                            }
                        }
                    }
                    // Not found: return default or None
                    if let Some(def) = default_arg {
                        self.eval(def)
                    } else {
                        Ok(Value::None)
                    }
                } else {
                    Err(EvalError::new("find requires a list"))
                }
            }
            PatternKind::Collect => {
                // collect(.range: 0..10, .transform: fn)
                let range_arg = self.get_named_arg(&args.named, "range")?;
                let transform = self.get_named_arg(&args.named, "transform")?;

                let range = self.eval(range_arg)?;
                let func = self.eval(transform)?;

                if let Value::Range(r) = range {
                    let results: Result<Vec<_>, _> = r.iter()
                        .map(|i| self.eval_call(func.clone(), vec![Value::Int(i)]))
                        .collect();
                    Ok(Value::List(Rc::new(results?)))
                } else {
                    Err(EvalError::new("collect requires a range"))
                }
            }
            PatternKind::Match => {
                // match(value, arms...)
                // Match pattern is handled separately with arms
                let positional = self.arena.get_expr_list(args.positional);
                if positional.is_empty() {
                    return Err(EvalError::new("match requires a value"));
                }
                let value = self.eval(positional[0])?;
                // For positional match, we'd need to handle the arm parsing
                // This is simplified - the proper match uses Match ExprKind
                Ok(value)
            }
            PatternKind::Recurse => {
                // recurse(.cond: base_case, .base: val, .step: recursive_expr, .memo: bool)
                // .cond determines if we're at the base case
                // .base is returned when at base case
                // .step is evaluated otherwise (may contain self() calls)
                // .memo enables memoization (not implemented yet)
                let cond_expr = self.get_named_arg(&args.named, "cond")?;
                let base_expr = self.get_named_arg(&args.named, "base")?;
                let step_expr = self.get_named_arg(&args.named, "step")?;
                // .memo is optional, not used in this simple implementation
                let _memo = self.get_named_arg_opt(&args.named, "memo");

                // Check the base case condition
                let cond_val = self.eval(cond_expr)?;
                if cond_val.is_truthy() {
                    // Base case: return the base value
                    self.eval(base_expr)
                } else {
                    // Recursive case: evaluate the step expression
                    // Note: `self` is already bound to the current function by eval_call
                    self.eval(step_expr)
                }
            }
            _ => Err(EvalError::new(format!("pattern {:?} not yet implemented", kind))),
        }
    }

    /// Get a named argument by name.
    fn get_named_arg(&self, named: &[crate::syntax::PatternArg], name: &str) -> Result<ExprId, EvalError> {
        let target = self.interner.intern(name);
        for arg in named {
            if arg.name == target {
                return Ok(arg.value);
            }
        }
        Err(EvalError::new(format!("missing required argument: .{}", name)))
    }

    /// Get an optional named argument by name.
    fn get_named_arg_opt(&self, named: &[crate::syntax::PatternArg], name: &str) -> Option<ExprId> {
        let target = self.interner.intern(name);
        for arg in named {
            if arg.name == target {
                return Some(arg.value);
            }
        }
        None
    }

    /// Evaluate a method call.
    fn eval_method_call(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        let method_name = self.interner.lookup(method);

        match (&receiver, method_name) {
            // List methods
            (Value::List(items), "len") => Ok(Value::Int(items.len() as i64)),
            (Value::List(items), "is_empty") => Ok(Value::Bool(items.is_empty())),
            (Value::List(items), "first") => {
                Ok(items.first().cloned().map(|v| Value::Some(Box::new(v)))
                    .unwrap_or(Value::None))
            }
            (Value::List(items), "last") => {
                Ok(items.last().cloned().map(|v| Value::Some(Box::new(v)))
                    .unwrap_or(Value::None))
            }
            (Value::List(items), "contains") => {
                if args.len() != 1 {
                    return Err(EvalError::new("contains expects 1 argument"));
                }
                Ok(Value::Bool(items.contains(&args[0])))
            }

            // String methods
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Str(s), "is_empty") => Ok(Value::Bool(s.is_empty())),
            (Value::Str(s), "to_uppercase") => Ok(Value::Str(Rc::new(s.to_uppercase()))),
            (Value::Str(s), "to_lowercase") => Ok(Value::Str(Rc::new(s.to_lowercase()))),
            (Value::Str(s), "trim") => Ok(Value::Str(Rc::new(s.trim().to_string()))),
            (Value::Str(s), "contains") => {
                if args.len() != 1 {
                    return Err(EvalError::new("contains expects 1 argument"));
                }
                if let Value::Str(needle) = &args[0] {
                    Ok(Value::Bool(s.contains(needle.as_str())))
                } else {
                    Err(EvalError::new("contains expects a string argument"))
                }
            }
            (Value::Str(s), "starts_with") => {
                if args.len() != 1 {
                    return Err(EvalError::new("starts_with expects 1 argument"));
                }
                if let Value::Str(prefix) = &args[0] {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(EvalError::new("starts_with expects a string argument"))
                }
            }
            (Value::Str(s), "ends_with") => {
                if args.len() != 1 {
                    return Err(EvalError::new("ends_with expects 1 argument"));
                }
                if let Value::Str(suffix) = &args[0] {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(EvalError::new("ends_with expects a string argument"))
                }
            }

            // Range methods
            (Value::Range(r), "len") => Ok(Value::Int(r.len() as i64)),
            (Value::Range(r), "contains") => {
                if args.len() != 1 {
                    return Err(EvalError::new("contains expects 1 argument"));
                }
                if let Value::Int(n) = args[0] {
                    Ok(Value::Bool(r.contains(n)))
                } else {
                    Err(EvalError::new("contains expects an int argument"))
                }
            }

            // Option methods
            (Value::Some(v), "unwrap") => Ok((**v).clone()),
            (Value::None, "unwrap") => Err(EvalError::new("called unwrap on None")),
            (Value::Some(_), "is_some") => Ok(Value::Bool(true)),
            (Value::None, "is_some") => Ok(Value::Bool(false)),
            (Value::Some(_), "is_none") => Ok(Value::Bool(false)),
            (Value::None, "is_none") => Ok(Value::Bool(true)),
            (Value::Some(v), "unwrap_or") => Ok((**v).clone()),
            (Value::None, "unwrap_or") => {
                if args.len() != 1 {
                    return Err(EvalError::new("unwrap_or expects 1 argument"));
                }
                Ok(args[0].clone())
            }

            // Result methods
            (Value::Ok(v), "unwrap") => Ok((**v).clone()),
            (Value::Err(e), "unwrap") => Err(EvalError::new(format!("called unwrap on Err: {:?}", e))),
            (Value::Ok(_), "is_ok") => Ok(Value::Bool(true)),
            (Value::Err(_), "is_ok") => Ok(Value::Bool(false)),
            (Value::Ok(_), "is_err") => Ok(Value::Bool(false)),
            (Value::Err(_), "is_err") => Ok(Value::Bool(true)),

            _ => Err(EvalError::new(format!(
                "no method '{}' on type {}", method_name, receiver.type_name()
            ))),
        }
    }

    /// Evaluate a match expression.
    fn eval_match(&mut self, value: Value, arms: crate::syntax::ArmRange) -> EvalResult {
        let arm_list = self.arena.get_arms(arms);

        for arm in arm_list {
            // Try to match the pattern first
            if let Some(bindings) = self.try_match(&arm.pattern, &value)? {
                // Push scope with bindings
                self.env.push_scope();
                for (name, val) in bindings {
                    self.env.define(name, val, false);
                }

                // Check if guard passes (if present) - bindings are now available
                if let Some(guard) = arm.guard {
                    let guard_result = self.eval(guard)?;
                    if !guard_result.is_truthy() {
                        self.env.pop_scope();
                        continue;
                    }
                }

                // Evaluate body
                let result = self.eval(arm.body);
                self.env.pop_scope();
                return result;
            }
        }

        Err(EvalError::new("non-exhaustive match"))
    }

    /// Try to match a pattern, returning bindings if successful.
    fn try_match(&self, pattern: &crate::syntax::MatchPattern, value: &Value) -> Result<Option<Vec<(Name, Value)>>, EvalError> {
        use crate::syntax::MatchPattern;

        match pattern {
            MatchPattern::Wildcard => Ok(Some(vec![])),

            MatchPattern::Binding(name) => {
                Ok(Some(vec![(*name, value.clone())]))
            }

            MatchPattern::Literal(expr_id) => {
                let lit_val = self.arena.get(*expr_id);
                let lit = match &lit_val.kind {
                    ExprKind::Int(n) => Value::Int(*n),
                    ExprKind::Float(f) => Value::Float(*f),
                    ExprKind::Bool(b) => Value::Bool(*b),
                    ExprKind::String(s) => Value::Str(Rc::new(self.interner.lookup(*s).to_string())),
                    ExprKind::Char(c) => Value::Char(*c),
                    _ => return Err(EvalError::new("invalid literal pattern")),
                };
                if &lit == value {
                    Ok(Some(vec![]))
                } else {
                    Ok(None)
                }
            }

            MatchPattern::Variant { name, inner } => {
                let variant_name = self.interner.lookup(*name);
                match (variant_name, value, inner) {
                    ("Some", Value::Some(v), Some(inner_pat)) => {
                        self.try_match(inner_pat, v.as_ref())
                    }
                    ("Some", Value::Some(_), None) => Ok(Some(vec![])),
                    ("None", Value::None, _) => Ok(Some(vec![])),
                    ("Ok", Value::Ok(v), Some(inner_pat)) => {
                        self.try_match(inner_pat, v.as_ref())
                    }
                    ("Ok", Value::Ok(_), None) => Ok(Some(vec![])),
                    ("Err", Value::Err(v), Some(inner_pat)) => {
                        self.try_match(inner_pat, v.as_ref())
                    }
                    ("Err", Value::Err(_), None) => Ok(Some(vec![])),
                    _ => Ok(None),
                }
            }

            MatchPattern::Tuple(patterns) => {
                if let Value::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return Ok(None);
                    }
                    let mut all_bindings = Vec::new();
                    for (pat, val) in patterns.iter().zip(values.iter()) {
                        match self.try_match(pat, val)? {
                            Some(bindings) => all_bindings.extend(bindings),
                            None => return Ok(None),
                        }
                    }
                    Ok(Some(all_bindings))
                } else {
                    Ok(None)
                }
            }

            MatchPattern::List { elements, rest } => {
                if let Value::List(values) = value {
                    if values.len() < elements.len() {
                        return Ok(None);
                    }
                    if rest.is_none() && values.len() != elements.len() {
                        return Ok(None);
                    }
                    let mut all_bindings = Vec::new();
                    for (pat, val) in elements.iter().zip(values.iter()) {
                        match self.try_match(pat, val)? {
                            Some(bindings) => all_bindings.extend(bindings),
                            None => return Ok(None),
                        }
                    }
                    if let Some(rest_name) = rest {
                        let rest_values: Vec<_> = values[elements.len()..].to_vec();
                        all_bindings.push((*rest_name, Value::List(Rc::new(rest_values))));
                    }
                    Ok(Some(all_bindings))
                } else {
                    Ok(None)
                }
            }

            MatchPattern::Or(patterns) => {
                for pat in patterns {
                    if let Some(bindings) = self.try_match(pat, value)? {
                        return Ok(Some(bindings));
                    }
                }
                Ok(None)
            }

            MatchPattern::At { name, pattern } => {
                if let Some(mut bindings) = self.try_match(pattern, value)? {
                    bindings.push((*name, value.clone()));
                    Ok(Some(bindings))
                } else {
                    Ok(None)
                }
            }

            MatchPattern::Struct { fields } => {
                if let Value::Struct(s) = value {
                    let mut all_bindings = Vec::new();
                    for (field_name, inner_pat) in fields {
                        if let Some(field_val) = s.get_field(*field_name) {
                            if let Some(pat) = inner_pat {
                                match self.try_match(pat, field_val)? {
                                    Some(bindings) => all_bindings.extend(bindings),
                                    None => return Ok(None),
                                }
                            } else {
                                // Shorthand: { x } binds x to the field value
                                all_bindings.push((*field_name, field_val.clone()));
                            }
                        } else {
                            return Ok(None);
                        }
                    }
                    Ok(Some(all_bindings))
                } else {
                    Ok(None)
                }
            }

            MatchPattern::Range { start, end, inclusive } => {
                if let Value::Int(n) = value {
                    let start_val = if let Some(s) = start {
                        let expr = self.arena.get(*s);
                        if let ExprKind::Int(i) = expr.kind { i } else { return Ok(None); }
                    } else {
                        i64::MIN
                    };
                    let end_val = if let Some(e) = end {
                        let expr = self.arena.get(*e);
                        if let ExprKind::Int(i) = expr.kind { i } else { return Ok(None); }
                    } else {
                        i64::MAX
                    };

                    let in_range = if *inclusive {
                        *n >= start_val && *n <= end_val
                    } else {
                        *n >= start_val && *n < end_val
                    };

                    if in_range {
                        Ok(Some(vec![]))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Evaluate a for loop.
    fn eval_for(&mut self, binding: Name, iter: Value, guard: Option<ExprId>, body: ExprId, is_yield: bool) -> EvalResult {
        let items = match iter {
            Value::List(list) => list.iter().cloned().collect::<Vec<_>>(),
            Value::Range(range) => range.iter().map(Value::Int).collect(),
            _ => return Err(EvalError::new("for requires an iterable")),
        };

        if is_yield {
            let mut results = Vec::new();
            for item in items {
                self.env.push_scope();
                self.env.define(binding, item, false);

                // Check guard if present
                if let Some(guard_expr) = guard {
                    let guard_result = self.eval(guard_expr)?;
                    if !guard_result.is_truthy() {
                        self.env.pop_scope();
                        continue;
                    }
                }

                let result = self.eval(body)?;
                results.push(result);
                self.env.pop_scope();
            }
            Ok(Value::List(Rc::new(results)))
        } else {
            for item in items {
                self.env.push_scope();
                self.env.define(binding, item, false);

                // Check guard if present
                if let Some(guard_expr) = guard {
                    let guard_result = self.eval(guard_expr)?;
                    if !guard_result.is_truthy() {
                        self.env.pop_scope();
                        continue;
                    }
                }

                match self.eval(body) {
                    Ok(_) => {}
                    Err(e) if e.message == "continue" => {}
                    Err(e) if e.message.starts_with("break:") => {
                        self.env.pop_scope();
                        // Parse break value if present
                        let val_str = &e.message[6..];
                        if val_str == "void" {
                            return Ok(Value::Void);
                        }
                        // For simplicity, just return void
                        return Ok(Value::Void);
                    }
                    Err(e) => {
                        self.env.pop_scope();
                        return Err(e);
                    }
                }
                self.env.pop_scope();
            }
            Ok(Value::Void)
        }
    }

    /// Evaluate a loop expression.
    fn eval_loop(&mut self, body: ExprId) -> EvalResult {
        loop {
            match self.eval(body) {
                Ok(_) => {}
                Err(e) if e.message == "continue" => {}
                Err(e) if e.message.starts_with("break:") => {
                    // Parse break value
                    let val_str = &e.message[6..];
                    if val_str == "void" {
                        return Ok(Value::Void);
                    }
                    // For simplicity, just return void
                    return Ok(Value::Void);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Evaluate an assignment.
    fn eval_assign(&mut self, target: ExprId, value: Value) -> EvalResult {
        let target_expr = self.arena.get(target);
        match &target_expr.kind {
            ExprKind::Ident(name) => {
                self.env.assign(*name, value.clone()).map_err(|_| {
                    let name_str = self.interner.lookup(*name);
                    EvalError::new(format!("cannot assign to immutable variable: {}", name_str))
                })?;
                Ok(value)
            }
            ExprKind::Index { receiver, index } => {
                // Assignment to index would require mutable values
                Err(EvalError::new("index assignment not yet implemented"))
            }
            ExprKind::Field { receiver, field } => {
                // Assignment to field would require mutable structs
                Err(EvalError::new("field assignment not yet implemented"))
            }
            _ => Err(EvalError::new("invalid assignment target")),
        }
    }

    /// Convert a value to a string representation.
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::Void => "void".to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Char(c) => format!("'{}'", c),
            Value::Byte(b) => format!("0x{:02x}", b),
            Value::Str(s) => format!("\"{}\"", s),
            Value::List(items) => format!("[...]<{}>", items.len()),
            Value::Tuple(items) => format!("(...)x{}", items.len()),
            Value::Map(map) => format!("{{...}}<{}>", map.len()),
            Value::Struct(s) => format!("struct<{}>", self.interner.lookup(s.name())),
            Value::Function(_) => "function".to_string(),
            Value::Builtin(_, name) => format!("builtin:{}", name),
            Value::Range(r) => format!("{}..{}", r.start, r.end),
            Value::Some(v) => format!("Some({})", self.value_to_string(v)),
            Value::None => "None".to_string(),
            Value::Ok(v) => format!("Ok({})", self.value_to_string(v)),
            Value::Err(v) => format!("Err({})", self.value_to_string(v)),
            Value::Duration(ms) => format!("{}ms", ms),
            Value::Size(bytes) => format!("{}b", bytes),
            Value::Error(msg) => format!("error:{}", msg),
        }
    }

    /// Get a reference to the environment.
    pub fn env(&self) -> &Environment {
        &self.env
    }

    /// Get a mutable reference to the environment.
    pub fn env_mut(&mut self) -> &mut Environment {
        &mut self.env
    }

    /// Register a built-in function.
    pub fn register_builtin(&mut self, name: &str, func: super::value::BuiltinFn, display_name: &'static str) {
        let name = self.interner.intern(name);
        self.env.define_global(name, Value::Builtin(func, display_name));
    }

    /// Register all standard built-in functions.
    pub fn register_prelude(&mut self) {
        // Assertion functions
        self.register_builtin("assert", builtin_assert, "assert");
        self.register_builtin("assert_eq", builtin_assert_eq, "assert_eq");

        // Print function
        self.register_builtin("print", builtin_print, "print");

        // Length function
        self.register_builtin("len", builtin_len, "len");

        // Type conversion functions
        self.register_builtin("str", builtin_str, "str");
        self.register_builtin("int", builtin_int, "int");
        self.register_builtin("float", builtin_float, "float");

        // Comparison
        self.register_builtin("compare", builtin_compare, "compare");

        // Panic
        self.register_builtin("panic", builtin_panic, "panic");
    }
}

// Built-in function implementations

fn builtin_assert(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("assert expects 1 argument".to_string());
    }
    if args[0].is_truthy() {
        Ok(Value::Void)
    } else {
        Err("assertion failed".to_string())
    }
}

fn builtin_assert_eq(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("assert_eq expects 2 arguments".to_string());
    }
    if args[0] == args[1] {
        Ok(Value::Void)
    } else {
        Err(format!("assertion failed: {:?} != {:?}", args[0], args[1]))
    }
}

fn builtin_print(args: &[Value]) -> Result<Value, String> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 { print!(" "); }
        print!("{}", arg);
    }
    println!();
    Ok(Value::Void)
}

fn builtin_len(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("len expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
        Value::List(items) => Ok(Value::Int(items.len() as i64)),
        Value::Map(map) => Ok(Value::Int(map.len() as i64)),
        Value::Range(r) => Ok(Value::Int(r.len() as i64)),
        _ => Err(format!("len not supported for {}", args[0].type_name())),
    }
}

fn builtin_str(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("str expects 1 argument".to_string());
    }
    Ok(Value::Str(Rc::new(format!("{}", args[0]))))
}

fn builtin_int(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("int expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Str(s) => s.parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("cannot parse '{}' as int", s)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        _ => Err(format!("cannot convert {} to int", args[0].type_name())),
    }
}

fn builtin_float(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("float expects 1 argument".to_string());
    }
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Str(s) => s.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("cannot parse '{}' as float", s)),
        _ => Err(format!("cannot convert {} to float", args[0].type_name())),
    }
}

fn builtin_compare(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("compare expects 2 arguments".to_string());
    }
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => {
            if a < b { Ok(Value::Int(-1)) }
            else if a > b { Ok(Value::Int(1)) }
            else { Ok(Value::Int(0)) }
        }
        (Value::Float(a), Value::Float(b)) => {
            if a < b { Ok(Value::Int(-1)) }
            else if a > b { Ok(Value::Int(1)) }
            else { Ok(Value::Int(0)) }
        }
        (Value::Str(a), Value::Str(b)) => {
            if a < b { Ok(Value::Int(-1)) }
            else if a > b { Ok(Value::Int(1)) }
            else { Ok(Value::Int(0)) }
        }
        _ => Err(format!("cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

fn builtin_panic(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        Err("panic".to_string())
    } else {
        Err(format!("panic: {}", args[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Lexer, Parser};

    fn eval_expr(code: &str) -> EvalResult {
        let interner = StringInterner::new();
        let lexer = Lexer::new(code, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, _) = parser.parse_expression();

        let mut evaluator = Evaluator::new(&interner, &arena);
        evaluator.eval(expr_id)
    }

    #[test]
    fn test_eval_literals() {
        assert_eq!(eval_expr("42").unwrap(), Value::Int(42));
        assert_eq!(eval_expr("3.14").unwrap(), Value::Float(3.14));
        assert_eq!(eval_expr("true").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_arithmetic() {
        assert_eq!(eval_expr("1 + 2").unwrap(), Value::Int(3));
        assert_eq!(eval_expr("10 - 3").unwrap(), Value::Int(7));
        assert_eq!(eval_expr("4 * 5").unwrap(), Value::Int(20));
        assert_eq!(eval_expr("15 / 3").unwrap(), Value::Int(5));
    }

    #[test]
    fn test_eval_comparison() {
        assert_eq!(eval_expr("1 < 2").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("2 <= 2").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("3 > 2").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("1 == 1").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("1 != 2").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_logical() {
        assert_eq!(eval_expr("true && true").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("true && false").unwrap(), Value::Bool(false));
        assert_eq!(eval_expr("false || true").unwrap(), Value::Bool(true));
        assert_eq!(eval_expr("false || false").unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_eval_if() {
        assert_eq!(eval_expr("if true then 1 else 2").unwrap(), Value::Int(1));
        assert_eq!(eval_expr("if false then 1 else 2").unwrap(), Value::Int(2));
    }

    #[test]
    fn test_eval_unary() {
        assert_eq!(eval_expr("-42").unwrap(), Value::Int(-42));
        assert_eq!(eval_expr("!true").unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_eval_list() {
        let result = eval_expr("[1, 2, 3]").unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_eval_string_concat() {
        let result = eval_expr("\"hello\" + \" world\"").unwrap();
        if let Value::Str(s) = result {
            assert_eq!(s.as_str(), "hello world");
        } else {
            panic!("expected string");
        }
    }
}
