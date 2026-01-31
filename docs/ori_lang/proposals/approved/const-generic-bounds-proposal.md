# Proposal: Const Generic Bounds

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, type system, generics

---

## Summary

This proposal formalizes const generic bounds (e.g., `where N > 0`), including allowed constraints, evaluation semantics, and error handling.

---

## Problem Statement

The spec shows `where N > 0` syntax but leaves unclear:

1. **Allowed constraints**: What comparison operators are valid?
2. **Expressions**: Can constraints include arithmetic?
3. **Evaluation**: When are constraints checked?
4. **Errors**: How are violations reported?
5. **Multiple bounds**: Can multiple bounds combine?

---

## Syntax

```ori
@function<T, $N: int> (...) -> R
    where N > 0
= ...

type Container<$N: int>
    where N > 0 && N <= 1000
= { ... }
```

---

## Grammar

Const bounds use a dedicated expression grammar within `where` clauses:

```ebnf
where_clause     = "where" constraint { "," constraint } .
constraint       = type_constraint | const_constraint .
type_constraint  = identifier ":" bounds .
const_constraint = const_bound_expr .

const_bound_expr = const_or_expr .
const_or_expr    = const_and_expr { "||" const_and_expr } .
const_and_expr   = const_not_expr { "&&" const_not_expr } .
const_not_expr   = "!" const_not_expr | const_cmp_expr .
const_cmp_expr   = const_expr comparison_op const_expr
                 | "(" const_bound_expr ")" .

comparison_op    = ">" | "<" | ">=" | "<=" | "==" | "!=" .
```

---

## Allowed Constraints

### Comparison Operators

Const bounds support standard comparison operators:

| Operator | Meaning |
|----------|---------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

```ori
where N > 0
where N != 0
where N >= 1 && N <= 100
```

### Logical Operators

Bounds can be combined:

| Operator | Meaning |
|----------|---------|
| `&&` | Both must hold |
| `\|\|` | Either must hold |
| `!` | Negation |

```ori
where N > 0 && N < 100
where N == 0 || N == 1
where !(N < 0)
```

### Multiple Where Clauses

Multiple `where` clauses are implicitly combined with `&&`. The following are equivalent:

```ori
where R > 0 && C > 0
// is equivalent to:
where R > 0
where C > 0
```

### Arithmetic in Bounds

Limited arithmetic is allowed:

```ori
where N + 1 > 0        // OK
where N * 2 <= 100     // OK
where N % 2 == 0       // OK (even numbers)
where N / 4 > 0        // OK
```

### Bitwise Operators in Bounds

Bitwise operators are allowed for low-level constraints:

```ori
where N & (N - 1) == 0  // Power of 2 check
where N | 1 > 0         // Ensure at least bit 0
where N << 1 <= 1024    // Shifted bound
```

### Bounds Between Parameters

Const parameters can be compared:

```ori
@matrix_multiply<$M: int, $N: int, $P: int> (
    a: [[float, max N], max M],
    b: [[float, max P], max N],
) -> [[float, max P], max M]
    where M > 0 && N > 0 && P > 0
= ...
```

---

## Evaluation Time

### Compile-Time Check

Bounds are checked at compile time when concrete values are known:

```ori
@non_empty_array<$N: int> () -> [int, max N]
    where N > 0
= [0]

let a = non_empty_array<5>()   // OK: 5 > 0
let b = non_empty_array<0>()   // ERROR: 0 > 0 is false
let c = non_empty_array<-1>()  // ERROR: -1 > 0 is false
```

### Deferred to Monomorphization

When const values aren't known until instantiation:

```ori
@replicate<$N: int, $M: int> () -> [[int, max M], max N]
    where N > 0 && M > 0
= ...

@wrapper<$K: int> () -> [[int, max K], max K]
    where K > 0
= replicate<K, K>()  // Bound check deferred
```

### Overflow Handling

Arithmetic overflow during const bound evaluation is a compile-time error:

```ori
@huge<$N: int> ()
    where N * 1000000000000 > 0  // error if N causes overflow
= ...
```

```
error[E1033]: const bound evaluation overflow
  --> src/main.ori:2:11
   |
 2 |     where N * 1000000000000 > 0
   |           ^^^^^^^^^^^^^^^^^^^ multiplication overflows
   |
   = note: const bound arithmetic uses 64-bit signed integers
```

---

## Constraint Propagation

### Satisfying Inner Bounds

When calling a function with bounds, the caller must satisfy them:

```ori
@inner<$N: int> () -> [int, max N]
    where N >= 10
= ...

@outer<$M: int> () -> [int, max M]
    where M >= 10  // Must be at least as strong
= inner<M>()       // OK: M >= 10 implies M >= 10

@bad_outer<$M: int> () -> [int, max M]
    where M > 0    // Not strong enough
= inner<M>()       // ERROR: M > 0 doesn't imply M >= 10
```

### Constraint Implication

The compiler performs **linear arithmetic implication checking** to verify caller bounds imply callee bounds. This covers:

- **Transitivity**: `M >= 20` implies `M >= 10`
- **Equivalence**: `M >= 10` implies `M > 9`
- **Arithmetic**: `M * 2 >= 20` implies `M >= 10`

For complex bounds beyond linear arithmetic, the compiler may require an explicit bound that syntactically matches the callee's requirement.

```ori
// M >= 20 implies M >= 10, so this is valid:
@strong_outer<$M: int> () -> [int, max M]
    where M >= 20
= inner<M>()  // OK
```

---

## Bool Const Generics

### Bool Parameters

`bool` is also allowed as a const generic type:

```ori
@conditional<$B: bool> () -> str =
    if B then "enabled" else "disabled"

conditional<true>()   // "enabled"
conditional<false>()  // "disabled"
```

### Bool in Bounds

Bool parameters can be used in bounds:

```ori
@either_or<$A: bool, $B: bool> () -> int
    where A || B  // At least one must be true
= if A then 1 else 2
```

---

## Complex Bounds

### Multiple Constraints

```ori
@matrix<$R: int, $C: int> (data: [float])
    where R > 0
    where C > 0
    where R * C <= 10000  // Size limit
= ...
```

### Divisibility

```ori
@split_even<$N: int> (items: [T, max N]) -> ([T, max N/2], [T, max N/2])
    where N % 2 == 0  // Must be even
= ...
```

### Power of Two

```ori
@power_of_two<$N: int> ()
    where N > 0 && (N & (N - 1)) == 0  // Bit trick for power of 2
= ...
```

---

## Error Messages

### Bound Violation

```
error[E1030]: const generic bound not satisfied
  --> src/main.ori:10:15
   |
 5 | @buffer<$N: int> () -> [int, max N]
   |                        where N > 0
   |                              ----- required by this bound
...
10 |     let b = buffer<0>()
   |                    ^ `0 > 0` is false
   |
   = note: const generic `N` must satisfy `N > 0`
```

### Insufficient Caller Bound

```
error[E1031]: caller bound does not imply callee bound
  --> src/main.ori:8:5
   |
 3 | @inner<$N: int> () where N >= 10 = ...
   |                          ------- callee requires `N >= 10`
...
 6 | @outer<$M: int> () where M > 0 =
   |                          ----- caller only guarantees `M > 0`
 8 |     inner<M>()
   |     ^^^^^^^^ cannot prove `M >= 10`
   |
   = help: strengthen the bound: `where M >= 10`
```

### Invalid Bound Expression

```
error[E1032]: invalid const bound expression
  --> src/main.ori:2:10
   |
 2 |     where N.to_str() == "5"
   |           ^^^^^^^^^^ method calls not allowed in const bounds
   |
   = note: const bounds may only use: comparisons, arithmetic, logical operators
```

### Overflow During Evaluation

```
error[E1033]: const bound evaluation overflow
  --> src/main.ori:2:11
   |
 2 |     where N * 1000000000000 > 0
   |           ^^^^^^^^^^^^^^^^^^^ multiplication overflows
   |
   = note: const bound arithmetic uses 64-bit signed integers
```

---

## Spec Changes Required

### Update `06-types.md`

Expand Const Generic Parameters section with:
1. Bound syntax
2. Allowed operators
3. Evaluation timing
4. Constraint propagation rules
5. Overflow handling

### Update `grammar.ebnf`

Add const bound expression grammar as specified above.

---

## Summary

| Aspect | Details |
|--------|---------|
| Syntax | `where N > 0` in generic declarations |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Logical | `&&`, `\|\|`, `!` |
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Bitwise | `&`, `\|`, `^`, `<<`, `>>` |
| Evaluation | Compile-time when values known |
| Overflow | Compile-time error (E1033) |
| Propagation | Caller bounds must imply callee bounds (linear arithmetic) |
| Bool generics | `$B: bool` supported with bounds |
| Multiple bounds | Combine with `&&` or separate `where` clauses |
