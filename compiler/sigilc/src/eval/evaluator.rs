//! Expression evaluator for the Sigil V3 interpreter.
//!
//! Ported from V2 with adaptations for V3's Salsa-compatible AST.

use std::path::Path;
use crate::ir::{
    Name, StringInterner, ExprId, ExprArena, SharedArena,
    ExprKind, BinaryOp, UnaryOp, StmtKind, BindingPattern,
    ArmRange, FunctionSeq, SeqBinding,
    CallArgRange,
};
use crate::parser::ParseResult;
use crate::patterns::{PatternRegistry, EvalContext, PatternExecutor};
use crate::context::{CompilerContext, SharedRegistry};
use super::value::{Value, FunctionValue, StructValue};
use super::environment::Environment;
use super::errors;
use super::operators::OperatorRegistry;
use super::methods::MethodRegistry;
use super::user_methods::{UserMethodRegistry, UserMethod};
use super::unary_operators::UnaryOperatorRegistry;

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
    /// Pattern registry for function_exp evaluation.
    registry: SharedRegistry<PatternRegistry>,
    /// Operator registry for binary operations.
    operator_registry: SharedRegistry<OperatorRegistry>,
    /// Method registry for built-in method dispatch.
    method_registry: SharedRegistry<MethodRegistry>,
    /// User-defined method registry for impl block methods.
    user_method_registry: UserMethodRegistry,
    /// Unary operator registry for unary operations.
    unary_operator_registry: SharedRegistry<UnaryOperatorRegistry>,
    /// Arena reference for imported functions.
    ///
    /// When evaluating an imported function, this holds the imported arena.
    /// Lambdas created during evaluation will inherit this arena reference.
    imported_arena: Option<SharedArena>,
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

/// Builder for creating Evaluator instances with various configurations.
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    env: Option<Environment>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    context: Option<&'a CompilerContext>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<UserMethodRegistry>,
}

impl<'a> EvaluatorBuilder<'a> {
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner, arena, env: None, registry: None, context: None,
            imported_arena: None, user_method_registry: None,
        }
    }

    pub fn env(mut self, env: Environment) -> Self { self.env = Some(env); self }
    pub fn registry(mut self, r: PatternRegistry) -> Self { self.registry = Some(SharedRegistry::new(r)); self }
    pub fn context(mut self, c: &'a CompilerContext) -> Self { self.context = Some(c); self }
    pub fn imported_arena(mut self, a: SharedArena) -> Self { self.imported_arena = Some(a); self }
    #[allow(dead_code)]
    pub fn user_method_registry(mut self, r: UserMethodRegistry) -> Self { self.user_method_registry = Some(r); self }

    pub fn build(self) -> Evaluator<'a> {
        let (pat_reg, op_reg, meth_reg, unary_reg) = if let Some(ctx) = self.context {
            (ctx.pattern_registry.clone(), ctx.operator_registry.clone(),
             ctx.method_registry.clone(), ctx.unary_operator_registry.clone())
        } else {
            (self.registry.unwrap_or_else(|| SharedRegistry::new(PatternRegistry::new())),
             SharedRegistry::new(OperatorRegistry::new()),
             SharedRegistry::new(MethodRegistry::new()),
             SharedRegistry::new(UnaryOperatorRegistry::new()))
        };
        Evaluator {
            interner: self.interner, arena: self.arena,
            env: self.env.unwrap_or_else(Environment::new),
            registry: pat_reg, operator_registry: op_reg,
            method_registry: meth_reg,
            user_method_registry: self.user_method_registry.unwrap_or_default(),
            unary_operator_registry: unary_reg,
            imported_arena: self.imported_arena,
        }
    }
}

impl<'a> Evaluator<'a> {
    /// Create a new evaluator with default registries.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        EvaluatorBuilder::new(interner, arena).build()
    }

    /// Create an evaluator with a custom compiler context.
    pub fn with_context(interner: &'a StringInterner, arena: &'a ExprArena, context: &'a CompilerContext) -> Self {
        EvaluatorBuilder::new(interner, arena).context(context).build()
    }

    /// Create an evaluator with a custom pattern registry.
    pub fn with_registry(interner: &'a StringInterner, arena: &'a ExprArena, registry: PatternRegistry) -> Self {
        EvaluatorBuilder::new(interner, arena).registry(registry).build()
    }

    /// Create an evaluator with an existing environment.
    pub fn with_env(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).build()
    }

    /// Create an evaluator with both a custom environment and pattern registry.
    pub fn with_env_and_registry(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment, registry: PatternRegistry) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).registry(registry).build()
    }

    /// Create an evaluator with a custom environment and compiler context.
    pub fn with_env_and_context(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment, context: &'a CompilerContext) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).context(context).build()
    }

    /// Create an evaluator with an imported arena context.
    pub fn with_imported_arena(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment, imported_arena: SharedArena) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).imported_arena(imported_arena).build()
    }

    /// Load a module: resolve imports and register all functions.
    ///
    /// This is the core module loading logic used by both the query system
    /// and test runner. It handles:
    /// 1. Resolving imports and registering imported functions
    /// 2. Registering all local functions
    /// 3. Registering all impl block methods
    ///
    /// After calling this, all functions from the module (and its imports)
    /// are available in the environment for evaluation.
    pub fn load_module(
        &mut self,
        parse_result: &ParseResult,
        file_path: &Path,
    ) -> Result<(), String> {
        use super::module::import;

        // First, resolve imports
        for imp in &parse_result.module.imports {
            let import_path = import::resolve_import_path(&imp.path, file_path, self.interner)
                .map_err(|e| e.message)?;

            let imported_result = import::load_imported_module(&import_path, self.interner)
                .map_err(|e| e.message)?;

            let imported_arena = SharedArena::new(imported_result.arena.clone());
            let module_functions = import::build_module_functions(&imported_result, &imported_arena);

            import::register_imports(
                imp,
                &imported_result,
                &imported_arena,
                &module_functions,
                &mut self.env,
                self.interner,
                &import_path,
                file_path,
            ).map_err(|e| e.message)?;
        }

        // Then register all local functions
        import::register_module_functions(parse_result, &mut self.env);

        // Register impl block methods
        self.register_impl_methods(&parse_result.module, &parse_result.arena);

        Ok(())
    }

    /// Register methods from impl blocks in the user method registry.
    fn register_impl_methods(&mut self, module: &crate::ir::Module, arena: &ExprArena) {
        for impl_def in &module.impls {
            // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
            let type_name = if !impl_def.self_path.is_empty() {
                // Use the last segment of the path as the type name
                self.interner.lookup(*impl_def.self_path.last().unwrap()).to_string()
            } else {
                continue; // Skip if no type path
            };

            // Register each method
            for method in &impl_def.methods {
                let method_name = self.interner.lookup(method.name).to_string();

                // Get parameter names
                let params: Vec<Name> = arena.get_params(method.params)
                    .iter()
                    .map(|p| p.name)
                    .collect();

                // Create user method with captures from current environment
                let user_method = UserMethod::with_captures(
                    params,
                    method.body,
                    self.env.capture(),
                );

                self.user_method_registry.register(type_name.clone(), method_name, user_method);
            }
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
            ExprKind::String(s) => Ok(Value::string(self.interner.lookup(*s).to_string())),
            ExprKind::Char(c) => Ok(Value::Char(*c)),
            ExprKind::Unit => Ok(Value::Void),
            ExprKind::Duration { value, unit } => Ok(Value::Duration(unit.to_millis(*value))),
            ExprKind::Size { value, unit } => Ok(Value::Size(unit.to_bytes(*value))),

            // Identifiers and references
            ExprKind::Ident(name) => self.env.lookup(*name)
                .ok_or_else(|| errors::undefined_variable(self.interner.lookup(*name))),

            // Operators
            ExprKind::Binary { left, op, right } => self.eval_binary(*left, *op, *right),
            ExprKind::Unary { op, operand } => self.eval_unary(*op, *operand),

            // Control flow
            ExprKind::If { cond, then_branch, else_branch } => {
                if self.eval(*cond)?.is_truthy() { self.eval(*then_branch) }
                else { else_branch.map(|e| self.eval(e)).transpose()?.map_or(Ok(Value::Void), Ok) }
            }

            // Collections
            ExprKind::List(range) => {
                let vals: Result<Vec<_>, _> = self.arena.get_expr_list(*range).iter().map(|id| self.eval(*id)).collect();
                Ok(Value::list(vals?))
            }
            ExprKind::Tuple(range) => {
                let vals: Result<Vec<_>, _> = self.arena.get_expr_list(*range).iter().map(|id| self.eval(*id)).collect();
                Ok(Value::tuple(vals?))
            }
            ExprKind::Range { start, end, inclusive } => super::exec::expr::eval_range(*start, *end, *inclusive, |e| self.eval(e)),

            // Access
            ExprKind::Index { receiver, index } => {
                let value = self.eval(*receiver)?;
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
                let names: Vec<Name> = self.arena.get_params(*params).iter().map(|p| p.name).collect();
                let captures = self.env.capture();
                let func = match &self.imported_arena {
                    Some(arena) => FunctionValue::from_import(names, *body, captures, arena.clone()),
                    None => FunctionValue::with_captures(names, *body, captures),
                };
                Ok(Value::Function(func))
            }

            ExprKind::Block { stmts, result } => self.eval_block(*stmts, *result),

            ExprKind::Call { func, args } => {
                let func_val = self.eval(*func)?;
                let arg_vals: Result<Vec<_>, _> = self.arena.get_expr_list(*args).iter().map(|id| self.eval(*id)).collect();
                self.eval_call(func_val, arg_vals?)
            }

            // Variant constructors
            ExprKind::Some(inner) => Ok(Value::some(self.eval(*inner)?)),
            ExprKind::None => Ok(Value::None),
            ExprKind::Ok(inner) => Ok(Value::ok(inner.map(|e| self.eval(e)).transpose()?.unwrap_or(Value::Void))),
            ExprKind::Err(inner) => Ok(Value::err(inner.map(|e| self.eval(e)).transpose()?.unwrap_or(Value::Void))),

            // Let binding
            ExprKind::Let { pattern, init, mutable, .. } => {
                let value = self.eval(*init)?;
                self.bind_pattern(pattern, value, *mutable)?;
                Ok(Value::Void)
            }

            ExprKind::FunctionSeq(seq) => self.eval_function_seq(seq),
            ExprKind::FunctionExp(exp) => self.eval_function_exp(exp),
            ExprKind::CallNamed { func, args } => {
                let func_val = self.eval(*func)?;
                self.eval_call_named(func_val, *args)
            }
            ExprKind::FunctionRef(name) => self.env.lookup(*name)
                .ok_or_else(|| errors::undefined_function(self.interner.lookup(*name))),
            ExprKind::MethodCall { receiver, method, args } => {
                let recv = self.eval(*receiver)?;
                let arg_vals: Result<Vec<_>, _> = self.arena.get_expr_list(*args).iter()
                    .map(|id| self.eval(*id)).collect();
                self.eval_method_call(recv, *method, arg_vals?)
            }
            ExprKind::Match { scrutinee, arms } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(value, *arms)
            }
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                let iter_val = self.eval(*iter)?;
                self.eval_for(*binding, iter_val, *guard, *body, *is_yield)
            }
            ExprKind::Loop { body } => self.eval_loop(*body),

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

            ExprKind::Return(v) => Err(EvalError::new(format!("return:{}", v.map(|x| self.eval(x)).transpose()?.unwrap_or(Value::Void)))),
            ExprKind::Break(v) => Err(EvalError::new(format!("break:{}", v.map(|x| self.eval(x)).transpose()?.unwrap_or(Value::Void)))),
            ExprKind::Continue => Err(EvalError::new("continue")),
            ExprKind::Assign { target, value } => {
                let val = self.eval(*value)?;
                self.eval_assign(*target, val)
            }
            ExprKind::Try(inner) => match self.eval(*inner)? {
                Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
                Value::Err(e) => Err(EvalError::propagate(Value::Err(e.clone()), format!("propagated error: {}", e))),
                Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
                other => Ok(other),
            },
            ExprKind::Config(name) => self.env.lookup(*name)
                .ok_or_else(|| errors::undefined_config(self.interner.lookup(*name))),
            ExprKind::Error => Err(errors::parse_error()),
            ExprKind::HashLength => Err(errors::hash_outside_index()),
            ExprKind::SelfRef => self.env.lookup(self.interner.intern("self")).ok_or_else(errors::self_outside_method),
            ExprKind::Await(_) => Err(errors::await_not_supported()),
        }
    }

    /// Evaluate a binary operation with short-circuit logic for && and ||.
    fn eval_binary(&mut self, left: ExprId, op: BinaryOp, right: ExprId) -> EvalResult {
        let left_val = self.eval(left)?;
        match op {
            BinaryOp::And => return Ok(Value::Bool(left_val.is_truthy() && self.eval(right)?.is_truthy())),
            BinaryOp::Or => return Ok(Value::Bool(left_val.is_truthy() || self.eval(right)?.is_truthy())),
            _ => {}
        }
        let right_val = self.eval(right)?;
        self.operator_registry.evaluate(left_val, right_val, op)
    }

    /// Evaluate a unary operation.
    fn eval_unary(&mut self, op: UnaryOp, operand: ExprId) -> EvalResult {
        let value = self.eval(operand)?;
        self.unary_operator_registry.evaluate(value, op)
    }

    /// Get the length of a collection for HashLength resolution.
    fn get_collection_length(&self, value: &Value) -> Result<i64, EvalError> {
        super::exec::expr::get_collection_length(value)
    }

    /// Evaluate an expression with # (HashLength) resolved to a specific length.
    fn eval_with_hash_length(&mut self, expr_id: ExprId, length: i64) -> EvalResult {
        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            ExprKind::HashLength => Ok(Value::Int(length)),
            ExprKind::Binary { left, op, right } => {
                let left_val = self.eval_with_hash_length(*left, length)?;
                let right_val = self.eval_with_hash_length(*right, length)?;
                super::exec::expr::eval_binary_values(left_val, *op, right_val)
            }
            _ => self.eval(expr_id),
        }
    }

    fn eval_index(&self, value: Value, index: Value) -> EvalResult {
        super::exec::expr::eval_index(value, index)
    }

    fn eval_field_access(&self, value: Value, field: Name) -> EvalResult {
        super::exec::expr::eval_field_access(value, field, self.interner)
    }

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: crate::ir::StmtRange, result: Option<ExprId>) -> EvalResult {
        self.env.push_scope();
        for stmt in self.arena.get_stmt_range(stmts) {
            match &stmt.kind {
                StmtKind::Expr(e) => { self.eval(*e)?; }
                StmtKind::Let { pattern, init, mutable, .. } => {
                    let value = self.eval(*init)?;
                    self.bind_pattern(pattern, value, *mutable)?;
                }
            }
        }
        let result_val = result.map(|r| self.eval(r)).transpose()?.unwrap_or(Value::Void);
        self.env.pop_scope();
        Ok(result_val)
    }

    /// Bind a pattern to a value using exec::control module.
    fn bind_pattern(&mut self, pattern: &BindingPattern, value: Value, mutable: bool) -> EvalResult {
        super::exec::control::bind_pattern(pattern, value, mutable, &mut self.env)
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
                // If the function has its own arena (from an import), use that arena
                // and pass it along so lambdas created during evaluation inherit it.
                // Otherwise use the current evaluator's arena.
                if let Some(func_arena) = f.arena() {
                    // Function from an imported module - use its arena and pass it along
                    let imported_arena = SharedArena::new(func_arena.clone());
                    let mut call_evaluator = Evaluator::with_imported_arena(
                        self.interner, func_arena, call_env, imported_arena
                    );
                    let result = call_evaluator.eval(f.body);
                    call_evaluator.env.pop_scope();
                    result
                } else if let Some(ref imported) = self.imported_arena {
                    // We're already in an imported context - pass it along
                    let mut call_evaluator = Evaluator::with_imported_arena(
                        self.interner, self.arena, call_env, imported.clone()
                    );
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

            FunctionSeq::ForPattern { over, map, arm, default, .. } => {
                // Evaluate the collection to iterate over
                let items = self.eval(*over)?;

                let items_list = match items {
                    Value::List(list) => list,
                    _ => return Err(EvalError::new(format!(
                        "for pattern requires a list, got {}",
                        items.type_name()
                    ))),
                };

                // Iterate and find first match
                for item in items_list.iter() {
                    // Optionally apply map function
                    let match_item = if let Some(map_expr) = map {
                        let map_fn = self.eval(*map_expr)?;
                        self.eval_call_value(map_fn, vec![item.clone()])?
                    } else {
                        item.clone()
                    };

                    // Try to match against the arm pattern
                    if let Some(bindings) = super::exec::control::try_match(
                        &arm.pattern,
                        &match_item,
                        self.arena,
                        self.interner,
                    )? {
                        // Pattern matched - bind variables and evaluate body
                        self.env.push_scope();
                        for (name, value) in bindings {
                            self.env.define(name, value, false);
                        }
                        let result = self.eval(arm.body);
                        self.env.pop_scope();
                        return result;
                    }
                }

                // No match found - return default
                self.eval(*default)
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
    /// First checks user-defined methods from impl blocks, then falls back
    /// to built-in methods in the MethodRegistry.
    fn eval_method_call(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        let method_name = self.interner.lookup(method);

        // Get the concrete type name for lookup
        let type_name = self.get_value_type_name(&receiver);

        // First, check user-defined methods
        if let Some(user_method) = self.user_method_registry.lookup(&type_name, method_name) {
            return self.eval_user_method(receiver, user_method.clone(), args);
        }

        // Fall back to built-in methods
        self.method_registry.dispatch(receiver, method_name, args)
    }

    /// Get the concrete type name for a value (for method lookup).
    ///
    /// For struct values, returns the actual struct name.
    /// For other values, returns the basic type name.
    fn get_value_type_name(&self, value: &Value) -> String {
        match value {
            Value::Struct(s) => self.interner.lookup(s.type_name).to_string(),
            Value::List(_) => "list".to_string(),
            Value::Str(_) => "str".to_string(),
            Value::Int(_) => "int".to_string(),
            Value::Float(_) => "float".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Char(_) => "char".to_string(),
            Value::Byte(_) => "byte".to_string(),
            Value::Map(_) => "map".to_string(),
            Value::Tuple(_) => "tuple".to_string(),
            Value::Some(_) | Value::None => "Option".to_string(),
            Value::Ok(_) | Value::Err(_) => "Result".to_string(),
            Value::Range(_) => "range".to_string(),
            _ => value.type_name().to_string(),
        }
    }

    /// Evaluate a user-defined method from an impl block.
    fn eval_user_method(&mut self, receiver: Value, method: UserMethod, args: Vec<Value>) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(errors::wrong_function_args(method.params.len() - 1, args.len()));
        }

        // Create new environment with captures
        let mut call_env = self.env.child();
        call_env.push_scope();

        // Bind captured variables
        for (name, value) in &method.captures {
            call_env.define(*name, value.clone(), false);
        }

        // Bind 'self' to receiver (first parameter)
        if let Some(&self_param) = method.params.first() {
            call_env.define(self_param, receiver, false);
        }

        // Bind remaining parameters
        for (param, arg) in method.params.iter().skip(1).zip(args.iter()) {
            call_env.define(*param, arg.clone(), false);
        }

        // Evaluate method body
        let result = if let Some(ref func_arena) = method.arena {
            // Method from an imported module - use its arena
            let imported_arena = SharedArena::new((**func_arena).clone());
            let mut call_evaluator = Evaluator::with_imported_arena(
                self.interner, func_arena, call_env, imported_arena
            );
            let result = call_evaluator.eval(method.body);
            call_evaluator.env.pop_scope();
            result
        } else {
            // Local method - use our arena
            let mut call_evaluator = Evaluator::with_env(self.interner, self.arena, call_env);
            let result = call_evaluator.eval(method.body);
            call_evaluator.env.pop_scope();
            result
        };

        result
    }

    /// Evaluate a match expression.
    fn eval_match(&mut self, value: Value, arms: ArmRange) -> EvalResult {
        use super::exec::control::try_match;

        let arm_list = self.arena.get_arms(arms);

        for arm in arm_list {
            // Try to match the pattern using the exec module
            if let Some(bindings) = try_match(&arm.pattern, &value, self.arena, self.interner)? {
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

    /// Evaluate a for loop using exec::control helpers.
    fn eval_for(&mut self, binding: Name, iter: Value, guard: Option<ExprId>, body: ExprId, is_yield: bool) -> EvalResult {
        use super::exec::control::{LoopAction, parse_loop_control};

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
                if let Some(g) = guard {
                    if !self.eval(g)?.is_truthy() { self.env.pop_scope(); continue; }
                }
                results.push(self.eval(body)?);
                self.env.pop_scope();
            }
            Ok(Value::list(results))
        } else {
            for item in items {
                self.env.push_scope();
                self.env.define(binding, item, false);
                if let Some(g) = guard {
                    if !self.eval(g)?.is_truthy() { self.env.pop_scope(); continue; }
                }
                match self.eval(body) {
                    Ok(_) => {}
                    Err(e) => match parse_loop_control(&e.message) {
                        LoopAction::Continue => {}
                        LoopAction::Break(v) => { self.env.pop_scope(); return Ok(v); }
                        LoopAction::Error(_) => { self.env.pop_scope(); return Err(e); }
                    }
                }
                self.env.pop_scope();
            }
            Ok(Value::Void)
        }
    }

    /// Evaluate a loop expression using exec::control helpers.
    fn eval_loop(&mut self, body: ExprId) -> EvalResult {
        use super::exec::control::{LoopAction, parse_loop_control};
        loop {
            match self.eval(body) {
                Ok(_) => {}
                Err(e) => match parse_loop_control(&e.message) {
                    LoopAction::Continue => {}
                    LoopAction::Break(v) => return Ok(v),
                    LoopAction::Error(_) => return Err(e),
                }
            }
        }
    }

    /// Evaluate an assignment using exec::control module.
    fn eval_assign(&mut self, target: ExprId, value: Value) -> EvalResult {
        super::exec::control::eval_assign(target, value, self.arena, self.interner, &mut self.env)
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
        use super::function_val::*;

        // Type conversion functions (positional args allowed per spec)
        self.register_function_val("str", function_val_str, "str");
        self.register_function_val("int", function_val_int, "int");
        self.register_function_val("float", function_val_float, "float");
        self.register_function_val("byte", function_val_byte, "byte");

        // Thread/parallel introspection (internal use)
        self.register_function_val("thread_id", function_val_thread_id, "thread_id");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Span, SharedInterner};
    use crate::ir::ast::{Expr, ExprKind};
    use std::collections::HashMap;

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
    fn test_user_method_dispatch() {
        let interner = SharedInterner::default();
        let mut arena = crate::ir::ExprArena::new();

        // Create a simple method body: self.x * 2
        // Since we can't easily build AST, we'll use a simpler expression: just return 42
        let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

        // Register a user method for type "Point"
        let self_name = interner.intern("self");
        let user_method = UserMethod::new(vec![self_name], body);

        let mut evaluator = Evaluator::new(&interner, &arena);
        evaluator.user_method_registry.register(
            "Point".to_string(),
            "get_value".to_string(),
            user_method,
        );

        // Create a struct value
        let point_name = interner.intern("Point");
        let x_name = interner.intern("x");
        let mut fields = HashMap::new();
        fields.insert(x_name, Value::Int(5));
        let point = Value::Struct(StructValue::new(point_name, fields));

        // Call the method
        let method_name = interner.intern("get_value");
        let result = evaluator.eval_method_call(point, method_name, vec![]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn test_user_method_with_self_access() {
        let interner = SharedInterner::default();
        let mut arena = crate::ir::ExprArena::new();

        // Create method body that accesses self.x: ExprKind::Field { receiver: self, field: x }
        let self_name = interner.intern("self");
        let x_name = interner.intern("x");

        // Build: self
        let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
        // Build: self.x
        let body = arena.alloc_expr(Expr::new(
            ExprKind::Field { receiver: self_ref, field: x_name },
            Span::new(0, 6),
        ));

        // Register a user method
        let user_method = UserMethod::new(vec![self_name], body);

        let mut evaluator = Evaluator::new(&interner, &arena);
        evaluator.user_method_registry.register(
            "Point".to_string(),
            "get_x".to_string(),
            user_method,
        );

        // Create a struct value with x = 7
        let point_name = interner.intern("Point");
        let mut fields = HashMap::new();
        fields.insert(x_name, Value::Int(7));
        let point = Value::Struct(StructValue::new(point_name, fields));

        // Call point.get_x()
        let method_name = interner.intern("get_x");
        let result = evaluator.eval_method_call(point, method_name, vec![]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(7));
    }

    #[test]
    fn test_user_method_with_args() {
        let interner = SharedInterner::default();
        let mut arena = crate::ir::ExprArena::new();

        // Create method body: self.x + n (where n is an argument)
        let self_name = interner.intern("self");
        let x_name = interner.intern("x");
        let n_name = interner.intern("n");

        // Build: self
        let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
        // Build: self.x
        let self_x = arena.alloc_expr(Expr::new(
            ExprKind::Field { receiver: self_ref, field: x_name },
            Span::new(0, 6),
        ));
        // Build: n
        let n_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(n_name), Span::new(7, 8)));
        // Build: self.x + n
        let body = arena.alloc_expr(Expr::new(
            ExprKind::Binary { left: self_x, op: crate::ir::BinaryOp::Add, right: n_ref },
            Span::new(0, 10),
        ));

        // Method params: [self, n]
        let user_method = UserMethod::new(vec![self_name, n_name], body);

        let mut evaluator = Evaluator::new(&interner, &arena);
        evaluator.user_method_registry.register(
            "Point".to_string(),
            "add_to_x".to_string(),
            user_method,
        );

        // Create a struct value with x = 10
        let point_name = interner.intern("Point");
        let mut fields = HashMap::new();
        fields.insert(x_name, Value::Int(10));
        let point = Value::Struct(StructValue::new(point_name, fields));

        // Call point.add_to_x(n: 5) -> should return 15
        let method_name = interner.intern("add_to_x");
        let result = evaluator.eval_method_call(point, method_name, vec![Value::Int(5)]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(15));
    }

    #[test]
    fn test_builtin_method_fallback() {
        let interner = SharedInterner::default();
        let arena = crate::ir::ExprArena::new();

        let mut evaluator = Evaluator::new(&interner, &arena);

        // Call built-in list.len() method (no user method registered)
        let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let method_name = interner.intern("len");
        let result = evaluator.eval_method_call(list, method_name, vec![]);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(3));
    }
}
