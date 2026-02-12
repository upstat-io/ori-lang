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

### CallStack — Frame-Based Depth Tracking

The interpreter uses a `CallStack` struct (defined in `compiler/ori_eval/src/diagnostics.rs`) that replaces the old `call_depth: usize` with proper frame tracking. Each frame captures the function name and call site span, enabling rich backtrace diagnostics on errors.

```rust
/// A single frame in the live call stack.
#[derive(Clone, Debug)]
pub struct CallFrame {
    /// Interned function or method name.
    pub name: Name,
    /// Source location of the call site (where the call was made, not the definition).
    pub call_span: Option<Span>,
}

/// Live call stack for the interpreter.
///
/// Each function/method call pushes a frame; return pops it. The depth
/// check is integrated into `push()` for ergonomic use.
#[derive(Clone, Debug)]
pub struct CallStack {
    frames: Vec<CallFrame>,
    max_depth: Option<usize>,
}
```

The `Interpreter` struct holds a `CallStack` instead of a raw depth counter:

```rust
pub struct Interpreter<'a> {
    // ... other fields ...

    /// Call stack with frame tracking and depth limit enforcement.
    pub(crate) call_stack: CallStack,
}
```

### Depth Check

The depth limit is checked inside `CallStack::push()` — no separate check method needed:

```rust
impl CallStack {
    /// Push a call frame, checking the depth limit.
    ///
    /// Returns `Err(EvalError)` with `StackOverflow` kind if the limit
    /// is exceeded. The frame is NOT pushed on overflow.
    pub fn push(&mut self, frame: CallFrame) -> Result<(), EvalError> {
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(ori_patterns::recursion_limit_exceeded(max));
            }
        }
        self.frames.push(frame);
        Ok(())
    }
}
```

### Depth Limit Source of Truth

The `max_depth` is derived from `EvalMode::max_recursion_depth()`, which returns `Option<usize>`:

- **`Interpret`**: `None` on native (stacker grows the stack), `Some(200)` on WASM
- **`ConstEval`**: `Some(64)` (tight budget prevents runaway)
- **`TestRun`**: `Some(500)` (generous but bounded)

```rust
impl EvalMode {
    pub fn max_recursion_depth(&self) -> Option<usize> {
        match self {
            Self::Interpret => {
                #[cfg(target_arch = "wasm32")]
                { Some(200) }
                #[cfg(not(target_arch = "wasm32"))]
                { None }
            }
            Self::ConstEval { .. } => Some(64),
            Self::TestRun { .. } => Some(500),
        }
    }
}
```

### Clone-per-Child Model

When the interpreter creates a child for a function call, it clones the parent's `CallStack` and calls `push()` on the clone. This is thread-safe (no shared mutable state) and O(N) per call, acceptable at practical depths (~24 bytes per frame, ~24 KiB at 1000 depth).

```rust
fn create_function_interpreter(&self, ...) -> Interpreter {
    let mut child_stack = self.call_stack.clone();
    child_stack.push(CallFrame { name, call_span: Some(span) })?;

    InterpreterBuilder::new(...)
        .call_stack(child_stack)
        .build()
}
```

### Backtrace Capture

When an error occurs, the call stack is captured as an `EvalBacktrace` and attached to the error. Frames are converted using the string interner to resolve interned `Name`s to display strings:

```rust
impl CallStack {
    /// Capture a snapshot of the current call stack as an `EvalBacktrace`.
    pub fn capture(&self, interner: &StringInterner) -> EvalBacktrace {
        let frames = self.frames.iter().rev()  // Most recent call first
            .map(|f| BacktraceFrame {
                name: interner.lookup(f.name).to_string(),
                span: f.call_span,
            })
            .collect();
        EvalBacktrace::new(frames)
    }

    /// Convenience: capture + attach backtrace to an error.
    pub fn attach_backtrace(&self, err: EvalError, interner: &StringInterner) -> EvalError {
        err.with_backtrace(self.capture(interner))
    }
}
```

## Configuration

### Default Limits

Limits are set per `EvalMode` via `max_recursion_depth()` (see above). The `CallStack` is initialized with the appropriate limit:

```rust
let call_stack = CallStack::new(mode.max_recursion_depth());
```

### Runtime Configuration

Custom limits can be set by constructing a `CallStack` directly:

```rust
let call_stack = CallStack::new(Some(500));  // Higher limit for Node.js WASM
let interpreter = InterpreterBuilder::new(&interner, &arena)
    .call_stack(call_stack)
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
6.              │  child_stack = self.call_stack.clone()
7.              │  child_stack.push(CallFrame { name, call_span })?  // Depth check here
8.              │  check_arg_count(f, args)?
9.              │  let mut call_env = self.env.child()
10.             │  call_env.push_scope()
11.             │  bind_captures(&mut call_env, f)
12.             │  bind_parameters(&mut call_env, f, args)
                │
13.             └─ create_function_interpreter(...)     // Build child
                   │  InterpreterBuilder::new(...)
                   │      .env(call_env)
                   │      .call_stack(child_stack)
                   │      .build()
                   │
14.                └─ call_interpreter.eval(f.body)     // RECURSE
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

The error is produced by `ori_patterns::recursion_limit_exceeded()` with `StackOverflow` kind, and includes a backtrace captured from the `CallStack`:

```rust
pub fn recursion_limit_exceeded(depth: usize) -> EvalError {
    EvalError {
        kind: EvalErrorKind::StackOverflow { depth },
        message: format!("maximum recursion depth exceeded (limit: {depth})"),
        ..
    }
}
```

When the error propagates, the interpreter attaches the call stack backtrace via `call_stack.attach_backtrace(err, interner)`, giving users:
- The exact depth limit reached
- A full backtrace showing the call chain that led to overflow
- The source spans of each call site in the chain
