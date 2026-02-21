---
title: "Pattern Compilation"
description: "Ori Compiler Design — Decision Tree Construction and Exhaustiveness Checking"
order: 652
section: "Canonicalization"
---

# Pattern Compilation

Pattern compilation transforms `match` expressions from a flat list of pattern → body arms into an efficient decision tree. The decision tree is then checked for exhaustiveness and redundancy.

## Decision Tree Construction

The compiler uses the **Maranget (2008)** algorithm to produce optimal decision trees from pattern matrices.

### Algorithm Overview

1. Build a pattern matrix from the match arms
2. Select the column with the best splitting heuristic
3. Specialize the matrix for each constructor in that column
4. Recurse until all patterns are consumed

### DecisionTree Type

```rust
pub enum DecisionTree {
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

    /// Test a value and branch based on the result.
    Switch {
        path: Vec<PathInstruction>,
        test_kind: TestKind,
        edges: Vec<(TestValue, DecisionTree)>,
        default: Option<Box<DecisionTree>>,
    },

    /// No pattern matches — non-exhaustive error at runtime.
    Fail,
}
```

### Test Kinds

| TestKind | Tests For | Finite? |
|----------|-----------|---------|
| `BoolEq` | `true` / `false` | Yes (2 values) |
| `IntEq` | Integer equality | No |
| `StrEq` | String equality | No |
| `FloatEq` | Float equality | No |
| `IntRange` | Integer range membership | No |
| `ListLen` | List length | No |
| `EnumTag` | Enum variant tag | Yes (known variants) |

### Example

```ori
match shape {
    Circle(r) -> pi * r * r
    Rect(w, h) -> w * h
}
```

Compiles to:

```text
Switch(path=[], test_kind=EnumTag)
├─ Tag(Circle) → Leaf { arm: 0, bindings: [(r, Project(0))] }
└─ Tag(Rect)   → Leaf { arm: 1, bindings: [(w, Project(0)), (h, Project(1))] }
```

## DecisionTreePool

Decision trees are stored in a pool and referenced by `DecisionTreeId`. The pool wraps each tree in `Arc` for O(1) cloning — both the evaluator and codegen share the same tree instances without copying.

```rust
pub struct DecisionTreePool {
    trees: Vec<Arc<DecisionTree>>,
}
```

## Exhaustiveness Checking

After a decision tree is compiled, the `exhaustiveness` module walks the tree to detect two classes of problems:

### Non-Exhaustive Matches

A match is non-exhaustive when some runtime value has no matching arm. This is detected by:

1. **`Fail` nodes**: A reachable `Fail` node means the pattern matrix has a gap
2. **Missing constructors**: A `Switch` with no `default` that doesn't cover all constructors of a finite type

```ori
// Non-exhaustive: missing `false`
match b {
    true -> "yes"
}
// Error: non-exhaustive match, missing: false
```

### Redundant Arms

An arm is redundant when no path through the decision tree reaches it. The checker marks each arm as reachable during the tree walk, then reports any arm that was never marked.

```ori
// Redundant: arm 3 is unreachable
match b {
    true -> "yes"
    false -> "no"
    _ -> "other"   // redundant — bool is fully covered
}
```

### Algorithm

The exhaustiveness checker performs a single walk of the decision tree, carrying the scrutinee type and `Pool` for enum variant resolution:

```text
walk(tree, reachable[], missing[], scrutinee_type, pool, interner)
├─ Leaf { arm_index }      → mark reachable[arm_index] = true
├─ Guard { arm_index, on_fail }
│  ├─ mark reachable[arm_index] = true (guard may succeed)
│  └─ walk(on_fail, ...)                (guard may fail)
├─ Fail                    → missing.push("_")
└─ Switch { path, test_kind, edges, default }
   ├─ walk each edge subtree
   └─ if default: walk default
      else: check_missing_constructors(test_kind, edges, path, missing,
                                       scrutinee_type, pool, interner)
```

### Constructor Coverage

For types with a finite set of constructors:

| Type | Exhaustive When |
|------|----------------|
| `bool` | Both `true` and `false` are covered |
| `Option<T>` | Both `None` and `Some(_)` are covered |
| `Result<T, E>` | Both `Ok(_)` and `Err(_)` are covered |
| User-defined enum | All variants covered (queried via `Pool`) |

For infinite types (`int`, `str`, `float`, ranges, list lengths), a `Switch` without a `default` (wildcard) is always non-exhaustive.

### Implementation Scope

The exhaustiveness checker covers:
- **Bool exhaustiveness**: Both `true`/`false` must be present
- **Infinite type detection**: `int`/`str`/`float`/ranges/list lengths without wildcard
- **Enum variant exhaustiveness** (root-level): User-defined enums, `Option`, `Result` — all variants must be covered or a wildcard present
- **Redundant arm detection**: Arms never reached in the decision tree
- **Guard fallthrough**: Guard failure continues matching (guards not considered exhaustive)
- **Nested switches**: Full tree depth traversal

#### Enum Exhaustiveness

The checker queries the type `Pool` for variant enumeration at root-level `EnumTag` switches:

```rust
fn check_exhaustiveness(
    tree: &DecisionTree,
    arm_count: usize,
    match_span: Span,
    arm_spans: &[Span],
    scrutinee_type: Idx,   // Type of the match scrutinee
    pool: &Pool,           // For enum variant lookup
    interner: &StringInterner,
) -> CheckResult
```

| Type Tag | What's Checked |
|----------|---------------|
| `Tag::Enum` | All variants of user-defined enum covered |
| `Tag::Option` | Both `None` (index 0) and `Some` (index 1) covered |
| `Tag::Result` | Both `Ok` (index 0) and `Err` (index 1) covered |

Missing variants are reported with their names and field wildcards (e.g., `"Rect(_, _)"`).

#### Nested Enum Tracking

The exhaustiveness checker tracks scrutinee types at nested paths via `path_types: FxHashMap<Vec<PathInstruction>, Idx>`. When walking an `EnumTag` switch edge, the checker resolves the type at the current path, extracts variant field types, and inserts child path types for each field. The `walk` function passes this map through recursion, enabling exhaustiveness checking at arbitrary nesting depth — not just the root switch. Path entries are cleaned up after each edge to avoid context leaking between sibling branches.

### Integration

Exhaustiveness checking runs in two contexts:

1. **`lower_match()`** — after compiling each match expression's decision tree
2. **`lower_multi_clause()`** — after synthesizing the match for multi-clause functions

Problems accumulate in `Lowerer.problems` and are returned in `CanonResult.problems`. The `check` command converts these to diagnostics:

```rust
for problem in &canon_result.problems {
    let diag = pattern_problem_to_diagnostic(problem, interner);
    emitter.emit(&diag);
}
```

## PatternProblem Type

```rust
pub enum PatternProblem {
    NonExhaustive {
        match_span: Span,
        missing: Vec<String>,  // e.g., ["false"], ["_"]
    },
    RedundantArm {
        arm_span: Span,
        match_span: Span,
        arm_index: usize,
    },
}
```

These are defined in `ori_ir::canon` so both `ori_canon` (producer) and `oric` (consumer) can access them without circular dependencies.

## Prior Art

| Language | Approach |
|----------|----------|
| **Rust** | `rustc_pattern_analysis` — full usefulness checking on pattern matrices |
| **Elm** | `Reporting/Error/Pattern.hs` — exhaustiveness on decision trees |
| **Gleam** | `exhaustiveness.rs` — custom exhaustiveness pass on pattern rows |
| **Koka** | Pattern matching compiled via case trees in Core |
