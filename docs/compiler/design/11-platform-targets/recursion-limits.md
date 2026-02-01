---
title: "Recursion Limits"
description: "Stack safety implementation for WASM builds"
order: 3
---

# Recursion Limits

The Ori interpreter implements platform-specific stack safety to handle deep recursion gracefully.

## The Problem

Without protection, deep recursion on WASM causes cryptic errors:

```
// Browser
"Maximum call stack size exceeded"

// Some WASM runtimes
"memory access out of bounds"
```

These errors provide no context about what went wrong or how to fix it.

## The Solution

On WASM builds, the interpreter tracks call depth and fails gracefully:

```
[runtime] maximum recursion depth exceeded (WASM limit: 200)
```

On native builds, the `stacker` crate dynamically grows the stack, allowing arbitrarily deep recursion.

## Implementation

### Call Depth Tracking

The `Interpreter` struct tracks the current call depth:

```rust
pub struct Interpreter<'a> {
    // ... other fields ...

    /// Current call depth (incremented on each function call)
    pub(crate) call_depth: usize,

    /// Maximum allowed depth (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub(crate) max_call_depth: usize,
}
```

### Depth Check

Before each function/method call, the limit is checked:

```rust
#[cfg(target_arch = "wasm32")]
pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
    if self.call_depth >= self.max_call_depth {
        Err(recursion_limit_exceeded(self.max_call_depth))
    } else {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
    Ok(())  // No-op on native
}
```

### Depth Propagation

Child interpreters inherit and increment the depth:

```rust
fn create_function_interpreter(&self, ...) -> Interpreter {
    InterpreterBuilder::new(...)
        .call_depth(self.call_depth.saturating_add(1))
        .max_call_depth(self.max_call_depth)  // WASM only
        .build()
}
```

## Configuration

### Default Limit

```rust
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_MAX_CALL_DEPTH: usize = 200;
```

### Runtime Configuration

The limit can be set when building the interpreter:

```rust
let interpreter = InterpreterBuilder::new(&interner, &arena)
    .max_call_depth(500)  // Higher limit for Node.js
    .build();
```

### Choosing a Limit

| Environment | Suggested Limit | Rationale |
|-------------|-----------------|-----------|
| Browser | 200 | Conservative for ~1MB stack |
| Node.js | 500-1000 | Larger default stack |
| Wasmtime | 1000+ | Configurable stack size |
| Embedded | 50-100 | Limited resources |

## Call Stack Breakdown

Here's the exact Rust call stack for a single Ori function call (e.g., `fib(n - 1)`):

```
Rust Call Stack (per Ori function call)
═══════════════════════════════════════

1. eval(&mut self, expr_id)                     // Entry point
   └─ ensure_sufficient_stack(|| ...)           // Stack guard (native only)
      │
2.    └─ eval_inner(&mut self, expr_id)         // Main dispatch
         │  match expr.kind {
         │      ExprKind::Call { func, args } => {
         │
3.       │      let func_val = self.eval(*func)?       // Eval function ref
4.       │      let arg_vals = self.eval_expr_list()?  // Eval arguments
         │
5.       └───── self.eval_call(&func_val, &arg_vals)   // Dispatch call
                │
                │  // Inside eval_call for Value::Function:
6.              │  check_recursion_limit()?             // WASM limit check
7.              │  check_arg_count(f, args)?
8.              │  let mut call_env = self.env.child()
9.              │  call_env.push_scope()
10.             │  bind_captures(&mut call_env, f)
11.             │  bind_parameters(&mut call_env, f, args)
                │
12.             └─ create_function_interpreter(...)     // Build child
                   │  InterpreterBuilder::new(...)
                   │      .env(call_env)
                   │      .call_depth(self.call_depth + 1)
                   │      .build()
                   │
13.                └─ call_interpreter.eval(f.body)     // RECURSE
```

### Frame Count Analysis

| Frame | Function | File |
|-------|----------|------|
| 1 | `eval` | `interpreter/mod.rs:287` |
| 2 | `ensure_sufficient_stack` closure | `ori_stack` |
| 3 | `eval_inner` | `interpreter/mod.rs:314` |
| 4 | `eval_call` | `interpreter/function_call.rs:12` |
| 5 | `create_function_interpreter` | `interpreter/mod.rs:968` |
| 6 | `InterpreterBuilder::build` | `interpreter/builder.rs:146` |
| 7 | `eval` (recursive) | Back to frame 1 |

**Minimum: 6-7 frames per Ori call**

Additional frames from:
- Argument evaluation: +2-3 frames per argument (`eval` → `eval_inner`)
- Helper functions: `bind_captures`, `bind_parameters`, `check_arg_count`
- Method calls add `eval_user_method` or `eval_associated_function`

**Realistic estimate: 8-12 Rust frames per Ori function call**

### Why 200 is Conservative

With ~1000 WASM stack frames available (browser default):

```
1000 frames ÷ 8 frames/call = 125 Ori calls (tight)
1000 frames ÷ 12 frames/call = 83 Ori calls (worst case)
```

The 200 default assumes:
- Some WASM runtimes have more stack space
- Simple functions use fewer frames
- Safety margin for expression nesting within function bodies

## Call Sites

The recursion check is performed at:

1. **Function calls** (`eval_call` in `function_call.rs`)
   - `Value::Function`
   - `Value::MemoizedFunction`

2. **Method calls** (`method_dispatch.rs`)
   - `eval_user_method`
   - `eval_associated_function`

Built-in methods and operators don't increment call depth since they don't create new interpreter instances.

## Native Stack Management

On native builds, `ori_stack::ensure_sufficient_stack` wraps recursive calls:

```rust
pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
    ensure_sufficient_stack(|| self.eval_inner(expr_id))
}
```

This uses the `stacker` crate to:
1. Check remaining stack space against a red zone (100KB)
2. If low, allocate a new stack segment (1MB)
3. Continue execution on the new segment

This allows recursion depths of 100,000+ on native builds.

## Error Message

The error message clearly indicates this is a WASM limitation:

```rust
pub fn recursion_limit_exceeded(limit: usize) -> EvalError {
    EvalError::new(format!(
        "maximum recursion depth exceeded (WASM limit: {limit})"
    ))
}
```

Users understand:
- The limit exists
- It's specific to WASM
- The exact limit value
- Native compilation has no such limit
