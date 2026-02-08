---
section: "03"
title: Environment V2
status: not-started
goal: Redesign scope management for thread-safety, improved RAII, and better closure capture
sections:
  - id: "03.1"
    title: ScopeStack Design
    status: not-started
  - id: "03.2"
    title: Thread-Safety Strategy
    status: not-started
  - id: "03.3"
    title: RAII Scope Guards
    status: not-started
  - id: "03.4"
    title: Closure Capture
    status: not-started
---

# Section 03: Environment V2

**Status:** 📋 Planned
**Goal:** Redesign the environment/scope system for optional thread-safety, cleaner RAII guards, and efficient closure capture — while maintaining the current API contract.

---

## Prior Art Analysis

### Current Ori Environment
Uses `Rc<RefCell<Scope>>` with parent pointers — each scope holds an `FxHashMap<Name, Value>` for O(1) lookup within a scope and an `Option<Rc<RefCell<Scope>>>` link to its parent. Lookup starts at the current scope and traverses via Rc-linked parent pointers, checking each scope's hash map along the chain. `Rc<RefCell>` is single-threaded (faster than Arc) but prevents future parallel eval.

**Already-solved problems in current code:**
- **Closure capture**: `FunctionValue.captures: Arc<FxHashMap<Name, Value>>` captures bindings at closure creation. The V2 design narrows this from capturing all visible bindings to capturing only the free variables the closure body references (requires free variable analysis).
- **RAII scope management**: `ScopedInterpreter` already provides RAII guards that pop scopes on drop. This pattern works well.

The V2 design should **evolve** these existing solutions rather than replace them from scratch.

### Rust CTFE: Frame-Based Locals
Rust stores locals per-frame as `IndexVec<Local, LocalState>`. No dynamic lookup — locals are indexed by position. Layout is cached (computed lazily). This is possible because MIR has already resolved all variable references to local indices.

### Zig: Runtime Index Tracking
Zig tracks a `runtime_index` on comptime allocations to prevent unsound comptime mutations inside runtime branches. This level of precision isn't needed for Ori's tree-walking interpreter but shows the value of context tracking.

### Roc: Symbol-Based Resolution
After canonicalization, Roc uses `Symbol` (ModuleId + IdentId = 64-bit) for all references. No string-based lookup at evaluation time. This is the gold standard for name resolution performance.

---

## 03.1 ScopeStack Design

Evolve the current `Environment` into a cleaner `ScopeStack` that preserves the existing FxHashMap-per-scope design for O(1) lookup:

```rust
/// Evaluation environment: a stack of lexical scopes.
/// Each scope uses FxHashMap for O(1) lookup (preserving current performance).
pub struct ScopeStack {
    /// Stack of scopes, each with its own hash map.
    scopes: Vec<Scope>,
}

struct Scope {
    /// O(1) lookup by name — preserves current FxHashMap-per-scope design.
    bindings: FxHashMap<Name, Binding>,
}

struct Binding {
    value: Value,
    mutability: Mutability,
}

impl ScopeStack {
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope {
            bindings: FxHashMap::default(),
        });
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop().expect("scope underflow");
    }

    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        let scope = self.scopes.last_mut().expect("no active scope");
        scope.bindings.insert(name, Binding { value, mutability });
    }

    pub fn lookup(&self, name: Name) -> Option<&Value> {
        // Search from innermost scope outward — O(1) per scope via hash lookup
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.bindings.get(&name) {
                return Some(&binding.value);
            }
        }
        None
    }

    pub fn update(&mut self, name: Name, new_value: Value) -> Result<(), EvalError> {
        // Find binding in nearest scope and check mutability
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.bindings.get_mut(&name) {
                if binding.mutability == Mutability::Immutable {
                    return Err(EvalError::immutable_binding(name));
                }
                binding.value = new_value;
                return Ok(());
            }
        }
        Err(EvalError::undefined_variable(name))
    }
}
```

**Why FxHashMap-per-scope (preserving current design):**
- **O(1) lookup within each scope**: Hash-based lookup is critical for scopes with many bindings
- **Proven performance**: The current evaluator already uses this design successfully
- **No regression**: A flat Vec with linear search would regress from the current O(1)-per-scope to O(n) total
- **Clean scope boundaries**: Each scope owns its bindings; pop is `Vec::pop()` which drops the scope's map
- **Reasonable overhead**: FxHashMap allocation per scope is acceptable since scope count is typically small

**Improvement over current**: Remove `Rc<RefCell>` wrapping — the interpreter owns the `ScopeStack` by value, so interior mutability is unnecessary. Use `&mut ScopeStack` for mutations, `&ScopeStack` for lookups.

**Global scope for function calls**: The first entry in the `scopes` Vec (index 0) is the global scope. When entering a function call, the interpreter creates a new ScopeStack by cloning the global scope (index 0) into the new stack's base. Globals are effectively immutable after initialization (set during `register_prelude()` and module registration), so cloning is semantically equivalent to sharing by reference and avoids `Rc`/`Arc` complexity in the `ScopeStack` design. Local scopes are pushed on top for the function body and popped on return. If mutable globals are needed in the future, a targeted `Arc<RwLock<Scope>>` can be added for just the global scope slot without changing the rest of the `ScopeStack` design.

- [ ] Implement `ScopeStack` with FxHashMap-per-scope
  - [ ] `push_scope()`, `pop_scope()` — scope management
  - [ ] `define(name, value, mutability)` — add binding to current scope
  - [ ] `lookup(name) -> Option<&Value>` — O(1)-per-scope hash lookup
  - [ ] `update(name, value) -> Result<(), EvalError>` — mutate binding with mutability check
  - [ ] `define_global(name, value)` — insert into root scope (index 0)
- [ ] Remove `Rc<RefCell>` wrapping
  - [ ] Interpreter owns `ScopeStack` by value
  - [ ] `&mut self` for define/update/push/pop, `&self` for lookup
- [ ] Benchmark against current Environment
  - [ ] Variable lookup latency (should be comparable — same hash-based approach)
  - [ ] Scope push/pop cost (should improve — no Rc allocation)
  - [ ] Memory usage

---

## 03.2 Thread-Safety Strategy

The current `Rc<RefCell<T>>` prevents any future parallel evaluation. The new design should support both single-threaded (fast) and multi-threaded (safe) modes:

```rust
/// Thread-safety abstraction — compile-time selected.
pub trait ScopeStorage: Clone {
    type Ref<'a, T: 'a>: Deref<Target = T>;
    type RefMut<'a, T: 'a>: DerefMut<Target = T>;

    fn new<T>(value: T) -> Self;
    fn borrow<'a, T: 'a>(&'a self) -> Self::Ref<'a, T>;
    fn borrow_mut<'a, T: 'a>(&'a self) -> Self::RefMut<'a, T>;
}

// Single-threaded (default, fast)
pub type LocalStorage = Rc<RefCell<T>>;

// Thread-safe (opt-in, for future parallel eval)
pub type SharedStorage = Arc<RwLock<T>>;
```

**Decision**: Start with the flat `ScopeStack` (no interior mutability needed for the Vec-based design). Thread-safety becomes relevant only when we parallelize function evaluation or tests — defer to when it's actually needed.

- [ ] Design ScopeStack to avoid interior mutability
  - [ ] `&mut ScopeStack` for define/update/push/pop (exclusive access)
  - [ ] `&ScopeStack` for lookup (shared access)
  - [ ] Interpreter holds `ScopeStack` by value (not behind Rc)
- [ ] For closure captures: clone the relevant bindings (not share the scope)
  - [ ] `capture(&self, names: &[Name]) -> Vec<(Name, Value)>` — snapshot specific bindings
  - [ ] Closures carry `Vec<(Name, Value)>` (current approach, proven)
- [ ] Document thread-safety strategy for future parallel eval
  - [ ] Option A: Per-thread ScopeStack (no sharing)
  - [ ] Option B: `Arc<RwLock<ScopeStack>>` for shared closures
  - [ ] Recommend Option A (Roc's approach — each task gets its own env)

---

## 03.3 RAII Scope Guards

The current `ScopedInterpreter` already provides RAII scope management (push on creation, pop on drop) and is a proven, working pattern. The V2 design evolves this into `ScopedInterpreter` that holds `&mut Interpreter` with `Deref`/`DerefMut` delegation, avoiding the borrow conflict that a naive `ScopeGuard<&mut ScopeStack>` would cause (the guard would borrow the stack exclusively, preventing the interpreter from evaluating the body).

```rust
/// RAII guard that holds &mut Interpreter, pushes a scope on creation,
/// and pops it on drop. Delegates all Interpreter methods via Deref/DerefMut.
///
/// Two-lifetime design: `'guard` is the guard's own lifetime (how long the scoped
/// region lasts), `'interp` is the interpreter's lifetime (must outlive the guard).
/// The current codebase uses a similar two-lifetime pattern — the exact signature
/// may be adjusted during V2 implementation depending on the final Interpreter design.
///
/// This avoids the borrow conflict where a ScopeGuard holding &mut ScopeStack
/// would prevent the interpreter from being used to evaluate the scoped body.
pub struct ScopedInterpreter<'guard, 'interp: 'guard> {
    interp: &'guard mut Interpreter<'interp>,
}

impl<'guard, 'interp: 'guard> ScopedInterpreter<'guard, 'interp> {
    pub fn new(interp: &'guard mut Interpreter<'interp>) -> Self {
        interp.env.push_scope();
        ScopedInterpreter { interp }
    }

    /// Define a binding in the guarded scope
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.interp.env.define(name, value, mutability);
    }
}

impl<'interp> Deref for ScopedInterpreter<'_, 'interp> {
    type Target = Interpreter<'interp>;
    fn deref(&self) -> &Self::Target {
        self.interp
    }
}

impl<'interp> DerefMut for ScopedInterpreter<'_, 'interp> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.interp
    }
}

impl Drop for ScopedInterpreter<'_, '_> {
    fn drop(&mut self) {
        self.interp.env.pop_scope();
    }
}

// Usage — no borrow conflict because ScopedInterpreter IS the interpreter:
fn eval_let(&mut self, ...) -> EvalResult<Value> {
    let mut scoped = ScopedInterpreter::new(self);
    scoped.define(name, value, Immutable);
    let result = scoped.eval(body)?;
    // scoped drops here, scope popped
    Ok(result)
}
```

**Why ScopedInterpreter (not ScopeGuard)**:
- A `ScopeGuard` holding `&mut ScopeStack` borrows the stack exclusively, making it impossible to call interpreter methods (which also need `&mut self`) while the guard is alive. This is a fundamental Rust borrow conflict.
- `ScopedInterpreter` holds `&mut Interpreter` and delegates via `Deref`/`DerefMut`, so you can call any interpreter method through the guard. The scope is automatically popped on drop.
- This pattern is already proven in the current codebase (`ScopedInterpreter` in `ori_eval`).

**Panic-safety motivation**: The current `Environment.with_scope()`, `with_binding()`, and `with_match_bindings()` methods are NOT panic-safe. If the closure passed to these methods panics, the scope is never popped, leaving the environment in a corrupted state with stale bindings. The V2 design eliminates these **unsafe Environment-level closure methods** in favor of `ScopedInterpreter` RAII guards, which guarantee scope cleanup via `Drop` even when unwinding through a panic.

**Note:** The Interpreter already provides panic-safe equivalents via `ScopedInterpreter` (`scope_guard.rs`). The V2 migration removes the **unsafe Environment-level** closure methods (`with_scope`, `with_binding`, `with_match_bindings`), since only the interpreter-level RAII versions (`ScopedInterpreter`) should be used. The **safe `ScopedInterpreter` convenience wrappers** (e.g., `with_match_bindings`, `with_binding` on `ScopedInterpreter`) are **preserved** -- these create an RAII guard internally and ARE panic-safe. The distinction is:
- **(a) Eliminated**: `Environment`-level closure methods (`Environment::with_scope`, etc.) -- not panic-safe, operate on raw scope stack.
- **(b) Preserved**: `ScopedInterpreter` convenience wrappers (`ScopedInterpreter::with_match_bindings`, etc.) -- panic-safe, create RAII guard internally, used by `eval_decision_tree()` (Section 04.3) and `eval_for()` (Section 05.3).

- [ ] Evolve current `ScopedInterpreter` for V2
  - [ ] Hold `&mut Interpreter` with `Deref`/`DerefMut` delegation
  - [ ] `new(interp)` — push scope via `interp.env.push_scope()`
  - [ ] `define(name, value, mut)` — bind in current scope
  - [ ] `Drop` — pop scope (guaranteed even on panic)
- [ ] Convenience wrappers on ScopedInterpreter
  - [ ] `with_match_bindings(bindings, closure)` — push scope, define all pattern bindings, run closure, pop scope on drop. Used by `eval_decision_tree()` (Section 04.3) to bind matched variables before evaluating arm bodies.
  - [ ] `with_binding(name, value, mutability, closure)` — push scope, define single binding, run closure, pop scope on drop. Used by `eval_for()` (Section 05.3) for loop variable bindings.
- [ ] Replace all manual `push_scope()`/`pop_scope()` pairs with `ScopedInterpreter`
  - [ ] `eval_let`
  - [ ] `eval_call` (function body scope)
  - [ ] `eval_match` (arm body scope)
  - [ ] `eval_for` (iteration body scope)
  - [ ] `eval_with_capability` (capability scope)
- [ ] Remove `owns_scoped_env` flag from Interpreter (no longer needed)
  - [ ] `ScopedInterpreter` makes this flag unnecessary
  - [ ] **Requires redesigning `create_function_interpreter()` scope management.** Currently `owns_scoped_env` controls whether the interpreter cleans up its scope on drop, which `create_function_interpreter()` relies on. Two options:
    - Option A: `create_function_interpreter()` returns a `ScopedInterpreter` that manages the function call scope via RAII (preferred — keeps scope management consistent)
    - Option B: Function call scope management moves to the caller, which wraps the call in a `ScopedInterpreter`

---

## 03.4 Closure Capture

The current implementation captures bindings via `FunctionValue.captures: Arc<FxHashMap<Name, Value>>`. The V2 design improves this in two ways: (1) uses `SmallVec` for the common case of 0-4 captures (representation optimization), and (2) narrows from capturing all visible bindings to only the free variables referenced by the closure body (precision improvement, requires free variable analysis):

```rust
/// Captured bindings for a closure.
/// Wrapped in Arc for O(1) clone — closures are frequently cloned when passed as
/// values, and the captured bindings are immutable after creation.
#[derive(Clone, Debug)]
pub struct CapturedEnv {
    inner: Arc<CapturedEnvInner>,
}

#[derive(Debug)]
struct CapturedEnvInner {
    /// Snapshot of captured bindings.
    /// SmallVec optimizes the common case of 0-4 captures (inline, no heap alloc).
    bindings: SmallVec<[(Name, Value); 4]>,
}

impl CapturedEnv {
    /// Capture specific names from the current scope stack.
    pub fn capture(stack: &ScopeStack, names: &[Name]) -> Self {
        let bindings = names.iter()
            .filter_map(|&name| {
                stack.lookup(name).map(|v| (name, v.clone()))
            })
            .collect();
        CapturedEnv {
            inner: Arc::new(CapturedEnvInner { bindings }),
        }
    }

    /// Restore captured bindings into a scope.
    pub fn restore_into(&self, stack: &mut ScopeStack) {
        for (name, value) in &self.inner.bindings {
            stack.define(*name, value.clone(), Mutability::Immutable);
        }
    }
}
```

**Design tradeoffs**:
- **Arc wrapping provides O(1) clone**: The current design uses `Arc<FxHashMap<Name, Value>>` which gives O(1) clone via Arc refcount bump. Switching to bare `SmallVec` would lose this — every closure clone would deep-copy all captured bindings. Since closures are frequently cloned when passed as values, `Arc<CapturedEnvInner>` preserves the O(1) clone property.
- **SmallVec inside Arc**: Best of both worlds — SmallVec avoids hash map overhead and heap allocation for the common 0-4 capture case, while Arc provides O(1) clone sharing. For closures with many captures (>4), SmallVec spills to heap inside the Arc.
- **Selective capture (precision improvement)**: The current code captures all visible bindings at closure creation. V2 narrows this to only the free variables the closure body actually references, requiring free variable analysis from the type checker or parser.

- [ ] Implement `CapturedEnv` with `Arc<CapturedEnvInner>` wrapping `SmallVec` (evolving from current `Arc<FxHashMap<Name, Value>>`)
  - [ ] `capture(stack, names)` — snapshot only the specified free variables (narrower than current capture-all)
  - [ ] `restore_into(stack)` — push captured bindings into new scope
  - [ ] `Clone` is O(1) via Arc refcount bump (preserves current behavior)
- [ ] Update `FunctionValue` to use `CapturedEnv`
  - [ ] Replace `captures: Arc<FxHashMap<Name, Value>>` with `CapturedEnv`
  - [ ] Ensure closure creation captures only free variables (behavioral change from current capture-all)
- [ ] Integrate with type checker's or parser's free variable analysis
  - [ ] Free variable analysis is required for selective capture — not optional
  - [ ] Determine whether free var set comes from type checker or a dedicated pass

---

## 03.5 Completion Checklist

- [ ] `ScopeStack` implemented with FxHashMap-per-scope (preserving current O(1) lookup)
- [ ] `Rc<RefCell>` wrapping removed — interpreter owns `ScopeStack` by value
- [ ] `ScopedInterpreter` RAII guards evolved from existing pattern (hold `&mut Interpreter`, Deref/DerefMut delegation)
- [ ] `CapturedEnv` with `Arc<SmallVec>` evolves from existing `Arc<FxHashMap<Name, Value>>` — O(1) clone preserved
- [ ] All manual `push_scope()`/`pop_scope()` pairs eliminated
- [ ] `owns_scoped_env` flag removed from Interpreter
- [ ] Thread-safety strategy documented (defer actual implementation)
- [ ] All existing tests pass unchanged
- [ ] Benchmarked against current Environment — no regression (hash-based approach ensures this)

**Exit Criteria:** Environment system uses hash-based scope lookup with `ScopedInterpreter<'guard, 'interp>` RAII guards (panic-safe, replacing closure-based `with_scope`/`with_binding`/`with_match_bindings`). Closure captures use `Arc<SmallVec>` with selective free-variable capture (O(1) clone preserved). Shared global scope pattern documented. Design documents a clear path to thread-safety.
