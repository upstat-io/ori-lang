---
section: "04"
title: Constant Folding
status: not-started
goal: Evaluate compile-time-known expressions during canonicalization and store results in the constant pool
sections:
  - id: "04.1"
    title: Constness Classification
    status: not-started
  - id: "04.2"
    title: Folding During Lowering
    status: not-started
  - id: "04.3"
    title: Completion Checklist
    status: not-started
---

# Section 04: Constant Folding

**Status:** Not Started
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

- [ ] Implement `classify(arena: &CanArena, id: CanId) -> Constness`
  - [ ] Literals (Int, Float, Bool, Str, Char, Unit, Duration, Size) → `Const`
  - [ ] `Const(name)` → `Const` ($-bindings are compile-time by definition)
  - [ ] `Binary { left, right }` → `Const` if both children `Const` and operator is pure
  - [ ] `Unary { operand }` → `Const` if operand `Const` and operator is pure
  - [ ] `If { cond, .. }` → `Const` if cond is `Const` (enables dead branch elimination)
  - [ ] `List/Tuple/Map` → `Const` if all elements `Const`
  - [ ] Everything else → `Runtime`
- [ ] Pure operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `&`, `|`, `^`, `~`, `<<`, `>>`

---

## 04.2 Folding During Lowering

Constant folding is **integrated into the lowering pass** (not a separate traversal). When lowering a node, the lowerer checks if the result is constant and, if so, evaluates it immediately.

- [ ] After lowering children, check if the result is `Const`
  - [ ] If `Const`: evaluate using a minimal const evaluator → store in `ConstantPool` → emit `CanExpr::Constant(id)`
  - [ ] If `Runtime`: emit the normal `CanExpr` node
- [ ] Implement const evaluation for foldable operations:
  - [ ] Integer arithmetic with overflow detection (overflow → don't fold, let runtime handle)
  - [ ] Float arithmetic
  - [ ] Boolean logic
  - [ ] Comparisons
  - [ ] Bitwise operations
  - [ ] String operations (limited: just literal concatenation)
  - [ ] Negation
- [ ] Dead branch elimination:
  - [ ] `if true { A } else { B }` → fold to lowered A
  - [ ] `if false { A } else { B }` → fold to lowered B
- [ ] Do NOT fold:
  - [ ] Division by zero (runtime error with proper span)
  - [ ] Integer overflow (runtime panic with context)
  - [ ] Function calls (requires CTFE infrastructure — future Section 05.3 ConstEval mode)

---

## 04.3 Completion Checklist

- [ ] Constness classification covers all literal and pure-arithmetic expressions
- [ ] Constant evaluator handles all pure operators without panicking
- [ ] Dead branch elimination for `if true/false` patterns
- [ ] Folded values stored in `ConstantPool` with dedup
- [ ] Backends skip constant nodes (return value directly or emit LLVM constant)
- [ ] `./test-all.sh` passes — no behavioral change (pure optimization)
- [ ] Overflow and division-by-zero correctly deferred to runtime

**Exit Criteria:** Compile-time-known expressions become `CanExpr::Constant` nodes during lowering. Both backends handle constant nodes trivially. All tests pass unchanged.
