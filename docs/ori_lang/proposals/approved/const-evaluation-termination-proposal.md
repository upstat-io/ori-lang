# Proposal: Const Evaluation Termination

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Compiler, const evaluation

---

## Summary

This proposal specifies the termination guarantees and limits for compile-time constant evaluation, addressing what happens when const functions don't terminate, how long compilation can spend on const evaluation, and the semantics of mixed const/runtime arguments.

---

## Problem Statement

The spec defines const functions (`$name (params) -> Type = expr`) but leaves critical questions:

1. **Non-termination**: What if `$factorial(n: 100)` recurses too deeply or `$fib(n: 50)` takes too long?
2. **Resource limits**: How much memory/time can const evaluation consume?
3. **Mixed arguments**: If a const function receives one const and one runtime argument, what happens?
4. **Serialization**: How are evaluated const values stored for library distribution?

---

## Const Evaluation Model

### Definition

**Const evaluation** is the process of computing values at compile time. A const expression is evaluated during compilation and replaced with its result in the compiled output.

### When Const Evaluation Occurs

| Context | Evaluated? | Example |
|---------|-----------|---------|
| Module-level `$` binding | Yes | `let $PI = 3.14159` |
| Const function with const args | Yes | `$square(x: 5)` → `25` |
| Const function with runtime args | No | `$square(x: n)` → deferred to runtime |
| Type-level expressions | Yes | `[int; $SIZE]` |
| Attribute arguments | Yes | `#timeout($DEFAULT_TIMEOUT)` |

---

## Termination Guarantees

### Evaluation Limits

The compiler enforces these limits on const evaluation:

| Limit | Default | Rationale |
|-------|---------|-----------|
| **Step limit** | 1,000,000 operations | Prevents infinite loops |
| **Recursion depth** | 1,000 frames | Prevents stack overflow |
| **Memory limit** | 100 MB | Prevents memory exhaustion |
| **Time limit** | 10 seconds | Prevents compilation hang |

An "operation" is approximately one expression evaluation (function call, arithmetic, etc.).

### Limit Exceeded Behavior

When any limit is exceeded:

1. Compilation fails with an error
2. Error indicates which limit was exceeded
3. Error shows the const expression that failed
4. Suggestions provided for resolution

```
error[E0500]: const evaluation exceeded step limit
  --> src/math.ori:5:1
   |
5  | let $fib_50 = $fib(n: 50)
   | ^^^^^^^^^^^^^^^^^^^^^^^^^ exceeded 1,000,000 evaluation steps
   |
   = note: const function $fib at src/math.ori:10:1
   = help: consider caching intermediate results or reducing input
   = help: if intentional, use `#const_limit(steps: 10000000)` attribute
```

### Configurable Limits

Projects can adjust limits via configuration:

```ori
// In ori.toml or equivalent
[const_eval]
max_steps = 10000000
max_depth = 2000
max_memory = "500mb"
max_time = "60s"
```

Or per-expression via attribute:

```ori
#const_limit(steps: 5000000)
let $large_table = $generate_lookup_table()
```

---

## Mixed Const/Runtime Arguments

### Rules

A const function called with **any runtime argument** is evaluated at runtime:

```ori
$multiply (a: int, b: int) -> int = a * b

// Both args const → evaluated at compile time
let $twelve = $multiply(a: 3, b: 4)  // Compiles to: let $twelve = 12

// One arg runtime → evaluated at runtime
@compute (x: int) -> int = $multiply(a: x, b: 4)  // Compiles to: x * 4
```

### Partial Evaluation

When a const function is called with some constant and some runtime arguments, the compiler **must** evaluate the constant portions at compile time where doing so would produce equivalent results:

```ori
$power (base: int, exp: int) -> int = ...

// base is runtime, exp is const
@square (x: int) -> int = $power(base: x, exp: 2)
// Must optimize to: x * x (inlining the const exponent)
```

This is required behavior, not an optional optimization.

### Type-Level Const

Arguments in type positions must always be const:

```ori
type Buffer<N: int> = { data: [byte; N] }

// OK: const argument
type SmallBuffer = Buffer<256>

// ERROR: runtime value in type position
@make_buffer (size: int) -> Buffer<size> = ...  // Error: size is not const
```

---

## Const Function Restrictions

### Allowed Operations

Const functions may use:

| Operation | Allowed? |
|-----------|----------|
| Arithmetic | Yes |
| Comparisons | Yes |
| Boolean logic | Yes |
| Pattern matching | Yes |
| Struct construction | Yes |
| Function calls (to other const functions) | Yes |
| Pure recursion | Yes |
| String operations | Yes |
| Local mutable bindings | Yes |
| Loop expressions (`for`, `loop`) | Yes |

### Local Mutable Bindings

Const functions may use mutable bindings for local computation. Local mutation is deterministic — given the same inputs, the function produces the same output:

```ori
// OK: local mutation that doesn't escape
$sum_to (n: int) -> int = {
    let total = 0
    for i in 1..=n do total = total + i
    total
}

$sum_squares (n: int) -> int = {
    let result = 0
    for i in 1..=n do result = result + i * i
    result
}
```

### Prohibited Operations

Const functions cannot use:

| Operation | Reason |
|-----------|--------|
| Capabilities (`uses ...`) | Side effects |
| I/O of any kind | Side effects |
| Random values | Non-determinism |
| Current time | Non-determinism |
| External data access | Reproducibility |

```ori
// ERROR: const function cannot use capabilities
$bad (url: str) -> str uses Http = fetch(url)
```

---

## Const Evaluation Caching

### Compilation Cache

Evaluated const expressions are cached by the compiler:

1. Cache key = hash of (function body + argument values)
2. Subsequent compilations reuse cached results
3. Cache invalidated when function source changes

### Cross-Module Caching

When a library exports const values:

```ori
// In library math.ori
pub let $E = 2.718281828459045
pub let $PRECOMPUTED_TABLE = $generate_table()
```

The compiled library artifact contains the evaluated values, not the computation.

### Serialization Format

The serialization format for const values in compiled artifacts is implementation-defined. Compilers may use any format that preserves value semantics.

### Determinism Requirement

Const evaluation must be deterministic — same inputs always produce same outputs. This is enforced by the prohibited operations list.

---

## Error Handling in Const Evaluation

### Panics

A panic during const evaluation is a compilation error:

```ori
$divide (a: int, b: int) -> int = a / b

let $oops = $divide(a: 1, b: 0)  // Compilation error: division by zero
```

### Integer Overflow

Integer overflow during const evaluation follows the same rules as runtime:

```ori
let $big = $multiply(a: 1000000000000, b: 1000000000000)
// Compilation error: integer overflow in const evaluation
```

### Option/Result

Const functions may return `Option` or `Result`:

```ori
$safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b)

let $result = $safe_div(a: 10, b: 0)  // $result = None (at compile time)
```

---

## Examples

### Valid Const Functions

```ori
// Simple arithmetic
$double (x: int) -> int = x * 2
let $twenty = $double(x: 10)

// Recursive with reasonable depth
$factorial (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1)
let $fact_10 = $factorial(n: 10)  // 3628800

// String manipulation
$greet (name: str) -> str = `Hello, {name}!`
let $greeting = $greet(name: "World")

// Lookup table generation
$squares (max: int) -> [int] =
    (0..max).map(n -> n * n).collect()
let $SQUARE_TABLE = $squares(max: 100)

// Iterative with local mutation
$sum_range (n: int) -> int = {
    let total = 0
    for i in 1..=n do total = total + i
    total
}
let $sum_100 = $sum_range(n: 100)  // 5050
```

### Invalid Const Functions

```ori
// Too deep recursion (default limit: 1000)
$ackermann (m: int, n: int) -> int = ...
let $big = $ackermann(m: 4, n: 2)  // Error: exceeds recursion depth

// Non-terminating
$infinite (x: int) -> int = $infinite(x: x)
let $oops = $infinite(x: 1)  // Error: exceeds step limit

// Uses capability
$fetch_config (url: str) -> Config uses Http = ...  // Error: uses capability
```

---

## Spec Changes Required

### Update `21-constant-expressions.md`

Add:
1. Evaluation limits table
2. Limit exceeded error format
3. Mixed argument semantics
4. Partial evaluation requirement
5. Const function restrictions (updated for loops/mutation)
6. Caching behavior
7. Serialization as implementation-defined
8. Error handling in const context

### Update `04-constants.md`

Cross-reference to const evaluation limits.

### Add Diagnostics

Define error codes:
- `E0500`: Step limit exceeded
- `E0501`: Recursion depth exceeded
- `E0502`: Memory limit exceeded
- `E0503`: Time limit exceeded
- `E0504`: Non-const operation in const function

---

## Design Rationale

### Why Limits Instead of Termination Proofs?

Proving termination for arbitrary recursive functions is undecidable (Halting Problem). Practical limits:
- Simple to implement and understand
- Sufficient for real use cases
- Configurable for edge cases

### Why These Default Limits?

- **1M steps**: Enough for large lookup tables, not enough to hang compilation
- **1000 depth**: Matches typical stack limits, catches runaway recursion
- **100MB memory**: Prevents OOM while allowing substantial tables
- **10 seconds**: User-noticeable delay triggers investigation

### Why Allow Local Mutation?

Local mutation within a const function is deterministic — given the same inputs, the function produces the same output. Prohibiting all mutation forces users into recursion-only patterns which can be less readable and hit recursion limits sooner. The real concern is non-determinism from external state, not local temporaries.

### Why Allow Loops?

The step limit already prevents infinite loops. Prohibiting loop syntax provides no additional safety and artificially restricts expressiveness. Many algorithms are naturally iterative.

### Why Require Partial Evaluation?

Predictable performance guarantees allow users to rely on patterns like `$power(base: x, exp: 2)` optimizing to `x * x`. Making this optional would create implementation-dependent performance characteristics.

### Why Cache Evaluated Results?

- Faster incremental compilation
- Library artifacts contain results, not computations
- Users don't pay evaluation cost repeatedly

---

## Summary

| Aspect | Specification |
|--------|--------------|
| Step limit | 1,000,000 (configurable) |
| Recursion depth | 1,000 (configurable) |
| Memory limit | 100 MB (configurable) |
| Time limit | 10 seconds (configurable) |
| Limit exceeded | Compilation error |
| Mixed arguments | Runtime evaluation |
| Partial evaluation | Required |
| Type-level const | Must be fully const |
| Local mutation | Allowed |
| Loops | Allowed |
| Prohibited | Capabilities, I/O, non-determinism |
| Panics in const | Compilation error |
| Caching | By function + args hash |
| Serialization | Implementation-defined |
| Determinism | Required |
