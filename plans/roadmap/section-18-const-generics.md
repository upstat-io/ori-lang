---
section: 18
title: Const Generics
status: in-progress
tier: 7
goal: Enable type parameters that are compile-time constant values
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
sections:
  - id: "18.0"
    title: Const Evaluation Termination
    status: not-started
  - id: "18.1"
    title: Const Type Parameters
    status: in-progress
  - id: "18.2"
    title: Fixed-Capacity Lists
    status: not-started
  - id: "18.3"
    title: Fixed-Size Arrays (Future)
    status: not-started
  - id: "18.4"
    title: Const Expressions in Types
    status: not-started
  - id: "18.5"
    title: Const Bounds
    status: in-progress
  - id: "18.6"
    title: Default Const Values
    status: not-started
  - id: "18.7"
    title: Const in Trait Bounds
    status: not-started
  - id: "18.8"
    title: "Expanded Const Generic Eligibility (Capability Unification)"
    status: not-started
  - id: "18.9"
    title: "Associated Consts in Traits (Capability Unification)"
    status: not-started
  - id: "18.10"
    title: "Const Functions in Type Positions (Capability Unification)"
    status: not-started
---

# Section 18: Const Generics

**Goal**: Enable type parameters that are compile-time constant values

**Criticality**: Medium — Type-level programming, fixed-size arrays

**Dependencies**: Sections 1-2 (Type System Foundation)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Value types | `int`, `bool` initially | Start simple, expand later |
| Const-generic syntax | `$N: int` | Consistent with Ori's `$` sigil for immutable bindings |
| Fixed-capacity list syntax | `[T, max N]` | Reads naturally, distinct from fixed-size arrays |
| Fixed-size array syntax | `[T, size N]` (future) | Clear distinction from max capacity |
| Const expressions | Limited initially | Avoid complexity |
| Default values | Supported | API ergonomics |

---

## Reference Implementation

### Rust

```
~/projects/reference_repos/lang_repos/rust/compiler/rustc_middle/src/ty/consts.rs    # Const type representation
~/projects/reference_repos/lang_repos/rust/compiler/rustc_hir/src/def.rs             # ConstParam definition
~/projects/reference_repos/lang_repos/rust/compiler/rustc_hir_typeck/src/lib.rs      # Const generic checking
```

---

## 18.0 Const Evaluation Termination

**Proposal**: `proposals/approved/const-evaluation-termination-proposal.md`

Specifies termination guarantees and limits for compile-time constant evaluation, preventing infinite computation during compilation.

### Implementation

- [ ] **Implement**: Step limit enforcement — stop const evaluation after 1,000,000 operations
  - [ ] **Rust Tests**: `ori_typeck/tests/const_eval_limits.rs`
  - [ ] **Ori Tests**: `tests/spec/const/step_limit.ori`
  - [ ] **LLVM Support**: LLVM codegen for const evaluation step counting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Recursion depth limit — stop const evaluation after 1,000 stack frames
  - [ ] **Rust Tests**: `ori_typeck/tests/const_eval_limits.rs`
  - [ ] **Ori Tests**: `tests/spec/const/recursion_limit.ori`
  - [ ] **LLVM Support**: LLVM codegen for recursion depth tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Memory limit — stop const evaluation after 100 MB allocation
  - [ ] **Rust Tests**: `ori_typeck/tests/const_eval_limits.rs`
  - [ ] **Ori Tests**: `tests/spec/const/memory_limit.ori`
  - [ ] **LLVM Support**: LLVM codegen for memory tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Time limit — stop const evaluation after 10 seconds
  - [ ] **Rust Tests**: `ori_typeck/tests/const_eval_limits.rs`
  - [ ] **Ori Tests**: `tests/spec/const/time_limit.ori`
  - [ ] **LLVM Support**: LLVM codegen for time limit enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Configurable limits via `ori.toml`
  - [ ] **Rust Tests**: `ori_config/tests/const_eval_config.rs`
  - [ ] **Ori Tests**: `tests/spec/const/configurable_limits.ori`

- [ ] **Implement**: Per-expression limit override via `#const_limit(...)` attribute
  - [ ] **Rust Tests**: `ori_parser/tests/const_limit_attr.rs`
  - [ ] **Ori Tests**: `tests/spec/const/const_limit_attribute.ori`
  - [ ] **LLVM Support**: LLVM codegen for attribute parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Partial evaluation for mixed const/runtime arguments (required behavior)
  - [ ] **Rust Tests**: `ori_typeck/tests/partial_eval.rs`
  - [ ] **Ori Tests**: `tests/spec/const/partial_evaluation.ori`
  - [ ] **LLVM Support**: LLVM codegen for partial const evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Allow local mutable bindings in const functions
  - [ ] **Rust Tests**: `ori_typeck/tests/const_local_mutation.rs`
  - [ ] **Ori Tests**: `tests/spec/const/local_mutation.ori`
  - [ ] **LLVM Support**: LLVM codegen for mutable locals in const
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Allow loop expressions (`for`, `loop`) in const functions
  - [ ] **Rust Tests**: `ori_typeck/tests/const_loops.rs`
  - [ ] **Ori Tests**: `tests/spec/const/const_loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loops in const
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_eval_tests.rs`

- [ ] **Implement**: Const evaluation caching (by function + args hash)
  - [ ] **Rust Tests**: `ori_typeck/tests/const_caching.rs`
  - [ ] **Ori Tests**: `tests/spec/const/caching.ori`

- [ ] **Implement**: Error diagnostics (E0500-E0504)
  - [ ] **Rust Tests**: `ori_reporting/tests/const_eval_errors.rs`
  - [ ] **Ori Tests**: `tests/spec/const/error_diagnostics.ori`

---

## 18.1 Const Type Parameters
<!-- unblocks:0.3.2 -->
<!-- unblocks:0.3.3 -->
<!-- unblocks:0.4.4 -->
<!-- unblocks:0.9.1 -->

**Proposal**: `proposals/approved/const-generics-proposal.md`

**Spec section**: `spec/06-types.md § Const Generic Parameters`

### Syntax

```ori
// Const parameter in type (using $ sigil for const)
type Array<T, $N: int> = {
    data: [T, max N],
    // len is known at compile time: N
}

// Usage
let arr: Array<int, 5> = Array.new()
arr[0] = 42

// In functions
@zeros<$N: int> () -> Array<int, N> = ...

let five_zeros: Array<int, 5> = zeros()
```

### Grammar

```ebnf
TypeParameter = Identifier [ ':' TypeBound ]
              | '$' Identifier ':' ConstType ;
ConstType     = 'int' | 'bool' ;
```

### Implementation

- [ ] **Spec**: Const parameter syntax
  - [ ] `const N: int` in type parameters
  - [ ] Allowed const types
  - [ ] Scope rules
  - [ ] **LLVM Support**: LLVM codegen for const parameter syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const parameter syntax codegen

- [ ] **Parser**: Parse const parameters
  - [x] `$` sigil in generics — parses `$N: int` [done] (2026-02-13)
  - [x] Type annotation required — enforced by parser [done] (2026-02-13)
  - [x] Position (can mix with type params) — `<T, $N: int>` works [done] (2026-02-13)
  - [ ] **LLVM Support**: LLVM codegen for parsed const parameters
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const parameter parsing codegen

- [x] **Type checker**: Const parameter validation (body-level)
  - [x] Track const vs type parameters [done] (2026-02-14)
  - [x] Validate const type (int, bool) [done] (2026-02-14)
  - [ ] Unification with const values (call-site deferred)
  - [ ] **LLVM Support**: LLVM codegen for const parameter validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const parameter validation codegen

- [x] **Test**: `tests/spec/types/const_generics.ori` (body-level)
  - [x] Basic const parameter [done] (2026-02-14)
  - [x] Multiple const parameters [done] (2026-02-14)
  - [x] Mixed type and const [done] (2026-02-14)
  - [ ] **LLVM Support**: LLVM codegen for const generic tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const generic tests codegen

---

## 18.2 Fixed-Capacity Lists

**Proposal**: `proposals/approved/fixed-capacity-list-proposal.md`

Inline-allocated lists with compile-time maximum capacity and runtime-dynamic length.

### Syntax

```ori
// Type: list of T with maximum capacity N
[T, max N]

// Examples
let buffer: [int, max 10] = []
buffer.push(1)          // OK
buffer.push(11)         // PANIC after 10 elements

// Generic over capacity
@swap_ends<T, $N: int> (items: [T, max N]) -> [T, max N] = ...
```

### Implementation

- [ ] **Spec**: Fixed-capacity list type — `spec/06-types.md § Fixed-Capacity List`
  - [ ] Type syntax `[T, max N]`
  - [ ] Relationship to dynamic `[T]` (subtype)
  - [ ] Capacity limit semantics
  - [ ] **LLVM Support**: LLVM codegen for fixed-capacity list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Grammar**: Parse fixed-capacity list type — `grammar.ebnf`
  - [ ] `list_type = "[" type "]" | "[" type "," "max" const_expr "]"`
  - [ ] `max` as soft keyword in this context
  - [ ] **LLVM Support**: LLVM codegen for parsed fixed-capacity types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Types**: Fixed-capacity list type representation
  - [ ] `Type::FixedList(elem, capacity)` in type system
  - [ ] Subtype relationship: `[T, max N] <: [T]`
  - [ ] Capacity must be compile-time constant
  - [ ] **LLVM Support**: LLVM codegen for fixed-capacity type representation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Methods**: Fixed-capacity list methods
  - [ ] `.capacity() -> int` — compile-time capacity
  - [ ] `.is_full() -> bool` — length == capacity
  - [ ] `.remaining() -> int` — capacity - length
  - [ ] `.push(item: T) -> void` — panic if full
  - [ ] `.try_push(item: T) -> bool` — return false if full
  - [ ] `.push_or_drop(item: T) -> void` — drop if full
  - [ ] `.push_or_oldest(item: T) -> void` — remove index 0 if full, push to end
  - [ ] `.to_dynamic() -> [T]` — convert to heap-allocated
  - [ ] **LLVM Support**: LLVM codegen for fixed-capacity methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Methods**: Dynamic list conversion methods
  - [ ] `[T].to_fixed<$N: int>() -> [T, max N]` — panic if too large
  - [ ] `[T].try_to_fixed<$N: int>() -> Option<[T, max N]>`
  - [ ] **LLVM Support**: LLVM codegen for conversion methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Traits**: Trait implementations for `[T, max N]`
  - [ ] `Eq` when `T: Eq`
  - [ ] `Hashable` when `T: Hashable`
  - [ ] `Comparable` when `T: Comparable`
  - [ ] `Clone` when `T: Clone`
  - [ ] `Debug` when `T: Debug`
  - [ ] `Printable` when `T: Printable`
  - [ ] `Sendable` when `T: Sendable`
  - [ ] `Iterable` always
  - [ ] `DoubleEndedIterator` always
  - [ ] `Collect` always (panic if exceeds capacity)
  - [ ] **LLVM Support**: LLVM codegen for trait impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Memory**: Inline storage representation
  - [ ] Elements stored inline (no separate heap allocation)
  - [ ] Length stored as part of structure
  - [ ] ARC semantics for reference-type elements
  - [ ] **LLVM Support**: LLVM codegen for inline storage
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

- [ ] **Test**: `tests/spec/types/fixed_capacity_list.ori`
  - [ ] Basic declaration and operations
  - [ ] Capacity checks and panics
  - [ ] Safe alternatives (`try_push`, etc.)
  - [ ] Subtype relationship with `[T]`
  - [ ] Generic functions with `$N: int`
  - [ ] In struct fields
  - [ ] **LLVM Support**: LLVM codegen for tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fixed_capacity_tests.rs`

---

## 18.3 Fixed-Size Arrays (Future)

**Spec section**: `spec/06-types.md § Fixed-Size Arrays`

> **Note:** This is a future extension. Fixed-capacity lists (`[T, max N]`) are prioritized first. Fixed-size arrays always have exactly N elements, unlike fixed-capacity lists which have 0 to N elements.

### Syntax (Proposed)

```ori
// Array type with fixed size (always exactly N elements)
let arr: [int, size 5] = [0, 0, 0, 0, 0]

// Array operations
let len = len(collection: arr)  // Const 5, known at compile time
let elem = arr[2]               // Bounds checked at compile time if index const

// Distinct from fixed-capacity: cannot have fewer than N elements
// [int, size 5] ≠ [int, max 5]
```

### Type Rules

```
[T, size N] where N: int (const)
- Length always exactly N, known at compile time
- Inline allocated (no heap)
- Cannot be empty, cannot grow, cannot shrink
- Bounds checks can be optimized away for const indices
```

### Implementation (Deferred)

- [ ] **Spec**: Fixed-size array type
  - [ ] Syntax `[T, size N]`
  - [ ] Distinct from fixed-capacity `[T, max N]`
  - [ ] Relationship to dynamic `[T]`
  - [ ] Operations
  - [ ] **LLVM Support**: LLVM codegen for fixed-size array type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs`

- [ ] **Types**: Array type with const
  - [ ] `Type::FixedArray(elem, ConstValue)`
  - [ ] Distinct from `Type::List(elem)` and `Type::FixedList(elem, capacity)`
  - [ ] **LLVM Support**: LLVM codegen for array type with const
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs`

- [ ] **Parser**: Parse array types
  - [ ] `[T, size expr]` syntax
  - [ ] Const expression for size
  - [ ] **LLVM Support**: LLVM codegen for parsed array types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs`

- [ ] **Test**: `tests/spec/types/fixed_arrays.ori`
  - [ ] Array declaration
  - [ ] Array literal with inferred size
  - [ ] **LLVM Support**: LLVM codegen for fixed array tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs`

---

## 18.4 Const Expressions in Types

**Spec section**: `spec/06-types.md § Const Expressions`

### Syntax

```ori
// Arithmetic in const position
type Matrix<$ROWS: int, $COLS: int> = {
    data: [float, max ROWS * COLS],
}

// Const function in type
$double (n: int) -> int = n * 2

type DoubleArray<$N: int> = {
    data: [int, max $double(n: N)],
}

// Conditional const
type Buffer<$SIZE: int> = {
    data: [byte, max if SIZE > 0 then SIZE else 1],
}
```

### Allowed Expressions

- Const parameters: `N`, `M`
- Literals: `5`, `true`
- Arithmetic: `N + M`, `N * 2`, `N / 2`
- Comparison: `N > 0`, `N == M`
- Const functions: `$func(n: N)`
- Conditionals: `if N > 0 then N else 1`

### Implementation

- [ ] **Spec**: Const expression rules
  - [ ] Allowed operations
  - [ ] Evaluation timing
  - [ ] Error handling
  - [ ] **LLVM Support**: LLVM codegen for const expression rules
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const expression rules codegen

- [ ] **Const evaluator**: Evaluate const expressions
  - [ ] At type checking time
  - [ ] Cache results
  - [ ] Error on non-const
  - [ ] **LLVM Support**: LLVM codegen for const expression evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const expression evaluation codegen

- [ ] **Type checker**: Validate const expressions
  - [ ] All operands must be const
  - [ ] Result must be correct type
  - [ ] **LLVM Support**: LLVM codegen for const expression validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const expression validation codegen

- [ ] **Test**: `tests/spec/types/const_expressions.ori`
  - [ ] Arithmetic in types
  - [ ] Const functions in types
  - [ ] Conditional const
  - [ ] **LLVM Support**: LLVM codegen for const expression tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const expression tests codegen

---

## 18.5 Const Bounds

**Proposal**: `proposals/approved/const-generic-bounds-proposal.md`

**Spec section**: `spec/06-types.md § Const Bounds`

Formalizes const generic bounds (e.g., `where N > 0`), including allowed constraints, evaluation semantics, constraint propagation, and error handling.

### Syntax

```ori
// Bound on const parameter value
@non_empty_array<$N: int> () -> [int, max N]
    where N > 0  // Const bound
= ...

// Multiple bounds (combined with && or separate where clauses)
@matrix_multiply<$M: int, $N: int, $P: int> (
    a: Matrix<M, N>,
    b: Matrix<N, P>,
) -> Matrix<M, P>
    where M > 0 && N > 0 && P > 0
= ...

// Arithmetic and bitwise in bounds
@power_of_two<$N: int> ()
    where N > 0 && (N & (N - 1)) == 0  // Bit trick for power of 2
= ...

// Bool const generics with bounds
@either_or<$A: bool, $B: bool> () -> int
    where A || B  // At least one must be true
= if A then 1 else 2
```

### Implementation

- [ ] **Grammar**: Update `grammar.ebnf` with const bound expression grammar
  - [ ] `const_bound_expr = const_or_expr`
  - [ ] `const_or_expr = const_and_expr { "||" const_and_expr }`
  - [ ] `const_and_expr = const_not_expr { "&&" const_not_expr }`
  - [ ] `const_not_expr = "!" const_not_expr | const_cmp_expr`
  - [ ] `const_cmp_expr = const_expr comparison_op const_expr | "(" const_bound_expr ")"`
  - [ ] **Rust Tests**: `ori_parser/tests/const_bound_grammar.rs`

- [ ] **Parser**: Parse const bounds
  - [x] In where clauses (compound expressions with `&&`, `||`, `!`) [done] (2026-02-13)
  - [x] Comparison expressions (`>`, `<`, `>=`, `<=`, `==`, `!=`) [done] (2026-02-13)
  - [x] Arithmetic in bounds (`+`, `-`, `*`, `/`, `%`) [done] (2026-02-13)
  - [x] Bitwise in bounds (`&`, `|`, `^`, `<<`, `>>`) [done] (2026-02-13)
  - [x] Multiple where clauses (implicitly AND-combined) [done] (2026-02-13)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item/generics.rs` — 5 where clause tests [done] (2026-02-13)
  - [ ] **LLVM Support**: LLVM codegen for parsed const bounds
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds parsing codegen

- [ ] **Type checker**: Validate const bounds at compile time
  - [ ] Check at instantiation when concrete values known
  - [ ] Defer to monomorphization when values unknown
  - [ ] Linear arithmetic implication checking (caller must imply callee bounds)
  - [ ] Transitivity (`M >= 20` implies `M >= 10`)
  - [ ] Equivalence (`M >= 10` implies `M > 9`)
  - [ ] **Rust Tests**: `ori_typeck/tests/const_bounds.rs`
  - [ ] **LLVM Support**: LLVM codegen for const bounds validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds validation codegen

- [ ] **Const evaluator**: Overflow handling
  - [ ] Arithmetic overflow during const bound evaluation is compile error (E1033)
  - [ ] 64-bit signed integer arithmetic
  - [ ] **Rust Tests**: `ori_typeck/tests/const_bound_overflow.rs`
  - [ ] **Ori Tests**: `tests/spec/types/const_bound_overflow.ori`

- [ ] **Error messages**: Const bound error codes
  - [ ] E1030: Const generic bound not satisfied
  - [ ] E1031: Caller bound does not imply callee bound (with help message)
  - [ ] E1032: Invalid const bound expression (method calls not allowed)
  - [ ] E1033: Const bound evaluation overflow
  - [ ] **Rust Tests**: `ori_reporting/tests/const_bound_errors.rs`

- [ ] **Test**: `tests/spec/types/const_bounds.ori`
  - [ ] Positive size constraint
  - [ ] Compound bounds with `&&` and `||`
  - [ ] Negation with `!`
  - [ ] Arithmetic in bounds (`N % 2 == 0`)
  - [ ] Bitwise in bounds (`N & (N - 1) == 0`)
  - [ ] Multiple where clauses
  - [ ] Bound violation error
  - [ ] Insufficient caller bound error
  - [ ] Bool const generics (`$B: bool`)
  - [ ] Bool in bounds (`A || B`)
  - [ ] **LLVM Support**: LLVM codegen for const bounds tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds tests codegen

---

## 18.6 Default Const Values

**Spec section**: `spec/06-types.md § Default Const Values`

### Syntax

```ori
// Default value for const parameter
type Buffer<$SIZE: int = 1024> = {
    data: [byte, max SIZE],
}

// Usage
let buf: Buffer = Buffer.new()           // SIZE = 1024
let small: Buffer<256> = Buffer.new()    // SIZE = 256

// In functions
@create_buffer<$SIZE: int = 4096> () -> Buffer<SIZE> = ...

let default_buf = create_buffer()         // 4096
let custom_buf = create_buffer<8192>()    // 8192
```

### Implementation

- [ ] **Spec**: Default const values
  - [ ] Syntax in declaration
  - [ ] Resolution at use site
  - [ ] Interaction with inference
  - [ ] **LLVM Support**: LLVM codegen for default const values
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — default const values codegen

- [ ] **Parser**: Parse default const
  - [ ] `= value` after const param
  - [ ] Must be const expression
  - [ ] **LLVM Support**: LLVM codegen for parsed default const
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — default const parsing codegen

- [ ] **Type checker**: Apply defaults
  - [ ] When not specified
  - [ ] Before other inference
  - [ ] **LLVM Support**: LLVM codegen for applying const defaults
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const defaults application codegen

- [ ] **Test**: `tests/spec/types/const_defaults.ori`
  - [ ] Type with default
  - [ ] Function with default
  - [ ] Override default
  - [ ] **LLVM Support**: LLVM codegen for const defaults tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const defaults tests codegen

---

## 18.7 Const in Trait Bounds

**Spec section**: `spec/07-properties-of-types.md § Const in Traits`

### Syntax

```ori
// Trait with const parameter
trait FixedSize {
    $SIZE: int
}

impl FixedSize for [int, max 5] {
    $SIZE: int = 5
}

// Use in bounds
@total_size<T: FixedSize, $N: int> () -> int = T.SIZE * N

// Associated const in generic context
@print_size<T: FixedSize> () -> void = {
    print(msg: `Size: {T.SIZE}`)
}
```

### Implementation

- [ ] **Spec**: Const in traits
  - [ ] Associated consts
  - [ ] Const bounds on traits
  - [ ] **LLVM Support**: LLVM codegen for const in traits
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const in traits codegen

- [ ] **Trait system**: Const associated items
  - [ ] Parse `const` in trait
  - [ ] Require in impl
  - [ ] Access via type
  - [ ] **LLVM Support**: LLVM codegen for const associated items
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const associated items codegen

- [ ] **Test**: `tests/spec/traits/const_associated.ori`
  - [ ] Trait with const
  - [ ] Impl with const
  - [ ] Use in generic context
  - [ ] **LLVM Support**: LLVM codegen for const associated tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const associated tests codegen

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Spec updated: `spec/06-types.md` and `spec/07-properties-of-types.md` const generics sections
- [ ] CLAUDE.md updated with const generic syntax
- [ ] `[T, max N]` fixed-capacity lists work
- [ ] `$N: int` const parameters in types work
- [ ] Const expressions in type positions work
- [ ] Const bounds work
- [ ] All tests pass: `./test-all.sh`

**Exit Criteria**: Can implement a matrix library with compile-time dimension checking

---

## Example: Matrix Library

```ori
type Matrix<$ROWS: int, $COLS: int> = {
    data: [float, max ROWS * COLS],
}

impl<$ROWS: int, $COLS: int> Matrix<ROWS, COLS> {
    @new () -> Matrix<ROWS, COLS> = Matrix {
        data: [],  // Will be filled with zeros
    }

    @get (self, row: int, col: int) -> float = {
        assert(condition: row >= 0 && row < ROWS)
        assert(condition: col >= 0 && col < COLS)
        self.data[row * COLS + col]
    }

    @set (self, row: int, col: int, value: float) -> void = {
        assert(condition: row >= 0 && row < ROWS)
        assert(condition: col >= 0 && col < COLS)
        self.data[row * COLS + col] = value
    }

    @rows (self) -> int = ROWS  // Compile-time constant

    @cols (self) -> int = COLS  // Compile-time constant
}

// Matrix multiplication with dimension checking at compile time
@multiply<$M: int, $N: int, $P: int> (
    a: Matrix<M, N>,
    b: Matrix<N, P>,
) -> Matrix<M, P> = {
    let result = Matrix.new()

    for i in 0..M do
        for j in 0..P do
            let sum = 0.0
            for k in 0..N do
                sum = sum + a.get(row: i, col: k) * b.get(row: k, col: j)
            result.set(row: i, col: j, value: sum)

    result
}

// Usage
let a: Matrix<2, 3> = Matrix.new()
let b: Matrix<3, 4> = Matrix.new()
let c: Matrix<2, 4> = multiply(a: a, b: b)  // Types checked at compile time!

// This would be a compile error:
// let bad: Matrix<2, 5> = multiply(a: a, b: b)  // Error: dimension mismatch
```

---

## 18.8 Expanded Const Generic Eligibility (Capability Unification)

**Proposal**: `proposals/approved/capability-unification-generics-proposal.md` — Phase 3

Replace the `{int, bool}` whitelist with: any type with `Eq + Hashable` is const-eligible. This expands const generics to `str`, `char`, `byte`, user enums, user structs, and compound types.

**Blocked by**: Section 5.5 (Compound Type Inference) — needed for `[int]` and `(T, U)` eligibility <!-- blocked-by:5 -->

### Implementation

- [ ] **Implement**: Type checker — replace hardcoded `matches!(type, Int | Bool)` with trait registry lookup for `Eq + Hashable`
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — const eligibility check tests
- [ ] **Implement**: Update E1040 error message — "requires Eq + Hashable" instead of "only int and bool allowed"
  - [ ] **Ori Tests**: `tests/compile-fail/const_generic_not_eligible.ori`
- [ ] **Ori Tests**: `tests/spec/types/const_generics_expanded.ori` — str, char, byte, user enum, user struct as const params
- [ ] **Update Spec**: `grammar.ebnf` — remove `const_type = "int" | "bool"` restriction
- [ ] **Update Spec**: `06-types.md` — const generic eligibility section
- [ ] **Verify**: `./test-all.sh` passes

---

## 18.9 Associated Consts in Traits (Capability Unification)

**Proposal**: `proposals/approved/capability-unification-generics-proposal.md` — Phase 4

Add `$name: Type` syntax to trait definitions and impls. Extends the associated types pattern to compile-time values.

**Blocked by**: Section 18.0 (Const Evaluation Termination) <!-- blocked-by:18 -->

### Implementation

- [ ] **Implement**: Parser — accept `$name: Type` and `$name: Type = expr` as trait items
- [ ] **Implement**: Parser — accept `$name = expr` as impl items
- [ ] **Implement**: IR — add `AssocConst` to `TraitItem` and `ImplItem`
- [ ] **Implement**: Type checker — register associated consts alongside methods and types
- [ ] **Implement**: Type checker — const expression unification for `T.$rank` in where clauses
- [ ] **Implement**: Evaluator — associated const resolution
- [ ] **Ori Tests**: `tests/spec/traits/associated_consts.ori`
- [ ] **LLVM Support**: Const folding for associated consts
- [ ] **Verify**: `./test-all.sh` passes

---

## 18.10 Const Functions in Type Positions (Capability Unification)

**Proposal**: `proposals/approved/capability-unification-generics-proposal.md` — Phase 5

Allow `$product(S)`, `$len(S)` etc. in type positions and where clauses. Most complex phase.

**Blocked by**: Section 18.9 (Associated Consts) <!-- blocked-by:18 -->

### Implementation

- [ ] **Implement**: Const function analysis — identify which functions are compile-time evaluable
- [ ] **Implement**: Type checker — const expression evaluation in type positions
- [ ] **Implement**: Type checker — const unification (`$product(FROM)` unifies with concrete values)
- [ ] **Implement**: Built-in const functions: `$len`, `$product`, `$sum`, `$min`, `$max`
- [ ] **Ori Tests**: `tests/spec/types/const_functions_in_types.ori`
- [ ] **LLVM Support**: Compile-time evaluation in LLVM codegen
- [ ] **Verify**: `./test-all.sh` passes
