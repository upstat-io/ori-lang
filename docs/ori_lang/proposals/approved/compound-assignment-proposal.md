# Proposal: Compound Assignment Operators

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-21
**Approved:** 2026-02-21
**Affects:** Lexer, parser, IR, type checker, evaluator, LLVM codegen
**Depends on:** Operator Traits (approved), MatMul Operator (approved)

---

## Summary

Add compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`, `@=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `&&=`, `||=`) that desugar to `x = x op y`. This is pure syntactic sugar — no new traits, no new semantics. The left-hand side must be a mutable binding (no `$` prefix).

---

## Motivation

### Universally Expected Syntax

Compound assignment exists in virtually every mainstream language: C, C++, Java, Python, Rust, Swift, Go, TypeScript, Zig, Kotlin, C#, Ruby, and more. Its absence is noticed immediately by every programmer who writes Ori code.

The current error message ("Use `x = x + y` instead of `x += y`") teaches the workaround, but the workaround is strictly worse:

```ori
// Current: verbose, repeats the variable name, error-prone for long names
accumulated_weighted_loss = accumulated_weighted_loss + batch_loss * weight;

// With this proposal: clear, concise, DRY
accumulated_weighted_loss += batch_loss * weight;
```

When variable names are short, the difference is minor. When they are descriptive (as they should be), the repetition becomes a readability problem — the reader must visually verify that the left-hand and right-hand variable names match.

### ML/Scientific Computing

Matrix computations frequently accumulate into existing variables:

```ori
// Current
gradient = gradient + learning_rate * delta;
weights = weights - gradient;
result = result @ batch;

// With this proposal
gradient += learning_rate * delta;
weights -= gradient;
result @= batch;
```

### Loop Accumulators

The most common use case — loop variables that accumulate:

```ori
let sum = 0;
for item in items {
    // Current
    sum = sum + item.value;
    // With this proposal
    sum += item.value;
}
```

---

## Design

### Desugaring (Not Separate Traits)

`x op= y` desugars to `x = x op y` at the parser level, before type checking. This means:

1. **No new traits** — reuses existing `Add`, `Sub`, `Mul`, etc.
2. **No new type checker logic** — the desugared form is already supported
3. **No new evaluator logic** — evaluates the desugared assignment
4. **Same semantics as writing it out** — no hidden in-place mutation

This is the approach used by Swift and Zig. Rust uses separate traits (`AddAssign`, `SubAssign`, etc.) to enable in-place mutation, but Ori's ARC memory model can optimize unique-reference reassignment without exposing `&mut self` semantics.

> **Note:** If performance analysis later shows that desugaring creates unnecessary copies for large types, Ori can add optional `*Assign` traits as an optimization without changing user-facing semantics. The desugaring remains the default behavior.

### Supported Operators

#### Trait-Based Operators

| Compound | Desugars To | Binary Trait |
|----------|-------------|-------------|
| `+=` | `x = x + y` | `Add` |
| `-=` | `x = x - y` | `Sub` |
| `*=` | `x = x * y` | `Mul` |
| `/=` | `x = x / y` | `Div` |
| `%=` | `x = x % y` | `Rem` |
| `@=` | `x = x @ y` | `MatMul` |
| `&=` | `x = x & y` | `BitAnd` |
| `\|=` | `x = x \| y` | `BitOr` |
| `^=` | `x = x ^ y` | `BitXor` |
| `<<=` | `x = x << y` | `Shl` |
| `>>=` | `x = x >> y` | `Shr` |

#### Logical Operators

| Compound | Desugars To | Mechanism |
|----------|-------------|-----------|
| `&&=` | `x = x && y` | Logical AND (`bool`-only, short-circuit preserved) |
| `\|\|=` | `x = x \|\| y` | Logical OR (`bool`-only, short-circuit preserved) |

The `&&` and `||` operators are not trait-based — they are hardcoded for `bool` with short-circuit semantics. The desugaring preserves short-circuit behavior because the right-hand side of the desugared form is `x && y` or `x || y`, which already short-circuits.

> **Note:** The `@` token is also used as the function declaration sigil (`@main`, `@add`). The parser disambiguates by syntactic context: `@` in item position introduces a function declaration; `@` and `@=` in expression position are the MatMul operator and compound MatMul assignment respectively. See the [MatMul Operator Proposal](../approved/matmul-operator-proposal.md) for disambiguation rules.

### Excluded Operators

| Operator | Reason |
|----------|--------|
| `div=` | `div` is a keyword operator, not a symbol. `x div= y` is syntactically awkward and has no precedent in any language. Use `x = x div y`. |
| `??=` | Coalesce assignment has a type mismatch: `x ?? y` where `x: Option<T>` and `y: T` produces `T`, which cannot be assigned back to `x: Option<T>`. Requires separate specification for the desired semantics. |

### Mutability Requirement

The left-hand side must be a mutable binding (declared without `$`):

```ori
let x = 10;
x += 5;        // Valid — mutable binding

let $y = 10;
$y += 5;       // Error — immutable binding
```

The error message for immutable bindings should be: "cannot use compound assignment on immutable binding `$y`. Remove `$` for mutability: `let y = ...`"

### Left-Hand Side Forms

Compound assignment supports any assignable target:

```ori
x += 1;                    // Variable
point.x += delta;          // Field access
matrix[i][j] += value;     // Subscript
self.weights += gradient;  // Self field
```

The desugaring preserves the target expression:
- `point.x += delta` → `point.x = point.x + delta`
- `matrix[i][j] += value` → `matrix[i][j] = matrix[i][j] + value`

> **Note:** The desugaring duplicates the target expression in the AST (once as assignment target, once as binary operand). For field access and subscript access, the target is referenced twice. This is semantically equivalent to single evaluation because field access and subscript indexing are pure (no side effects) in Ori.

### Expression vs Statement

Compound assignment is a **statement**, not an expression. It does not produce a value:

```ori
let result = (x += 1);   // Error — compound assignment is not an expression
```

This prevents the C-style confusion of `if (x += 1)` and keeps Ori's expression semantics clean. Regular assignment (`x = expr`) is already a statement in Ori.

---

## Grammar Changes

```ebnf
(* New compound assignment operators *)
compound_assign_op = "+=" | "-=" | "*=" | "/=" | "%=" | "@="
                   | "&=" | "|=" | "^=" | "<<=" | ">>="
                   | "&&=" | "||=" .

(* Updated assignment statement *)
assign_stmt = assignable "=" expr ";"
            | assignable compound_assign_op expr ";" .
```

---

## Implementation

### Changes by Crate

| Crate | File | Change |
|-------|------|--------|
| `ori_lexer_core` | `tag/mod.rs` | Add 13 new token tags: `PlusEq`, `MinusEq`, `StarEq`, `SlashEq`, `PercentEq`, `AtEq`, `AmpEq`, `PipeEq`, `CaretEq`, `ShlEq`, `ShrEq`, `AmpAmpEq`, `PipePipeEq` |
| `ori_lexer_core` | `raw_scanner/mod.rs` | Scan two-char sequences (`+=`, `-=`, `*=`, `/=`, `%=`, `@=`, `&=`, `\|=`, `^=`) and three-char (`<<=`, `>>=`, `&&=`, `\|\|=`) |
| `ori_lexer` | `cook.rs` | Map raw tags to `TokenKind` compound assignment variants |
| `ori_parse` | `grammar/expr/` | Parse compound assignment, desugar to `Expr::Assign { target, value: Expr::Binary { target, op, rhs } }` |
| `ori_parse` | `error/mistakes.rs` | **Remove** the "common mistake" detection for compound assignment operators (keep `??=` as mistake since it remains unsupported) |
| `ori_ir` | — | No change — desugared before reaching IR |
| `ori_types` | — | No change — sees only the desugared `x = x op y` |
| `ori_eval` | — | No change — evaluates the desugared assignment |
| `ori_llvm` | — | No change — compiles the desugared assignment |

### Parser Desugaring Detail

When the parser encounters `target op= expr`:

1. Parse the left-hand side as an assignable expression
2. Recognize the compound operator token (e.g., `PlusEq`)
3. Map to the corresponding `BinaryOp` (e.g., `BinaryOp::Add`) or logical operator (`&&`, `||`)
4. Parse the right-hand side expression
5. Emit: `Assign { target, value: Binary { left: target, op, right: rhs } }`

The target expression is duplicated in the AST (once as assignment target, once as binary left operand). This is correct because the parser uses arena allocation — the duplication is just two `ExprId` references.

For `&&=` and `||=`, the desugaring uses the existing `ExprKind::And` / `ExprKind::Or` nodes (not `Binary`), which already implement short-circuit evaluation.

---

## Examples

### Accumulator Pattern

```ori
@sum_squares (values: [int]) -> int = {
    let total = 0;
    for v in values {
        total += v * v;
    }
    total
}
```

### In-Place Update

```ori
@normalize (self) -> void = {
    let magnitude = self.length();
    self.x /= magnitude;
    self.y /= magnitude;
    self.z /= magnitude;
}
```

### Bitwise Flags

```ori
@set_permissions (path: str, read: bool, write: bool, execute: bool) -> int = {
    let flags = 0;
    if read    { flags |= READ_FLAG; }
    if write   { flags |= WRITE_FLAG; }
    if execute { flags |= EXEC_FLAG; }
    flags
}
```

### Logical Short-Circuit

```ori
@validate_all (checks: [() -> bool]) -> bool = {
    let passed = true;
    for check in checks {
        passed &&= check();   // Short-circuits: if passed is already false, check() not called
    }
    passed
}
```

---

## Design Decisions

### Why desugar instead of separate traits?

Rust uses separate `AddAssign`, `SubAssign` traits to allow in-place mutation via `&mut self`. Ori's ARC memory model doesn't expose `&mut self` — mutation happens through mutable bindings, and ARC can optimize unique-reference reassignment internally. Separate traits would add 11 new traits to the prelude with zero semantic benefit.

If performance profiling later shows unnecessary copies, Ori can add optional `*Assign` traits as an optimization path without changing the user-facing desugaring semantics.

### Why exclude `div=`?

The `div` operator uses a keyword, not a symbol. No language has `div=` as a compound form. The asymmetry (`+=` exists but `div=` doesn't) is acceptable because `div` (floor division) is far less common than `/` (true division), and `x = x div y` is perfectly readable for the rare cases it appears.

### Why include `&&=` and `||=` but exclude `??=`?

`&&=` and `||=` desugar straightforwardly: `x &&= y` → `x = x && y`. The short-circuit semantics are preserved because the desugared form uses the short-circuiting `&&`/`||` operators. Both operate on `bool` only, and the result type (`bool`) matches the binding type.

`??=` has a genuine type system complication: `x ?? y` where `x: Option<T>` and `y: T` produces `T`, which cannot be assigned back to `x: Option<T>` without additional semantic specification. This requires a separate proposal.

---

## Verification

1. `x += 1` desugars to `x = x + 1` and type-checks
2. All 13 compound operators parse and desugar correctly
3. Immutable binding `$y += 1` produces clear error message
4. Field access `point.x += delta` desugars correctly
5. Subscript `arr[i] += value` desugars correctly
6. Compound assignment is a statement, not an expression
7. Existing `@` function declarations unaffected by `@=` token
8. The "common mistake" error for compound assignment operators is removed
9. Precedence of RHS: `x += a * b` desugars to `x = x + (a * b)` (RHS is full expression)
10. `&&=` short-circuits correctly: `x &&= expr` does not evaluate `expr` when `x` is `false`
11. `||=` short-circuits correctly: `x ||= expr` does not evaluate `expr` when `x` is `true`
