# Evaluator Overview

The Sigil evaluator is a tree-walking interpreter that executes typed ASTs. It handles expression evaluation, function calls, pattern execution, and module loading.

## Location

```
compiler/sigilc/src/eval/
├── mod.rs              # Module exports
├── evaluator/          # Main evaluator
│   ├── mod.rs              # Evaluator struct, eval dispatch, arena threading
│   ├── builder.rs          # EvaluatorBuilder with MethodDispatcher construction
│   ├── scope_guard.rs      # RAII scope management (with_env_scope, with_bindings)
│   ├── module_loading.rs   # load_module, load_prelude, method collection
│   ├── function_call.rs    # eval_call, eval_call_named
│   ├── method_dispatch.rs  # Method dispatch, iterator helpers, type resolution
│   ├── derived_methods.rs  # Derived method evaluation (Eq, Clone, Hash, etc.)
│   ├── function_seq.rs     # eval_function_seq (run, try, match)
│   ├── resolvers/          # Method resolution chain (Chain of Responsibility)
│   │   ├── mod.rs          # MethodDispatcher, MethodResolver trait
│   │   ├── user_registry.rs # UserRegistryResolver (user + derived methods)
│   │   ├── collection.rs   # CollectionMethodResolver (list/range methods)
│   │   └── builtin.rs      # BuiltinMethodResolver (built-in methods)
│   └── tests.rs            # Unit tests
├── environment.rs      # Variable scoping (~408 lines)
├── operators.rs        # Binary operations (~486 lines)
├── methods.rs          # Method dispatch (~377 lines)
├── function_val.rs     # Type conversions (~104 lines)
├── output.rs           # Output types
├── value/
│   └── mod.rs          # Value type (~566 lines)
├── exec/
│   ├── mod.rs          # Execution modules
│   ├── expr.rs         # Expression evaluation (~295 lines)
│   ├── call.rs         # Call evaluation (~163 lines)
│   ├── control.rs      # Control flow (~584 lines)
│   └── pattern.rs      # Pattern evaluation (~202 lines)
└── module/
    └── import.rs       # Module loading (~240 lines)
```

## Design Goals

1. **Correctness** - Match language specification exactly
2. **Clear error messages** - Track source locations, provide context
3. **Modularity** - Separate concerns into focused modules
4. **Testability** - Dependency injection for registries

## Evaluation Flow

```
TypedModule { Module, ExprArena, expr_types }
    │
    │ create Evaluator
    ▼
Evaluator {
    env: Environment,          // Variables
    pattern_registry: ...,     // Pattern handlers
    type_registry: ...,        // User types
    output: EvalOutput,        // Captured output
}
    │
    │ find and call @main (or evaluate top-level)
    ▼
ModuleEvalResult {
    value: Value,              // Final result
    output: EvalOutput,        // Captured stdout/stderr
}
```

## Core Components

### Evaluator

```rust
pub struct Evaluator<'a> {
    /// String interner for Name lookups
    interner: &'a StringInterner,

    /// Expression arena (from parser)
    arena: &'a ExprArena,

    /// Variable environment
    env: Environment,

    /// User method registry with interior mutability
    /// Uses SharedMutableRegistry to allow method registration after
    /// MethodDispatcher construction (needed for load_module)
    user_method_registry: SharedMutableRegistry<UserMethodRegistry>,

    /// Cached method dispatcher (Chain of Responsibility pattern)
    /// Built once in EvaluatorBuilder, resolves methods via 3 resolvers:
    /// UserRegistryResolver → CollectionMethodResolver → BuiltinMethodResolver
    method_dispatcher: MethodDispatcher,

    /// Arena for imported functions (keeps them alive during evaluation)
    imported_arena: Option<SharedArena>,

    /// Whether prelude has been loaded
    prelude_loaded: bool,

    /// Captured output
    output: EvalOutput,
}
```

#### Why `SharedMutableRegistry`?

The `MethodDispatcher` is constructed once in `EvaluatorBuilder` with references to
the `UserMethodRegistry`. However, `load_module()` needs to register new methods
(from impl blocks, extends, and derives) after the Evaluator is created.

Using `SharedMutableRegistry<T>` (which wraps `Arc<RwLock<T>>`) allows:
1. The cached `MethodDispatcher` to see newly registered methods
2. Efficient read access during method resolution (no rebuilding)
3. Thread-safe method registration during module loading

```rust
// In load_module():
self.user_method_registry.write().merge(new_methods);

// In method resolution (via MethodDispatcher):
if let Some(method) = self.registry.read().lookup(type_name, method_name) { ... }
```

### Evaluation Entry Point

```rust
impl Evaluator {
    pub fn evaluate(&mut self, module: &Module) -> Result<Value, EvalError> {
        // Register module-level items
        self.register_functions(module)?;
        self.register_types(module)?;

        // Find and call @main
        if let Some(main_fn) = module.find_function("main") {
            self.call_function(main_fn, vec![])
        } else {
            // No main - evaluate top-level expression
            self.eval_module_expression(module)
        }
    }

    fn eval_expr(&mut self, id: ExprId) -> Result<Value, EvalError> {
        let expr = self.arena.get(id);

        match &expr.kind {
            ExprKind::Literal(lit) => self.eval_literal(lit),
            ExprKind::Ident(name) => self.eval_ident(*name),
            ExprKind::Binary { left, op, right } => {
                self.eval_binary(*left, *op, *right)
            }
            ExprKind::Call { func, args } => {
                self.eval_call(*func, args)
            }
            ExprKind::If { cond, then, else_ } => {
                self.eval_if(*cond, *then, *else_)
            }
            ExprKind::Pattern { name, args } => {
                self.eval_pattern(*name, args)
            }
            // ... more cases
        }
    }
}
```

## Key Features

### Value System

Runtime values with Arc-based sharing:

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Arc<String>),
    List(Arc<Vec<Value>>),
    Map(Arc<HashMap<Value, Value>>),
    Function(FunctionValue),
    Option(Option<Arc<Value>>),
    Result(Result<Arc<Value>, Arc<Value>>),
    Struct { name: Name, fields: Arc<HashMap<Name, Value>> },
    Void,
}
```

### Environment

Stack-based lexical scoping:

```rust
let x = 1           // Outer scope: x = 1
run(
    let x = 2,      // Inner scope: x = 2
    x + outer_x,    // Can't access outer x directly
)
// x = 1 again
```

### RAII Scope Guards

The evaluator uses RAII-style scope guards for safe scope management:

```rust
// Execute within a new environment scope (auto-cleanup)
self.with_env_scope(|eval| {
    eval.env.define(name, value, mutable);
    eval.eval(body)
})

// Execute with pre-defined bindings
self.with_bindings(bindings, |eval| eval.eval(body))

// Execute with match bindings (immutable)
self.with_match_bindings(pattern_bindings, |eval| eval.eval(arm_body))

// Execute with a single binding
self.with_binding(name, value, mutable, |eval| eval.eval(body))
```

These guards guarantee cleanup even on early returns or errors.

### Pattern Delegation

Patterns are evaluated via the registry:

```rust
fn eval_pattern(&mut self, name: Name, args: &[NamedArg]) -> Result<Value, EvalError> {
    let pattern = self.pattern_registry.get(name)?;
    let eval_args = self.eval_pattern_args(args)?;
    pattern.evaluate(&eval_args, self)
}
```

### Module Loading

Imports load and cache modules:

```rust
fn load_module(&mut self, path: &Path) -> Result<ModuleEvalResult, EvalError> {
    if let Some(cached) = self.module_cache.get(path) {
        return Ok(cached.clone());
    }

    let source = fs::read_to_string(path)?;
    let result = compile_and_evaluate(&source)?;

    self.module_cache.insert(path.to_path_buf(), result.clone());
    Ok(result)
}
```

## Method Dispatch Architecture

The evaluator uses a Chain of Responsibility pattern for method resolution:

```
receiver.method(args)
    │
    ▼
MethodDispatcher.resolve(receiver, type_name, method_name)
    │
    ├─► UserRegistryResolver      → User + derived methods (impl blocks, #[derive])
    │       │
    │       ▼
    ├─► CollectionMethodResolver  → Collection methods (map, filter, fold)
    │       │
    │       ▼
    └─► BuiltinMethodResolver     → Built-in methods (len, push, etc.)
```

The `UserRegistryResolver` is a unified resolver that checks both user-defined methods
(from impl blocks) and derived methods (from `#[derive(...)]`) in a single lookup.

### Iterator Helpers

Collection methods share logic via internal iterator helpers:

```rust
// Shared iterator-based implementations
fn map_iterator(&mut self, iter: impl Iterator<Item=Value>, transform: &Value) -> EvalResult
fn filter_iterator(&mut self, iter: impl Iterator<Item=Value>, predicate: &Value) -> EvalResult
fn fold_iterator(&mut self, iter: impl Iterator<Item=Value>, acc: Value, op: &Value) -> EvalResult
fn find_in_iterator(&mut self, iter: impl Iterator<Item=Value>, predicate: &Value) -> EvalResult
fn any_in_iterator(&mut self, iter: impl Iterator<Item=Value>, predicate: &Value) -> EvalResult
fn all_in_iterator(&mut self, iter: impl Iterator<Item=Value>, predicate: &Value) -> EvalResult

// Used by both list and range methods:
fn eval_list_map(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
    self.map_iterator(items.iter().cloned(), &args[0])
}

fn eval_range_map(&mut self, range: &RangeValue, args: &[Value]) -> EvalResult {
    self.map_iterator(range.iter().map(Value::Int), &args[0])
}
```

### Type Name Resolution

The `get_value_type_name()` method uses the `StringLookup` trait for unified type name resolution:

```rust
pub(super) fn get_value_type_name(&self, value: &Value) -> String {
    value.type_name_with_interner(self.interner).into_owned()
}
```

This handles struct type names (which require interner lookup) while delegating
to `Value::type_name()` for primitives and built-in types.

## Arena Threading Pattern

When evaluating functions or methods from different modules, the evaluator must
use the correct arena for expression lookups. The `create_function_evaluator`
helper ensures this:

```rust
/// Create a new evaluator for function/method evaluation with the correct arena.
///
/// This is critical for cross-module calls: functions from imported modules
/// carry their own SharedArena, and we must use that arena when evaluating
/// their body expressions.
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
```

This pattern appears in:
- `function_call.rs` - calling user functions
- `method_dispatch.rs` - calling user methods

## Related Documents

- [Tree Walking](tree-walking.md) - Execution strategy
- [Environment](environment.md) - Variable scoping
- [Value System](value-system.md) - Runtime values
- [Module Loading](module-loading.md) - Import resolution
