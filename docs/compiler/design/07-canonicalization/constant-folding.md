---
title: "Constant Folding"
description: "Ori Compiler Design — Compile-Time Constant Evaluation"
order: 703
section: "Canonicalization"
---

# Constant Folding

## What Is Constant Folding?

When a programmer writes `let $tau = 2 * 3.14159`, the multiplication has the same result every time — both operands are known at compile time. **Constant folding** is the compiler optimization that evaluates such expressions during compilation, replacing them with their computed result. The runtime never performs the multiplication; it simply uses the pre-computed value.

The idea is as old as optimizing compilers. The term appears in IBM's FORTRAN compiler documentation from the early 1960s, where it referred to evaluating arithmetic on literal constants during compilation. The technique is a special case of **partial evaluation** — evaluating parts of a program that depend only on static (compile-time-known) inputs while leaving dynamic (runtime-dependent) parts untouched. Futamura's 1971 work formalized this connection, showing that an interpreter partially evaluated with respect to a program produces a compiled version of that program.

### Why Fold Constants?

Constant folding provides three benefits:

**Runtime performance.** Every folded expression is one fewer operation at runtime. For tight loops over pre-computed tables, array dimensions, or mathematical constants, this eliminates work that would otherwise be repeated on every execution.

**Code size reduction.** A folded constant is typically smaller in the compiled output than the expression that computes it. `Constant(id)` occupies one node in the canonical IR, while `Binary { Add, Int(1), Int(2) }` occupies three nodes. For LLVM codegen, a constant can be emitted as an immediate operand rather than a sequence of instructions.

**Enabling downstream optimizations.** When a condition folds to `true` or `false`, the compiler can eliminate the dead branch entirely — a transformation called **dead branch elimination** (or dead code elimination for the unreachable side). This can expose further folding opportunities in the surviving branch.

### The Spectrum of Compile-Time Evaluation

Constant folding is the simplest form of compile-time evaluation. More powerful forms exist:

**Constant propagation** tracks the values of named constants across assignments and substitutes them at use sites. If `let $x = 3` is followed by `$x + 1`, constant propagation substitutes `3` for `$x`, enabling the folder to compute `4`. Ori's constant folding does *not* perform constant propagation — named constants (`$name`) are classified as runtime because their values aren't resolved at the canonicalization level.

**Partial evaluation** evaluates arbitrary code when sufficient inputs are known statically. Zig's `comptime` is the most aggressive example in a systems language — it can execute loops, function calls, and memory allocation at compile time. Ori's constant folding is deliberately simpler: it evaluates only pure arithmetic, logical, and comparison operations on literal values.

**Abstract interpretation** evaluates programs over abstract domains (e.g., "positive," "negative," "zero") rather than concrete values, enabling optimizations like range checking and overflow detection without knowing exact values. This is a separate optimization family from constant folding.

Ori positions itself at the simplest end of this spectrum. The folder handles literal arithmetic and dead branch elimination, leaving constant propagation and more sophisticated partial evaluation to future optimization passes or to LLVM's own optimization pipeline.

### When to Fold: Separate Pass vs Inline

Compilers typically fold constants in one of two ways:

**Separate pass.** A dedicated constant-folding pass traverses the IR, identifies foldable expressions, and replaces them. This is clean and modular — the pass can be tested, enabled, or disabled independently. GCC and LLVM both have dedicated constant-folding passes. The downside is an additional traversal of the entire IR.

**Inline during construction.** When the compiler constructs an IR node, it immediately checks whether the node can be folded and replaces it on the spot. This avoids a separate traversal but couples folding logic with IR construction. Ori uses this approach — constant folding is integrated into the `lower_expr()` method of the canonicalization phase.

The inline approach is natural for Ori because the lowerer already traverses every expression once. After constructing a `Binary`, `Unary`, or `If` node, a single call to `try_fold()` checks whether the node can be replaced with a constant. The check is fast (classify the node and its children), and the common case (runtime expressions) returns `None` immediately.

## How Ori Folds Constants

### Integration with Lowering

Constant folding is **not** a separate pass. It runs inline during `lower_expr()`, immediately after each arithmetic, logical, or conditional node is constructed:

```rust
// In lower/expr.rs — after building a Binary node
let id = self.push(CanExpr::Binary { op, left, right }, span, ty);
if let Some(folded) = const_fold::try_fold(&mut self.arena, &mut self.constants, id) {
    folded   // Replace with Constant(id)
} else {
    id       // Keep as Binary
}
```

The same pattern applies to `Unary` and `If` nodes. Because lowering is recursive, nested constant expressions fold bottom-up: `(1 + 2) * 3` first folds `1 + 2` to `3`, then folds `3 * 3` to `9`.

### Constness Classification

Before attempting to fold, the classifier determines whether an expression is compile-time evaluable:

| Expression Kind | Classification | Reason |
|-----------------|---------------|--------|
| `Int`, `Float`, `Bool`, `Str`, `Char`, `Unit` | Const | Literal values are known at compile time |
| `Duration`, `Size` | Const | Literal duration/size values |
| `Constant(id)` | Const | Already folded — prevents re-classification |
| `Binary { op, left, right }` | Const if operator is pure AND both operands are Const | All three conditions must hold |
| `Unary { op, operand }` | Const if operator is pure AND operand is Const | Both conditions must hold |
| `If { cond, ... }` | Const if condition is Const | Enables dead branch elimination regardless of branch contents |
| `Ident`, `Const($name)` | Runtime | Variable and named constant values aren't resolved at this phase |
| Everything else | Runtime | Function calls, method calls, indexing, etc. |

The `If` classification is deliberately permissive: an `if` expression is considered foldable when only the *condition* is constant, even if the branches contain runtime code. This enables `if true { complex_expr } else { other_expr }` to collapse to `complex_expr` without evaluating or classifying the branches.

### Pure Operators

Only **pure** operators — those with no side effects and deterministic results — are candidates for folding:

**Binary (pure):** `Add`, `Sub`, `Mul`, `Div`, `Mod`, `FloorDiv`, `Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`

**Unary (pure):** `Neg`, `Not`, `BitNot`

Operators not in these lists (method calls, string concatenation via the `Concat` operator) are never folded, even if their operands are constant. This is a conservative choice — some of these operations could theoretically be folded, but the folder limits itself to operations with well-defined, platform-independent semantics.

### What Gets Folded

#### Integer Arithmetic

```ori
1 + 2          // → Constant(3)
10 - 7         // → Constant(3)
6 * 7          // → Constant(42)
10 / 3         // → Constant(3)        (truncating division)
10 div 3       // → Constant(3)        (floor division)
-7 div 2       // → Constant(-4)       (rounds toward -∞)
10 % 3         // → Constant(1)
-42            // → Constant(-42)
```

All integer arithmetic uses Rust's `checked_*()` methods. If an operation would overflow (e.g., `i64::MAX + 1`), folding is **declined** — the expression remains as a runtime node, where Ori's overflow detection will panic with a clear error message. The compiler never silently wraps.

Floor division (`div`) uses sign-aware rounding: when the operands have different signs and the remainder is nonzero, the quotient is adjusted by subtracting 1 to round toward negative infinity rather than toward zero.

#### Float Arithmetic

```ori
3.14 * 2.0     // → Constant(6.28)
1.0 / 3.0      // → Constant(0.333...)
-2.5           // → Constant(-2.5)
```

Floats are stored as `u64` bit patterns (`f64::to_bits()`) throughout the constant pool. This ensures deterministic hashing and exact deduplication — two constants with identical bit patterns always share one pool entry, and NaN bit patterns hash consistently (unlike `f64`, which has `NaN != NaN`).

Float division by zero is **not folded** — it's deferred to runtime, where IEEE-754 semantics produce `+∞`, `-∞`, or `NaN` as appropriate.

#### Boolean Logic and Comparisons

```ori
true && false  // → Constant(false)
!true          // → Constant(false)
1 < 2          // → Constant(true)
"a" == "a"     // → Constant(true)
```

Boolean `And` and `Or` are folded as pure operations — both operands are always evaluated. This is safe because both sides are already classified as constants, so evaluation order is irrelevant and no side effects can be missed.

#### Bitwise Operations

```ori
0xFF & 0x0F    // → Constant(15)
1 << 3         // → Constant(8)
~0             // → Constant(-1)
```

Shift operations have careful validation: negative shift counts and counts ≥ 64 are rejected (deferred to runtime, where they panic). For left shifts, a round-trip check (`(a << n) >> n == a`) detects overflow — if the original value can't be recovered, the shift is declined.

#### Duration and Size Arithmetic

```ori
1s + 500ms        // → Constant(1500000000ns)
2 * 100ms         // → Constant(200000000ns)
1kb + 500b        // → Constant(1500b)
```

Duration and Size values are normalized to their base units (nanoseconds and bytes respectively) before folding. Cross-unit comparisons like `1000ms == 1s` evaluate correctly because both sides normalize to the same nanosecond value. Results are always stored in the base unit to avoid unit-mismatch bugs in downstream phases.

Size arithmetic rejects negative results — the `Size` type is non-negative by definition, so operations like multiplying by a negative integer or subtracting to produce a negative result are deferred to runtime.

### Dead Branch Elimination

When an `if` condition folds to a boolean constant, the entire conditional collapses:

```ori
if true then complex_computation() else fallback()
// → complex_computation()   (else branch eliminated)

if false then unreachable_code() else safe_default()
// → safe_default()          (then branch eliminated)
```

This works even when the branches contain runtime expressions — the classifier only requires the *condition* to be constant. The surviving branch is returned directly; the dead branch is never evaluated or included in the canonical IR.

One edge case: `if false { expr }` with no else clause returns `None` (folding declined). Without an else branch, the block evaluates to `void`, which still needs a canonical representation. The runtime will handle this as an unreachable `if` that produces `void`.

### What Does NOT Get Folded

| Expression | Why Not Folded |
|-----------|----------------|
| `$PI * 2.0` | Named constants (`$name`) aren't resolved at canonicalization |
| `len([1, 2, 3])` | Function calls may have side effects |
| `"hello" ++ " world"` | String concatenation uses the `Concat` operator (method-based) |
| `[1, 2, 3]` | Collection literals involve allocation |
| `i64::MAX + 1` | Integer overflow — deferred to runtime panic |
| `1 / 0` | Division by zero — deferred to runtime panic |
| `1.0 / 0.0` | Float division by zero — deferred to runtime (IEEE-754) |
| `1 << 64` | Shift count out of range — deferred to runtime panic |

The folder is deliberately conservative: if there's any possibility that an operation would panic, produce platform-dependent results, or require capabilities beyond pure arithmetic, it declines and lets the runtime handle it. This ensures that constant folding never changes program semantics — a folded program behaves identically to an unfolded one.

## ConstantPool

The `ConstantPool` is a content-addressed store of compile-time values:

```rust
pub struct ConstantPool {
    values: Vec<ConstValue>,
    dedup: FxHashMap<ConstValue, ConstantId>,
}
```

### Content Addressing

When a constant is interned, the pool checks its dedup map first. If an identical value already exists, the existing `ConstantId` is returned — no new entry is allocated. This guarantees that identical constant expressions across a module share a single pool entry.

```rust
pub fn intern(&mut self, value: ConstValue) -> ConstantId {
    if let Some(&id) = self.dedup.get(&value) {
        return id;  // Reuse existing
    }
    let id = ConstantId::new(self.values.len());
    self.dedup.insert(value.clone(), id);
    self.values.push(value);
    id
}
```

### Pre-Interned Sentinels

The six most common constant values are pre-interned at construction time for O(1) access:

| Sentinel | Value | ConstantId |
|----------|-------|------------|
| `UNIT` | `()` | 0 |
| `TRUE` | `true` | 1 |
| `FALSE` | `false` | 2 |
| `ZERO` | `0` (int) | 3 |
| `ONE` | `1` (int) | 4 |
| `EMPTY_STR` | `""` | 5 |

These sentinels cover the most frequent folding results — boolean conditions, zero-checks, unit returns — without requiring a hash lookup.

### ConstValue

```rust
pub enum ConstValue {
    Int(i64),
    Float(u64),      // IEEE-754 bit pattern, not f64
    Bool(bool),
    Str(Name),       // Interned string reference
    Char(char),
    Unit,
    Duration { value: u64, unit: DurationUnit },
    Size { value: u64, unit: SizeUnit },
}
```

The `Float(u64)` representation is a deliberate design choice. Storing IEEE-754 bit patterns rather than `f64` values ensures:

- **Hashability** — `f64` doesn't implement `Hash` (because `NaN != NaN`), but `u64` does
- **Deterministic deduplication** — identical bit patterns always map to the same pool entry
- **Platform consistency** — bit-level representation avoids cross-platform floating-point representation differences

## Prior Art

### GCC — Constant Folding as Dedicated Passes

[GCC](https://gcc.gnu.org/) performs constant folding at multiple levels. The `fold-const.cc` module handles algebraic simplification during tree construction (similar to Ori's inline approach), while the `tree-ssa-ccp.cc` pass performs **Conditional Constant Propagation** — a more powerful analysis that combines constant folding with reachability analysis to find constants that simple folding misses. GCC also performs constant folding in its RTL (Register Transfer Language) backend, catching opportunities that higher-level passes missed. Ori's single-level inline approach is simpler but handles fewer cases.

### LLVM — InstCombine and SCCP

[LLVM](https://llvm.org/) separates constant folding into several passes. `ConstantFolding.cpp` handles pure compile-time evaluation of arithmetic and intrinsics. `InstructionCombining` (InstCombine) performs algebraic simplification and strength reduction (replacing expensive operations with cheaper equivalents). `SCCP` (Sparse Conditional Constant Propagation) is the most powerful, combining constant propagation with unreachable code elimination in a single fixed-point analysis. Since Ori emits LLVM IR for its AOT backend, LLVM's passes will catch folding opportunities that Ori's simple inline folder misses — Ori's folder is not redundant, though, because it also benefits the tree-walking evaluator, which doesn't go through LLVM.

### Zig — Comptime as Full Partial Evaluation

[Zig](https://github.com/ziglang/zig) takes compile-time evaluation to its logical extreme with `comptime`. Any Zig expression can be evaluated at compile time if its inputs are known, including loops, function calls, memory allocation, and type manipulation. This makes Zig's "constant folding" a full-featured interpreter embedded in the compiler. The tradeoff is implementation complexity — Zig's `Sema.zig` (which implements comptime evaluation) is enormous. Ori's approach is deliberately more limited: fold literals, skip everything else, and let LLVM handle the rest.

### Roc — Constant Folding in Mono IR

[Roc](https://github.com/roc-lang/roc) performs constant folding during monomorphization (`crates/compiler/mono/src/ir.rs`), when type variables have been resolved to concrete types. This is later in the pipeline than Ori's canonicalization-phase folding, which means Roc can fold expressions that depend on monomorphized type information. Ori folds earlier (during canonicalization), which means fewer folding opportunities but simpler implementation — type information is already resolved from the type checker, and no monomorphization pass exists.

### Rust — MIRI and Constant Evaluation

[Rust](https://github.com/rust-lang/rust) evaluates `const` expressions using MIRI (Mid-level IR Interpreter), a full interpreter for Rust's MIR. This is far more powerful than simple constant folding — MIRI can evaluate function calls, loops, and even heap allocation (within limits). Rust's `const fn` system allows annotating which functions are eligible for compile-time evaluation, giving the programmer control over what the compiler attempts. Ori has no equivalent of `const fn` — its folding is limited to expressions that are *obviously* constant (literal operands, pure operators).

### Elm — No Explicit Constant Folding

[Elm](https://github.com/elm/compiler) performs no explicit constant-folding pass. The Elm compiler relies on the JavaScript engine's JIT compiler to fold constants at runtime. This is a valid strategy for a language that compiles to a JIT-compiled target — the runtime optimizer handles constant folding "for free." Ori cannot rely on this because it targets both a tree-walking interpreter (no JIT) and LLVM (where folding before codegen produces better IR).

## Design Tradeoffs

**Inline folding vs. separate pass.** Ori folds constants inline during `lower_expr()` rather than in a dedicated pass. This avoids an extra traversal of the canonical IR and ensures that folded constants are available immediately to the rest of the lowering process (e.g., dead branch elimination can expose further folding in the surviving branch). The tradeoff is coupling — the folding logic is invoked from the lowerer rather than being an independent, composable transformation. A separate pass would be easier to test in isolation and could be enabled/disabled independently.

**Conservative overflow handling.** When an integer operation would overflow, Ori declines to fold rather than wrapping or producing a sentinel value. The expression remains as a runtime node, where Ori's overflow detection will panic with a source-located error message. This is more conservative than C (undefined behavior), Rust (wrapping in release mode), or Java (silent wrapping). The tradeoff is that some theoretically foldable expressions — like `i64::MAX + 1 - 1`, which would produce `i64::MAX` after wrapping — are not folded. Correctness over cleverness.

**No constant propagation.** Named constants (`$name`) are classified as runtime because their values aren't resolved during canonicalization. Adding constant propagation would require threading resolved constant values through the lowerer, which adds complexity. Since LLVM performs its own constant propagation on the AOT path, the optimization gap primarily affects the tree-walking evaluator. This is an acceptable tradeoff for the current implementation.

**Float bit-pattern storage.** Storing floats as `u64` bit patterns enables hashing and exact deduplication but requires `f64::from_bits()` / `to_bits()` conversions at every folding operation. The conversion is a no-op on all modern architectures (same register, different interpretation), so the runtime cost is zero. The benefit — deterministic, hashable, platform-independent float constants — is substantial.

**Limited scope of folding.** Ori folds only pure arithmetic, logical, comparison, and bitwise operations on literal values. It does not fold function calls, method calls, string concatenation, collection operations, or any expression that might allocate or have side effects. This limitation means some "obviously constant" expressions (like `len([1, 2, 3])` or `"a" ++ "b"`) are not folded. The conservative scope keeps the folder simple, correct, and predictable — programmers can reason about what will and won't be folded without understanding the folder's internals.

## Related Documents

- [Canonicalization Overview](index.md) — Pipeline position and canonical IR architecture
- [Desugaring](desugaring.md) — Sugar elimination (produces the nodes that constant folding operates on)
- [Pattern Compilation](pattern-compilation.md) — Decision tree construction (runs alongside constant folding)
