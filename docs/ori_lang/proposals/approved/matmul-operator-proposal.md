# Proposal: `@` Matrix Multiplication Operator

**Status:** Approved
**Approved:** 2026-02-21
**Author:** Eric
**Created:** 2026-02-21
**Affects:** Lexer, parser, IR, type checker, evaluator, LLVM codegen, standard library prelude
**Depends on:** Operator Traits (approved)

---

## Summary

Add `@` as a binary operator for matrix multiplication, following Python's PEP 465 convention. The operator desugars to the `MatMul` trait method `matrix_multiply()`, following the same pattern as all existing operator traits. The `@` token is already used as the function declaration sigil — the parser disambiguates by syntactic context (item position vs expression position), which is already how every language with overloaded tokens works.

---

## Motivation

### The Copy-Paste Adoption Vector

ML adoption depends on researchers translating existing Python/PyTorch code. The core mathematical operations must transfer with minimal friction:

```python
# Python — every ML researcher writes this
attn = softmax((q @ k.transpose(-2, -1)) * scale, dim=-1)
out = attn @ v
```

Without `@`:

```ori
// Ori today — method calls break the math notation
let $attn = softmax(input: q.matmul(other: k.T) * scale, dim: -1);
let $out = attn.matmul(other: v);
```

With `@`:

```ori
// Ori with this proposal — identical to Python for the math
let $attn = softmax(input: q @ k.T * scale, dim: -1);
let $out = attn @ v;
```

The mathematical expression `q @ k.T * scale` is **character-for-character identical** between Python and Ori. This is not a convenience — it is the difference between "translate this line" and "copy this line." At scale across thousands of lines of model code, that difference determines adoption.

### Prior Art: Python PEP 465

Python added `@` for matmul in 2014 (Python 3.5) specifically because the ML/scientific computing community needed it. The rationale:

- `*` was already taken for element-wise multiplication
- Matrix multiplication is common enough to deserve an operator
- `numpy.dot(a, b)` and `a.dot(b)` were too verbose for mathematical notation
- The `@` symbol visually suggests "at" as in "a at b" — a matrix operation

PEP 465 has been an unqualified success. Every major ML framework uses it. Ori should match it exactly.

### Why Not Another Symbol?

| Symbol | Problem |
|--------|---------|
| `**` | Universally means exponentiation (Python, Ruby, etc.) — would confuse ML researchers |
| `*@` | Novel — no prior art, nothing to copy-paste from |
| `@@` | Visually heavy, no precedent |
| `><` | Unusual, no mathematical meaning |
| `@` | **Exact Python match. Zero mental translation.** |

---

## Design

### Trait Definition

Added to the standard library prelude, following the same pattern as `Add`, `Mul`, etc.:

```ori
trait MatMul<Rhs = Self> {
    type Output = Self
    @matrix_multiply (self, rhs: Rhs) -> Self.Output
}
```

### Operator Desugaring

`a @ b` desugars to `MatMul.matrix_multiply(a, rhs: b)`, identical to how `a + b` desugars to `Add.add(a, rhs: b)`.

### Precedence

`@` sits at the same precedence as `*`, `/`, `%`, `div` (multiplicative, level 3). This matches Python's precedence for `@` and gives the mathematically expected behavior:

```ori
// q @ k.T * scale
// Parsed as: (q @ k.T) * scale — correct
// Because @ and * are same precedence, left-associative
```

Updated precedence table:

| Level | Operators |
|-------|-----------|
| 3 | `*` `/` `%` `div` **`@`** |
| 4 | `+` `-` |
| ... | ... |

### Associativity

Left-to-right, matching `*` and Python's `@`:

```ori
a @ b @ c  // Parsed as: (a @ b) @ c
```

### Disambiguation

The `@` token appears in three syntactic contexts. The parser already knows which context it is in:

| Context | Example | Parser State |
|---------|---------|-------------|
| **Function declaration** | `@forward (self) -> T` | `parse_item()` — `@` starts a function |
| **Binary operator** | `q @ k.T` | `parse_expr()` — `@` between two expressions |
| **Match pattern binding** | `x @ Some(v)` | `parse_match_pattern()` — separate grammar |

No ambiguity exists because:

1. **Item context**: The parser enters `parse_item()` at the top level or inside `impl`/`trait` blocks. Here, `@` is always followed by an identifier (the function name). This path is unchanged.

2. **Expression context**: The parser enters `parse_expr()` when evaluating the right-hand side of `=`, inside function bodies, `if` conditions, etc. Here, `@` appears *after* a complete sub-expression (identifier, method call, closing paren). An `@` token following an expression is unambiguously a binary operator.

3. **Pattern context**: `parse_match_pattern()` is a completely separate grammar. The `x @ pat` syntax is parsed only inside `match` arms. No change needed.

**This is the same disambiguation Python uses** for `@decorator` vs `a @ b`, and the same approach every language uses for `-` (unary negation vs binary subtraction).

---

## Grammar Changes

Additive change to `grammar.ebnf`:

```ebnf
(* Updated multiplicative_expr to include @ *)
multiplicative_expr = unary_expr { ( "*" | "/" | "%" | "div" | "@" ) unary_expr } .
```

---

## Implementation

### Changes by Crate

| Crate | File | Change |
|-------|------|--------|
| `ori_ir` | `ast/operators.rs` | Add `MatMul` to `BinaryOp` + arms in `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()` |
| `ori_lexer` | — | `TokenKind::At` already exists — no change |
| `ori_parse` | `grammar/expr/` (binary expr parser) | Add `TokenKind::At` to multiplicative precedence level |
| `ori_types` | — | Falls through via `BinaryOp::trait_name()` — no special-casing |
| `ori_eval` | `operators.rs` | Add `BinaryOp::MatMul` error arms to ~17 primitive type handlers (no primitive implements `MatMul`) |
| `ori_llvm` | — | Falls through via trait dispatch — no special-casing |
| `library/std` | `prelude.ori` | Add `MatMul` trait definition |
| `docs/spec` | `operator-rules.md`, `grammar.ebnf` | Add `@` to multiplicative group |

The type checker and LLVM backend require no special-casing — they dispatch through `BinaryOp::trait_name()` → trait method lookup. The evaluator needs mechanical error arms in primitive type handlers since no primitive implements `MatMul`.

> **Note:** Compound assignment (`@=`) depends on a future compound-assignment proposal that will cover all `op=` forms (`+=`, `-=`, `*=`, `/=`, `%=`, `@=`, etc.).

---

## Examples

### Self-Attention (Transformers)

```ori
@forward (self, x: Tensor) -> Tensor = {
    let ($b, $t, $c) = x.shape_3d();
    let $qkv = self.qkv.forward(input: x)
        .reshape(shape: [b, t, 3, self.num_heads, self.head_dim])
        .permute(dims: [2, 0, 3, 1, 4]);
    let $q = qkv.select(dim: 0, index: 0);
    let $k = qkv.select(dim: 0, index: 1);
    let $v = qkv.select(dim: 0, index: 2);

    let $attn = softmax(input: q @ k.T * self.scale, dim: -1);

    self.proj.forward(input: (attn @ v).transpose(dim0: 1, dim1: 2).reshape(shape: [b, t, c]))
}
```

### Linear Algebra

```ori
// Solve Ax = b
let $x = A.inverse() @ b;

// Gram matrix
let $gram = X @ X.T;

// Projection matrix
let $P = X @ (X.T @ X).inverse() @ X.T;
```

### Polynomial Regression

```ori
@fit_polynomial (x: Tensor, y: Tensor, degree: int) -> Tensor = {
    let $features = make_features(x: x, degree: degree);
    // Normal equation: w = (X^T X)^-1 X^T y
    (features.T @ features).inverse() @ features.T @ y
}
```

---

## Design Decisions

### Why reuse `@` instead of a new token?

Python established `@` as matmul. ML researchers already have muscle memory for it. Using any other symbol means every line of mathematical code requires mental translation. The entire point is zero-friction copy-paste from Python.

### Why multiplicative precedence?

Matrix multiplication is mathematically a multiplication. `A @ B + C` should parse as `(A @ B) + C`, and `A @ B * c` should parse as `(A @ B) * c` (left-to-right at same level). This matches Python's PEP 465 precedence.

### Why not just use `*` for matmul?

Element-wise multiplication (`*`) and matrix multiplication (`@`) are different operations on the same types. A `Tensor` needs both:

```ori
let $elementwise = a * b;   // Mul trait — element-by-element
let $matmul = a @ b;        // MatMul trait — matrix product
```

This is the same reason Python added `@` — `*` was already taken for element-wise operations in NumPy.

---

## Errata (added 2026-02-21)

> **Precedence renumbered by power-operator-proposal**: The `**` (power) operator was inserted at precedence level 2, shifting all subsequent levels by +1. Multiplicative (including `@`) moved from level 3 to level 4. The relative ordering is unchanged — `@` still has the same precedence as `*`, `/`, `%`, `div`.

## Verification

1. Existing `@` usage (function declarations, pattern bindings) unchanged
2. `q @ k.T` parses and type-checks when `MatMul` is implemented
3. Precedence: `a @ b + c` parses as `(a @ b) + c`
4. Precedence: `a @ b * c` parses as `(a @ b) * c` (left-assoc at same level)
5. Error message: `int @ int` reports "type `int` does not implement `MatMul`"
6. Grammar and spec synced
