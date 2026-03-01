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
//! # Architecture
//!
//! All evaluation goes through `eval_can(CanId)` in `can_eval.rs`. The canonical
//! IR (`CanExpr`) is the sole evaluation representation. Helper modules in
//! `crate::exec` provide shared utilities:
//!
//! - `exec::expr` - Identifiers, indexing, field access, ranges
//! - `exec::call` - Function calls, argument binding
//! - `exec::control` - Pattern matching, loop actions, assignment
//! - `exec::decision_tree` - Decision tree evaluation for multi-clause functions
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
mod can_eval;
mod derived_methods;
mod format;
mod function_call;
mod interned_names;
mod method_dispatch;
mod operator_dispatch;
mod prelude;
pub mod resolvers;
mod scope_guard;

pub use builder::InterpreterBuilder;
pub use scope_guard::ScopedInterpreter;

#[allow(
    clippy::disallowed_types,
    reason = "Arc<String> shared across child interpreters"
)]
use std::sync::Arc;

use crate::errors::no_member_in_module;
use crate::eval_mode::{EvalMode, ModeState};
use crate::print_handler::SharedPrintHandler;
use crate::{Environment, Mutability, SharedMutableRegistry, UserMethodRegistry, Value};
use ori_ir::canon::SharedCanonResult;
use ori_ir::{ExprArena, ExprId, Name, SharedArena, StringInterner};
use ori_patterns::{
    recursion_limit_exceeded, ControlAction, EvalError, EvalResult, PatternExecutor,
};

pub(crate) use interned_names::{FormatNames, OpNames, PrintNames, PropNames, TypeNames};

/// Whether this interpreter owns a scoped environment that should be popped on drop.
///
/// Replaces a bare `bool` flag for self-documenting intent at construction sites.
/// - `Borrowed`: No scope cleanup on drop (default for top-level interpreters).
/// - `Owned`: Pop the environment scope on drop (for function/method call interpreters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopeOwnership {
    /// This interpreter does not own a scope. No cleanup on drop.
    Borrowed,
    /// This interpreter owns a pushed scope. Pop it on drop (RAII panic safety).
    Owned,
}

/// Tree-walking interpreter for Ori expressions.
///
/// This is the portable interpreter that works in both native and WASM contexts.
/// For Salsa-integrated evaluation with imports, see `oric::Evaluator`.
///
/// # Evaluation Modes
///
/// The interpreter's behavior is parameterized by `EvalMode`:
/// - `Interpret` — full I/O for `ori run`
/// - `ConstEval` — budget-limited, no I/O, deterministic
/// - `TestRun` — captures output, collects test results
#[allow(
    clippy::disallowed_types,
    reason = "Arc<String> for source metadata shared across children"
)]
pub struct Interpreter<'a> {
    /// String interner for name lookup.
    pub(crate) interner: &'a StringInterner,
    /// Expression arena.
    pub(crate) arena: &'a ExprArena,
    /// Current environment.
    pub(crate) env: Environment,
    /// Pre-computed Name for "self" keyword (avoids repeated interning).
    pub(crate) self_name: Name,
    /// Pre-interned type names for hot-path method dispatch.
    /// Avoids repeated `intern()` calls in `get_value_type_name()`.
    pub(crate) type_names: TypeNames,
    /// Pre-interned print method names for `eval_method_call` print dispatch.
    pub(crate) print_names: PrintNames,
    /// Pre-interned `FunctionExp` property names for prop dispatch.
    pub(crate) prop_names: PropNames,
    /// Pre-interned operator trait method names for user-defined operator dispatch.
    pub(crate) op_names: OpNames,
    /// Pre-interned format-related names for `FormatSpec` value construction.
    pub(crate) format_names: FormatNames,
    /// Pre-interned builtin method names for `Name`-based dispatch.
    /// Avoids de-interning in `dispatch_builtin_method` on every call.
    pub(crate) builtin_method_names: crate::methods::BuiltinMethodNames,
    /// Evaluation mode — determines I/O, recursion, budget policies.
    pub(crate) mode: EvalMode,
    /// Per-mode mutable state (budget counters, profiling).
    ///
    /// Tracks call budget for `ConstEval` mode and optional performance counters
    /// for `--profile`. Counter increments are inlined no-ops when profiling is off.
    pub(crate) mode_state: ModeState,
    /// Live call stack for recursion tracking and backtrace capture.
    ///
    /// Replaces the old `call_depth: usize` with proper frame tracking.
    /// Depth is checked on `push()` against `mode.max_recursion_depth()`.
    /// On error, `capture()` produces an `EvalBacktrace` for diagnostics.
    pub(crate) call_stack: crate::diagnostics::CallStack,
    /// User-defined method registry for impl block methods.
    ///
    /// Uses `SharedMutableRegistry` to allow method registration after the
    /// evaluator (and its cached dispatcher) is created.
    pub(crate) user_method_registry: SharedMutableRegistry<UserMethodRegistry>,
    /// Default field type registry for `#[derive(Default)]`.
    ///
    /// Stored separately from `DerivedMethodInfo` because default field types
    /// are evaluator-specific — LLVM codegen uses `const_zero` instead.
    pub(crate) default_field_types: SharedMutableRegistry<crate::DefaultFieldTypeRegistry>,
    /// Cached method dispatcher for efficient method resolution.
    ///
    /// The dispatcher chains all method resolvers (user, derived, collection, builtin)
    /// and is built once during construction. Because `user_method_registry` uses
    /// interior mutability, the dispatcher sees method registrations made after creation.
    pub(crate) method_dispatcher: resolvers::MethodDispatcher,
    /// Shared arena for imported functions and lambda capture.
    ///
    /// Always set — either provided explicitly via the builder or created from
    /// the borrowed arena at build time. Lambdas capture this via O(1) Arc clone.
    pub(crate) imported_arena: SharedArena,
    /// Print handler for the Print capability.
    ///
    /// Determined by evaluation mode:
    /// - `Interpret`: stdout (default)
    /// - `TestRun`: buffer for capture
    /// - `ConstEval`: silent (discards output)
    pub(crate) print_handler: SharedPrintHandler,
    /// Scope ownership for RAII-style panic-safe scope cleanup.
    ///
    /// When `Owned`, the interpreter was created for a function/method call
    /// via `create_function_interpreter` and will pop its environment scope on drop.
    pub(crate) scope_ownership: ScopeOwnership,
    /// Source file path for Traceable trait trace entries.
    ///
    /// Set by `oric` when creating the top-level interpreter. Propagated to
    /// child interpreters in `create_function_interpreter()`.
    pub(crate) source_file_path: Option<Arc<String>>,
    /// Source text for Traceable trait trace entries.
    ///
    /// Used to compute line/column from byte offsets in spans. Set by `oric`
    /// and propagated to child interpreters.
    pub(crate) source_text: Option<Arc<String>>,
    /// Canonical IR for the current module (optional during migration).
    ///
    /// When present, function calls on `FunctionValue`s with canonical bodies
    /// dispatch via `eval_can()` instead of `eval()`. This enables incremental
    /// migration from `ExprArena` to `CanonResult` without a big-bang rewrite.
    pub(crate) canon: Option<SharedCanonResult>,
}

/// RAII Drop implementation for panic-safe scope cleanup.
///
/// When `scope_ownership` is `Owned`, this interpreter was created for a function/method
/// call and owns a scope that must be popped. This ensures scope cleanup even if
/// evaluation panics during the call.
impl Drop for Interpreter<'_> {
    fn drop(&mut self) {
        if self.scope_ownership == ScopeOwnership::Owned {
            self.env.pop_scope();
        }
    }
}

/// Implement `PatternExecutor` for Interpreter.
///
/// This allows patterns to request function calls and variable operations
/// without needing direct access to the interpreter's internals.
///
/// Note: `eval(ExprId)` is no longer supported — all evaluation goes through
/// `eval_can(CanId)`. The trait method returns an error if called. All
/// `FunctionExpKind` variants (Print, Panic, Catch, Recurse, Cache, Parallel,
/// Spawn, Timeout, With) are now dispatched inline in `can_eval.rs`, so the
/// `ori_patterns` execute functions (`fusion.rs`, `parallel.rs`, `spawn.rs`,
/// `with_pattern.rs`, `recurse.rs`) are never reached through this Interpreter.
///
/// The trait now uses `Name` directly, so the impl is a zero-cost pass-through
/// with no redundant interning.
impl PatternExecutor for Interpreter<'_> {
    /// Dead code path — returns an error unconditionally.
    ///
    /// This method is required by the `PatternExecutor` trait but is never called
    /// in practice. The `ori_eval` Interpreter evaluates all expressions through
    /// `eval_can(CanId)` (canonical IR), not `eval(ExprId)` (raw AST). All
    /// `FunctionExpKind` patterns are dispatched inline in `can_eval.rs`.
    ///
    /// **Removal:** Once `ori_patterns` consumers migrate to canonical IR,
    /// `PatternExecutor::eval(ExprId)` can be removed from the trait (cross-crate).
    fn eval(&mut self, _expr_id: ExprId) -> EvalResult {
        Err(EvalError::new(
            "legacy PatternExecutor::eval(ExprId) is not supported — use eval_can(CanId)"
                .to_string(),
        )
        .into())
    }

    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, &args)
    }

    fn lookup_capability(&self, name: Name) -> Option<Value> {
        self.env.lookup(name)
    }

    fn call_method(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        self.eval_method_call(receiver, method, args)
    }

    fn lookup_var(&self, name: Name) -> Option<Value> {
        self.env.lookup(name)
    }

    fn bind_var(&mut self, name: Name, value: Value) {
        self.env.define(name, value, Mutability::Immutable);
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
    /// Depth is tracked via `call_stack.depth()`. Frame names for backtraces
    /// are populated by `create_function_interpreter()` (placeholder names
    /// for now; proper function names in Section 07 with `CanExpr` context).
    ///
    /// The limit is determined by `EvalMode::max_recursion_depth()`:
    /// - `Interpret` (native): `None` — stacker grows the stack dynamically
    /// - `Interpret` (WASM): `Some(200)` — fixed stack
    /// - `ConstEval`: `Some(64)` — tight budget
    /// - `TestRun`: `Some(500)` — generous but bounded
    #[inline]
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        if let Some(max_depth) = self.mode.max_recursion_depth() {
            if self.call_stack.depth() >= max_depth {
                return Err(recursion_limit_exceeded(max_depth));
            }
        }
        Ok(())
    }

    /// Get the string interner.
    #[inline]
    pub fn interner(&self) -> &StringInterner {
        self.interner
    }

    /// Get the current evaluation mode.
    #[inline]
    pub fn mode(&self) -> &EvalMode {
        &self.mode
    }

    /// Enable performance counters for `--profile` mode.
    ///
    /// Must be called before evaluation begins. When enabled, expression,
    /// function call, method call, and pattern match counts are tracked.
    /// When disabled (default), all counter increments are inlined no-ops.
    pub fn enable_counters(&mut self) {
        self.mode_state.enable_counters();
    }

    /// Get the counter report string, if counters are enabled.
    ///
    /// Returns `None` when profiling is off (default).
    pub fn counters_report(&self) -> Option<String> {
        self.mode_state
            .counters()
            .map(crate::diagnostics::EvalCounters::report)
    }

    /// Attach a span to an error if it doesn't already have one.
    ///
    /// Only attaches spans to `ControlAction::Error` variants; control flow
    /// signals (Break, Continue, Propagate) pass through unchanged.
    #[inline]
    fn attach_span(action: ControlAction, span: ori_ir::Span) -> ControlAction {
        action.with_span_if_error(span)
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

    /// Get the expression arena.
    pub fn arena(&self) -> &ExprArena {
        self.arena
    }

    /// Get the user method registry.
    pub fn user_method_registry(&self) -> &SharedMutableRegistry<UserMethodRegistry> {
        &self.user_method_registry
    }

    /// Get the default field type registry.
    pub fn default_field_types(&self) -> &SharedMutableRegistry<crate::DefaultFieldTypeRegistry> {
        &self.default_field_types
    }

    /// Get a reference to the environment.
    pub fn env(&self) -> &Environment {
        &self.env
    }

    /// Get a mutable reference to the environment.
    pub fn env_mut(&mut self) -> &mut Environment {
        &mut self.env
    }

    /// Look up a canonical root by name.
    ///
    /// Returns the `CanId` for a named root (function or test body) if canonical
    /// IR is available and the name exists in the roots list.
    pub fn canon_root_for(&self, name: Name) -> Option<ori_ir::canon::CanId> {
        self.canon.as_ref().and_then(|c| c.root_for(name))
    }

    /// Create an interpreter for function/method body evaluation.
    ///
    /// This helper implements the arena threading pattern: the callee's arena
    /// is used to evaluate the body, ensuring expression IDs are valid and
    /// enabling thread-safe parallel evaluation.
    ///
    /// # Arguments
    /// * `imported_arena` - The callee's shared arena (O(1) Arc clone, not deep copy)
    /// * `call_env` - The environment with parameters bound
    /// * `call_name` - Name for the call stack frame
    /// * `canon` - Canonical IR for the callee. When `Some`, uses the callee's
    ///   canon directly instead of cloning the parent's (avoids a wasted Arc clone
    ///   that would be immediately overwritten).
    ///
    /// # Returns
    /// A new interpreter configured to evaluate the function body.
    /// The returned interpreter's lifetime is tied to `imported_arena`, not `self`,
    /// but requires that `'a` outlives `'b` since we pass the interner through.
    pub(crate) fn create_function_interpreter<'b>(
        &self,
        imported_arena: &'b SharedArena,
        call_env: Environment,
        call_name: Name,
        canon: Option<SharedCanonResult>,
    ) -> Interpreter<'b>
    where
        'a: 'b,
    {
        // Clone the parent's call stack and push a frame for this call.
        // The depth check in check_recursion_limit() has already passed,
        // so this push cannot fail (depth < max at the check point).
        let mut child_stack = self.call_stack.clone();
        // Invariant: check_recursion_limit() passed, so depth < max_depth.
        // push() cannot fail here.
        #[expect(
            clippy::expect_used,
            reason = "Invariant: check_recursion_limit already passed"
        )]
        child_stack
            .push(crate::diagnostics::CallFrame {
                name: call_name,
                call_span: None,
            })
            .expect("check_recursion_limit passed but CallStack::push failed");

        Interpreter {
            interner: self.interner,
            arena: imported_arena,
            env: call_env,
            self_name: self.self_name,
            type_names: self.type_names,
            print_names: self.print_names,
            prop_names: self.prop_names,
            op_names: self.op_names,
            format_names: self.format_names,
            builtin_method_names: self.builtin_method_names,
            source_file_path: self.source_file_path.clone(),
            source_text: self.source_text.clone(),
            mode: self.mode,
            mode_state: ModeState::child(&self.mode, &self.mode_state),
            call_stack: child_stack,
            user_method_registry: self.user_method_registry.clone(),
            default_field_types: self.default_field_types.clone(),
            method_dispatcher: self.method_dispatcher.clone(),
            imported_arena: imported_arena.clone(),
            print_handler: self.print_handler.clone(),
            scope_ownership: ScopeOwnership::Owned,
            canon: canon.or_else(|| self.canon.clone()),
        }
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
mod tests;
