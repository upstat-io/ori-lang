# Proposal: Parallel Iterators and Data Parallelism

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-05
**Affects:** Compiler, runtime, type system, standard library, spec (Clauses 13, 16, 22)

---

## Summary

This proposal adds data parallelism to Ori through parallel iterators and `for...parallel yield` comprehensions. It leverages Ori's unique combination of capability tracking, value semantics, and `Sendable` enforcement to provide compile-time safety guarantees that no other mainstream language achieves — Rayon-level ergonomics with Haskell-level safety, without requiring total purity or a borrow checker.

---

## Problem Statement

Ori has task-level concurrency (`parallel`, `spawn`, `nursery`) but no data-level parallelism. When a developer has a million items to transform, the only option is manual partitioning:

```ori
// Today: manual, verbose, error-prone
let $chunks = partition(items, size: items.len() div cpu_count())
let results = parallel(
    tasks: for chunk in chunks yield () -> {
        for item in chunk yield expensive(item)
    },
)
// ... then flatten results, handle errors, reassemble order
```

This is the equivalent of writing thread pools by hand when you want a parallel `for` loop. Every other language that has solved this (Rust/Rayon, Chapel, Java Streams, Haskell Strategies) provides a higher-level abstraction.

### What's Missing

1. **No way to parallelize iteration** — `.map()`, `.filter()`, `.fold()` are always sequential
2. **No work-stealing** — manual chunking gives static partitioning with no load balancing
3. **No parallel reductions** — `sum`, `count`, `min/max` over large collections are single-threaded
4. **No integration with `for...yield`** — the most ergonomic iteration form has no parallel variant

### Why Ori Is Uniquely Positioned

Most languages either lack safety enforcement (Java, C++, Julia) or require total purity (Haskell, Futhark). Ori has a middle path that no other production language offers:

| Language | Side-effect safety | Mechanism |
|----------|-------------------|-----------|
| Java Streams | None | Documentation says "don't" |
| C++ `par` | None | UB on violation |
| Rayon (Rust) | Partial | `Send`/`Sync` prevent data races but allow I/O |
| Haskell | Total | Requires pure functions |
| **Ori** | **Effect-checked** | **`uses` capabilities reject I/O at compile time** |

Ori's capability system can statically verify that parallel closures don't perform I/O, access the filesystem, or use network — while still permitting pure computation and `Print` for debugging. This is the "purity reflection" approach from academic research (Flix/ECOOP 2023), but built into an existing practical language.

---

## Design

### Part 1: `for...parallel yield` Comprehensions

The primary user-facing syntax extends `for...yield` with a `parallel` keyword:

```ori
// Sequential (existing)
let results = for x in items yield expensive(x)

// Parallel (new)
let results = for x in items parallel yield expensive(x)
```

#### Semantics

`for x in source parallel yield expr` desugars to:

1. Split `source` into work chunks using the `Splittable` protocol
2. Distribute chunks across a work-stealing thread pool
3. Each worker evaluates `expr` for its chunk elements, preserving element order within the chunk
4. Merge chunk results in source order
5. Collect into the target collection type

The result is identical to the sequential version — same elements, same order. Parallelism is an execution strategy, not a semantic change.

#### With Filters

```ori
// Parallel filter-map
let results = for x in items parallel if predicate(x) yield transform(x)
```

Filtering reduces parallelism efficiency (output size unknown, cannot pre-allocate), but is supported. Each chunk filters independently, then results are concatenated in order.

#### With Patterns

```ori
// Parallel over structured data
let names = for { name, age } in people parallel if age >= 18 yield name
```

Pattern matching works identically to sequential `for...yield`.

#### Capability Restriction

The body of a `for...parallel yield` shall have no environmental capabilities except `Print`:

```ori
// OK: pure computation
let squares = for x in data parallel yield x ** 2

// OK: Print allowed (for debugging)
let results = for x in data parallel yield {
    dbg(value: x)
    expensive(x)
}

// ERROR: Http not allowed in parallel body
let pages = for url in urls parallel yield fetch(url) uses Http
//                                        ^^^^^^^^^^^^^^^^^^^^^^^^
// error[E0720]: parallel body shall not use capability `Http`
//   = help: use `parallel(tasks: ...)` for concurrent I/O
```

This is the key safety property. The type checker rejects parallel bodies that use `Http`, `FileSystem`, `Random`, `Clock`, or any other environmental capability. This eliminates:
- Non-determinism from I/O ordering
- Resource contention (file handles, connections)
- Side-effect ordering dependencies

`Print` is exempted because it is synchronized and commonly needed for debugging. `Suspend` is not required — parallel iteration uses OS thread parallelism, not cooperative task scheduling.

#### Type of `for...parallel yield`

The type is the same as the sequential equivalent. `for x in [T] parallel yield U` produces `[U]`.

### Part 2: The `Splittable` Trait

Parallel iteration requires a way to divide work. The `Splittable` trait defines this:

```ori
trait Splittable: Iterable {
    // Split into two roughly equal halves.
    // Returns None if the collection is too small to split further.
    @split (self) -> Option<(Self, Self)>
}
```

#### Standard Implementations

| Type | Split Strategy | Indexed? |
|------|---------------|----------|
| `[T]` | Split at midpoint | Yes |
| `[T, max N]` | Split at midpoint | Yes |
| `{K: V}` | Split entries | No |
| `Set<T>` | Split elements | No |
| `Range<int>` | Split range at midpoint | Yes |
| `str` | Split at character boundary near midpoint | No |

#### Indexed Splitting

An extended trait provides position-aware splitting for more efficient operations:

```ori
trait IndexedSplittable: Splittable {
    @len (self) -> int
    @split_at (self, index: int) -> (Self, Self)
}
```

Lists, fixed-capacity lists, and integer ranges implement `IndexedSplittable`. This enables:
- Pre-allocated result buffers (exact output size known)
- Efficient `zip` (synchronized splitting)
- Position-aware operations (`enumerate`)

#### User Types

User-defined collections can implement `Splittable` to participate in parallel iteration:

```ori
type Matrix<T> = { rows: [[T]] }

impl Matrix<T>: Splittable where T: Sendable {
    @split (self) -> Option<(Self, Self)> = {
        if self.rows.len() < 2 then None
        else {
            let $mid = self.rows.len() div 2
            Some((
                Matrix { rows: self.rows.take(count: mid) },
                Matrix { rows: self.rows.skip(count: mid) },
            ))
        }
    }
}
```

### Part 3: Parallel Iterator Adapters

For method-chain style, iterators gain a `.par()` adapter:

```ori
let result = items.iter().par().map(transform: expensive).collect()
```

`.par()` converts an `Iterator` into a `ParallelIterator` if the source implements `Splittable`. The parallel iterator provides these adapters:

#### Transformations (order-preserving)

```ori
items.iter().par().map(transform: f)              // parallel map
items.iter().par().filter(predicate: f)            // parallel filter
items.iter().par().filter_map(transform: f)        // parallel filter + map
items.iter().par().flat_map(transform: f)           // parallel flat_map
```

All transformation adapters preserve element order in the output.

#### Reductions (unordered)

Reductions require an associative combining operation. The runtime splits work into chunks, reduces each chunk, then merges results in a binary tree:

```ori
// Built-in reductions
items.iter().par().sum()                            // requires Add + Default
items.iter().par().count()                          // always available
items.iter().par().min()                            // requires Comparable
items.iter().par().max()                            // requires Comparable
items.iter().par().any(predicate: f)                // short-circuits
items.iter().par().all(predicate: f)                // short-circuits

// Custom reduction
items.iter().par().reduce(
    identity: 0,
    op: (a, b) -> a + b,
)

// Fold + merge (for non-commutative accumulation)
items.iter().par().fold(
    identity: () -> new_accumulator(),
    fold: (acc, item) -> acc.add(item),
    merge: (left, right) -> left.merge(right),
)
```

The `reduce` operator requires associativity: `op(op(a, b), c) == op(a, op(b, c))`. This is not checked statically — incorrect associativity produces implementation-defined (but deterministic per run) results. A future lint may warn about suspicious operators.

The `fold` three-function pattern handles the general case: each worker folds its chunk independently, then pairs of results are merged up a tree. This supports accumulator types that differ from the element type (e.g., building a histogram from integers).

#### Search (with ordering variants)

```ori
items.iter().par().find_any(predicate: f)           // any match, fastest
items.iter().par().find_first(predicate: f)         // first in source order
items.iter().par().find_last(predicate: f)          // last in source order
```

`find_any` returns the first match found by any worker — non-deterministic but fastest. `find_first` and `find_last` guarantee source-order results at additional synchronization cost.

#### Collection

```ori
let list: [int] = items.iter().par().map(transform: f).collect()
let map: {str: int} = pairs.iter().par().map(transform: f).collect()
```

`.collect()` on a parallel iterator uses the same `Collect` trait as sequential iterators. For `IndexedSplittable` sources with order-preserving adapters, the runtime pre-allocates the result buffer.

### Part 4: Capability and Safety Rules

#### Rule 1: No Environmental Capabilities in Parallel Bodies

Closures passed to parallel adapters (`.map`, `.filter`, `.fold`, etc.) shall not declare or use environmental capabilities:

```ori
// Allowed capabilities in parallel closures:
// - (none)         Pure computation
// - uses Print     Debugging output (synchronized)

// Disallowed:
// - uses Http, FileSystem, Random, Clock, Cache, Logger, Env
// - uses Suspend (parallel iteration is thread-based, not task-based)
// - uses Unsafe
// - uses FFI(*)
```

The rationale for each:
- **`Http`/`FileSystem`/`Cache`**: I/O ordering would be non-deterministic
- **`Random`**: Results would depend on thread scheduling. (Future: per-worker seeded `Random` may be allowed)
- **`Clock`**: Timing would vary by scheduling
- **`Suspend`**: Parallel iteration uses OS threads, not cooperative tasks — mixing models creates deadlock risk
- **`Unsafe`/`FFI`**: Cannot verify thread safety of foreign code

#### Rule 2: Captured Values Must Be Sendable

All values captured by parallel closures shall implement `Sendable`:

```ori
let $config = load_config()  // config: Config, Config: Sendable
let results = for x in items parallel yield process(item: x, config: config)
// OK: config is Sendable, captured by value

let $handle = open_file()  // handle: FileHandle, NOT Sendable
let results = for x in items parallel yield format(item: x, out: handle)
// ERROR: FileHandle is not Sendable
```

This reuses the existing `Sendable` infrastructure from the channels proposal.

#### Rule 3: Parallel Bodies Are Deterministic

Given the above rules, the compiler guarantees: if the sequential version produces result `R`, the parallel version produces result `R` (for order-preserving adapters) or an equivalent result under the reduction's associativity contract (for reductions).

This is a stronger guarantee than any other mainstream language provides.

### Part 5: Work-Stealing Runtime

#### Thread Pool

Parallel iteration uses a work-stealing thread pool separate from the cooperative task scheduler. The pool is:

- **Created lazily** on first parallel operation
- **Sized to CPU count** by default (`$cpu_count` threads)
- **Shared across parallel operations** within a program (but not blocking — see nesting)
- **Configurable** via the `Parallel` capability handler

```ori
// Default: use all cores
let results = for x in items parallel yield f(x)

// Custom: limit parallelism
with Parallel = Parallel.with_threads(count: 4) in {
    let results = for x in items parallel yield f(x)
}

// Testing: force sequential execution
with Parallel = Sequential in {
    let results = for x in items parallel yield f(x)
    // Runs sequentially — deterministic, same result
}
```

#### Splitting Strategy

The runtime uses recursive bisection with work stealing:

1. If the collection is smaller than `min_batch_size`, process sequentially
2. Split the collection into two halves via `Splittable.split()`
3. Push one half onto the local work deque
4. Process the other half immediately
5. Idle workers steal from other workers' deques
6. Continue splitting stolen work until below threshold

`min_batch_size` defaults to `source.len() / (cpu_count * 4)` (enough chunks for load balancing without excessive overhead). Users can override:

```ori
// Explicit batch size for expensive per-item work
let results = items.iter().par().with_min_batch(size: 1).map(transform: very_expensive).collect()

// Larger batches for cheap per-item work
let results = items.iter().par().with_min_batch(size: 10000).map(transform: cheap).collect()
```

#### ARC-Aware Optimization

Ori's ARC memory model enables specific optimizations:

- **Value types** (`T: Value`): Distributed to workers with zero ARC overhead — bitwise copy
- **Shared immutable data**: Workers share read-only captured data via a single ARC increment per worker (not per item)
- **COW collections**: If a parallel operation produces a new collection, COW ensures the source is not modified — parallel reads are safe without synchronization

### Part 6: Error Handling

#### Panics

If any worker panics during parallel iteration:

1. The panic is captured (not propagated immediately)
2. All other workers are signaled to stop (cooperative — they check at split boundaries)
3. Workers finish their current item, then stop
4. The captured panic is re-raised in the calling thread

Only the first panic is propagated. This matches `parallel()` semantics.

#### Fallible Operations

For operations that return `Result`, use `try_` variants:

```ori
// Sequential fallible
let results: Result<[Output], Error> = try {
    for x in items yield fallible(x)?
}

// Parallel fallible
let results: Result<[Output], Error> =
    items.iter().par().try_map(transform: fallible).collect()
```

`try_map` propagates the first `Err` and cancels remaining work. The cancellation is cooperative — workers check for early termination at split boundaries.

Available `try_` adapters:
- `try_map(transform: (T) -> Result<U, E>)` — parallel map with early exit on error
- `try_filter(predicate: (T) -> Result<bool, E>)` — parallel filter with early exit
- `try_fold(identity:, fold:, merge:)` — parallel fold with early exit
- `try_for_each(action: (T) -> Result<void, E>)` — parallel iteration with early exit

### Part 7: `for...parallel do`

The `do` variant executes side-effect-free work without collecting results:

```ori
// Parallel for_each (void body)
for x in items parallel do process(x)
```

This is equivalent to `items.iter().par().for_each(action: x -> process(x))` and follows the same capability restrictions.

NOTE: Since parallel bodies cannot have environmental capabilities, `for...parallel do` is primarily useful for CPU-bound mutation of captured mutable collections via the body's return value, or for `Print`-based debugging.

### Part 8: Nested and Composed Parallelism

#### Nested Parallel Iteration

```ori
// Outer parallel, inner sequential
let results = for row in matrix parallel yield {
    for x in row yield transform(x)  // sequential inner loop
}

// Outer parallel, inner parallel
let results = for row in matrix parallel yield {
    for x in row parallel yield transform(x)  // nested parallelism
}
```

Nested parallelism uses the same thread pool. Inner parallel operations are executed by the worker thread and may steal work from the shared pool, but do not spawn additional threads. This prevents thread explosion.

#### Composition with Sequential Adapters

Parallel and sequential adapters can be mixed in a chain:

```ori
items.iter()
    .filter(predicate: cheap_check)     // sequential pre-filter (cheap)
    .par()                               // switch to parallel
    .map(transform: expensive)           // parallel map (expensive)
    .collect()                           // collect results
```

`.par()` is the entry point to parallelism. After `.par()`, all subsequent adapters are parallel until `.collect()` or another terminal operation.

There is no `.seq()` to switch back — once parallel, stay parallel. If sequential post-processing is needed, collect first:

```ori
let intermediate = items.iter().par().map(transform: expensive).collect()
let final_result = intermediate.iter().take(count: 10).collect()
```

---

## Interaction with Existing Features

### `parallel()` vs `for...parallel yield`

| Feature | `parallel(tasks: [...])` | `for...parallel yield` |
|---------|--------------------------|------------------------|
| **Unit of work** | Independent closures | Items in a collection |
| **Use case** | Heterogeneous concurrent tasks | Homogeneous data transformation |
| **Capabilities** | `uses Suspend` + any capability | Pure + `Print` only |
| **Scheduling** | Cooperative task scheduler | OS thread work-stealing |
| **Error handling** | `[Result<T, E>]` per task | First error cancels all |
| **Ordering** | Result order = task order | Result order = source order |

`parallel()` is for I/O-bound concurrent work. `for...parallel yield` is for CPU-bound data parallelism. They serve different purposes and use different runtime mechanisms.

### `Suspend` Capability

`for...parallel yield` does NOT require `uses Suspend`. Parallel iteration uses OS threads managed by the work-stealing pool, not cooperative tasks managed by the Ori scheduler. This is intentional:

1. No function coloring penalty — parallel iteration works in non-`Suspend` contexts
2. No scheduler dependency — CPU-bound work doesn't need cooperative scheduling
3. No interaction with the task cancellation model — parallel cancellation is handled internally by the thread pool

### `for...yield` Desugaring

```ori
// Sequential
for x in items yield f(x)
// desugars to: items.iter().map(transform: x -> f(x)).collect()

// Parallel
for x in items parallel yield f(x)
// desugars to: items.iter().par().map(transform: x -> f(x)).collect()
```

The `parallel` keyword inserts a `.par()` call in the desugaring chain.

---

## Prior Art

| Language | Approach | Safety | Ordering | Ergonomics |
|----------|----------|--------|----------|------------|
| Rust (Rayon) | `.par_iter()` library | Send/Sync (no I/O check) | Preserved by default | Excellent |
| Java | `.parallel()` stream | None | Preserved (expensive) | Good syntax, bad semantics |
| C++ | `std::execution::par` | None (UB on violation) | Implementation-defined | Good syntax, dangerous |
| Haskell | `parMap rdeepseq` | Total purity | Deterministic | Poor ergonomics |
| Chapel | `forall` loop | Convention only | No guarantee | Excellent |
| Futhark | `map`/`reduce` | Total purity + uniqueness | Deterministic | Good (domain-specific) |
| **Ori** | **`for...parallel yield`** | **Effect-checked** | **Preserved by default** | **Excellent** |

Ori is the first language to combine all four: ergonomic syntax, compile-time effect safety, order preservation, and general-purpose applicability.

---

## Grammar Changes

```ebnf
(* Extend for_expression *)
for_yield_expression
  = "for" pattern "in" expression [ "parallel" ] [ "if" expression ] "yield" expression .

for_do_expression
  = "for" pattern "in" expression [ "parallel" ] [ "if" expression ] "do" expression .

(* Labels compose *)
labeled_for
  = "for" ":" identifier pattern "in" expression [ "parallel" ] ... .
```

`parallel` is a new context-sensitive keyword in `for` expressions, appearing after the source expression and before `if`/`yield`/`do`.

---

## New Types and Traits

```ori
// In std.parallel (new module)

trait Splittable: Iterable {
    @split (self) -> Option<(Self, Self)>
}

trait IndexedSplittable: Splittable {
    @len (self) -> int
    @split_at (self, index: int) -> (Self, Self)
}
```

### ParallelIterator (built-in, not a user-implementable trait)

`ParallelIterator` is a built-in type returned by `.par()`. It provides parallel versions of iterator adapters. Users do not implement `ParallelIterator` directly — they implement `Splittable` on their collections, and `.par()` handles the rest.

```ori
// Methods on ParallelIterator<T>

// Transformations (order-preserving)
@map<U> (self, transform: (T) -> U) -> ParallelIterator<U>
@filter (self, predicate: (T) -> bool) -> ParallelIterator<T>
@filter_map<U> (self, transform: (T) -> Option<U>) -> ParallelIterator<U>
@flat_map<U> (self, transform: (T) -> impl Iterable where Item == U) -> ParallelIterator<U>

// Reductions
@reduce (self, identity: T, op: (T, T) -> T) -> T
@fold<A> (self, identity: () -> A, fold: (A, T) -> A, merge: (A, A) -> A) -> A
@sum (self) -> T where T: Add + Default
@count (self) -> int
@min (self) -> Option<T> where T: Comparable
@max (self) -> Option<T> where T: Comparable
@any (self, predicate: (T) -> bool) -> bool
@all (self, predicate: (T) -> bool) -> bool

// Search
@find_any (self, predicate: (T) -> bool) -> Option<T>
@find_first (self, predicate: (T) -> bool) -> Option<T>
@find_last (self, predicate: (T) -> bool) -> Option<T>

// Fallible
@try_map<U, E> (self, transform: (T) -> Result<U, E>) -> Result<ParallelIterator<U>, E>
@try_for_each<E> (self, action: (T) -> Result<void, E>) -> Result<void, E>

// Terminal
@collect<C: Collect<T>> (self) -> C
@for_each (self, action: (T) -> void) -> void

// Tuning
@with_min_batch (self, size: int) -> ParallelIterator<T>
```

---

## Spec Changes Required

| Clause | Change |
|--------|--------|
| 13 (Traits) | Add `Splittable`, `IndexedSplittable` trait definitions |
| 14 (Expressions) | Add `.par()` method on iterators |
| 16 (Control Flow) | Extend `for...yield` and `for...do` with `parallel` keyword |
| 22 (Concurrency) | Add section on data parallelism (distinct from task concurrency) |
| Annex A (Grammar) | Add `parallel` to for-expression productions |
| Annex C (Built-ins) | Add `ParallelIterator` methods, `Parallel` capability |

---

## Error Messages

```
error[E0720]: parallel body shall not use capability `Http`
  --> example.ori:3:45
   |
 3 |     let pages = for url in urls parallel yield fetch(url)
   |                                                ^^^^^^^^^^
   = note: parallel iteration requires pure computation
   = help: for concurrent I/O, use `parallel(tasks: [...])`

error[E0721]: type `FileHandle` does not implement `Sendable`
  --> example.ori:5:50
   |
 5 |     for x in items parallel yield write(handle, x)
   |                                         ^^^^^^
   = note: values captured by parallel closures must implement Sendable

error[E0722]: type `LinkedList<int>` does not implement `Splittable`
  --> example.ori:3:20
   |
 3 |     for x in my_list parallel yield f(x)
   |                      ^^^^^^^^
   = help: implement `Splittable` for `LinkedList<int>` or collect into a `[int]` first

error[E0723]: `uses Suspend` is not permitted in parallel bodies
  --> example.ori:4:5
   |
 4 |     for x in items parallel yield async_work(x)
   |                                   ^^^^^^^^^^^^^
   = note: parallel iteration uses thread-based parallelism, not cooperative tasks
   = help: for concurrent async work, use `nursery` with `n.spawn()`
```

---

## Examples

### Basic Parallel Map

```ori
@process_images (paths: [str]) -> [Image] =
    for path in paths parallel yield {
        let $raw = embed(path)
        decode_image(data: raw)
    }
```

### Parallel Reduction

```ori
@sum_of_squares (data: [float]) -> float =
    data.iter().par().map(transform: x -> x ** 2).reduce(identity: 0.0, op: (a, b) -> a + b)
```

### Parallel with Pre-filter

```ori
@find_primes (limit: int) -> [int] =
    for n in 2..limit parallel if is_prime(n) yield n
```

### Parallel Fold (Histogram)

```ori
@histogram (words: [str]) -> {str: int} =
    words.iter().par().fold(
        identity: () -> {},
        fold: (acc, word) -> {
            let $count = acc[word] ?? 0
            { ...acc, [word]: count + 1 }
        },
        merge: (left, right) -> {
            for (key, value) in right do {
                let $existing = left[key] ?? 0
                left = { ...left, [key]: existing + value }
            }
            left
        },
    )
```

### Testing with Sequential Fallback

```ori
@test_parallel_determinism tests @process_images () -> void =
    with Parallel = Sequential in {
        // Runs sequentially for deterministic testing
        let $result = process_images(paths: test_paths)
        assert_eq(actual: result.len(), expected: 100)
    }
```

### Custom Splittable Type

```ori
type Grid<T> = { width: int, height: int, cells: [T] }

impl<T: Sendable> Grid<T>: Splittable {
    @split (self) -> Option<(Self, Self)> = {
        if self.height < 2 then None
        else {
            let $mid = self.height div 2
            let $top_cells = self.cells.take(count: mid * self.width)
            let $bottom_cells = self.cells.skip(count: mid * self.width)
            Some((
                Grid { width: self.width, height: mid, cells: top_cells },
                Grid { width: self.width, height: self.height - mid, cells: bottom_cells },
            ))
        }
    }
}

// Now works with parallel iteration
let $result = for cell in grid parallel yield transform(cell)
```

---

## Performance Considerations

### When to Use Parallel Iteration

Parallel iteration has overhead (thread synchronization, work distribution, result merging). It is beneficial when:

- **Per-item cost is high**: Image processing, cryptographic operations, complex math
- **Collection is large**: Thousands of items or more
- **Work is uniform**: Similar cost per item (work-stealing handles some imbalance)

It is NOT beneficial when:
- **Per-item cost is trivial**: Simple arithmetic on small collections
- **Collection is small**: < ~1000 items (overhead dominates)
- **I/O bound**: Use `parallel(tasks: [...])` instead

The runtime may fall back to sequential execution for small collections (below `min_batch_size`). This is transparent to the user.

### Memory Usage

Parallel iteration may allocate:
- Per-worker result buffers (for order-preserving operations)
- Work deque entries (lightweight — pointers to split ranges)
- Merged result collection (same size as sequential)

Peak memory is approximately `O(result_size + workers * chunk_result_size)`.

---

## Future Extensions (Not in This Proposal)

1. **Parallel `scan`** (prefix sum) — requires Blelloch's algorithm, complex but valuable
2. **GPU offload** — `for x in data parallel(gpu) yield f(x)` for SIMD/GPU execution
3. **Per-worker `Random`** — deterministic per-worker seeded RNG for stochastic algorithms
4. **Async parallel** — combining `Suspend` with parallelism for mixed I/O + CPU workloads
5. **`par_chunks`** — iterate over fixed-size chunks in parallel
6. **Parallel `sort`** — merge sort with parallel merge step

---

## Summary

| Feature | Description |
|---------|-------------|
| `for x in items parallel yield expr` | Parallel comprehension — order-preserving, effect-checked |
| `for x in items parallel do expr` | Parallel iteration without collection |
| `items.iter().par().map(f).collect()` | Method-chain parallel iteration |
| `Splittable` trait | User-extensible work division protocol |
| `IndexedSplittable` trait | Position-aware splitting for efficient collection |
| `ParallelIterator<T>` | Built-in parallel adapter type |
| `with Parallel = Sequential in { ... }` | Deterministic testing via capability |
| Capability restriction | Compile-time rejection of I/O in parallel bodies |
| Work-stealing runtime | Adaptive load balancing, configurable thread count |
| `try_map`, `try_for_each` | Early-exit error handling in parallel |
