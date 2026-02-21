---
title: "Tree Walking Interpretation"
description: "Ori Compiler Design — Tree Walking Interpretation"
order: 703
section: "Evaluator"
---

# Tree Walking Interpretation

The Ori evaluator uses tree-walking interpretation, where the canonical IR is traversed and evaluated directly without compilation to bytecode.

## How It Works

Tree-walking interpretation:
1. Receives canonical IR (`CanonResult` containing `CanExpr` nodes)
2. Recursively walks the tree via `eval_can(CanId)` in `interpreter/can_eval.rs`
3. Evaluates each node, producing a Value
4. Returns final value

All evaluation goes through `eval_can(CanId)` as the sole entry point. The canonical IR (`CanExpr`)
is a sugar-free representation — spread operators, template strings, named arguments, and other
syntactic sugar are desugared during canonicalization. Pattern matching is handled by decision tree
evaluation via `exec/decision_tree.rs`.

```rust
impl Interpreter<'_> {
    /// Entry point for canonical expression evaluation with stack safety.
    pub fn eval_can(&mut self, can_id: CanId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_can_inner(can_id))
    }
}
```

The `CanExpr` type is `Copy` (24 bytes), so the kind is copied out of the arena before
dispatching. This releases the immutable borrow on `self.canon`, allowing recursive
`self.eval_can()` calls in each arm.

## Canonical Expression Dispatch

The `eval_can_inner()` method handles all `CanExpr` variants exhaustively (no `_ =>` catch-all). Currently 48 variants. The kind is copied out of the arena before dispatching to release the immutable borrow:

```rust
fn eval_can_inner(&mut self, can_id: CanId) -> EvalResult {
    let kind = *self.canon_ref().arena.kind(can_id);

    match kind {
        // Literals — direct value construction
        CanExpr::Int(n) => Ok(Value::int(n)),
        CanExpr::Float(bits) => Ok(Value::Float(f64::from_bits(bits))),
        CanExpr::Bool(b) => Ok(Value::Bool(b)),
        CanExpr::Str(name) => Ok(Value::string_static(self.interner.lookup_static(name))),
        CanExpr::Char(c) => Ok(Value::Char(c)),
        CanExpr::Duration { value, unit } => { /* nanosecond conversion */ }
        CanExpr::Size { value, unit } => { /* byte conversion */ }
        CanExpr::Unit => Ok(Value::Void),

        // Compile-time constant (from ConstantPool)
        CanExpr::Constant(id) => Ok(const_to_value(self.canon_ref().constants.get(id), self.interner)),

        // Identifiers — environment lookup, then type ref fallback
        CanExpr::Ident(name) => { /* env.lookup(name), then TypeRef fallback */ }
        CanExpr::Const(name) => { /* constant reference: $name */ }
        CanExpr::TypeRef(name) => { /* resolved at canonicalization time */ }
        CanExpr::FunctionRef(name) => { /* function reference: @name */ }
        CanExpr::FunctionExp { kind, props } => { /* print, panic, todo, etc. */ }

        // Binary/Unary/Cast — dual dispatch (primitives direct, user types via traits)
        CanExpr::Binary { left, op, right } => self.eval_can_binary(can_id, left, op, right),
        CanExpr::Unary { op, operand } => { /* evaluate_unary dispatch */ }
        CanExpr::Cast { expr, target, fallible } => { /* type cast: as / as? */ }

        // Control flow
        CanExpr::If { cond, then, else_ } => { /* eval cond, branch */ }
        CanExpr::Block(range) => { /* eval statements, return last */ }
        CanExpr::Loop { body } => { /* loop with break value */ }
        CanExpr::For { binding, iter, body, yields } => { /* iterator-based loop */ }
        CanExpr::While { cond, body } => { /* condition-checked loop */ }

        // Pattern matching via decision trees
        CanExpr::Match { scrutinee, tree_id } => self.eval_can_match(can_id, scrutinee, tree_id),

        // Let bindings with pattern destructuring
        CanExpr::Let { pattern, value, .. } => { /* bind_can_pattern */ }

        // Calls and methods
        CanExpr::Call { func, args } => { /* eval func, eval args, dispatch */ }
        CanExpr::MethodCall { receiver, method, args } => { /* method dispatch chain */ }

        // Collections
        CanExpr::List(range) => { /* eval elements into Vec */ }
        CanExpr::Map(entries) => { /* eval key-value pairs */ }
        CanExpr::Tuple(range) => { /* eval elements into tuple */ }

        // Error handling
        CanExpr::Try(expr) => { /* error propagation: expr? */ }
        CanExpr::Unsafe(expr) => { /* transparent — evaluates inner expression */ }
        CanExpr::WithCapability { capability, provider, body } => { /* with Cap = x in body */ }
        CanExpr::FormatWith { expr, spec } => { /* format value with spec string */ }

        // ... remaining variants (StructLit, FieldAccess, Index, Lambda, etc.)
    }
}
```

### Literals

Literal values in canonical IR are already desugared — no `Literal` enum wrapper. Each literal type is a direct `CanExpr` variant with the value inline:

- `CanExpr::Int(i64)` → `Value::int(n)` (uses factory for small-int optimization)
- `CanExpr::Float(u64)` → `Value::Float(f64::from_bits(bits))`
- `CanExpr::Str(Name)` → `Value::string_static(interned_str)` (zero-copy from interner)
- `CanExpr::Constant(ConstId)` → looked up in `ConstantPool` (pre-folded at compile time)

### Identifiers and Type References

Two identifier variants exist in canonical IR:

- `CanExpr::Ident(name)` — Standard identifier. Checks the environment first, then falls back to `TypeRef` for user-defined types with associated functions (e.g., `Point.origin()`).
- `CanExpr::TypeRef(name)` — Resolved during canonicalization. Eliminates name resolution from the evaluator (phase boundary discipline). Still checks the environment first for variable shadowing.

### Binary Operations

Binary operators use a dual-dispatch strategy:
- **Primitive types** (int, float, bool, str, Duration, Size, etc.) use direct evaluation via `evaluate_binary` for performance
- **User-defined types** dispatch through operator trait methods (`Add::add`, `Sub::subtract`, `Mul::multiply`, etc.)
- **Short-circuit**: `&&` and `||` evaluate left operand first; skip right if result is determined

### Method Calls and Associated Functions

Method calls dispatch based on the receiver type:

- **`TypeRef` receivers** (associated function calls): `Point.origin()` — receiver is not passed as argument, dispatches to user-defined or built-in associated functions
- **Instance methods**: receiver is evaluated and passed as first argument through the method dispatch chain (built-in → collection → trait impl → user method)

### Function Calls

Function call evaluation:
1. Evaluate the callee expression to get a `Value::Function(FunctionValue)` or `Value::Builtin`
2. Evaluate all argument expressions (canonical IR — no named args, already reordered by desugaring)
3. Create a child interpreter with cloned call stack + pushed frame
4. Bind parameters in the child's environment
5. Evaluate the function body via `eval_can(body_can_id)`

### Let Bindings and Pattern Destructuring

`CanExpr::Let { pattern, value, .. }` evaluates the initializer via `eval_can(value)`, then binds via `bind_can_pattern()`. The canonical binding pattern (`CanBindingPattern`) supports Name, Wildcard, Tuple, Struct, and List destructuring.

### Control Flow

- **If**: `CanExpr::If { cond, then, else_ }` — evaluates condition, branches. Missing `else_` yields `Value::Void`.
- **For loops**: `CanExpr::For { binding, iter, body, yields }` — creates an iterator, binds each element, evaluates body. `yields: true` collects results into a list.
- **While loops**: `CanExpr::While { cond, body }` — condition-checked loop with `break`/`continue` support via `ControlAction`.
- **Loop**: `CanExpr::Loop { body }` — infinite loop, exited via `break(value)`.

### Match Expressions (Decision Trees)

Match expressions use **compiled decision trees** (Maranget 2008 algorithm), not sequential arm testing. The `CanExpr::Match { scrutinee, tree_id }` variant references a `DecisionTree` from the `DecisionTreePool`.

Decision tree evaluation (`exec/decision_tree.rs`) walks the tree:
- **Leaf** — Binds captured variables and evaluates the arm body
- **Guard** — Evaluates a guard expression; on failure, falls through to `on_fail`
- **Switch** — Tests the scrutinee at a path (e.g., enum tag, bool value, list length) and follows the matching edge
- **Fail** — Non-exhaustive match error (should be caught by exhaustiveness checking)

### Pattern Matching for Variants

**Variant vs Binding Disambiguation:** Uppercase pattern names are treated as variant constructors, lowercase as bindings:
- `Some` → variant pattern (matches `Value::Variant { name: "Some", ... }`)
- `x` → binding pattern (binds value to `x`)

## Advantages of Tree-Walking

1. **Simple implementation** - Direct mapping from AST to execution
2. **Good error messages** - Source spans available at runtime
3. **Easy debugging** - Can inspect state at any point
4. **No compilation step** - Immediate execution

## Disadvantages

1. **Slower than bytecode** - Interpretation overhead
2. **Memory overhead** - AST in memory during execution
3. **No optimizations** - Limited optimization opportunities

## Performance Considerations

Tree-walking is sufficient for:
- Small to medium programs
- Development and testing
- REPL interactions

For production, consider:
- JIT compilation
- Bytecode VM
- Ahead-of-time compilation

## Tail Call Optimization

Currently, Ori does not implement tail call optimization. Deep recursion can cause stack overflow:

```ori
// This will overflow for large n
@factorial (n: int) -> int =
    if n <= 1 then 1 else n * factorial(n - 1)
```

Future work: implement trampolining or continuation-passing for TCO.
