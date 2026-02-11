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
mod function_call;
mod method_dispatch;
pub mod resolvers;
mod scope_guard;

pub use builder::InterpreterBuilder;
pub use scope_guard::ScopedInterpreter;

use crate::eval_mode::{EvalMode, ModeState};
use crate::print_handler::SharedPrintHandler;
use crate::{
    no_member_in_module, Environment, Mutability, SharedMutableRegistry, UserMethodRegistry, Value,
};
use ori_ir::canon::SharedCanonResult;
use ori_ir::{BinaryOp, ExprArena, ExprId, Name, SharedArena, StringInterner, UnaryOp};
use ori_patterns::{
    recursion_limit_exceeded, ControlAction, EvalError, EvalResult, PatternExecutor,
};

/// Pre-interned print method names for print dispatch in `eval_method_call`.
///
/// These names are interned once at Interpreter construction so that
/// `eval_method_call` can check for print methods via `Name` comparison
/// (a single `u32 == u32` check) instead of string lookup.
#[derive(Clone, Copy)]
pub(crate) struct PrintNames {
    pub(crate) print: Name,
    pub(crate) println: Name,
    pub(crate) builtin_print: Name,
    pub(crate) builtin_println: Name,
}

impl PrintNames {
    /// Pre-intern all print method names.
    fn new(interner: &StringInterner) -> Self {
        Self {
            print: interner.intern("print"),
            println: interner.intern("println"),
            builtin_print: interner.intern("__builtin_print"),
            builtin_println: interner.intern("__builtin_println"),
        }
    }
}

/// Pre-interned type names for hot-path method dispatch.
///
/// These names are interned once at Interpreter construction to avoid
/// repeated hash lookups in `get_value_type_name()`, which is called
/// on every method dispatch.
#[derive(Clone, Copy)]
pub(crate) struct TypeNames {
    pub(crate) range: Name,
    pub(crate) int: Name,
    pub(crate) float: Name,
    pub(crate) bool_: Name,
    pub(crate) str_: Name,
    pub(crate) char_: Name,
    pub(crate) byte: Name,
    pub(crate) void: Name,
    pub(crate) duration: Name,
    pub(crate) size: Name,
    pub(crate) ordering: Name,
    pub(crate) list: Name,
    pub(crate) map: Name,
    pub(crate) tuple: Name,
    pub(crate) option: Name,
    pub(crate) result: Name,
    pub(crate) function: Name,
    pub(crate) function_val: Name,
    pub(crate) module: Name,
    pub(crate) error: Name,
}

impl TypeNames {
    /// Pre-intern all primitive type names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
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
pub struct Interpreter<'a> {
    /// String interner for name lookup.
    pub interner: &'a StringInterner,
    /// Expression arena.
    pub(crate) arena: &'a ExprArena,
    /// Current environment.
    pub env: Environment,
    /// Pre-computed Name for "self" keyword (avoids repeated interning).
    pub(crate) self_name: Name,
    /// Pre-interned type names for hot-path method dispatch.
    /// Avoids repeated `intern()` calls in `get_value_type_name()`.
    pub(crate) type_names: TypeNames,
    /// Pre-interned print method names for `eval_method_call` print dispatch.
    pub(crate) print_names: PrintNames,
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
    /// Cached method dispatcher for efficient method resolution.
    ///
    /// The dispatcher chains all method resolvers (user, derived, collection, builtin)
    /// and is built once during construction. Because `user_method_registry` uses
    /// interior mutability, the dispatcher sees method registrations made after creation.
    pub(crate) method_dispatcher: resolvers::MethodDispatcher,
    /// Arena reference for imported functions.
    ///
    /// When evaluating an imported function, this holds the imported arena.
    /// Lambdas created during evaluation will inherit this arena reference.
    pub(crate) imported_arena: Option<SharedArena>,
    /// Whether the prelude has been auto-loaded.
    /// Used by `oric::Evaluator` when module loading is enabled.
    #[expect(
        dead_code,
        reason = "Used by oric::Evaluator for prelude tracking, not exposed via ori_eval"
    )]
    pub(crate) prelude_loaded: bool,
    /// Print handler for the Print capability.
    ///
    /// Determined by evaluation mode:
    /// - `Interpret`: stdout (default)
    /// - `TestRun`: buffer for capture
    /// - `ConstEval`: silent (discards output)
    pub(crate) print_handler: SharedPrintHandler,
    /// Whether this interpreter owns a scoped environment that should be popped on drop.
    ///
    /// When an interpreter is created for function/method calls via `create_function_interpreter`,
    /// the caller pushes a scope before passing the environment. This flag ensures the scope
    /// is popped when the interpreter is dropped, even if evaluation panics.
    ///
    /// This provides RAII-style panic safety for function call evaluation.
    pub(crate) owns_scoped_env: bool,
    /// Resolved pattern disambiguations from the type checker.
    ///
    /// Used by `try_match` to distinguish `Binding("Pending")` (unit variant)
    /// from `Binding("x")` (variable). Sorted by `PatternKey` for binary search.
    pub(crate) pattern_resolutions: &'a [(ori_types::PatternKey, ori_types::PatternResolution)],
    /// Canonical IR for the current module (optional during migration).
    ///
    /// When present, function calls on `FunctionValue`s with canonical bodies
    /// dispatch via `eval_can()` instead of `eval()`. This enables incremental
    /// migration from `ExprArena` to `CanonResult` without a big-bang rewrite.
    pub(crate) canon: Option<SharedCanonResult>,
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

/// Implement `PatternExecutor` for Interpreter.
///
/// This allows patterns to request function calls and variable operations
/// without needing direct access to the interpreter's internals.
///
/// Note: `eval(ExprId)` is no longer supported — all evaluation goes through
/// `eval_can(CanId)`. The trait method panics if called. Pattern implementations
/// that previously called `exec.eval()` are now handled inline in `can_eval.rs`.
impl PatternExecutor for Interpreter<'_> {
    fn eval(&mut self, _expr_id: ExprId) -> EvalResult {
        panic!(
            "legacy PatternExecutor::eval(ExprId) called — \
             all evaluation must use eval_can(CanId)"
        );
    }

    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult {
        self.eval_call(func, &args)
    }

    fn lookup_capability(&self, name: &str) -> Option<Value> {
        let name_id = self.interner.intern(name);
        self.env.lookup(name_id)
    }

    fn call_method(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
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
            mode: self.mode.clone(),
            mode_state: ModeState::new(&self.mode),
            call_stack: child_stack,
            user_method_registry: self.user_method_registry.clone(),
            method_dispatcher: self.method_dispatcher.clone(),
            imported_arena: Some(imported_arena.clone()),
            prelude_loaded: false,
            print_handler: self.print_handler.clone(),
            owns_scoped_env: true,
            pattern_resolutions: self.pattern_resolutions,
            canon: self.canon.clone(),
        }
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
