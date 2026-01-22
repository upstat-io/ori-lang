//! Expression evaluator for the Sigil interpreter.

use std::rc::Rc;
use std::cell::RefCell;
use crate::intern::{Name, StringInterner};
use crate::syntax::{ExprId, ExprKind, ExprArena, BinaryOp, UnaryOp, StmtKind, BindingPattern};
use super::value::{Value, FunctionValue, RangeValue};
use super::environment::Environment;

/// Result of evaluation.
pub type EvalResult = Result<Value, EvalError>;

/// Evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Error message.
    pub message: String,
}

impl EvalError {
    pub fn new(message: impl Into<String>) -> Self {
        EvalError { message: message.into() }
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
                let idx = self.eval(*index)?;
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

            // Default cases for unimplemented expressions
            _ => Err(EvalError::new("unimplemented expression kind")),
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
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::Lt => Ok(Value::Bool(a < b)),
                BinaryOp::Le => Ok(Value::Bool(a <= b)),
                BinaryOp::Gt => Ok(Value::Bool(a > b)),
                BinaryOp::Ge => Ok(Value::Bool(a >= b)),
                BinaryOp::BitAnd => Ok(Value::Int(a & b)),
                BinaryOp::BitOr => Ok(Value::Int(a | b)),
                BinaryOp::BitXor => Ok(Value::Int(a ^ b)),
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

            // List concatenation
            (Value::List(a), Value::List(b)) => match op {
                BinaryOp::Add => {
                    let mut result = (*a).clone();
                    result.extend((*b).iter().cloned());
                    Ok(Value::List(Rc::new(result)))
                }
                _ => Err(EvalError::new("invalid operator for lists")),
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
        match func {
            Value::Function(f) => {
                if args.len() != f.params.len() {
                    return Err(EvalError::new(format!(
                        "expected {} arguments, got {}",
                        f.params.len(), args.len()
                    )));
                }

                // Create new environment with captures
                let mut call_env = self.env.child();

                // Bind captured variables
                for (name, value) in f.captures.borrow().iter() {
                    call_env.define(*name, value.clone(), false);
                }

                // Bind parameters
                for (param, arg) in f.params.iter().zip(args.iter()) {
                    call_env.define(*param, arg.clone(), false);
                }

                // Evaluate body in new environment
                let mut call_evaluator = Evaluator::with_env(self.interner, self.arena, call_env);
                call_evaluator.eval(f.body)
            }
            _ => Err(EvalError::new(format!("{} is not callable", func.type_name()))),
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
