---
title: "Pattern Fusion"
description: "Ori Compiler Design — Pattern Fusion"
order: 602
section: "Pattern System"
---

# Pattern Fusion

## What Is Fusion?

When a programmer writes a pipeline of data transformations — map, then filter, then fold — the naive execution creates an intermediate data structure at each step. Map produces a temporary list. Filter produces another temporary list. Fold consumes the final list to produce a scalar. Two temporary lists are allocated, filled, and discarded, even though the programmer's intent is a single pass over the data.

**Fusion** (also called **deforestation**) is the compiler optimization that eliminates these intermediate data structures by combining multiple transformations into a single pass. Instead of three separate loops (map, filter, fold), the fused version runs one loop that transforms each element, tests the predicate, and accumulates the result — no intermediate allocations, no wasted iterations.

```text
Without fusion:                      With fusion:
[1, 2, 3, 4, 5]                     [1, 2, 3, 4, 5]
    ↓ map(× 2)                          ↓ single pass:
[2, 4, 6, 8, 10]  ← temporary           for each x:
    ↓ filter(> 5)                          y = x × 2
[6, 8, 10]         ← temporary            if y > 5: acc += y
    ↓ fold(+, 0)                     result: 24
result: 24
```

The fused version processes each element exactly once, allocates no intermediate lists, and is more cache-friendly (no pointer-chasing between temporary allocations).

### A Brief History of Fusion

The idea of eliminating intermediate data structures dates to Philip Wadler's 1988 paper ["Deforestation: Transforming Programs to Eliminate Trees"](https://homepages.inf.ed.ac.uk/wadler/papers/deforest/deforest.ps), which showed that compositions of `foldr`/`build` pairs could be mechanically fused. The name "deforestation" comes from eliminating the intermediate tree structures (lists being a degenerate case of trees).

Andrew Gill, John Launchbury, and Simon Peyton Jones extended this in their 1993 paper ["A Short Cut to Deforestation"](https://www.microsoft.com/en-us/research/publication/a-short-cut-to-deforestation/), introducing the `foldr`/`build` fusion rule that became the foundation of GHC's list fusion. Their key insight: if a producer builds a list using `build` and a consumer tears it down using `foldr`, the intermediate list can be eliminated by composing the producer and consumer directly.

Duncan Coutts, Roman Leshchinskiy, and Don Stewart later developed [stream fusion](https://dl.acm.org/doi/10.1145/1291151.1291199) (2007), which represents lists as unfold-based streams rather than fold-based constructors. Stream fusion handles more cases than `foldr`/`build` (particularly left folds and zips) and is the basis of Haskell's `Data.Vector` and `Data.Text` libraries.

### Fusion Strategies in Compilers

Different compilers approach fusion at different levels:

**Rewrite rules (GHC).** GHC's RULES pragma lets library authors specify algebraic identities like `map f . map g = map (f . g)`. The simplifier applies these rules during optimization, fusing compositions mechanically. This is maximally flexible (any library can define fusion rules) but relies on the simplifier firing at the right time and in the right order.

**Iterator adapters (Rust).** Rust's `Iterator` trait uses lazy adapters — `.map()`, `.filter()`, `.take()` return new iterator structs rather than allocating new collections. Fusion happens naturally because the final consumer (`.collect()`, `.sum()`, `.for_each()`) drives evaluation through the adapter chain. No compiler transformation is needed; the type system handles it.

**Stream API (Java).** Java's `Stream` API similarly uses lazy intermediate operations and eager terminal operations. The JIT compiler can sometimes fuse the pipeline, but fusion is not guaranteed — it depends on escape analysis and inlining heuristics.

**Explicit fusion pass (Ori).** Ori's pattern fusion detects fusible patterns at the AST level and combines them into single-pass `FusedPattern` values. This is more explicit than GHC's rewrite rules (the fusible combinations are enumerated) and more compiler-driven than Rust's lazy iterators (the compiler identifies and rewrites the fusion).

## Status

The fusion infrastructure — data structures, chain detection, hint generation, and evaluation logic — exists in `ori_patterns`. However, the fusible combinations it references (`map`, `filter`, `fold`, `find`) are collection methods dispatched through `ori_eval`'s method system, not `FunctionExpKind` patterns. The fusion code is infrastructure for potential future optimization of pattern pipelines, not currently connected to active evaluation.

This is intentional: the data structures and evaluation logic are designed and implemented ahead of the patterns they would optimize. When collection operations are eventually expressible as fusible patterns (or when the optimizer can detect method-call chains), the fusion infrastructure is ready.

## Fusible Combinations

The fusion system supports seven combinations of two or three operations:

| Stage 1 | Stage 2 | Stage 3 | Fused Form | Allocations Saved |
|---|---|---|---|---|
| map | filter | — | MapFilter | 1 intermediate list |
| filter | map | — | FilterMap | 1 intermediate list |
| map | fold | — | MapFold | 1 intermediate list |
| filter | fold | — | FilterFold | 1 intermediate list |
| map | filter | fold | MapFilterFold | 2 intermediate lists |
| map | find | — | MapFind | 1 intermediate list |
| filter | find | — | FilterFind | 1 intermediate list |

**Terminal operations** (`fold`, `find`) can be fused into but do not chain further — they consume the pipeline and produce a scalar or `Option`, not a collection.

### Fusion Rules

Each combination has a precise semantic equivalence:

**map + filter → MapFilter:**
```text
filter(over: map(over: xs, transform: f), predicate: p)
≡ single pass: for x in xs, let y = f(x), if p(y) then yield y
```

**filter + map → FilterMap:**
```text
map(over: filter(over: xs, predicate: p), transform: f)
≡ single pass: for x in xs, if p(x) then yield f(x)
```

Note the difference: `MapFilter` transforms first then tests, while `FilterMap` tests first then transforms. The order matters — `MapFilter` applies the predicate to the *transformed* value, while `FilterMap` applies the transform only to values that *pass* the predicate.

**map + fold → MapFold:**
```text
fold(over: map(over: xs, transform: f), init: i, op: g)
≡ single pass: acc = i; for x in xs, acc = g(acc, f(x)); return acc
```

**filter + fold → FilterFold:**
```text
fold(over: filter(over: xs, predicate: p), init: i, op: g)
≡ single pass: acc = i; for x in xs, if p(x) then acc = g(acc, x); return acc
```

**map + filter + fold → MapFilterFold:**
```text
fold(over: filter(over: map(over: xs, transform: f), predicate: p), init: i, op: g)
≡ single pass: acc = i; for x in xs, let y = f(x), if p(y) then acc = g(acc, y); return acc
```

This is the most impactful fusion — it eliminates two intermediate lists and reduces three passes to one.

**map + find → MapFind:**
```text
find(over: map(over: xs, transform: f), where: p)
≡ single pass: for x in xs, let y = f(x), if p(y) then return Some(y); return None
```

**filter + find → FilterFind:**
```text
find(over: filter(over: xs, predicate: p1), where: p2)
≡ single pass: for x in xs, if p1(x) && p2(x) then return Some(x); return None
```

## Data Structures

### FusedPattern

The `FusedPattern` enum represents all supported fusion combinations. Each variant stores the `ExprId`s needed to evaluate the fused pipeline in a single pass:

```rust
pub enum FusedPattern {
    MapFilter { input: ExprId, map_fn: ExprId, filter_fn: ExprId },
    FilterMap { input: ExprId, filter_fn: ExprId, map_fn: ExprId },
    MapFold { input: ExprId, map_fn: ExprId, init: ExprId, fold_fn: ExprId },
    FilterFold { input: ExprId, filter_fn: ExprId, init: ExprId, fold_fn: ExprId },
    MapFilterFold {
        input: ExprId, map_fn: ExprId, filter_fn: ExprId,
        init: ExprId, fold_fn: ExprId,
    },
    MapFind { input: ExprId, map_fn: ExprId, find_fn: ExprId },
    FilterFind { input: ExprId, filter_fn: ExprId, find_fn: ExprId },
}
```

Each variant captures exactly the expressions needed for single-pass evaluation. The `input` field is the original collection; the `*_fn` fields are the transformation, predicate, or accumulation functions; `init` is the fold's initial accumulator.

`FusedPattern` has an `evaluate()` method that uses the `PatternExecutor` trait to execute fused operations in a single loop over the input collection. Each variant's evaluation logic mirrors what the individual operations would do, but without intermediate allocations.

### PatternChain

A `PatternChain` represents a sequence of patterns detected as fusion candidates, ordered from innermost to outermost:

```rust
pub struct PatternChain {
    /// Patterns from innermost to outermost.
    pub links: Vec<ChainLink>,
    /// The original input expression (innermost source).
    pub input: ExprId,
    /// Span covering the entire chain.
    pub span: Span,
}
```

### ChainLink

Each link in a chain records the pattern kind, its named arguments, and source location:

```rust
pub struct ChainLink {
    /// The pattern kind.
    pub kind: FunctionExpKind,
    /// Named expression IDs for this pattern's arguments.
    pub props: Vec<(Name, ExprId)>,
    /// The expression ID of this pattern call.
    pub expr_id: ExprId,
    /// Span of this pattern.
    pub span: Span,
}
```

### FusionHints

`FusionHints` describes the optimization benefit of a fusion, used for diagnostics and decision-making:

```rust
pub struct FusionHints {
    /// Estimated number of intermediate allocations avoided.
    pub allocations_avoided: usize,
    /// Estimated number of iterations saved.
    pub iterations_saved: usize,
    /// Whether this fusion eliminates intermediate lists.
    pub eliminates_intermediate_lists: bool,
}
```

Factory methods `FusionHints::two_pattern()` and `FusionHints::three_pattern()` create hints for common fusion depths: a two-pattern fusion saves 1 allocation and 1 iteration; a three-pattern fusion saves 2 allocations and 2 iterations.

## When Fusion Applies

Three conditions must hold for fusion to apply:

1. **Pipeline structure** — the inner pattern's result is the `over` argument to the outer pattern (one feeds into the next)
2. **Fusible combination** — the pair (or triple) appears in the fusible combinations table
3. **No side effects** — there are no observable effects between the patterns that would change behavior if the intermediate data structure is eliminated

The third condition is important: if the map function has side effects (printing, mutation, I/O), fusing with a filter could change the order or number of side-effect executions. Fusion is only correct when the intermediate operations are pure.

## Prior Art

### GHC — foldr/build and Stream Fusion

[GHC](https://gitlab.haskell.org/ghc/ghc) pioneered practical fusion in functional languages. The `foldr`/`build` system (Gill, Launchbury, Peyton Jones 1993) uses RULES pragmas to eliminate intermediate lists:

```haskell
{-# RULES "map/map" forall f g xs. map f (map g xs) = map (f . g) xs #-}
```

[Stream fusion](https://dl.acm.org/doi/10.1145/1291151.1291199) (Coutts, Leshchinskiy, Stewart 2007) extended this to handle cases `foldr`/`build` cannot, particularly left folds and zips. Haskell's approach is more general than Ori's — any library can define fusion rules — but also more fragile (rules depend on the simplifier's pass ordering).

### Rust — Iterator Adapter Fusion

[Rust](https://github.com/rust-lang/rust)'s `Iterator` trait achieves fusion through lazy evaluation at the type level. `.map(f).filter(p).collect()` creates a chain of adapter structs (`Map<Filter<Iter>>`) that the compiler can inline and optimize into a single loop. No explicit fusion pass is needed — LLVM's optimizer fuses the inlined iterator chain naturally. This is zero-cost in practice but relies on the optimizer's ability to see through adapter types.

### Java — Stream API

[Java's Stream API](https://docs.oracle.com/javase/8/docs/api/java/util/stream/package-summary.html) uses lazy intermediate operations and eager terminal operations, similar to Rust's iterators. However, Java's streams go through interface dispatch (virtual calls), which limits the JIT's ability to fuse operations. The `Stream` implementation maintains an internal pipeline representation that batches operations, achieving some fusion at the library level rather than the compiler level.

### Futhark — Explicit Fusion Pass

[Futhark](https://github.com/diku-dk/futhark) (a data-parallel functional language) has an explicit [fusion pass](https://futhark-lang.org/publications/array16.pdf) that combines parallel operations like `map`, `reduce`, `scan`, and `filter`. Futhark's fusion is more aggressive than Ori's — it fuses across parallel boundaries and handles multi-dimensional arrays — because Futhark targets GPUs where eliminating intermediate arrays has massive performance impact.

### Polyhedral Compilation

The [polyhedral model](https://en.wikipedia.org/wiki/Polyhedral_model) (used in tools like [Polly](https://polly.llvm.org/) for LLVM) represents loop nests as polyhedra in integer space and applies affine transformations to fuse, tile, and parallelize loops. This is the most general approach to loop fusion but is limited to affine access patterns and has high compile-time cost. Ori's pattern-level fusion is far simpler — it handles enumerated combinations rather than arbitrary loop nests.

## Design Tradeoffs

**Enumerated combinations vs. general fusion rules.** Ori enumerates seven specific fusible combinations. An alternative is a general fusion rule system (like GHC's RULES) that could discover fusion opportunities automatically. The enumerated approach is simpler to implement and verify (each combination has a clear semantic equivalence) but cannot discover novel fusions. The general approach is more powerful but harder to make reliable (rule ordering, termination, correctness).

**Ahead-of-time fusion vs. lazy iterators.** Ori detects and rewrites fusion at the AST level. An alternative is Rust's approach: make all collection operations lazy by default (returning iterator adapters) and let the final consumer drive evaluation. Lazy iterators achieve fusion without compiler support but change the programming model (every operation returns an iterator, explicit `.collect()` needed). Ori keeps the eager, list-producing semantics at the source level and applies fusion as an optimization.

**Infrastructure-first development.** The fusion data structures and evaluation logic exist before the patterns they would optimize. This is a deliberate investment: when collection operations become fusible (either as patterns or through method-chain optimization), the infrastructure is ready. The tradeoff is carrying code that isn't yet connected to active evaluation, which adds to the codebase without providing immediate user-facing benefit.

**Side effect restrictions.** Fusion is only correct for pure operations. If a map function logs each element, fusing with a filter would suppress logging for filtered elements — changing observable behavior. Ori's fusion infrastructure assumes purity, which is sound for the targeted use cases (mathematical transformations) but limits applicability to side-effectful pipelines.
