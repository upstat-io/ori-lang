//! Tree-walking interpreter for Ori.
//!
//! This is the portable interpreter that can run in both native and WASM contexts.
//! For the full Salsa-integrated evaluator, see `oric::Evaluator`.
//!
//! # Specification
//!
//! - Eval rules: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`
//!
//! Implementation must match the evaluation rules in operator-rules.md.
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

use crate::evaluate_binary;
use crate::print_handler::SharedPrintHandler;
use crate::{
    // Error factories
    await_not_supported,
    evaluate_unary,
    for_requires_iterable,
    hash_outside_index,
    map_key_not_hashable,
    no_member_in_module,
    non_exhaustive_match,
    parse_error,
    self_outside_method,
    spread_requires_map,
    undefined_const,
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
    ArmRange, BinaryOp, BindingPattern, ExprArena, ExprId, ExprKind, Name, SharedArena, StmtKind,
    StringInterner, UnaryOp,
};
use ori_types::Idx;
use rustc_hash::FxHashMap;

/// Pre-interned type names for hot-path method dispatch.
///
/// These names are interned once at Interpreter construction to avoid
/// repeated hash lookups in `get_value_type_name()`, which is called
/// on every method dispatch.
#[derive(Clone, Copy)]
pub struct TypeNames {
    pub range: Name,
    pub int: Name,
    pub float: Name,
    pub bool_: Name,
    pub str_: Name,
    pub char_: Name,
    pub byte: Name,
    pub void: Name,
    pub duration: Name,
    pub size: Name,
    pub ordering: Name,
    pub list: Name,
    pub map: Name,
    pub tuple: Name,
    pub option: Name,
    pub result: Name,
    pub function: Name,
    pub function_val: Name,
    pub module: Name,
    pub error: Name,
}

impl TypeNames {
    /// Pre-intern all primitive type names.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            range: interner.intern("range"),
            int: interner.intern("int"),
            float: interner.intern("float"),
            bool_: interner.intern("bool"),
            str_: interner.intern("str"),
            char_: interner.intern("char"),
            byte: interner.intern("byte"),
            void: interner.intern("void"),
            duration: interner.intern("Duration"),
            size: interner.intern("Size"),
            ordering: interner.intern("Ordering"),
            list: interner.intern("list"),
            map: interner.intern("map"),
            tuple: interner.intern("tuple"),
            option: interner.intern("Option"),
            result: interner.intern("Result"),
            function: interner.intern("function"),
            function_val: interner.intern("function_val"),
            module: interner.intern("module"),
            error: interner.intern("error"),
        }
    }
}
#[cfg(target_arch = "wasm32")]
use ori_patterns::recursion_limit_exceeded;
use ori_patterns::{
    propagated_error_message, EvalContext, EvalError, EvalResult, PatternDefinition,
    PatternExecutor, PatternRegistry,
};
use ori_stack::ensure_sufficient_stack;

/// Default maximum call depth for WASM builds to prevent stack exhaustion.
///
/// WASM has a fixed stack that cannot grow dynamically like native builds.
/// This limit prevents cryptic "Maximum call stack size exceeded" errors
/// by failing gracefully with a clear "maximum recursion depth exceeded" message.
///
/// The default (200) is conservative for browser environments. WASM runtimes
/// outside browsers (Node.js, Wasmtime, etc.) may support higher limits.
///
/// On native builds, `stacker` handles deep recursion by growing the stack,
/// so this limit is not enforced.
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_MAX_CALL_DEPTH: usize = 200;

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
    /// Pre-computed Name for "self" keyword (avoids repeated interning).
    pub self_name: Name,
    /// Pre-interned type names for hot-path method dispatch.
    /// Avoids repeated `intern()` calls in `get_value_type_name()`.
    pub(crate) type_names: TypeNames,
    /// Current call depth for recursion limit tracking (WASM only).
    ///
    /// On WASM builds, this is checked against `max_call_depth` to prevent
    /// stack exhaustion. On native builds with `stacker`, this is tracked
    /// but not enforced.
    pub(crate) call_depth: usize,
    /// Maximum call depth before erroring (WASM only).
    ///
    /// Configurable at runtime via `InterpreterBuilder::max_call_depth()`.
    /// Defaults to `DEFAULT_MAX_CALL_DEPTH` (200).
    #[cfg(target_arch = "wasm32")]
    pub(crate) max_call_depth: usize,
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
    /// Expression type table from type checking.
    ///
    /// Maps `ExprId.index()` to `Idx`. Used by operators like `??` that need
    /// type information to determine correct behavior (e.g., chaining vs unwrapping).
    /// Optional because some evaluator uses don't require type info.
    pub(crate) expr_types: Option<&'a [Idx]>,
    /// Resolved pattern disambiguations from the type checker.
    ///
    /// Used by `try_match` to distinguish `Binding("Pending")` (unit variant)
    /// from `Binding("x")` (variable). Sorted by `PatternKey` for binary search.
    pub(crate) pattern_resolutions: &'a [(ori_types::PatternKey, ori_types::PatternResolution)],
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

    /// Check if the current call depth exceeds the recursion limit.
    ///
    /// On WASM, this enforces the configured `max_call_depth` to prevent stack exhaustion.
    /// On native builds with `stacker`, this is a no-op since the stack grows dynamically.
    #[cfg(target_arch = "wasm32")]
    #[inline]
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        if self.call_depth >= self.max_call_depth {
            Err(recursion_limit_exceeded(self.max_call_depth))
        } else {
            Ok(())
        }
    }

    /// Check if the current call depth exceeds the recursion limit.
    ///
    /// On native builds, this is a no-op since `stacker` handles stack growth.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    #[expect(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        reason = "API parity with WASM version which uses self.call_depth and returns Result"
    )]
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        Ok(())
    }

    /// Check if two expressions have the same type.
    ///
    /// Used by the `??` operator to determine whether to chain or unwrap.
    /// Chaining: left type == result type → return left unchanged
    /// Unwrapping: left type ≠ result type → return inner value
    ///
    /// Returns `None` if type info is not available.
    #[inline]
    fn types_match(&self, expr1: ExprId, expr2: ExprId) -> Option<bool> {
        let expr_types = self.expr_types?;
        let type1 = expr_types.get(expr1.index())?;
        let type2 = expr_types.get(expr2.index())?;
        // Error types don't represent real types — return None (unknown)
        // to prevent incorrect chaining/unwrapping decisions in ??
        if *type1 == ori_types::Idx::ERROR || *type2 == ori_types::Idx::ERROR {
            return None;
        }
        Some(type1 == type2)
    }

    /// Evaluate an expression.
    ///
    /// Uses `ensure_sufficient_stack` to prevent stack overflow
    /// on deeply nested expressions.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_inner(expr_id))
    }

    /// Attach a span to an error if it doesn't already have one.
    ///
    /// This ensures errors from operator evaluation have source location
    /// information for better error messages.
    #[inline]
    fn attach_span(err: EvalError, span: ori_ir::Span) -> EvalError {
        if err.span.is_none() {
            err.with_span(span)
        } else {
            err
        }
    }

    /// Evaluate a list of expressions from an `ExprRange`.
    ///
    /// Helper to reduce repetition in collection and call evaluation.
    fn eval_expr_list(&mut self, range: ori_ir::ExprRange) -> Result<Vec<Value>, EvalError> {
        self.arena
            .get_expr_list(range)
            .iter()
            .copied()
            .map(|id| self.eval(id))
            .collect()
    }

    /// Evaluate call arguments from a `CallArgRange`.
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
            | ExprKind::Size { .. }
            | ExprKind::TemplateFull(_) => unreachable!("handled by eval_literal"),

            // Identifiers
            ExprKind::Ident(name) => crate::exec::expr::eval_ident(
                *name,
                &self.env,
                self.interner,
                Some(&self.user_method_registry.read()),
            ),

            // Operators
            ExprKind::Binary { left, op, right } => self.eval_binary(expr_id, *left, *op, *right),
            ExprKind::Unary { op, operand } => self.eval_unary(expr_id, *op, *operand),

            // Control flow
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                if self.eval(*cond)?.is_truthy() {
                    self.eval(*then_branch)
                } else if else_branch.is_present() {
                    self.eval(*else_branch)
                } else {
                    Ok(Value::Void)
                }
            }

            // Collections
            ExprKind::List(range) => Ok(Value::list(self.eval_expr_list(*range)?)),
            ExprKind::ListWithSpread(elements) => {
                let element_list = self.arena.get_list_elements(*elements);
                let mut result = Vec::new();
                for element in element_list {
                    match element {
                        ori_ir::ListElement::Expr { expr, .. } => {
                            result.push(self.eval(*expr)?);
                        }
                        ori_ir::ListElement::Spread { expr, .. } => {
                            // Evaluate the spread expression and append all its elements
                            let spread_val = self.eval(*expr)?;
                            if let Value::List(items) = spread_val {
                                result.extend(items.iter().cloned());
                            } else {
                                return Err(EvalError::new(
                                    "spread operator requires a list".to_string(),
                                ));
                            }
                        }
                    }
                }
                Ok(Value::list(result))
            }
            ExprKind::Tuple(range) => Ok(Value::tuple(self.eval_expr_list(*range)?)),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => crate::exec::expr::eval_range(*start, *end, *step, *inclusive, |e| self.eval(e)),

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
            ExprKind::Ok(inner) => Ok(Value::ok(if inner.is_present() {
                self.eval(*inner)?
            } else {
                Value::Void
            })),
            ExprKind::Err(inner) => Ok(Value::err(if inner.is_present() {
                self.eval(*inner)?
            } else {
                Value::Void
            })),

            // Let binding
            ExprKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => {
                let pat = self.arena.get_binding_pattern(*pattern);
                let value = self.eval(*init)?;
                let mutability = if *mutable {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                self.bind_pattern(pat, value, mutability)?;
                Ok(Value::Void)
            }

            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.arena.get_function_seq(*seq_id);
                self.eval_function_seq(seq)
            }
            ExprKind::FunctionExp(exp_id) => {
                let exp = self.arena.get_function_exp(*exp_id);
                self.eval_function_exp(exp)
            }
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
                    let key_str = key.to_map_key().map_err(|_| map_key_not_hashable())?;
                    map.insert(key_str, value);
                }
                Ok(Value::map(map))
            }

            // Map literal with spread
            ExprKind::MapWithSpread(elements) => {
                let element_list = self.arena.get_map_elements(*elements);
                let mut map = std::collections::BTreeMap::new();
                for element in element_list {
                    match element {
                        ori_ir::MapElement::Entry(entry) => {
                            let key = self.eval(entry.key)?;
                            let value = self.eval(entry.value)?;
                            let key_str = key.to_map_key().map_err(|_| map_key_not_hashable())?;
                            map.insert(key_str, value);
                        }
                        ori_ir::MapElement::Spread { expr, .. } => {
                            let spread_val = self.eval(*expr)?;
                            if let Value::Map(spread_map) = spread_val {
                                for (k, v) in spread_map.iter() {
                                    map.insert(k.clone(), v.clone());
                                }
                            } else {
                                return Err(spread_requires_map());
                            }
                        }
                    }
                }
                Ok(Value::map(map))
            }

            // Struct literal
            ExprKind::Struct { name, fields } => {
                let field_list = self.arena.get_field_inits(*fields);
                let mut field_values: FxHashMap<Name, Value> = FxHashMap::default();
                field_values.reserve(field_list.len());
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

            // Struct literal with spread (not yet implemented in evaluator)
            ExprKind::StructWithSpread { name, fields } => {
                // Parse the fields, applying spreads and overrides
                let field_list = self.arena.get_struct_lit_fields(*fields);
                let mut field_values: FxHashMap<Name, Value> = FxHashMap::default();

                for field in field_list {
                    match field {
                        ori_ir::StructLitField::Field(init) => {
                            let value = if let Some(v) = init.value {
                                self.eval(v)?
                            } else {
                                // Shorthand: { x } means { x: x }
                                self.env.lookup(init.name).ok_or_else(|| {
                                    let name_str = self.interner.lookup(init.name);
                                    undefined_variable(name_str)
                                })?
                            };
                            field_values.insert(init.name, value);
                        }
                        ori_ir::StructLitField::Spread { expr, .. } => {
                            // Evaluate the spread expression and merge its fields
                            let spread_val = self.eval(*expr)?;
                            if let Value::Struct(sv) = spread_val {
                                // Iterate through the layout's field indices to get names and values
                                for (field_name, idx) in sv.layout.iter() {
                                    if let Some(v) = sv.fields.get(idx) {
                                        field_values.insert(field_name, v.clone());
                                    }
                                }
                            } else {
                                return Err(EvalError::new(
                                    "spread requires a struct value".to_string(),
                                ));
                            }
                        }
                    }
                }
                Ok(Value::Struct(StructValue::new(*name, field_values)))
            }

            ExprKind::Break(v) => {
                let val = if v.is_present() {
                    self.eval(*v)?
                } else {
                    Value::Void
                };
                Err(EvalError::break_with(val))
            }
            ExprKind::Continue(v) => {
                let val = if v.is_present() {
                    self.eval(*v)?
                } else {
                    Value::Void
                };
                Err(EvalError::continue_with(val))
            }
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
            ExprKind::Cast { expr, ty, fallible } => {
                let value = self.eval(*expr)?;
                self.eval_cast(value, self.arena.get_parsed_type(*ty), *fallible)
            }
            ExprKind::Const(name) => self
                .env
                .lookup(*name)
                .ok_or_else(|| undefined_const(self.interner.lookup(*name))),
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
            ExprKind::TemplateLiteral { head, parts } => {
                let mut result = String::from(self.interner.lookup(*head));
                for part in self.arena.get_template_parts(*parts) {
                    let value = self.eval(part.expr)?;
                    result.push_str(&value.display_value());
                    result.push_str(self.interner.lookup(part.text_after));
                }
                Ok(Value::string(result))
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

    /// Evaluate a type cast: `expr as type` or `expr as? type`
    ///
    /// Handles conversions between primitive types:
    /// - int -> float, int -> byte, byte -> int, char -> int, int -> char
    /// - str -> int (with `as?`), str -> float (with `as?`)
    fn eval_cast(&self, value: Value, ty: &ori_ir::ParsedType, fallible: bool) -> EvalResult {
        // Get the target type name from the parsed type
        let target_name = match ty {
            ori_ir::ParsedType::Primitive(type_id) => type_id.name().unwrap_or("?"),
            ori_ir::ParsedType::Named { name, type_args } if type_args.is_empty() => {
                self.interner.lookup(*name)
            }
            _ => {
                return Err(EvalError::new(format!(
                    "unsupported cast target type: {ty:?}"
                )));
            }
        };

        let result = match (target_name, &value) {
            // int conversions
            #[allow(clippy::cast_precision_loss)] // intentional: int to float conversion
            ("float", Value::Int(n)) => Ok(Value::Float(n.raw() as f64)),
            ("byte", Value::Int(n)) => {
                let raw = n.raw();
                if !(0..=255).contains(&raw) {
                    if fallible {
                        return Ok(Value::None);
                    }
                    return Err(EvalError::new(format!(
                        "value {raw} out of range for byte (0-255)"
                    )));
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Ok(Value::Byte(raw as u8))
            }
            ("char", Value::Int(n)) => {
                let raw = n.raw();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                if let Some(c) = char::from_u32(raw as u32) {
                    Ok(Value::Char(c))
                } else if fallible {
                    return Ok(Value::None);
                } else {
                    return Err(EvalError::new(format!(
                        "value {raw} is not a valid Unicode codepoint"
                    )));
                }
            }

            // byte conversions
            ("int", Value::Byte(b)) => Ok(Value::int(i64::from(*b))),

            // char conversions
            ("int", Value::Char(c)) => Ok(Value::int(i64::from(*c as u32))),

            // float conversions
            #[allow(clippy::cast_possible_truncation)]
            ("int", Value::Float(f)) => Ok(Value::int(*f as i64)),

            // string parsing (always fallible semantically, but `as` will panic)
            ("int", Value::Str(s)) => match s.parse::<i64>() {
                Ok(n) => Ok(Value::int(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as int")));
                }
            },
            ("float", Value::Str(s)) => match s.parse::<f64>() {
                Ok(n) => Ok(Value::Float(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as float")));
                }
            },

            // Identity conversions (value already matches target type)
            ("int", Value::Int(_))
            | ("float", Value::Float(_))
            | ("str", Value::Str(_))
            | ("bool", Value::Bool(_))
            | ("byte", Value::Byte(_))
            | ("char", Value::Char(_)) => Ok(value),

            // str conversion - anything can become a string
            ("str", v) => Ok(Value::string(v.to_string())),

            _ => {
                if fallible {
                    return Ok(Value::None);
                }
                Err(EvalError::new(format!(
                    "cannot convert {} to {target_name}",
                    value.type_name()
                )))
            }
        };

        // For `as?`, wrap successful result in Some
        if fallible {
            result.map(Value::some)
        } else {
            result
        }
    }

    /// Evaluate a unary operation.
    fn eval_unary(&mut self, expr_id: ExprId, op: UnaryOp, operand: ExprId) -> EvalResult {
        let value = self.eval(operand)?;
        let span = self.arena.get_expr(expr_id).span;

        // Primitive types use direct evaluation (built-in operators)
        if is_primitive_value(&value) {
            return evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span));
        }

        // User-defined types dispatch unary operators through trait methods
        if let Some(method_name) = unary_op_to_method(op) {
            let method = self.interner.intern(method_name);
            return self.eval_method_call(value, method, vec![]);
        }

        // Try operator (?) doesn't have a trait
        evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span))
    }

    /// Evaluate a binary operation.
    ///
    /// Operators with trait implementations (Add, Sub, Mul, etc.) dispatch through
    /// the method system uniformly for all types. Comparison, logical, and range
    /// operators use direct evaluation.
    ///
    /// The `binary_expr_id` is the ID of the binary expression itself (not the operands),
    /// used for looking up the result type when type information is available.
    fn eval_binary(
        &mut self,
        binary_expr_id: ExprId,
        left: ExprId,
        op: BinaryOp,
        right: ExprId,
    ) -> EvalResult {
        let left_val = self.eval(left)?;
        let span = self.arena.get_expr(binary_expr_id).span;

        // Short-circuit for &&, ||, and ??
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
            BinaryOp::Coalesce => {
                // Null coalescing with type-aware behavior:
                // - Option<T> ?? Option<T> -> Option<T> (chaining: return left as-is)
                // - Option<T> ?? T -> T (unwrap: return inner value)
                // Same pattern for Result<T, E>.
                //
                // Chaining vs unwrapping is determined by comparing types:
                // - If left type == result type → chaining (return left unchanged)
                // - If left type ≠ result type → unwrapping (return inner value)
                //
                // This handles nested Options correctly:
                // - Option<Option<int>> ?? Option<int> → unwrap (types differ)
                // - Option<int> ?? Option<int> → chain (types match)
                //
                // Short-circuit: right is NOT evaluated when left is Some/Ok.
                let is_chaining = self.types_match(left, binary_expr_id) == Some(true);

                match left_val {
                    Value::Some(inner) => {
                        // If left type == result type, we're chaining - return left unchanged
                        if is_chaining {
                            return Ok(Value::Some(inner));
                        }
                        // Otherwise unwrap (return inner value)
                        return Ok((*inner).clone());
                    }
                    Value::Ok(inner) => {
                        // If left type == result type, we're chaining - return left unchanged
                        if is_chaining {
                            return Ok(Value::Ok(inner));
                        }
                        // Otherwise unwrap (return inner value)
                        return Ok((*inner).clone());
                    }
                    Value::None | Value::Err(_) => {
                        return self.eval(right);
                    }
                    _ => {
                        let err = EvalError::new(format!(
                            "operator '??' requires Option or Result, got {}",
                            left_val.type_name()
                        ));
                        return Err(Self::attach_span(err, span));
                    }
                }
            }
            _ => {}
        }

        let right_val = self.eval(right)?;

        // Primitive types use direct evaluation (built-in operators)
        // User-defined types dispatch through operator trait methods
        if is_primitive_value(&left_val) {
            return evaluate_binary(left_val, right_val, op)
                .map_err(|e| Self::attach_span(e, span));
        }

        // For user-defined types, dispatch arithmetic/bitwise operators through trait methods
        if let Some(method_name) = binary_op_to_method(op) {
            let method = self.interner.intern(method_name);
            return self.eval_method_call(left_val, method, vec![right_val]);
        }

        // Comparison, range, and null-coalescing operators use direct evaluation
        evaluate_binary(left_val, right_val, op).map_err(|e| Self::attach_span(e, span))
    }

    /// Evaluate an expression with # (`HashLength`) resolved to a specific length.
    fn eval_with_hash_length(&mut self, expr_id: ExprId, length: i64) -> EvalResult {
        let expr = self.arena.get_expr(expr_id);
        let span = expr.span;
        match &expr.kind {
            ExprKind::HashLength => Ok(Value::int(length)),
            ExprKind::Binary { left, op, right } => {
                let left_val = self.eval_with_hash_length(*left, length)?;
                let right_val = self.eval_with_hash_length(*right, length)?;

                // Primitive types use direct evaluation (built-in operators)
                if is_primitive_value(&left_val) {
                    return evaluate_binary(left_val, right_val, *op)
                        .map_err(|e| Self::attach_span(e, span));
                }

                // Check if this is a mixed-type operation that needs special handling
                if is_mixed_primitive_op(&left_val, &right_val) {
                    return evaluate_binary(left_val, right_val, *op)
                        .map_err(|e| Self::attach_span(e, span));
                }

                // Dispatch through methods for operators with trait implementations
                if let Some(method_name) = binary_op_to_method(*op) {
                    let method = self.interner.intern(method_name);
                    return self.eval_method_call(left_val, method, vec![right_val]);
                }

                // Comparison, range, and null-coalescing operators use direct evaluation
                evaluate_binary(left_val, right_val, *op).map_err(|e| Self::attach_span(e, span))
            }
            _ => self.eval(expr_id),
        }
    }

    /// Evaluate a block of statements.
    fn eval_block(&mut self, stmts: ori_ir::StmtRange, result: ExprId) -> EvalResult {
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
                        let pat = eval.arena.get_binding_pattern(*pattern);
                        let value = eval.eval(*init)?;
                        let mutability = if *mutable {
                            Mutability::Mutable
                        } else {
                            Mutability::Immutable
                        };
                        eval.bind_pattern(pat, value, mutability)?;
                    }
                }
            }
            if result.is_present() {
                eval.eval(result)
            } else {
                Ok(Value::Void)
            }
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

        let arm_range_start = arms.start;
        let arm_list = self.arena.get_arms(arms);

        for (i, arm) in arm_list.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::arithmetic_side_effects,
                reason = "arm count bounded by AST size, cannot overflow u32"
            )]
            let arm_key = ori_types::PatternKey::Arm(arm_range_start + i as u32);

            // Try to match the pattern using the exec module
            if let Some(bindings) = try_match(
                &arm.pattern,
                value,
                self.arena,
                self.interner,
                Some(arm_key),
                self.pattern_resolutions,
            )? {
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
        guard: ExprId,
        body: ExprId,
        is_yield: bool,
    ) -> EvalResult {
        use crate::exec::control::{to_loop_action, LoopAction};

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
                current: Option<i64>,
                end: i64,
                step: i64,
                inclusive: bool,
            },
        }

        impl Iterator for ForIterator {
            type Item = Value;

            #[expect(
                clippy::arithmetic_side_effects,
                reason = "range bound arithmetic on user-provided i64 values"
            )]
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
                    ForIterator::Range {
                        current,
                        end,
                        step,
                        inclusive,
                    } => {
                        let curr = (*current)?;
                        let next_val = curr + *step;

                        // Check if current is in bounds
                        let in_bounds = match (*step).cmp(&0) {
                            std::cmp::Ordering::Greater => {
                                if *inclusive {
                                    curr <= *end
                                } else {
                                    curr < *end
                                }
                            }
                            std::cmp::Ordering::Less => {
                                if *inclusive {
                                    curr >= *end
                                } else {
                                    curr > *end
                                }
                            }
                            std::cmp::Ordering::Equal => false, // step == 0, stop immediately
                        };

                        if in_bounds {
                            *current = Some(next_val);
                            Some(Value::int(curr))
                        } else {
                            *current = None;
                            None
                        }
                    }
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                match self {
                    ForIterator::List { list, index } => {
                        let remaining = list.len().saturating_sub(*index);
                        (remaining, Some(remaining))
                    }
                    ForIterator::Range { .. } => (0, None),
                }
            }
        }

        let items = match iter {
            Value::List(list) => ForIterator::List { list, index: 0 },
            Value::Range(range) => ForIterator::Range {
                current: Some(range.start),
                end: range.end,
                step: range.step,
                inclusive: range.inclusive,
            },
            _ => return Err(for_requires_iterable()),
        };

        if is_yield {
            use crate::exec::control::to_loop_action;

            let (lower, _) = items.size_hint();
            let mut results = Vec::with_capacity(lower);
            for item in items {
                // Use RAII guard for scope safety
                let iter_result = self.with_binding(binding, item, Mutability::Immutable, |eval| {
                    // Check guard
                    if guard.is_present() {
                        match eval.eval(guard) {
                            Ok(v) if !v.is_truthy() => return IterResult::Continue,
                            Err(e) => return IterResult::Error(e),
                            Ok(_) => {}
                        }
                    }
                    // Evaluate body and handle loop control
                    match eval.eval(body) {
                        Ok(v) => IterResult::Yield(v),
                        Err(e) => match to_loop_action(e) {
                            LoopAction::Continue => IterResult::Continue,
                            LoopAction::ContinueWith(v) => IterResult::Yield(v),
                            LoopAction::Break(v) => IterResult::Break(v),
                            LoopAction::Error(e) => IterResult::Error(e),
                        },
                    }
                });

                match iter_result {
                    IterResult::Continue => {}
                    IterResult::Yield(v) => results.push(v),
                    IterResult::Break(v) => {
                        // For for...yield, break value adds final element
                        if !matches!(v, Value::Void) {
                            results.push(v);
                        }
                        return Ok(Value::list(results));
                    }
                    IterResult::Error(e) => return Err(e),
                }
            }
            Ok(Value::list(results))
        } else {
            for item in items {
                // Use RAII guard for scope safety
                let iter_result = self.with_binding(binding, item, Mutability::Immutable, |eval| {
                    // Check guard
                    if guard.is_present() {
                        match eval.eval(guard) {
                            Ok(v) if !v.is_truthy() => return IterResult::Continue,
                            Err(e) => return IterResult::Error(e),
                            Ok(_) => {}
                        }
                    }
                    // Evaluate body and handle loop control
                    match eval.eval(body) {
                        Ok(_) => IterResult::Continue,
                        Err(e) => match to_loop_action(e) {
                            LoopAction::Continue | LoopAction::ContinueWith(_) => {
                                IterResult::Continue
                            }
                            LoopAction::Break(v) => IterResult::Break(v),
                            LoopAction::Error(e) => IterResult::Error(e),
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
        use crate::exec::control::{to_loop_action, LoopAction};
        loop {
            match self.eval(body) {
                Ok(_) => {}
                Err(e) => match to_loop_action(e) {
                    LoopAction::Continue | LoopAction::ContinueWith(_) => {}
                    LoopAction::Break(v) => return Ok(v),
                    LoopAction::Error(e) => return Err(e),
                },
            }
        }
    }

    /// Evaluate an assignment using `exec::control` module.
    fn eval_assign(&mut self, target: ExprId, value: Value) -> EvalResult {
        crate::exec::control::eval_assign(target, value, self.arena, self.interner, &mut self.env)
    }

    /// Dispatch a method call, handling `ModuleNamespace` specially.
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
    #[cfg(target_arch = "wasm32")]
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
            .call_depth(self.call_depth.saturating_add(1))
            .max_call_depth(self.max_call_depth)
            .pattern_resolutions(self.pattern_resolutions)
            .with_scoped_env_ownership() // RAII: scope will be popped when interpreter drops
            .build()
    }

    #[cfg(not(target_arch = "wasm32"))]
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
            .call_depth(self.call_depth.saturating_add(1))
            .pattern_resolutions(self.pattern_resolutions)
            .with_scoped_env_ownership() // RAII: scope will be popped when interpreter drops
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

    /// Register all `function_val` (type conversion) functions and built-in values.
    ///
    /// Includes:
    /// - Type conversion functions like int(x), str(x), float(x) (positional args per spec)
    /// - Built-in enum variants like Less, Equal, Greater (Ordering type)
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

        // Built-in Ordering enum variants (Less, Equal, Greater)
        // These are first-class Ordering values, used by compare() and comparison operators
        let less_name = self.interner.intern("Less");
        let equal_name = self.interner.intern("Equal");
        let greater_name = self.interner.intern("Greater");

        self.env.define_global(less_name, Value::ordering_less());
        self.env.define_global(equal_name, Value::ordering_equal());
        self.env
            .define_global(greater_name, Value::ordering_greater());
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

/// Check if this is a mixed-type operation between primitives that needs special handling.
///
/// Mixed-type operations like `int * Duration`, `int * Size`, etc. cannot be dispatched
/// through the method system (e.g., `int.mul(Duration)` doesn't exist) and must use
/// direct evaluation.
fn is_mixed_primitive_op(left: &Value, right: &Value) -> bool {
    matches!(
        (left, right),
        // int <op> Duration/Size or Duration/Size <op> int
        (Value::Int(_), Value::Duration(_) | Value::Size(_))
            | (Value::Duration(_) | Value::Size(_), Value::Int(_))
    )
}

/// Check if a value is a primitive type that uses built-in operator evaluation.
///
/// Primitive types (int, float, bool, str, char, byte, Duration, Size) use direct
/// evaluation via `evaluate_binary`. User-defined types dispatch through operator
/// trait methods (`Add::add`, `Sub::subtract`, `Mul::multiply`, etc.).
fn is_primitive_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Int(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Str(_)
            | Value::Char(_)
            | Value::Byte(_)
            | Value::Duration(_)
            | Value::Size(_)
            | Value::List(_)
            | Value::Tuple(_)
            | Value::Map(_)
            | Value::Some(_)
            | Value::None
            | Value::Ok(_)
            | Value::Err(_)
            | Value::Range(_)
    )
}

/// Map a binary operator to its trait method name.
///
/// Returns `Some(method_name)` for operators that have trait implementations,
/// or `None` for comparison, logical, range, and null-coalescing operators
/// which use direct evaluation.
fn binary_op_to_method(op: BinaryOp) -> Option<&'static str> {
    match op {
        // Arithmetic operators
        BinaryOp::Add => Some("add"),
        BinaryOp::Sub => Some("subtract"),
        BinaryOp::Mul => Some("multiply"),
        // Note: "divide" not "div" because `div` is a keyword (floor division operator)
        BinaryOp::Div => Some("divide"),
        BinaryOp::FloorDiv => Some("floor_divide"),
        BinaryOp::Mod => Some("remainder"),
        // Bitwise operators
        BinaryOp::BitAnd => Some("bit_and"),
        BinaryOp::BitOr => Some("bit_or"),
        BinaryOp::BitXor => Some("bit_xor"),
        BinaryOp::Shl => Some("shift_left"),
        BinaryOp::Shr => Some("shift_right"),
        // Comparison, logical, range, and null-coalescing operators
        // use direct evaluation (no trait method)
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Coalesce => None,
    }
}

/// Map a unary operator to its trait method name.
///
/// Returns `Some(method_name)` for operators that have trait implementations,
/// or `None` for the Try operator which doesn't have a trait.
fn unary_op_to_method(op: UnaryOp) -> Option<&'static str> {
    match op {
        UnaryOp::Neg => Some("negate"),
        UnaryOp::Not => Some("not"),
        UnaryOp::BitNot => Some("bit_not"),
        UnaryOp::Try => None, // Try operator doesn't have a trait
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
