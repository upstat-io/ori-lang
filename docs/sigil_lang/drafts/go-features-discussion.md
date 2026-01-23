# Go Language Features - Discussion for Sigil

**Date**: 2026-01-22
**Status**: In Discussion

This document lists Go language features to discuss interactively for potential Sigil adoption. Each feature will be discussed one by one - no pre-made decisions.

---

## Features To Discuss

### 1. Goroutines (Lightweight Concurrency Primitives)
**Go**: `go func()` spawns a lightweight green thread managed by the runtime
**What it does**: Thousands of goroutines can run concurrently with minimal memory overhead (~2KB stack)
**Discussion status**: Rejected
**Decision**: Sigil uses structured concurrency via `parallel` pattern. Fire-and-forget spawning rejected - orphan tasks are impossible by design. All concurrent work must complete before scope exits.

---

### 2. Channels (Communication Between Concurrent Units)
**Go**: `ch := make(chan int)`, `ch <- value`, `value := <-ch`
**What it does**: Type-safe communication pipes between goroutines; can be buffered or unbuffered
**Discussion status**: Already in proposal
**Decision**: See parallel-concurrency-proposal.md for role-based channel design (Producer/Consumer split, Sendable constraint, ownership transfer).

---

### 3. Select Statement (Multi-way Channel Operations)
**Go**: `select { case <-ch1: ... case ch2 <- v: ... default: ... }`
**What it does**: Wait on multiple channel operations, proceed with first ready one
**Discussion status**: Already in proposal
**Decision**: See parallel-concurrency-proposal.md for enhanced `select` pattern with priority ordering and timeout support.

---

### 4. Defer (Deferred Function Execution)
**Go**: `defer file.Close()` - executes when surrounding function returns
**What it does**: Guarantees cleanup regardless of how function exits (return, panic)
**Discussion status**: Rejected
**Decision**: Sigil uses structured `with` pattern for resource management. More explicit acquire/use/release structure preferred over implicit defer stack.

---

### 5. Panic/Recover (Controlled Crashes and Recovery)
**Go**: `panic("error")` crashes up stack; `recover()` in defer catches it
**What it does**: Mechanism for unrecoverable errors with optional recovery
**Discussion status**: Accepted (modified)
**Decision**: Reject Go-style `recover()`. Instead, add app-wide `@panic` handler - a top-level function that runs before crash (for logging, error reporting, cleanup). Program still terminates; no local recovery. **Draft proposal needed.**

---

### 6. Multiple Return Values
**Go**: `func divide(a, b int) (int, error)`
**What it does**: Functions return multiple values directly; common pattern: `result, err`
**Discussion status**: Already have
**Decision**: Sigil uses tuples `(T, U)` with destructuring `let (a, b) = f()`. Equivalent functionality.

---

### 7. Named Return Values
**Go**: `func f() (result int, err error) { result = 42; return }`
**What it does**: Return values have names, initialized to zero values; naked return uses them
**Discussion status**: Rejected
**Decision**: Sigil is expression-based. Naked returns obscure what's being returned. No benefit for added complexity.

---

### 8. Blank Identifier (`_`)
**Go**: `_, err := doSomething()` - discards first return value
**What it does**: Explicitly ignore values; compiler enforces no unused variables
**Discussion status**: Already have
**Decision**: Sigil has `_` wildcard in patterns: `let (_, b) = tuple`, `match(x, _ -> default)`.

---

### 9. Iota (Auto-incrementing Constants)
**Go**: `const ( A = iota; B; C )` produces 0, 1, 2
**What it does**: Auto-incrementing integer constants; can use in expressions like `1 << iota`
**Discussion status**: Rejected
**Decision**: Sigil values explicitness. Use sum types for enums, explicit `$` constants for numeric values. No magic auto-increment.

---

### 10. Implicit Interface Implementation (Structural Typing)
**Go**: Types implement interfaces by having matching methods, no explicit declaration
**What it does**: Decouples interface definition from implementation; enables ad-hoc polymorphism
**Discussion status**: Rejected
**Decision**: Sigil uses explicit `impl Trait for Type` blocks. More discoverable, searchable, and clear intent. No accidental implementations.

---

### 11. Embedding (Composition Over Inheritance)
**Go**: `type Reader struct { io.Reader }` - embeds Reader's methods
**What it does**: Automatic method forwarding; "has-a" that acts like "is-a"
**Discussion status**: Rejected
**Decision**: Sigil only allows explicit composition. No automatic method promotion. Delegation must be explicit via `impl` blocks.

---

### 12. Type Assertions
**Go**: `value := i.(ConcreteType)` or `value, ok := i.(ConcreteType)`
**What it does**: Extract concrete type from interface; two-value form avoids panic
**Discussion status**: Rejected
**Decision**: Violates explicitness. Runtime type checking is a code smell. Use sum types with explicit variants instead of `dyn Trait` + downcasting.

---

### 13. Type Switches
**Go**: `switch v := i.(type) { case int: ... case string: ... }`
**What it does**: Branch based on dynamic type of interface value
**Discussion status**: Rejected
**Decision**: Same as #12. Runtime type inspection violates explicitness. Use sum types with `match` for compile-time exhaustive variant handling.

---

### 14. Slices (Dynamic Arrays with Capacity)
**Go**: `[]int` - reference to underlying array with length and capacity
**What it does**: Dynamic arrays that share underlying storage; efficient subslicing
**Discussion status**: Already in proposal
**Decision**: See fixed-capacity-list-proposal.md for `[T, max N]` syntax. Sigil uses value semantics, not shared underlying storage.

---

### 15. Maps with Zero-Value Access
**Go**: `m["key"]` returns zero value if key missing; `v, ok := m["key"]` for presence check
**What it does**: Missing keys return type's zero value instead of error
**Discussion status**: Skipped
**Decision**: Keep Sigil's current design: `map[key]` returns `Option<V>`. More explicit than zero values.

---

### 16. Init Functions (Package Initialization)
**Go**: `func init() { ... }` runs automatically before main
**What it does**: Per-package initialization code; multiple init functions allowed per file
**Discussion status**: Rejected
**Decision**: Hidden execution order violates explicitness. Use `$` config constants or explicit initialization in `@main`.

---

### 17. Visibility by Naming Convention
**Go**: `PublicFunc` (uppercase) is exported; `privateFunc` (lowercase) is package-private
**What it does**: No keywords for visibility; case determines export status
**Discussion status**: Rejected
**Decision**: Sigil uses explicit `pub` keyword. More searchable, clearer intent, no magic naming rules.

---

### 18. No Circular Imports (Enforced)
**Go**: Compiler rejects import cycles
**What it does**: Forces clean dependency graphs; improves build times and reasoning
**Discussion status**: Accepted
**Decision**: Sigil compiler must reject circular imports. Forces clean architecture, faster compilation, easier reasoning.

---

### 19. Error Wrapping (`%w` and errors.Is/As)
**Go**: `fmt.Errorf("context: %w", err)`, `errors.Is(err, target)`, `errors.As(err, &target)`
**What it does**: Chain errors with context; inspect error chains without type assertions
**Discussion status**: Rejected
**Decision**: Use explicit error types with sum types. Match on error variants directly. **Note:** This discussion revealed Sigil lacks string interpolation - needs separate proposal.

---

### 20. Context Package (Cancellation/Deadline Propagation)
**Go**: `ctx, cancel := context.WithTimeout(parent, 5*time.Second)`
**What it does**: Propagate cancellation, deadlines, and request-scoped values through call chains
**Discussion status**: Already in proposal
**Decision**: See parallel-concurrency-proposal.md for CancellationToken and `timeout` pattern.

---

### 21. Pointer Receivers vs Value Receivers
**Go**: `func (p *Point) Move()` vs `func (p Point) Distance()`
**What it does**: Choose whether methods mutate receiver or work on copy
**Discussion status**: Rejected
**Decision**: No mutable `self` in methods. All methods receive immutable self and return new values. Simplifies ARC memory management and reasoning.

---

### 22. No Pointer Arithmetic
**Go**: Pointers exist but no `ptr++` or pointer math
**What it does**: Pointers for indirection only; prevents memory corruption bugs
**Discussion status**: N/A
**Decision**: Sigil has no raw pointers. Not a low-level language. Memory managed by ARC.

---

### 23. Zero Values (Default Initialization)
**Go**: All types have a zero value (0, "", nil, empty struct, etc.)
**What it does**: No uninitialized variables; everything starts at predictable state
**Discussion status**: Rejected
**Decision**: Sigil requires explicit initialization. Forgetting to initialize is a bug the compiler should catch. Zero might not be the right default.

---

### 24. Range-Based For Loops
**Go**: `for i, v := range slice { ... }`, `for k, v := range map { ... }`
**What it does**: Iterate over collections with index/key and value
**Discussion status**: Already have
**Decision**: Sigil has `for item in items do`, `for (i, item) in items.enumerate() do`, `for (k, v) in map do`.

---

### 25. Variadic Functions
**Go**: `func sum(nums ...int) int` - accepts any number of int arguments
**What it does**: Variable number of arguments of same type; received as slice
**Discussion status**: Rejected
**Decision**: Use explicit lists. `@sum (nums: [int])` called as `sum([1, 2, 3])`. Clearer, no magic.

---

### 26. Build Tags and Conditional Compilation
**Go**: `// +build linux` at file top
**What it does**: Include/exclude files from build based on OS, architecture, custom tags
**Discussion status**: Accepted
**Decision**: Support conditional compilation. Likely attribute-based: `#[target(os: "linux")]` or similar. **Draft proposal needed.**

---

### 27. Generics with Type Parameters (Go 1.18+)
**Go**: `func Map[T, U any](s []T, f func(T) U) []U`
**What it does**: Type-safe generic functions and types; constraints via interfaces
**Discussion status**: Already have
**Decision**: Sigil has `@name<T, U>` generics with trait bounds `T: Trait` and `where` clauses.

---

### 28. Comparable/Ordered Constraints
**Go**: `func Max[T constraints.Ordered](a, b T) T`
**What it does**: Built-in constraints for types supporting comparison operators
**Discussion status**: Pending

---

## Features Already in Sigil (For Reference)

These Go features have equivalents already in Sigil:
- **Generics** - Sigil has `@name<T>`, `T: Trait` bounds
- **No inheritance** - Sigil uses traits and composition
- **No null** - Sigil has `Option<T>` and `Result<T, E>`
- **Strong typing** - Both are statically typed
- **First-class functions** - Both support lambdas and higher-order functions
- **Struct types** - Sigil has `type Name = { field: Type }`
- **Pattern matching** - Sigil has more powerful `match`
- **Explicit visibility** - Sigil uses `pub` keyword

---

## Discussion Progress

| # | Feature | Discussed | Decision |
|---|---------|-----------|----------|
| 1 | Goroutines | Yes | Rejected |
| 2 | Channels | Yes | In proposal |
| 3 | Select Statement | Yes | In proposal |
| 4 | Defer | Yes | Rejected |
| 5 | Panic/Recover | Yes | Accepted (modified) |
| 6 | Multiple Return Values | Yes | Already have |
| 7 | Named Return Values | Yes | Rejected |
| 8 | Blank Identifier | Yes | Already have |
| 9 | Iota | Yes | Rejected |
| 10 | Implicit Interfaces | Yes | Rejected |
| 11 | Embedding | Yes | Rejected |
| 12 | Type Assertions | Yes | Rejected |
| 13 | Type Switches | Yes | Rejected |
| 14 | Slices | Yes | In proposal |
| 15 | Maps Zero-Value | Yes | Skipped |
| 16 | Init Functions | Yes | Rejected |
| 17 | Visibility by Naming | Yes | Rejected |
| 18 | No Circular Imports | Yes | Accepted |
| 19 | Error Wrapping | Yes | Rejected |
| 20 | Context Package | Yes | In proposal |
| 21 | Pointer/Value Receivers | Yes | Rejected |
| 22 | No Pointer Arithmetic | Yes | N/A |
| 23 | Zero Values | Yes | Rejected |
| 24 | Range-Based For | Yes | Already have |
| 25 | Variadic Functions | Yes | Rejected |
| 26 | Build Tags | Yes | Accepted |
| 27 | Type Parameters | Yes | Already have |
| 28 | Comparable/Ordered | No | - |

---

## Sources

- [Go at Google: Language Design](https://go.dev/talks/2012/splash.article)
- [Effective Go](https://go.dev/doc/effective_go)
- [Go FAQ](https://go.dev/doc/faq)
- [Go Language Specification](https://go.dev/ref/spec)
- [Working with Errors in Go 1.13](https://go.dev/blog/go1.13-errors)
