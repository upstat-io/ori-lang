---
section: "04"
title: Constant Folding
status: complete
completed: 2026-02-09
goal: Evaluate compile-time-known expressions during canonicalization and store results in the constant pool
sections:
  - id: "04.1"
    title: Constness Classification
    status: complete
  - id: "04.2"
    title: Folding During Lowering
    status: complete
  - id: "04.3"
    title: Completion Checklist
    status: complete
---

# Section 04: Constant Folding

**Status:** Complete (2026-02-09)
**Goal:** Identify and evaluate compile-time-known expressions during the lowering pass. Folded values are stored in `ConstantPool` and emitted as `CanExpr::Constant(ConstantId)`. Both backends skip evaluation/emission for constant nodes.

**File:** `compiler/ori_canon/src/const_fold.rs`

**Prior art:**
- **Roc** — Constants interned in `InternPool`; monomorphizer propagates constant values
- **Elm** — Boolean constructor optimization (`Bool True` → enum variant)
- **Zig** — `comptime` evaluates arbitrary expressions at compile time
- **Go** `cmd/compile/internal/ir/const.go` — Constant folding during IR construction; representation promotion

**Scope:** Simple constant folding — expressions whose values can be determined from their operands alone. Does NOT cover CTFE, function call memoization, or algebraic simplification (future work).

---

## 04.1 Constness Classification

```rust
enum Constness {
    /// Value known at compile time
    Const,
    /// Depends on runtime values
    Runtime,
}
```

- [x] Implement `classify(arena: &CanArena, id: CanId) -> Constness`
  - [x] Literals (Int, Float, Bool, Str, Char, Unit, Duration, Size) → `Const`
  - [x] `Const(name)` → `Const` ($-bindings are compile-time by definition)
  - [x] `Binary { left, right }` → `Const` if both children `Const` and operator is pure
  - [x] `Unary { operand }` → `Const` if operand `Const` and operator is pure
  - [x] `If { cond, .. }` → `Const` if cond is `Const` (enables dead branch elimination)
  - [x] `List/Tuple/Map` → `Const` if all elements `Const`
  - [x] Everything else → `Runtime`
- [x] Pure operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `&`, `|`, `^`, `~`, `<<`, `>>`

---

## 04.2 Folding During Lowering

Constant folding is **integrated into the lowering pass** (not a separate traversal). When lowering a node, the lowerer checks if the result is constant and, if so, evaluates it immediately.

- [x] After lowering children, check if the result is `Const`
  - [x] If `Const`: evaluate using a minimal const evaluator → store in `ConstantPool` → emit `CanExpr::Constant(id)`
  - [x] If `Runtime`: emit the normal `CanExpr` node
- [x] Implement const evaluation for foldable operations:
  - [x] Integer arithmetic with overflow detection (overflow → don't fold, let runtime handle)
  - [x] Float arithmetic
  - [x] Boolean logic
  - [x] Comparisons
  - [x] Bitwise operations
  - [x] String operations (limited: just literal concatenation)
  - [x] Negation
- [x] Dead branch elimination:
  - [x] `if true { A } else { B }` → fold to lowered A
  - [x] `if false { A } else { B }` → fold to lowered B
- [x] Do NOT fold:
  - [x] Division by zero (runtime error with proper span)
  - [x] Integer overflow (runtime panic with context)
  - [x] Function calls (requires CTFE infrastructure — future Section 05.3 ConstEval mode)

---

## 04.3 Completion Checklist

- [x] Constness classification covers all literal and pure-arithmetic expressions
- [x] Constant evaluator handles all pure operators without panicking
- [x] Dead branch elimination for `if true/false` patterns
- [x] Folded values stored in `ConstantPool` with dedup
- [x] Backends skip constant nodes (return value directly or emit LLVM constant)
- [x] `./test-all.sh` passes — no behavioral change (pure optimization)
- [x] Overflow and division-by-zero correctly deferred to runtime

**Exit Criteria:** Compile-time-known expressions become `CanExpr::Constant` nodes during lowering. Both backends handle constant nodes trivially. All tests pass unchanged.
