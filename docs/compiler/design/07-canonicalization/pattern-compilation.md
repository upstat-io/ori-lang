---
title: "Pattern Compilation"
description: "Ori Compiler Design — Decision Tree Construction and Exhaustiveness Checking"
order: 702
section: "Canonicalization"
---

# Pattern Compilation

## What Is Pattern Compilation?

When a programmer writes a `match` expression, they describe *what* they want to test — "if this value is a `Circle`, extract its radius; if it's a `Rect`, extract width and height." But this declarative description says nothing about *how* to test efficiently at runtime. Should the compiler test the variant tag first, then extract fields? Should it test fields across multiple arms simultaneously? What if several arms share a common prefix? **Pattern compilation** is the phase that answers these questions by transforming declarative pattern-matching syntax into an efficient, executable test plan.

The problem is older than most programmers realize. Fortran's computed GOTO (1957) was an early form of multi-way branching, and Lisp's `cond` (1960) introduced sequential condition testing. But the modern problem — compiling algebraic data type patterns into efficient decision procedures — emerged with ML in the late 1970s. When Robin Milner introduced algebraic data types and pattern matching to ML, compilers needed algorithms to turn nested pattern matches into sequences of runtime tests that were both correct (no case missed, no case tested twice) and efficient (minimal number of tests for any input).

### The Core Challenge

Consider a match on a pair of values:

```ori
match (shape, color) {
    (Circle(r), Red) -> ...,
    (Rect(w, h), _) -> ...,
    (_, Blue) -> ...,
}
```

A naive approach tests arms top-to-bottom: try arm 1, if it fails try arm 2, if that fails try arm 3. But this duplicates work — if the first arm fails because `color` isn't `Red`, the second arm re-tests `shape` even though we already know it's a `Circle`. With N arms and M columns, naive backtracking can perform up to N×M tests per match.

The goal of pattern compilation is to produce a **decision tree** that shares tests across arms, testing each value position at most once on any execution path. The tree should also be *complete* (handle every possible input) and *irredundant* (every arm is reachable by some input).

### Classical Approaches

Three major families of algorithms have emerged for compiling pattern matches:

**Backtracking automata (Augustsson 1985).** Lennart Augustsson's algorithm for the LML compiler processes the pattern matrix left-to-right, one column at a time. For each column, it groups arms by their outermost constructor and recurses on each group. When a wildcard appears in the tested column, the corresponding arm is included in all groups. This produces compact code but can duplicate arm bodies — if a wildcard arm spans multiple constructor groups, its body is compiled once per group. Augustsson's approach is simple to implement and produces good code for most practical patterns, but the body duplication can cause code size blowup in pathological cases.

**Decision trees (Maranget 2008).** Luc Maranget's algorithm, described in "[Compiling Pattern Matching to Good Decision Trees](http://moscova.inria.fr/~maranget/papers/ml05e-maranget.pdf)," generalizes Augustsson's approach with two key innovations. First, it selects the *best* column to test at each step using a heuristic, rather than always going left-to-right. Second, it handles the "default matrix" (rows with wildcards at the tested column) explicitly, producing a default edge in the decision tree rather than duplicating arm bodies. The result is a tree where each arm's body appears exactly once, referenced by index. This is the algorithm Ori uses.

**Backtracking with sharing (Pettersson 1992).** Krister Pettersson's approach generates a DAG (directed acyclic graph) rather than a tree, sharing common subtrees between arms. This can produce smaller code than Maranget's pure trees, but the DAG structure is harder to analyze for exhaustiveness and harder to generate efficient code for in an SSA-based backend.

The tradeoff between these approaches is essentially code size vs. test count vs. implementation complexity:

| Approach | Body duplication | Test sharing | Exhaustiveness | Complexity |
|----------|-----------------|--------------|----------------|------------|
| Backtracking (Augustsson) | Yes — wildcards duplicated | Partial | Separate pass needed | Simple |
| Decision trees (Maranget) | No — arms by index | Full — column heuristic | Falls out naturally | Moderate |
| DAGs (Pettersson) | Shared subtrees | Full | Harder to check | Higher |

### Exhaustiveness and Usefulness

Pattern compilation is intimately connected to two static analyses:

**Exhaustiveness checking** verifies that every possible input value is handled by at least one arm. A non-exhaustive match can crash at runtime when the unhandled case is encountered. Languages differ in how they handle this — some require exhaustive matches (ML, Haskell, Ori), some insert implicit default arms (Scala), and some silently fall through (C's `switch`).

**Usefulness checking** (also called redundancy detection) verifies that every arm can be reached by some input. A redundant arm is dead code — it can never execute because earlier arms catch all its cases. Redundant arms are usually bugs (the programmer thought they were adding a case, but it's already covered).

Both analyses fall out naturally from the Maranget algorithm: a `Fail` node in the decision tree indicates non-exhaustiveness, and an arm index that never appears in any `Leaf` node is redundant.

## What Makes Ori's Pattern Compilation Distinctive

### Two-Phase Architecture: Flatten Then Compile

Ori separates pattern compilation into two distinct phases. First, **flattening** converts the arena-allocated AST patterns (`MatchPattern` with `MatchPatternId` indices into `ExprArena`) into self-contained `FlatPattern` values that own their data. Then **compilation** operates purely on `FlatPattern` values without any arena dependencies. This clean separation means the compilation algorithm never touches the source arena, making it independent of AST lifetimes and testable in isolation.

### Path-Based Value Navigation

Rather than generating code that destructures values into local variables, Ori's decision trees reference sub-values via **scrutinee paths** — sequences of `PathInstruction` values like `[TagPayload(0), TupleIndex(1)]` that describe how to navigate from the root scrutinee to a nested sub-value. Both the evaluator and LLVM codegen interpret these paths to extract values at runtime. This indirection avoids materializing intermediate destructured values and enables the same decision tree to be consumed by multiple backends.

### Arc-Shared Trees Across Backends

Decision trees are wrapped in `Arc` and stored in a `DecisionTreePool`. The evaluator and LLVM codegen share the exact same tree instances — no copying, no serialization. For complex match expressions with nested patterns and guards, decision trees can be substantial data structures, and avoiding duplication matters.

### Integrated Exhaustiveness via Tree Walking

Rather than running a separate exhaustiveness algorithm on the source patterns (which would duplicate much of the Maranget algorithm's logic), Ori's exhaustiveness checker walks the *compiled* decision tree. A reachable `Fail` node means non-exhaustiveness; an arm index that appears in no `Leaf` node means redundancy. This reuse is sound because the decision tree faithfully encodes the pattern matrix's coverage — the tree and the source patterns accept exactly the same set of values.

### Nested Type Tracking

The exhaustiveness checker tracks types at each scrutinee path via `path_types: FxHashMap<ScrutineePath, Idx>`. When an `EnumTag` switch edge is entered, the checker records the types of the variant's fields at their child paths. This enables exhaustiveness checking at *arbitrary nesting depth* — not just the root switch, but nested enum patterns like `Ok(Some(x))` where the inner `Option` must also be exhaustive. Path entries are cleaned up when backtracking to prevent context leakage between sibling branches.

## The Maranget Algorithm in Ori

### Pattern Matrix

The algorithm operates on a **pattern matrix** where each row is a match arm and each column corresponds to a scrutinee path:

```rust
pub struct PatternRow {
    pub patterns: Vec<FlatPattern>,           // One per column
    pub arm_index: usize,                     // Original arm position
    pub guard: Option<CanId>,                 // Guard expression (if any)
    pub bindings: Vec<(Name, ScrutineePath)>, // Accumulated variable bindings
}
```

Initially, a single-scrutinee match has one column with an empty path. A multi-parameter function (compiled from multi-clause syntax) has N columns with `TupleIndex(i)` paths. As the algorithm recurses, columns are replaced by sub-columns when destructuring constructors.

### FlatPattern

The self-contained pattern representation used by the compilation algorithm:

```rust
pub enum FlatPattern {
    Wildcard,
    Binding(Name),
    LitInt(i64), LitFloat(u64), LitBool(bool), LitStr(Name), LitChar(char),
    Variant { variant_name: Name, variant_index: u32, fields: Vec<FlatPattern> },
    Tuple(Vec<FlatPattern>),
    Struct { fields: Vec<(Name, FlatPattern)> },
    List { elements: Vec<FlatPattern>, rest: Option<Name> },
    Range { start: Option<i64>, end: Option<i64>, inclusive: bool },
    Or(Vec<FlatPattern>),
    At { name: Name, inner: Box<FlatPattern> },
}
```

Flattening resolves arena references, extracts literal values, and looks up variant discriminant indices via the type `Pool`. After flattening, no arena or pool access is needed during compilation.

### Algorithm Steps

The compilation algorithm is a recursive function on the pattern matrix:

**Base case 1 — Empty matrix:** No arms match. Return `DecisionTree::Fail`.

**Base case 2 — First row all wildcards:** The first arm matches everything remaining. If it has a guard, return a `Guard` node (on failure, compile the remaining rows). Otherwise return a `Leaf` node with the arm's accumulated bindings.

**Recursive case:**

1. **Pick the best column** — count distinct constructors at each column; pick the one with the most. This heuristic maximizes branching power, splitting the matrix into the most subsets.

2. **Single-constructor check** — if the column contains only tuples or structs (plus wildcards), there's only one possible shape. Skip the test entirely and decompose directly, replacing the column with sub-columns for each field. This avoids generating redundant switch nodes.

3. **Collect test values** — extract the distinct constructors from the chosen column: variant tags, literal values, list lengths.

4. **Specialize for each test value** — for each constructor, build a sub-matrix containing only the rows compatible with that constructor, with the tested column replaced by sub-columns for the constructor's fields. Wildcards at the tested column are compatible with *every* constructor and propagate to all sub-matrices.

5. **Build default matrix** — collect rows where the tested column is a wildcard (no specific constructor). These form the default edge of the switch, handling values not covered by any explicit constructor.

6. **Recurse** — compile each sub-matrix and the default matrix recursively. Assemble the results into a `Switch` node.

### Worked Example

Consider matching a list of known shapes:

```ori
match shape {
    Circle(r) if r > 0 -> pi * r ** 2,
    Rect(w, h) -> w * h,
    Circle(r) -> 0,
}
```

**Step 1: Build pattern matrix** (1 column, 3 rows):

| Row | Pattern | Guard |
|-----|---------|-------|
| 0 | `Variant(Circle, [Binding(r)])` | `r > 0` |
| 1 | `Variant(Rect, [Binding(w), Binding(h)])` | — |
| 2 | `Variant(Circle, [Binding(r)])` | — |

**Step 2: Pick column** — column 0 has 2 distinct constructors (`Circle`, `Rect`). Only one column, so it's chosen.

**Step 3: Collect test values** — `{Tag(Circle, 0), Tag(Rect, 1)}`.

**Step 4: Specialize for `Circle`** — rows 0 and 2 match. The variant's single field creates a new column at path `[TagPayload(0)]`:

| Row | Pattern | Guard | Bindings |
|-----|---------|-------|----------|
| 0 | `Binding(r)` | `r > 0` | — |
| 2 | `Binding(r)` | — | — |

Both patterns are wildcards → base case 2. Row 0 has a guard → `Guard` node. On guard failure, compile row 2 → `Leaf(2)`.

**Step 5: Specialize for `Rect`** — only row 1 matches. Two fields create columns at `[TagPayload(0)]` and `[TagPayload(1)]`:

| Row | Pattern 0 | Pattern 1 | Bindings |
|-----|-----------|-----------|----------|
| 1 | `Binding(w)` | `Binding(h)` | — |

All wildcards → `Leaf(1)`.

**Step 6: Default matrix** — no rows have wildcards at column 0, so no default edge.

**Result:**

```text
Switch(path=[], test_kind=EnumTag)
├─ Tag(Circle) → Guard {
│    arm: 0,
│    bindings: [(r, [TagPayload(0)])],
│    guard: <r > 0>,
│    on_fail: Leaf { arm: 2, bindings: [(r, [TagPayload(0)])] }
│  }
└─ Tag(Rect) → Leaf {
     arm: 1,
     bindings: [(w, [TagPayload(0)]), (h, [TagPayload(1)])]
   }
```

## DecisionTree Type

The compiled decision tree is a recursive enum:

```rust
pub enum DecisionTree {
    /// Test a value and branch based on the result.
    Switch {
        path: ScrutineePath,
        test_kind: TestKind,
        edges: Vec<(TestValue, DecisionTree)>,
        default: Option<Box<DecisionTree>>,
    },

    /// Pattern matched — execute this arm's body.
    Leaf {
        arm_index: usize,
        bindings: Vec<(Name, ScrutineePath)>,
    },

    /// Pattern matched but a guard must be checked.
    /// If the guard fails, fall through to `on_fail`.
    Guard {
        arm_index: usize,
        bindings: Vec<(Name, ScrutineePath)>,
        guard: CanId,
        on_fail: Box<DecisionTree>,
    },

    /// No pattern matches — non-exhaustive error at runtime.
    Fail,
}
```

### Test Kinds and Values

Each `Switch` node tests a value at a specific path. The `TestKind` determines what kind of comparison to perform:

| TestKind | Tests For | Finite? | Example |
|----------|-----------|---------|---------|
| `BoolEq` | `true` / `false` | Yes (2 values) | `match flag { true -> ..., false -> ... }` |
| `IntEq` | Integer equality | No | `match n { 0 -> ..., 1 -> ..., _ -> ... }` |
| `StrEq` | String equality | No | `match cmd { "help" -> ..., _ -> ... }` |
| `FloatEq` | Float equality | No | `match f { 0.0 -> ..., _ -> ... }` |
| `CharEq` | Character equality | No | `match c { 'a' -> ..., _ -> ... }` |
| `IntRange` | Integer range membership | No | `match n { 0..10 -> ..., _ -> ... }` |
| `ListLen` | List length | No | `match xs { [] -> ..., [x] -> ..., _ -> ... }` |
| `EnumTag` | Enum variant tag | Yes (known variants) | `match opt { Some(x) -> ..., None -> ... }` |

### Scrutinee Paths

A `ScrutineePath` is a sequence of navigation instructions from the root scrutinee to a nested sub-value:

```rust
pub enum PathInstruction {
    TagPayload(u32),   // Extract field i of an enum variant
    TupleIndex(u32),   // Extract element i of a tuple
    StructField(u32),  // Extract field i of a struct (by position)
    ListElement(u32),  // Extract element i of a list
    ListRest(u32),     // Extract the rest of a list starting at index i
}
```

For example, matching `Ok(Point { x, y })` where `x` is bound at path `[TagPayload(0), StructField(0)]` — navigate into the `Ok` variant's payload (field 0), then into the struct's first field.

## DecisionTreePool

Decision trees are stored in a pool and referenced by `DecisionTreeId`:

```rust
pub struct DecisionTreePool {
    trees: Vec<SharedDecisionTree>,  // SharedDecisionTree = Arc<DecisionTree>
}
```

The `Arc` wrapping enables both the evaluator and LLVM codegen to share tree instances without copying. The evaluator clones the `Arc` when entering a match to release its borrow on `self`; the ARC pipeline receives the same shared reference during codegen.

## Multi-Clause Function Compilation

When multiple function clauses share a name, the lowerer synthesizes a match expression and compiles it through the same algorithm:

```ori
@fib (0: int) -> int = 0
@fib (1: int) -> int = 1
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)
```

The lowerer creates a pattern matrix with one column per parameter. For a single-parameter function, the path is `[]` (test at root). For multi-parameter functions, each column gets a `TupleIndex(i)` path, and the algorithm tests each parameter position independently.

This unification means there is exactly one code path for pattern compilation — the same Maranget algorithm handles both explicit `match` expressions and multi-clause function definitions.

## Exhaustiveness Checking

After a decision tree is compiled, the `exhaustiveness` module walks the tree to detect two classes of problems.

### Algorithm

The checker performs a single depth-first walk of the decision tree, carrying three pieces of mutable state:

- **`reachable: Vec<bool>`** — one entry per arm, initially all `false`. Set to `true` when a `Leaf` or `Guard` node with that arm's index is visited.
- **`missing: Vec<String>`** — collects human-readable descriptions of uncovered patterns (e.g., `"false"`, `"Rect(_, _)"`, `"_"`).
- **`path_types: FxHashMap<ScrutineePath, Idx>`** — maps scrutinee paths to their types, enabling nested enum exhaustiveness. Initialized with the root scrutinee type; extended when entering variant edges; cleaned up when backtracking.

```text
walk(tree)
├─ Leaf { arm_index }      → mark reachable[arm_index] = true
├─ Guard { arm_index, on_fail }
│  ├─ mark reachable[arm_index] = true  (guard may succeed)
│  └─ walk(on_fail)                     (guard may fail)
├─ Fail                    → missing.push("_")
└─ Switch { path, test_kind, edges, default }
   ├─ for each (test_value, subtree) in edges:
   │  ├─ if EnumTag: insert child field types into path_types
   │  ├─ walk(subtree)
   │  └─ cleanup inserted path_types entries
   └─ if default exists: walk(default)
      else: check_missing_constructors(...)
```

### Non-Exhaustive Detection

A match is non-exhaustive when some runtime value has no matching arm. This is detected in two ways:

1. **Reachable `Fail` nodes** — a `Fail` node means the pattern matrix had a gap that no arm covers.
2. **Missing constructors** — a `Switch` with no `default` edge that doesn't cover all constructors of a finite type.

For finite types, the checker enumerates all constructors and reports the missing ones:

| Type | Exhaustive When |
|------|----------------|
| `bool` | Both `true` and `false` are covered |
| `Option<T>` | Both `None` and `Some(_)` are covered |
| `Result<T, E>` | Both `Ok(_)` and `Err(_)` are covered |
| User-defined enum | All variants covered (queried via `Pool`) |

For infinite types (`int`, `str`, `float`, ranges, list lengths), a `Switch` without a `default` (wildcard) is always non-exhaustive — the checker reports `"_"` as the missing pattern.

### Redundant Arm Detection

An arm is redundant when no path through the decision tree reaches it. After the walk completes, any arm whose `reachable` flag is still `false` is reported:

```ori
// Redundant: arm 3 is unreachable
match b {
    true -> "yes",
    false -> "no",
    _ -> "other",   // redundant — bool is fully covered
}
```

### Guard Semantics

Guards are treated conservatively: a guarded arm is marked reachable (the guard *may* succeed), but the `on_fail` subtree is also walked (the guard *may* fail). Guards are never considered exhaustive — the checker cannot statically evaluate arbitrary boolean expressions, so a match that relies on guards for coverage will still require a wildcard or complete constructor coverage.

### Nested Enum Exhaustiveness

The `path_types` map enables the checker to verify exhaustiveness at nested positions. When walking an `EnumTag` switch edge for variant `Some` at path `[]`, the checker:

1. Looks up the type at path `[]` (e.g., `Option<Result<int, str>>`)
2. Resolves the `Some` variant's field types (e.g., `[Result<int, str>]`)
3. Inserts `path_types[TagPayload(0)] = Result<int, str>`
4. Walks the subtree — if it contains a switch on the `Result` type at path `[TagPayload(0)]`, the checker can verify both `Ok` and `Err` are covered
5. Removes the inserted entry when backtracking

This nesting tracking also enables readable diagnostic messages. The checker maintains a `nesting` stack of wrapper strings (e.g., `["Ok({})", "Some({})"]`) that wrap missing patterns to produce messages like `"Ok(Some(None))"` rather than bare `"None"`.

### Uninhabited Variants

Variants with a `Never`-typed field are **uninhabited** — they cannot be constructed in any well-typed program. The exhaustiveness checker skips these when reporting missing patterns, avoiding false positives like "missing pattern: `Impossible(_, _)`" for variants that can never exist at runtime.

### PatternProblem Type

```rust
pub enum PatternProblem {
    NonExhaustive {
        match_span: Span,
        missing: Vec<String>,  // e.g., ["false"], ["Rect(_, _)"]
    },
    RedundantArm {
        arm_span: Span,
        match_span: Span,
        arm_index: usize,
    },
}
```

These are defined in `ori_ir::canon` so both `ori_canon` (producer) and `oric` (consumer) can access them without circular dependencies. Problems accumulate in `Lowerer.problems` and are returned in `CanonResult.problems`.

### Integration Points

Exhaustiveness checking runs in two contexts:

1. **`lower_match()`** — after compiling each match expression's decision tree
2. **`lower_multi_clause()`** — after synthesizing the match for multi-clause functions

The `check` command converts problems to diagnostics for user-facing output.

## Prior Art

### Wadler (1987) — The Foundational Algorithm

Philip Wadler's chapter in [*The Implementation of Functional Programming Languages*](https://www.microsoft.com/en-us/research/publication/the-implementation-of-functional-programming-languages/) introduced the standard approach to compiling pattern matching for lazy functional languages. Wadler showed how to transform a sequence of equations with patterns into nested `case` expressions, handling variable patterns, constructor patterns, and default cases. His formulation directly influenced the ML and Haskell communities and established the "pattern matrix" abstraction that all subsequent algorithms build on.

### Augustsson (1985) — Practical Compiler Integration

Lennart Augustsson's implementation in the [LML compiler](https://link.springer.com/chapter/10.1007/3-540-15975-4_48) was the first practical compiler to use systematic pattern compilation. His left-to-right column processing is simpler than Maranget's heuristic-based approach but can duplicate arm bodies when wildcards span multiple constructor groups. Many ML compilers (including early versions of SML/NJ) used variants of Augustsson's algorithm.

### Maranget (2008) — Good Decision Trees

Luc Maranget's "[Compiling Pattern Matching to Good Decision Trees](http://moscova.inria.fr/~maranget/papers/ml05e-maranget.pdf)" refined the algorithm with a column selection heuristic that minimizes tree size. The key insight is that choosing the column with the most distinct constructors at each step produces trees where each arm's body appears exactly once (referenced by index, never duplicated). This is the algorithm Ori implements. Maranget also formalized the relationship between decision trees and exhaustiveness, showing that `Fail` nodes correspond exactly to non-exhaustive gaps.

### Rust — rustc_pattern_analysis

[Rust's pattern analysis](https://github.com/rust-lang/rust/tree/master/compiler/rustc_pattern_analysis) uses a **usefulness-based** approach rather than decision trees for exhaustiveness checking. Each pattern row is tested for usefulness against the accumulated rows above it. This is more powerful than tree-walking — it can detect subtleties like overlapping integer ranges and nested or-patterns — but is also more complex. Rust compiles match expressions to MIR decision trees separately from the usefulness analysis, using two distinct algorithms for what Ori handles with one.

### Elm — Exhaustiveness on Decision Trees

[Elm](https://github.com/elm/compiler) uses a two-phase approach similar to Ori's: compile patterns to decision trees first, then check exhaustiveness by walking the tree (`compiler/src/Nitpick/PatternMatches.hs`). Elm's exhaustiveness messages are famously readable, using "this match is missing the following patterns:" with complete constructor names and field wildcards — a diagnostic style that Ori's nesting-aware formatter follows.

### GHC — Pattern Match Checking via Refinement Types

[GHC](https://gitlab.haskell.org/ghc/ghc) has evolved through several pattern-checking algorithms. The current approach (since GHC 9.2) uses the [Lower Your Guards](https://dl.acm.org/doi/10.1145/3408989) algorithm by Karachalias et al., which reformulates pattern matching as a constraint satisfaction problem over refinement types. This is dramatically more powerful than tree-walking — it handles GADTs, view patterns, and pattern synonyms — but requires a constraint solver. Ori's simpler tree-walking approach is sufficient for its current pattern language (no GADTs or view patterns).

### Gleam — Custom Exhaustiveness Pass

[Gleam](https://github.com/gleam-lang/gleam) implements exhaustiveness checking independently from pattern compilation (`compiler-core/src/exhaustiveness.rs`). Gleam's checker operates on pattern rows directly rather than compiled decision trees, performing its own matrix specialization. This duplication of the matrix-processing logic is the tradeoff Ori avoids by checking the compiled tree instead.

### Koka — Pattern Compilation via Case Trees

[Koka](https://github.com/koka-lang/koka) compiles pattern matching via case trees in its Core IR (`src/Core/`), following the functional compilation tradition. Koka's algebraic effect system adds a dimension that Ori doesn't face — effect handlers can introduce new matching forms that interact with the pattern compilation. Koka handles this by desugaring effect operations before pattern compilation.

## Design Tradeoffs

**Decision trees vs. backtracking automata.** Ori uses Maranget's decision tree approach, which never duplicates arm bodies (arms are referenced by index). The alternative — backtracking automata (Augustsson style) — can produce more compact code for some patterns but duplicates arm bodies when wildcards span multiple constructor groups. For Ori, where both the evaluator and LLVM codegen consume the same tree, avoiding body duplication is important: each arm body is a `CanId` reference into the `CanArena`, and duplicating it would require duplicating the canonical expression subtree.

**Column selection heuristic.** Ori picks the column with the most distinct constructors. This greedy heuristic isn't optimal in all cases — Maranget's paper discusses alternative heuristics (necessity, "first non-wildcard") that can produce smaller trees for specific pattern shapes. The "most constructors" heuristic works well in practice because it maximizes the splitting power of each test, producing balanced trees for typical code. A more sophisticated heuristic would add complexity for marginal gains on uncommon patterns.

**Exhaustiveness via tree walk vs. separate analysis.** Checking the compiled decision tree for exhaustiveness reuses the compilation work and guarantees consistency — the tree and the check agree on what's covered. The alternative (Rust's approach) is a separate usefulness analysis that can be more powerful (handling subtle overlap and containment relationships) but must be kept in sync with the compilation algorithm. Ori's approach is simpler and sufficient for its current pattern language. If Ori adds features like view patterns or GADTs, a separate analysis may become necessary.

**Flattening as a separate phase.** Separating pattern flattening from compilation means the algorithm operates on owned, self-contained values rather than arena references. This adds a copying step (every pattern is cloned from the arena into a `FlatPattern`) but decouples the compilation algorithm from the AST representation. The copying cost is negligible — patterns are small data structures, and flattening happens once per match expression.

**Path-based binding vs. immediate destructuring.** Ori's decision trees record bindings as `(Name, ScrutineePath)` pairs — the binding says *where* the value is, not *what* it is. The evaluator and codegen interpret these paths at runtime to extract values. An alternative would be to generate destructuring code during compilation (as GHC does when lowering to Core). Path-based binding is more flexible — both backends interpret the same paths — but requires a path interpreter in each backend.

**Guard nodes in the tree vs. guard-free compilation.** Including guards as first-class tree nodes (rather than converting guarded arms to guard-free form) preserves the original program's structure and enables the exhaustiveness checker to reason about guard fallthrough. The tradeoff is that guard nodes add a case to every tree-processing function (evaluator, codegen, exhaustiveness checker). Since guards are common in practice, this cost is justified.

## Related Documents

- [Canonicalization Overview](index.md) — Pipeline position and canonical IR architecture
- [Desugaring](desugaring.md) — Sugar elimination (runs before pattern compilation in the same pass)
- [Constant Folding](constant-folding.md) — Compile-time evaluation (runs alongside pattern compilation)
