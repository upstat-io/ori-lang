# Evaluator Overview

The Sigil evaluator is a tree-walking interpreter that executes typed ASTs. It handles expression evaluation, function calls, pattern execution, and module loading.

## Location

```
compiler/sigilc/src/eval/
├── mod.rs              # Module exports
├── evaluator.rs        # Main evaluator (~764 lines)
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
pub struct Evaluator {
    /// Variable environment
    env: Environment,

    /// Expression arena (from parser)
    arena: ExprArena,

    /// Type information (from type checker)
    types: Vec<Type>,

    /// Pattern definitions
    pattern_registry: SharedPatternRegistry,

    /// User-defined types
    type_registry: SharedTypeRegistry,

    /// Captured output
    output: EvalOutput,

    /// Module cache (for imports)
    module_cache: HashMap<PathBuf, ModuleEvalResult>,
}
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

## Related Documents

- [Tree Walking](tree-walking.md) - Execution strategy
- [Environment](environment.md) - Variable scoping
- [Value System](value-system.md) - Runtime values
- [Module Loading](module-loading.md) - Import resolution
