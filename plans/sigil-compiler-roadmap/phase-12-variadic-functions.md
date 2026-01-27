# Phase 12: Variadic Functions

**Goal**: Enable functions with variable number of arguments

**Criticality**: Medium — API design flexibility, required for C interop

**Dependencies**: Phase 11 (FFI) for C variadic interop

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
~/lang_repos/rust/library/core/src/fmt/mod.rs    # format! handles variadics via macro
```

### Go

```
~/lang_repos/golang/src/go/types/signature.go    # Variadic signature handling
~/lang_repos/golang/src/cmd/compile/internal/types2/signature.go
```

---

## 12.1 Homogeneous Variadics

**Spec section**: `spec/08-declarations.md § Variadic Parameters`

### Syntax

```ori
// Variadic parameter (receives as list)
@sum (numbers: ...int) -> int = run(
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)
)

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

- [ ] **Lexer**: Add `...` token (if not exists)
  - [ ] Three-dot token
  - [ ] Distinguish from range `..`

- [ ] **Parser**: Parse variadic parameters
  - [ ] In function signatures
  - [ ] Spread in call expressions
  - [ ] Validation (last param only)

- [ ] **Type checker**: Variadic type rules
  - [ ] Convert `...T` to `[T]` internally
  - [ ] Check spread type compatibility
  - [ ] Infer element type

- [ ] **Evaluator**: Handle variadic calls
  - [ ] Collect args into list
  - [ ] Handle spread expansion
  - [ ] Mixed literal and spread

- [ ] **Test**: `tests/spec/functions/variadic.ori`
  - [ ] Basic variadic function
  - [ ] With required parameters
  - [ ] Spread operator
  - [ ] Empty variadic call

---

## 12.2 Minimum Argument Count

**Spec section**: `spec/08-declarations.md § Variadic Constraints`

### Syntax

```ori
// Require at least one argument
@max (first: int, rest: ...int) -> int = run(
    rest.fold(initial: first, op: (a, b) -> if a > b then a else b)
)

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

- [ ] **Test**: `tests/spec/functions/variadic_min.ori`
  - [ ] Minimum 1 with required param
  - [ ] Minimum 0 (variadic only)
  - [ ] Error cases

---

## 12.3 Trait Bounds on Variadics

**Spec section**: `spec/08-declarations.md § Variadic Trait Bounds`

### Syntax

```ori
// Generic variadic with trait bound
@print_all<T: Printable> (items: ...T) -> void = run(
    for item in items do print(item.to_str())
)

print_all(1, 2, 3)              // OK: all int
print_all("a", "b", "c")        // OK: all str
print_all(1, "a")               // Error: mixed types

// With explicit heterogeneous (dyn trait)
@print_any (items: ...dyn Printable) -> void = run(
    for item in items do print(item.to_str())
)

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

- [ ] **Test**: `tests/spec/functions/variadic_bounds.ori`
  - [ ] Generic variadic
  - [ ] Trait object variadic
  - [ ] Bound violations

---

## 12.4 C Variadic Interop

**Spec section**: `spec/23-ffi.md § C Variadics`

### Syntax

```ori
// Declare C variadic function
extern "C" {
    @printf (format: *byte, ...) -> c_int  // C-style variadic
}

// Call with any types (unsafe, no type checking)
unsafe {
    printf("Number: %d, String: %s\n".as_c_str(), 42, "hello".as_c_str())
}
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

- [ ] **Parser**: Parse C variadics
  - [ ] `...` without type in extern
  - [ ] Distinguish from Ori variadics

- [ ] **Type checker**: C variadic rules
  - [ ] Must be in extern block
  - [ ] Caller must be unsafe
  - [ ] No type inference

- [ ] **Codegen**: va_list ABI
  - [ ] Platform-specific ABI
  - [ ] Argument passing conventions

- [ ] **Test**: `tests/spec/ffi/c_variadics.ori`
  - [ ] printf call
  - [ ] Mixed argument types
  - [ ] Requires unsafe

---

## 12.5 Variadic in Patterns

**Spec section**: `spec/10-patterns.md § Variadic Patterns`

### Consideration

```ori
// Should variadic work in patterns?
@process_commands (commands: ...(str, int)) -> void = run(
    for (name, priority) in commands do
        print(`Command: {name}, Priority: {priority}`)
)

process_commands(("init", 1), ("run", 2), ("cleanup", 3))
```

### Decision

Defer to future consideration. Current phase focuses on function parameters only.

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/08-declarations.md` variadic section complete
- [ ] CLAUDE.md updated with variadic syntax
- [ ] Homogeneous variadics work
- [ ] Spread operator works
- [ ] C variadic interop works (after Phase 11)
- [ ] All tests pass: `cargo test && ori test tests/spec/functions/`

**Exit Criteria**: Can implement `format()` and call C's `printf()`

---

## Example: Format Function

```ori
// Ori's format function (like Python's format or Rust's format!)
@format (template: str, args: ...dyn Printable) -> str = run(
    let mut result = ""
    let mut arg_index = 0
    let mut i = 0

    loop(run(
        if i >= template.len() then break result

        if template[i] == "{" && i + 1 < template.len() && template[i + 1] == "}" then run(
            if arg_index >= args.len() then
                panic("Not enough arguments for format string")
            result = result + args[arg_index].to_str()
            arg_index = arg_index + 1
            i = i + 2
        )
        else run(
            result = result + template[i]
            i = i + 1
        )
    ))
)

// Usage
let msg = format("{} + {} = {}", 1, 2, 3)  // "1 + 2 = 3"
```
