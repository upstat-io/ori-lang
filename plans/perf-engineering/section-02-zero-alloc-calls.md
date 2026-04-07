---
section: "02"
title: "Zero-Allocation Call Path"
status: not-started
reviewed: false
goal: "Eliminate all heap allocations from the interpreter function call hot path — target 0 mallocs per call"
inspired_by:
  - "Lua CallInfo frame pointers (no-clone stack management)"
  - "Roc arena-allocated stack (src/eval/stack.zig — pointer bumping)"
  - "CPython frame pool (f_localsplus array, no hash map)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "CallStack Frame Pointer Refactor"
    status: not-started
  - id: "02.2"
    title: "Environment Scope Arena"
    status: not-started
  - id: "02.3"
    title: "Interpreter Construction Elimination"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Zero-Allocation Call Path

**Status:** Not Started
**OPTIONAL — Profile-gated.** This section optimizes the tree-walker, which the bytecode VM (Sections 04-05) will replace. Work this ONLY if Phase 0 profiling shows allocation churn dominates over recursive dispatch cost AND the bytecode VM timeline is long enough to justify the investment. Most of this work (Environment refactor, CallStack refactor) is rendered obsolete by the VM's register file and frame stack. The critical path skips this section.

**Goal:** Zero heap allocations per function call. Current: 5-8 mallocs per call (Vec, Rc, RefCell, FxHashMap). Target: 0. This is the single highest-impact optimization for the tree-walker — 44% of execution time is kernel-side malloc/free.

**Context:** The function call hot path in `ori_eval` allocates on every call:
1. `CallStack::clone()` in `create_function_interpreter()` — deep copies `Vec<CallFrame>` proportional to call depth (~10-30µs)
2. `Environment::child()` — allocates new `Vec<LocalScope<Scope>>` + clones global `Rc` (~5µs)
3. `push_scope()` — allocates `Rc<RefCell<Scope>>` with new `FxHashMap` (~5-10µs)

These are in `compiler/ori_eval/src/interpreter/mod.rs` (lines 407-458 `create_function_interpreter()`), `compiler/ori_eval/src/environment/mod.rs` (lines 208-211 `push_scope()`, lines 273-280 `child()`), and `compiler/ori_eval/src/interpreter/function_call.rs` (lines 137-159 `prepare_call_env()`).

**Reference implementations:**
- **Lua**: Uses a global `CallInfo` array with frame pointer indexing. `push_frame()` increments an index; `pop_frame()` decrements. Zero allocation.
- **Roc** `src/eval/stack.zig`: Arena-allocated stack with `alloca()` (pointer bump) and `restore()` (pointer rewind). Zero per-call allocation.
- **CPython**: Frame objects allocated from a pool; locals stored in fixed-size `f_localsplus` array, not hash maps.

**Depends on:** Section 01 (must measure before optimizing).

---

## 02.1 CallStack Frame Pointer Refactor

**File(s):** `compiler/ori_eval/src/diagnostics/mod.rs`, `compiler/ori_eval/src/interpreter/mod.rs`

Replace `CallStack::clone()` with a shared global frame stack and frame pointer indexing. Instead of cloning the entire Vec on every call, push a frame and record the starting index.

**Current code** (`diagnostics/mod.rs:48-52`):
```rust
pub struct CallStack {
    frames: Vec<CallFrame>,     // Cloned per call
    max_depth: Option<usize>,
}
```

**Target design:**
```rust
pub struct CallStack {
    frames: Vec<CallFrame>,     // Shared, never cloned
    max_depth: Option<usize>,
}

impl CallStack {
    /// Push a frame. Returns the frame index for later pop.
    #[inline]
    pub fn push(&mut self, frame: CallFrame) -> usize {
        let index = self.frames.len();
        self.frames.push(frame);
        index
    }

    /// Pop back to a saved frame index.
    #[inline]
    pub fn pop_to(&mut self, saved_index: usize) {
        self.frames.truncate(saved_index);
    }
}
```

- [ ] **Refactor `CallStack`** to support push/pop instead of clone:
  - Add `push(&mut self, frame) -> usize` method
  - Add `pop_to(&mut self, saved: usize)` method
  - Remove `Clone` derive (it should never be cloned)
- [ ] **Refactor `create_function_interpreter()`** in `interpreter/mod.rs`:
  - Instead of `self.call_stack.clone()`, pass `&mut self.call_stack` (or pass the saved index)
  - The child interpreter borrows the call stack, doesn't own it
  - On return, `pop_to(saved_index)` restores the stack
- [ ] **Update diagnostic capture**: Any code that reads the call stack for error messages must work with the shared stack (read frames `[0..current]` instead of cloning)
- [ ] **Matrix tests** (in `compiler/ori_eval/src/diagnostics/tests.rs` and `compiler/oric/tests/phases/eval/`):
  - **Dimensions**: call pattern (recursive, iterative for-loop, closure, multi-clause) x error path (normal return, panic mid-stack, `?` propagation, break from nested loop) x depth (shallow 5, medium 50, deep 500)
  - **Semantic pin**: Ackermann A(3,5) must produce 253 with the refactored CallStack (same result as before, zero regressions)
  - **Negative pin**: verify `CallStack` no longer implements `Clone` (compile-time enforcement)
  - **TDD ordering**: write tests for push/pop_to behavior FIRST, verify they fail with current clone-based code, then implement

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.2 Environment Scope Arena

**File(s):** `compiler/ori_eval/src/environment/mod.rs`

Replace `Environment::child()` + `push_scope()` with an arena-based scope stack. Instead of allocating `Rc<RefCell<Scope>>` with a new `FxHashMap` per call, use a pre-allocated scope pool or flat binding array.

**Current code** (`environment/mod.rs:273-280, 208-211`):
```rust
pub fn child(&self) -> Self {
    // Clone global once and reuse to avoid redundant Rc::clone
    let global = self.global.clone();   // Rc::clone
    Environment {
        scopes: vec![global.clone()],   // Vec allocation + Rc::clone
        global,
    }
}

pub fn push_scope(&mut self) {
    let parent = self.current_scope();  // Rc::clone
    let new_scope = LocalScope::new(Scope::with_parent(parent)); // Rc + RefCell + FxHashMap
    self.scopes.push(new_scope);
}
```

**Target design — Option A (Flat scope stack with linear lookup):**
```rust
pub struct Environment {
    /// All bindings across all scopes, in a flat Vec.
    bindings: Vec<Binding>,           // Pre-allocated, grows rarely
    /// Scope boundaries: each entry is the start index in `bindings`.
    scope_starts: Vec<usize>,         // Pre-allocated, grows rarely
    /// Global scope boundary.
    global_end: usize,
}

impl Environment {
    #[inline]
    pub fn push_scope(&mut self) {
        self.scope_starts.push(self.bindings.len());
    }

    #[inline]
    pub fn pop_scope(&mut self) {
        let start = self.scope_starts.pop().unwrap();
        self.bindings.truncate(start);
    }

    #[inline]
    pub fn define(&mut self, name: Name, value: Value, mutability: Mutability) {
        self.bindings.push(Binding { name, value, mutability });
    }

    #[inline]
    pub fn lookup(&self, name: Name) -> Option<Value> {
        // Search backwards (newest scope first)
        for binding in self.bindings.iter().rev() {
            if binding.name == name {
                return Some(binding.value.clone());
            }
        }
        None
    }
}
```

This eliminates ALL per-call allocations. `push_scope()` is a single `Vec::push(usize)`. `pop_scope()` is a truncate. `define()` is a `Vec::push`. `lookup()` is a linear scan — for typical scopes with <20 bindings, this is faster than HashMap due to cache locality.

- [ ] **Design decision**: Flat Vec with linear lookup (Option A, recommended for scopes <20 bindings — covers 99% of Ori functions) vs arena-allocated HashMap scopes (Option B, for pathological cases). Profile both.
- [ ] **Implement new `Environment`** with push/pop scope semantics, no Rc/RefCell
- [ ] **Migrate all callers**: `child()` becomes `save_scope_state()` → `restore_scope_state()`. `push_scope()`/`pop_scope()` become index operations.
- [ ] **Handle closures**: `capture()` currently walks the parent chain via `Scope::parent` (a linked list of `LocalScope<Scope>`). The flat Vec replaces this chain, so `capture()` must scan all bindings from `[0..current]` to collect visible names. This is semantically equivalent (same name shadowing behavior) and is cold path (closure creation), not hot path (function call). Verify that the linear scan produces the same capture set as the recursive parent walk for all existing closure tests.
- [ ] **Handle `assign()` to outer-scope variables**: The current `Scope::assign()` walks the parent chain to find mutable variables in enclosing scopes. The flat Vec must support backwards scanning for assignment (find the most recent binding with the given name, verify mutability, replace value). This is critical for `let x = 0; while cond do { x = x + 1 }` patterns.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/environment/tests.rs` and `compiler/oric/tests/phases/eval/`):
  - **Dimensions**: scope operation (define, lookup, assign, capture) x scope depth (1, 3, 10) x binding type (immutable, mutable, shadowed) x closure interaction (no closure, simple capture, nested capture)
  - **Semantic pin**: variable shadowing test where inner scope defines `x = 2` over outer `x = 1`, and after pop_scope, `lookup(x)` returns `1`
  - **Negative pin**: `assign()` to immutable binding returns `AssignError::Immutable` (unchanged behavior)
  - **TDD ordering**: write scope push/pop/lookup tests FIRST against new Environment API, verify they fail with current Rc/RefCell implementation, then implement flat Vec

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.3 Interpreter Construction Elimination

**File(s):** `compiler/ori_eval/src/interpreter/mod.rs`, `compiler/ori_eval/src/interpreter/function_call.rs`

Eliminate `create_function_interpreter()` entirely. Currently, every function call creates a new `Interpreter` struct with 22 fields. Instead, reuse a single interpreter instance, saving/restoring state on call/return (like a real CPU does with registers).

**Current code** (`interpreter/mod.rs:407-458`): Creates a new Interpreter per call with:
- 5-8× Arc clones: `source_file_path` (Option<Arc<String>>), `source_text` (Option<Arc<String>>), `user_method_registry` (Arc<RwLock>), `default_field_types` (Arc<RwLock>), `method_dispatcher` (Arc<Vec>), `imported_arena` (Arc), `print_handler` (Arc), `canon` (Option<Arc>)
- CallStack clone (eliminated in 02.1)
- Environment child (eliminated in 02.2)
- ModeState::child() — constructs a new ModeState with fresh counters; no heap alloc but copies budget/counter state
- 5× Copy field copies (self_name, type_names, print_names, prop_names, op_names, format_names, builtin_method_names, mode, scope_ownership)

**Target design**: Single interpreter instance with save/restore:
```rust
impl Interpreter<'_> {
    fn eval_call(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        // Save state
        let saved_scope = self.env.save();
        let saved_stack = self.call_stack.push(frame);
        let saved_arena = self.arena;

        // Set up call
        self.env.push_scope();
        self.arena = func.shared_arena();
        bind_parameters(self, func, args)?;

        // Execute
        let result = self.eval_can(func.can_body);

        // Restore state
        self.arena = saved_arena;
        self.call_stack.pop_to(saved_stack);
        self.env.restore(saved_scope);

        catch_propagation(result)
    }
}
```

This eliminates ALL per-call construction overhead. No Arc clones, no struct allocation, no field copying.

- [ ] **Refactor `eval_call()`** to use save/restore instead of creating a new interpreter
- [ ] **Handle arena switching**: The function's arena may differ from the caller's (imported functions). Save/restore the arena pointer. **Feasibility concern**: The current `create_function_interpreter` returns `Interpreter<'b>` where the child's arena lifetime `'b` may differ from the parent's `'a` (with `'a: 'b`). A save/restore model on a single interpreter instance must handle this lifetime difference — the simplest approach is to erase the arena lifetime by storing `&ExprArena` as a raw pointer or using an arena index, then restoring the typed reference on return. Alternatively, since the arena is accessed via `SharedArena` (Arc), the lifetime could be extended by holding the Arc.
- [ ] **Handle imported functions**: Functions from other modules may have different `SharedCanonResult`s. The interpreter must switch `self.canon` to the callee's canon and restore on return. This is just an `Option<Arc>` swap.
- [ ] **Eliminate `ModeState::child()`**: Pass budget/counters by mutable reference, not by cloning a child state.
- [ ] **Handle `ScopeOwnership` and `Drop`**: The current interpreter uses RAII scope cleanup — when `scope_ownership == Owned`, the `Drop` impl calls `env.pop_scope()`. A save/restore model replaces this with explicit restore on both the success and error paths. Verify that panics during evaluation still clean up properly (the Drop impl was panic-safe).
- [ ] **Handle `ScopedInterpreter`**: The `scope_guard.rs` module provides `ScopedInterpreter` for temporary scope changes (e.g., `with_binding`, `with_env_scope`). These must work with the new save/restore model.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/interpreter/function_call.rs` tests and spec tests):
  - **Dimensions**: call type (recursive, imported module, closure, multi-clause) x error handling (`?` propagation, nested try, panic) x state (capabilities, self-binding, arena switching)
  - **Semantic pin**: recursive Ackermann A(3,5) = 253 with single-interpreter save/restore (identical to clone-based)
  - **Negative pin**: panic during evaluation still cleans up scope (verify via a test that panics mid-call and checks scope depth returns to pre-call level)
  - **TDD ordering**: write save/restore correctness tests FIRST, verify they fail (save/restore API doesn't exist yet), then implement

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Zero heap allocations on the Ackermann function call path (verified via custom allocator or `ORI_TRACE_ALLOC` flag)
- [ ] Ackermann A(3,7) completes in <15s (down from 55s) — measured via Section 01 benchmarks
- [ ] `cargo bench -p oric --bench interpreter -- ackermann` shows ≥3x improvement over Phase 0 baseline
- [ ] All spec tests pass: `cargo st` green (5,800+ passing)
- [ ] All Rust tests pass: `cargo t` green
- [ ] `./test-all.sh` green
- [ ] No new `unsafe` code introduced (if arena lifetime erasure in 02.3 requires `unsafe`, it must be minimally scoped with `// SAFETY:` comment and encapsulated in a safe API)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `cargo bench -p oric --bench interpreter -- ackermann_3_5` shows per-call cost ≤10µs (down from 63µs). System time fraction drops from 44% to <15%. Ackermann A(3,7) completes in <15 seconds. All 5,800+ spec tests pass unchanged.
