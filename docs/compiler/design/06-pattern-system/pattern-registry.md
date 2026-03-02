---
title: "Pattern Registry"
description: "Ori Compiler Design — Pattern Registry"
order: 603
section: "Pattern System"
---

# Pattern Registry

## What Is a Pattern Registry?

A compiler with multiple built-in constructs needs a way to look up the right behavior for each construct. When the evaluator encounters a `recurse(...)` expression, it needs to find the `RecursePattern` implementation. When the type checker encounters `parallel(...)`, it needs to find the pattern's required properties and scoped bindings. The component that maps construct names (or kinds) to their implementations is the **pattern registry**.

This is a specific instance of the general dispatch problem in compiler design: given a tag (the construct kind), find and invoke the right handler. Compilers solve this in several ways, each with different performance, safety, and extensibility characteristics.

### Dispatch Strategies

**HashMap dispatch.** Store implementations in a `HashMap<Name, Box<dyn Handler>>`. Lookup is O(1) amortized but involves hashing, comparison, and trait object indirection. Registration is dynamic — new entries can be added at runtime. This is the natural choice for extensible systems (plugin architectures, user-defined operators) but overkill for a fixed set known at compile time.

**Trait object dispatch.** Store implementations behind `dyn Trait` pointers. Each call goes through a vtable — an array of function pointers indexed by method. This adds one level of indirection per call. Useful when the set of implementations varies at runtime, but for a fixed set, the indirection is pure overhead.

**Visitor pattern.** Define an `accept()` method on each construct that calls the appropriate `visit_*()` method on a visitor. This is common in Java-style compilers (ANTLR, Eclipse JDT) and provides open extensibility on the visitor side while keeping the construct side fixed. However, it requires every construct to implement `accept()`, and adding a new construct requires modifying the visitor interface.

**Enum dispatch.** Wrap each implementation in an enum variant and use `match` to dispatch. Lookup is a simple branch (compiled to a jump table or binary search by the CPU). The Rust compiler enforces exhaustive matching, so adding a new variant without handling it everywhere is a compile error. This is the fastest dispatch mechanism and provides the strongest static guarantees, but requires modifying the enum for each new implementation.

**Direct function pointers.** Store function pointers in a static array indexed by construct kind. This is essentially a manual vtable — fast (array index + indirect call) but lacks type safety and exhaustiveness checking. Used in some C compilers for opcode dispatch.

The choice depends on whether the set of constructs is open or closed. For open sets (user-defined types, plugin systems), HashMap or trait objects are appropriate. For closed sets known at compile time (keywords, operators, built-in constructs), enum dispatch provides the best combination of performance, safety, and maintainability.

## Ori's Enum Dispatch Design

Ori's pattern set is closed — only the compiler team adds patterns, and each addition is a deliberate architectural decision. This makes enum dispatch the natural choice:

```rust
pub enum Pattern {
    Recurse(RecursePattern),
    Parallel(ParallelPattern),
    Spawn(SpawnPattern),
    Timeout(TimeoutPattern),
    Cache(CachePattern),
    With(WithPattern),
    Print(PrintPattern),
    Panic(PanicPattern),
    Catch(CatchPattern),
    Todo(TodoPattern),
    Unreachable(UnreachablePattern),
    Channel(ChannelPattern),
}
```

Each variant wraps a concrete pattern type — a zero-sized struct (ZST) that implements `PatternDefinition`. The enum itself implements `PatternDefinition` by delegating to the inner type:

```rust
impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        match self {
            Pattern::Recurse(p) => p.name(),
            Pattern::Parallel(p) => p.name(),
            Pattern::Spawn(p) => p.name(),
            // ... delegates to each inner type
        }
    }
    // Same delegation for required_props, evaluate, etc.
}
```

This delegation pattern means callers work with `Pattern` (the enum) through the `PatternDefinition` trait, while each pattern's logic lives in its own struct. The enum acts as a type-safe adapter between the uniform interface and the concrete implementations.

## The PatternRegistry

The registry itself is minimal — a zero-state struct whose only job is to map `FunctionExpKind` (the AST's construct kind) to `Pattern` (the evaluation-ready handler):

```rust
pub struct PatternRegistry {
    _private: (),  // Marker to prevent external construction
}

impl PatternRegistry {
    pub fn new() -> Self {
        PatternRegistry { _private: () }
    }

    /// Map AST construct kind to pattern handler (static dispatch).
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            FunctionExpKind::Recurse => Pattern::Recurse(RecursePattern),
            FunctionExpKind::Parallel => Pattern::Parallel(ParallelPattern),
            FunctionExpKind::Spawn => Pattern::Spawn(SpawnPattern),
            FunctionExpKind::Timeout => Pattern::Timeout(TimeoutPattern),
            FunctionExpKind::Cache => Pattern::Cache(CachePattern),
            FunctionExpKind::With => Pattern::With(WithPattern),
            FunctionExpKind::Print => Pattern::Print(PrintPattern),
            FunctionExpKind::Panic => Pattern::Panic(PanicPattern),
            FunctionExpKind::Catch => Pattern::Catch(CatchPattern),
            FunctionExpKind::Todo => Pattern::Todo(TodoPattern),
            FunctionExpKind::Unreachable => Pattern::Unreachable(UnreachablePattern),
            FunctionExpKind::Channel
            | FunctionExpKind::ChannelIn
            | FunctionExpKind::ChannelOut
            | FunctionExpKind::ChannelAll => Pattern::Channel(ChannelPattern),
        }
    }

    /// Iterate all registered pattern kinds.
    pub fn kinds(&self) -> impl Iterator<Item = FunctionExpKind> { /* ... */ }

    /// Number of registered pattern kinds.
    pub fn len(&self) -> usize { self.kinds().count() }

    /// Always false (registry always has patterns).
    pub fn is_empty(&self) -> bool { false }
}
```

Note the `_private: ()` field — this prevents external code from constructing a `PatternRegistry` directly, ensuring the registry is created only through `PatternRegistry::new()`. This is a Rust idiom for controlling construction without making the struct's existence private.

### Why the Registry Has No State

The `PatternRegistry` stores nothing. It has no `HashMap`, no `Vec`, no cached data. Each call to `get()` creates a fresh `Pattern` value on the stack — which costs nothing because all pattern types are ZSTs (zero-sized types, occupying zero bytes of memory). The registry is purely a dispatch mechanism: given a `FunctionExpKind`, return the corresponding `Pattern`.

This design means the registry is trivially `Send + Sync`, requires no initialization, and has no lifecycle management concerns. It's a function disguised as a struct — the struct exists only to provide a namespace and prevent construction from outside `ori_patterns`.

### Channel Variant Collapse

Four `FunctionExpKind` variants (`Channel`, `ChannelIn`, `ChannelOut`, `ChannelAll`) all map to a single `Pattern::Channel(ChannelPattern)`. The specific channel flavor is distinguished at the type level (different return types) and in the evaluator (which passes the original `FunctionExpKind` to the pattern's context), not at the registry level. This collapse keeps the `Pattern` enum smaller while preserving the parser's ability to distinguish channel syntax.

## Registered Patterns

| Pattern | Purpose | Required Props | Optional Props |
|---|---|---|---|
| `recurse` | Self-referential recursion with `self` binding | `condition`, `base`, `step` | `memo` |
| `parallel` | Concurrent task execution | `tasks` | `max_concurrent`, `timeout` |
| `spawn` | Fire-and-forget task execution | `tasks` | `max_concurrent` |
| `timeout` | Time-limited execution | `operation`, `after` | — |
| `cache` | Capability-aware memoization | `operation` | `key`, `ttl` |
| `with` | RAII resource management | `acquire`, `action` | `release` |
| `print` | Output via Print capability | `msg` | — |
| `panic` | Diverge with message (returns `Never`) | `msg` | — |
| `catch` | Catch panics as `Result` | `expr` | — |
| `todo` | Unimplemented marker (returns `Never`) | — | `reason` |
| `unreachable` | Unreachable marker (returns `Never`) | — | `reason` |
| `channel` | Message passing channel (stub) | `buffer` | — |

Concurrency patterns (`parallel`, `spawn`, `timeout`, `with`, `channel` variants) are defined in the registry but rejected by the type checker as E2040 (post-2026 features). The evaluator provides honest stub implementations that emit `tracing::warn!()` diagnostics. This approach lets the parser and registry accept valid syntax while deferring full implementation to a later milestone.

## Usage in the Type Checker

The type checker uses pattern metadata from the registry to drive inference. It does NOT call `evaluate()` — that's the evaluator's job. Instead, it reads `required_props()`, `optional_props()`, and `scoped_bindings()` to know which property expressions to type-check and which identifiers to inject:

```rust
// In ori_types — simplified
fn infer_function_exp(&mut self, kind: FunctionExpKind, props: &[NamedProp]) -> Idx {
    let pattern = self.pattern_registry.get(kind);

    // Validate required properties are present
    for prop_name in pattern.required_props() {
        // ...
    }

    // Set up scoped bindings for specific properties
    for binding in pattern.scoped_bindings() {
        // Extend type environment for binding.for_props
    }

    // Type-check each property expression
    // ...
}
```

The metadata-driven approach means the type checker doesn't need pattern-specific code for each construct — it reads the metadata and applies generic property-checking logic. Only patterns with unusual type signatures (like `recurse`, which needs the enclosing function's type for `self`) require custom handling in `ori_types`.

## Usage in the Evaluator

The evaluator dispatches through the registry to execute patterns:

```rust
// In ori_eval — simplified
fn eval_function_exp(&mut self, kind: FunctionExpKind, props: &[NamedExpr]) -> EvalResult {
    let pattern = self.pattern_registry.get(kind);
    let ctx = EvalContext::new(self.interner, self.arena, props);
    pattern.evaluate(&ctx, self)  // self implements PatternExecutor
}
```

The evaluator implements `PatternExecutor`, providing `eval()`, `call()`, `lookup_capability()`, and `bind_var()` methods that patterns call during evaluation. This makes the evaluator the bridge between patterns and the execution environment.

## Properties of Enum Dispatch

The enum dispatch approach provides several guarantees:

**Exhaustive matching.** If a new variant is added to `Pattern`, every `match` on `Pattern` must handle it — the Rust compiler rejects incomplete matches. This catches "forgot to handle the new pattern" bugs at compile time rather than runtime.

**Zero heap allocation.** All pattern types are ZSTs. The `Pattern` enum is stack-allocated with a discriminant tag (typically 1 byte) and no payload data. Creating a pattern via `PatternRegistry::get()` has the same cost as assigning an integer.

**Static dispatch through enum.** Although `PatternDefinition` is a trait, the enum's `match`-based delegation enables the Rust compiler to see through to each concrete type. With optimization, this can inline the trait call entirely, eliminating even the enum match overhead for known-constant kinds.

**No borrow issues.** Pattern values are owned, not references to statics. Each `get()` call creates a fresh value. There are no lifetime constraints, no shared references, and no synchronization requirements.

**Direct dispatch.** The mapping from `FunctionExpKind` to `Pattern` is a `match` statement (compiled to a jump table or branch chain). There is no hash computation, no bucket traversal, and no comparison chain — just a direct branch on the discriminant.

## Prior Art

### Rust Compiler — Intrinsic Recognition

[Rust's compiler](https://github.com/rust-lang/rust) recognizes intrinsic functions by matching their `DefId` against a known set during code generation. The dispatch is a large `match` on the intrinsic name string, similar in spirit to Ori's enum match but using string matching rather than typed enum variants. Ori's approach is more type-safe (enum variants vs. strings) but less flexible (can't add intrinsics via string matching in plugins).

### GHC — PrimOp Dispatch

[GHC](https://gitlab.haskell.org/ghc/ghc) has a `PrimOp` type (algebraic data type) that enumerates all primitive operations. Code generation dispatches on `PrimOp` variants, much like Ori dispatches on `Pattern` variants. GHC generates much of the dispatch code from a specification file (`primops.txt.pp`), ensuring consistency between the type checker and code generator. Ori achieves similar consistency through the `PatternDefinition` trait, which provides metadata for both type checking and evaluation.

### Zig — Builtin Function Table

[Zig's compiler](https://github.com/ziglang/zig) stores builtin functions in a static array of structs (`builtin_fns`), each containing a name, parameter types, return type, and an evaluation function pointer. Lookup is a linear scan of the array. This is simpler than Ori's trait-based approach but lacks exhaustiveness checking — if a new builtin is added to the array but not handled in the evaluator, the error is only caught at runtime.

### Roslyn — Operation Kind Enum

[Roslyn](https://github.com/dotnet/roslyn) (C# compiler) uses `OperationKind` enums to dispatch built-in operations. Like Ori's `FunctionExpKind → Pattern` mapping, Roslyn maps `SyntaxKind` to `OperationKind` and dispatches evaluation through enum matching. The C# approach is similar in structure to Ori's but operates at a lower level (individual operators and statements rather than high-level constructs).

## Design Tradeoffs

**Enum vs. HashMap for a fixed set.** Enum dispatch is faster (branch vs. hash + compare), safer (exhaustive matching), and lighter (no allocation) than HashMap dispatch. The only advantage of HashMap is dynamic extensibility, which patterns don't need — they're a closed set that changes only when the compiler team explicitly adds a new one.

**Single registry vs. per-phase registries.** Ori uses one `PatternRegistry` shared between the type checker and evaluator. An alternative is separate registries for each phase, allowing different metadata for type checking vs. evaluation. The single-registry approach is simpler and ensures consistency (both phases see the same required properties), but means every pattern must implement the full `PatternDefinition` trait even if a phase only uses a subset of methods.

**ZST variants vs. singleton statics.** Ori creates fresh ZST instances in each `get()` call rather than returning references to static instances. Both approaches have zero runtime cost (ZSTs occupy no memory), but fresh instances avoid lifetime annotations on the return type and make the API simpler. Static instances would enable caching pattern-level data across calls, but since patterns have no state, there's nothing to cache.

**Channel variant collapse vs. separate patterns.** Four channel kinds map to one `ChannelPattern`. An alternative is four separate pattern implementations, each tailored to its channel flavor. The collapse keeps the enum smaller and avoids code duplication (the four channel types share most of their behavior), but means `ChannelPattern.evaluate()` must check the original `FunctionExpKind` to distinguish flavors — a small complexity tax for a simpler type hierarchy.
