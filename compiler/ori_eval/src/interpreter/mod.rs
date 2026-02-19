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
mod method_dispatch;
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
    pub(crate) set: Name,
    pub(crate) tuple: Name,
    pub(crate) option: Name,
    pub(crate) result: Name,
    pub(crate) function: Name,
    pub(crate) function_val: Name,
    pub(crate) iterator: Name,
    pub(crate) module: Name,
    pub(crate) error: Name,
}

/// Pre-interned property names for `FunctionExp` prop dispatch.
///
/// These names are interned once at Interpreter construction so that
/// `find_prop_value` and `find_prop_can_id` can compare `Name` values
/// directly (single `u32 == u32`) instead of string lookup per prop.
#[derive(Clone, Copy)]
pub(crate) struct PropNames {
    pub(crate) msg: Name,
    pub(crate) operation: Name,
    pub(crate) tasks: Name,
    pub(crate) acquire: Name,
    pub(crate) action: Name,
    pub(crate) release: Name,
    pub(crate) expr: Name,
    pub(crate) condition: Name,
    pub(crate) base: Name,
    pub(crate) step: Name,
    pub(crate) memo: Name,
}

impl PropNames {
    /// Pre-intern all `FunctionExp` property names.
    fn new(interner: &StringInterner) -> Self {
        Self {
            msg: interner.intern("msg"),
            operation: interner.intern("operation"),
            tasks: interner.intern("tasks"),
            acquire: interner.intern("acquire"),
            action: interner.intern("action"),
            release: interner.intern("release"),
            expr: interner.intern("expr"),
            condition: interner.intern("condition"),
            base: interner.intern("base"),
            step: interner.intern("step"),
            memo: interner.intern("memo"),
        }
    }
}

/// Pre-interned operator trait method names for user-defined operator dispatch.
///
/// These names are interned once at Interpreter construction so that
/// `eval_can_binary` and `eval_can_unary` can dispatch user-defined operator
/// trait methods via `Name` comparison instead of re-interning on every call.
#[derive(Clone, Copy)]
pub(crate) struct OpNames {
    pub(crate) add: Name,
    pub(crate) subtract: Name,
    pub(crate) multiply: Name,
    pub(crate) divide: Name,
    pub(crate) floor_divide: Name,
    pub(crate) remainder: Name,
    pub(crate) bit_and: Name,
    pub(crate) bit_or: Name,
    pub(crate) bit_xor: Name,
    pub(crate) shift_left: Name,
    pub(crate) shift_right: Name,
    pub(crate) negate: Name,
    pub(crate) not: Name,
    pub(crate) bit_not: Name,
    pub(crate) index: Name,
}

impl OpNames {
    /// Pre-intern all operator trait method names.
    fn new(interner: &StringInterner) -> Self {
        Self {
            add: interner.intern("add"),
            subtract: interner.intern("subtract"),
            multiply: interner.intern("multiply"),
            divide: interner.intern("divide"),
            floor_divide: interner.intern("floor_divide"),
            remainder: interner.intern("remainder"),
            bit_and: interner.intern("bit_and"),
            bit_or: interner.intern("bit_or"),
            bit_xor: interner.intern("bit_xor"),
            shift_left: interner.intern("shift_left"),
            shift_right: interner.intern("shift_right"),
            negate: interner.intern("negate"),
            not: interner.intern("not"),
            bit_not: interner.intern("bit_not"),
            index: interner.intern("index"),
        }
    }
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
            set: interner.intern("Set"),
            tuple: interner.intern("tuple"),
            option: interner.intern("Option"),
            result: interner.intern("Result"),
            function: interner.intern("function"),
            function_val: interner.intern("function_val"),
            iterator: interner.intern("Iterator"),
            module: interner.intern("module"),
            error: interner.intern("error"),
        }
    }
}

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
            function_val_byte, function_val_error, function_val_float, function_val_hash_combine,
            function_val_int, function_val_repeat, function_val_str, function_val_thread_id,
        };

        // Type conversion functions (positional args allowed per spec)
        self.register_function_val("str", function_val_str, "str");
        self.register_function_val("int", function_val_int, "int");
        self.register_function_val("float", function_val_float, "float");
        self.register_function_val("byte", function_val_byte, "byte");

        // Error constructor (Traceable errors with trace storage)
        self.register_function_val("Error", function_val_error, "Error");

        // Iterator constructors
        self.register_function_val("repeat", function_val_repeat, "repeat");

        // Hash utility (wrapping arithmetic — can't be pure Ori due to overflow)
        self.register_function_val("hash_combine", function_val_hash_combine, "hash_combine");

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

        // Built-in format spec enum variants (§3.16 Formattable)
        self.register_format_variants();
    }

    /// Register `Alignment`, `Sign`, and `FormatType` enum variants as globals.
    ///
    /// These unit variants are used by the `Formattable` trait's `FormatSpec` struct.
    /// Uses the generic `Value::Variant` representation (not a dedicated Value variant)
    /// since format spec types are only used in formatting, not in hot-path operators.
    fn register_format_variants(&mut self) {
        let alignment = self.interner.intern("Alignment");
        for name in ["Left", "Center", "Right"] {
            let n = self.interner.intern(name);
            self.env
                .define_global(n, Value::variant(alignment, n, vec![]));
        }

        let sign = self.interner.intern("Sign");
        for name in ["Plus", "Minus", "Space"] {
            let n = self.interner.intern(name);
            self.env.define_global(n, Value::variant(sign, n, vec![]));
        }

        let format_type = self.interner.intern("FormatType");
        for name in [
            "Binary", "Octal", "Hex", "HexUpper", "Exp", "ExpUpper", "Fixed", "Percent",
        ] {
            let n = self.interner.intern(name);
            self.env
                .define_global(n, Value::variant(format_type, n, vec![]));
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

/// Check if a value is a built-in indexable type (list, map, str, tuple).
///
/// These types use the fast-path direct indexing with `#` hash-length support.
/// User-defined types dispatch through `Index` trait methods instead.
fn is_builtin_indexable(value: &Value) -> bool {
    matches!(
        value,
        Value::List(_) | Value::Map(_) | Value::Str(_) | Value::Tuple(_)
    )
}

/// Map a binary operator to its pre-interned trait method name.
///
/// Returns `Some(Name)` for operators that have trait implementations,
/// or `None` for comparison, logical, range, and null-coalescing operators
/// which use direct evaluation.
fn binary_op_to_method(op: BinaryOp, names: OpNames) -> Option<Name> {
    match op {
        // Arithmetic operators
        BinaryOp::Add => Some(names.add),
        BinaryOp::Sub => Some(names.subtract),
        BinaryOp::Mul => Some(names.multiply),
        // Note: "divide" not "div" because `div` is a keyword (floor division operator)
        BinaryOp::Div => Some(names.divide),
        BinaryOp::FloorDiv => Some(names.floor_divide),
        BinaryOp::Mod => Some(names.remainder),
        // Bitwise operators
        BinaryOp::BitAnd => Some(names.bit_and),
        BinaryOp::BitOr => Some(names.bit_or),
        BinaryOp::BitXor => Some(names.bit_xor),
        BinaryOp::Shl => Some(names.shift_left),
        BinaryOp::Shr => Some(names.shift_right),
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

/// Map a unary operator to its pre-interned trait method name.
///
/// Returns `Some(Name)` for operators that have trait implementations,
/// or `None` for the Try operator which doesn't have a trait.
fn unary_op_to_method(op: UnaryOp, names: OpNames) -> Option<Name> {
    match op {
        UnaryOp::Neg => Some(names.negate),
        UnaryOp::Not => Some(names.not),
        UnaryOp::BitNot => Some(names.bit_not),
        UnaryOp::Try => None, // Try operator doesn't have a trait
    }
}

// Tests for full expression evaluation are in oric/src/eval/evaluator/tests.rs
// since they require the Salsa database infrastructure.

#[cfg(test)]
mod tests;
