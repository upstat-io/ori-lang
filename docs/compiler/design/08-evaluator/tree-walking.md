---
title: "Tree Walking Interpretation"
description: "Ori Compiler Design — Tree Walking Interpretation"
order: 803
section: "Evaluator"
---

# Tree Walking Interpretation

## What Is Tree-Walking Interpretation?

A tree-walking interpreter evaluates a program by recursively traversing its abstract syntax tree and computing results at each node. When it encounters `a + b`, it recursively evaluates `a` to get a value, recursively evaluates `b` to get a value, then adds them. When it encounters `if c then t else e`, it evaluates `c`, checks whether it is true, and recursively evaluates either `t` or `e`. The evaluation function's structure mirrors the grammar's structure — one case per syntactic construct, recursive calls for sub-expressions.

This approach dates to McCarthy's original 1960 definition of Lisp, where `eval` was a recursive function over S-expressions. It remains the standard way to define a language's semantics precisely — the **definitional interpreter** approach. Denotational semantics, operational semantics, and interpreter-based language specifications all describe this same recursive structure. When a language specification says "the value of `if e1 then e2 else e3` is the value of `e2` if `e1` evaluates to true, otherwise the value of `e3`," it is describing exactly what a tree-walking interpreter does.

### The Execution Strategy Spectrum

Tree-walking sits at one end of a spectrum of execution strategies. Understanding the full spectrum clarifies why Ori chose tree-walking for development and AOT compilation for production:

| Strategy | How It Works | Speed | Startup | Examples |
|----------|-------------|-------|---------|----------|
| **Tree-walking** | Recursive dispatch on AST nodes | Slowest | Instant | Ori (dev), early Ruby, Roc REPL |
| **Bytecode VM** | Compile to bytecodes, dispatch loop | 5-20x faster | Fast | CPython, Lua, Ruby 1.9+, Erlang |
| **JIT compilation** | Compile hot paths to machine code at runtime | 10-100x faster | Warm-up delay | V8, JVM HotSpot, LuaJIT, PyPy |
| **AOT compilation** | Compile entire program to machine code ahead of time | Fastest | Compile step | Ori (prod), GCC, Rust, Go, Zig |

Each step down the table trades simplicity and startup speed for steady-state performance. Tree-walking interpreters are the simplest to build and maintain but the slowest to execute. AOT compilers produce the fastest executables but require a full compilation step. Bytecode VMs and JITs sit between, balancing complexity and performance.

Ori bypasses the middle of this spectrum: tree-walking for development (where instant startup and debuggability matter more than throughput), AOT via LLVM for production (where maximum performance matters). This avoids the engineering cost of designing a bytecode instruction set and maintaining a bytecode compiler — complexity that would deliver only modest improvements for the development workflow.

### Why Not a Bytecode VM?

A bytecode VM would provide 5-20x speedup over tree-walking. For Ori's development use case, this speedup rarely matters:

- **Test suites** are typically I/O-bound (file loading, Salsa caching) rather than CPU-bound
- **Type checking** dominates compile time; evaluation is a small fraction
- **The REPL and playground** evaluate short programs where interpretation overhead is imperceptible
- **Production performance** comes from LLVM, which provides 100-1000x speedup over tree-walking — making a bytecode VM an intermediate step with limited value

The engineering cost of a bytecode VM is substantial: instruction set design, a compiler from CanExpr to bytecodes, a dispatch loop (switch-threaded, direct-threaded, or computed-goto), a value stack, and all the associated debugging infrastructure. This cost buys performance improvements primarily for long-running interpreted programs — a use case Ori handles with AOT compilation instead.

## Ori's Tree-Walking Design

### The Entry Point: `eval_can`

All evaluation flows through a single entry point:

```rust
impl Interpreter<'_> {
    pub fn eval_can(&mut self, can_id: CanId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_can_inner(can_id))
    }
}
```

The `ensure_sufficient_stack` wrapper (from the `stacker` crate) checks that the native thread stack has enough space for the recursive call. If the stack is running low, it allocates a new segment. This prevents stack overflows on deeply nested expressions — a real concern for tree-walking interpreters, which use the host language's call stack as the evaluation stack.

### The Copy-Out Pattern

The inner dispatch function uses a critical pattern that deserves detailed explanation:

```rust
fn eval_can_inner(&mut self, can_id: CanId) -> EvalResult {
    let kind = *self.canon_ref().arena.kind(can_id);

    match kind {
        CanExpr::Int(n) => Ok(Value::int(n)),
        // ... 50+ variants
    }
}
```

The first line **copies** the `CanExpr` value out of the arena before dispatching. This is not an accident — it is essential for correctness. Here is why:

`self.canon_ref()` borrows `self` immutably to access the canonical IR. If we matched directly on `self.canon_ref().arena.kind(can_id)`, the immutable borrow on `self` would persist for the entire match body. But each match arm needs to call `self.eval_can()` recursively, which requires a mutable borrow on `self`. Holding an immutable and mutable borrow simultaneously violates Rust's borrowing rules.

The copy-out pattern resolves this: copy the 24-byte `CanExpr` value (which is `Copy`) onto the stack, immediately releasing the immutable borrow on `self`. Now each match arm can freely call `self.eval_can()` and other `&mut self` methods. The 24-byte copy is trivially cheap — it fits in three 64-bit registers.

This pattern is Ori-specific but the underlying problem is universal. Any tree-walking interpreter written in a language with strict aliasing or ownership rules (Rust, C++ with strict aliasing, linear types) must solve the "read the tree, then mutate the evaluator" tension. Copying the dispatch tag out of the tree is the simplest and cheapest solution.

### Exhaustive Dispatch

The `eval_can_inner` function matches **exhaustively** on all `CanExpr` variants — there is no `_ =>` catch-all arm. This is a deliberate safety measure: when a new variant is added to `CanExpr` (during language evolution), the Rust compiler forces every match site to handle it. A catch-all arm would silently pass through unhandled variants, potentially producing wrong results rather than compile errors.

The dispatch covers roughly 54 variants, organized by category:

**Literals** — direct value construction, no recursion:
- `Int(i64)`, `Float(u64)`, `Bool(bool)`, `Str(Name)`, `Char(char)`, `Unit`
- `Duration { value, unit }`, `Size { value, unit }` — unit conversion at evaluation time
- `Constant(ConstId)` — lookup in the pre-computed `ConstantPool`

**References** — environment lookup or type reference:
- `Ident(name)` — variable lookup in the scope chain
- `Const(name)` — immutable constant reference
- `TypeRef(name)` — resolved type reference (for associated function calls like `Point.origin()`)
- `FunctionRef(name)` — named function reference
- `SelfRef` — `self` in method bodies

**Operators** — dual dispatch (primitives direct, user types via traits):
- `Binary { left, op, right }` — delegated to `eval_can_binary()`
- `Unary { op, operand }` — negation, logical not, bitwise not
- `Cast { expr, target, fallible }` — type casts (`as` / `as?`)

**Control flow** — delegated to `control_flow.rs`:
- `If { cond, then, else_ }` — conditional branching
- `Block(range)` — evaluate statements in order, return last
- `Loop { body }` — infinite loop with `break` value
- `For { binding, iter, body, yields }` — iterator-based loop
- `While { cond, body }` — condition-checked loop
- `Match { scrutinee, tree_id }` — decision tree evaluation

**Calls and methods**:
- `Call { func, args }` — evaluate callee and arguments, then dispatch
- `MethodCall { receiver, method, args }` — method dispatch chain

**Collections** — evaluate elements into containers:
- `List(range)`, `Map(entries)`, `Tuple(range)`

**Bindings and access**:
- `Let { pattern, value, .. }` — pattern destructuring with `bind_can_pattern()`
- `FieldAccess { expr, field }`, `Index { expr, index }`, `Range { start, end, step, inclusive }`
- `StructLit`, `Lambda`, `Assign`

**Error handling and capabilities**:
- `Try(expr)` — error propagation (`?` operator)
- `Unsafe(expr)` — transparent wrapper (evaluates inner expression)
- `WithCapability { capability, provider, body }` — `with Cap = x in body`

### Decision Tree Evaluation

Match expressions do not use sequential arm testing. The [canonicalization phase](../07-canonicalization/pattern-compilation.md) compiles patterns into **decision trees** using the Maranget 2008 algorithm. The evaluator walks these trees:

- **Leaf** — binds captured variables and evaluates the arm body
- **Guard** — evaluates a guard expression; on failure, falls through to `on_fail`
- **Switch** — tests the scrutinee at a path (enum tag, bool value, list length) and follows the matching edge
- **Fail** — non-exhaustive match error (should be caught by exhaustiveness checking at compile time)

This approach avoids the O(patterns × depth) worst case of sequential testing. Each switch node splits the search space, and the tree's depth is bounded by the pattern depth rather than the number of arms.

### Function Call Evaluation

Function calls follow a consistent protocol:

1. **Evaluate the callee** to get a callable value (`Function`, `FunctionVal`, `VariantConstructor`, `NewtypeConstructor`)
2. **Evaluate all arguments** — canonical IR has already reordered named arguments to positional form
3. **Push a call frame** onto the call stack (for recursion depth tracking and backtrace capture)
4. **Create a child interpreter** with the callee's arena (arena threading for cross-module calls)
5. **Bind parameters** in the child's environment
6. **Evaluate the body** via `eval_can(body_can_id)`
7. **Pop the call frame** (via RAII `ScopedInterpreter` — guaranteed even on panic)

The child interpreter creation is the most distinctive step. Each `FunctionValue` carries a `SharedArena` reference to the arena where its body expressions live. The evaluator creates a new `Interpreter` bound to that arena, inheriting the user method registry (via `SharedMutableRegistry`) but with its own scope stack. This ensures expression ID lookups always hit the correct arena.

### Literal Evaluation

Literals in canonical IR are already desugared — no `Literal` enum wrapper. Each literal type is a direct `CanExpr` variant with the value inline:

- `CanExpr::Int(i64)` → `Value::int(n)` — wraps in `ScalarInt` for checked arithmetic
- `CanExpr::Float(u64)` → `Value::Float(f64::from_bits(bits))` — stored as bit pattern for Salsa compatibility
- `CanExpr::Str(Name)` → `Value::string_static(interned_str)` — zero-copy from the interner via `Cow::Borrowed`
- `CanExpr::Constant(ConstId)` → looked up in the `ConstantPool` (pre-folded at compile time by constant folding)

The float bit-pattern encoding is worth noting: floats are stored as `u64` in the canonical IR because `f64` does not implement `Eq` or `Hash` (due to NaN), which would prevent `CanExpr` from being used in Salsa's query system. The `from_bits` conversion at evaluation time is a no-op on all modern architectures.

### Binary Operator Dispatch

Binary operators use a tiered dispatch strategy:

1. **Short-circuit first**: `&&` and `||` evaluate the left operand, and skip the right if the result is determined
2. **Comparison operators**: `==`, `!=`, `<`, `<=`, `>`, `>=` use direct evaluation for primitives
3. **Mixed-type operations**: `int * Duration`, `int * Size` — detected and handled directly because primitives do not implement operator trait methods
4. **Primitive arithmetic**: `int + int`, `float * float`, etc. — direct evaluation without method lookup
5. **User-type operators**: if none of the above applies, dispatch through operator trait methods (`Add::add`, etc.)

This tiered approach avoids the overhead of method resolution for the common case (primitive operations) while supporting user-defined operator overloading for custom types.

## Prior Art

**Roc's `eval` module** is the closest structural analog. Like Ori, Roc evaluates a canonicalized IR (not the raw AST) via tree-walking. Roc's canonical expressions are defined in `can::expr::Expr` — a sugar-free enum similar to Ori's `CanExpr`. Both interpreters use exhaustive matching with no catch-all arms. The key difference is that Roc's interpreter is integrated with its compilation infrastructure, while Ori's `ori_eval` is deliberately standalone.

**GHC's Core evaluator** walks a tiny functional calculus (System FC) with roughly 10 expression forms. This demonstrates the power of aggressive desugaring — Haskell's rich surface syntax maps to a handful of core constructs. Ori's canonical IR is larger (~54 variants) because it preserves more operational detail (distinct loop forms, try expressions, format specifiers) rather than encoding everything as function application.

**Zig's comptime interpreter** in [`Sema.zig`](https://github.com/ziglang/zig/blob/master/src/Sema.zig) evaluates compile-time expressions by walking ZIR (Zig Intermediate Representation). Like Ori's `ConstEval` mode, it operates with budget limits and restricted capabilities. Zig's interpreter is more tightly integrated with semantic analysis — it runs during type checking rather than as a separate phase.

**Lua 5.0's original interpreter** (before the register-based VM) used a tree-walking approach similar to Ori's. Lua's transition to a register-based bytecode VM in 5.0 delivered roughly 2x speedup, demonstrating the performance ceiling of tree-walking for a general-purpose scripting language. Ori addresses this differently — via AOT compilation rather than a bytecode VM.

## Design Tradeoffs

**Exhaustive match vs catch-all.** Exhaustive matching on 54+ variants creates large match expressions that are harder to read than a short match with a `_ => unreachable!()` catch-all. But the safety benefit is significant: adding a new `CanExpr` variant immediately surfaces every evaluation site that needs updating. This is the same reasoning that Rust uses for requiring exhaustive enum matches — the compile-time cost prevents runtime bugs.

**Copy-out vs borrow splitting.** An alternative to copying `CanExpr` out of the arena would be to split the `Interpreter` struct so that the canonical IR is accessed through a separate reference that does not conflict with the mutable `self` borrow. This would avoid the copy but require restructuring the interpreter's data layout. At 24 bytes per copy, the current approach has negligible overhead and keeps the struct design simple.

**Single `eval_can` vs visitor pattern.** Some interpreters use a visitor pattern where each expression type has an `accept()` method that dispatches to a visitor. This is common in Java-based interpreters (following the pattern in Nystrom's *Crafting Interpreters*). Ori uses a single match expression instead — more idiomatic in Rust, avoids dynamic dispatch, and keeps the evaluation logic co-located rather than scattered across accept methods.

**`ensure_sufficient_stack` vs explicit trampoline.** The `stacker` crate's stack extension is a pragmatic choice. A trampoline (converting recursion to an explicit stack of continuations) would give full control over stack usage but requires significant restructuring of the evaluation logic. The stacker approach preserves the natural recursive structure while preventing stack overflows.

## Related Documents

- [Evaluator Overview](index.md) — Architecture, method dispatch, evaluation modes
- [Environment](environment.md) — Variable scoping and closure capture
- [Value System](value-system.md) — Runtime value representation
- [Canonicalization](../07-canonicalization/index.md) — The phase that produces CanExpr
- [Pattern Compilation](../07-canonicalization/pattern-compilation.md) — How match patterns become decision trees
