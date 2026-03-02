---
section: "03"
title: "Bindings and Declarations (§10, §12, §13)"
status: not-started
goal: "Expand §10, §12, §13 with precise binding semantics, predeclared identifiers, and declaration rules"
inspired_by:
  - "Go spec 'Declarations and scope' — predeclared identifiers, exported identifiers, uniqueness"
  - "Go spec 'Constants' — untyped constants, representability"
  - "Go spec 'Variables' — zero values, short declarations"
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Predeclared Identifiers"
    status: not-started
  - id: "03.2"
    title: "Exported Identifiers (pub)"
    status: not-started
  - id: "03.3"
    title: "Uniqueness and Name Conflicts"
    status: not-started
  - id: "03.4"
    title: "Blank Identifier (_)"
    status: not-started
  - id: "03.5"
    title: "Variable Initialization and Zero Values"
    status: not-started
  - id: "03.6"
    title: "Assignment Semantics"
    status: not-started
  - id: "03.7"
    title: "Drop Ordering and Value Lifetime"
    status: not-started
  - id: "03.8"
    title: "Constant Expression Boundaries"
    status: not-started
  - id: "03.9"
    title: "Forward References"
    status: not-started
  - id: "03.10"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Bindings and Declarations (§10, §12, §13)

**Status:** Not Started
**Goal:** These three clauses together should answer every question about how names are introduced, what values they hold initially, when they can be reassigned, and when their values are destroyed. Currently they total ~976 lines but miss several critical topics that Go covers exhaustively.

**Context:** Go's "Declarations and scope" chapter is ~1500 lines and covers predeclared identifiers, exported identifiers, uniqueness, constants with `iota`, type declarations, type parameters, variable declarations, short variable declarations, function declarations, and method declarations — all in one place. Ori splits this across §10-§13 but drops several topics entirely: predeclared identifiers list, uniqueness rules, forward reference rules, and drop ordering.

**Reference implementations:**
- **Go** `ref/spec#Declarations_and_scope`: Comprehensive declaration rules
- **Go** `ref/spec#The_zero_value`: Every type has a well-defined zero value
- **Rust** `reference/src/items.md`: Item declarations, visibility, namespaces

---

## 03.1 Predeclared Identifiers

**File:** `docs/ori_lang/v2026/spec/10-declarations.md` or `docs/ori_lang/v2026/spec/11-blocks-and-scope.md`

Go has a complete list of predeclared identifiers (types, constants, zero value, functions, and the blank identifier). Ori's equivalent is scattered or missing.

- [ ] Add a "Predeclared Identifiers" subsection listing everything in the universe scope:

  **Types:** `int`, `float`, `bool`, `str`, `byte`, `char`, `void`, `Never`, `Duration`, `Size`

  **Compound types:** `Option`, `Result`, `Error`, `Range`, `Set`, `Ordering`

  **Prelude types:** `TraceEntry`, `PanicInfo`, `FormatSpec`, `Alignment`, `Sign`, `FormatType`, `CancellationError`, `CancellationReason`

  **Traits:** `Eq`, `Comparable`, `Hashable`, `Printable`, `Formattable`, `Debug`, `Clone`, `Default`, `Drop`, `Len`, `IsEmpty`, `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `Into`, `Traceable`, `Index`, `Sendable`

  **Operator traits:** `Add`, `Sub`, `Mul`, `Div`, `FloorDiv`, `Rem`, `Pow`, `MatMul`, `Neg`, `Not`, `BitNot`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `As`, `TryAs`

  **Constants/Constructors:** `Some`, `None`, `Ok`, `Err`, `true`, `false`, `Less`, `Equal`, `Greater`

  **Functions:** `print`, `panic`, `todo`, `unreachable`, `dbg`, `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`, `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `compare`, `min`, `max`, `hash_combine`, `repeat`, `is_cancelled`, `drop_early`, `compile_error`, `embed`, `has_embed`

- [ ] State: predeclared identifiers can be shadowed (unlike keywords)
- [ ] State: predeclared identifiers are in the _universe scope_ (outermost, enclosing all modules)

---

## 03.2 Exported Identifiers (pub)

**File:** `docs/ori_lang/v2026/spec/10-declarations.md`

- [ ] Formalize `pub` rules:
  - `pub` applies to: functions, types, constants, traits, default impls, extensions
  - `pub` does NOT apply to: local bindings, parameters, loop variables
  - `pub type` exports the type name and its constructors
  - `pub type` with private fields: fields accessible within defining module only
  - Decision needed: Can individual fields be `pub`? Or is it all-or-nothing?
- [ ] Re-export rules: `pub use` — what exactly becomes visible?
- [ ] Trait method visibility: trait methods inherit the trait's visibility

---

## 03.3 Uniqueness and Name Conflicts

**File:** `docs/ori_lang/v2026/spec/10-declarations.md`

- [ ] Define when two declarations conflict:
  - Same name in same scope = error (applies to all declaration kinds)
  - Exception: function clauses (same name, same scope = multi-clause pattern matching)
  - Exception: impl blocks (multiple `impl` blocks for same type OK)
  - Exception: trait impls for different traits on same type OK
- [ ] State: overloading by parameter types is NOT supported (no ad-hoc overloading)
- [ ] State: a type and a function with the same name in the same scope is an error

---

## 03.4 Blank Identifier (_)

**File:** `docs/ori_lang/v2026/spec/13-variables.md` or `docs/ori_lang/v2026/spec/15-patterns.md`

- [ ] Formalize `_` as the blank identifier / wildcard:
  - In patterns: matches any value, does not create a binding
  - In `let _ = expr;`: evaluates `expr`, discards the result (but runs side effects and Drop)
  - Cannot be used as a regular identifier in non-pattern contexts
  - Multiple `_` in same pattern OK (unlike named bindings)
- [ ] Cross-reference to §15 Patterns for pattern-specific rules

---

## 03.5 Variable Initialization and Zero Values

**File:** `docs/ori_lang/v2026/spec/13-variables.md`

Go's "The zero value" section is a first-class concept. Ori needs the same.

- [ ] State: every `let` binding shall have an initializer expression
  - `let x: int;` without initializer — is this legal? Decision needed
  - Recommendation: require initializers always (unlike Go's zero-value init)
  - If required: `let x: int = int.default();` or `let x = 0;`
- [ ] If Ori allows uninitialized variables, define zero values per type (table already in §9.5)
- [ ] State: function parameters are always initialized by the caller
- [ ] State: struct fields with defaults — when are defaults applied?

---

## 03.6 Assignment Semantics

**File:** `docs/ori_lang/v2026/spec/13-variables.md`

- [ ] Formalize: assignment is value copy (Ori has value semantics)
  - `x = y` copies the value of `y` into `x`
  - ARC manages the reference count behind the scenes
  - No aliasing between `x` and `y` after assignment (COW may defer physical copy)
- [ ] State: assignment to immutable binding (`$`) is a compile-time error
- [ ] State: assignment to function parameter is a compile-time error
- [ ] State: assignment to loop variable is a compile-time error
- [ ] Cross-reference to §14.15.7 compound assignment desugaring
- [ ] Field assignment desugaring: `x.field = v` → `x = { ...x, field: v }` (already in §14 but state clearly here)
- [ ] Index assignment desugaring: `x[i] = v` → `x = x.updated(key: i, value: v)` (already in §14)

---

## 03.7 Drop Ordering and Value Lifetime

**File:** `docs/ori_lang/v2026/spec/13-variables.md` or `docs/ori_lang/v2026/spec/21-memory-model.md`

- [ ] State: values are dropped when their binding goes out of scope
- [ ] Drop order within a block: reverse declaration order (like Rust)
  ```ori
  {
      let a = acquire_a();  // dropped third
      let b = acquire_b();  // dropped second
      let c = acquire_c();  // dropped first
      result
  }
  ```
- [ ] Drop order for function parameters: implementation-defined (after body returns)
- [ ] `drop_early(value:)` — explicitly drops before end of scope
- [ ] Cross-reference to §21 Memory Model for ARC details
- [ ] Cross-reference to Drop trait in §9

---

## 03.8 Constant Expression Boundaries

**File:** `docs/ori_lang/v2026/spec/12-constants.md`

Currently §12 cross-references §24 for constant expression rules. Need to consolidate.

- [ ] Clarify the boundary: what makes an expression "constant"?
  - Literals: always constant
  - Arithmetic on constants: constant
  - String concatenation of constants: constant
  - `$`-prefixed function calls where function is const and all args are const: constant
  - Everything else: not constant
- [ ] State: constant expressions are evaluated at compile time
- [ ] State: overflow in constant expressions is a compile-time error (not a runtime panic)
- [ ] Cross-reference to §24 for evaluation limits

---

## 03.9 Forward References

**File:** `docs/ori_lang/v2026/spec/10-declarations.md`

- [ ] State: top-level declarations are visible throughout the entire module (order-independent)
  - Function A can call function B even if B is defined later in the file
  - Type A can reference type B defined later
  - Mutual recursion between top-level functions is allowed
- [ ] State: local bindings are NOT order-independent (no forward references)
  - `let a = b; let b = 1;` is an error — `b` not yet declared
- [ ] State: recursive types — a struct field can reference its own type through `Option`, `[T]`, etc.
  - Direct recursive field is an error (infinite size)

---

## 03.10 Completion Checklist

- [ ] Predeclared identifiers list is complete and matches compiler prelude
- [ ] `pub` rules formalized with all declaration kinds
- [ ] Name conflict rules cover all edge cases
- [ ] `_` blank identifier fully specified
- [ ] Variable initialization rules clear (initializer required or zero value?)
- [ ] Assignment = value copy explicitly stated
- [ ] Drop ordering formalized
- [ ] Constant/non-constant boundary precisely defined
- [ ] Forward reference rules stated
- [ ] All additions use ISO normative style

**Exit Criteria:** A reader can determine from §10-§13 alone: what identifiers exist by default, when names conflict, whether a binding needs an initializer, what happens on assignment, and when values are destroyed.
