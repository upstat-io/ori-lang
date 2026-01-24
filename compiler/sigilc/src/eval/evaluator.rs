//! Expression evaluator for the Sigil V3 interpreter.
//!
//! Ported from V2 with adaptations for V3's Salsa-compatible AST.

use std::sync::Arc;
use std::hash::{Hash, Hasher};
use crate::ir::{
    Name, StringInterner, ExprId, ExprArena,
    ExprKind, BinaryOp, UnaryOp, StmtKind, BindingPattern,
    ArmRange, MatchPattern, FunctionSeq, SeqBinding,
    CallArgRange,
};
use crate::patterns::{PatternRegistry, EvalContext, PatternExecutor};
use crate::context::CompilerContext;
use super::value::{Value, FunctionValue, RangeValue, StructValue};
use super::environment::Environment;
use super::errors;
use super::operators::OperatorRegistry;
use super::methods::MethodRegistry;
use super::unary_operators::UnaryOperatorRegistry;

/// Result of evaluation.
pub type EvalResult = Result<Value, EvalError>;

// =============================================================================
// Salsa-Compatible Evaluation Output
// =============================================================================

/// Salsa-compatible representation of an evaluated value.
///
/// Unlike `Value`, this type has Clone + Eq + Hash for use in Salsa queries.
/// Complex values (functions, structs) are represented as strings.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, PartialEq, Hash, Debug
#[derive(Clone, Debug)]
pub enum EvalOutput {
    /// Integer value.
    Int(i64),
    /// Floating-point value (stored as bits for Hash).
    Float(u64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(String),
    /// Character value.
    Char(char),
    /// Byte value.
    Byte(u8),
    /// Void (unit) value.
    Void,
    /// List of values.
    List(Vec<EvalOutput>),
    /// Tuple of values.
    Tuple(Vec<EvalOutput>),
    /// Option: Some(value).
    Some(Box<EvalOutput>),
    /// Option: None.
    None,
    /// Result: Ok(value).
    Ok(Box<EvalOutput>),
    /// Result: Err(error).
    Err(Box<EvalOutput>),
    /// Duration in milliseconds.
    Duration(u64),
    /// Size in bytes.
    Size(u64),
    /// Range value.
    Range { start: i64, end: i64, inclusive: bool },
    /// Function (not directly representable, stored as description).
    Function(String),
    /// Struct (stored as description).
    Struct(String),
    /// Map (stored as key-value pairs).
    Map(Vec<(String, EvalOutput)>),
    /// Error during evaluation.
    Error(String),
}

impl EvalOutput {
    /// Convert a runtime Value to a Salsa-compatible EvalOutput.
    pub fn from_value(value: &Value, interner: &StringInterner) -> Self {
        match value {
            Value::Int(n) => EvalOutput::Int(*n),
            Value::Float(f) => EvalOutput::Float(f.to_bits()),
            Value::Bool(b) => EvalOutput::Bool(*b),
            Value::Str(s) => EvalOutput::Str(s.to_string()),
            Value::Char(c) => EvalOutput::Char(*c),
            Value::Byte(b) => EvalOutput::Byte(*b),
            Value::Void => EvalOutput::Void,
            Value::List(items) => {
                EvalOutput::List(items.iter().map(|v| Self::from_value(v, interner)).collect())
            }
            Value::Tuple(items) => {
                EvalOutput::Tuple(items.iter().map(|v| Self::from_value(v, interner)).collect())
            }
            Value::Some(v) => EvalOutput::Some(Box::new(Self::from_value(v, interner))),
            Value::None => EvalOutput::None,
            Value::Ok(v) => EvalOutput::Ok(Box::new(Self::from_value(v, interner))),
            Value::Err(v) => EvalOutput::Err(Box::new(Self::from_value(v, interner))),
            Value::Duration(ms) => EvalOutput::Duration(*ms),
            Value::Size(bytes) => EvalOutput::Size(*bytes),
            Value::Range(r) => EvalOutput::Range {
                start: r.start,
                end: r.end,
                inclusive: r.inclusive,
            },
            Value::Function(f) => {
                EvalOutput::Function(format!("<function with {} params>", f.params.len()))
            }
            Value::FunctionVal(_, name) => EvalOutput::Function(format!("<{}>", name)),
            Value::Struct(s) => {
                EvalOutput::Struct(format!("<struct {}>", interner.lookup(s.name())))
            }
            Value::Map(map) => {
                let entries: Vec<_> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::from_value(v, interner)))
                    .collect();
                EvalOutput::Map(entries)
            }
            Value::Error(msg) => EvalOutput::Error(msg.clone()),
        }
    }

    /// Get a display string for this output.
    pub fn display(&self) -> String {
        match self {
            EvalOutput::Int(n) => n.to_string(),
            EvalOutput::Float(bits) => f64::from_bits(*bits).to_string(),
            EvalOutput::Bool(b) => b.to_string(),
            EvalOutput::Str(s) => format!("\"{}\"", s),
            EvalOutput::Char(c) => format!("'{}'", c),
            EvalOutput::Byte(b) => format!("0x{:02x}", b),
            EvalOutput::Void => "void".to_string(),
            EvalOutput::List(items) => {
                let inner: Vec<_> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", inner.join(", "))
            }
            EvalOutput::Tuple(items) => {
                let inner: Vec<_> = items.iter().map(|v| v.display()).collect();
                format!("({})", inner.join(", "))
            }
            EvalOutput::Some(v) => format!("Some({})", v.display()),
            EvalOutput::None => "None".to_string(),
            EvalOutput::Ok(v) => format!("Ok({})", v.display()),
            EvalOutput::Err(v) => format!("Err({})", v.display()),
            EvalOutput::Duration(ms) => format!("{}ms", ms),
            EvalOutput::Size(bytes) => format!("{}b", bytes),
            EvalOutput::Range { start, end, inclusive } => {
                if *inclusive {
                    format!("{}..={}", start, end)
                } else {
                    format!("{}..{}", start, end)
                }
            }
            EvalOutput::Function(desc) => desc.clone(),
            EvalOutput::Struct(desc) => desc.clone(),
            EvalOutput::Map(entries) => {
                let inner: Vec<_> = entries
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.display()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            EvalOutput::Error(msg) => format!("<error: {}>", msg),
        }
    }
}

impl PartialEq for EvalOutput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EvalOutput::Int(a), EvalOutput::Int(b)) => a == b,
            (EvalOutput::Float(a), EvalOutput::Float(b)) => a == b,
            (EvalOutput::Bool(a), EvalOutput::Bool(b)) => a == b,
            (EvalOutput::Str(a), EvalOutput::Str(b)) => a == b,
            (EvalOutput::Char(a), EvalOutput::Char(b)) => a == b,
            (EvalOutput::Byte(a), EvalOutput::Byte(b)) => a == b,
            (EvalOutput::Void, EvalOutput::Void) => true,
            (EvalOutput::List(a), EvalOutput::List(b)) => a == b,
            (EvalOutput::Tuple(a), EvalOutput::Tuple(b)) => a == b,
            (EvalOutput::Some(a), EvalOutput::Some(b)) => a == b,
            (EvalOutput::None, EvalOutput::None) => true,
            (EvalOutput::Ok(a), EvalOutput::Ok(b)) => a == b,
            (EvalOutput::Err(a), EvalOutput::Err(b)) => a == b,
            (EvalOutput::Duration(a), EvalOutput::Duration(b)) => a == b,
            (EvalOutput::Size(a), EvalOutput::Size(b)) => a == b,
            (EvalOutput::Range { start: s1, end: e1, inclusive: i1 },
             EvalOutput::Range { start: s2, end: e2, inclusive: i2 }) => {
                s1 == s2 && e1 == e2 && i1 == i2
            }
            (EvalOutput::Function(a), EvalOutput::Function(b)) => a == b,
            (EvalOutput::Struct(a), EvalOutput::Struct(b)) => a == b,
            (EvalOutput::Map(a), EvalOutput::Map(b)) => a == b,
            (EvalOutput::Error(a), EvalOutput::Error(b)) => a == b,
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
            EvalOutput::Float(bits) => bits.hash(state),
            EvalOutput::Bool(b) => b.hash(state),
            EvalOutput::Str(s) => s.hash(state),
            EvalOutput::Char(c) => c.hash(state),
            EvalOutput::Byte(b) => b.hash(state),
            EvalOutput::Void => {}
            EvalOutput::List(items) => items.hash(state),
            EvalOutput::Tuple(items) => items.hash(state),
            EvalOutput::Some(v) => v.hash(state),
            EvalOutput::None => {}
            EvalOutput::Ok(v) => v.hash(state),
            EvalOutput::Err(v) => v.hash(state),
            EvalOutput::Duration(ms) => ms.hash(state),
            EvalOutput::Size(bytes) => bytes.hash(state),
            EvalOutput::Range { start, end, inclusive } => {
                start.hash(state);
                end.hash(state);
                inclusive.hash(state);
            }
            EvalOutput::Function(desc) => desc.hash(state),
            EvalOutput::Struct(desc) => desc.hash(state),
            EvalOutput::Map(entries) => entries.hash(state),
            EvalOutput::Error(msg) => msg.hash(state),
        }
    }
}

/// Result of evaluating a module.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, PartialEq, Hash, Debug
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleEvalResult {
    /// The result value (if evaluation succeeded).
    pub result: Option<EvalOutput>,
    /// Error message (if evaluation failed).
    pub error: Option<String>,
    /// Captured stdout output (if any).
    pub stdout: String,
}

impl ModuleEvalResult {
    /// Create a successful result.
    pub fn success(result: EvalOutput) -> Self {
        ModuleEvalResult {
            result: Some(result),
            error: None,
            stdout: String::new(),
        }
    }

    /// Create an error result.
    pub fn failure(error: String) -> Self {
        ModuleEvalResult {
            result: None,
            error: Some(error),
            stdout: String::new(),
        }
    }

    /// Check if evaluation succeeded.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if evaluation failed.
    pub fn is_failure(&self) -> bool {
        self.error.is_some()
    }
}

impl Default for ModuleEvalResult {
    fn default() -> Self {
        ModuleEvalResult {
            result: Some(EvalOutput::Void),
            error: None,
            stdout: String::new(),
        }
    }
}

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
    /// Pattern registry for function_exp evaluation.
    registry: Arc<PatternRegistry>,
    /// Operator registry for binary operations.
    operator_registry: Arc<OperatorRegistry>,
    /// Method registry for method dispatch.
    method_registry: Arc<MethodRegistry>,
    /// Unary operator registry for unary operations.
    unary_operator_registry: Arc<UnaryOperatorRegistry>,
}

/// Implement PatternExecutor for Evaluator to enable pattern evaluation.
///
/// This allows patterns to request expression evaluation and function calls
/// without needing direct access to the evaluator's internals.
impl<'a> PatternExecutor for Evaluator<'a> {
    fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        Evaluator::eval(self, expr_id)
    }

    fn call(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, args)
    }
}

impl<'a> Evaluator<'a> {
    /// Create a new evaluator with default registries.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Evaluator {
            interner,
            arena,
            env: Environment::new(),
            registry: Arc::new(PatternRegistry::new()),
            operator_registry: Arc::new(OperatorRegistry::new()),
            method_registry: Arc::new(MethodRegistry::new()),
            unary_operator_registry: Arc::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create an evaluator with a custom compiler context.
    ///
    /// This enables dependency injection for testing with mock registries.
    pub fn with_context(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        context: &CompilerContext,
    ) -> Self {
        Evaluator {
            interner,
            arena,
            env: Environment::new(),
            registry: context.pattern_registry.clone(),
            operator_registry: context.operator_registry.clone(),
            method_registry: context.method_registry.clone(),
            unary_operator_registry: context.unary_operator_registry.clone(),
        }
    }

    /// Create an evaluator with a custom pattern registry.
    ///
    /// This enables dependency injection for testing with mock patterns.
    pub fn with_registry(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        registry: PatternRegistry,
    ) -> Self {
        Evaluator {
            interner,
            arena,
            env: Environment::new(),
            registry: Arc::new(registry),
            operator_registry: Arc::new(OperatorRegistry::new()),
            method_registry: Arc::new(MethodRegistry::new()),
            unary_operator_registry: Arc::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create an evaluator with an existing environment.
    pub fn with_env(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment) -> Self {
        Evaluator {
            interner,
            arena,
            env,
            registry: Arc::new(PatternRegistry::new()),
            operator_registry: Arc::new(OperatorRegistry::new()),
            method_registry: Arc::new(MethodRegistry::new()),
            unary_operator_registry: Arc::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create an evaluator with both a custom environment and pattern registry.
    pub fn with_env_and_registry(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        env: Environment,
        registry: PatternRegistry,
    ) -> Self {
        Evaluator {
            interner,
            arena,
            env,
            registry: Arc::new(registry),
            operator_registry: Arc::new(OperatorRegistry::new()),
            method_registry: Arc::new(MethodRegistry::new()),
            unary_operator_registry: Arc::new(UnaryOperatorRegistry::new()),
        }
    }

    /// Create an evaluator with a custom environment and compiler context.
    pub fn with_env_and_context(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        env: Environment,
        context: &CompilerContext,
    ) -> Self {
        Evaluator {
            interner,
            arena,
            env,
            registry: context.pattern_registry.clone(),
            operator_registry: context.operator_registry.clone(),
            method_registry: context.method_registry.clone(),
            unary_operator_registry: context.unary_operator_registry.clone(),
        }
    }

    /// Evaluate an expression.
    pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            // Literals
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(bits) => Ok(Value::Float(f64::from_bits(*bits))),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::String(s) => {
                let string = self.interner.lookup(*s).to_string();
                Ok(Value::string(string))
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
                    errors::undefined_variable(name_str)
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
                Ok(Value::list(values?))
            }

            ExprKind::Tuple(range) => {
                let items = self.arena.get_expr_list(*range);
                let values: Result<Vec<_>, _> = items.iter()
                    .map(|id| self.eval(*id))
                    .collect();
                Ok(Value::tuple(values?))
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

                // Capture the current environment (frozen at creation)
                let captures = self.env.capture();

                Ok(Value::Function(FunctionValue::with_captures(
                    param_names,
                    *body,
                    captures,
                )))
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
                Ok(Value::some(value))
            }
            ExprKind::None => Ok(Value::None),
            ExprKind::Ok(inner) => {
                let value = if let Some(e) = inner {
                    self.eval(*e)?
                } else {
                    Value::Void
                };
                Ok(Value::ok(value))
            }
            ExprKind::Err(inner) => {
                let value = if let Some(e) = inner {
                    self.eval(*e)?
                } else {
                    Value::Void
                };
                Ok(Value::err(value))
            }

            // Let binding (in expression context)
            ExprKind::Let { pattern, init, mutable, .. } => {
                let value = self.eval(*init)?;
                self.bind_pattern(pattern, value, *mutable)?;
                Ok(Value::Void)
            }

            // FunctionSeq: run, try, match
            ExprKind::FunctionSeq(func_seq) => {
                self.eval_function_seq(func_seq)
            }

            // FunctionExp: map, filter, fold, etc.
            ExprKind::FunctionExp(func_exp) => {
                self.eval_function_exp(func_exp)
            }

            // Function call with named arguments
            ExprKind::CallNamed { func, args } => {
                let func_val = self.eval(*func)?;
                self.eval_call_named(func_val, *args)
            }

            // Function reference
            ExprKind::FunctionRef(name) => {
                self.env.lookup(*name).ok_or_else(|| {
                    let name_str = self.interner.lookup(*name);
                    errors::undefined_function(name_str)
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
                Ok(Value::map(map))
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
                Ok(Value::Struct(StructValue::new(*name, field_values)))
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
                    Value::Ok(v) => Ok((*v).clone()),
                    Value::Err(e) => Err(EvalError::propagate(
                        Value::Err(e.clone()),
                        format!("propagated error: {}", self.value_to_string(&e))
                    )),
                    Value::Some(v) => Ok((*v).clone()),
                    Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
                    other => Ok(other),
                }
            }

            // Config reference
            ExprKind::Config(name) => {
                self.env.lookup(*name).ok_or_else(|| {
                    let name_str = self.interner.lookup(*name);
                    errors::undefined_config(name_str)
                })
            }

            // Error placeholder
            ExprKind::Error => Err(errors::parse_error()),

            // Hash length (in index context)
            ExprKind::HashLength => Err(errors::hash_outside_index()),

            // Self reference
            ExprKind::SelfRef => {
                // Look for "self" in the environment
                let self_name = self.interner.intern("self");
                self.env.lookup(self_name).ok_or_else(errors::self_outside_method)
            }

            // Await (not supported in interpreter)
            ExprKind::Await(_) => Err(errors::await_not_supported()),
        }
    }

    /// Evaluate a binary operation.
    ///
    /// Delegates to the OperatorRegistry for most operations.
    /// Short-circuit logic for && and || is handled here since it requires
    /// lazy evaluation of the right operand.
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

        // Delegate to operator registry
        self.operator_registry.evaluate(left_val, right_val, op)
    }

    /// Evaluate a unary operation.
    ///
    /// Delegates to the UnaryOperatorRegistry for all operations.
    fn eval_unary(&mut self, op: UnaryOp, operand: ExprId) -> EvalResult {
        let value = self.eval(operand)?;
        self.unary_operator_registry.evaluate(value, op)
    }

    /// Get the length of a collection for HashLength resolution.
    fn get_collection_length(&self, value: &Value) -> Result<i64, EvalError> {
        match value {
            Value::List(items) => Ok(items.len() as i64),
            Value::Str(s) => Ok(s.chars().count() as i64),
            Value::Map(map) => Ok(map.len() as i64),
            Value::Tuple(items) => Ok(items.len() as i64),
            _ => Err(errors::cannot_get_length(value.type_name())),
        }
    }

    /// Evaluate an expression with # (HashLength) resolved to a specific length.
    fn eval_with_hash_length(&mut self, expr_id: ExprId, length: i64) -> EvalResult {
        let expr = self.arena.get_expr(expr_id);
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
    fn eval_binary_values(&self, left_val: Value, op: BinaryOp, right_val: Value) -> EvalResult {
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
                items.get(idx).cloned().ok_or_else(|| errors::index_out_of_bounds(i))
            }
            (Value::Str(s), Value::Int(i)) => {
                let idx = if i < 0 {
                    (s.len() as i64 + i) as usize
                } else {
                    i as usize
                };
                s.chars().nth(idx)
                    .map(Value::Char)
                    .ok_or_else(|| errors::index_out_of_bounds(i))
            }
            (Value::Map(map), Value::Str(key)) => {
                map.get(key.as_str()).cloned()
                    .ok_or_else(|| errors::key_not_found(&key))
            }
            (value, index) => Err(errors::cannot_index(value.type_name(), index.type_name())),
        }
    }

    /// Evaluate field access.
    fn eval_field_access(&self, value: Value, field: Name) -> EvalResult {
        match value {
            Value::Struct(s) => {
                s.get_field(field).cloned().ok_or_else(|| {
                    let field_name = self.interner.lookup(field);
                    errors::no_field_on_struct(field_name)
                })
            }
            Value::Tuple(items) => {
                // Tuple field access like t.0, t.1
                let field_name = self.interner.lookup(field);
                if let Ok(idx) = field_name.parse::<usize>() {
                    items.get(idx).cloned().ok_or_else(|| errors::tuple_index_out_of_bounds(idx))
                } else {
                    Err(errors::invalid_tuple_field(field_name))
                }
            }
            value => Err(errors::cannot_access_field(value.type_name())),
        }
    }

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: crate::ir::StmtRange, result: Option<ExprId>) -> EvalResult {
        self.env.push_scope();

        let stmt_list = self.arena.get_stmt_range(stmts);

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
                        return Err(errors::tuple_pattern_mismatch());
                    }
                    for (pat, val) in patterns.iter().zip(values.iter()) {
                        self.bind_pattern(pat, val.clone(), mutable)?;
                    }
                    Ok(Value::Void)
                } else {
                    Err(errors::expected_tuple())
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
                            return Err(errors::missing_struct_field());
                        }
                    }
                    Ok(Value::Void)
                } else {
                    Err(errors::expected_struct())
                }
            }
            BindingPattern::List { elements, rest } => {
                if let Value::List(values) = value {
                    if values.len() < elements.len() {
                        return Err(errors::list_pattern_too_long());
                    }
                    for (pat, val) in elements.iter().zip(values.iter()) {
                        self.bind_pattern(pat, val.clone(), mutable)?;
                    }
                    if let Some(rest_name) = rest {
                        let rest_values: Vec<_> = values[elements.len()..].to_vec();
                        self.env.define(*rest_name, Value::list(rest_values), mutable);
                    }
                    Ok(Value::Void)
                } else {
                    Err(errors::expected_list())
                }
            }
        }
    }

    /// Evaluate a function call.
    fn eval_call(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        match func.clone() {
            Value::Function(f) => {
                if args.len() != f.params.len() {
                    return Err(errors::wrong_function_args(f.params.len(), args.len()));
                }

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();  // Push a new scope for this call's locals

                // Bind captured variables (immutable captures via iterator)
                for (name, value) in f.captures() {
                    call_env.define(*name, value.clone(), false);
                }

                // Bind parameters
                for (param, arg) in f.params.iter().zip(args.iter()) {
                    call_env.define(*param, arg.clone(), false);
                }

                // Bind 'self' to the current function for recursive patterns
                let self_name = self.interner.intern("self");
                call_env.define(self_name, func, false);

                // Evaluate body in new environment.
                // If the function has its own arena (from an import), use that arena.
                // Otherwise use the current evaluator's arena.
                if let Some(func_arena) = f.arena() {
                    // Function from an imported module - use its arena
                    let mut call_evaluator = Evaluator::with_env(self.interner, func_arena, call_env);
                    let result = call_evaluator.eval(f.body);
                    call_evaluator.env.pop_scope();
                    result
                } else {
                    // Local function - use our arena
                    let mut call_evaluator = Evaluator::with_env(self.interner, self.arena, call_env);
                    let result = call_evaluator.eval(f.body);
                    call_evaluator.env.pop_scope();
                    result
                }
            }
            Value::FunctionVal(func, _name) => {
                func(&args).map_err(EvalError::new)
            }
            _ => Err(errors::not_callable(func.type_name())),
        }
    }

    /// Evaluate a function_seq expression (run, try, match).
    fn eval_function_seq(&mut self, func_seq: &FunctionSeq) -> EvalResult {
        match func_seq {
            FunctionSeq::Run { bindings, result, .. } => {
                // Evaluate bindings and statements in sequence
                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, mutable, .. } => {
                            let val = self.eval(*value)?;
                            self.bind_pattern(pattern, val, *mutable)?;
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Evaluate for side effects (e.g., assignment)
                            self.eval(*expr)?;
                        }
                    }
                }
                // Evaluate and return result
                self.eval(*result)
            }

            FunctionSeq::Try { bindings, result, .. } => {
                // Evaluate bindings, unwrapping Result/Option and short-circuiting on error
                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, mutable, .. } => {
                            match self.eval(*value) {
                                Ok(value) => {
                                    // Unwrap Result/Option types per spec:
                                    // "If any binding expression returns a Result<T, E>, the binding variable has type T"
                                    let unwrapped = match value {
                                        Value::Ok(inner) => (*inner).clone(),
                                        Value::Err(e) => {
                                            // Early return with the error
                                            return Ok(Value::Err(e));
                                        }
                                        Value::Some(inner) => (*inner).clone(),
                                        Value::None => {
                                            // Early return with None
                                            return Ok(Value::None);
                                        }
                                        other => other,
                                    };
                                    self.bind_pattern(pattern, unwrapped, *mutable)?;
                                }
                                Err(e) => {
                                    // If this is a propagated error, return the value
                                    if let Some(propagated) = e.propagated_value {
                                        return Ok(propagated);
                                    }
                                    return Err(e);
                                }
                            }
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Evaluate for side effects
                            self.eval(*expr)?;
                        }
                    }
                }
                // Evaluate and return result
                self.eval(*result)
            }

            FunctionSeq::Match { scrutinee, arms, .. } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(value, *arms)
            }
        }
    }

    /// Evaluate a function_exp expression (map, filter, fold, etc.).
    ///
    /// Uses the pattern registry for Open/Closed principle compliance.
    /// Each pattern implementation is in a separate file under `patterns/`.
    fn eval_function_exp(&mut self, func_exp: &crate::ir::FunctionExp) -> EvalResult {
        let props = self.arena.get_named_exprs(func_exp.props);

        // Look up pattern definition from registry
        let pattern = self.registry.get(func_exp.kind)
            .ok_or_else(|| EvalError::new(format!(
                "unknown pattern: {:?}",
                func_exp.kind
            )))?;

        // Create evaluation context
        let ctx = EvalContext::new(self.interner, self.arena, props);

        // Evaluate via the pattern definition
        // Pass self as the executor which implements PatternExecutor
        pattern.evaluate(&ctx, self)
    }

    /// Evaluate a function call with named arguments.
    fn eval_call_named(&mut self, func: Value, args: CallArgRange) -> EvalResult {
        let call_args = self.arena.get_call_args(args);
        let arg_values: Result<Vec<_>, _> = call_args.iter()
            .map(|arg| self.eval(arg.value))
            .collect();
        self.eval_call(func, arg_values?)
    }

    // Note: get_prop and get_prop_opt methods were removed as part of the
    // pattern system refactoring. Patterns now use EvalContext directly.

    /// Evaluate a method call.
    ///
    /// Delegates to the MethodRegistry for dispatch.
    fn eval_method_call(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        let method_name = self.interner.lookup(method);
        self.method_registry.dispatch(receiver, method_name, args)
    }

    /// Evaluate a match expression.
    fn eval_match(&mut self, value: Value, arms: ArmRange) -> EvalResult {
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

        Err(errors::non_exhaustive_match())
    }

    /// Try to match a pattern, returning bindings if successful.
    fn try_match(&self, pattern: &MatchPattern, value: &Value) -> Result<Option<Vec<(Name, Value)>>, EvalError> {
        match pattern {
            MatchPattern::Wildcard => Ok(Some(vec![])),

            MatchPattern::Binding(name) => {
                Ok(Some(vec![(*name, value.clone())]))
            }

            MatchPattern::Literal(expr_id) => {
                let lit_val = self.arena.get_expr(*expr_id);
                let lit = match &lit_val.kind {
                    ExprKind::Int(n) => Value::Int(*n),
                    ExprKind::Float(bits) => Value::Float(f64::from_bits(*bits)),
                    ExprKind::Bool(b) => Value::Bool(*b),
                    ExprKind::String(s) => Value::string(self.interner.lookup(*s).to_string()),
                    ExprKind::Char(c) => Value::Char(*c),
                    _ => return Err(errors::invalid_literal_pattern()),
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
                        all_bindings.push((*rest_name, Value::list(rest_values)));
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
                        let expr = self.arena.get_expr(*s);
                        if let ExprKind::Int(i) = expr.kind { i } else { return Ok(None); }
                    } else {
                        i64::MIN
                    };
                    let end_val = if let Some(e) = end {
                        let expr = self.arena.get_expr(*e);
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
            _ => return Err(errors::for_requires_iterable()),
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
            Ok(Value::list(results))
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
        let target_expr = self.arena.get_expr(target);
        match &target_expr.kind {
            ExprKind::Ident(name) => {
                self.env.assign(*name, value.clone()).map_err(|_| {
                    let name_str = self.interner.lookup(*name);
                    errors::cannot_assign_immutable(name_str)
                })?;
                Ok(value)
            }
            ExprKind::Index { .. } => {
                // Assignment to index would require mutable values
                Err(EvalError::new("index assignment not yet implemented"))
            }
            ExprKind::Field { .. } => {
                // Assignment to field would require mutable structs
                Err(EvalError::new("field assignment not yet implemented"))
            }
            _ => Err(errors::invalid_assignment_target()),
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
            Value::FunctionVal(_, name) => format!("{}()", name),
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

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, args)
    }

    /// Register a function_val (type conversion function).
    pub fn register_function_val(&mut self, name: &str, func: super::value::FunctionValFn, display_name: &'static str) {
        let name = self.interner.intern(name);
        self.env.define_global(name, Value::FunctionVal(func, display_name));
    }

    /// Register all function_val (type conversion) functions.
    ///
    /// function_val: Type conversion functions like int(x), str(x), float(x)
    /// that allow positional arguments per the spec.
    pub fn register_prelude(&mut self) {
        // Type conversion functions (positional args allowed per spec)
        self.register_function_val("str", function_val_str, "str");
        self.register_function_val("int", function_val_int, "int");
        self.register_function_val("float", function_val_float, "float");

        // Thread/parallel introspection (internal use)
        self.register_function_val("thread_id", function_val_thread_id, "thread_id");
    }
}

// function_val implementations (type conversion functions)

fn function_val_str(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("str expects 1 argument".to_string());
    }
    Ok(Value::string(format!("{}", args[0])))
}

fn function_val_int(args: &[Value]) -> Result<Value, String> {
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

fn function_val_float(args: &[Value]) -> Result<Value, String> {
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

/// Returns the current OS thread ID as an integer.
/// Useful for verifying parallel execution.
fn function_val_thread_id(_args: &[Value]) -> Result<Value, String> {
    // Get the current thread ID and convert to a stable integer
    let thread_id = std::thread::current().id();
    // ThreadId doesn't have a direct to_u64, so we use Debug format and parse
    // Format is "ThreadId(N)" where N is the ID number
    let id_str = format!("{:?}", thread_id);
    let id_num = id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<i64>()
        .unwrap_or(0);
    Ok(Value::Int(id_num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_error() {
        let err = EvalError::new("test error");
        assert_eq!(err.message, "test error");
        assert!(err.propagated_value.is_none());
    }

    #[test]
    fn test_eval_error_propagate() {
        let err = EvalError::propagate(Value::None, "propagated");
        assert_eq!(err.message, "propagated");
        assert!(err.propagated_value.is_some());
    }

    #[test]
    fn test_function_val_str() {
        assert_eq!(
            function_val_str(&[Value::Int(42)]).unwrap(),
            Value::string("42")
        );
    }

    #[test]
    fn test_function_val_int() {
        assert_eq!(function_val_int(&[Value::Float(3.7)]).unwrap(), Value::Int(3));
        assert_eq!(function_val_int(&[Value::Bool(true)]).unwrap(), Value::Int(1));
        assert_eq!(
            function_val_int(&[Value::string("42")]).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_function_val_float() {
        assert_eq!(function_val_float(&[Value::Int(3)]).unwrap(), Value::Float(3.0));
    }

    #[test]
    fn test_eval_output_from_value() {
        let interner = crate::ir::SharedInterner::default();

        // Test various value conversions
        assert_eq!(
            EvalOutput::from_value(&Value::Int(42), &interner),
            EvalOutput::Int(42)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Bool(true), &interner),
            EvalOutput::Bool(true)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Void, &interner),
            EvalOutput::Void
        );
        assert_eq!(
            EvalOutput::from_value(&Value::None, &interner),
            EvalOutput::None
        );
    }

    #[test]
    fn test_eval_output_display() {
        assert_eq!(EvalOutput::Int(42).display(), "42");
        assert_eq!(EvalOutput::Bool(true).display(), "true");
        assert_eq!(EvalOutput::Void.display(), "void");
        assert_eq!(EvalOutput::None.display(), "None");
        assert_eq!(EvalOutput::Some(Box::new(EvalOutput::Int(1))).display(), "Some(1)");
        assert_eq!(EvalOutput::Ok(Box::new(EvalOutput::Int(1))).display(), "Ok(1)");
        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1), EvalOutput::Int(2)]).display(),
            "[1, 2]"
        );
        assert_eq!(
            EvalOutput::Tuple(vec![EvalOutput::Int(1), EvalOutput::Bool(true)]).display(),
            "(1, true)"
        );
    }

    #[test]
    fn test_eval_output_equality() {
        assert_eq!(EvalOutput::Int(42), EvalOutput::Int(42));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Int(43));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Bool(true));

        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1)]),
            EvalOutput::List(vec![EvalOutput::Int(1)])
        );
    }

    #[test]
    fn test_module_eval_result() {
        let success = ModuleEvalResult::success(EvalOutput::Int(42));
        assert!(success.is_success());
        assert!(!success.is_failure());

        let failure = ModuleEvalResult::failure("test error".to_string());
        assert!(!failure.is_success());
        assert!(failure.is_failure());
    }
}
