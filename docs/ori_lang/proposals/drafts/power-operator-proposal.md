# Proposal: `**` Power / Exponentiation Operator

**Status:** Draft
**Author:** Eric
**Created:** 2026-02-21
**Affects:** Lexer, parser, IR, type checker, evaluator, LLVM codegen, standard library prelude
**Depends on:** Operator Traits (approved)

---

## Summary

Add `**` as a binary operator for exponentiation. The operator desugars to the `Pow` trait method `power()`, following the same pattern as all existing operator traits. `**` is right-associative and binds tighter than multiplicative operators, matching mathematical convention and Python's precedence.

---

## Motivation

### Copy-Paste from Python

Exponentiation is pervasive in ML, scientific computing, and general math:

```python
# Python — appears constantly
scale = head_dim ** -0.5
loss = ((pred - target) ** 2).mean()
norm = (x ** 2).sum().sqrt()
decay = base_lr * gamma ** epoch
```

Without `**`:

```ori
// Ori today — method calls
let $scale = (head_dim as float).pow(n: -0.5);
let $loss = (pred - target).pow(n: 2).mean();
let $norm = x.pow(n: 2).sum().sqrt();
let $decay = base_lr * gamma.pow(n: epoch as float);
```

With `**`:

```ori
// Ori with this proposal
let $scale = head_dim ** -0.5;
let $loss = ((pred - target) ** 2).mean();
let $norm = (x ** 2).sum().sqrt();
let $decay = base_lr * gamma ** epoch;
```

Every line copies from Python character-for-character. Combined with `@` for matmul, the entire mathematical vocabulary of ML translates without friction.

### Mathematical Convention

`**` for exponentiation is established in:
- Python (since 1991)
- Ruby
- Perl
- JavaScript (`**` since ES2016)
- Fortran (`**` since 1957 — the original)
- R (`^`, but `**` also works)

It is arguably the most widely recognized operator symbol after `+`, `-`, `*`, `/`.

---

## Design

### Trait Definition

Added to the standard library prelude:

```ori
trait Pow<Rhs = Self> {
    type Output = Self
    @power (self, rhs: Rhs) -> Self.Output
}
```

Built-in implementations for primitives:

```ori
impl Pow for int {
    type Output = int
    @power (self, rhs: int) -> int  // integer exponentiation
}

impl Pow for float {
    type Output = float
    @power (self, rhs: float) -> float  // delegates to libm pow()
}

impl Pow<int> for float {
    type Output = float
    @power (self, rhs: int) -> float  // float ** int (common case)
}

impl Pow<float> for int {
    type Output = float
    @power (self, rhs: float) -> float  // int ** float (e.g., head_dim ** -0.5)
}
```

### Operator Desugaring

`a ** b` desugars to `Pow.power(a, rhs: b)`.

### Precedence and Associativity

`**` has **higher precedence** than multiplicative operators and is **right-associative**. This matches mathematical convention and Python:

```ori
2 ** 3 ** 2    // = 2 ** 9 = 512 (right-associative)
-2 ** 2        // = -(2 ** 2) = -4 (** binds tighter than unary -)
a * b ** 2     // = a * (b ** 2)
a @ b ** 0.5   // = a @ (b ** 0.5)
```

Updated precedence table:

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | `.` `[]` `()` `?` | left |
| 2 | `!` `-` `~` | right (unary) |
| **2.5** | **`**`** | **right** |
| 3 | `*` `/` `%` `div` `@` | left |
| 4 | `+` `-` | left |
| ... | ... | ... |

**Note on unary minus:** `-x ** 2` parses as `-(x ** 2)`, not `(-x) ** 2`. This matches Python and mathematical convention. If the programmer wants `(-x) ** 2`, they write it with parentheses.

### Integer Exponentiation

`int ** int` returns `int`. Negative exponents on integers panic:

```ori
2 ** 10     // = 1024
2 ** -1     // panic: negative exponent on integer
3 ** 0      // = 1
0 ** 0      // = 1 (mathematical convention)
```

For negative exponents, use float: `2.0 ** -1` or `2 ** -1.0`.

Overflow follows Ori's standard overflow behavior (panic in debug, wrapping behavior defined by overflow-behavior proposal).

---

## Grammar Changes

```ebnf
(* New precedence level between unary and multiplicative *)
power_expr          = unary_expr [ "**" power_expr ] .    (* right-associative *)
multiplicative_expr = power_expr { ( "*" | "/" | "%" | "div" | "@" ) power_expr } .
```

---

## Implementation

| Crate | File | Change |
|-------|------|--------|
| `ori_ir` | `ast/operators.rs` | Add `Pow` to `BinaryOp` + arms in `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()` |
| `ori_lexer` | token/cooker | Recognize `**` as a two-character token (currently `*` is `Mul`) |
| `ori_parse` | `grammar/expr/` | Add `parse_power_expr()` between unary and multiplicative, right-associative |
| `ori_types` | — | Falls through via `BinaryOp::trait_name()` — no special-casing |
| `ori_eval` | — | Falls through for user types; built-in `int`/`float` dispatch for primitives |
| `ori_llvm` | — | Falls through via trait dispatch; primitive implementations via `llvm.pow` intrinsic |
| `library/std` | `prelude.ori` | Add `Pow` trait definition + primitive impls |
| `docs/spec` | `operator-rules.md`, `grammar.ebnf` | Add `**` to precedence table |

### Lexer Consideration

`**` is two `*` characters. The lexer must recognize `**` as a distinct token, not two `Mul` tokens. This is the same pattern as `==` (not two `=`), `<=` (not `<` then `=`), etc. — the lexer already handles multi-character operators by longest-match.

### Compound Assignment

`**=` follows the pattern of `+=`, `-=`, etc.:

```ori
let scale = 2.0;
scale **= 10;  // scale = scale ** 10
```

---

## Examples

### ML: Loss Functions

```ori
@mse_loss (pred: Tensor, target: Tensor) -> Tensor =
    ((pred - target) ** 2).mean()

@huber_loss (pred: Tensor, target: Tensor, delta: float = 1.0) -> Tensor = {
    let $diff = (pred - target).abs();
    if diff <= delta then
        0.5 * diff ** 2
    else
        delta * (diff - 0.5 * delta)
}
```

### ML: Learning Rate Schedules

```ori
@cosine_decay (initial_lr: float, step: int, total_steps: int) -> float =
    initial_lr * 0.5 * (1.0 + cos(x: pi * step as float / total_steps as float))

@exponential_decay (initial_lr: float, step: int, decay_rate: float, decay_steps: int) -> float =
    initial_lr * decay_rate ** (step / decay_steps)
```

### General Math

```ori
// Compound interest
@compound (principal: float, rate: float, years: int) -> float =
    principal * (1.0 + rate) ** years

// Distance
@distance (a: Point, b: Point) -> float =
    ((b.x - a.x) ** 2 + (b.y - a.y) ** 2) ** 0.5

// Polynomial evaluation
@horner (coeffs: [float], x: float) -> float =
    coeffs.iter().enumerate().fold(
        initial: 0.0,
        op: (acc, (i, c)) -> acc + c * x ** i,
    )
```

---

## Design Decisions

### Why `**` and not `^`?

`^` is already used for `BitXor` in Ori (and in C, Rust, Go, Java, JavaScript). Overloading it for exponentiation would be ambiguous and would differ from every C-family language. `**` is unambiguous and has 70 years of precedent (Fortran, 1957).

### Why right-associative?

Mathematical convention: `2^3^2 = 2^(3^2) = 2^9 = 512`. Every language with `**` makes it right-associative (Python, Ruby, JavaScript, Fortran). Left-associativity would give `(2^3)^2 = 8^2 = 64`, which is mathematically unexpected.

### Why higher precedence than `*`?

Mathematical convention: `a * b^2` means `a * (b^2)`, not `(a * b)^2`. Every language agrees on this. `**` binding tighter than `*`/`@`/`div` is universal.

### Why does `-x ** 2` equal `-(x ** 2)`?

Python, JavaScript, and Ruby all parse `-x ** 2` as `-(x ** 2)`. The mathematical notation `-x²` means `-(x²)`. Making `**` bind tighter than unary minus is consistent with both convention and prior art. This is the one case where a programmer might be surprised, but it matches what every other language does and what mathematicians expect.

### Why allow mixed int/float operands?

`head_dim ** -0.5` is idiomatic ML code. Requiring explicit conversion (`(head_dim as float) ** -0.5` or `head_dim as float |> _ ** -0.5`) adds friction for the most common use case. The mixed-type `impl Pow<float> for int` returns `float`, which is always the correct behavior.

---

## Verification

1. `2 ** 10` evaluates to `1024`
2. `2.0 ** -1.0` evaluates to `0.5`
3. `2 ** 3 ** 2` evaluates to `512` (right-associative)
4. `-2 ** 2` evaluates to `-4` (`**` tighter than unary `-`)
5. `a * b ** 2` parses as `a * (b ** 2)`
6. `2 ** -1` panics (negative exponent on int)
7. `head_dim ** -0.5` works with mixed int/float (returns float)
8. Compound assignment: `x **= 2` desugars correctly
9. User-defined types can implement `Pow` trait
10. Error message: `str ** int` reports "type `str` does not implement `Pow`"
