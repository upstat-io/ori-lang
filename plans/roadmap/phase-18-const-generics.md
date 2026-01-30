# Phase 18: Const Generics

**Goal**: Enable type parameters that are compile-time constant values

**Criticality**: Medium — Type-level programming, fixed-size arrays

**Dependencies**: Phases 1-2 (Type System Foundation)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Value types | `int`, `bool` initially | Start simple, expand later |
| Array syntax | `[T; N]` | Familiar from Rust |
| Const expressions | Limited initially | Avoid complexity |
| Default values | Supported | API ergonomics |

---

## Reference Implementation

### Rust

```
~/lang_repos/rust/compiler/rustc_middle/src/ty/consts.rs    # Const type representation
~/lang_repos/rust/compiler/rustc_hir/src/def.rs             # ConstParam definition
~/lang_repos/rust/compiler/rustc_hir_typeck/src/lib.rs      # Const generic checking
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

**Spec section**: `spec/06-types.md § Const Generic Parameters`

### Syntax

```ori
// Const parameter in type
type Array<T, const N: int> = {
    data: *T,  // Or internal representation
    // len is known at compile time: N
}

// Usage
let arr: Array<int, 5> = Array.new()
arr[0] = 42

// In functions
@zeros<const N: int> () -> Array<int, N> = ...

let five_zeros: Array<int, 5> = zeros()
```

### Grammar

```ebnf
TypeParameter = Identifier [ ':' TypeBound ]
              | 'const' Identifier ':' ConstType ;
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
  - [ ] `const` keyword in generics
  - [ ] Type annotation required
  - [ ] Position (can mix with type params)
  - [ ] **LLVM Support**: LLVM codegen for parsed const parameters
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const parameter parsing codegen

- [ ] **Type checker**: Const parameter validation
  - [ ] Track const vs type parameters
  - [ ] Validate const type (int, bool)
  - [ ] Unification with const values
  - [ ] **LLVM Support**: LLVM codegen for const parameter validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const parameter validation codegen

- [ ] **Test**: `tests/spec/types/const_generics.ori`
  - [ ] Basic const parameter
  - [ ] Multiple const parameters
  - [ ] Mixed type and const
  - [ ] **LLVM Support**: LLVM codegen for const generic tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const generic tests codegen

---

## 18.2 Fixed-Size Arrays

**Spec section**: `spec/06-types.md § Fixed-Size Arrays`

### Syntax

```ori
// Array type with const size
let arr: [int; 5] = [0, 0, 0, 0, 0]

// Inferred size from literal
let arr = [1, 2, 3]  // Type: [int; 3]

// Array operations
let len = arr.len()  // Const 5, known at compile time
let elem = arr[2]    // Bounds checked at compile time if index const

// Conversion to slice
let slice: [int] = arr.as_slice()
```

### Type Rules

```
[T; N] where N: int (const)
- Length known at compile time
- Stack allocated (no heap)
- Bounds checks can be optimized away for const indices
```

### Implementation

- [ ] **Spec**: Fixed-size array type
  - [ ] Syntax `[T; N]`
  - [ ] Relationship to dynamic `[T]`
  - [ ] Operations
  - [ ] **LLVM Support**: LLVM codegen for fixed-size array type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — fixed-size array type codegen

- [ ] **Types**: Array type with const
  - [ ] `Type::Array(elem, ConstValue)`
  - [ ] Distinct from `Type::List(elem)`
  - [ ] **LLVM Support**: LLVM codegen for array type with const
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — array type with const codegen

- [ ] **Parser**: Parse array types
  - [ ] `[T; expr]` syntax
  - [ ] Const expression for size
  - [ ] **LLVM Support**: LLVM codegen for parsed array types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — array type parsing codegen

- [ ] **Type checker**: Array type checking
  - [ ] Validate size is const
  - [ ] Literal inference
  - [ ] Bounds check optimization
  - [ ] **LLVM Support**: LLVM codegen for array type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — array type checking codegen

- [ ] **Test**: `tests/spec/types/fixed_arrays.ori`
  - [ ] Array declaration
  - [ ] Array literal with inferred size
  - [ ] Slice conversion
  - [ ] **LLVM Support**: LLVM codegen for fixed array tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — fixed array tests codegen

---

## 18.3 Const Expressions in Types

**Spec section**: `spec/06-types.md § Const Expressions`

### Syntax

```ori
// Arithmetic in const position
type Matrix<const ROWS: int, const COLS: int> = {
    data: [float; ROWS * COLS],
}

// Const function in type
$double (n: int) -> int = n * 2

type DoubleArray<const N: int> = {
    data: [int; $double(n: N)],
}

// Conditional const
type Buffer<const SIZE: int> = {
    data: [byte; if SIZE > 0 then SIZE else 1],
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

## 18.4 Const Bounds

**Spec section**: `spec/06-types.md § Const Bounds`

### Syntax

```ori
// Bound on const parameter value
@non_empty_array<const N: int> () -> [int; N]
    where N > 0  // Const bound
= ...

// Multiple bounds
@matrix_multiply<const M: int, const N: int, const P: int> (
    a: Matrix<M, N>,
    b: Matrix<N, P>,
) -> Matrix<M, P>
    where M > 0, N > 0, P > 0
= ...

// Equality constraints
@square_matrix<const N: int> () -> Matrix<N, N> = ...
```

### Implementation

- [ ] **Spec**: Const bounds syntax
  - [ ] Where clause for const
  - [ ] Comparison operators
  - [ ] Equality constraints
  - [ ] **LLVM Support**: LLVM codegen for const bounds syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds syntax codegen

- [ ] **Parser**: Parse const bounds
  - [ ] In where clauses
  - [ ] Comparison expressions
  - [ ] **LLVM Support**: LLVM codegen for parsed const bounds
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds parsing codegen

- [ ] **Type checker**: Validate const bounds
  - [ ] Check at instantiation
  - [ ] Error messages
  - [ ] **LLVM Support**: LLVM codegen for const bounds validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds validation codegen

- [ ] **Test**: `tests/spec/types/const_bounds.ori`
  - [ ] Positive size constraint
  - [ ] Equality constraint
  - [ ] Bound violation error
  - [ ] **LLVM Support**: LLVM codegen for const bounds tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/const_generic_tests.rs` — const bounds tests codegen

---

## 18.5 Default Const Values

**Spec section**: `spec/06-types.md § Default Const Values`

### Syntax

```ori
// Default value for const parameter
type Buffer<const SIZE: int = 1024> = {
    data: [byte; SIZE],
}

// Usage
let buf: Buffer = Buffer.new()           // SIZE = 1024
let small: Buffer<256> = Buffer.new()    // SIZE = 256

// In functions
@create_buffer<const SIZE: int = 4096> () -> Buffer<SIZE> = ...

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

## 18.6 Const in Trait Bounds

**Spec section**: `spec/07-properties-of-types.md § Const in Traits`

### Syntax

```ori
// Trait with const parameter
trait FixedSize {
    const SIZE: int
}

impl FixedSize for [int; 5] {
    const SIZE: int = 5
}

// Use in bounds
@total_size<T: FixedSize, const N: int> () -> int = T.SIZE * N

// Associated const in generic context
@print_size<T: FixedSize> () -> void = run(
    print(`Size: {T.SIZE}`)
)
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

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/06-types.md` and `spec/07-properties-of-types.md` const generics sections
- [ ] CLAUDE.md updated with const generic syntax
- [ ] `[T; N]` fixed-size arrays work
- [ ] Const parameters in types work
- [ ] Const expressions in type positions work
- [ ] Const bounds work
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Can implement a matrix library with compile-time dimension checking

---

## Example: Matrix Library

```ori
type Matrix<const ROWS: int, const COLS: int> = {
    data: [float; ROWS * COLS],
}

impl<const ROWS: int, const COLS: int> Matrix<ROWS, COLS> {
    @new () -> Matrix<ROWS, COLS> = Matrix {
        data: [0.0; ROWS * COLS],
    }

    @get (self, row: int, col: int) -> float = run(
        assert(row >= 0 && row < ROWS)
        assert(col >= 0 && col < COLS)
        self.data[row * COLS + col]
    )

    @set (self, row: int, col: int, value: float) -> void = run(
        assert(row >= 0 && row < ROWS)
        assert(col >= 0 && col < COLS)
        self.data[row * COLS + col] = value
    )

    @rows (self) -> int = ROWS  // Compile-time constant

    @cols (self) -> int = COLS  // Compile-time constant
}

// Matrix multiplication with dimension checking at compile time
@multiply<const M: int, const N: int, const P: int> (
    a: Matrix<M, N>,
    b: Matrix<N, P>,
) -> Matrix<M, P> = run(
    let mut result = Matrix.new()

    for i in 0..M do
        for j in 0..P do
            let mut sum = 0.0
            for k in 0..N do
                sum = sum + a.get(row: i, col: k) * b.get(row: k, col: j)
            result.set(row: i, col: j, value: sum)

    result
)

// Usage
let a: Matrix<2, 3> = Matrix.new()
let b: Matrix<3, 4> = Matrix.new()
let c: Matrix<2, 4> = multiply(a: a, b: b)  // Types checked at compile time!

// This would be a compile error:
// let bad: Matrix<2, 5> = multiply(a: a, b: b)  // Error: dimension mismatch
```
