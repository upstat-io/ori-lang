---
paths:
  - "**eval**"
---

# Interpreter

## Architecture
- Tree-walking, arena threading
- Use callee's arena for function calls
- Enum dispatch for fixed sets

## Method Dispatch Chain
- Priority 0: UserRegistryResolver -- user impls + `#[derive]`
- Priority 1: CollectionMethodResolver -- map/filter/fold
- Priority 2: BuiltinMethodResolver -- primitives

## Value Types
- Primitives: `Int` `Float` `Bool` `Str` `Char` `Byte` `Void` `Duration` `Size`
- Collections: `List` `Map` `Tuple` (all `Heap<T>`)
- Wrappers: `Some` `None` `Ok` `Err`
- User: `Struct` `Variant` `Newtype`
- Functions: `Function` `FunctionVal`

## Environment
- Scope stack with `LocalScope<T>` = `Rc<RefCell<T>>`
- `env.capture()` for closures

## RAII Scope Guards
- `scoped()` -> `ScopedInterpreter`
- `with_env_scope(|s| { ... })`
- `with_binding(name, value, mutability, |s| { ... })`

## Derived Method Dispatch
- `interpreter/derived_methods.rs` dispatches `#[derive(...)]` via strategy-based dispatch from `DerivedTrait::strategy()` (in `ori_ir`)
- Strategies (FieldOp + CombineOp) drive unified `eval_derived_method()` -- no per-trait handlers
- **DO NOT** add a DerivedTrait variant without verifying strategy dispatch covers it | see CLAUDE.md "Adding a New Derived Trait"

## Tracing
- Target: `ori_eval` | `ORI_LOG=ori_eval=debug` (method dispatch, function calls) | `=trace` (every eval call)
- AOT mismatch: `diagnostics/dual-exec-debug.sh file.ori` (auto-dumps IR + RC stats) | see compiler.md for full reference

## Key Files
- `lib.rs`: Interpreter, eval dispatch
- `interpreter/resolvers/`: MethodDispatcher (priority chain)
- `interpreter/method_dispatch/`: Method dispatch implementation + iterator methods
- `interpreter/derived_methods.rs`: Derived trait method dispatch (sync point)
- `methods/`: Built-in method implementations (collections, numeric, compare, etc.)
- `derives/mod.rs`: Derive processing pipeline
- `environment.rs`: Environment, scopes
- `function_val.rs`: Built-in function registrations (prelude)
