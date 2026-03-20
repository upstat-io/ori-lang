---
section: 12
title: Variadic Functions
status: in-progress
reviewed: false
tier: 4
goal: Enable functions with variable number of arguments
spec:
  - spec/10-declarations.md
  - spec/26-ffi.md
sections:
  - id: "12.1"
    title: Homogeneous Variadics
    status: in-progress
  - id: "12.2"
    title: Minimum Argument Count
    status: not-started
  - id: "12.3"
    title: Trait Bounds on Variadics
    status: not-started
  - id: "12.4"
    title: C Variadic Interop
    status: in-progress
  - id: "12.5"
    title: Variadic in Patterns
    status: not-started
---

# Section 12: Variadic Functions

**Goal**: Enable functions with variable number of arguments

**Criticality**: Medium — API design flexibility, required for C interop

**Dependencies**: Section 11 (FFI — C variadic interop in 12.4), Section 3 (Traits — trait bounds on variadics in 12.3)

**Proposal**: `proposals/approved/variadic-functions-proposal.md`

### Sync Points: Variadic Parameter Type (multi-crate sync required)

Adding variadic parameter support requires updates across these crates:

1. **`ori_ir`** — [done] `is_variadic: bool` on `Param`, `is_c_variadic: bool` on `ExternItem`, `is_spread: bool` on `CallArg`
2. **`ori_parse`** — [done] Parse `...T` in parameter position, `...expr` spread in call position, `...` in extern blocks
3. **`ori_types`** — Convert `...T` to `[T]` internally, validate spread type compatibility, enforce "last param only" rule
4. **`ori_eval`** — Collect variadic args into list at call site, expand spread operator
5. **`ori_llvm`** — Codegen for variadic arg collection (stack allocation or heap list), C variadic ABI (`va_list`) for extern functions

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Type safety | Homogeneous only | Type safety over flexibility |
| Syntax | `...T` parameter | Clear, matches common convention |
| C variadics | Separate syntax | Different semantics require distinction |
| Minimum args | Configurable | Allow `printf`-style (min 1) or `sum`-style (min 0) |

---

## Reference Implementation

### Rust

Rust doesn't have variadic functions in the language; uses macros instead.
```
~/projects/reference_repos/lang_repos/rust/library/core/src/fmt/mod.rs    # format! handles variadics via macro
```

### Go

```
~/projects/reference_repos/lang_repos/golang/src/go/types/signature.go    # Variadic signature handling
~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/types2/signature.go
```

### Swift

```
# Swift has variadic generics (parameter packs: each T, repeat each T)
~/projects/reference_repos/lang_repos/swift/lib/Sema/CSSimplify.cpp       # Pack expansion type checking
~/projects/reference_repos/lang_repos/swift/include/swift/AST/Types.h     # PackExpansionType
```

---

## 12.1 Homogeneous Variadics

**Spec section**: `spec/10-declarations.md § Variadic Parameters`

### Syntax

```ori
// Variadic parameter (receives as list)
@sum (numbers: ...int) -> int = {
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)
}

// Usage
sum(1, 2, 3)        // 6
sum(1)              // 1
sum()               // 0

// With required parameters before
@printf (format: str, args: ...Printable) -> str = ...

printf("Hello")                    // "Hello"
printf("{} + {} = {}", 1, 2, 3)   // "1 + 2 = 3"

// Spread operator to pass list as varargs
let nums = [1, 2, 3]
sum(...nums)        // 6
sum(0, ...nums, 4)  // 10
```

### Grammar

```ebnf
Parameter        = Identifier ':' Type | VariadicParameter ;
VariadicParameter = Identifier ':' '...' Type ;
SpreadExpr       = '...' Expression ;
```

### Type Rules

- Variadic parameter must be last
- Only one variadic parameter allowed
- Parameter type `...T` becomes `[T]` inside function
- Spread `...list` requires `list: [T]` where `T` matches variadic

### Implementation

- [ ] **Spec**: Add variadic parameter syntax
  - [ ] Parameter syntax `...T`
  - [ ] Spread operator `...expr`
  - [ ] Type rules

- [x] **Lexer**: `...` token implemented (`DotDotDot = 58` in raw scanner, `DotDotDot` tag 92 in token kind)
  - [x] Three-dot token (distinguished from `DotDot` and `DotDotEq`)
  - [x] Distinguish from range `..`

- [x] **Parser**: Parse variadic parameters (tests pass: `ori_parse::tests::compositional` — 76 passed)
  - [x] In function signatures (`is_variadic: true` on `Param`)
  - [x] Spread in call expressions (`is_spread: true` on `CallArg`)
  - [ ] Validation (last param only — may be deferred to type checker)

- [ ] **Type checker**: Variadic type rules
  - [ ] Convert `...T` to `[T]` internally
  - [ ] Check spread type compatibility
  - [ ] Infer element type

- [ ] **Evaluator**: Handle variadic calls
  - [ ] Collect args into list
  - [ ] Handle spread expansion
  - [ ] Mixed literal and spread

- [ ] **LLVM Support**: LLVM codegen for homogeneous variadics
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — homogeneous variadics codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic.ori`
  - [ ] Basic variadic function
  - [ ] With required parameters
  - [ ] Spread operator
  - [ ] Empty variadic call

---

## 12.2 Minimum Argument Count

**Spec section**: `spec/10-declarations.md § Variadic Constraints`

### Syntax

```ori
// Require at least one argument
@max (first: int, rest: ...int) -> int = {
    rest.fold(initial: first, op: (a, b) -> if a > b then a else b)
}

max(1, 2, 3)   // 3
max(5)         // 5
max()          // Error: max requires at least 1 argument

// Explicit minimum (alternative syntax consideration)
@format (fmt: str, args: ...Printable) -> str = ...
// Minimum 1 (fmt) is implicit from required param
```

### Implementation

- [ ] **Spec**: Minimum argument rules
  - [ ] Required params before variadic
  - [ ] Error messages

- [ ] **Type checker**: Validate minimum args
  - [ ] Count required params
  - [ ] Error if insufficient

- [ ] **Diagnostics**: Clear error messages
  - [ ] "expected at least N arguments, got M"
  - [ ] Show required vs optional

- [ ] **LLVM Support**: LLVM codegen for minimum argument validation
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — minimum argument count codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic_min.ori`
  - [ ] Minimum 1 with required param
  - [ ] Minimum 0 (variadic only)
  - [ ] Error cases

---

## 12.3 Trait Bounds on Variadics

**Spec section**: `spec/10-declarations.md § Variadic Trait Bounds`

### Syntax

```ori
// Generic variadic with trait bound
@print_all<T: Printable> (items: ...T) -> void = {
    for item in items do print(item.to_str())
}

print_all(1, 2, 3)              // OK: all int
print_all("a", "b", "c")        // OK: all str
print_all(1, "a")               // Error: mixed types

// With explicit heterogeneous (trait object)
@print_any (items: ...Printable) -> void = {
    for item in items do print(item.to_str())
}

print_any(1, "hello", true)     // OK: all Printable
```

### Implementation

- [ ] **Spec**: Trait bounds on variadics
  - [ ] Homogeneous generics
  - [ ] Trait object variadics
  - [ ] Error messages

- [ ] **Type checker**: Bound validation
  - [ ] All args satisfy bound
  - [ ] Infer common type
  - [ ] Trait object boxing

- [ ] **LLVM Support**: LLVM codegen for trait bounds on variadics
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — variadic trait bounds codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic_bounds.ori`
  - [ ] Generic variadic
  - [ ] Trait object variadic
  - [ ] Bound violations

---

## 12.4 C Variadic Interop

**Spec section**: `spec/26-ffi.md § C Variadics`

### Syntax

```ori
// Declare C variadic function
extern "C" {
    @printf (format: *byte, ...) -> c_int  // C-style variadic
}

// Call with any types (unsafe, no type checking)
unsafe { printf("Number: %d, String: %s\n".as_c_str(), 42, "hello".as_c_str()) }
```

### Distinction from Ori Variadics

| Feature | Ori `...T` | C `...` |
|---------|--------------|---------|
| Type safety | Homogeneous, checked | Unchecked |
| Context | Safe code | Unsafe only |
| Implementation | List | va_list ABI |

### Implementation

- [ ] **Spec**: C variadic syntax
  - [ ] Extern function with `...`
  - [ ] No type after `...`
  - [ ] Unsafe requirement

- [x] **Parser**: Parse C variadics (2 dedicated tests in `compiler/oric/tests/phases/parse/extern_def.rs`, formatter handles `is_c_variadic`)
  - [x] `...` without type in extern (`parse_extern_params()` in `ori_parse/src/grammar/item/extern_def.rs`)
  - [x] Distinguish from Ori variadics (separate `ExternParam`/`ExternItem` types vs `Param`)

- [ ] **Type checker**: C variadic rules
  - [ ] Must be in extern block
  - [ ] Caller must be unsafe
  - [ ] No type inference

- [ ] **Codegen**: va_list ABI
  - [ ] Platform-specific ABI
  - [ ] Argument passing conventions

- [ ] **LLVM Support**: LLVM codegen for C variadic interop
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — C variadic interop codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/c_variadics.ori`
  - [ ] printf call
  - [ ] Mixed argument types
  - [ ] Requires unsafe

---

## 12.5 Variadic in Patterns

**Spec section**: `spec/15-patterns.md § Variadic Patterns`

### Consideration

```ori
// Should variadic work in patterns?
@process_commands (commands: ...(str, int)) -> void = {
    for (name, priority) in commands do
        print(`Command: {name}, Priority: {priority}`)
}

process_commands(("init", 1), ("run", 2), ("cleanup", 3))
```

### Decision

Defer to future consideration. Current section focuses on function parameters only.

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Spec updated: `spec/10-declarations.md` variadic section complete
- [ ] CLAUDE.md updated with variadic syntax
- [ ] Homogeneous variadics work
- [ ] Spread operator works
- [ ] C variadic interop works (after Section 11)
- [ ] All tests pass: `./test-all.sh`

**Exit Criteria**: Can implement `format()` and call C's `printf()`

---

## Example: Format Function

```ori
// Ori's format function (like Python's format or Rust's format!)
@format (template: str, args: ...Printable) -> str = {
    let mut result = ""
    let mut arg_index = 0
    let mut i = 0

    loop {
        if i >= template.len() then break result

        if template[i] == "{" && i + 1 < template.len() && template[i + 1] == "}" then {
            if arg_index >= args.len() then
                panic("Not enough arguments for format string")
            result = result + args[arg_index].to_str()
            arg_index = arg_index + 1
            i = i + 2
        }
        else {
            result = result + template[i]
            i = i + 1
        }
    }
}

// Usage
let msg = format("{} + {} = {}", 1, 2, 3)  // "1 + 2 = 3"
```
