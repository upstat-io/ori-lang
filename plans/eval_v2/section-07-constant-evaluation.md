---
section: "07"
title: Constant Evaluation
status: not-started
goal: Evaluate compile-time-known expressions during IR lowering, enabling constant folding and static initialization
sections:
  - id: "07.1"
    title: Constness Classification
    status: not-started
  - id: "07.2"
    title: Compile-Time Evaluator
    status: not-started
  - id: "07.3"
    title: Constant Folding (Integrated into Lowering)
    status: not-started
  - id: "07.4"
    title: Memoized Pure Functions
    status: not-started
---

# Section 07: Constant Evaluation

**Status:** Planned
**Goal:** Evaluate compile-time-known expressions during type checking or IR lowering, enabling constant folding, dead branch elimination, and memoization of pure function calls.
**Dependencies:** Section 02 (Machine Abstraction — EvalMode::ConstEval), Section 01 (Value System — ValuePool for interning folded constants)
**Used by:** Section 08 (Canonical EvalIR — constant folding integrated into lowering pass)

---

## Prior Art Analysis

### Zig: Comptime (The Gold Standard)
Zig's comptime system evaluates arbitrary code at compile time. Functions with comptime parameters are instantiated for each call site. Memoized call results are stored in the InternPool. A `branch_quota` prevents infinite loops. Comptime blocks can mutate comptime-allocated variables. Errors during comptime are compile errors.

### Rust CTFE: Validated Const Evaluation
Rust evaluates `const` items, `const fn` calls, and static initializers at compile time using the same interpreter as Miri. Results are validated (no dangling pointers, no UB) and interned. The `CompileTimeMachine` policy rejects I/O and enforces determinism.

### Go: Untyped Constant Arithmetic
Go evaluates constant expressions with arbitrary precision during type checking. The `go/constant` package provides exact rational arithmetic. Constants are typed only when used in a typed context. Lazy string concatenation defers materialization.

### Elm: Constraint-Based Const Propagation
Elm tracks constness through its constraint system — if all operands of a binary operation are constant, the result is constant. This happens during type inference, not as a separate pass.

---

## 07.1 Constness Classification

Determine which expressions are compile-time evaluable:

```rust
/// Classification of an expression's constness.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Constness {
    /// Fully compile-time evaluable (literal, const binding, pure arithmetic on consts)
    Const,
    /// Depends on runtime values but has no side effects (pure function on runtime args)
    Pure,
    /// Has side effects (I/O, mutation, capability access)
    Effectful,
    /// Unknown (not yet classified)
    Unknown,
}
```

**Classification rules**:
- **Literals**: Always `Const` (Int, Float, Bool, String, Char, None, Unit, Duration, Size)
- **Binary/Unary operators on Const operands**: `Const` (e.g., `1 + 2`, `!true`)
- **Let bindings where init is Const**: Binding is `Const`
- **If/match where condition and all branches are Const**: `Const`
- **Function calls where function is pure and all args are Const**: `Const` (with memoization)
- **Algebraic constructors (Ok, Err, Some) where inner is Const**: `Const`
- **Tuple/List/Map literals where all elements are Const**: `Const`
- **Variable references**: Constness of the binding
- **Capability access**: `Effectful` (always)
- **Print, panic, break, continue**: `Effectful`

```rust
pub fn classify_constness(
    expr: ExprId,
    arena: &ExprArena,
    bindings: &FxHashMap<Name, Constness>,
) -> Constness {
    match arena.expr_kind(expr) {
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) |
        ExprKind::String(_) | ExprKind::Char(_) | ExprKind::None |
        ExprKind::Unit | ExprKind::Duration { .. } |
        ExprKind::Size { .. } => Constness::Const,

        ExprKind::Tuple(range) | ExprKind::List(range) => {
            arena.get_expr_list(*range).iter()
                .map(|e| classify_constness(*e, arena, bindings))
                .fold(Constness::Const, Constness::merge)
        }

        ExprKind::Map(range) => {
            arena.get_expr_list(*range).iter()
                .map(|e| classify_constness(*e, arena, bindings))
                .fold(Constness::Const, Constness::merge)
        }

        ExprKind::Binary { left, right, .. } => {
            let l = classify_constness(left, arena, bindings);
            let r = classify_constness(right, arena, bindings);
            l.merge(r)
        }

        ExprKind::Ident(name) => {
            bindings.get(&name).copied().unwrap_or(Constness::Unknown)
        }

        ExprKind::Ok(inner) | ExprKind::Err(inner) | ExprKind::Some(inner) => {
            classify_constness(*inner, arena, bindings)
        }

        ExprKind::Call { func, args } => {
            // If function is known-pure and all args are const, result is const
            let func_constness = classify_constness(*func, arena, bindings);
            let args_constness = arena.get_expr_list(*args).iter()
                .map(|a| classify_constness(*a, arena, bindings))
                .fold(Constness::Const, Constness::merge);
            func_constness.merge(args_constness)
        }

        _ => Constness::Unknown,
    }
}

impl Constness {
    fn merge(self, other: Constness) -> Constness {
        match (self, other) {
            (Constness::Const, Constness::Const) => Constness::Const,
            (Constness::Effectful, _) | (_, Constness::Effectful) => Constness::Effectful,
            (Constness::Unknown, _) | (_, Constness::Unknown) => Constness::Unknown,
            _ => Constness::Pure, // Both are Pure or (Const, Pure)
        }
    }
}
```

- [ ] Define `Constness` enum
- [ ] Implement `classify_constness()` for all expression kinds
  - [ ] Literals → Const (Int, Float, Bool, String, Char, None, Unit, Duration, Size)
  - [ ] Operators on Const → Const
  - [ ] Algebraic constructors (Ok, Err, Some) → constness of inner expression
  - [ ] Compound literals (Tuple, List, Map) → Const when all elements are Const
  - [ ] Variables → look up in binding map
  - [ ] Function calls → merge func + args constness
  - [ ] Capability access → Effectful
- [ ] Integrate with type checker output
  - [ ] Type checker already knows about `@const` annotations (future)
  - [ ] Purity information from capabilities

---

## 07.2 Compile-Time Evaluator

Use `EvalMode::ConstEval` (Section 02) to evaluate constant expressions:

```rust
pub struct ConstEvaluator<'a> {
    /// The interpreter configured with EvalMode::ConstEval { budget }
    /// (mode is set at construction time, not via generic parameter).
    /// Budget is accessed via interpreter.mode if needed — no redundant field.
    interpreter: Interpreter<'a>,
    /// Constness classification for known bindings.
    ///
    /// Populated from:
    /// - `const` declarations (always `Constness::Const`)
    /// - Let bindings whose init expression was successfully const-evaluated
    /// - Function parameters (always `Constness::Pure` — runtime values, no effects)
    /// - Prelude/global bindings (classified based on purity analysis)
    bindings: FxHashMap<Name, Constness>,
}

impl<'a> ConstEvaluator<'a> {
    /// Try to evaluate an expression at compile time.
    /// Returns `Some(value)` if successful, `None` if the expression
    /// requires runtime evaluation.
    ///
    /// `pool` is passed as a parameter rather than stored in ConstEvaluator
    /// to avoid aliasing conflicts when the Lowerer also holds `&mut ValuePool`.
    pub fn try_eval(&mut self, expr: ExprId, pool: &mut ValuePool) -> Option<Value> {
        let constness = classify_constness(expr, self.interpreter.arena, &self.bindings);
        if constness != Constness::Const {
            return None;
        }

        match self.interpreter.eval(expr) {
            Ok(value) => {
                // pool: reserved for interning folded constants into ValuePool.
                // Once ValuePool integration (Section 01) is complete, successful
                // results will be interned here: `pool.intern(value)`.
                Some(value)
            }
            Err(_) => None, // Const eval failed — defer to runtime
        }
    }

    /// Evaluate a constant expression, returning an error if it fails.
    /// Used for `const` bindings where evaluation MUST succeed.
    ///
    /// `pool` is passed as a parameter (same rationale as `try_eval`).
    pub fn eval_const(&mut self, expr: ExprId, pool: &mut ValuePool) -> Result<Value, ConstEvalError> {
        // pool: reserved for interning the evaluated constant into ValuePool.
        // Once ValuePool integration (Section 01) is complete, successful results
        // will be interned: `pool.intern(value)`.
        self.interpreter.eval(expr)
            .map_err(|e| ConstEvalError::from_eval_error(e, expr))
    }
}

/// Thin wrapper around `EvalError` that adds const-eval context (the expression
/// that failed). The error variants themselves (`ConstEvalBudgetExceeded`,
/// `ConstEvalSideEffect`, `DivisionByZero`, `IntegerOverflow`) live in
/// `EvalErrorKind` (Section 10) — no separate `ConstEvalErrorKind` enum.
///
/// This follows the principle of a single unified error hierarchy. Const-eval
/// specific variants are identified by their `EvalErrorKind` discriminant, not
/// by a separate error type.
pub struct ConstEvalError {
    /// The underlying evaluation error (uses EvalErrorKind variants from Section 10)
    pub inner: EvalError,
    /// The expression that failed const evaluation
    pub expr: ExprId,
}

impl ConstEvalError {
    /// Wrap an EvalError with const-eval expression context.
    pub fn from_eval_error(error: EvalError, expr: ExprId) -> Self {
        ConstEvalError { inner: error, expr }
    }
}
```

- [ ] Implement `ConstEvaluator` using `EvalMode::ConstEval` interpreter
  - [ ] Fields: `interpreter: Interpreter<'a>`, `bindings: FxHashMap<Name, Constness>`
  - [ ] Populate `bindings` from const declarations, let bindings, function params, and globals
  - [ ] `try_eval(expr, &mut ValuePool) -> Option<Value>` — optimistic compile-time eval
  - [ ] `eval_const(expr, &mut ValuePool) -> Result<Value, ConstEvalError>` — mandatory const eval
  - [ ] `&mut ValuePool` passed as parameter (not stored) to avoid aliasing with Lowerer
- [ ] Define `ConstEvalError` as thin wrapper `{ inner: EvalError, expr: ExprId }`
  - [ ] No separate `ConstEvalErrorKind` — uses `EvalErrorKind` variants from Section 10:
    - `ConstEvalBudgetExceeded` — too many steps (like Zig's branch_quota)
    - `ConstEvalSideEffect { capability }` — tried to use I/O in const context
    - `DivisionByZero`, `IntegerOverflow` — arithmetic errors become compile errors
  - [ ] `from_eval_error(error: EvalError, expr: ExprId) -> ConstEvalError` — wrap with expression context
- [ ] Integration point: call from EvalIR lowering (Section 08)
  - [ ] During lowering, `try_eval` constant expressions
  - [ ] Replace with `EvalIR::Const(value)` if successful

---

## 07.3 Constant Folding (Integrated into Lowering)

Constant folding is **not a separate pass** — it is integrated directly into the lowering pass (Section 08). During bottom-up lowering, the lowerer checks whether child nodes resolved to `Const` and folds eagerly in a single pass:

```rust
/// Inside Lowerer::lower_expr() — constant folding happens during lowering.
fn lower_binary(&mut self, left: ExprId, op: BinaryOp, right: ExprId) -> EvalIrId {
    // Lower children first (bottom-up)
    let l = self.lower_expr(left);
    let r = self.lower_expr(right);

    // Check if both children resolved to constants — if so, fold eagerly
    let span = self.arena.expr_span(left).merge(self.arena.expr_span(right));
    if let (EvalIrNode::Const(lv), EvalIrNode::Const(rv)) =
        (self.ir_arena.get(l), self.ir_arena.get(r))
    {
        if let Some(result) = eval_const_binop(lv, op, rv) {
            return self.ir_arena.alloc(EvalIrNode::Const(result), span);
        }
    }

    // Could not fold — emit the operator node
    self.ir_arena.alloc(EvalIrNode::BinaryOp { left: l, op, right: r }, span)
}

fn lower_if(&mut self, cond: ExprId, then_br: ExprId, else_br: ExprId) -> EvalIrId {
    let c = self.lower_expr(cond);

    // Dead branch elimination: if condition is constant, only lower the live branch
    if let EvalIrNode::Const(Value::Bool(b)) = self.ir_arena.get(c) {
        return if *b {
            self.lower_expr(then_br)
        } else {
            self.lower_expr(else_br)
        };
    }

    let t = self.lower_expr(then_br);
    let e = self.lower_expr(else_br);
    let span = self.arena.expr_span(cond);
    self.ir_arena.alloc(EvalIrNode::If { cond: c, then_branch: t, else_branch: e }, span)
}
```

**Why integrated, not a separate pass:** Bottom-up lowering already visits every node once. Checking for foldable children at each step adds negligible cost and avoids a second full traversal. While the arena does support in-place mutation by index (`&mut EvalIrArena`), integrating folding into lowering is still preferable: it handles constants as they are discovered and avoids a separate traversal pass entirely.

**Folding opportunities** (all handled during lowering):
- Arithmetic on literals: `1 + 2` → `3`
- Boolean logic (both operands constant): `true && false` → `false`, `true || false` → `true`
- Dead branch elimination: `if true { a } else { b }` → `a`
- String concatenation of literals: `"hello" + " world"` → `"hello world"` (BinaryOp::Add on strings)
- Collection literals: `[1, 2, 3].len()` → `3`

**Deferred to future algebraic simplification pass:**
- Single-operand boolean simplification: `true && x` → `x`, `false || x` → `x`, `false && x` → `false`, `true || x` → `true`. These require reasoning about one non-constant operand and are better handled in a dedicated algebraic simplification pass after lowering. Initial constant folding only folds when **both** operands are constant.

- [ ] Implement `eval_const_binop(left, op, right) -> Option<Value>` (used by lowerer)
  - [ ] Arithmetic: +, -, *, /, %
  - [ ] Comparison: ==, !=, <, >, <=, >=
  - [ ] Boolean: &&, ||
  - [ ] String: + (BinaryOp::Add, overloaded for string concatenation)
- [ ] Implement `eval_const_unaryop(op, operand) -> Option<Value>`
- [ ] Integrate folding into `Lowerer::lower_expr()` (Section 08.2)
  - [ ] Binary operators on constant operands → fold
  - [ ] Unary operators on constant operands → fold
  - [ ] If with constant condition → eliminate dead branch (only lower live branch)
  - [ ] String concatenation of constant strings → fold
  - [ ] List/tuple with all constant elements → fold to constant
- [ ] Track folding statistics (for diagnostics/debugging)
  - [ ] Number of expressions folded
  - [ ] Number of dead branches eliminated

---

## 07.4 Memoized Pure Functions

**Prerequisite**: Extend `Value::equals` to cover `Map`, `Struct`, and `Range` comparisons (currently return `false` via catch-all). MemoCache uses `Value::equals` for argument comparison, so compound values must be handled correctly for cache lookups to work with functions that take Map/Struct/Range arguments.

- [ ] Extend `Value::equals` to handle `Map`, `Struct`, `Range` (prerequisite for MemoCache)

Cache results of pure function calls with constant arguments (inspired by Zig's memoized calls):

```rust
pub struct MemoCache {
    /// (function body ExprId, args) → cached result.
    ///
    /// Uses `ExprId` (function body ID) instead of raw `u64` for type safety.
    /// Stores the actual `Vec<Value>` args alongside the key to avoid hash
    /// collision issues — two calls with different args that happen to hash
    /// the same are correctly distinguished.
    entries: FxHashMap<ExprId, Vec<MemoEntry>>,
}

struct MemoEntry {
    args: Vec<Value>,
    result: Value,
    call_count: u32,  // How many times this was looked up
}

impl MemoCache {
    pub fn lookup(&mut self, func_id: ExprId, args: &[Value]) -> Option<&Value> {
        if let Some(entries) = self.entries.get_mut(&func_id) {
            for entry in entries.iter_mut() {
                if entry.args.len() == args.len()
                    && entry.args.iter().zip(args).all(|(a, b)| a.equals(b))
                {
                    entry.call_count += 1;
                    return Some(&entry.result);
                }
            }
        }
        None
    }

    pub fn insert(&mut self, func_id: ExprId, args: &[Value], result: Value) {
        self.entries.entry(func_id).or_default().push(MemoEntry {
            args: args.to_vec(),
            result,
            call_count: 0,
        });
    }
}
```

**When to memoize**:
- Function is marked as pure (no capabilities in its `uses` clause)
- All arguments are `Constness::Const`
- Function body doesn't use mutable state
- Budget allows evaluation

**When NOT to memoize**:
- Function uses capabilities (Http, FileSystem, etc.)
- Function accesses mutable state
- Arguments contain heap values that might mutate (conservative)

- [ ] Implement `MemoCache` with `ExprId` (function body ID) keying
  - [ ] Key: `ExprId` for the function body; entries store full `Vec<Value>` args for collision-safe lookup
  - [ ] `lookup(func_id: ExprId, args) -> Option<&Value>` — check cache
  - [ ] `insert(func_id: ExprId, args, result)` — store result
  - [ ] `clear()` — invalidate all entries
- [ ] Integrate with `EvalMode::ConstEval` (Section 02)
  - [ ] Before evaluating a const function call, check memo cache
  - [ ] After successful evaluation, store in memo cache
- [ ] Implement `Value::equals`-based argument comparison for cache lookup
  - [ ] Must be deterministic (no Arc pointer comparison)
  - [ ] Content-based equality for compound values (already provided by `Value::equals`)
- [ ] Track cache hit/miss statistics

---

## 07.5 Completion Checklist

- [ ] `Constness` classification for all expression types
- [ ] `ConstEvaluator` using `Interpreter<'a>` with `EvalMode::ConstEval { budget }` (pool passed as param, not stored)
- [ ] Constant folding integrated into lowering pass (Section 08.2), not a separate pass
- [ ] Dead branch elimination during lowering for constant conditions
- [ ] `eval_const_binop` and `eval_const_unaryop` helper functions
- [ ] `MemoCache` for pure function results
- [ ] `ConstEvalError` as thin wrapper `{ inner: EvalError, expr: ExprId }` (no separate ConstEvalErrorKind — uses Section 10's EvalErrorKind)
- [ ] Integration with EvalIR lowering (Section 08)
- [ ] Tests: constant expressions folded correctly
- [ ] Tests: side-effect functions rejected in const context
- [ ] Tests: memo cache hit/miss behavior

**Exit Criteria:** Compile-time-known expressions are evaluated and folded during IR lowering (single pass), reducing runtime work. Pure functions with constant arguments are memoized.
