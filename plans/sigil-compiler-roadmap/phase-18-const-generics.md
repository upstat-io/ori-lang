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

## 18.1 Const Type Parameters

**Spec section**: `spec/06-types.md § Const Generic Parameters`

### Syntax

```sigil
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

- [ ] **Parser**: Parse const parameters
  - [ ] `const` keyword in generics
  - [ ] Type annotation required
  - [ ] Position (can mix with type params)

- [ ] **Type checker**: Const parameter validation
  - [ ] Track const vs type parameters
  - [ ] Validate const type (int, bool)
  - [ ] Unification with const values

- [ ] **Test**: `tests/spec/types/const_generics.si`
  - [ ] Basic const parameter
  - [ ] Multiple const parameters
  - [ ] Mixed type and const

---

## 18.2 Fixed-Size Arrays

**Spec section**: `spec/06-types.md § Fixed-Size Arrays`

### Syntax

```sigil
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

- [ ] **Types**: Array type with const
  - [ ] `Type::Array(elem, ConstValue)`
  - [ ] Distinct from `Type::List(elem)`

- [ ] **Parser**: Parse array types
  - [ ] `[T; expr]` syntax
  - [ ] Const expression for size

- [ ] **Type checker**: Array type checking
  - [ ] Validate size is const
  - [ ] Literal inference
  - [ ] Bounds check optimization

- [ ] **Test**: `tests/spec/types/fixed_arrays.si`
  - [ ] Array declaration
  - [ ] Array literal with inferred size
  - [ ] Slice conversion

---

## 18.3 Const Expressions in Types

**Spec section**: `spec/06-types.md § Const Expressions`

### Syntax

```sigil
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

- [ ] **Const evaluator**: Evaluate const expressions
  - [ ] At type checking time
  - [ ] Cache results
  - [ ] Error on non-const

- [ ] **Type checker**: Validate const expressions
  - [ ] All operands must be const
  - [ ] Result must be correct type

- [ ] **Test**: `tests/spec/types/const_expressions.si`
  - [ ] Arithmetic in types
  - [ ] Const functions in types
  - [ ] Conditional const

---

## 18.4 Const Bounds

**Spec section**: `spec/06-types.md § Const Bounds`

### Syntax

```sigil
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

- [ ] **Parser**: Parse const bounds
  - [ ] In where clauses
  - [ ] Comparison expressions

- [ ] **Type checker**: Validate const bounds
  - [ ] Check at instantiation
  - [ ] Error messages

- [ ] **Test**: `tests/spec/types/const_bounds.si`
  - [ ] Positive size constraint
  - [ ] Equality constraint
  - [ ] Bound violation error

---

## 18.5 Default Const Values

**Spec section**: `spec/06-types.md § Default Const Values`

### Syntax

```sigil
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

- [ ] **Parser**: Parse default const
  - [ ] `= value` after const param
  - [ ] Must be const expression

- [ ] **Type checker**: Apply defaults
  - [ ] When not specified
  - [ ] Before other inference

- [ ] **Test**: `tests/spec/types/const_defaults.si`
  - [ ] Type with default
  - [ ] Function with default
  - [ ] Override default

---

## 18.6 Const in Trait Bounds

**Spec section**: `spec/07-properties-of-types.md § Const in Traits`

### Syntax

```sigil
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

- [ ] **Trait system**: Const associated items
  - [ ] Parse `const` in trait
  - [ ] Require in impl
  - [ ] Access via type

- [ ] **Test**: `tests/spec/traits/const_associated.si`
  - [ ] Trait with const
  - [ ] Impl with const
  - [ ] Use in generic context

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/06-types.md` and `spec/07-properties-of-types.md` const generics sections
- [ ] CLAUDE.md updated with const generic syntax
- [ ] `[T; N]` fixed-size arrays work
- [ ] Const parameters in types work
- [ ] Const expressions in type positions work
- [ ] Const bounds work
- [ ] All tests pass: `cargo test && sigil test tests/spec/types/`

**Exit Criteria**: Can implement a matrix library with compile-time dimension checking

---

## Example: Matrix Library

```sigil
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
