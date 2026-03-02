<div align="center">

# Ori

**Code That Proves Itself**

Functional semantics. Imperative performance. Zero compromise.

A statically-typed, expression-based language with mandatory testing, explicit effects, and a memory model that turns value semantics into in-place mutations.

[Website](https://ori-lang.com) | [Playground](https://ori-lang.com/playground) | [Getting Started](#quick-start) | [Specification](https://ori-lang.com/docs/spec/01-notation) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)

</div>

> **Experimental:** Ori is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Why Ori?

### Two Problems, One Design

**Problem 1: Code without tests is a hypothesis.** Developers forget to write tests, skip them under pressure, and ignore failures. AI assistants are even worse — they generate code without tests and "fix" failures by weakening assertions.

**Problem 2: "Safe" languages force you to choose.** Garbage collector pauses, borrow checker complexity, or manual memory management. Pick two at most.

### One Solution

Ori makes both automatic. Verification is mandatory — the compiler enforces what discipline cannot. Memory management is invisible — no garbage collector, no borrow checker, no manual allocation. And the same design choices that make code verifiable make it fast: value semantics eliminate aliasing, which lets the compiler optimize aggressively.

## The Memory Model Nobody Else Has

Most languages ask you to make a tradeoff:

| | GC pauses | Lifetime annotations | Manual alloc/free | Reference cycles |
|---|:-:|:-:|:-:|:-:|
| **Go / Java / JS** | Yes | No | No | Handled by GC |
| **Rust** | No | Yes | No | Prevented by borrow checker |
| **C / C++** | No | No | Yes | Your problem |
| **Swift** | No | No | No | Weak refs required |
| **Ori** | No | No | No | Prevented by design |

Ori uses **Automatic Reference Counting** with **value semantics** — every variable owns its data, every assignment is a logical copy, there is no shared mutable state. This is the simplest model for programmers: no aliasing bugs, no data races, no action-at-a-distance through shared references.

But value semantics, naively implemented, is expensive. Every list push copies the entire list. Every struct update allocates a fresh struct. That is where Ori's compiler comes in.

### What You Get

- **No garbage collector** — no pauses, no tuning, no unpredictable latency
- **No borrow checker** — no lifetime annotations, no fighting the compiler
- **No manual memory management** — no malloc/free, no use-after-free
- **No reference cycles** — prevented at compile time by language design, not by weak refs or a cycle collector
- **Functional code, imperative performance** — the compiler transforms value-semantic code into in-place mutations automatically

### How the Compiler Makes It Fast

Write functional code. The compiler generates what an imperative programmer would write by hand.

```ori
@transform (items: [int]) -> [int] =
    items.iter()
        .filter(predicate: x -> x > 0)
        .map(transform: x -> x * 2)
        .collect()
```

When `items` is uniquely owned, the compiler transforms this into in-place mutations — zero intermediate allocations, zero copies. The programmer writes pure transformations; the compiled output mutates buffers directly.

This works because Ori stacks **eight optimization layers** that compound on each other:

1. **Half your variables need zero memory management** — scalars (ints, bools, small structs with no heap data) skip reference counting entirely. In a typical program, roughly 50% of variables are scalars.

2. **Read-only parameters cost nothing** — the compiler analyzes your entire module's call graph and infers which function parameters are only read, not stored or returned. Those parameters skip all reference counting at every call site. `len(list)` never touches the refcount.

3. **Values are freed at last use, not scope end** — instead of waiting until a variable goes out of scope, the compiler places cleanup at the precise point where each value is last used, creating more opportunities for the next optimization.

4. **Drop + allocate = reuse in place** — when the compiler sees a value being freed followed by a new allocation of the same type, it reuses the memory directly. A list map that deconstructs each node and constructs a new one reuses every node allocation in place.

5. **Struct field updates are surgical** — updating one field of a struct with `{ ...point, x: new_x }` only writes the changed field when the struct is uniquely owned. Unchanged fields are left untouched.

6. **Redundant bookkeeping is eliminated** — after all other optimizations, a dataflow pass removes any remaining reference count operations that cancel each other out.

7. **Collection mutations are O(1) when uniquely owned** — every list push, map insert, or set remove checks ownership at runtime. If you are the only owner, it mutates in place. If shared, it copies first (copy-on-write).

8. **The compiler proves ownership at compile time** — when the compiler can prove a value is uniquely owned, it skips even the runtime ownership check. No branch, no check — just the fast path.

Each layer creates opportunities for the next. The net result: code written in pure value semantics — no mutation syntax, no ownership annotations, no lifetime parameters — compiles to code that mutates in-place, reuses allocations, and eliminates branches on provably-unique values.

For the full technical details, see the [ARC System Design](https://ori-lang.com/docs/compiler-design/09-arc-system).

### How Ori Compares

| | Ori | Swift | Rust | Go | Lean 4 | Koka |
|---|---|---|---|---|---|---|
| **Memory strategy** | ARC + value semantics | ARC + ref semantics | Ownership + borrowing | Tracing GC | ARC + value semantics | Perceus RC |
| **Developer burden** | None | Weak refs for cycles | Lifetime annotations | None | None | None |
| **Optimization layers** | 8 stacked | Retain/release pairing | Zero-cost (static) | GC tuning | 4 (classify, borrow, RC, reuse) | 3 (Perceus, borrow, reuse) |
| **Borrow inference** | Interprocedural (whole module) | No (manual annotations) | Static (borrow checker) | N/A | Interprocedural | Two-pass |
| **In-place reuse** | Yes (cross-block) | COW collections only | By construction | No | Yes (intra-block) | Yes (Perceus) |
| **Cycle prevention** | Structural (language design) | Weak refs (manual) | Ownership rules | GC handles it | Structural | Structural |
| **Latency** | Deterministic | Deterministic | Deterministic | GC pauses | Deterministic | Deterministic |

## Mandatory Testing

Every function requires tests. No exceptions. No skipping. No "I'll add tests later."

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: fibonacci(n: n - 1) + fibonacci(n: n - 2),
    memo: true,
)

@test_fibonacci tests @fibonacci () -> void = {
    assert_eq(fibonacci(n: 0), 0);
    assert_eq(fibonacci(n: 1), 1);
    assert_eq(fibonacci(n: 10), 55);
}
```

`@test tests @target` creates a compiler-enforced bond between test and function. The compiler ensures every function has tests. No tests, no compile.

## Dependency-Aware Test Execution

Tests are in the dependency graph. Change `@parse`, and tests for `@compile` (which calls `@parse`) run too.

```ori
@parse (input: str) -> Result<Ast, Error> = ...
@test_parse tests @parse () -> void = ...

@compile (input: str) -> Result<Binary, Error> = {
    let ast = parse(input: input)?;
    generate_code(ast: ast)
}
@test_compile tests @compile () -> void = ...
```

Change `@parse` → the compiler runs `@test_parse` AND `@test_compile`.

## Explicit Effects & Trivial Mocking

Side effects are tracked through capabilities. Mocking is just providing a different implementation.

```ori
@fetch_user (id: UserId) -> Result<User, Error> uses Http =
    Http.get("/users/" + str(id))

@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp(responses: {"/users/1": mock_user}) in {
        let result = fetch_user(id: 1);
        assert_ok(result);
        assert_eq(result.unwrap().name, "Alice");
    }
```

No test framework. No mocking library. Just the language.

## Contracts

Functions declare and enforce their invariants.

```ori
@sqrt (x: float) -> float
    pre(x >= 0.0)
    post(r -> r >= 0.0)
= newton_raphson(x)

@test_sqrt tests @sqrt () -> void = {
    assert_eq(sqrt(x: 4.0), 2.0);
    assert_panics(sqrt(x: -1.0));
}
```

## The Design That Compounds

These are not independent features bolted together. They are one coherent design where each choice reinforces the others:

```
Value semantics
├── No aliasing → ARC optimizes aggressively (in-place reuse, COW, borrow inference)
├── No shared mutable state → capabilities track ALL effects
│   └── Capabilities → trivial mocking → mandatory testing isn't painful
│       └── Mandatory testing → code integrity → refactoring is safe
└── Pure functions (no capabilities) → compiler can memoize, reorder, parallelize
```

**Value semantics** make the memory model possible — without aliasing, the compiler can prove ownership and reuse allocations. They also make capabilities complete — when mutation is explicit, every side effect is visible. Capabilities make mocking trivial, which makes mandatory testing practical rather than painful. And mandatory testing means code integrity is enforced, not hoped for.

The same design choice that gives you safe memory gives you testable code gives you optimizable programs. One decision, compounding returns.

## Quick Start

Install Ori (latest nightly):

```bash
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh
```

Write your first program (`hello.ori`):

```ori
@main () -> void = print("Hello, Ori!")
```

Run it:

```bash
ori run hello.ori
```

## Usage

```bash
ori run program.ori      # Run a program
ori test                 # Run all tests (parallel)
ori test file.test.ori   # Run specific test file
ori build program.ori    # Compile to native binary
ori check program.ori    # Type-check and verify test coverage
ori fmt src/             # Format source files
```

## Installation

### Quick Install (Recommended)

```bash
# Install latest nightly (default during alpha)
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh

# Install specific version
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh -s -- --version v0.1.0-alpha.2
```

### Binary Distributions

Official binaries are available at [GitHub Releases](https://github.com/upstat-io/ori-lang/releases).

**Platforms:** Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x86_64)

### Install from Source

```bash
git clone https://github.com/upstat-io/ori-lang
cd ori-lang
cargo build --release
cp target/release/ori ~/.local/bin/
```

Requires Rust 1.70+.

## Documentation

- [Website](https://ori-lang.com) — Official website with guides and documentation
- [Playground](https://ori-lang.com/playground) — Try Ori in your browser
- [Language Specification](https://ori-lang.com/docs/spec/01-notation) — Formal language definition
- [Compiler Design](https://ori-lang.com/docs/compiler-design/01-architecture) — Compiler architecture and internals
- [ARC System Design](https://ori-lang.com/docs/compiler-design/09-arc-system) — Memory model and optimization pipeline
- [Roadmap](https://ori-lang.com/roadmap) — Development roadmap and progress
- [Proposals](docs/ori_lang/proposals/) — Design decisions and rationale

## Design Philosophy

**Code that proves itself.** Every function tested. Every change traced. Every effect explicit. Every allocation optimized.

Ori makes verification and performance automatic — the compiler enforces what discipline alone cannot.

| Traditional Approach | Ori Approach |
|---------------------|----------------|
| Tests are optional | Tests are mandatory |
| Tests are external | Tests are in the dependency graph |
| Change and hope | Change and know what broke |
| Mock with frameworks | Mock with capabilities |
| Runtime errors | Compile-time guarantees |
| Hidden effects | Explicit capabilities |
| GC pauses or borrow checker | ARC with value semantics |
| Manual optimization hints | Optimization from semantics |

## Getting Help

- **Website:** [ori-lang.com](https://ori-lang.com)
- **Bug reports & feature requests:** [GitHub Issues](https://github.com/upstat-io/ori-lang/issues)
- **Discussions:** [GitHub Discussions](https://github.com/upstat-io/ori-lang/discussions)

## Contributing

Ori is open source and we welcome contributions. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Running Tests

```bash
cargo test               # Compiler unit tests
ori test                 # Language test suite
```

## License

Ori is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
