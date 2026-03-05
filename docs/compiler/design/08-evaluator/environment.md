---
title: "Environment"
description: "Ori Compiler Design — Environment and Scoping"
order: 801
section: "Evaluator"
---

# Environment

## What Is an Environment?

In programming language theory, an **environment** is a mapping from names to values. When a program says `let x = 42` and later references `x`, the environment is the data structure that remembers "x means 42." This concept is so fundamental that it appears in the very first formal definitions of programming language semantics — Landin's 1964 SECD machine had an explicit Environment component, and the lambda calculus uses substitution (a mathematical environment) as its core evaluation mechanism.

The interesting design questions are not about what an environment does — that is straightforward — but about how it handles **scope**, **mutation**, **closures**, and **lifetime**. These four concerns interact in ways that force real tradeoffs.

### Scope: Who Can See What?

**Lexical scoping** (also called static scoping) means that a name's meaning is determined by where it appears in the source text, not by the runtime call stack. This is the dominant scoping discipline today — C, Java, Python, Rust, Haskell, JavaScript (with `let`/`const`), Go, and nearly every modern language use lexical scoping. The alternative, **dynamic scoping** (Emacs Lisp, early Lisp, Bash), determines a name's meaning by looking up the call stack at runtime, making it impossible to determine a function's behavior by reading its source code alone.

Within lexical scoping, implementations differ in how they represent the scope chain:

| Approach | How It Works | Used By |
|----------|-------------|---------|
| **Flat map per scope** | Each scope has its own hash map; lookup walks the scope chain | Ori, many interpreters |
| **Persistent map** | Scopes share structure via persistent data structures | Haskell (GHC), some ML compilers |
| **De Bruijn indices** | Names replaced by numeric depth + offset at compile time | Lambda calculus implementations, Lean |
| **Stack frames** | Variables at fixed offsets in stack memory | C, Rust, compiled languages |
| **Display registers** | Array indexed by scope depth for O(1) access | Some Algol/Pascal implementations |

Tree-walking interpreters almost universally use the flat-map approach because it is the simplest and requires no preprocessing step. Compiled languages use stack frames because they are the fastest at runtime. The persistent map approach is elegant for functional languages where environments are immutable and shared.

### Closures: When Functions Outlive Their Scope

A closure is a function bundled with the environment where it was defined. When a lambda captures variables from its enclosing scope, those variables must remain accessible even after the enclosing scope has been exited. This creates a fundamental tension: how do you keep captured bindings alive when the scope that created them is gone?

The two main strategies are:

- **Capture by reference** — the closure holds a pointer to the original binding. If the binding changes, the closure sees the new value. Used by JavaScript, Python, C# (for mutable captures). Requires keeping the original scope alive (or allocating captured variables on the heap).
- **Capture by value** — the closure copies the binding's value at creation time. Changes to the original binding do not affect the closure. Used by Ori, C++ (`[=]`), and pure functional languages.

Ori captures by value because it aligns with Ori's value semantics: values are self-contained, copying is cheap (primitives are inline, collections use `Arc`-based sharing), and there is no shared mutable state between a closure and its enclosing scope.

## Ori's Environment Design

### Data Structures

The environment uses a parent-linked scope chain with `Rc<RefCell<_>>` wrapped in a `LocalScope<T>` newtype:

```rust
/// Single-threaded scope wrapper.
#[repr(transparent)]
pub struct LocalScope<T>(Rc<RefCell<T>>);

pub struct Scope {
    bindings: FxHashMap<Name, Binding>,
    parent: Option<LocalScope<Scope>>,
}

struct Binding {
    value: Value,
    mutability: Mutability,
}

pub enum Mutability {
    Mutable,    // let x = ...
    Immutable,  // let $x = ...
}

pub struct Environment {
    scopes: Vec<LocalScope<Scope>>,
    global: LocalScope<Scope>,
}
```

Three design choices deserve explanation:

**`LocalScope<T>` is `Rc<RefCell<T>>`, not `Arc<RwLock<T>>`.** The interpreter is single-threaded — there is no concurrent access to scopes during evaluation. Using `Rc` instead of `Arc` avoids atomic reference counting overhead (~2-5x cheaper for inc/dec operations). Using `RefCell` instead of `RwLock` avoids mutex overhead. The `#[repr(transparent)]` annotation ensures the newtype has zero memory overhead.

**`FxHashMap<Name, Binding>` for bindings.** `Name` is a 32-bit interned index, and `FxHashMap` (from the `rustc-hash` crate) uses a fast non-cryptographic hash function optimized for small integer keys. This is significantly faster than `HashMap` with the default `SipHash` hasher for the common case of looking up interned names.

**Parent chain, not scope stack.** Each `Scope` holds an `Option<LocalScope<Scope>>` pointing to its parent. This means closure capture works by sharing `Scope` nodes — a closure can hold a reference to a scope in the middle of the chain, keeping it alive even after the environment has popped past it. A flat scope stack without parent pointers would require copying all visible bindings at closure creation time.

### Core Operations

**Variable binding** inserts into the current scope's hash map:

```rust
pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
    self.bindings.insert(name, Binding { value, mutability });
}
```

This allows shadowing — defining a name that already exists in an outer scope creates a new binding in the current scope, leaving the outer binding untouched.

**Variable lookup** walks the parent chain from the current scope outward:

```rust
pub fn lookup(&self, name: Name) -> Option<Value> {
    if let Some(binding) = self.bindings.get(&name) {
        return Some(binding.value.clone());
    }
    if let Some(parent) = &self.parent {
        return parent.borrow().lookup(name);
    }
    None
}
```

This is O(depth) in the worst case, but typical scope depths are shallow (5-10 levels), so the linear scan is fast in practice. The alternative — maintaining a flat hash map of all visible bindings — would give O(1) lookup but require copying on every scope push/pop.

**Variable assignment** also walks the parent chain, but additionally checks mutability:

```rust
pub fn assign(&mut self, name: Name, value: Value) -> Result<(), AssignError> {
    if let Some(binding) = self.bindings.get_mut(&name) {
        if !binding.mutability.is_mutable() {
            return Err(AssignError::Immutable);
        }
        binding.value = value;
        return Ok(());
    }
    if let Some(parent) = &self.parent {
        return parent.borrow_mut().assign(name, value);
    }
    Err(AssignError::Undefined)
}
```

Assignment walks up the chain to find the binding and modify it in place. If the binding is immutable (`let $x = ...`), the assignment fails with `AssignError::Immutable`. If the name is not found in any scope, it fails with `AssignError::Undefined`. This two-error-variant design gives precise error messages — "cannot assign to immutable constant x" vs "undefined variable x."

### Scope Management

The `Environment` manages a stack of scopes for block entry/exit:

```rust
pub fn push_scope(&mut self) {
    let current = self.scopes.last().cloned();
    let new_scope = LocalScope::new(match current {
        Some(parent) => Scope::with_parent(parent),
        None => Scope::new(),
    });
    self.scopes.push(new_scope);
}

pub fn pop_scope(&mut self) {
    if self.scopes.len() > 1 {
        self.scopes.pop();
    }
}
```

The scope stack is separate from the parent chain — `push_scope()` creates a new scope whose parent is the current top of the stack. This dual structure exists because closures need parent chain traversal (for captured variable access) while block scoping needs stack push/pop (for entering and exiting blocks).

### Closure Capture

When a lambda is created, the interpreter captures all visible bindings as a flat `FxHashMap<Name, Value>`:

```rust
pub fn capture(&self) -> FxHashMap<Name, Value> {
    let mut captures = FxHashMap::default();
    // Collect from all scopes, innermost-first
    // (inner bindings shadow outer ones)
    captures
}
```

This **flattening** is a deliberate design choice. The alternative — having the closure hold a reference to the scope chain — would avoid the copy but create complex lifetime dependencies between closures and the environments that created them. By flattening to a `FxHashMap`, closure capture is a value operation: the closure owns its captured bindings, and the original environment can be freely modified or destroyed.

For module-level functions, captures are shared across all functions via `Arc<FxHashMap<Name, Value>>` — avoiding redundant copies when registering dozens of functions from the same module.

### RAII Scope Guards

Scope management must be panic-safe. If an evaluation panics inside a block, the scope pushed for that block must still be popped. Ori uses RAII guards to guarantee this:

```rust
pub struct ScopedInterpreter<'g, 'i> {
    interpreter: &'g mut Interpreter<'i>,
}

impl Drop for ScopedInterpreter<'_, '_> {
    fn drop(&mut self) {
        self.interpreter.env_mut().pop_scope();
    }
}
```

The guard pushes a scope on creation and pops it on drop — even if the evaluation panics, the scope is cleaned up. Both `ori_eval::Interpreter` and `oric::Evaluator` provide guards, with `Deref`/`DerefMut` implementations for transparent access.

The guard API offers several convenience methods:

```rust
// Direct guard usage
let mut scoped = interpreter.scoped();
scoped.env.define(name, value, Mutability::Immutable);
let result = scoped.eval(body)?;
// scope automatically popped here

// Closure-based
self.with_env_scope(|scoped| {
    scoped.env.define(name, value, Mutability::Immutable);
    scoped.eval(body)
})

// Single binding convenience
self.with_binding(name, value, Mutability::Immutable, |s| s.eval(body))

// Pattern match bindings (all immutable)
self.with_match_bindings(bindings, |s| s.eval(arm_body))
```

### Mutability Model

Ori inverts the common default: `let` creates a mutable binding, and `let $` creates an immutable one:

```ori
let x = 0          // Mutable — can be reassigned
let $y = 10        // Immutable constant — assignment is an error

x = x + 1          // OK
y = y + 1          // error E1012: cannot assign to immutable constant y
```

The environment tracks mutability per binding and enforces it at assignment time, not at definition time. This is a runtime check in the interpreter; the type checker also validates it statically.

### Function Scopes

When calling a function, the evaluator creates a new scope chain rooted in the function's captured environment:

```rust
fn call_function(&mut self, func: &FunctionValue, args: Vec<Value>) -> Result<Value, EvalError> {
    // Start with captured environment (for closures)
    self.env.push_scope_with(func.captured_env.clone());

    // Bind parameters (immutable — params cannot be reassigned, except self)
    for (param, arg) in func.params.iter().zip(args) {
        self.env.define(*param, arg, Mutability::Immutable);
    }

    let result = self.eval_can(func.can_body);

    self.env.pop_scope();
    result
}
```

Function parameters are immutable — they cannot be reassigned within the function body. This is enforced by defining them with `Mutability::Immutable`. The exception is `self` in method bodies, which is bound as `Mutability::Mutable` to support self-mutating methods (see `mutable-self-proposal`).

## Prior Art

**OCaml's environment** uses persistent maps (balanced binary trees) that share structure between scopes. This is more memory-efficient for deeply nested scopes but slower for lookup (O(log n) vs O(1) amortized for hash maps). Ori chose hash maps because scope depths are shallow in practice, making lookup speed more important than sharing efficiency.

**Rust's MIR interpreter** (Miri) uses a flat memory model with allocations indexed by `AllocId`. Variables are stored at specific offsets within stack-frame allocations, mirroring compiled code's stack layout. This is faster for execution but requires more complex scope management — Ori's hash-map approach is simpler and sufficient for an interpreter.

**Roc's environment** also uses flat maps for scope management during evaluation. Like Ori, Roc captures by value for closures. The primary difference is that Roc's variable representations are more tightly integrated with its type system, while Ori's `Value` enum is a fully separate type from the type checker's representations.

**GHC's STG machine** uses a fundamentally different model — closures carry pointers to heap-allocated thunks, and the environment is implicit in the closure's free-variable list. This model supports lazy evaluation, which Ori does not need.

**V8's JavaScript engine** faces a harder version of the scope problem because JavaScript closures capture by reference and variables can be modified by any closure that captured them. V8 solves this with "context" objects (heap-allocated scope frames) that persist as long as any closure references them. Ori avoids this complexity entirely through capture-by-value.

## Design Tradeoffs

**`Rc<RefCell>` vs direct ownership.** The scope chain uses `Rc<RefCell<Scope>>` for shared ownership. An alternative would be to own scopes directly in a `Vec<Scope>` and use indices for parent references. This would avoid reference counting overhead but would prevent closures from holding references to specific scopes (closures would need to copy all bindings eagerly). The current design defers the copy to `capture()` time, which is called only when creating lambdas and module functions.

**Capture-by-value vs capture-by-reference.** Ori captures by value (copying bindings into the closure at creation time). Capture-by-reference would be more efficient for closures that capture many variables but only read a few, but would require keeping scope frames alive and introduce shared mutable state — contradicting Ori's value semantics design.

**Flat capture map vs structured scope chain.** When capturing for a closure, Ori flattens all visible bindings into a single `FxHashMap`. An alternative would be to capture a reference to the scope chain itself, preserving its structure. Flattening is simpler and makes closures fully self-contained (no dependency on the original scope chain's lifetime), at the cost of copying more bindings than strictly necessary.

**Global scope as special case.** The `Environment` maintains both a `scopes` stack and a separate `global` scope. Globals are defined via `define_global()` and are always accessible. This means module-level definitions (functions, types, constructors) live in the global scope and are not affected by scope push/pop during evaluation. The alternative — treating the bottom of the scope stack as the global scope — would simplify the data structure but complicate the invariant that popping a scope never removes global definitions.

## Related Documents

- [Evaluator Overview](index.md) — Architecture and evaluation modes
- [Tree Walking](tree-walking.md) — How `eval_can` uses the environment
- [Value System](value-system.md) — What gets stored in bindings
- [Module Loading](module-loading.md) — How module functions are registered in the environment
- [Type Environment](../05-type-system/type-environment.md) — The type checker's parallel scope mechanism
