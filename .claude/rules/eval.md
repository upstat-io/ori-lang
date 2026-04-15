---
paths:
  - "compiler/ori_eval/**/*.rs"
---

# Interpreter

## Architecture

Tree-walking interpreter over canonical IR (`CanExpr`). All evaluation goes through `eval_can(CanId)` in `interpreter/can_eval/mod.rs`. The canonical IR is the sole evaluation representation — the legacy `eval(ExprId)` path on `PatternExecutor` returns an error if called. The interpreter is portable (native + WASM contexts). For the full Salsa-integrated evaluator, see `oric::Evaluator`.

- **Arena threading**: functions carry their own `SharedArena`; callee's arena is used (not caller's) for thread safety in parallel evaluation
- **Enum dispatch** for fixed sets (no vtable overhead)
- **Spec references**: `docs/ori_lang/v2026/spec/operator-rules.md`, `docs/ori_lang/v2026/spec/14-expressions.md`

## Input

`CanExpr` + `CanArena` + `DecisionTreePool` (from `ori_canon`). The evaluator does NOT consume the AST (`ExprArena`) or raw typed IR directly — canonicalization is a prerequisite.

## Method Dispatch Chain
- Priority 0: UserRegistryResolver — user impls + `#[derive]`
- Priority 1: CollectionMethodResolver — map/filter/fold
- Priority 2: BuiltinMethodResolver — primitives

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

See `ir.md` §DerivedTrait for the canonical sync point list. This crate's sync point: `interpreter/derived_methods.rs` dispatches via strategy-based dispatch from `DerivedTrait::strategy()` (FieldOp + CombineOp → unified `eval_derived_method()`).

## Helper Submodules

- `exec::expr` — identifiers, indexing, field access, ranges
- `exec::call` — function calls, argument binding
- `exec::control` — pattern matching, loop actions, assignment
- `exec::decision_tree` — decision tree evaluation for multi-clause functions

## Tracing
- Target: `ori_eval` | `ORI_LOG=ori_eval=debug` (method dispatch, function calls) | `=trace` (every eval call)
- AOT mismatch: `diagnostics/dual-exec-debug.sh file.ori` (auto-dumps IR + RC stats) | see compiler.md for full reference

## Key Files
- `lib.rs`: Interpreter, eval dispatch
- `interpreter/can_eval/mod.rs`: Core `eval_can(CanId)` — sole evaluation entry point
- `interpreter/resolvers/`: MethodDispatcher (priority chain)
- `interpreter/method_dispatch/`: Method dispatch implementation + iterator methods
- `interpreter/derived_methods.rs`: Derived trait method dispatch (sync point)
- `methods/`: Built-in method implementations (collections, numeric, compare, etc.)
- `derives/mod.rs`: Derive processing pipeline
- `environment/mod.rs`: Environment, scopes
- `function_val.rs`: Built-in function registrations (prelude)
