//! Expression evaluator for the Sigil interpreter.
//!
//! Provides tree-walking evaluation using the Salsa-compatible AST.
//!
//! # Arena Threading Pattern
//!
//! Functions and methods in Sigil carry their own expression arena (`SharedArena`)
//! for thread safety. When evaluating a function or method call, we must use the
//! callee's arena rather than the caller's, because:
//!
//! 1. **Thread Safety**: In parallel evaluation, different threads may evaluate
//!    different functions simultaneously. Each function's arena contains only its
//!    own expression nodes, avoiding shared mutable state.
//!
//! 2. **Expression IDs**: Each `ExprId` is valid only within its originating arena.
//!    A function's body expression ID references nodes in that function's arena.
//!
//! 3. **Lambda Capture**: When a lambda is created, it captures a reference to the
//!    current arena (via `imported_arena`). When the lambda is later called, we use
//!    that captured arena to evaluate its body.
//!
//! The pattern appears in three places:
//! - `function_call.rs`: Regular function calls
//! - `method_dispatch.rs`: User-defined method calls
//! - Lambda evaluation inherits from the arena captured at creation time
//!
//! Use `create_function_evaluator()` to correctly set up evaluation context
//! with the callee's arena.

mod builder;
mod module_loading;
mod function_call;
mod method_dispatch;
mod derived_methods;
mod function_seq;
mod scope_guard;
pub mod resolvers;

pub use builder::EvaluatorBuilder;

use crate::ir::{
    Name, StringInterner, ExprId, ExprArena, SharedArena,
    ExprKind, BinaryOp, UnaryOp, StmtKind, BindingPattern,
    ArmRange,
};
use sigil_patterns::{PatternRegistry, EvalContext, PatternExecutor, EvalError, EvalResult};
use crate::context::{CompilerContext, SharedRegistry};
use crate::stack::ensure_sufficient_stack;
use super::value::{Value, FunctionValue, StructValue};
use sigil_eval::{
    Environment, MethodRegistry, OperatorRegistry, UnaryOperatorRegistry, UserMethodRegistry,
    // Error factories
    await_not_supported, for_requires_iterable, hash_outside_index,
    map_keys_must_be_strings, non_exhaustive_match, parse_error, self_outside_method,
    undefined_config, undefined_function, undefined_variable, unknown_pattern,
};
use crate::context::SharedMutableRegistry;

/// Tree-walking evaluator for Sigil expressions.
pub struct Evaluator<'a> {
    /// String interner for name lookup.
    pub(super) interner: &'a StringInterner,
    /// Expression arena.
    pub(super) arena: &'a ExprArena,
    /// Current environment.
    pub(super) env: Environment,
    /// Pattern registry for `function_exp` evaluation.
    pub(super) registry: SharedRegistry<PatternRegistry>,
    /// Operator registry for binary operations.
    pub(super) operator_registry: SharedRegistry<OperatorRegistry>,
    /// Method registry for built-in method dispatch.
    pub(super) method_registry: SharedRegistry<MethodRegistry>,
    /// User-defined method registry for impl block methods.
    ///
    /// Uses `SharedMutableRegistry` to allow method registration after the
    /// evaluator (and its cached dispatcher) is created.
    pub(super) user_method_registry: SharedMutableRegistry<UserMethodRegistry>,
    /// Unary operator registry for unary operations.
    pub(super) unary_operator_registry: SharedRegistry<UnaryOperatorRegistry>,
    /// Cached method dispatcher for efficient method resolution.
    ///
    /// The dispatcher chains all method resolvers (user, derived, collection, builtin)
    /// and is built once during construction. Because `user_method_registry` uses
    /// interior mutability, the dispatcher sees method registrations made after creation.
    pub(super) method_dispatcher: resolvers::MethodDispatcher,
    /// Arena reference for imported functions.
    ///
    /// When evaluating an imported function, this holds the imported arena.
    /// Lambdas created during evaluation will inherit this arena reference.
    pub(super) imported_arena: Option<SharedArena>,
    /// Whether the prelude has been auto-loaded.
    pub(super) prelude_loaded: bool,
}

/// Implement `PatternExecutor` for Evaluator to enable pattern evaluation.
///
/// This allows patterns to request expression evaluation and function calls
/// without needing direct access to the evaluator's internals.
impl PatternExecutor for Evaluator<'_> {
    fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        Evaluator::eval(self, expr_id)
    }

    fn call(&mut self, func: Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, &args)
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
    pub fn with_env(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment, user_methods: SharedMutableRegistry<UserMethodRegistry>) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).user_method_registry(user_methods).build()
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
    pub fn with_imported_arena(interner: &'a StringInterner, arena: &'a ExprArena, env: Environment, imported_arena: SharedArena, user_methods: SharedMutableRegistry<UserMethodRegistry>) -> Self {
        EvaluatorBuilder::new(interner, arena).env(env).imported_arena(imported_arena).user_method_registry(user_methods).build()
    }

    /// Evaluate an expression.
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_inner(expr_id))
    }

    /// Inner evaluation logic (wrapped by `eval` for stack safety).
    fn eval_inner(&mut self, expr_id: ExprId) -> EvalResult {
        // Debug: Check arena bounds before access
        if expr_id.index() >= self.arena.expr_count() {
            // Try to get more context about what we were evaluating
            let thread_id = std::thread::current().id();
            let has_imported = self.imported_arena.is_some();
            let arena_ptr = self.arena as *const _;
            let imported_ptr = self.imported_arena.as_ref().map(|a| &**a as *const _);

            // Get backtrace for more context
            let bt = std::backtrace::Backtrace::force_capture();

            panic!(
                "ExprId {} out of bounds (arena has {} expressions). \n\
                 Thread: {:?}\n\
                 has_imported_arena: {}\n\
                 arena_ptr: {:?}\n\
                 imported_ptr: {:?}\n\
                 Backtrace:\n{}",
                expr_id.index(),
                self.arena.expr_count(),
                thread_id,
                has_imported,
                arena_ptr,
                imported_ptr,
                bt
            );
        }
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
                .ok_or_else(|| undefined_variable(self.interner.lookup(*name))),

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
                let length = super::exec::expr::get_collection_length(&value)?;
                let idx = self.eval_with_hash_length(*index, length)?;
                super::exec::expr::eval_index(value, idx)
            }
            ExprKind::Field { receiver, field } => {
                let value = self.eval(*receiver)?;
                super::exec::expr::eval_field_access(value, *field, self.interner)
            }

            // Lambda
            //
            // IMPORTANT: Lambdas MUST always carry their arena reference to ensure
            // correct evaluation when called from different contexts (e.g., passed
            // to a prelude function and called from within that function's context).
            ExprKind::Lambda { params, body, .. } => {
                let names = self.arena.get_param_names(*params);
                let captures = self.env.capture();
                let arena = match &self.imported_arena {
                    Some(arena) => arena.clone(),
                    None => SharedArena::new(self.arena.clone()),
                };
                let func = FunctionValue::new(names, *body, captures, arena);
                Ok(Value::Function(func))
            }

            ExprKind::Block { stmts, result } => self.eval_block(*stmts, *result),

            ExprKind::Call { func, args } => {
                let func_val = self.eval(*func)?;
                let arg_vals: Result<Vec<_>, _> = self.arena.get_expr_list(*args).iter().map(|id| self.eval(*id)).collect();
                self.eval_call(func_val, &arg_vals?)
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
                .ok_or_else(|| undefined_function(self.interner.lookup(*name))),
            ExprKind::MethodCall { receiver, method, args } => {
                let recv = self.eval(*receiver)?;
                let arg_vals: Result<Vec<_>, _> = self.arena.get_expr_list(*args).iter()
                    .map(|id| self.eval(*id)).collect();
                self.eval_method_call(recv, *method, arg_vals?)
            }
            ExprKind::MethodCallNamed { receiver, method, args } => {
                let recv = self.eval(*receiver)?;
                let arg_vals: Result<Vec<_>, _> = self.arena.get_call_args(*args).iter()
                    .map(|arg| self.eval(arg.value)).collect();
                self.eval_method_call(recv, *method, arg_vals?)
            }
            ExprKind::Match { scrutinee, arms } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(&value, *arms)
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
                        return Err(map_keys_must_be_strings());
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
                            undefined_variable(name_str)
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
                Value::Err(e) => Err(EvalError::propagate(Value::Err(e.clone()), format!("propagated error: {e}"))),
                Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
                other => Ok(other),
            },
            ExprKind::Config(name) => self.env.lookup(*name)
                .ok_or_else(|| undefined_config(self.interner.lookup(*name))),
            // Capability provision: with Capability = Provider in body
            // For now, we evaluate the provider (which may have side effects),
            // then bind it to the capability name in a new scope and evaluate the body.
            ExprKind::WithCapability { capability, provider, body } => {
                let provider_val = self.eval(*provider)?;
                self.with_binding(*capability, provider_val, false, |e| e.eval(*body))
            }
            ExprKind::Error => Err(parse_error()),
            ExprKind::HashLength => Err(hash_outside_index()),
            ExprKind::SelfRef => self.env.lookup(self.interner.intern("self")).ok_or_else(self_outside_method),
            ExprKind::Await(_) => Err(await_not_supported()),
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

    /// Evaluate an expression with # (`HashLength`) resolved to a specific length.
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

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: crate::ir::StmtRange, result: Option<ExprId>) -> EvalResult {
        self.with_env_scope_result(|eval| {
            for stmt in eval.arena.get_stmt_range(stmts) {
                match &stmt.kind {
                    StmtKind::Expr(e) => { eval.eval(*e)?; }
                    StmtKind::Let { pattern, init, mutable, .. } => {
                        let value = eval.eval(*init)?;
                        eval.bind_pattern(pattern, value, *mutable)?;
                    }
                }
            }
            result.map(|r| eval.eval(r)).transpose().map(|v| v.unwrap_or(Value::Void))
        })
    }

    /// Bind a pattern to a value using `exec::control` module.
    pub(super) fn bind_pattern(&mut self, pattern: &BindingPattern, value: Value, mutable: bool) -> EvalResult {
        super::exec::control::bind_pattern(pattern, value, mutable, &mut self.env)
    }

    /// Evaluate a `function_exp` expression (map, filter, fold, etc.).
    ///
    /// Uses the pattern registry for Open/Closed principle compliance.
    /// Each pattern implementation is in a separate file under `patterns/`.
    fn eval_function_exp(&mut self, func_exp: &crate::ir::FunctionExp) -> EvalResult {
        let props = self.arena.get_named_exprs(func_exp.props);

        // Look up pattern definition from registry
        let pattern = self.registry.get(func_exp.kind)
            .ok_or_else(|| unknown_pattern(&format!("{:?}", func_exp.kind)))?;

        // Create evaluation context
        let ctx = EvalContext::new(self.interner, self.arena, props);

        // Evaluate via the pattern definition
        // Pass self as the executor which implements PatternExecutor
        pattern.evaluate(&ctx, self)
    }

    /// Evaluate a match expression.
    pub(super) fn eval_match(&mut self, value: &Value, arms: ArmRange) -> EvalResult {
        use super::exec::control::try_match;

        let arm_list = self.arena.get_arms(arms);

        for arm in arm_list {
            // Try to match the pattern using the exec module
            if let Some(bindings) = try_match(&arm.pattern, value, self.arena, self.interner)? {
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

        Err(non_exhaustive_match())
    }

    /// Evaluate a for loop using `exec::control` helpers.
    fn eval_for(&mut self, binding: Name, iter: Value, guard: Option<ExprId>, body: ExprId, is_yield: bool) -> EvalResult {
        use super::exec::control::{LoopAction, parse_loop_control};

        let items = match iter {
            Value::List(list) => list.iter().cloned().collect::<Vec<_>>(),
            Value::Range(range) => range.iter().map(Value::Int).collect(),
            _ => return Err(for_requires_iterable()),
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

    /// Evaluate a loop expression using `exec::control` helpers.
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

    /// Evaluate an assignment using `exec::control` module.
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

    /// Create an evaluator for function/method body evaluation.
    ///
    /// This helper implements the arena threading pattern: the callee's arena
    /// is used to evaluate the body, ensuring expression IDs are valid and
    /// enabling thread-safe parallel evaluation.
    ///
    /// # Arguments
    /// * `func_arena` - The callee's expression arena
    /// * `call_env` - The environment with parameters bound
    ///
    /// # Returns
    /// A new evaluator configured to evaluate the function body.
    /// The returned evaluator's lifetime is tied to `func_arena`, not `self`,
    /// but requires that `'a` outlives `'b` since we pass the interner through.
    pub(super) fn create_function_evaluator<'b>(
        &self,
        func_arena: &'b ExprArena,
        call_env: Environment,
    ) -> Evaluator<'b>
    where
        'a: 'b,
    {
        let imported_arena = SharedArena::new(func_arena.clone());
        Evaluator::with_imported_arena(
            self.interner,
            func_arena,
            call_env,
            imported_arena,
            self.user_method_registry.clone(),
        )
    }

    /// Register a `function_val` (type conversion function).
    pub fn register_function_val(&mut self, name: &str, func: super::value::FunctionValFn, display_name: &'static str) {
        let name = self.interner.intern(name);
        self.env.define_global(name, Value::FunctionVal(func, display_name));
    }

    /// Register all `function_val` (type conversion) functions.
    ///
    /// `function_val`: Type conversion functions like int(x), str(x), float(x)
    /// that allow positional arguments per the spec.
    pub fn register_prelude(&mut self) {
        use sigil_eval::{function_val_str, function_val_int, function_val_float, function_val_byte, function_val_thread_id};

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
mod tests;
