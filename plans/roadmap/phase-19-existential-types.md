# Phase 19: Existential Types (impl Trait)

**Goal**: Enable returning opaque types that implement a trait without exposing concrete type

**Criticality**: Low — API design improvement

**Dependencies**: Phase 3 (Traits)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Syntax | `impl Trait` | Matches Rust, clear meaning |
| Position | Return only (initially) | Simpler, covers main use case |
| Multiple traits | `impl A + B` | Flexibility |
| Inference | Per-function | Predictable |

---

## Reference Implementation

### Rust

```
~/lang_repos/rust/compiler/rustc_hir/src/hir.rs     # OpaqueTy definition
~/lang_repos/rust/compiler/rustc_hir_typeck/src/   # Type inference for impl Trait
~/lang_repos/rust/compiler/rustc_middle/src/ty/    # Type representation
```

---

## 19.1 Return Position impl Trait

**Spec section**: `spec/06-types.md § Existential Types`

### Syntax

```ori
// Return opaque type
@make_iterator (items: [int]) -> impl Iterator<Item = int> = run(
    items.iter()
)

// Caller sees: impl Iterator<Item = int>
// Cannot access concrete type
let iter = make_iterator(items: [1, 2, 3])
for x in iter do print(str(x))  // Works via Iterator trait

// Multiple bounds
@make_printable_iterator () -> impl Iterator<Item = int> + Clone = ...
```

### Semantics

- Return type is opaque to caller
- Compiler knows concrete type internally
- All return paths must return same concrete type
- Trait bounds must be satisfied

### Implementation

- [ ] **Spec**: Existential type syntax
  - [ ] `impl Trait` in return position
  - [ ] Multiple bounds with `+`
  - [ ] Associated type constraints
  - [ ] **LLVM Support**: LLVM codegen for existential type syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — existential type syntax codegen

- [ ] **Parser**: Parse impl Trait
  - [ ] In return type position
  - [ ] Trait bounds parsing
  - [ ] Associated types
  - [ ] **LLVM Support**: LLVM codegen for parsed impl Trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — impl Trait parsing codegen

- [ ] **Type checker**: Existential type handling
  - [ ] Infer concrete type from body
  - [ ] Verify all returns same type
  - [ ] Check trait bounds satisfied
  - [ ] **LLVM Support**: LLVM codegen for existential type handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — existential type handling codegen

- [ ] **Test**: `tests/spec/types/impl_trait.ori`
  - [ ] Basic impl Trait return
  - [ ] Multiple bounds
  - [ ] Associated type constraints
  - [ ] **LLVM Support**: LLVM codegen for impl Trait tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — impl Trait tests codegen

---

## 19.2 Type Inference

**Spec section**: `spec/06-types.md § Existential Type Inference`

### Rules

```ori
// Concrete type inferred from function body
@numbers () -> impl Iterator<Item = int> = run(
    [1, 2, 3].iter()  // Concrete: ListIterator<int>
)

// All return paths must have same concrete type
@maybe_numbers (flag: bool) -> impl Iterator<Item = int> = run(
    if flag then
        [1, 2, 3].iter()
    else
        [4, 5, 6].iter()  // OK: same concrete type
)

// Error: different concrete types
@bad_numbers (flag: bool) -> impl Iterator<Item = int> = run(
    if flag then
        [1, 2, 3].iter()       // ListIterator<int>
    else
        (1..10).iter()         // RangeIterator<int>
    // Error: impl Trait returns different types
)
```

### Implementation

- [ ] **Spec**: Inference rules
  - [ ] Single concrete type requirement
  - [ ] Branch unification
  - [ ] Error messages
  - [ ] **LLVM Support**: LLVM codegen for impl Trait inference rules
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — inference rules codegen

- [ ] **Type checker**: Unify return types
  - [ ] Track expected opaque type
  - [ ] Unify concrete returns
  - [ ] Clear error on mismatch
  - [ ] **LLVM Support**: LLVM codegen for return type unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — return type unification codegen

- [ ] **Diagnostics**: Helpful errors
  - [ ] Show both concrete types
  - [ ] Suggest Box<dyn Trait>
  - [ ] **LLVM Support**: LLVM codegen for impl Trait diagnostics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — diagnostics codegen

- [ ] **Test**: `tests/spec/types/impl_trait_inference.ori`
  - [ ] Multiple return paths same type
  - [ ] Error on different types
  - [ ] **LLVM Support**: LLVM codegen for impl Trait inference tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — impl Trait inference tests codegen

---

## 19.3 Associated Type Constraints

**Spec section**: `spec/06-types.md § Existential Associated Types`

### Syntax

```ori
// Constrain associated type
@int_iterator () -> impl Iterator<Item = int> = ...

// Use with other traits
@cloneable_ints () -> impl Iterator<Item = int> + Clone = ...

// Multiple associated types
trait Mapping {
    type Key
    type Value
    @get (self, key: Self.Key) -> Option<Self.Value>
}

@string_int_map () -> impl Mapping<Key = str, Value = int> = ...
```

### Implementation

- [ ] **Spec**: Associated type syntax
  - [ ] `<Assoc = Type>` constraint
  - [ ] Multiple constraints
  - [ ] **LLVM Support**: LLVM codegen for associated type syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — associated type syntax codegen

- [ ] **Type checker**: Validate associated types
  - [ ] Match concrete type's assoc types
  - [ ] Error on mismatch
  - [ ] **LLVM Support**: LLVM codegen for associated type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — associated type validation codegen

- [ ] **Test**: `tests/spec/types/impl_trait_assoc.ori`
  - [ ] Iterator with Item
  - [ ] Custom trait with assoc types
  - [ ] **LLVM Support**: LLVM codegen for associated type tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — associated type tests codegen

---

## 19.4 Limitations and Errors

**Spec section**: `spec/06-types.md § Existential Limitations`

### Not Supported (Initially)

```ori
// Argument position - NOT supported
@take_iterator (iter: impl Iterator<Item = int>) -> void = ...
// Use generic instead:
@take_iterator<T: Iterator<Item = int>> (iter: T) -> void = ...

// In struct fields - NOT supported
type Container = {
    iter: impl Iterator<Item = int>,  // Error
}
// Use generic instead:
type Container<T: Iterator<Item = int>> = {
    iter: T,
}

// In trait definitions - NOT supported
trait Foo {
    @make () -> impl Bar  // Error
}
// Use associated type instead:
trait Foo {
    type Output: Bar
    @make () -> Self.Output
}
```

### Error Messages

```
error: `impl Trait` is only allowed in return position
  --> src/main.ori:5:20
  |
5 | @foo (x: impl Trait) -> void
  |          ^^^^^^^^^^ impl Trait not allowed here
  |
  = help: use a generic parameter instead: @foo<T: Trait> (x: T) -> void
```

### Implementation

- [ ] **Spec**: Document limitations
  - [ ] Return position only
  - [ ] Not in structs
  - [ ] Not in traits
  - [ ] **LLVM Support**: LLVM codegen for impl Trait limitations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — limitations codegen

- [ ] **Type checker**: Reject invalid positions
  - [ ] Error on arg position
  - [ ] Error in struct fields
  - [ ] Error in trait methods
  - [ ] **LLVM Support**: LLVM codegen for invalid position rejection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — invalid position rejection codegen

- [ ] **Diagnostics**: Suggest alternatives
  - [ ] Generic parameter
  - [ ] Associated type
  - [ ] **LLVM Support**: LLVM codegen for alternative suggestions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — alternative suggestions codegen

- [ ] **Test**: `tests/compile-fail/types/impl_trait_position.ori`
  - [ ] Arg position error
  - [ ] Struct field error
  - [ ] Trait method error
  - [ ] **LLVM Support**: LLVM codegen for position error tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — position error tests codegen

---

## 19.5 impl Trait vs dyn Trait

**Spec section**: `spec/06-types.md § Static vs Dynamic Dispatch`

### Comparison

| Feature | `impl Trait` | `dyn Trait` |
|---------|--------------|-------------|
| Dispatch | Static (monomorphized) | Dynamic (vtable) |
| Size | Concrete type size | Pointer + vtable |
| Performance | Better (inlined) | Overhead |
| Flexibility | One concrete type | Any type at runtime |
| Recursion | Cannot (infinite size) | Can (via Box) |

### When to Use

```ori
// Use impl Trait: single concrete type, performance matters
@fast_iterator () -> impl Iterator<Item = int> = [1, 2, 3].iter()

// Use dyn Trait: multiple types possible, flexibility needed
@any_iterator (flag: bool) -> Box<dyn Iterator<Item = int>> = run(
    if flag then
        Box.new([1, 2, 3].iter())
    else
        Box.new((1..10).iter())
)
```

### Implementation

- [ ] **Spec**: Compare impl vs dyn
  - [ ] Use cases
  - [ ] Performance implications
  - [ ] When each is appropriate
  - [ ] **LLVM Support**: LLVM codegen for impl vs dyn comparison
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — impl vs dyn comparison codegen

- [ ] **Documentation**: Best practices guide
  - [ ] Decision flowchart
  - [ ] Common patterns
  - [ ] **LLVM Support**: LLVM codegen for best practices examples
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — best practices examples codegen

- [ ] **Test**: `tests/spec/types/impl_vs_dyn.ori`
  - [ ] impl Trait usage
  - [ ] dyn Trait usage
  - [ ] Conversion between them
  - [ ] **LLVM Support**: LLVM codegen for impl vs dyn tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/impl_trait_tests.rs` — impl vs dyn tests codegen

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/06-types.md` existential types section
- [ ] CLAUDE.md updated with impl Trait syntax
- [ ] Return position `impl Trait` works
- [ ] Type inference correct
- [ ] Associated type constraints work
- [ ] Clear errors for invalid positions
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Can write iterator-returning functions with clean APIs

---

## Example: Iterator Combinators

```ori
// Clean API with impl Trait in return position
// Note: impl Trait is only allowed in return position, not argument position
// Use generics for arguments instead

@map<I: Iterator, U> (
    iter: I,
    f: (I.Item) -> U,
) -> impl Iterator<Item = U> = run(
    MapIterator { inner: iter, transform: f }
)

@filter<I: Iterator> (
    iter: I,
    predicate: (I.Item) -> bool,
) -> impl Iterator<Item = I.Item> = run(
    FilterIterator { inner: iter, predicate: predicate }
)

@take<I: Iterator<Item = int>> (
    iter: I,
    n: int,
) -> impl Iterator<Item = int> = run(
    TakeIterator { inner: iter, remaining: n }
)

// Usage - clean, composable
@first_10_even_squares () -> impl Iterator<Item = int> = run(
    (1..100)
        |> filter(predicate: n -> n % 2 == 0)
        |> map(f: n -> n * n)
        |> take(n: 10)
)

// Caller doesn't know concrete type (MapIterator<FilterIterator<...>>)
// but can use it as Iterator
let squares = first_10_even_squares()
for sq in squares do print(str(sq))
```
