# Language Design Principles

Quick-reference guide to core language design philosophy and patterns.

---

## Design Philosophy

### Explicitness
- Make all behavior visible in source code - no hidden control flow
- Prefer explicit over implicit conversions, casts, and coercions
- Surface all side effects - allocations, I/O, errors should be visible
- Avoid "magic" - if code doesn't look like it calls a function, it shouldn't
- Make dependencies explicit through imports/requires

### Consistency
- One way to do common things (Zen of Python: "one obvious way")
- Uniform syntax patterns - similar things look similar
- Predictable behavior across contexts
- Orthogonal features that compose without surprises

### Minimalism
- Every feature must justify its complexity cost
- Prefer removing features to adding workarounds
- Small core + libraries beats large core
- Fewer keywords = easier to learn and parse
- "When in doubt, leave it out" (Go philosophy)

### Pragmatism
- Prioritize real-world use over theoretical purity
- Fast compilation often more valuable than micro-optimizations
- Readable error messages over clever implementations
- Support tooling (formatters, linters, LSP) from day one

---

## Syntax Design Patterns

### Sigils & Prefixes
- `@` for function definitions (Sigil) or decorators/attributes (Python, Java)
- `$` for variables (PHP, shell) or configuration (Sigil)
- `#` for preprocessor/macros (C), comments, or length in context (Sigil)
- `_` for unused/private (Python, Rust, Go)
- `!` for macros (Rust) or unwrap operations
- `?` for optionals (Swift, Kotlin) or error propagation (Rust, Sigil)

### Keywords vs Symbols
- Keywords for control flow: `if`, `for`, `while`, `match`, `return`
- Symbols for operators: `+`, `-`, `*`, `/`, `==`, `!=`
- Keywords more readable; symbols more compact
- Avoid symbol overload (C++ `<<` for streams vs bit-shift)

### Operator Design
- Familiar math operators: `+`, `-`, `*`, `/`, `%`
- Comparison: `==` for equality, `=` for assignment (avoid C's `=` vs `==` trap)
- Logical: `&&`, `||`, `!` (short-circuit by default)
- Assignment variants: `+=`, `-=`, etc. as syntactic sugar
- Consider: `..` for ranges, `=>` for arrows, `::` for paths

### Delimiters
- Braces `{}` for blocks (C-family)
- Indentation for blocks (Python, Haskell)
- `do`/`end` for blocks (Ruby, Elixir)
- Parentheses for grouping and function calls
- Square brackets for indexing and arrays
- Angle brackets for generics (but parsing complications)

### Context-Sensitive Keywords
- Allow keywords as identifiers in non-conflicting contexts
- Examples: `map`, `filter`, `fold` usable as variable names (pattern names only)
- Reduces reserved word count
- Requires careful grammar design

---

## Semantic Design

### Expression-Based vs Statement-Based

**Expression-based (preferred for modern languages):**
- Everything returns a value
- `if`/`else` returns a value, no ternary needed
- Blocks return last expression
- Easier to compose and chain
- Encourages functional patterns
- Examples: Rust, Kotlin, Scala, Ruby, F#

**Statement-based:**
- Statements perform actions, don't return values
- Requires explicit `return` statements
- Ternary operator needed for conditional expressions
- More familiar to C/Java programmers
- Examples: C, Java, Python (mostly)

### Declarations

**Explicit type annotations:**
```
let x: int = 5
```
- Self-documenting
- Easier to parse and implement
- Better error messages

**Type inference:**
```
let x = 5  // inferred as int
```
- Less boilerplate
- Requires more sophisticated type checker
- Can hurt readability in large codebases

**Bidirectional (best of both):**
- Infer where obvious, require where ambiguous
- Allow optional annotations anywhere
- Push expected types down, pull inferred types up

### Mutability
- Default immutable (Rust `let` vs `let mut`)
- Explicit mutation markers
- Immutable by default catches more bugs
- Consider: immutable data structures in stdlib

---

## Error Handling Philosophy

### Exceptions (Traditional)
- Pros: Clean happy path, existing ecosystem
- Cons: Hidden control flow, easy to forget handling
- Can't see throws in function signature (without annotations)
- Examples: Java, Python, C++, JavaScript

### Result/Either Types (Modern)
- Pros: Errors in type system, must be handled
- Cons: More verbose, monadic chaining needed
- `Result<T, E>` - success or error
- `Option<T>` / `Maybe T` - value or nothing
- Propagation operator: `?` (Rust), `try` (Zig)
- Examples: Rust, Haskell, Zig, Swift

### Error Codes (Traditional)
- Return special values (null, -1, errno)
- Pros: Simple, no overhead
- Cons: Easy to ignore, mixed with normal values
- Examples: C, Go (with multi-return)

### Multi-Value Returns (Go-style)
```go
value, err := doSomething()
if err != nil { ... }
```
- Pros: Explicit, no hidden control flow
- Cons: Verbose, can still ignore `err`
- Lint tools to catch unchecked errors

### Design Recommendations
- Pick one approach and use consistently
- Make errors visible in types/signatures
- Provide ergonomic propagation (`?`, `try`)
- Distinguish recoverable vs unrecoverable (panic)
- Errors are values, not control flow

---

## Memory Management Strategies

### Garbage Collection (GC)
- **Tracing GC**: Mark-and-sweep, generational
- Pros: Simple for programmer, handles cycles
- Cons: Pause times, memory overhead, unpredictable
- Examples: Java, Go, JavaScript, Python
- Modern GCs: concurrent, low-pause (<1ms Go)

### Reference Counting (RC)
- Increment/decrement counts, free at zero
- Pros: Deterministic destruction, incremental
- Cons: Cycle leaks (need cycle collector), overhead
- Examples: Python (+ cycle collector), Swift (ARC)
- ARC (Automatic RC): Compile-time inserted retain/release

### Ownership/Borrowing (Rust-style)
- One owner, borrowers have references
- Pros: No GC, compile-time safety, zero overhead
- Cons: Learning curve, complex lifetimes
- Borrow checker ensures safety at compile time
- Examples: Rust, emerging in others

### Manual Management
- Explicit `malloc`/`free`, `new`/`delete`
- Pros: Full control, zero overhead
- Cons: Use-after-free, memory leaks, double-free
- RAII pattern helps (C++ destructors)
- Examples: C, C++ (with smart pointers)

### Arena/Region Allocation
- Allocate from arena, free all at once
- Pros: Fast allocation, bulk deallocation
- Cons: Can't free individual objects
- Good for: parsers, request handlers, games
- Examples: Zig allocator parameter, custom allocators

### Selection Guide
| Strategy | Best For |
|----------|----------|
| GC | Apps, scripts, rapid development |
| RC/ARC | Mobile apps, deterministic cleanup |
| Ownership | Systems, performance-critical |
| Manual | Legacy, extreme control |
| Arena | Compilers, games, servers |

---

## Modern Design Trends

### Null Safety
- Null is "billion-dollar mistake" (Tony Hoare)
- Eliminate null at type level
- `Option<T>` = `Some(value)` or `None`
- Non-nullable by default, `T?` for nullable
- Flow analysis for type promotion
- Required checks before use
- Examples: Kotlin, Swift, Dart, Rust

### Algebraic Data Types (ADTs)
- **Sum types**: One of several variants (enum/union)
  ```rust
  enum Result<T, E> { Ok(T), Err(E) }
  ```
- **Product types**: All of several fields (struct/tuple)
  ```rust
  struct Point { x: f64, y: f64 }
  ```
- Pattern matching on sum types
- Make illegal states unrepresentable
- Examples: Rust, Haskell, ML, Scala, Swift, Kotlin

### Immutability-First
- Default immutable variables
- Immutable collections in stdlib
- Explicit `mut`, `var`, or `mutable` for mutation
- Enables: concurrency safety, easier reasoning
- Examples: Rust, Kotlin, Scala

### Pattern Matching
- Destructure and match in one construct
- Exhaustiveness checking by compiler
- Guards for conditions
- Examples: Rust `match`, Haskell `case`, Kotlin `when`

### Traits/Protocols/Interfaces
- Behavior without inheritance
- Composition over inheritance
- Can add implementations after type defined
- Examples: Rust traits, Go interfaces, Swift protocols

### Generics/Parametric Polymorphism
- Type parameters: `List<T>`, `fn foo<T>(x: T)`
- Bounds/constraints: `T: Clone + Debug`
- Monomorphization (compile-time specialization) vs boxing
- Associated types for cleaner APIs

### First-Class Functions
- Functions as values
- Closures capture environment
- Higher-order functions (map, filter, fold)
- Essential for modern APIs

### Async/Await
- Structured concurrency
- `async fn` and `.await` syntax
- Cooperative multitasking without callbacks
- Examples: Rust, JavaScript, Python, C#, Kotlin

---

## Language Design Checklist

### Before Adding a Feature
- [ ] What problem does it solve?
- [ ] Is there an existing way to solve it?
- [ ] What's the complexity cost?
- [ ] How does it interact with other features?
- [ ] Can it be added later if needed?
- [ ] Does it compose well with existing features?

### Core Decisions
- [ ] Expression-based or statement-based?
- [ ] Static or dynamic typing?
- [ ] Null handling strategy?
- [ ] Error handling approach?
- [ ] Memory management model?
- [ ] Mutability default?
- [ ] Semicolons required or inferred?
- [ ] Braces or indentation for blocks?

### Tooling Requirements
- [ ] Parseable without type information (for IDE support)
- [ ] Deterministic formatting possible
- [ ] Incremental compilation strategy
- [ ] Error message quality standards
- [ ] LSP integration plan

---

## Key References
- Crafting Interpreters (Bob Nystrom): https://craftinginterpreters.com/
- Go FAQ (design rationale): https://go.dev/doc/faq
- Rust Book (error handling): https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Zig Language Overview: https://ziglang.org/learn/overview/
- Dart Null Safety: https://dart.dev/null-safety/understanding-null-safety
