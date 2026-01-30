//! Tree-walking interpreter for Ori.
//!
//! This is the portable interpreter that can run in both native and WASM contexts.
//! For the full Salsa-integrated evaluator, see `oric::Evaluator`.
//!
//! # Modular Architecture
//!
//! The interpreter's `eval()` method acts as a dispatcher, delegating to specialized
//! helper modules in `crate::exec`:
//!
//! - `exec::expr` - Literals, identifiers, binary/unary operators, field access
//! - `exec::call` - Function calls, argument evaluation
//! - `exec::control` - Control flow (if, match, loop, for, break, continue)
//! - `exec::pattern` - Pattern matching for let bindings and match arms
//!
//! This keeps the main match statement focused on dispatch while logic lives in
//! the helper modules. The match remains necessary as the coordination point.
//!
//! # Arena Threading Pattern
//!
//! Functions and methods in Ori carry their own expression arena (`SharedArena`)
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
//! Use `create_function_interpreter()` to correctly set up evaluation context
//! with the callee's arena.

mod builder;
mod derived_methods;
mod function_call;
mod function_seq;
mod method_dispatch;
pub mod resolvers;
mod scope_guard;

pub use builder::InterpreterBuilder;
pub use scope_guard::ScopedInterpreter;

use crate::print_handler::SharedPrintHandler;
use crate::stack::ensure_sufficient_stack;
use crate::{
    // Error factories
    await_not_supported,
    evaluate_unary,
    for_requires_iterable,
    hash_outside_index,
    map_keys_must_be_strings,
    no_member_in_module,
    non_exhaustive_match,
    parse_error,
    self_outside_method,
    undefined_config,
    undefined_function,
    undefined_variable,
    Environment,
    FunctionValue,
    Mutability,
    SharedMutableRegistry,
    SharedRegistry,
    StructValue,
    UserMethodRegistry,
    Value,
};
use ori_ir::{
    ArmRange, BindingPattern, ExprArena, ExprId, ExprKind, Name, SharedArena, StmtKind,
    StringInterner, UnaryOp,
};
use ori_patterns::{
    propagated_error_message, EvalContext, EvalError, EvalResult, PatternExecutor, PatternRegistry,
};

/// Tree-walking interpreter for Ori expressions.
///
/// This is the portable interpreter that works in both native and WASM contexts.
/// For Salsa-integrated evaluation with imports, see `oric::Evaluator`.
pub struct Interpreter<'a> {
    /// String interner for name lookup.
    pub interner: &'a StringInterner,
    /// Expression arena.
    pub arena: &'a ExprArena,
    /// Current environment.
    pub env: Environment,
    /// Pattern registry for `function_exp` evaluation.
    pub registry: SharedRegistry<PatternRegistry>,
    /// User-defined method registry for impl block methods.
    ///
    /// Uses `SharedMutableRegistry` to allow method registration after the
    /// evaluator (and its cached dispatcher) is created.
    pub user_method_registry: SharedMutableRegistry<UserMethodRegistry>,
    /// Cached method dispatcher for efficient method resolution.
    ///
    /// The dispatcher chains all method resolvers (user, derived, collection, builtin)
    /// and is built once during construction. Because `user_method_registry` uses
    /// interior mutability, the dispatcher sees method registrations made after creation.
    pub method_dispatcher: resolvers::MethodDispatcher,
    /// Arena reference for imported functions.
    ///
    /// When evaluating an imported function, this holds the imported arena.
    /// Lambdas created during evaluation will inherit this arena reference.
    pub imported_arena: Option<SharedArena>,
    /// Whether the prelude has been auto-loaded.
    /// Used by `oric::Evaluator` when module loading is enabled.
    #[expect(
        dead_code,
        reason = "Used by oric::Evaluator for prelude tracking, not exposed via ori_eval"
    )]
    pub(crate) prelude_loaded: bool,
    /// Print handler for the Print capability.
    ///
    /// Determines where print output goes:
    /// - Native: stdout (default)
    /// - WASM: buffer for capture
    /// - Tests: buffer for assertions
    pub print_handler: SharedPrintHandler,
    /// Whether this interpreter owns a scoped environment that should be popped on drop.
    ///
    /// When an interpreter is created for function/method calls via `create_function_interpreter`,
    /// the caller pushes a scope before passing the environment. This flag ensures the scope
    /// is popped when the interpreter is dropped, even if evaluation panics.
    ///
    /// This provides RAII-style panic safety for function call evaluation.
    pub(crate) owns_scoped_env: bool,
}

/// RAII Drop implementation for panic-safe scope cleanup.
///
/// If `owns_scoped_env` is true, this interpreter was created for a function/method
/// call and owns a scope that must be popped. This ensures scope cleanup even if
/// evaluation panics during the call.
impl Drop for Interpreter<'_> {
    fn drop(&mut self) {
        if self.owns_scoped_env {
            self.env.pop_scope();
        }
    }
}

/// Implement `PatternExecutor` for Interpreter to enable pattern evaluation.
///
/// This allows patterns to request expression evaluation and function calls
/// without needing direct access to the interpreter's internals.
impl PatternExecutor for Interpreter<'_> {
    fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        Interpreter::eval(self, expr_id)
    }

    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, &args)
    }

    fn lookup_capability(&self, name: &str) -> Option<Value> {
        let name_id = self.interner.intern(name);
        self.env.lookup(name_id)
    }

    fn call_method(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
        // Special handling for built-in print methods
        if method == "__builtin_println" || method == "println" {
            if let Some(msg) = args.first() {
                if let Value::Str(s) = msg {
                    self.print_handler.println(s);
                } else {
                    self.print_handler.println(&msg.display_value());
                }
            }
            return Ok(Value::Void);
        }
        if method == "__builtin_print" || method == "print" {
            if let Some(msg) = args.first() {
                if let Value::Str(s) = msg {
                    self.print_handler.print(s);
                } else {
                    self.print_handler.print(&msg.display_value());
                }
            }
            return Ok(Value::Void);
        }

        // For other methods, use regular method dispatch
        let method_name = self.interner.intern(method);
        self.eval_method_call(receiver, method_name, args)
    }

    fn lookup_var(&self, name: &str) -> Option<Value> {
        let name_id = self.interner.intern(name);
        self.env.lookup(name_id)
    }

    fn bind_var(&mut self, name: &str, value: Value) {
        let name_id = self.interner.intern(name);
        self.env.define(name_id, value, Mutability::Immutable);
    }
}

impl<'a> Interpreter<'a> {
    /// Create a new interpreter with default registries.
    ///
    /// For more configuration options, use `InterpreterBuilder::new(interner, arena)`.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        InterpreterBuilder::new(interner, arena).build()
    }

    /// Create an interpreter builder for more configuration options.
    pub fn builder(interner: &'a StringInterner, arena: &'a ExprArena) -> InterpreterBuilder<'a> {
        InterpreterBuilder::new(interner, arena)
    }

    /// Evaluate an expression.
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_inner(expr_id))
    }

    /// Evaluate a list of expressions from an ExprRange.
    ///
    /// Helper to reduce repetition in collection and call evaluation.
    fn eval_expr_list(&mut self, range: ori_ir::ExprRange) -> Result<Vec<Value>, EvalError> {
        self.arena
            .get_expr_list(range)
            .iter()
            .map(|id| self.eval(*id))
            .collect()
    }

    /// Evaluate call arguments from a CallArgRange.
    ///
    /// Helper for named argument evaluation in method calls.
    fn eval_call_args(&mut self, range: ori_ir::CallArgRange) -> Result<Vec<Value>, EvalError> {
        self.arena
            .get_call_args(range)
            .iter()
            .map(|arg| self.eval(arg.value))
            .collect()
    }

    /// Inner evaluation logic (wrapped by `eval` for stack safety).
    fn eval_inner(&mut self, expr_id: ExprId) -> EvalResult {
        // Check arena bounds before access
        if expr_id.index() >= self.arena.expr_count() {
            return Err(EvalError::new(format!(
                "Internal error: expression {} not found (arena has {} expressions). \
                 This is likely a compiler bug.",
                expr_id.index(),
                self.arena.expr_count()
            )));
        }
        let expr = self.arena.get_expr(expr_id);

        // Try literal evaluation first (handles Int, Float, Bool, String, Char, Unit, Duration, Size)
        if let Some(result) = crate::exec::expr::eval_literal(&expr.kind, self.interner) {
            return result;
        }

        match &expr.kind {
            // Literals handled by eval_literal above
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Unit
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. } => unreachable!("handled by eval_literal"),

            // Identifiers
            ExprKind::Ident(name) => crate::exec::expr::eval_ident(*name, &self.env, self.interner),

            // Operators
            ExprKind::Binary { left, op, right } => {
                crate::exec::expr::eval_binary(*left, *op, *right, |e| self.eval(e))
            }
            ExprKind::Unary { op, operand } => self.eval_unary(*op, *operand),

            // Control flow
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                if self.eval(*cond)?.is_truthy() {
                    self.eval(*then_branch)
                } else {
                    else_branch
                        .map(|e| self.eval(e))
                        .transpose()?
                        .map_or(Ok(Value::Void), Ok)
                }
            }

            // Collections
            ExprKind::List(range) => Ok(Value::list(self.eval_expr_list(*range)?)),
            ExprKind::Tuple(range) => Ok(Value::tuple(self.eval_expr_list(*range)?)),
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => crate::exec::expr::eval_range(*start, *end, *inclusive, |e| self.eval(e)),

            // Access
            ExprKind::Index { receiver, index } => {
                let value = self.eval(*receiver)?;
                let length = crate::exec::expr::get_collection_length(&value)?;
                let idx = self.eval_with_hash_length(*index, length)?;
                crate::exec::expr::eval_index(value, idx)
            }
            ExprKind::Field { receiver, field } => {
                let value = self.eval(*receiver)?;
                crate::exec::expr::eval_field_access(value, *field, self.interner)
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
                let arg_vals = self.eval_expr_list(*args)?;
                self.eval_call(&func_val, &arg_vals)
            }

            // Variant constructors
            ExprKind::Some(inner) => Ok(Value::some(self.eval(*inner)?)),
            ExprKind::None => Ok(Value::None),
            ExprKind::Ok(inner) => Ok(Value::ok(
                inner
                    .map(|e| self.eval(e))
                    .transpose()?
                    .unwrap_or(Value::Void),
            )),
            ExprKind::Err(inner) => Ok(Value::err(
                inner
                    .map(|e| self.eval(e))
                    .transpose()?
                    .unwrap_or(Value::Void),
            )),

            // Let binding
            ExprKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => {
                let value = self.eval(*init)?;
                let mutability = if *mutable {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                self.bind_pattern(pattern, value, mutability)?;
                Ok(Value::Void)
            }

            ExprKind::FunctionSeq(seq) => self.eval_function_seq(seq),
            ExprKind::FunctionExp(exp) => self.eval_function_exp(exp),
            ExprKind::CallNamed { func, args } => {
                let func_val = self.eval(*func)?;
                self.eval_call_named(&func_val, *args)
            }
            ExprKind::FunctionRef(name) => self
                .env
                .lookup(*name)
                .ok_or_else(|| undefined_function(self.interner.lookup(*name))),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.eval(*receiver)?;
                let arg_vals = self.eval_expr_list(*args)?;
                self.dispatch_method_call(recv, *method, arg_vals)
            }
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => {
                let recv = self.eval(*receiver)?;
                let arg_vals = self.eval_call_args(*args)?;
                self.dispatch_method_call(recv, *method, arg_vals)
            }
            ExprKind::Match { scrutinee, arms } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(&value, *arms)
            }
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                let iter_val = self.eval(*iter)?;
                self.eval_for(*binding, iter_val, *guard, *body, *is_yield)
            }
            ExprKind::Loop { body } => self.eval_loop(*body),

            // Map literal
            ExprKind::Map(entries) => {
                let entry_list = self.arena.get_map_entries(*entries);
                let mut map = std::collections::BTreeMap::new();
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

            ExprKind::Return(v) => {
                let val = v.map(|x| self.eval(x)).transpose()?.unwrap_or(Value::Void);
                Err(EvalError::return_with(val))
            }
            ExprKind::Break(v) => {
                let val = v.map(|x| self.eval(x)).transpose()?.unwrap_or(Value::Void);
                Err(EvalError::break_with(val))
            }
            ExprKind::Continue => Err(EvalError::continue_signal()),
            ExprKind::Assign { target, value } => {
                let val = self.eval(*value)?;
                self.eval_assign(*target, val)
            }
            ExprKind::Try(inner) => match self.eval(*inner)? {
                Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
                Value::Err(e) => Err(EvalError::propagate(
                    Value::Err(e.clone()),
                    propagated_error_message(&e),
                )),
                Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
                other => Ok(other),
            },
            ExprKind::Config(name) => self
                .env
                .lookup(*name)
                .ok_or_else(|| undefined_config(self.interner.lookup(*name))),
            // Capability provision: with Capability = Provider in body
            // For now, we evaluate the provider (which may have side effects),
            // then bind it to the capability name in a new scope and evaluate the body.
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider_val = self.eval(*provider)?;
                self.with_binding(*capability, provider_val, Mutability::Immutable, |e| {
                    e.eval(*body)
                })
            }
            ExprKind::Error => Err(parse_error()),
            ExprKind::HashLength => Err(hash_outside_index()),
            ExprKind::SelfRef => self
                .env
                .lookup(self.interner.intern("self"))
                .ok_or_else(self_outside_method),
            ExprKind::Await(_) => Err(await_not_supported()),
        }
    }

    /// Evaluate a unary operation.
    fn eval_unary(&mut self, op: UnaryOp, operand: ExprId) -> EvalResult {
        let value = self.eval(operand)?;
        evaluate_unary(value, op)
    }

    /// Evaluate an expression with # (`HashLength`) resolved to a specific length.
    fn eval_with_hash_length(&mut self, expr_id: ExprId, length: i64) -> EvalResult {
        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            ExprKind::HashLength => Ok(Value::int(length)),
            ExprKind::Binary { left, op, right } => {
                let left_val = self.eval_with_hash_length(*left, length)?;
                let right_val = self.eval_with_hash_length(*right, length)?;
                crate::exec::expr::eval_binary_values(left_val, *op, right_val)
            }
            _ => self.eval(expr_id),
        }
    }

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: ori_ir::StmtRange, result: Option<ExprId>) -> EvalResult {
        self.with_env_scope_result(|eval| {
            for stmt in eval.arena.get_stmt_range(stmts) {
                match &stmt.kind {
                    StmtKind::Expr(e) => {
                        eval.eval(*e)?;
                    }
                    StmtKind::Let {
                        pattern,
                        init,
                        mutable,
                        ..
                    } => {
                        let value = eval.eval(*init)?;
                        let mutability = if *mutable {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        eval.bind_pattern(pattern, value, mutability)?;
                    }
                }
            }
            result
                .map(|r| eval.eval(r))
                .transpose()
                .map(|v| v.unwrap_or(Value::Void))
        })
    }

    /// Bind a pattern to a value using `exec::control` module.
    pub(super) fn bind_pattern(
        &mut self,
        pattern: &BindingPattern,
        value: Value,
        mutability: Mutability,
    ) -> EvalResult {
        crate::exec::control::bind_pattern(pattern, value, mutability, &mut self.env)
    }

    /// Evaluate a `function_exp` expression (map, filter, fold, etc.).
    ///
    /// Uses the pattern registry for Open/Closed principle compliance.
    /// Each pattern implementation is in a separate file under `patterns/`.
    fn eval_function_exp(&mut self, func_exp: &ori_ir::FunctionExp) -> EvalResult {
        let props = self.arena.get_named_exprs(func_exp.props);

        // Look up pattern definition from registry (all kinds are covered)
        let pattern = self.registry.get(func_exp.kind);

        // Create evaluation context
        let ctx = EvalContext::new(self.interner, self.arena, props);

        // Evaluate via the pattern definition
        // Pass self as the executor which implements PatternExecutor
        pattern.evaluate(&ctx, self)
    }

    /// Evaluate a match expression.
    ///
    /// Uses RAII scope guard to ensure scope is popped even on panic.
    pub(super) fn eval_match(&mut self, value: &Value, arms: ArmRange) -> EvalResult {
        use crate::exec::control::try_match;

        let arm_list = self.arena.get_arms(arms);

        for arm in arm_list {
            // Try to match the pattern using the exec module
            if let Some(bindings) = try_match(&arm.pattern, value, self.arena, self.interner)? {
                // Use RAII guard for scope safety - scope is popped even on panic
                // Returns Option<EvalResult>: None = guard failed, Some = result
                let result: Option<EvalResult> = self.with_match_bindings(bindings, |eval| {
                    // Check if guard passes (if present) - bindings are now available
                    if let Some(guard) = arm.guard {
                        match eval.eval(guard) {
                            Ok(v) if !v.is_truthy() => return None, // Guard failed
                            Err(e) => return Some(Err(e)),          // Propagate error
                            Ok(_) => {}                             // Guard passed
                        }
                    }
                    // Evaluate body
                    Some(eval.eval(arm.body))
                });

                if let Some(r) = result {
                    return r; // Either Ok(value) or Err(e)
                }
                // Guard failed, try next arm
            }
        }

        Err(non_exhaustive_match())
    }

    /// Evaluate a for loop using `exec::control` helpers.
    ///
    /// Uses RAII scope guard to ensure scope is popped even on panic.
    fn eval_for(
        &mut self,
        binding: Name,
        iter: Value,
        guard: Option<ExprId>,
        body: ExprId,
        is_yield: bool,
    ) -> EvalResult {
        use crate::exec::control::{parse_loop_control, LoopAction};

        /// Result of a single loop iteration with RAII guard.
        enum IterResult {
            Continue,         // Normal continue or guard failed
            Yield(Value),     // Yield mode: value to collect
            Break(Value),     // Break with value
            Error(EvalError), // Propagate error
        }

        /// Lazy iterator over for loop items to avoid pre-collecting all elements.
        enum ForIterator {
            List {
                list: crate::Heap<Vec<Value>>,
                index: usize,
            },
            Range {
                iter: std::ops::Range<i64>,
            },
        }

        impl Iterator for ForIterator {
            type Item = Value;

            fn next(&mut self) -> Option<Value> {
                match self {
                    ForIterator::List { list, index } => {
                        if *index < list.len() {
                            let item = list[*index].clone();
                            *index = index.saturating_add(1);
                            Some(item)
                        } else {
                            None
                        }
                    }
                    ForIterator::Range { iter } => iter.next().map(Value::int),
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                match self {
                    ForIterator::List { list, index } => {
                        let remaining = list.len().saturating_sub(*index);
                        (remaining, Some(remaining))
                    }
                    ForIterator::Range { iter } => iter.size_hint(),
                }
            }
        }

        let items = match iter {
            Value::List(list) => ForIterator::List { list, index: 0 },
            Value::Range(range) => ForIterator::Range {
                iter: range.start..if range.inclusive {
                    range.end.saturating_add(1)
                } else {
                    range.end
                },
            },
            _ => return Err(for_requires_iterable()),
        };

        if is_yield {
            let (lower, _) = items.size_hint();
            let mut results = Vec::with_capacity(lower);
            for item in items {
                // Use RAII guard for scope safety
                let iter_result = self.with_binding(binding, item, Mutability::Immutable, |eval| {
                    // Check guard
                    if let Some(g) = guard {
                        match eval.eval(g) {
                            Ok(v) if !v.is_truthy() => return IterResult::Continue,
                            Err(e) => return IterResult::Error(e),
                            Ok(_) => {}
                        }
                    }
                    // Evaluate body
                    match eval.eval(body) {
                        Ok(v) => IterResult::Yield(v),
                        Err(e) => IterResult::Error(e),
                    }
                });

                match iter_result {
                    IterResult::Continue => {}
                    IterResult::Yield(v) => results.push(v),
                    IterResult::Break(v) => return Ok(v),
                    IterResult::Error(e) => return Err(e),
                }
            }
            Ok(Value::list(results))
        } else {
            for item in items {
                // Use RAII guard for scope safety
                let iter_result = self.with_binding(binding, item, Mutability::Immutable, |eval| {
                    // Check guard
                    if let Some(g) = guard {
                        match eval.eval(g) {
                            Ok(v) if !v.is_truthy() => return IterResult::Continue,
                            Err(e) => return IterResult::Error(e),
                            Ok(_) => {}
                        }
                    }
                    // Evaluate body and handle loop control
                    match eval.eval(body) {
                        Ok(_) => IterResult::Continue,
                        Err(e) => match parse_loop_control(&e.message) {
                            LoopAction::Continue => IterResult::Continue,
                            LoopAction::Break(v) => IterResult::Break(v),
                            LoopAction::Error(_) => IterResult::Error(e),
                        },
                    }
                });

                match iter_result {
                    IterResult::Continue => {}
                    IterResult::Yield(_) => unreachable!("Yield only in yield mode"),
                    IterResult::Break(v) => return Ok(v),
                    IterResult::Error(e) => return Err(e),
                }
            }
            Ok(Value::Void)
        }
    }

    /// Evaluate a loop expression using `exec::control` helpers.
    fn eval_loop(&mut self, body: ExprId) -> EvalResult {
        use crate::exec::control::{parse_loop_control, LoopAction};
        loop {
            match self.eval(body) {
                Ok(_) => {}
                Err(e) => match parse_loop_control(&e.message) {
                    LoopAction::Continue => {}
                    LoopAction::Break(v) => return Ok(v),
                    LoopAction::Error(_) => return Err(e),
                },
            }
        }
    }

    /// Evaluate an assignment using `exec::control` module.
    fn eval_assign(&mut self, target: ExprId, value: Value) -> EvalResult {
        crate::exec::control::eval_assign(target, value, self.arena, self.interner, &mut self.env)
    }

    /// Dispatch a method call, handling ModuleNamespace specially.
    ///
    /// For module namespaces, looks up the function and calls it directly.
    /// For other receivers, uses `eval_method_call`.
    fn dispatch_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        if let Value::ModuleNamespace(ns) = &receiver {
            let func = ns
                .get(&method)
                .ok_or_else(|| no_member_in_module(self.interner.lookup(method)))?;
            self.eval_call(func, &args)
        } else {
            self.eval_method_call(receiver, method, args)
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

    /// Create an interpreter for function/method body evaluation.
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
    /// A new interpreter configured to evaluate the function body.
    /// The returned interpreter's lifetime is tied to `func_arena`, not `self`,
    /// but requires that `'a` outlives `'b` since we pass the interner through.
    pub(crate) fn create_function_interpreter<'b>(
        &self,
        func_arena: &'b ExprArena,
        call_env: Environment,
    ) -> Interpreter<'b>
    where
        'a: 'b,
    {
        let imported_arena = SharedArena::new(func_arena.clone());
        InterpreterBuilder::new(self.interner, func_arena)
            .env(call_env)
            .imported_arena(imported_arena)
            .user_method_registry(self.user_method_registry.clone())
            .print_handler(self.print_handler.clone())
            .owns_scoped_env(true) // RAII: scope will be popped when interpreter drops
            .build()
    }

    /// Register a `function_val` (type conversion function).
    pub fn register_function_val(
        &mut self,
        name: &str,
        func: crate::FunctionValFn,
        display_name: &'static str,
    ) {
        let name = self.interner.intern(name);
        self.env
            .define_global(name, Value::FunctionVal(func, display_name));
    }

    /// Register all `function_val` (type conversion) functions.
    ///
    /// `function_val`: Type conversion functions like int(x), str(x), float(x)
    /// that allow positional arguments per the spec.
    pub fn register_prelude(&mut self) {
        use crate::{
            function_val_byte, function_val_float, function_val_int, function_val_str,
            function_val_thread_id,
        };

        // Type conversion functions (positional args allowed per spec)
        self.register_function_val("str", function_val_str, "str");
        self.register_function_val("int", function_val_int, "int");
        self.register_function_val("float", function_val_float, "float");
        self.register_function_val("byte", function_val_byte, "byte");

        // Thread/parallel introspection (internal use)
        self.register_function_val("thread_id", function_val_thread_id, "thread_id");
    }

    /// Get captured print output.
    ///
    /// Returns all output written via print/println since the last clear.
    /// For stdout handler, this returns an empty string (stdout doesn't capture).
    /// For buffer handler, this returns the accumulated output.
    pub fn get_print_output(&self) -> String {
        self.print_handler.get_output()
    }

    /// Clear captured print output.
    pub fn clear_print_output(&self) {
        self.print_handler.clear();
    }
}

// Tests for full expression evaluation are in oric/src/eval/evaluator/tests.rs
// since they require the Salsa database infrastructure.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_handler::buffer_handler;
    use ori_ir::SharedInterner;

    #[test]
    fn print_handler_integration_println() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        // Directly call the print handler
        interpreter.print_handler.println("hello world");

        assert_eq!(interpreter.get_print_output(), "hello world\n");
    }

    #[test]
    fn print_handler_integration_print() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        interpreter.print_handler.print("hello");
        interpreter.print_handler.print(" world");

        assert_eq!(interpreter.get_print_output(), "hello world");
    }

    #[test]
    fn print_handler_integration_clear() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        interpreter.print_handler.println("first");
        interpreter.clear_print_output();
        interpreter.print_handler.println("second");

        assert_eq!(interpreter.get_print_output(), "second\n");
    }

    #[test]
    fn default_handler_is_stdout() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();

        let interpreter = InterpreterBuilder::new(&interner, &arena).build();

        // Default stdout handler doesn't capture, returns empty
        assert_eq!(interpreter.get_print_output(), "");
    }

    #[test]
    fn handler_shared_between_interpreters() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let interpreter1 = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        let interpreter2 = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        interpreter1.print_handler.println("from 1");
        interpreter2.print_handler.println("from 2");

        // Both wrote to the same handler
        let output = handler.get_output();
        assert!(output.contains("from 1"));
        assert!(output.contains("from 2"));
    }

    #[test]
    fn call_method_println_uses_handler() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let mut interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        // Test that call_method routes println to the handler
        let result = <Interpreter as PatternExecutor>::call_method(
            &mut interpreter,
            Value::Void,
            "println",
            vec![Value::string("test message")],
        );

        assert!(result.is_ok());
        assert_eq!(interpreter.get_print_output(), "test message\n");
    }

    #[test]
    fn call_method_print_uses_handler() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let mut interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        // Test that call_method routes print to the handler
        let result = <Interpreter as PatternExecutor>::call_method(
            &mut interpreter,
            Value::Void,
            "print",
            vec![Value::string("no newline")],
        );

        assert!(result.is_ok());
        assert_eq!(interpreter.get_print_output(), "no newline");
    }

    #[test]
    fn call_method_builtin_println_uses_handler() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let handler = buffer_handler();

        let mut interpreter = InterpreterBuilder::new(&interner, &arena)
            .print_handler(handler.clone())
            .build();

        // Test the __builtin_println fallback path
        let result = <Interpreter as PatternExecutor>::call_method(
            &mut interpreter,
            Value::Void,
            "__builtin_println",
            vec![Value::string("builtin test")],
        );

        assert!(result.is_ok());
        assert_eq!(interpreter.get_print_output(), "builtin test\n");
    }
}
