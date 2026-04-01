---
section: 19
title: Existential Types (impl Trait)
status: not-started
last_verified: "2026-03-29"
reviewed: true
tier: 7
goal: Enable returning opaque types that implement a trait without exposing concrete type
spec:
  - spec/08-types.md
sections:
  - id: "19.1"
    title: Return Position impl Trait
    status: not-started
  - id: "19.2"
    title: Type Inference
    status: not-started
  - id: "19.3"
    title: Associated Type Constraints
    status: not-started
  - id: "19.4"
    title: Limitations and Errors
    status: not-started
  - id: "19.5"
    title: impl Trait vs dyn Trait
    status: not-started
---

# Section 19: Existential Types (impl Trait)

**Goal**: Enable returning opaque types that implement a trait without exposing concrete type

**Criticality**: Low — API design improvement

**Dependencies**: Section 3 (Traits), Monomorphization infrastructure (for AOT compilation — `impl Trait` is statically dispatched/monomorphized)

**Proposal**: `proposals/approved/existential-types-proposal.md`

> **Verified 2026-03-29**: Status `not-started` confirmed accurate -- zero implementation exists across all compiler phases (no `ExistentialType` in ParsedType, no parser support, no type pool tag, no typeck machinery). Spec section 8.15, grammar EBNF, and proposal are all written and ready. Error codes E0810/E0811/E0812 defined in spec but not implemented in `ori_diagnostic`. NOTE: LLVM sub-items throughout this section are inflated -- `impl Trait` is erased at type-check time to concrete types, so no special LLVM codegen is needed (standard ARC + LLVM pipeline handles it automatically). NOTE: existing test file `tests/spec/types/existential.ori` references wrong spec section (`06-types.md` instead of `08-types.md`), has commented-out code (hygiene violation), and decorative banners.

### Sync Points: Opaque Type Representation (multi-crate sync required)

Adding `impl Trait` return types requires updates across these crates:

1. **`ori_ir`** — Add `Type::ImplTrait { bounds, where_clause }` variant in type AST
2. **`ori_types`** — Infer concrete type from function body, verify all return paths yield same type, check trait bounds satisfied, reject invalid positions (arg, struct field)
3. **`ori_eval`** — Evaluator sees concrete type (opaque only to callers), no special handling needed
4. **`ori_llvm`** — Monomorphize to concrete type at codegen time, no vtable needed (static dispatch)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Syntax | `impl Trait where Assoc == Type` | Type-local where clause for associated types |
| Position | Return only | Argument position uses generics instead |
| Multiple traits | `impl A + B` | Flexibility |
| Inference | Per-function, from body | Predictable |
| Where clause | Type-local | Constraints on associated types, not type params |

---

## Reference Implementation

### Rust

```
~/projects/reference_repos/lang_repos/rust/compiler/rustc_hir/src/hir.rs     # OpaqueTy definition
~/projects/reference_repos/lang_repos/rust/compiler/rustc_hir_typeck/src/   # Type inference for impl Trait
~/projects/reference_repos/lang_repos/rust/compiler/rustc_middle/src/ty/    # Type representation
```

### Swift

```
# Swift has `any Protocol` (existential) and `some Protocol` (opaque return)
~/projects/reference_repos/lang_repos/swift/lib/Sema/CSSimplify.cpp         # Existential type solving
~/projects/reference_repos/lang_repos/swift/lib/AST/Type.cpp                # ExistentialType representation
```

---

## 19.1 Return Position impl Trait
<!-- unblocks:0.4.2 -->
<!-- unblocks:0.9.1 -->

**Spec section**: `spec/08-types.md § 8.15 Existential Types`

> (verified 2026-03-29) Not started. No `ExistentialType` variant in `ParsedType` enum (13 variants, none for impl Trait). Parser type grammar never references `TokenKind::Impl`. No opaque type tag in type pool. LLVM sub-items below are inflated -- `impl Trait` is erased to concrete type before codegen, no special LLVM handling needed. Missing error codes: E0810 (invalid position), E0811 (different concrete types), E0812 (associated type mismatch) -- defined in spec but not in `ori_diagnostic`.

### Syntax

```ori
// Return opaque type
@make_iterator (items: [int]) -> impl Iterator where Item == int = {
    items.iter()
}

// Caller sees: impl Iterator where Item == int
// Cannot access concrete type
let iter = make_iterator(items: [1, 2, 3])
for x in iter do print(msg: `{x}`)  // Works via Iterator trait

// Multiple bounds
@make_printable_iterator () -> impl Iterator + Clone where Item == int = ...
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
  - [ ] **LLVM/AOT**: Auto-satisfied once typeck resolves `impl Trait` to concrete types (no special codegen needed -- static dispatch, erased before LLVM)

- [ ] **Parser**: Parse impl Trait
  - [ ] In return type position (`TokenKind::Impl` exists but type parser does not consume it)
  - [ ] Trait bounds parsing in type context
  - [ ] Associated types (`where Assoc == Type` in impl-trait context)

- [ ] **Type checker**: Existential type handling
  - [ ] Infer concrete type from body
  - [ ] Verify all returns same type (E0811 on mismatch)
  - [ ] Check trait bounds satisfied
  - [ ] Register error codes E0810, E0811, E0812 in `ori_diagnostic`

- [ ] **Test**: `tests/spec/types/impl_trait.ori` (existing `existential.ori` has all `impl Trait` tests commented out -- only generic/trait fallbacks pass)
  - [ ] Basic impl Trait return
  - [ ] Multiple bounds
  - [ ] Associated type constraints
  - [ ] **LLVM/AOT**: Standard dual-execution parity tests (no special codegen tests needed)

---

## 19.2 Type Inference

> (verified 2026-03-29) Not started. No opaque type representation, no inference machinery, no same-concrete-type unification.

**Spec section**: `spec/08-types.md § 8.15.2 Existential Type Inference`

### Rules

```ori
// Concrete type inferred from function body
@numbers () -> impl Iterator where Item == int = {
    [1, 2, 3].iter()  // Concrete: ListIterator<int>
}

// All return paths must have same concrete type
@maybe_numbers (flag: bool) -> impl Iterator where Item == int = {
    if flag then
        [1, 2, 3].iter()
    else
        [4, 5, 6].iter()  // OK: same concrete type
}

// Error: different concrete types
@bad_numbers (flag: bool) -> impl Iterator where Item == int = {
    if flag then
        [1, 2, 3].iter()       // ListIterator<int>
    else
        (1..10).iter()         // RangeIterator<int>
    // Error: impl Trait returns different types
}
```

### Implementation

- [ ] **Spec**: Inference rules
  - [ ] Single concrete type requirement
  - [ ] Branch unification
  - [ ] Error messages (E0811: different concrete types in return paths)

- [ ] **Type checker**: Unify return types
  - [ ] Track expected opaque type
  - [ ] Unify concrete returns
  - [ ] Clear error on mismatch (E0811)

- [ ] **Diagnostics**: Helpful errors
  - [ ] Show both concrete types on E0811
  - [ ] Suggest `Trait` object when different concrete types needed

- [ ] **Test**: `tests/spec/types/impl_trait_inference.ori`
  - [ ] Multiple return paths same type
  - [ ] Error on different types (`#compile_fail`)
  - [ ] **LLVM/AOT**: Dual-execution parity tests (no special codegen)

---

## 19.3 Associated Type Constraints

> (verified 2026-03-29) Not started. Grammar EBNF has `impl_where_clause` and `assoc_constraint` productions but no parser or typeck implementation.

**Spec section**: `spec/08-types.md § 8.15 Existential Associated Types`

### Syntax

```ori
// Constrain associated type
@int_iterator () -> impl Iterator where Item == int = ...

// Use with other traits
@cloneable_ints () -> impl Iterator + Clone where Item == int = ...

// Multiple associated types
trait Mapping {
    type Key
    type Value
    @get (self, key: Self.Key) -> Option<Self.Value>
}

@string_int_map () -> impl Mapping where Key == str, Value == int = ...
```

### Implementation

- [ ] **Spec**: Associated type syntax
  - [ ] `where Assoc == Type` constraint (Ori uses `==` not `=`)
  - [ ] Multiple constraints

- [ ] **Type checker**: Validate associated types
  - [ ] Match concrete type's assoc types against constraints
  - [ ] Error on mismatch (E0812)

- [ ] **Test**: `tests/spec/types/impl_trait_assoc.ori`
  - [ ] Iterator with `where Item == int`
  - [ ] Custom trait with assoc types
  - [ ] **LLVM/AOT**: Dual-execution parity tests (no special codegen)

---

## 19.4 Limitations and Errors

> (verified 2026-03-29) Not started. PLAN INACCURACY CORRECTED: "Not in traits" was wrong -- spec 8.15.3 ALLOWS `impl Trait` in trait method returns with specific semantics. Corrected to "Validate impl Trait in trait method returns" below. Error code E0810 (invalid position) defined in spec but not implemented.

**Spec section**: `spec/08-types.md § 8.15.3 Existential Limitations`

### Not Supported

```ori
// Argument position - NOT supported (use generics)
@take_iterator (iter: impl Iterator where Item == int) -> void = ...  // Error
// Correct:
@take_iterator<I: Iterator> (iter: I) -> void where I.Item == int = ...

// In struct fields - NOT supported (use generics)
type Container = {
    iter: impl Iterator where Item == int,  // Error
}
// Correct:
type Container<I: Iterator> = { iter: I } where I.Item == int
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
  - [ ] Return position only (E0810 on invalid position)
  - [ ] Not in struct fields
  - [ ] Allowed in trait method returns (with constraints -- spec 8.15.3)

- [ ] **Type checker**: Reject invalid positions
  - [ ] Error on arg position (E0810, suggest generic parameter)
  - [ ] Error in struct fields (E0810, suggest generic type parameter)
  - [ ] Validate `impl Trait` in trait method returns (allowed per spec, not "error")

- [ ] **Diagnostics**: Suggest alternatives
  - [ ] Generic parameter suggestion on E0810 (arg position)
  - [ ] Generic type parameter suggestion on E0810 (struct field)

- [ ] **Test**: `tests/compile-fail/types/impl_trait_position.ori`
  - [ ] Arg position error (`#compile_fail`)
  - [ ] Struct field error (`#compile_fail`)
  - [ ] Trait method return (should compile -- positive test, not error)

---

## 19.5 impl Trait vs dyn Trait

> (verified 2026-03-29) Not started. Documentation/comparison subsection -- spec 8.15.4 has comparison table.

**Spec section**: `spec/08-types.md § 8.15.4 Static vs Dynamic Dispatch`

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
@fast_iterator () -> impl Iterator where Item == int = [1, 2, 3].iter()

// Use trait object: multiple types possible, flexibility needed
@any_iterator (flag: bool) -> Iterator where Item == int = {
    if flag then
        [1, 2, 3].iter()
    else
        (1..10).iter()
}
```

### Implementation

- [ ] **Spec**: Compare impl vs dyn
  - [ ] Use cases
  - [ ] Performance implications (static dispatch vs vtable)
  - [ ] When each is appropriate

- [ ] **Documentation**: Best practices guide
  - [ ] Decision flowchart
  - [ ] Common patterns

- [ ] **Test**: `tests/spec/types/impl_vs_dyn.ori`
  - [ ] impl Trait usage (static dispatch)
  - [ ] Trait object usage (dynamic dispatch)
  - [ ] When to choose each
  - [ ] **LLVM/AOT**: Dual-execution parity tests

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [x] Spec updated: `spec/08-types.md` section 8.15 existential types (written, defines E0810/E0811/E0812) (verified 2026-03-29)
- [x] CLAUDE.md updated with impl Trait syntax (ori-syntax.md has "Existential Types" subsection) (verified 2026-03-29)
- [ ] Return position `impl Trait` works
- [ ] Type inference correct
- [ ] Associated type constraints work
- [ ] Clear errors for invalid positions (E0810/E0811/E0812)
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] Fix `tests/spec/types/existential.ori` line 1 wrong spec reference (`06-types.md` -> `08-types.md`) DRIFT
- [ ] Fix `tests/spec/types/existential.ori` commented-out code (replace with `#skip` tests or remove) HYGIENE
- [ ] Fix `tests/spec/types/existential.ori` decorative banners HYGIENE

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
) -> impl Iterator where Item == U = {
    MapIterator { inner: iter, transform: f }
}

@filter<I: Iterator> (
    iter: I,
    predicate: (I.Item) -> bool,
) -> impl Iterator where Item == I.Item = {
    FilterIterator { inner: iter, predicate: predicate }
}

@take<I: Iterator> (
    iter: I,
    n: int,
) -> impl Iterator where Item == I.Item = {
    TakeIterator { inner: iter, remaining: n }
}

// Usage - clean, composable
@first_10_even_squares () -> impl Iterator where Item == int = {
    (1..100)
        .filter(predicate: n -> n % 2 == 0)
        .map(transform: n -> n * n)
        .take(count: 10)
}

// Caller doesn't know concrete type (MapIterator<FilterIterator<...>>)
// but can use it as Iterator
let squares = first_10_even_squares()
for sq in squares do print(msg: `{sq}`)
```
