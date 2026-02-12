---
title: "Pattern Fusion"
description: "Ori Compiler Design — Pattern Fusion"
order: 602
section: "Pattern System"
---

# Pattern Fusion

Pattern fusion is an optimization that combines multiple sequential patterns into a single pass, eliminating intermediate data structures.

## Location

```
compiler/ori_patterns/src/fusion.rs
```

## Motivation

Without fusion:
```ori
// Creates intermediate list after map
filter(
    over: map(over: items, transform: x -> x * 2),
    predicate: x -> x > 10,
)
```

Execution:
1. map creates new list: `[2, 4, 6, 8, ...]`
2. filter creates another list: `[12, 14, ...]`

With fusion:
```
map + filter becomes MapFilter (single pass)
```

Execution:
1. Single pass: transform and test each element
2. One output list: `[12, 14, ...]`

## Fusible Combinations

| Pattern 1 | Pattern 2 | Pattern 3 | Fused Form |
|-----------|-----------|-----------|------------|
| map | filter | - | MapFilter |
| filter | map | - | FilterMap |
| map | fold | - | MapFold |
| filter | fold | - | FilterFold |
| map | filter | fold | MapFilterFold |
| map | find | - | MapFind |
| filter | find | - | FilterFind |

Terminal patterns (`fold`, `find`) can be fused into but do not chain further.

## Data Structures

### FusedPattern Enum

The `FusedPattern` enum represents all supported fusion combinations. Each variant stores the `ExprId`s needed to evaluate the fused pipeline in a single pass:

```rust
pub enum FusedPattern {
    /// map + filter: transform, then keep if predicate passes
    MapFilter { input: ExprId, map_fn: ExprId, filter_fn: ExprId },

    /// filter + map: keep if predicate passes, then transform
    FilterMap { input: ExprId, filter_fn: ExprId, map_fn: ExprId },

    /// map + fold: transform each element, accumulate
    MapFold { input: ExprId, map_fn: ExprId, init: ExprId, fold_fn: ExprId },

    /// filter + fold: keep matching elements, accumulate
    FilterFold { input: ExprId, filter_fn: ExprId, init: ExprId, fold_fn: ExprId },

    /// map + filter + fold: full three-stage pipeline
    MapFilterFold { input: ExprId, map_fn: ExprId, filter_fn: ExprId, init: ExprId, fold_fn: ExprId },

    /// map + find: transform, then return first match
    MapFind { input: ExprId, map_fn: ExprId, find_fn: ExprId },

    /// filter + find: combined predicate search
    FilterFind { input: ExprId, filter_fn: ExprId, find_fn: ExprId },
}
```

`FusedPattern` has an `evaluate()` method that uses the `PatternExecutor` trait to execute fused operations in a single loop over the input collection. Each variant's evaluation logic mirrors what the individual patterns would do, but without intermediate allocations.

### PatternChain

A `PatternChain` represents a chain of patterns detected as candidates for fusion, ordered from innermost to outermost:

```rust
pub struct PatternChain {
    /// Patterns from innermost to outermost.
    pub links: Vec<ChainLink>,
    /// The original input expression (innermost .over).
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

Factory methods `FusionHints::two_pattern()` and `FusionHints::three_pattern()` create hints for common fusion depths.

## Fusion Rules

### map + filter

```ori
filter(over: map(over: xs, transform: f), predicate: p)
// Fuses to: MapFilter
// Single pass: for x in xs, let y = f(x), if p(y) then yield y
```

### filter + map

```ori
map(over: filter(over: xs, predicate: p), transform: f)
// Fuses to: FilterMap
// Single pass: for x in xs, if p(x) then yield f(x)
```

### map + fold

```ori
fold(over: map(over: xs, transform: f), init: i, op: g)
// Fuses to: MapFold
// Single pass: acc = i; for x in xs, acc = g(acc, f(x)); return acc
```

### filter + fold

```ori
fold(over: filter(over: xs, predicate: p), init: i, op: g)
// Fuses to: FilterFold
// Single pass: acc = i; for x in xs, if p(x) then acc = g(acc, x); return acc
```

### map + filter + fold

```ori
fold(over: filter(over: map(over: xs, transform: f), predicate: p), init: i, op: g)
// Fuses to: MapFilterFold
// Single pass: acc = i; for x in xs, let y = f(x), if p(y) then acc = g(acc, y); return acc
```

### map + find

```ori
find(over: map(over: xs, transform: f), where: p)
// Fuses to: MapFind
// Single pass: for x in xs, let y = f(x), if p(y) then return Some(y); return None
```

### filter + find

```ori
find(over: filter(over: xs, predicate: p1), where: p2)
// Fuses to: FilterFind
// Single pass: for x in xs, if p1(x) && p2(x) then return Some(x); return None
```

## When Fusion Applies

Conditions:
- Inner pattern's result is the `over` argument to the outer pattern
- Patterns are in the fusible combinations table above
- No side effects between patterns

## Benefits

1. **Memory** - No intermediate allocations (eliminated lists between stages)
2. **Cache** - Single pass is cache-friendly
3. **Iterations** - Each element is processed once through the entire pipeline

## Limitations

1. **Side effects** - Cannot fuse if intermediate results have side effects
2. **Debugging** - Fused patterns harder to step through
3. **Complexity** - Fusion detection and application adds compiler complexity
